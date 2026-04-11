use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::json;
use time::{Date, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::error::{Result, RingmasterError};
use crate::review::registry::{SignalDirectionality, signal_definitions};
use crate::store::queries::{
    DailyActivityRecord, DailyCardiovascularAgeRecord, DailyOverviewRow, DailyReadinessRecord,
    DailyResilienceRecord, DailyStressRecord, RestModePeriodRecord, ReviewSignalDayRecord,
    SleepTimeRecord, Vo2MaxRecord,
};

const COMPARABLE_MEDIUM_DAYS: usize = 7;
const COMPARABLE_STRONG_DAYS: usize = 14;
const COMPARABLE_MEDIUM_WEEKS: usize = 2;
const COMPARABLE_STRONG_WEEKS: usize = 4;
const MIN_STDDEV: f64 = 0.01;
const PERSISTENCE_Z_THRESHOLD: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReviewSufficiency {
    Missing,
    Thin,
    Medium,
    Strong,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeatureInputs<'a> {
    pub daily_history: &'a [DailyOverviewRow],
    pub daily_activity: &'a [DailyActivityRecord],
    pub daily_readiness: &'a [DailyReadinessRecord],
    pub daily_stress: &'a [DailyStressRecord],
    pub daily_resilience: &'a [DailyResilienceRecord],
    pub daily_cardiovascular_age: &'a [DailyCardiovascularAgeRecord],
    pub vo2_max: &'a [Vo2MaxRecord],
    pub sleep_time: &'a [SleepTimeRecord],
    pub rest_mode_periods: &'a [RestModePeriodRecord],
    pub captured_at: &'a str,
}

#[derive(Debug, Clone, PartialEq)]
struct SeedPoint {
    day: String,
    numeric_value: Option<f64>,
    text_value: Option<String>,
    metadata_json: String,
}

#[derive(Debug, Clone, PartialEq)]
struct NumericPoint {
    day: String,
    date: Date,
    value: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct ComparableStats {
    count: usize,
    mean: Option<f64>,
    stddev: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct NumericSeriesIndex {
    points: Vec<NumericPoint>,
    day_to_index: BTreeMap<String, usize>,
    prefix_sums: Vec<f64>,
    prefix_squared_sums: Vec<f64>,
}

impl ReviewSufficiency {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Thin => "thin",
            Self::Medium => "medium",
            Self::Strong => "strong",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Missing => "Missing",
            Self::Thin => "Thin",
            Self::Medium => "Medium",
            Self::Strong => "Strong",
        }
    }

    #[must_use]
    pub const fn from_comparable_days(comparable_days: usize) -> Self {
        if comparable_days == 0 {
            Self::Missing
        } else if comparable_days < COMPARABLE_MEDIUM_DAYS {
            Self::Thin
        } else if comparable_days < COMPARABLE_STRONG_DAYS {
            Self::Medium
        } else {
            Self::Strong
        }
    }

    #[must_use]
    pub const fn from_comparable_weeks(comparable_weeks: usize) -> Self {
        if comparable_weeks == 0 {
            Self::Missing
        } else if comparable_weeks < COMPARABLE_MEDIUM_WEEKS {
            Self::Thin
        } else if comparable_weeks < COMPARABLE_STRONG_WEEKS {
            Self::Medium
        } else {
            Self::Strong
        }
    }
}

/// # Errors
///
/// Returns an error if source timestamps cannot be parsed or signal metadata cannot be serialized.
pub fn build_review_signal_days(inputs: &FeatureInputs<'_>) -> Result<Vec<ReviewSignalDayRecord>> {
    let captured_day = parse_day(&captured_day(inputs.captured_at)?)?;
    let mut series: BTreeMap<&'static str, Vec<SeedPoint>> = BTreeMap::new();

    for row in inputs.daily_history {
        insert_numeric_seed(
            &mut series,
            "sleep_score",
            &row.day,
            row.sleep_score.map(f64::from),
            &json!({ "source_family": "daily_sleep" }),
        )?;
        insert_numeric_seed(
            &mut series,
            "readiness_score",
            &row.day,
            row.readiness_score.map(f64::from),
            &json!({ "source_family": "daily_readiness" }),
        )?;
        insert_numeric_seed(
            &mut series,
            "activity_score",
            &row.day,
            row.activity_score.map(f64::from),
            &json!({ "source_family": "daily_activity" }),
        )?;
    }

    for row in inputs.daily_activity {
        insert_numeric_seed(
            &mut series,
            "active_calories",
            &row.day,
            Some(crate::numeric::i64_to_f64(row.active_calories)),
            &json!({ "source_family": "daily_activity" }),
        )?;
        insert_numeric_seed(
            &mut series,
            "steps",
            &row.day,
            Some(crate::numeric::i64_to_f64(row.steps)),
            &json!({ "source_family": "daily_activity" }),
        )?;
    }

    for row in inputs.daily_readiness {
        insert_numeric_seed(
            &mut series,
            "temperature_deviation",
            &row.day,
            row.temperature_deviation,
            &json!({
                "source_family": "daily_readiness",
                "temperature_trend_deviation": row.temperature_trend_deviation,
            }),
        )?;
    }

    for row in inputs.daily_stress {
        insert_numeric_seed(
            &mut series,
            "stress_high",
            &row.day,
            row.stress_high.map(crate::numeric::i64_to_f64),
            &json!({
                "source_family": "daily_stress",
                "day_summary": row.day_summary,
            }),
        )?;
        insert_numeric_seed(
            &mut series,
            "recovery_high",
            &row.day,
            row.recovery_high.map(crate::numeric::i64_to_f64),
            &json!({
                "source_family": "daily_stress",
                "day_summary": row.day_summary,
            }),
        )?;
    }

    for row in inputs.daily_resilience {
        insert_numeric_seed(
            &mut series,
            "resilience_level",
            &row.day,
            Some(f64::from(resilience_level_score(&row.level))),
            &json!({
                "source_family": "daily_resilience",
                "level": row.level,
            }),
        )?;
        insert_numeric_seed(
            &mut series,
            "sleep_recovery",
            &row.day,
            Some(row.sleep_recovery),
            &json!({ "source_family": "daily_resilience" }),
        )?;
        insert_numeric_seed(
            &mut series,
            "daytime_recovery",
            &row.day,
            Some(row.daytime_recovery),
            &json!({ "source_family": "daily_resilience" }),
        )?;
        insert_numeric_seed(
            &mut series,
            "resilience_stress",
            &row.day,
            Some(row.stress),
            &json!({ "source_family": "daily_resilience" }),
        )?;
    }

    for row in inputs.daily_cardiovascular_age {
        insert_numeric_seed(
            &mut series,
            "cardiovascular_age",
            &row.day,
            row.vascular_age.map(crate::numeric::i64_to_f64),
            &json!({ "source_family": "daily_cardiovascular_age" }),
        )?;
    }

    for row in inputs.vo2_max {
        insert_numeric_seed(
            &mut series,
            "vo2_max",
            &row.day,
            row.vo2_max,
            &json!({
                "source_family": "vo2_max",
                "recorded_at": row.recorded_at,
            }),
        )?;
    }

    for row in inputs.sleep_time {
        insert_text_seed(
            &mut series,
            "sleep_time_status",
            &row.day,
            row.status.clone(),
            &json!({
                "source_family": "sleep_time",
                "recommendation": row.recommendation,
                "optimal_bedtime_start_offset": row.optimal_bedtime_start_offset,
                "optimal_bedtime_end_offset": row.optimal_bedtime_end_offset,
                "optimal_bedtime_day_tz": row.optimal_bedtime_day_tz,
            }),
        )?;
    }

    let rest_days = expand_rest_mode_days(inputs.rest_mode_periods, captured_day)?;
    for day in rest_days {
        insert_numeric_seed(
            &mut series,
            "rest_mode_active",
            &day,
            Some(1.0),
            &json!({ "source_family": "rest_mode_period" }),
        )?;
    }

    let mut rows = Vec::new();
    for definition in signal_definitions() {
        let Some(seed_points) = series.get(definition.key) else {
            continue;
        };
        let numeric_series = NumericSeriesIndex::build(seed_points)?;

        for seed in seed_points {
            let day = parse_day(&seed.day)?;
            let stale_days = (captured_day - day).whole_days().max(0);
            let comparable_stats =
                numeric_series.comparable_stats(&seed.day, definition.baseline_window_days);
            let sufficiency = ReviewSufficiency::from_comparable_days(comparable_stats.count);
            let baseline_mean = comparable_stats.mean;
            let baseline_stddev = comparable_stats.stddev;
            let delta = match (seed.numeric_value, baseline_mean) {
                (Some(numeric_value), Some(mean_value)) => Some(numeric_value - mean_value),
                _ => None,
            };
            let z_score = match (delta, baseline_stddev) {
                (Some(delta_value), Some(stddev)) if stddev >= MIN_STDDEV => {
                    Some(delta_value / stddev)
                }
                _ => None,
            };
            let persistence_days = persistence_days(
                &numeric_series,
                &seed.day,
                z_score,
                definition.directionality,
                definition.baseline_window_days,
            )?;
            let metadata_json = serde_json::to_string(&json!({
                "comparable_days": comparable_stats.count,
                "source_family": definition.family,
                "seed_metadata": serde_json::from_str::<serde_json::Value>(&seed.metadata_json)
                    .unwrap_or_else(|_| json!({})),
                "wording_constraint": definition.wording_constraint,
            }))?;

            rows.push(ReviewSignalDayRecord {
                signal_key: definition.key.to_owned(),
                day: seed.day.clone(),
                numeric_value: seed.numeric_value,
                text_value: seed.text_value.clone(),
                baseline_mean,
                baseline_stddev,
                delta,
                z_score,
                persistence_days,
                sufficiency,
                stale_days: u32::try_from(stale_days).map_err(|error| {
                    RingmasterError::Config(format!(
                        "stale_days overflowed u32 for {} {}: {error}",
                        definition.key, seed.day
                    ))
                })?,
                metadata_json,
                updated_at: inputs.captured_at.to_owned(),
            });
        }
    }

    rows.sort_by(|left, right| {
        left.day
            .cmp(&right.day)
            .then(left.signal_key.cmp(&right.signal_key))
    });
    Ok(rows)
}

fn insert_numeric_seed(
    series: &mut BTreeMap<&'static str, Vec<SeedPoint>>,
    signal_key: &'static str,
    day: &str,
    numeric_value: Option<f64>,
    metadata: &serde_json::Value,
) -> Result<()> {
    let seed_point = SeedPoint {
        day: day.to_owned(),
        numeric_value,
        text_value: None,
        metadata_json: serde_json::to_string(metadata)?,
    };
    upsert_seed_point(series.entry(signal_key).or_default(), seed_point);
    Ok(())
}

fn insert_text_seed(
    series: &mut BTreeMap<&'static str, Vec<SeedPoint>>,
    signal_key: &'static str,
    day: &str,
    text_value: Option<String>,
    metadata: &serde_json::Value,
) -> Result<()> {
    let seed_point = SeedPoint {
        day: day.to_owned(),
        numeric_value: None,
        text_value,
        metadata_json: serde_json::to_string(metadata)?,
    };
    upsert_seed_point(series.entry(signal_key).or_default(), seed_point);
    Ok(())
}

fn upsert_seed_point(seed_points: &mut Vec<SeedPoint>, seed_point: SeedPoint) {
    if let Some(existing) = seed_points
        .iter_mut()
        .find(|existing| existing.day == seed_point.day)
    {
        *existing = seed_point;
    } else {
        seed_points.push(seed_point);
    }
}

fn persistence_days(
    numeric_series: &NumericSeriesIndex,
    anchor_day: &str,
    z_score: Option<f64>,
    directionality: SignalDirectionality,
    baseline_window_days: usize,
) -> Result<u32> {
    let Some(anchor_sign) = signal_sign(z_score, directionality) else {
        return Ok(0);
    };
    let mut streak = 0_u32;
    let mut current_day = parse_day(anchor_day)?;

    loop {
        let day_label = current_day.to_string();
        let Some((value, comparable_stats)) =
            numeric_series.value_and_comparable_stats(&day_label, baseline_window_days)
        else {
            break;
        };
        let day_z_score = match (comparable_stats.mean, comparable_stats.stddev) {
            (Some(mean_value), Some(stddev)) if stddev >= MIN_STDDEV => {
                Some((value - mean_value) / stddev)
            }
            _ => None,
        };
        let Some(day_sign) = signal_sign(day_z_score, directionality) else {
            break;
        };
        if day_sign != anchor_sign {
            break;
        }

        streak = streak.saturating_add(1);
        current_day = current_day.previous_day().ok_or_else(|| {
            RingmasterError::Config(
                "review persistence walked before supported date range".to_owned(),
            )
        })?;
    }

    Ok(streak)
}

impl NumericSeriesIndex {
    fn build(seed_points: &[SeedPoint]) -> Result<Self> {
        let mut points = seed_points
            .iter()
            .filter_map(|seed| seed.numeric_value.map(|value| (seed, value)))
            .map(|(seed, value)| {
                Ok(NumericPoint {
                    day: seed.day.clone(),
                    date: parse_day(&seed.day)?,
                    value,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        points.sort_by(|left, right| left.date.cmp(&right.date).then(left.day.cmp(&right.day)));

        let day_to_index = points
            .iter()
            .enumerate()
            .map(|(index, point)| (point.day.clone(), index))
            .collect::<BTreeMap<_, _>>();

        let mut prefix_sums = Vec::with_capacity(points.len().saturating_add(1));
        let mut prefix_squared_sums = Vec::with_capacity(points.len().saturating_add(1));
        prefix_sums.push(0.0);
        prefix_squared_sums.push(0.0);

        let mut running_sum = 0.0;
        let mut running_squared_sum = 0.0;
        for point in &points {
            running_sum += point.value;
            running_squared_sum += point.value.powi(2);
            prefix_sums.push(running_sum);
            prefix_squared_sums.push(running_squared_sum);
        }

        Ok(Self {
            points,
            day_to_index,
            prefix_sums,
            prefix_squared_sums,
        })
    }

    fn comparable_stats(&self, day: &str, baseline_window_days: usize) -> ComparableStats {
        self.day_to_index
            .get(day)
            .copied()
            .map(|index| self.comparable_stats_for_index(index, baseline_window_days))
            .unwrap_or_default()
    }

    fn value_and_comparable_stats(
        &self,
        day: &str,
        baseline_window_days: usize,
    ) -> Option<(f64, ComparableStats)> {
        let index = self.day_to_index.get(day).copied()?;
        Some((
            self.points.get(index)?.value,
            self.comparable_stats_for_index(index, baseline_window_days),
        ))
    }

    fn comparable_stats_for_index(
        &self,
        target_index: usize,
        baseline_window_days: usize,
    ) -> ComparableStats {
        let start_index = self.window_start_index(target_index, baseline_window_days);
        let count = target_index.saturating_sub(start_index);
        if count == 0 {
            return ComparableStats::default();
        }

        let sum = self.prefix_sums[target_index] - self.prefix_sums[start_index];
        let mean = sum / crate::numeric::usize_to_f64(count);
        let squared_sum =
            self.prefix_squared_sums[target_index] - self.prefix_squared_sums[start_index];
        let variance = mean.mul_add(-mean, squared_sum / crate::numeric::usize_to_f64(count));

        ComparableStats {
            count,
            mean: Some(mean),
            stddev: Some(variance.max(0.0).sqrt()),
        }
    }

    fn window_start_index(&self, target_index: usize, baseline_window_days: usize) -> usize {
        if target_index == 0 || baseline_window_days == 0 {
            return target_index;
        }

        let Some(window_days) = i64::try_from(baseline_window_days).ok() else {
            return 0;
        };
        let Some(window_start) = self.points[target_index]
            .date
            .checked_sub(time::Duration::days(window_days))
        else {
            return 0;
        };

        self.points[..target_index].partition_point(|point| point.date < window_start)
    }
}

fn signal_sign(z_score: Option<f64>, directionality: SignalDirectionality) -> Option<i8> {
    let z_score = z_score?;
    if z_score.abs() < PERSISTENCE_Z_THRESHOLD {
        return None;
    }

    match directionality {
        SignalDirectionality::HigherBetter => Some(if z_score.is_sign_positive() { 1 } else { -1 }),
        SignalDirectionality::LowerBetter => Some(if z_score.is_sign_positive() { -1 } else { 1 }),
        SignalDirectionality::Neutral | SignalDirectionality::Contextual => {
            Some(if z_score.is_sign_positive() { 1 } else { -1 })
        }
    }
}

fn resilience_level_score(level: &str) -> u8 {
    match level {
        "limited" => 1,
        "adequate" => 2,
        "solid" => 3,
        "strong" => 4,
        "exceptional" => 5,
        _ => 0,
    }
}

fn expand_rest_mode_days(
    rest_mode_periods: &[RestModePeriodRecord],
    captured_day: Date,
) -> Result<Vec<String>> {
    let mut days = BTreeSet::new();

    for period in rest_mode_periods {
        let start_day = parse_day(&period.start_day)?;
        let end_day = period
            .end_day
            .as_deref()
            .map(parse_day)
            .transpose()?
            .unwrap_or(captured_day);
        let mut current_day = start_day;
        loop {
            days.insert(current_day.to_string());
            if current_day >= end_day {
                break;
            }
            current_day = current_day.next_day().ok_or_else(|| {
                RingmasterError::Config(
                    "rest mode expansion exceeded supported date range".to_owned(),
                )
            })?;
        }
    }

    Ok(days.into_iter().collect())
}

fn captured_day(timestamp: &str) -> Result<String> {
    let parsed = OffsetDateTime::parse(timestamp, &Rfc3339).map_err(|error| {
        RingmasterError::Config(format!(
            "failed to parse captured_at timestamp `{timestamp}` for review features: {error}"
        ))
    })?;
    Ok(parsed.date().to_string())
}

fn parse_day(day: &str) -> Result<Date> {
    Date::parse(
        day,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .map_err(|error| {
        RingmasterError::Config(format!(
            "failed to parse day `{day}` for review features: {error}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::{
        FeatureInputs, NumericSeriesIndex, ReviewSufficiency, SeedPoint, build_review_signal_days,
        persistence_days, signal_sign,
    };
    use crate::review::registry::SignalDirectionality;
    use crate::review::registry::signal_definition;
    use crate::store::queries::{
        DailyActivityRecord, DailyCardiovascularAgeRecord, DailyOverviewRow, DailyReadinessRecord,
        DailyResilienceRecord, DailyStressRecord, RestModePeriodRecord, SleepTimeRecord,
        Vo2MaxRecord,
    };

    #[test]
    fn sufficiency_bucket_thresholds_are_explicit() {
        assert_eq!(
            ReviewSufficiency::from_comparable_days(0),
            ReviewSufficiency::Missing
        );
        assert_eq!(
            ReviewSufficiency::from_comparable_days(3),
            ReviewSufficiency::Thin
        );
        assert_eq!(
            ReviewSufficiency::from_comparable_days(8),
            ReviewSufficiency::Medium
        );
        assert_eq!(
            ReviewSufficiency::from_comparable_days(20),
            ReviewSufficiency::Strong
        );
        assert_eq!(
            ReviewSufficiency::from_comparable_weeks(0),
            ReviewSufficiency::Missing
        );
        assert_eq!(
            ReviewSufficiency::from_comparable_weeks(1),
            ReviewSufficiency::Thin
        );
        assert_eq!(
            ReviewSufficiency::from_comparable_weeks(3),
            ReviewSufficiency::Medium
        );
        assert_eq!(
            ReviewSufficiency::from_comparable_weeks(5),
            ReviewSufficiency::Strong
        );
    }

    fn make_daily_overview_row(
        day: &str,
        sleep_score: u8,
        readiness_score: u8,
        activity_score: u8,
    ) -> DailyOverviewRow {
        DailyOverviewRow {
            day: day.to_owned(),
            sleep_score: Some(sleep_score),
            readiness_score: Some(readiness_score),
            activity_score: Some(activity_score),
            updated_at: format!("{day}T08:00:00Z"),
        }
    }

    fn make_daily_activity_record() -> DailyActivityRecord {
        DailyActivityRecord {
            oura_id: Some("act-1".to_owned()),
            day: "2026-04-02".to_owned(),
            activity_score: Some(70),
            active_calories: 400,
            steps: 5000,
            total_calories: 2200,
            raw_cache_key: None,
            updated_at: "2026-04-02T08:00:00Z".to_owned(),
        }
    }

    fn make_daily_readiness_record() -> DailyReadinessRecord {
        DailyReadinessRecord {
            oura_id: Some("ready-1".to_owned()),
            day: "2026-04-02".to_owned(),
            readiness_score: Some(76),
            temperature_deviation: Some(0.4),
            temperature_trend_deviation: Some(0.2),
            raw_cache_key: None,
            updated_at: "2026-04-02T08:00:00Z".to_owned(),
        }
    }

    fn make_daily_stress_record() -> DailyStressRecord {
        DailyStressRecord {
            oura_id: Some("stress-1".to_owned()),
            day: "2026-04-02".to_owned(),
            stress_high: Some(180),
            recovery_high: Some(40),
            day_summary: Some("stressed".to_owned()),
            raw_cache_key: None,
            updated_at: "2026-04-02T08:00:00Z".to_owned(),
        }
    }

    fn make_daily_resilience_record() -> DailyResilienceRecord {
        DailyResilienceRecord {
            oura_id: Some("res-1".to_owned()),
            day: "2026-04-02".to_owned(),
            level: "solid".to_owned(),
            sleep_recovery: 78.0,
            daytime_recovery: 64.0,
            stress: 55.0,
            raw_cache_key: None,
            updated_at: "2026-04-02T08:00:00Z".to_owned(),
        }
    }

    fn make_daily_cardiovascular_age_record() -> DailyCardiovascularAgeRecord {
        DailyCardiovascularAgeRecord {
            day: "2026-04-02".to_owned(),
            vascular_age: Some(37),
            raw_cache_key: None,
            updated_at: "2026-04-02T08:00:00Z".to_owned(),
        }
    }

    fn make_vo2_max_record() -> Vo2MaxRecord {
        Vo2MaxRecord {
            oura_id: Some("vo2-1".to_owned()),
            day: "2026-04-02".to_owned(),
            recorded_at: "2026-04-02T08:00:00Z".to_owned(),
            vo2_max: Some(42.5),
            raw_cache_key: None,
            updated_at: "2026-04-02T08:00:00Z".to_owned(),
        }
    }

    fn make_sleep_time_record() -> SleepTimeRecord {
        SleepTimeRecord {
            oura_id: Some("sleep-time-1".to_owned()),
            day: "2026-04-02".to_owned(),
            status: Some("optimal_found".to_owned()),
            recommendation: Some("follow_optimal_bedtime".to_owned()),
            optimal_bedtime_start_offset: Some(79200),
            optimal_bedtime_end_offset: Some(82800),
            optimal_bedtime_day_tz: Some(0),
            raw_cache_key: None,
            updated_at: "2026-04-02T08:00:00Z".to_owned(),
        }
    }

    fn make_rest_mode_period_record() -> RestModePeriodRecord {
        RestModePeriodRecord {
            period_id: "rest-1".to_owned(),
            start_day: "2026-04-02".to_owned(),
            start_time: Some("2026-04-02T00:00:00Z".to_owned()),
            end_day: Some("2026-04-03".to_owned()),
            end_time: Some("2026-04-03T12:00:00Z".to_owned()),
            episode_count: 1,
            tags_json: "[]".to_owned(),
            raw_cache_key: None,
            updated_at: "2026-04-02T08:00:00Z".to_owned(),
        }
    }

    #[test]
    fn feature_builder_emits_phase5_signal_rows() {
        let daily_history = vec![
            make_daily_overview_row("2026-04-01", 80, 82, 79),
            make_daily_overview_row("2026-04-02", 74, 76, 70),
        ];

        let rows = build_review_signal_days(&FeatureInputs {
            daily_history: &daily_history,
            daily_activity: &[make_daily_activity_record()],
            daily_readiness: &[make_daily_readiness_record()],
            daily_stress: &[make_daily_stress_record()],
            daily_resilience: &[make_daily_resilience_record()],
            daily_cardiovascular_age: &[make_daily_cardiovascular_age_record()],
            vo2_max: &[make_vo2_max_record()],
            sleep_time: &[make_sleep_time_record()],
            rest_mode_periods: &[make_rest_mode_period_record()],
            captured_at: "2026-04-03T10:00:00Z",
        })
        .unwrap_or_else(|error| panic!("feature build should succeed: {error}"));

        for key in [
            "sleep_score",
            "readiness_score",
            "stress_high",
            "resilience_level",
            "cardiovascular_age",
            "vo2_max",
            "sleep_time_status",
            "rest_mode_active",
        ] {
            assert!(
                rows.iter().any(|row| row.signal_key == key),
                "expected derived review signal row for {key}"
            );
            assert!(
                signal_definition(key).is_some(),
                "{key} should exist in the registry"
            );
        }
    }

    #[test]
    fn feature_builder_keeps_latest_vo2_max_per_day() {
        let rows = build_review_signal_days(&FeatureInputs {
            daily_history: &[],
            daily_activity: &[],
            daily_readiness: &[],
            daily_stress: &[],
            daily_resilience: &[],
            daily_cardiovascular_age: &[],
            vo2_max: &[
                Vo2MaxRecord {
                    oura_id: Some("vo2-1".to_owned()),
                    day: "2026-04-02".to_owned(),
                    recorded_at: "2026-04-02T08:00:00Z".to_owned(),
                    vo2_max: Some(41.5),
                    raw_cache_key: None,
                    updated_at: "2026-04-02T08:00:00Z".to_owned(),
                },
                Vo2MaxRecord {
                    oura_id: Some("vo2-2".to_owned()),
                    day: "2026-04-02".to_owned(),
                    recorded_at: "2026-04-02T12:00:00Z".to_owned(),
                    vo2_max: Some(42.0),
                    raw_cache_key: None,
                    updated_at: "2026-04-02T12:00:00Z".to_owned(),
                },
            ],
            sleep_time: &[],
            rest_mode_periods: &[],
            captured_at: "2026-04-03T10:00:00Z",
        })
        .unwrap_or_else(|error| panic!("feature build should succeed: {error}"));

        let vo2_rows = rows
            .iter()
            .filter(|row| row.signal_key == "vo2_max" && row.day == "2026-04-02")
            .collect::<Vec<_>>();

        assert_eq!(vo2_rows.len(), 1);
        assert_eq!(vo2_rows[0].numeric_value, Some(42.0));
    }

    #[test]
    fn feature_builder_expands_open_rest_mode_through_captured_day() {
        let rows = build_review_signal_days(&FeatureInputs {
            daily_history: &[],
            daily_activity: &[],
            daily_readiness: &[],
            daily_stress: &[],
            daily_resilience: &[],
            daily_cardiovascular_age: &[],
            vo2_max: &[],
            sleep_time: &[],
            rest_mode_periods: &[RestModePeriodRecord {
                period_id: "rest-open".to_owned(),
                start_day: "2026-04-02".to_owned(),
                start_time: Some("2026-04-02T00:00:00Z".to_owned()),
                end_day: None,
                end_time: None,
                episode_count: 1,
                tags_json: "[]".to_owned(),
                raw_cache_key: None,
                updated_at: "2026-04-02T08:00:00Z".to_owned(),
            }],
            captured_at: "2026-04-05T10:00:00Z",
        })
        .unwrap_or_else(|error| panic!("feature build should succeed: {error}"));

        let rest_days = rows
            .iter()
            .filter(|row| row.signal_key == "rest_mode_active")
            .map(|row| row.day.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            rest_days,
            vec!["2026-04-02", "2026-04-03", "2026-04-04", "2026-04-05"]
        );
    }

    #[test]
    fn persistence_uses_signal_baseline_window() {
        let start_day = time::Date::from_calendar_date(2026, time::Month::March, 1)
            .unwrap_or_else(|error| panic!("test start day should be valid: {error}"));
        let seed_points = (0_i64..35_i64)
            .map(|offset| {
                let day = start_day
                    .checked_add(time::Duration::days(offset))
                    .unwrap_or_else(|| panic!("test day should stay in range"));
                let value = match offset {
                    0..=15 if offset % 2 == 0 => 50.0,
                    0..=15 => 150.0,
                    16..=30 => 100.0,
                    _ => 110.0,
                };
                SeedPoint {
                    day: day.to_string(),
                    numeric_value: Some(value),
                    text_value: None,
                    metadata_json: "{}".to_owned(),
                }
            })
            .collect::<Vec<_>>();
        let numeric_series = NumericSeriesIndex::build(&seed_points)
            .unwrap_or_else(|error| panic!("numeric series index should build: {error}"));

        let z_score = Some(1.0);
        assert_eq!(
            signal_sign(z_score, SignalDirectionality::HigherBetter),
            Some(1)
        );

        let short_window = persistence_days(
            &numeric_series,
            "2026-04-04",
            z_score,
            SignalDirectionality::HigherBetter,
            14,
        )
        .unwrap_or_else(|error| panic!("short baseline persistence should build: {error}"));
        let long_window = persistence_days(
            &numeric_series,
            "2026-04-04",
            z_score,
            SignalDirectionality::HigherBetter,
            30,
        )
        .unwrap_or_else(|error| panic!("long baseline persistence should build: {error}"));

        assert!(short_window >= 3);
        assert_eq!(long_window, 0);
    }

    #[test]
    fn comparable_stats_use_calendar_days_not_sample_count() {
        let seed_points = [
            ("2026-01-01", 10.0),
            ("2026-02-01", 20.0),
            ("2026-03-20", 100.0),
            ("2026-04-08", 110.0),
        ]
        .into_iter()
        .map(|(day, value)| SeedPoint {
            day: day.to_owned(),
            numeric_value: Some(value),
            text_value: None,
            metadata_json: "{}".to_owned(),
        })
        .collect::<Vec<_>>();
        let numeric_series = NumericSeriesIndex::build(&seed_points)
            .unwrap_or_else(|error| panic!("numeric series index should build: {error}"));

        let comparable_stats = numeric_series.comparable_stats("2026-04-08", 30);

        assert_eq!(comparable_stats.count, 1);
        assert_eq!(comparable_stats.mean, Some(100.0));
    }
}
