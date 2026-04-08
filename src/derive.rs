use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::json;
use time::{Date, Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::config::{AppPaths, Config, RefreshConfig};
use crate::error::{AuthError, Result, RingmasterError};
use crate::oura::models::TagRecord;
use crate::oura::sync::{SyncOptions, sync_once};
use crate::refresh::SyncFamily;
use crate::store::Store;
use crate::store::queries::{
    ContextEventFamily, ContextEventRecord, DailyOverviewRow, DataSufficiency, EffectDirection,
    PatternMetric, PatternRelationWindow, PatternSummaryRecord, RecordCounts, SessionRecord,
    TimeSemantics, WorkoutRecord,
};

const MIN_PATTERN_SAMPLES: usize = 3;
const BASELINE_WINDOW_DAYS: usize = 30;
const FLAT_DELTA_THRESHOLD: f64 = 0.5;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeriveOptions {
    pub demo: bool,
    pub fixture_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeriveReport {
    pub database_path: String,
    pub context_event_count: usize,
    pub pattern_summary_count: usize,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
struct PatternOccurrence {
    family: ContextEventFamily,
    normalized_key: String,
    anchor_day: String,
}

#[derive(Debug, Clone, Default)]
struct DailyMetricRow {
    sleep: Option<f64>,
    readiness: Option<f64>,
    activity: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeriveBounds {
    start_day: String,
    end_day: String,
    note: Option<String>,
}

#[derive(Debug)]
struct TempRootGuard {
    path: PathBuf,
}

pub async fn rebuild(config: &Config, options: DeriveOptions) -> Result<DeriveReport> {
    if options.demo || options.fixture_dir.is_some() {
        let fixture_dir = options
            .fixture_dir
            .clone()
            .or_else(|| config.refresh.demo_fixture_dir.clone())
            .unwrap_or_else(|| PathBuf::from("tests/fixtures/phase3"));
        let temp_root = TempRootGuard::new("derive");
        let mut temp_config = config.clone();
        temp_config.paths = AppPaths::from_roots(
            config.paths.home_dir.clone(),
            temp_root.path().join("config"),
            temp_root.path().join("state"),
            temp_root.path().join("cache"),
        )?;

        let store = Store::open(&temp_config)?;
        let sync_report = sync_once(
            &temp_config,
            &store,
            SyncOptions {
                dry_run: false,
                fixture_dir: Some(fixture_dir.clone()),
                families: SyncFamily::ALL.to_vec(),
                trigger_source: Some("periodic_reconcile".to_owned()),
                trigger_detail: Some("derive seed sync".to_owned()),
            },
        )
        .await?;
        let mut report = rebuild_store(&store)?;
        report.notes.push(format!(
            "Seeded demo store from fixture data at {}.",
            fixture_dir.display()
        ));
        report.notes.push(format!(
            "Imported {} sync slices before derivation.",
            sync_report.slice_reports.len()
        ));
        return Ok(report);
    }

    let store = Store::open(config)?;
    rebuild_store(&store)
}

pub fn rebuild_store(store: &Store) -> Result<DeriveReport> {
    rebuild_store_with_bounds(store, DeriveBounds::full_history())
}

pub fn rebuild_recent_store(store: &Store, config: &Config) -> Result<DeriveReport> {
    let latest_source_day = store.views().latest_source_day()?;
    rebuild_store_with_bounds(
        store,
        DeriveBounds::bounded_refresh(&config.refresh, latest_source_day.as_deref()),
    )
}

fn rebuild_store_with_bounds(store: &Store, bounds: DeriveBounds) -> Result<DeriveReport> {
    let daily_history = store
        .views()
        .daily_history_between_days(&bounds.start_day, &bounds.end_day)?;
    let workouts = store
        .views()
        .workouts_between_days(&bounds.start_day, &bounds.end_day)?;
    let tags = store
        .views()
        .tags_between_days(&bounds.start_day, &bounds.end_day)?;
    let enhanced_tags = store
        .views()
        .enhanced_tags_between_days(&bounds.start_day, &bounds.end_day)?;
    let sessions = store
        .views()
        .sessions_between_days(&bounds.start_day, &bounds.end_day)?;

    let context_events = build_context_events(&workouts, &tags, &enhanced_tags, &sessions)?;
    let pattern_summaries = build_pattern_summaries(&daily_history, &context_events)?;

    store.derived().replace_context_events(&context_events)?;
    store
        .derived()
        .replace_pattern_summaries(&pattern_summaries)?;

    let counts = store.views().record_counts()?;
    let mut notes = derivation_notes(&counts);
    if let Some(note) = bounds.note {
        notes.insert(0, note);
    }
    Ok(DeriveReport {
        database_path: store.plan().db_path.display().to_string(),
        context_event_count: context_events.len(),
        pattern_summary_count: pattern_summaries.len(),
        notes,
    })
}

fn build_context_events(
    workouts: &[WorkoutRecord],
    tags: &[TagRecord],
    enhanced_tags: &[crate::store::queries::EnhancedTagRecord],
    sessions: &[SessionRecord],
) -> Result<Vec<ContextEventRecord>> {
    let updated_at = now_rfc3339()?;
    let mut records = Vec::new();

    for workout in workouts {
        records.push(ContextEventRecord {
            context_event_id: format!("workout:{}", workout.workout_id),
            family: ContextEventFamily::Workout,
            source_id: workout.workout_id.clone(),
            anchor_day: workout.day.clone(),
            start_at: workout.started_at.clone(),
            end_at: workout.ended_at.clone(),
            time_semantics: classify_time_semantics(
                &workout.started_at,
                workout.ended_at.as_deref(),
                false,
            ),
            title: workout.title.clone(),
            subtype: workout.sport.clone().or_else(|| workout.activity.clone()),
            notes: workout.notes.clone(),
            intensity: workout.intensity.clone(),
            metadata_json: serde_json::to_string(&json!({
                "timezone": workout.timezone.clone(),
                "sport": workout.sport.clone(),
                "activity": workout.activity.clone(),
                "source": workout.source.clone(),
            }))?,
            updated_at: updated_at.clone(),
        });
    }

    for tag in tags {
        records.push(ContextEventRecord {
            context_event_id: format!("tag:{}", tag.tag_id),
            family: ContextEventFamily::Tag,
            source_id: tag.tag_id.clone(),
            anchor_day: tag.day.clone(),
            start_at: format!("{}T00:00:00Z", tag.day),
            end_at: Some(format!("{}T23:59:59Z", tag.day)),
            time_semantics: TimeSemantics::AllDay,
            title: tag.label.clone(),
            subtype: Some("legacy_tag".to_owned()),
            notes: None,
            intensity: None,
            metadata_json: serde_json::to_string(&json!({
                "source": "legacy_tag"
            }))?,
            updated_at: updated_at.clone(),
        });
    }

    for tag in enhanced_tags {
        let all_day = tag.started_at.is_none() && tag.ended_at.is_none();
        let start_at = tag
            .started_at
            .clone()
            .unwrap_or_else(|| format!("{}T00:00:00Z", tag.day));
        let end_at = if all_day {
            Some(format!("{}T23:59:59Z", tag.day))
        } else {
            tag.ended_at.clone()
        };

        records.push(ContextEventRecord {
            context_event_id: format!("enhanced_tag:{}", tag.enhanced_tag_id),
            family: ContextEventFamily::EnhancedTag,
            source_id: tag.enhanced_tag_id.clone(),
            anchor_day: tag.day.clone(),
            start_at: start_at.clone(),
            end_at: end_at.clone(),
            time_semantics: classify_time_semantics(&start_at, end_at.as_deref(), all_day),
            title: tag.label.clone(),
            subtype: tag.subtype.clone(),
            notes: tag.comment.clone(),
            intensity: tag.intensity.clone(),
            metadata_json: serde_json::to_string(&json!({
                "comment": tag.comment.clone(),
            }))?,
            updated_at: updated_at.clone(),
        });
    }

    for session in sessions {
        records.push(ContextEventRecord {
            context_event_id: format!("session:{}", session.session_id),
            family: ContextEventFamily::Session,
            source_id: session.session_id.clone(),
            anchor_day: session.day.clone(),
            start_at: session.started_at.clone(),
            end_at: session.ended_at.clone(),
            time_semantics: classify_time_semantics(
                &session.started_at,
                session.ended_at.as_deref(),
                false,
            ),
            title: session.title.clone(),
            subtype: session.kind.clone(),
            notes: session.state.clone(),
            intensity: None,
            metadata_json: serde_json::to_string(&json!({
                "state": session.state.clone(),
                "score": session.score,
            }))?,
            updated_at: updated_at.clone(),
        });
    }

    records.sort_by(|left, right| {
        left.anchor_day
            .cmp(&right.anchor_day)
            .then(left.start_at.cmp(&right.start_at))
            .then(left.context_event_id.cmp(&right.context_event_id))
    });
    Ok(records)
}

fn build_pattern_summaries(
    daily_history: &[DailyOverviewRow],
    context_events: &[ContextEventRecord],
) -> Result<Vec<PatternSummaryRecord>> {
    let metric_history = metric_history_map(daily_history);
    let occurrences = collect_pattern_occurrences(context_events);
    let mut grouped: BTreeMap<
        (
            ContextEventFamily,
            String,
            PatternRelationWindow,
            PatternMetric,
        ),
        Vec<f64>,
    > = BTreeMap::new();

    for occurrence in occurrences {
        for (window, metric, delta) in supported_deltas(&metric_history, &occurrence.anchor_day) {
            grouped
                .entry((
                    occurrence.family,
                    occurrence.normalized_key.clone(),
                    window,
                    metric,
                ))
                .or_default()
                .push(delta);
        }
    }

    let updated_at = now_rfc3339()?;
    let mut records = Vec::new();
    for ((family, normalized_key, relation_window, metric), deltas) in grouped {
        if deltas.len() < MIN_PATTERN_SAMPLES {
            continue;
        }

        let median_delta = median(&deltas);
        let sample_count = u32::try_from(deltas.len()).map_err(|error| {
            RingmasterError::Config(format!("pattern sample count overflowed u32: {error}"))
        })?;
        let confidence = classify_confidence(deltas.len());
        let effect_direction = classify_effect_direction(median_delta);
        let summary_id = format!(
            "{}:{}:{}:{}",
            family.as_str(),
            normalized_key,
            relation_window.as_str(),
            metric.as_str()
        );
        let metadata_json = serde_json::to_string(&json!({
            "aggregation": "median_delta",
            "baseline_window_days": BASELINE_WINDOW_DAYS,
            "min_pattern_samples": MIN_PATTERN_SAMPLES,
            "flat_delta_threshold": FLAT_DELTA_THRESHOLD,
            "deltas": deltas,
        }))?;

        records.push(PatternSummaryRecord {
            summary_id,
            family,
            normalized_key,
            relation_window,
            metric,
            sample_count,
            median_delta,
            effect_direction,
            confidence,
            metadata_json,
            updated_at: updated_at.clone(),
        });
    }

    records.sort_by(|left, right| {
        right
            .confidence
            .cmp(&left.confidence)
            .then(right.sample_count.cmp(&left.sample_count))
            .then_with(|| {
                right
                    .median_delta
                    .abs()
                    .partial_cmp(&left.median_delta.abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then(left.normalized_key.cmp(&right.normalized_key))
    });

    Ok(records)
}

fn metric_history_map(daily_history: &[DailyOverviewRow]) -> BTreeMap<String, DailyMetricRow> {
    daily_history
        .iter()
        .map(|row| {
            (
                row.day.clone(),
                DailyMetricRow {
                    sleep: row.sleep_score.map(f64::from),
                    readiness: row.readiness_score.map(f64::from),
                    activity: row.activity_score.map(f64::from),
                },
            )
        })
        .collect()
}

fn collect_pattern_occurrences(context_events: &[ContextEventRecord]) -> Vec<PatternOccurrence> {
    let mut unique = BTreeSet::new();

    for event in context_events {
        for key in pattern_keys_for_event(event) {
            unique.insert((event.family, key, event.anchor_day.clone()));
        }
    }

    unique
        .into_iter()
        .map(|(family, normalized_key, anchor_day)| PatternOccurrence {
            family,
            normalized_key,
            anchor_day,
        })
        .collect()
}

fn pattern_keys_for_event(event: &ContextEventRecord) -> Vec<String> {
    let metadata = serde_json::from_str::<serde_json::Value>(&event.metadata_json).ok();
    let mut keys = Vec::new();
    match event.family {
        ContextEventFamily::Workout => {
            if let Some(activity) = metadata
                .as_ref()
                .and_then(|value| value.get("activity"))
                .and_then(serde_json::Value::as_str)
            {
                keys.push(format!("activity:{}", normalize_key(activity)));
            }
            if let Some(sport) = metadata
                .as_ref()
                .and_then(|value| value.get("sport"))
                .and_then(serde_json::Value::as_str)
            {
                keys.push(format!("sport:{}", normalize_key(sport)));
            }
            if let Some(intensity) = &event.intensity {
                keys.push(format!("intensity:{}", normalize_key(intensity)));
            }
        }
        ContextEventFamily::EnhancedTag => {
            if let Some(subtype) = &event.subtype {
                keys.push(format!("subtype:{}", normalize_key(subtype)));
            }
            keys.push(format!("label:{}", normalize_key(&event.title)));
        }
        ContextEventFamily::Session => {
            if let Some(subtype) = &event.subtype {
                keys.push(format!("type:{}", normalize_key(subtype)));
            }
        }
        ContextEventFamily::Tag => {
            keys.push(format!("label:{}", normalize_key(&event.title)));
        }
    }

    keys.sort();
    keys.dedup();
    keys
}

fn supported_deltas(
    metric_history: &BTreeMap<String, DailyMetricRow>,
    anchor_day: &str,
) -> Vec<(PatternRelationWindow, PatternMetric, f64)> {
    let mut deltas = Vec::new();

    if let Some(delta) = metric_delta(metric_history, anchor_day, PatternMetric::ActivityScore) {
        deltas.push((
            PatternRelationWindow::SameDayActivity,
            PatternMetric::ActivityScore,
            delta,
        ));
    }

    if let Some(next_day) = shift_day(anchor_day, 1) {
        if let Some(delta) = metric_delta(metric_history, &next_day, PatternMetric::ReadinessScore)
        {
            deltas.push((
                PatternRelationWindow::NextDayReadiness,
                PatternMetric::ReadinessScore,
                delta,
            ));
        }
        if let Some(delta) = metric_delta(metric_history, &next_day, PatternMetric::SleepScore) {
            deltas.push((
                PatternRelationWindow::SameNightSleep,
                PatternMetric::SleepScore,
                delta,
            ));
        }
    }

    deltas
}

fn metric_delta(
    metric_history: &BTreeMap<String, DailyMetricRow>,
    target_day: &str,
    metric: PatternMetric,
) -> Option<f64> {
    let target_value = metric_history
        .get(target_day)
        .and_then(|row| metric_value(row, metric))?;
    let baseline = rolling_baseline(metric_history, target_day, metric)?;
    Some(target_value - baseline)
}

fn rolling_baseline(
    metric_history: &BTreeMap<String, DailyMetricRow>,
    target_day: &str,
    metric: PatternMetric,
) -> Option<f64> {
    let mut prior_values = metric_history
        .iter()
        .filter(|(day, _)| day.as_str() < target_day)
        .filter_map(|(_, row)| metric_value(row, metric))
        .collect::<Vec<_>>();
    if prior_values.len() < MIN_PATTERN_SAMPLES {
        return None;
    }

    if prior_values.len() > BASELINE_WINDOW_DAYS {
        let drain_count = prior_values.len().saturating_sub(BASELINE_WINDOW_DAYS);
        prior_values.drain(..drain_count);
    }

    Some(prior_values.iter().sum::<f64>() / prior_values.len() as f64)
}

fn metric_value(row: &DailyMetricRow, metric: PatternMetric) -> Option<f64> {
    match metric {
        PatternMetric::ActivityScore => row.activity,
        PatternMetric::ReadinessScore => row.readiness,
        PatternMetric::SleepScore => row.sleep,
    }
}

fn shift_day(day: &str, offset_days: i64) -> Option<String> {
    Date::parse(
        day,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .ok()
    .map(|date| (date + Duration::days(offset_days)).to_string())
}

fn median(values: &[f64]) -> f64 {
    let mut values = values.to_vec();
    values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let midpoint = values.len() / 2;

    if values.len().is_multiple_of(2) {
        f64::midpoint(values[midpoint - 1], values[midpoint])
    } else {
        values[midpoint]
    }
}

fn classify_confidence(sample_count: usize) -> DataSufficiency {
    if sample_count >= 10 {
        DataSufficiency::Strong
    } else if sample_count >= 5 {
        DataSufficiency::Medium
    } else {
        DataSufficiency::Thin
    }
}

fn classify_effect_direction(delta: f64) -> EffectDirection {
    if delta >= FLAT_DELTA_THRESHOLD {
        EffectDirection::Higher
    } else if delta <= -FLAT_DELTA_THRESHOLD {
        EffectDirection::Lower
    } else {
        EffectDirection::Flat
    }
}

fn classify_time_semantics(start_at: &str, end_at: Option<&str>, all_day: bool) -> TimeSemantics {
    if all_day {
        TimeSemantics::AllDay
    } else if end_at.is_some_and(|value| value != start_at) {
        TimeSemantics::Interval
    } else {
        TimeSemantics::Point
    }
}

fn normalize_key(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '/', '-'], "_")
}

fn derivation_notes(counts: &RecordCounts) -> Vec<String> {
    vec![
        format!(
            "Derived state rebuilt from {} workouts, {} enhanced tags, {} sessions, and {} legacy tags.",
            counts.workouts, counts.enhanced_tags, counts.sessions, counts.tags
        ),
        "Pattern summaries are descriptive associations only and never causal claims.".to_owned(),
        "Same-night sleep is defined as the sleep row whose closeout day is the day after the event.".to_owned(),
    ]
}

impl DeriveBounds {
    fn full_history() -> Self {
        Self {
            start_day: "0000-01-01".to_owned(),
            end_day: "9999-12-31".to_owned(),
            note: None,
        }
    }

    fn bounded_refresh(refresh: &RefreshConfig, anchor_day: Option<&str>) -> Self {
        let recent_window_days = usize::from(
            refresh
                .daily_history_days
                .max(refresh.workout_history_days)
                .max(refresh.enhanced_tag_history_days)
                .max(refresh.session_history_days),
        )
        .saturating_add(BASELINE_WINDOW_DAYS)
        .saturating_add(1);
        let anchor_date = anchor_day
            .and_then(|day| {
                Date::parse(
                    day,
                    &time::macros::format_description!("[year]-[month]-[day]"),
                )
                .ok()
            })
            .unwrap_or_else(|| OffsetDateTime::now_utc().date());
        let start_day =
            (anchor_date - Duration::days(recent_window_days.saturating_sub(1) as i64)).to_string();
        let end_day = (anchor_date + Duration::days(1)).to_string();

        Self {
            start_day: start_day.clone(),
            end_day: end_day.clone(),
            note: Some(format!(
                "Auto rebuild refreshed a bounded recent window ({start_day}..{end_day}) so sync stays responsive as history grows."
            )),
        }
    }
}

impl TempRootGuard {
    fn new(label: &str) -> Self {
        let timestamp = OffsetDateTime::now_utc()
            .format(&time::macros::format_description!(
                "[year][month][day][hour][minute][second][subsecond digits:6]"
            ))
            .unwrap_or_else(|_| "temp".to_owned());
        let path = std::env::temp_dir().join(format!(
            "ringmaster-{label}-{}-{timestamp}",
            std::process::id()
        ));
        Self { path }
    }

    fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for TempRootGuard {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc().format(&Rfc3339).map_err(|error| {
        AuthError::OAuthFlow(format!("failed to format derive timestamp: {error}")).into()
    })
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::{
        DeriveBounds, TempRootGuard, build_context_events, build_pattern_summaries, rebuild_store,
    };
    use crate::config::RefreshConfig;
    use crate::store::Store;
    use crate::store::queries::{
        ContextEventFamily, DailyActivityRecord, DailyReadinessRecord, DailySleepRecord,
        DataSufficiency, EnhancedTagRecord, ImportStore, PatternMetric, PatternRelationWindow,
        SessionRecord, WorkoutRecord,
    };
    use time::Date;

    fn populate_history(store: &Store) {
        let imports = store.imports();
        let updated_at = "2026-04-08T12:00:00Z".to_owned();
        for (day, sleep, readiness, activity) in [
            ("2026-04-01", 85, 83, 60),
            ("2026-04-02", 84, 82, 61),
            ("2026-04-03", 80, 78, 79),
            ("2026-04-04", 86, 85, 62),
            ("2026-04-05", 78, 74, 81),
            ("2026-04-06", 88, 86, 63),
            ("2026-04-07", 76, 72, 82),
            ("2026-04-08", 83, 79, 64),
        ] {
            upsert_daily_rows(&imports, day, sleep, readiness, activity, &updated_at);
        }

        for workout in [
            WorkoutRecord {
                workout_id: "w1".to_owned(),
                day: "2026-04-02".to_owned(),
                started_at: "2026-04-02T18:00:00Z".to_owned(),
                ended_at: Some("2026-04-02T18:30:00Z".to_owned()),
                timezone: Some("UTC".to_owned()),
                sport: Some("running".to_owned()),
                activity: Some("cardio".to_owned()),
                intensity: Some("moderate".to_owned()),
                title: "Run".to_owned(),
                notes: None,
                source: Some("manual".to_owned()),
                raw_cache_key: None,
                updated_at: updated_at.clone(),
            },
            WorkoutRecord {
                workout_id: "w2".to_owned(),
                day: "2026-04-04".to_owned(),
                started_at: "2026-04-04T18:00:00Z".to_owned(),
                ended_at: Some("2026-04-04T18:30:00Z".to_owned()),
                timezone: Some("UTC".to_owned()),
                sport: Some("running".to_owned()),
                activity: Some("cardio".to_owned()),
                intensity: Some("moderate".to_owned()),
                title: "Run".to_owned(),
                notes: None,
                source: Some("manual".to_owned()),
                raw_cache_key: None,
                updated_at: updated_at.clone(),
            },
            WorkoutRecord {
                workout_id: "w3".to_owned(),
                day: "2026-04-06".to_owned(),
                started_at: "2026-04-06T18:00:00Z".to_owned(),
                ended_at: Some("2026-04-06T18:30:00Z".to_owned()),
                timezone: Some("UTC".to_owned()),
                sport: Some("running".to_owned()),
                activity: Some("cardio".to_owned()),
                intensity: Some("moderate".to_owned()),
                title: "Run".to_owned(),
                notes: None,
                source: Some("manual".to_owned()),
                raw_cache_key: None,
                updated_at: updated_at.clone(),
            },
            WorkoutRecord {
                workout_id: "w4".to_owned(),
                day: "2026-04-08".to_owned(),
                started_at: "2026-04-08T18:00:00Z".to_owned(),
                ended_at: Some("2026-04-08T18:30:00Z".to_owned()),
                timezone: Some("UTC".to_owned()),
                sport: Some("running".to_owned()),
                activity: Some("cardio".to_owned()),
                intensity: Some("moderate".to_owned()),
                title: "Run".to_owned(),
                notes: None,
                source: Some("manual".to_owned()),
                raw_cache_key: None,
                updated_at: updated_at.clone(),
            },
        ] {
            imports
                .upsert_workout(&workout)
                .unwrap_or_else(|error| panic!("workout should insert: {error}"));
        }

        for tag in [
            EnhancedTagRecord {
                enhanced_tag_id: "t1".to_owned(),
                day: "2026-04-01".to_owned(),
                started_at: Some("2026-04-01T21:00:00Z".to_owned()),
                ended_at: Some("2026-04-01T21:00:00Z".to_owned()),
                label: "Late coffee".to_owned(),
                subtype: Some("caffeine".to_owned()),
                comment: None,
                intensity: Some("medium".to_owned()),
                raw_cache_key: None,
                updated_at: updated_at.clone(),
            },
            EnhancedTagRecord {
                enhanced_tag_id: "t2".to_owned(),
                day: "2026-04-03".to_owned(),
                started_at: Some("2026-04-03T21:00:00Z".to_owned()),
                ended_at: Some("2026-04-03T21:00:00Z".to_owned()),
                label: "Late coffee".to_owned(),
                subtype: Some("caffeine".to_owned()),
                comment: None,
                intensity: Some("medium".to_owned()),
                raw_cache_key: None,
                updated_at: updated_at.clone(),
            },
            EnhancedTagRecord {
                enhanced_tag_id: "t3".to_owned(),
                day: "2026-04-05".to_owned(),
                started_at: Some("2026-04-05T21:00:00Z".to_owned()),
                ended_at: Some("2026-04-05T21:00:00Z".to_owned()),
                label: "Late coffee".to_owned(),
                subtype: Some("caffeine".to_owned()),
                comment: None,
                intensity: Some("medium".to_owned()),
                raw_cache_key: None,
                updated_at: updated_at.clone(),
            },
            EnhancedTagRecord {
                enhanced_tag_id: "t4".to_owned(),
                day: "2026-04-07".to_owned(),
                started_at: Some("2026-04-07T21:00:00Z".to_owned()),
                ended_at: Some("2026-04-07T21:00:00Z".to_owned()),
                label: "Late coffee".to_owned(),
                subtype: Some("caffeine".to_owned()),
                comment: None,
                intensity: Some("medium".to_owned()),
                raw_cache_key: None,
                updated_at: updated_at.clone(),
            },
        ] {
            imports
                .upsert_enhanced_tag(&tag)
                .unwrap_or_else(|error| panic!("enhanced tag should insert: {error}"));
        }

        for session in [
            SessionRecord {
                session_id: "s1".to_owned(),
                day: "2026-04-02".to_owned(),
                started_at: "2026-04-02T22:00:00Z".to_owned(),
                ended_at: Some("2026-04-02T22:15:00Z".to_owned()),
                kind: Some("meditation".to_owned()),
                state: Some("completed".to_owned()),
                score: Some(70),
                title: "Meditation".to_owned(),
                raw_cache_key: None,
                updated_at: updated_at.clone(),
            },
            SessionRecord {
                session_id: "s2".to_owned(),
                day: "2026-04-04".to_owned(),
                started_at: "2026-04-04T22:00:00Z".to_owned(),
                ended_at: Some("2026-04-04T22:15:00Z".to_owned()),
                kind: Some("meditation".to_owned()),
                state: Some("completed".to_owned()),
                score: Some(72),
                title: "Meditation".to_owned(),
                raw_cache_key: None,
                updated_at: updated_at.clone(),
            },
            SessionRecord {
                session_id: "s3".to_owned(),
                day: "2026-04-06".to_owned(),
                started_at: "2026-04-06T22:00:00Z".to_owned(),
                ended_at: Some("2026-04-06T22:15:00Z".to_owned()),
                kind: Some("meditation".to_owned()),
                state: Some("completed".to_owned()),
                score: Some(74),
                title: "Meditation".to_owned(),
                raw_cache_key: None,
                updated_at: updated_at.clone(),
            },
            SessionRecord {
                session_id: "s4".to_owned(),
                day: "2026-04-08".to_owned(),
                started_at: "2026-04-08T22:00:00Z".to_owned(),
                ended_at: Some("2026-04-08T22:15:00Z".to_owned()),
                kind: Some("meditation".to_owned()),
                state: Some("completed".to_owned()),
                score: Some(76),
                title: "Meditation".to_owned(),
                raw_cache_key: None,
                updated_at,
            },
        ] {
            imports
                .upsert_session(&session)
                .unwrap_or_else(|error| panic!("session should insert: {error}"));
        }
    }

    fn upsert_daily_rows(
        imports: &ImportStore<'_>,
        day: &str,
        sleep: u8,
        readiness: u8,
        activity: u8,
        updated_at: &str,
    ) {
        imports
            .upsert_daily_sleep(&DailySleepRecord {
                day: day.to_owned(),
                sleep_score: Some(sleep),
                raw_cache_key: None,
                updated_at: updated_at.to_owned(),
            })
            .unwrap_or_else(|error| panic!("sleep row should insert: {error}"));
        imports
            .upsert_daily_readiness(&DailyReadinessRecord {
                day: day.to_owned(),
                readiness_score: Some(readiness),
                temperature_deviation: None,
                temperature_trend_deviation: None,
                raw_cache_key: None,
                updated_at: updated_at.to_owned(),
            })
            .unwrap_or_else(|error| panic!("readiness row should insert: {error}"));
        imports
            .upsert_daily_activity(&DailyActivityRecord {
                day: day.to_owned(),
                activity_score: Some(activity),
                active_calories: 0,
                steps: 0,
                total_calories: 0,
                raw_cache_key: None,
                updated_at: updated_at.to_owned(),
            })
            .unwrap_or_else(|error| panic!("activity row should insert: {error}"));
    }

    #[test]
    fn context_events_preserve_family_and_time_semantics() {
        let workouts = vec![WorkoutRecord {
            workout_id: "w1".to_owned(),
            day: "2026-04-08".to_owned(),
            started_at: "2026-04-08T18:00:00Z".to_owned(),
            ended_at: Some("2026-04-08T18:45:00Z".to_owned()),
            timezone: Some("UTC".to_owned()),
            sport: Some("running".to_owned()),
            activity: Some("cardio".to_owned()),
            intensity: Some("moderate".to_owned()),
            title: "Run".to_owned(),
            notes: Some("steady".to_owned()),
            source: Some("manual".to_owned()),
            raw_cache_key: None,
            updated_at: "2026-04-08T12:00:00Z".to_owned(),
        }];

        let events = build_context_events(&workouts, &[], &[], &[])
            .unwrap_or_else(|error| panic!("context events should derive: {error}"));

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].family, ContextEventFamily::Workout);
        assert_eq!(
            events[0].time_semantics,
            crate::store::queries::TimeSemantics::Interval
        );
        assert_eq!(events[0].title, "Run");
    }

    #[test]
    fn pattern_summaries_require_enough_samples_and_report_confidence() {
        let store = Store::open_in_memory()
            .unwrap_or_else(|error| panic!("store should open in memory: {error}"));
        populate_history(&store);

        let context_events = build_context_events(
            &store
                .views()
                .workouts_between_days("0000-01-01", "9999-12-31")
                .unwrap_or_else(|error| panic!("workouts should load: {error}")),
            &[],
            &store
                .views()
                .enhanced_tags_between_days("0000-01-01", "9999-12-31")
                .unwrap_or_else(|error| panic!("enhanced tags should load: {error}")),
            &store
                .views()
                .sessions_between_days("0000-01-01", "9999-12-31")
                .unwrap_or_else(|error| panic!("sessions should load: {error}")),
        )
        .unwrap_or_else(|error| panic!("context events should derive: {error}"));
        let patterns = build_pattern_summaries(
            &store
                .views()
                .daily_history_all()
                .unwrap_or_else(|error| panic!("daily history should load: {error}")),
            &context_events,
        )
        .unwrap_or_else(|error| panic!("patterns should derive: {error}"));

        assert!(patterns.iter().any(|summary| {
            summary.family == ContextEventFamily::Workout
                && summary.normalized_key == "sport:running"
                && summary.relation_window == PatternRelationWindow::SameDayActivity
                && summary.metric == PatternMetric::ActivityScore
                && summary.sample_count >= 3
                && summary.confidence == DataSufficiency::Thin
        }));
    }

    #[test]
    fn rebuild_store_persists_derived_tables() {
        let store = Store::open_in_memory()
            .unwrap_or_else(|error| panic!("store should open in memory: {error}"));
        populate_history(&store);

        let report =
            rebuild_store(&store).unwrap_or_else(|error| panic!("rebuild should succeed: {error}"));

        assert!(report.context_event_count >= 9);
        assert!(report.pattern_summary_count > 0);
        assert_eq!(
            store
                .views()
                .record_counts()
                .unwrap_or_else(|error| panic!("counts should load: {error}"))
                .derived_context_events,
            report.context_event_count as u64
        );
    }

    #[test]
    fn bounded_refresh_extends_window_for_baseline_history() {
        let refresh = RefreshConfig {
            personal_interval_secs: 3_600,
            daily_interval_secs: 300,
            heartrate_interval_secs: 60,
            workout_interval_secs: 600,
            enhanced_tag_interval_secs: 300,
            session_interval_secs: 300,
            personal_stale_after_secs: 72 * 60 * 60,
            daily_stale_after_secs: 12 * 60 * 60,
            heartrate_stale_after_secs: 15 * 60,
            workout_stale_after_secs: 24 * 60 * 60,
            enhanced_tag_stale_after_secs: 12 * 60 * 60,
            session_stale_after_secs: 12 * 60 * 60,
            daily_history_days: 14,
            daily_overlap_days: 2,
            heartrate_history_days: 7,
            heartrate_overlap_minutes: 60,
            workout_history_days: 90,
            workout_overlap_days: 2,
            enhanced_tag_history_days: 45,
            enhanced_tag_overlap_days: 2,
            session_history_days: 30,
            session_overlap_days: 2,
            max_backoff_secs: 60 * 60,
            demo_fixture_dir: None,
        };

        let bounds = DeriveBounds::bounded_refresh(&refresh, Some("2026-04-08"));
        let start = Date::parse(
            &bounds.start_day,
            &time::macros::format_description!("[year]-[month]-[day]"),
        )
        .unwrap_or_else(|error| panic!("start day should parse: {error}"));
        let end = Date::parse(
            &bounds.end_day,
            &time::macros::format_description!("[year]-[month]-[day]"),
        )
        .unwrap_or_else(|error| panic!("end day should parse: {error}"));

        assert!(end > start);
        assert!(
            (end - start).whole_days() >= 90 + 30,
            "window should include history plus baseline"
        );
        assert!(
            bounds
                .note
                .as_deref()
                .is_some_and(|note| note.contains("bounded recent window"))
        );
    }

    #[test]
    fn temp_root_guard_cleans_up_when_dropped() {
        let path = {
            let guard = TempRootGuard::new("test");
            let path = guard.path().clone();
            std::fs::create_dir_all(&path)
                .unwrap_or_else(|error| panic!("temp directory should create: {error}"));
            std::fs::write(path.join("sentinel.txt"), "ok")
                .unwrap_or_else(|error| panic!("sentinel should write: {error}"));
            path
        };

        assert!(
            !path.exists(),
            "temp root should be removed when the guard drops"
        );
    }
}
