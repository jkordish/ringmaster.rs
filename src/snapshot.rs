use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use time::{Date, Duration, OffsetDateTime};

use crate::config::Config;
use crate::error::{Result, RingmasterError};
use crate::oura::models::{AuthStatus, CapabilityReport};
use crate::review::features::ReviewSufficiency;
use crate::store::Store;
use crate::store::queries::{
    AiArtifactRecord, ContextEventFamily, ContextEventRecord, DailyActivityRecord,
    DailyCardiovascularAgeRecord, DailyOverviewRow, DailyResilienceRecord, DailyStressRecord,
    EffectDirection, PatternMetric, PatternRelationWindow, PatternSummaryRecord,
    RestModePeriodRecord, ReviewSignalDayRecord, SleepTimeRecord, SnapshotExportRecord,
    SnapshotProvenanceRefRecord, SyncRunStatus, SyncStateRecord, Vo2MaxRecord,
};

pub const SNAPSHOT_SCHEMA_VERSION: &str = "ringmaster.snapshot.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyProfile {
    Redacted,
    Balanced,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotSourceMode {
    Live,
    Demo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSnapshotScope {
    pub raw_spec: String,
    pub normalized_spec: String,
    pub start_day: String,
    pub end_day: String,
    pub anchor_day: String,
    pub day_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotExportOutput {
    pub bundle: SnapshotBundleV1,
    pub compact_json: String,
    pub pretty_json: String,
    pub manifest_record: SnapshotExportRecord,
    pub provenance_records: Vec<SnapshotProvenanceRefRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedSnapshotArtifact {
    pub bundle: SnapshotBundleV1,
    pub compact_json: String,
}

pub struct ArtifactRecordInput<'a> {
    pub artifact_id: String,
    pub artifact_kind: &'a str,
    pub output_schema_version: &'a str,
    pub prompt_version: &'a str,
    pub provider: &'a str,
    pub model: &'a str,
    pub reasoning_effort: Option<&'a str>,
    pub request_mode: &'a str,
    pub input_transport: &'a str,
    pub run_mode: &'a str,
    pub snapshot_hash_a: &'a str,
    pub snapshot_hash_b: Option<&'a str>,
    pub privacy_profile: PrivacyProfile,
    pub artifact_status: &'a str,
    pub overview: &'a str,
    pub summary_cache: &'a str,
    pub request_fingerprint: Option<&'a str>,
    pub payload_json: String,
    pub rendered_briefing: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotCatalogSummary {
    pub latest_source_day: Option<String>,
    pub latest_review_day: Option<String>,
    pub freshness_summary: String,
    pub trust_summary: String,
    pub capability_summary: String,
    pub provenance_summary: String,
}

struct GeneratedAtInputs<'a> {
    auth_status: &'a AuthStatus,
    sync_states: &'a [SyncStateRecord],
    daily_history: &'a [DailyOverviewRow],
    daily_activity: &'a [DailyActivityRecord],
    sleep_time: &'a [SleepTimeRecord],
    daily_stress: &'a [DailyStressRecord],
    daily_resilience: &'a [DailyResilienceRecord],
    cardiovascular_age: &'a [DailyCardiovascularAgeRecord],
    vo2_max: &'a [Vo2MaxRecord],
    rest_mode_periods: &'a [RestModePeriodRecord],
    context_events: &'a [ContextEventRecord],
    pattern_summaries: &'a [PatternSummaryRecord],
    review_signals: &'a [ReviewSignalDayRecord],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotBundleV1 {
    pub schema_version: String,
    pub metadata: SnapshotMetadata,
    pub freshness: SnapshotFreshness,
    pub capabilities: SnapshotCapabilities,
    pub record_counts: SnapshotRecordCounts,
    pub metrics: SnapshotMetrics,
    pub baselines: Vec<SnapshotBaseline>,
    pub trend_summaries: Vec<SnapshotTrendSummary>,
    pub context_events: Vec<SnapshotContextEvent>,
    pub pattern_summaries: Vec<SnapshotPatternSummary>,
    pub review_signals: Vec<SnapshotReviewSignal>,
    pub follow_up_targets: Vec<SnapshotFollowUpTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub app_version: String,
    pub generated_at: String,
    pub snapshot_hash: String,
    pub scope: String,
    pub start_day: String,
    pub end_day: String,
    pub anchor_day: String,
    pub privacy_profile: PrivacyProfile,
    pub source_mode: SnapshotSourceMode,
    pub schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotFreshness {
    pub latest_source_day: Option<String>,
    pub latest_review_day: Option<String>,
    pub warnings: Vec<String>,
    pub sync_states: Vec<SnapshotSyncState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotSyncState {
    pub sync_key: String,
    pub status: String,
    pub last_attempted_at: String,
    pub last_completed_at: Option<String>,
    pub failure_count: u32,
    pub next_attempt_after: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotCapabilities {
    pub requested_scopes: Vec<String>,
    pub granted_scopes: Vec<String>,
    pub missing_scopes: Vec<String>,
    pub entries: Vec<SnapshotCapabilityEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotCapabilityEntry {
    pub key: String,
    pub label: String,
    pub requested: bool,
    pub granted: bool,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRecordCounts {
    pub daily_history_days: usize,
    pub heartrate_days: usize,
    pub context_events: usize,
    pub pattern_summaries: usize,
    pub review_signals: usize,
    pub raw_tables: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotMetrics {
    pub daily_scores: Vec<SnapshotDailyScore>,
    pub activity: Vec<SnapshotActivityDay>,
    pub heartrate_daily_averages: Vec<SnapshotMetricPoint>,
    pub sleep_windows: Vec<SnapshotSleepWindow>,
    pub stress: Vec<SnapshotStressDay>,
    pub resilience: Vec<SnapshotResilienceDay>,
    pub cardiovascular_age: Vec<SnapshotMetricPoint>,
    pub vo2_max: Vec<SnapshotMetricPoint>,
    pub rest_mode_periods: Vec<SnapshotRestModePeriod>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotDailyScore {
    pub export_ref: String,
    pub day: String,
    pub sleep_score: Option<u8>,
    pub readiness_score: Option<u8>,
    pub activity_score: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotActivityDay {
    pub export_ref: String,
    pub day: String,
    pub active_calories: i64,
    pub steps: i64,
    pub total_calories: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotMetricPoint {
    pub export_ref: String,
    pub day: String,
    pub value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotSleepWindow {
    pub export_ref: String,
    pub day: String,
    pub status: Option<String>,
    pub recommendation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotStressDay {
    pub export_ref: String,
    pub day: String,
    pub stress_high: Option<i64>,
    pub recovery_high: Option<i64>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotResilienceDay {
    pub export_ref: String,
    pub day: String,
    pub level: String,
    pub sleep_recovery: f64,
    pub daytime_recovery: f64,
    pub stress: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRestModePeriod {
    pub export_ref: String,
    pub start_day: String,
    pub end_day: Option<String>,
    pub episode_count: u32,
    pub tag_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotBaseline {
    pub metric_key: String,
    pub label: String,
    pub scope_average: Option<f64>,
    pub baseline_average: Option<f64>,
    pub delta: Option<f64>,
    pub scope_samples: usize,
    pub baseline_samples: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotTrendSummary {
    pub metric_key: String,
    pub label: String,
    pub direction: String,
    pub summary: String,
    pub current_average: Option<f64>,
    pub previous_average: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotContextEvent {
    pub export_ref: String,
    pub anchor_day: String,
    pub family: String,
    pub label: String,
    pub subtype: Option<String>,
    pub intensity: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotPatternSummary {
    pub export_ref: String,
    pub family: String,
    pub label: String,
    pub metric: String,
    pub relation_window: String,
    pub sample_count: u32,
    pub median_delta: f64,
    pub effect_direction: String,
    pub confidence: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotReviewSignal {
    pub export_ref: String,
    pub day: String,
    pub signal_key: String,
    pub numeric_value: Option<f64>,
    pub text_value: Option<String>,
    pub delta: Option<f64>,
    pub z_score: Option<f64>,
    pub persistence_days: u32,
    pub sufficiency: String,
    pub stale_days: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotFollowUpTarget {
    pub label: String,
    pub command: String,
    pub reason: String,
}

pub fn summarize_snapshot_bundle(
    bundle: &SnapshotBundleV1,
    provenance_refs: &[SnapshotProvenanceRefRecord],
) -> SnapshotCatalogSummary {
    let warning_count = bundle.freshness.warnings.len();
    let stale_signal_count = bundle
        .review_signals
        .iter()
        .filter(|signal| signal.stale_days > 0)
        .count();
    let strong_signal_count = bundle
        .review_signals
        .iter()
        .filter(|signal| signal.sufficiency == "strong")
        .count();
    let unique_local_kinds = provenance_refs
        .iter()
        .map(|record| record.local_kind.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let granted_capabilities = bundle
        .capabilities
        .entries
        .iter()
        .filter(|entry| entry.granted)
        .count();

    SnapshotCatalogSummary {
        latest_source_day: bundle.freshness.latest_source_day.clone(),
        latest_review_day: bundle.freshness.latest_review_day.clone(),
        freshness_summary: format!(
            "latest_source_day={} latest_review_day={} warnings={warning_count}",
            bundle
                .freshness
                .latest_source_day
                .as_deref()
                .unwrap_or("none"),
            bundle
                .freshness
                .latest_review_day
                .as_deref()
                .unwrap_or("none"),
        ),
        trust_summary: format!(
            "review_signals={} strong={} stale={} follow_up_targets={}",
            bundle.review_signals.len(),
            strong_signal_count,
            stale_signal_count,
            bundle.follow_up_targets.len(),
        ),
        capability_summary: format!(
            "granted={} missing={} requested={}",
            granted_capabilities,
            bundle.capabilities.missing_scopes.len(),
            bundle.capabilities.requested_scopes.len(),
        ),
        provenance_summary: format!(
            "refs={} local_kinds={}",
            provenance_refs.len(),
            unique_local_kinds,
        ),
    }
}

pub fn resolve_scope(store: &Store, raw_spec: &str) -> Result<ResolvedSnapshotScope> {
    let trimmed = raw_spec.trim();
    if trimmed.is_empty() {
        return Err(RingmasterError::Config(
            "snapshot scope must not be empty".to_owned(),
        ));
    }

    let latest_source_day = store
        .views()
        .latest_source_day()?
        .unwrap_or_else(current_local_day_string);
    let latest_date = parse_day(&latest_source_day)?;

    if trimmed == "today" {
        return resolved_scope_from_dates(trimmed, latest_date, latest_date);
    }

    if trimmed == "week" {
        return resolved_scope_from_dates(trimmed, latest_date - Duration::days(6), latest_date);
    }

    if let Some(day) = trimmed.strip_prefix("day:") {
        let day = parse_day(day)?;
        return resolved_scope_from_dates(trimmed, day, day);
    }

    if let Some(range) = trimmed.strip_prefix("range:") {
        let Some((start_raw, end_raw)) = range.split_once("..") else {
            return Err(RingmasterError::Config(format!(
                "invalid snapshot range `{trimmed}`; expected range:YYYY-MM-DD..YYYY-MM-DD"
            )));
        };
        let start = parse_day(start_raw)?;
        let end = parse_day(end_raw)?;
        return resolved_scope_from_dates(trimmed, start, end);
    }

    Err(RingmasterError::Config(format!(
        "unsupported snapshot scope `{trimmed}`; use today, week, day:YYYY-MM-DD, or range:YYYY-MM-DD..YYYY-MM-DD"
    )))
}

pub fn export_snapshot(
    config: &Config,
    store: &Store,
    auth_status: &AuthStatus,
    source_mode: SnapshotSourceMode,
    fixture_dir: Option<&Path>,
    scope: &ResolvedSnapshotScope,
    privacy_profile: PrivacyProfile,
) -> Result<SnapshotExportOutput> {
    let sync_states = store.sync_state().list()?;
    let capability_report = capability_report(auth_status, &sync_states);
    let daily_history_all = store.views().daily_history_all()?;
    let daily_history = store
        .views()
        .daily_history_between_days(&scope.start_day, &scope.end_day)?;
    let daily_activity = store
        .views()
        .daily_activity_between_days(&scope.start_day, &scope.end_day)?;
    let sleep_time = store
        .views()
        .sleep_time_between_days(&scope.start_day, &scope.end_day)?;
    let daily_stress = store
        .views()
        .daily_stress_between_days(&scope.start_day, &scope.end_day)?;
    let daily_resilience = store
        .views()
        .daily_resilience_between_days(&scope.start_day, &scope.end_day)?;
    let cardiovascular_age = store
        .views()
        .daily_cardiovascular_age_between_days(&scope.start_day, &scope.end_day)?;
    let vo2_max = store
        .views()
        .vo2_max_between_days(&scope.start_day, &scope.end_day)?;
    let rest_mode_periods = store
        .views()
        .rest_mode_periods_between_days(&scope.start_day, &scope.end_day)?;
    let materialized_context_events = store
        .views()
        .context_events_between_days(&scope.start_day, &scope.end_day)?;
    let materialized_pattern_summaries = store.views().pattern_summaries(None, None)?;
    let materialized_review_signals = store
        .views()
        .review_signal_days_between_days(&scope.start_day, &scope.end_day)?;
    let derived = crate::derive::derive_review_artifacts_for_anchor_day(
        store,
        config,
        Some(scope.anchor_day.as_str()),
    )?;

    let context_events = derived
        .as_ref()
        .map(|artifacts| {
            artifacts
                .context_events
                .iter()
                .filter(|record| {
                    record.anchor_day.as_str() >= scope.start_day.as_str()
                        && record.anchor_day.as_str() <= scope.end_day.as_str()
                })
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or(materialized_context_events);
    let pattern_summaries = derived
        .as_ref()
        .map(|artifacts| artifacts.pattern_summaries.clone())
        .unwrap_or(materialized_pattern_summaries);
    let review_signals = derived
        .as_ref()
        .map(|artifacts| {
            artifacts
                .review_signal_days
                .iter()
                .filter(|record| {
                    record.day.as_str() >= scope.start_day.as_str()
                        && record.day.as_str() <= scope.end_day.as_str()
                })
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or(materialized_review_signals);
    let heartrate_daily_averages =
        load_heartrate_daily_averages(store, &scope.start_day, &scope.end_day)?;
    let latest_source_day = store.views().latest_source_day()?;
    let latest_review_day = store.views().latest_review_day()?;
    let record_counts = store.views().record_counts()?;
    let generated_at = deterministic_generated_at(GeneratedAtInputs {
        auth_status,
        sync_states: &sync_states,
        daily_history: &daily_history,
        daily_activity: &daily_activity,
        sleep_time: &sleep_time,
        daily_stress: &daily_stress,
        daily_resilience: &daily_resilience,
        cardiovascular_age: &cardiovascular_age,
        vo2_max: &vo2_max,
        rest_mode_periods: &rest_mode_periods,
        context_events: &context_events,
        pattern_summaries: &pattern_summaries,
        review_signals: &review_signals,
    })?;

    let mut provenance_records = Vec::new();
    let daily_scores = daily_history
        .iter()
        .map(|row| {
            let export_ref = format!("daily:{}", row.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "daily_overview",
                &row.day,
                &generated_at,
            ));
            SnapshotDailyScore {
                export_ref,
                day: row.day.clone(),
                sleep_score: row.sleep_score,
                readiness_score: row.readiness_score,
                activity_score: row.activity_score,
            }
        })
        .collect::<Vec<_>>();
    let activity = daily_activity
        .iter()
        .map(|row| {
            let export_ref = format!("activity:{}", row.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "daily_activity",
                &row.day,
                &generated_at,
            ));
            SnapshotActivityDay {
                export_ref,
                day: row.day.clone(),
                active_calories: row.active_calories,
                steps: row.steps,
                total_calories: row.total_calories,
            }
        })
        .collect::<Vec<_>>();
    let sleep_windows = sleep_time
        .iter()
        .map(|row| {
            let export_ref = format!("sleep_time:{}", row.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "sleep_time",
                &row.day,
                &generated_at,
            ));
            SnapshotSleepWindow {
                export_ref,
                day: row.day.clone(),
                status: row.status.clone(),
                recommendation: row.recommendation.clone(),
            }
        })
        .collect::<Vec<_>>();
    let stress = daily_stress
        .iter()
        .map(|row| {
            let export_ref = format!("stress:{}", row.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "daily_stress",
                &row.day,
                &generated_at,
            ));
            SnapshotStressDay {
                export_ref,
                day: row.day.clone(),
                stress_high: row.stress_high,
                recovery_high: row.recovery_high,
                summary: redact_optional_text(privacy_profile, row.day_summary.as_deref()),
            }
        })
        .collect::<Vec<_>>();
    let resilience = daily_resilience
        .iter()
        .map(|row| {
            let export_ref = format!("resilience:{}", row.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "daily_resilience",
                &row.day,
                &generated_at,
            ));
            SnapshotResilienceDay {
                export_ref,
                day: row.day.clone(),
                level: row.level.clone(),
                sleep_recovery: row.sleep_recovery,
                daytime_recovery: row.daytime_recovery,
                stress: row.stress,
            }
        })
        .collect::<Vec<_>>();
    let cardiovascular_age = cardiovascular_age
        .iter()
        .map(|row| {
            let export_ref = format!("cardio_age:{}", row.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "daily_cardiovascular_age",
                &row.day,
                &generated_at,
            ));
            SnapshotMetricPoint {
                export_ref,
                day: row.day.clone(),
                value: row.vascular_age.map(|value| value as f64),
            }
        })
        .collect::<Vec<_>>();
    let vo2_max = vo2_max
        .iter()
        .map(|row| {
            let export_ref = format!("vo2_max:{}:{}", row.day, row.recorded_at);
            provenance_records.push(provenance_record(
                &export_ref,
                "vo2_max",
                &format!("{}|{}", row.day, row.recorded_at),
                &generated_at,
            ));
            SnapshotMetricPoint {
                export_ref,
                day: row.day.clone(),
                value: row.vo2_max,
            }
        })
        .collect::<Vec<_>>();
    let rest_mode_periods = rest_mode_periods
        .iter()
        .map(|row| {
            let export_ref = format!("rest_mode:{}", row.period_id);
            provenance_records.push(provenance_record(
                &export_ref,
                "rest_mode_period",
                &row.period_id,
                &generated_at,
            ));
            SnapshotRestModePeriod {
                export_ref,
                start_day: row.start_day.clone(),
                end_day: row.end_day.clone(),
                episode_count: row.episode_count,
                tag_count: count_json_array_items(&row.tags_json),
            }
        })
        .collect::<Vec<_>>();
    let heartrate_daily_averages = heartrate_daily_averages
        .into_iter()
        .map(|point| {
            let export_ref = format!("heartrate:{}", point.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "heartrate_day",
                &point.day,
                &generated_at,
            ));
            SnapshotMetricPoint {
                export_ref,
                day: point.day,
                value: Some(point.value),
            }
        })
        .collect::<Vec<_>>();
    let context_events = context_events
        .iter()
        .map(|record| {
            let export_ref = format!("context:{}", record.context_event_id);
            provenance_records.push(provenance_record(
                &export_ref,
                "context_event",
                &record.context_event_id,
                &generated_at,
            ));
            SnapshotContextEvent {
                export_ref,
                anchor_day: record.anchor_day.clone(),
                family: record.family.as_str().to_owned(),
                label: context_label(privacy_profile, record),
                subtype: record.subtype.clone(),
                intensity: record.intensity.clone(),
                summary: context_summary(privacy_profile, record),
            }
        })
        .collect::<Vec<_>>();
    let pattern_summaries = pattern_summaries
        .iter()
        .map(|record| {
            let export_ref = format!("pattern:{}", record.summary_id);
            provenance_records.push(provenance_record(
                &export_ref,
                "pattern_summary",
                &record.summary_id,
                &generated_at,
            ));
            SnapshotPatternSummary {
                export_ref,
                family: record.family.as_str().to_owned(),
                label: pattern_label(privacy_profile, record),
                metric: record.metric.as_str().to_owned(),
                relation_window: record.relation_window.as_str().to_owned(),
                sample_count: record.sample_count,
                median_delta: record.median_delta,
                effect_direction: record.effect_direction.as_str().to_owned(),
                confidence: record.confidence.as_str().to_owned(),
                summary: pattern_summary_text(record),
            }
        })
        .collect::<Vec<_>>();
    let review_signals = review_signals
        .iter()
        .map(|record| {
            let export_ref = format!("signal:{}:{}", record.signal_key, record.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "review_signal",
                &format!("{}|{}", record.signal_key, record.day),
                &generated_at,
            ));
            SnapshotReviewSignal {
                export_ref,
                day: record.day.clone(),
                signal_key: record.signal_key.clone(),
                numeric_value: record.numeric_value,
                text_value: match privacy_profile {
                    PrivacyProfile::Full => record.text_value.clone(),
                    PrivacyProfile::Redacted | PrivacyProfile::Balanced => None,
                },
                delta: record.delta,
                z_score: record.z_score,
                persistence_days: record.persistence_days,
                sufficiency: review_sufficiency_string(record.sufficiency),
                stale_days: record.stale_days,
            }
        })
        .collect::<Vec<_>>();
    provenance_records.sort_by(|left, right| left.export_ref.cmp(&right.export_ref));

    let baselines = build_baselines(&daily_history_all, scope)?;
    let trend_summaries =
        build_trend_summaries(&daily_history_all, &heartrate_daily_averages, scope)?;
    let follow_up_targets = build_follow_up_targets(scope, &review_signals);
    let warnings = build_freshness_warnings(
        &sync_states,
        latest_source_day.as_deref(),
        latest_review_day.as_deref(),
        &capability_report,
    );

    let raw_tables = BTreeMap::from([
        ("raw_payloads".to_owned(), record_counts.raw_payloads),
        ("personal_info".to_owned(), record_counts.personal_info),
        ("daily_sleep".to_owned(), record_counts.daily_sleep),
        ("daily_readiness".to_owned(), record_counts.daily_readiness),
        ("daily_activity".to_owned(), record_counts.daily_activity),
        ("sleep_time".to_owned(), record_counts.sleep_time),
        ("daily_stress".to_owned(), record_counts.daily_stress),
        (
            "daily_resilience".to_owned(),
            record_counts.daily_resilience,
        ),
        (
            "daily_cardiovascular_age".to_owned(),
            record_counts.daily_cardiovascular_age,
        ),
        ("vo2_max".to_owned(), record_counts.vo2_max),
        (
            "rest_mode_periods".to_owned(),
            record_counts.rest_mode_periods,
        ),
        (
            "heartrate_samples".to_owned(),
            record_counts.heartrate_samples,
        ),
        ("workouts".to_owned(), record_counts.workouts),
        ("tags".to_owned(), record_counts.tags),
        ("enhanced_tags".to_owned(), record_counts.enhanced_tags),
        ("sessions".to_owned(), record_counts.sessions),
        (
            "derived_context_events".to_owned(),
            record_counts.derived_context_events,
        ),
        (
            "derived_pattern_summaries".to_owned(),
            record_counts.derived_pattern_summaries,
        ),
        (
            "derived_review_signal_days".to_owned(),
            record_counts.derived_review_signal_days,
        ),
    ]);

    let mut bundle = SnapshotBundleV1 {
        schema_version: SNAPSHOT_SCHEMA_VERSION.to_owned(),
        metadata: SnapshotMetadata {
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            generated_at: generated_at.clone(),
            snapshot_hash: String::new(),
            scope: scope.normalized_spec.clone(),
            start_day: scope.start_day.clone(),
            end_day: scope.end_day.clone(),
            anchor_day: scope.anchor_day.clone(),
            privacy_profile,
            source_mode,
            schema_version: store.metadata().schema_version()?,
        },
        freshness: SnapshotFreshness {
            latest_source_day,
            latest_review_day,
            warnings,
            sync_states: sync_states
                .iter()
                .map(|state| SnapshotSyncState {
                    sync_key: state.sync_key.clone(),
                    status: sync_status_string(&state.status),
                    last_attempted_at: state.last_attempted_at.clone(),
                    last_completed_at: state.last_completed_at.clone(),
                    failure_count: state.failure_count,
                    next_attempt_after: state.next_attempt_after.clone(),
                    message: state.message.clone(),
                })
                .collect(),
        },
        capabilities: SnapshotCapabilities {
            requested_scopes: auth_status.requested_scopes.clone(),
            granted_scopes: capability_report
                .entries
                .iter()
                .filter(|entry| entry.granted)
                .map(|entry| entry.kind.scope_name().to_owned())
                .collect(),
            missing_scopes: capability_report
                .entries
                .iter()
                .filter(|entry| entry.requested && !entry.granted)
                .map(|entry| entry.kind.scope_name().to_owned())
                .collect(),
            entries: capability_report
                .entries
                .iter()
                .map(|entry| SnapshotCapabilityEntry {
                    key: entry.kind.scope_name().to_owned(),
                    label: entry.kind.label().to_owned(),
                    requested: entry.requested,
                    granted: entry.granted,
                    note: entry.note.clone(),
                })
                .collect(),
        },
        record_counts: SnapshotRecordCounts {
            daily_history_days: daily_scores.len(),
            heartrate_days: heartrate_daily_averages.len(),
            context_events: context_events.len(),
            pattern_summaries: pattern_summaries.len(),
            review_signals: review_signals.len(),
            raw_tables,
        },
        metrics: SnapshotMetrics {
            daily_scores,
            activity,
            heartrate_daily_averages,
            sleep_windows,
            stress,
            resilience,
            cardiovascular_age,
            vo2_max,
            rest_mode_periods,
        },
        baselines,
        trend_summaries,
        context_events,
        pattern_summaries,
        review_signals,
        follow_up_targets,
    };

    let serialized_without_hash = serde_json::to_string(&bundle)?;
    let round_tripped_without_hash =
        serde_json::from_str::<SnapshotBundleV1>(&serialized_without_hash)?;
    let canonical_without_hash = serde_json::to_string(&round_tripped_without_hash)?;
    let snapshot_hash = hex::encode(Sha256::digest(canonical_without_hash.as_bytes()));
    bundle.metadata.snapshot_hash.clone_from(&snapshot_hash);

    let serialized_with_hash = serde_json::to_string(&bundle)?;
    let bundle = serde_json::from_str::<SnapshotBundleV1>(&serialized_with_hash)?;
    let compact_json = serde_json::to_string(&bundle)?;
    let pretty_json = serde_json::to_string_pretty(&bundle)?;
    validate_snapshot_bundle(&bundle)?;
    let manifest_record = SnapshotExportRecord {
        snapshot_hash: snapshot_hash.clone(),
        schema_version: SNAPSHOT_SCHEMA_VERSION.to_owned(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        generated_at,
        scope: scope.normalized_spec.clone(),
        start_day: String::new(),
        end_day: String::new(),
        anchor_day: String::new(),
        day_count: 0,
        privacy_profile: privacy_profile.as_str().to_owned(),
        source_mode: source_mode.as_str().to_owned(),
        fixture_dir: fixture_dir.map(|path| path.display().to_string()),
        latest_source_day: None,
        latest_review_day: None,
        freshness_summary: String::new(),
        trust_summary: String::new(),
        capability_summary: String::new(),
        provenance_summary: String::new(),
        snapshot_json: compact_json.clone(),
        created_at: now_rfc3339()?,
    };
    let provenance_records = provenance_records
        .into_iter()
        .map(|mut record| {
            record.snapshot_hash.clone_from(&snapshot_hash);
            record
        })
        .collect::<Vec<_>>();
    let catalog_summary = summarize_snapshot_bundle(&bundle, &provenance_records);

    Ok(SnapshotExportOutput {
        bundle,
        compact_json,
        pretty_json,
        manifest_record: SnapshotExportRecord {
            start_day: scope.start_day.clone(),
            end_day: scope.end_day.clone(),
            anchor_day: scope.anchor_day.clone(),
            day_count: u32::try_from(scope.day_count).unwrap_or(u32::MAX),
            latest_source_day: catalog_summary.latest_source_day,
            latest_review_day: catalog_summary.latest_review_day,
            freshness_summary: catalog_summary.freshness_summary,
            trust_summary: catalog_summary.trust_summary,
            capability_summary: catalog_summary.capability_summary,
            provenance_summary: catalog_summary.provenance_summary,
            ..manifest_record
        },
        provenance_records,
    })
}

pub fn load_snapshot_artifact(path: &Path) -> Result<LoadedSnapshotArtifact> {
    let raw_json = fs::read_to_string(path)
        .map_err(|error| RingmasterError::io("reading snapshot artifact", error))?;
    let bundle = deserialize_snapshot_bundle(&raw_json)?;
    let compact_json = canonicalize_snapshot_bundle(&bundle)?;
    Ok(LoadedSnapshotArtifact {
        bundle,
        compact_json,
    })
}

pub fn catalog_record_from_loaded_artifact(
    artifact: &LoadedSnapshotArtifact,
    fixture_dir: Option<&Path>,
) -> SnapshotExportRecord {
    let summary = summarize_snapshot_bundle(&artifact.bundle, &[]);
    let day_count = Date::parse(
        &artifact.bundle.metadata.start_day,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .and_then(|start| {
        Date::parse(
            &artifact.bundle.metadata.end_day,
            &time::macros::format_description!("[year]-[month]-[day]"),
        )
        .map(|end| (end - start).whole_days() + 1)
    })
    .ok()
    .and_then(|value| u32::try_from(value).ok())
    .unwrap_or(0);

    SnapshotExportRecord {
        snapshot_hash: artifact.bundle.metadata.snapshot_hash.clone(),
        schema_version: artifact.bundle.schema_version.clone(),
        app_version: artifact.bundle.metadata.app_version.clone(),
        generated_at: artifact.bundle.metadata.generated_at.clone(),
        scope: artifact.bundle.metadata.scope.clone(),
        start_day: artifact.bundle.metadata.start_day.clone(),
        end_day: artifact.bundle.metadata.end_day.clone(),
        anchor_day: artifact.bundle.metadata.anchor_day.clone(),
        day_count,
        privacy_profile: artifact.bundle.metadata.privacy_profile.as_str().to_owned(),
        source_mode: artifact.bundle.metadata.source_mode.as_str().to_owned(),
        fixture_dir: fixture_dir.map(|path| path.display().to_string()),
        latest_source_day: summary.latest_source_day,
        latest_review_day: summary.latest_review_day,
        freshness_summary: summary.freshness_summary,
        trust_summary: summary.trust_summary,
        capability_summary: summary.capability_summary,
        provenance_summary: summary.provenance_summary,
        snapshot_json: artifact.compact_json.clone(),
        created_at: artifact.bundle.metadata.generated_at.clone(),
    }
}

pub fn deserialize_snapshot_bundle(raw_json: &str) -> Result<SnapshotBundleV1> {
    let bundle = serde_json::from_str::<SnapshotBundleV1>(raw_json)?;
    validate_snapshot_bundle(&bundle)?;
    Ok(bundle)
}

pub fn canonicalize_snapshot_bundle(bundle: &SnapshotBundleV1) -> Result<String> {
    validate_snapshot_bundle(bundle)?;
    serde_json::to_string(bundle).map_err(Into::into)
}

pub fn validate_snapshot_bundle(bundle: &SnapshotBundleV1) -> Result<()> {
    if bundle.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return Err(RingmasterError::Config(format!(
            "unsupported snapshot schema version `{}`",
            bundle.schema_version
        )));
    }

    let mut without_hash = bundle.clone();
    let observed_hash = without_hash.metadata.snapshot_hash.clone();
    if observed_hash.trim().is_empty() {
        return Err(RingmasterError::Config(
            "snapshot artifact is missing metadata.snapshot_hash".to_owned(),
        ));
    }
    without_hash.metadata.snapshot_hash.clear();
    let canonical_without_hash = serde_json::to_string(&without_hash)?;
    let expected_hash = hex::encode(Sha256::digest(canonical_without_hash.as_bytes()));
    if observed_hash != expected_hash {
        return Err(RingmasterError::Config(format!(
            "snapshot hash mismatch: expected `{expected_hash}` but found `{observed_hash}`"
        )));
    }

    Ok(())
}

pub fn artifact_record(input: ArtifactRecordInput<'_>) -> Result<AiArtifactRecord> {
    Ok(AiArtifactRecord {
        artifact_id: input.artifact_id,
        artifact_kind: input.artifact_kind.to_owned(),
        output_schema_version: input.output_schema_version.to_owned(),
        prompt_version: input.prompt_version.to_owned(),
        provider: input.provider.to_owned(),
        model: input.model.to_owned(),
        reasoning_effort: input.reasoning_effort.map(ToOwned::to_owned),
        request_mode: input.request_mode.to_owned(),
        input_transport: input.input_transport.to_owned(),
        run_mode: input.run_mode.to_owned(),
        created_at: now_rfc3339()?,
        snapshot_hash_a: input.snapshot_hash_a.to_owned(),
        snapshot_hash_b: input.snapshot_hash_b.map(ToOwned::to_owned),
        privacy_profile: input.privacy_profile.as_str().to_owned(),
        artifact_status: input.artifact_status.to_owned(),
        overview: input.overview.to_owned(),
        summary_cache: input.summary_cache.to_owned(),
        request_fingerprint: input.request_fingerprint.map(ToOwned::to_owned),
        payload_json: input.payload_json,
        rendered_briefing: input.rendered_briefing,
    })
}

fn provenance_record(
    export_ref: &str,
    local_kind: &str,
    local_locator: &str,
    created_at: &str,
) -> SnapshotProvenanceRefRecord {
    SnapshotProvenanceRefRecord {
        snapshot_hash: String::new(),
        export_ref: export_ref.to_owned(),
        local_kind: local_kind.to_owned(),
        local_locator: local_locator.to_owned(),
        created_at: created_at.to_owned(),
    }
}

fn capability_report(
    auth_status: &AuthStatus,
    sync_states: &[SyncStateRecord],
) -> CapabilityReport {
    if !auth_status.capability_report.entries.is_empty() {
        return auth_status.capability_report.clone();
    }

    let granted_scopes = sync_states
        .iter()
        .flat_map(|state| state.granted_scopes.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    CapabilityReport::from_scopes(&auth_status.requested_scopes, &granted_scopes)
}

fn resolved_scope_from_dates(
    raw_spec: &str,
    start: Date,
    end: Date,
) -> Result<ResolvedSnapshotScope> {
    if end < start {
        return Err(RingmasterError::Config(format!(
            "snapshot scope `{raw_spec}` ends before it starts"
        )));
    }

    let day_count = (end - start).whole_days() + 1;
    let day_count = usize::try_from(day_count).map_err(|error| {
        RingmasterError::Config(format!("snapshot day count is invalid: {error}"))
    })?;

    Ok(ResolvedSnapshotScope {
        raw_spec: raw_spec.to_owned(),
        normalized_spec: match day_count {
            1 => format!("day:{start}"),
            7 if raw_spec == "week" => "week".to_owned(),
            _ => format!("range:{start}..{end}"),
        },
        start_day: start.to_string(),
        end_day: end.to_string(),
        anchor_day: end.to_string(),
        day_count,
    })
}

fn parse_day(value: &str) -> Result<Date> {
    Date::parse(
        value,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .map_err(|error| RingmasterError::Config(format!("invalid day `{value}`: {error}")))
}

fn current_local_day_string() -> String {
    let local_offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    OffsetDateTime::now_utc()
        .to_offset(local_offset)
        .date()
        .to_string()
}

fn load_heartrate_daily_averages(
    store: &Store,
    start_day: &str,
    end_day: &str,
) -> Result<Vec<DayValuePoint>> {
    let start = parse_day(start_day)?;
    let end = parse_day(end_day)?;
    let available_days = store.views().available_heartrate_days(366)?;
    let mut points = Vec::new();
    for day in available_days {
        let parsed_day = parse_day(&day)?;
        if parsed_day < start || parsed_day > end {
            continue;
        }
        let samples = store.views().heartrate_for_day(&day)?;
        if samples.is_empty() {
            continue;
        }
        let sum = samples
            .iter()
            .map(|sample| f64::from(sample.bpm))
            .sum::<f64>();
        let average = sum / samples.len() as f64;
        points.push(DayValuePoint {
            day,
            value: round_metric(average),
        });
    }

    Ok(points)
}

#[derive(Debug, Clone, PartialEq)]
struct DayValuePoint {
    day: String,
    value: f64,
}

fn deterministic_generated_at(inputs: GeneratedAtInputs<'_>) -> Result<String> {
    let mut timestamps = Vec::new();
    timestamps.extend(inputs.auth_status.access_token_expires_at.iter().cloned());
    timestamps.extend(inputs.auth_status.last_authenticated_at.iter().cloned());
    timestamps.extend(inputs.auth_status.last_refresh_at.iter().cloned());
    timestamps.extend(
        inputs
            .sync_states
            .iter()
            .map(|record| record.last_attempted_at.clone()),
    );
    timestamps.extend(
        inputs
            .sync_states
            .iter()
            .filter_map(|record| record.last_completed_at.clone()),
    );
    timestamps.extend(
        inputs
            .daily_history
            .iter()
            .map(|record| record.updated_at.clone()),
    );
    timestamps.extend(
        inputs
            .daily_activity
            .iter()
            .map(|record| record.updated_at.clone()),
    );
    timestamps.extend(
        inputs
            .sleep_time
            .iter()
            .map(|record| record.updated_at.clone()),
    );
    timestamps.extend(
        inputs
            .daily_stress
            .iter()
            .map(|record| record.updated_at.clone()),
    );
    timestamps.extend(
        inputs
            .daily_resilience
            .iter()
            .map(|record| record.updated_at.clone()),
    );
    timestamps.extend(
        inputs
            .cardiovascular_age
            .iter()
            .map(|record| record.updated_at.clone()),
    );
    timestamps.extend(
        inputs
            .vo2_max
            .iter()
            .map(|record| record.updated_at.clone()),
    );
    timestamps.extend(
        inputs
            .rest_mode_periods
            .iter()
            .map(|record| record.updated_at.clone()),
    );
    timestamps.extend(
        inputs
            .context_events
            .iter()
            .map(|record| record.updated_at.clone()),
    );
    timestamps.extend(
        inputs
            .pattern_summaries
            .iter()
            .map(|record| record.updated_at.clone()),
    );
    timestamps.extend(
        inputs
            .review_signals
            .iter()
            .map(|record| record.updated_at.clone()),
    );

    let mut latest: Option<OffsetDateTime> = None;
    for timestamp in timestamps {
        if let Ok(parsed) = OffsetDateTime::parse(&timestamp, &Rfc3339) {
            latest = Some(latest.map_or(parsed, |current| current.max(parsed)));
        }
    }

    latest
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .format(&Rfc3339)
        .map_err(|error| {
            RingmasterError::Config(format!(
                "failed to format deterministic snapshot timestamp: {error}"
            ))
        })
}

fn build_baselines(
    daily_history_all: &[DailyOverviewRow],
    scope: &ResolvedSnapshotScope,
) -> Result<Vec<SnapshotBaseline>> {
    let scope_start = parse_day(&scope.start_day)?;
    let baseline_end = scope_start - Duration::days(1);
    let scope_days = i64::try_from(scope.day_count).map_err(|error| {
        RingmasterError::Config(format!("snapshot day count is too large: {error}"))
    })?;
    let baseline_start = baseline_end - Duration::days(scope_days - 1);

    let scope_rows = daily_history_all
        .iter()
        .filter(|row| {
            row.day.as_str() >= scope.start_day.as_str()
                && row.day.as_str() <= scope.end_day.as_str()
        })
        .collect::<Vec<_>>();
    let baseline_rows = daily_history_all
        .iter()
        .filter(|row| {
            let Ok(day) = parse_day(&row.day) else {
                return false;
            };
            day >= baseline_start && day <= baseline_end
        })
        .collect::<Vec<_>>();

    let sleep = metric_baseline(
        "sleep_score",
        "Sleep score",
        scope_rows
            .iter()
            .filter_map(|row| row.sleep_score.map(f64::from)),
        baseline_rows
            .iter()
            .filter_map(|row| row.sleep_score.map(f64::from)),
    );
    let readiness = metric_baseline(
        "readiness_score",
        "Readiness score",
        scope_rows
            .iter()
            .filter_map(|row| row.readiness_score.map(f64::from)),
        baseline_rows
            .iter()
            .filter_map(|row| row.readiness_score.map(f64::from)),
    );
    let activity = metric_baseline(
        "activity_score",
        "Activity score",
        scope_rows
            .iter()
            .filter_map(|row| row.activity_score.map(f64::from)),
        baseline_rows
            .iter()
            .filter_map(|row| row.activity_score.map(f64::from)),
    );

    Ok(vec![sleep, readiness, activity])
}

fn metric_baseline<I, J>(
    metric_key: &str,
    label: &str,
    scope_values: I,
    baseline_values: J,
) -> SnapshotBaseline
where
    I: IntoIterator<Item = f64>,
    J: IntoIterator<Item = f64>,
{
    let scope_values = scope_values.into_iter().collect::<Vec<_>>();
    let baseline_values = baseline_values.into_iter().collect::<Vec<_>>();
    let scope_average = average(&scope_values);
    let baseline_average = average(&baseline_values);

    SnapshotBaseline {
        metric_key: metric_key.to_owned(),
        label: label.to_owned(),
        scope_average,
        baseline_average,
        delta: scope_average
            .zip(baseline_average)
            .map(|(left, right)| round_metric(left - right)),
        scope_samples: scope_values.len(),
        baseline_samples: baseline_values.len(),
    }
}

fn build_trend_summaries(
    daily_history_all: &[DailyOverviewRow],
    heartrate_daily_averages: &[SnapshotMetricPoint],
    scope: &ResolvedSnapshotScope,
) -> Result<Vec<SnapshotTrendSummary>> {
    let scope_values = daily_history_all
        .iter()
        .filter(|row| {
            row.day.as_str() >= scope.start_day.as_str()
                && row.day.as_str() <= scope.end_day.as_str()
        })
        .collect::<Vec<_>>();
    let previous_end = parse_day(&scope.start_day)? - Duration::days(1);
    let scope_days = i64::try_from(scope.day_count).map_err(|error| {
        RingmasterError::Config(format!("snapshot day count is too large: {error}"))
    })?;
    let previous_start = previous_end - Duration::days(scope_days - 1);
    let previous_values = daily_history_all
        .iter()
        .filter(|row| {
            let Ok(day) = parse_day(&row.day) else {
                return false;
            };
            day >= previous_start && day <= previous_end
        })
        .collect::<Vec<_>>();

    let mut summaries = Vec::new();
    summaries.push(build_metric_trend(
        "sleep_score",
        "Sleep score",
        scope_values
            .iter()
            .filter_map(|row| row.sleep_score.map(f64::from))
            .collect(),
        previous_values
            .iter()
            .filter_map(|row| row.sleep_score.map(f64::from))
            .collect(),
    ));
    summaries.push(build_metric_trend(
        "readiness_score",
        "Readiness score",
        scope_values
            .iter()
            .filter_map(|row| row.readiness_score.map(f64::from))
            .collect(),
        previous_values
            .iter()
            .filter_map(|row| row.readiness_score.map(f64::from))
            .collect(),
    ));
    summaries.push(build_metric_trend(
        "activity_score",
        "Activity score",
        scope_values
            .iter()
            .filter_map(|row| row.activity_score.map(f64::from))
            .collect(),
        previous_values
            .iter()
            .filter_map(|row| row.activity_score.map(f64::from))
            .collect(),
    ));

    if !heartrate_daily_averages.is_empty() {
        let split_index = heartrate_daily_averages.len() / 2;
        let current = heartrate_daily_averages
            .iter()
            .skip(split_index)
            .filter_map(|point| point.value)
            .collect::<Vec<_>>();
        let previous = heartrate_daily_averages
            .iter()
            .take(split_index)
            .filter_map(|point| point.value)
            .collect::<Vec<_>>();
        summaries.push(build_metric_trend(
            "heartrate_daily_average",
            "Daily heartrate average",
            current,
            previous,
        ));
    }

    Ok(summaries
        .into_iter()
        .filter(|summary| summary.current_average.is_some() || summary.previous_average.is_some())
        .collect())
}

fn build_metric_trend(
    metric_key: &str,
    label: &str,
    current_values: Vec<f64>,
    previous_values: Vec<f64>,
) -> SnapshotTrendSummary {
    let current_average = average(&current_values);
    let previous_average = average(&previous_values);
    let direction = current_average
        .zip(previous_average)
        .map(|(current, previous)| {
            if (current - previous).abs() < 0.5 {
                "flat"
            } else if current > previous {
                "higher"
            } else {
                "lower"
            }
        })
        .unwrap_or("insufficient");
    let summary = match current_average.zip(previous_average) {
        Some((current, previous)) => format!(
            "{label} averaged {:.1} in-scope versus {:.1} in the comparison window.",
            current, previous
        ),
        None => format!("Not enough {label} samples were available to compare windows."),
    };

    SnapshotTrendSummary {
        metric_key: metric_key.to_owned(),
        label: label.to_owned(),
        direction: direction.to_owned(),
        summary,
        current_average,
        previous_average,
    }
}

fn build_freshness_warnings(
    sync_states: &[SyncStateRecord],
    latest_source_day: Option<&str>,
    latest_review_day: Option<&str>,
    capability_report: &CapabilityReport,
) -> Vec<String> {
    let mut warnings = Vec::new();
    if latest_source_day.is_none() {
        warnings.push("No source data has been synced yet.".to_owned());
    }
    if latest_review_day.is_none() {
        warnings.push("No derived review signals are available yet.".to_owned());
    }
    for sync_state in sync_states {
        if sync_state.status == SyncRunStatus::Failed {
            warnings.push(format!(
                "{} last failed at {}.",
                sync_state.sync_key, sync_state.last_attempted_at
            ));
        }
    }
    for scope_name in capability_report.missing_scope_names() {
        warnings.push(format!(
            "Capability scope `{scope_name}` was requested but is not currently granted."
        ));
    }
    warnings.sort();
    warnings.dedup();
    warnings
}

fn build_follow_up_targets(
    scope: &ResolvedSnapshotScope,
    review_signals: &[SnapshotReviewSignal],
) -> Vec<SnapshotFollowUpTarget> {
    let mut targets = vec![
        SnapshotFollowUpTarget {
            label: "Review day brief".to_owned(),
            command: format!("review today --day {}", scope.anchor_day),
            reason: "Open the bounded local daily brief for this anchor day.".to_owned(),
        },
        SnapshotFollowUpTarget {
            label: "Review week brief".to_owned(),
            command: format!("review week --end-day {}", scope.anchor_day),
            reason: "Inspect the local weekly brief around the same period.".to_owned(),
        },
    ];
    for signal in review_signals.iter().take(3) {
        targets.push(SnapshotFollowUpTarget {
            label: format!("Investigate {}", signal.signal_key),
            command: format!(
                "review investigate --focus {} --anchor-day {}",
                signal_focus(&signal.signal_key),
                signal.day
            ),
            reason: "Follow the strongest structured signal back into local review tooling."
                .to_owned(),
        });
    }
    targets
}

fn signal_focus(signal_key: &str) -> &str {
    if signal_key.contains("sleep") {
        "sleep"
    } else if signal_key.contains("activity") {
        "activity"
    } else {
        "readiness"
    }
}

fn context_label(profile: PrivacyProfile, record: &ContextEventRecord) -> String {
    match profile {
        PrivacyProfile::Full => record.title.clone(),
        PrivacyProfile::Balanced => record
            .subtype
            .clone()
            .unwrap_or_else(|| family_label(record.family).to_owned()),
        PrivacyProfile::Redacted => family_label(record.family).to_owned(),
    }
}

fn context_summary(profile: PrivacyProfile, record: &ContextEventRecord) -> Option<String> {
    match profile {
        PrivacyProfile::Full => record
            .notes
            .clone()
            .or_else(|| record.subtype.clone())
            .or_else(|| record.intensity.clone()),
        PrivacyProfile::Balanced => record.subtype.clone().or_else(|| record.intensity.clone()),
        PrivacyProfile::Redacted => record.intensity.clone(),
    }
}

fn pattern_label(profile: PrivacyProfile, record: &PatternSummaryRecord) -> String {
    match profile {
        PrivacyProfile::Full => record.normalized_key.clone(),
        PrivacyProfile::Balanced => family_label(record.family).to_owned(),
        PrivacyProfile::Redacted => format!("{} pattern", family_label(record.family)),
    }
}

fn pattern_summary_text(record: &PatternSummaryRecord) -> String {
    format!(
        "{} tends {} {} by {:.1} across {} samples.",
        family_label(record.family),
        relation_verb(record.relation_window),
        record.metric.label(),
        record.median_delta,
        record.sample_count
    )
}

fn relation_verb(window: PatternRelationWindow) -> &'static str {
    match window {
        PatternRelationWindow::SameDayActivity => "to shift same-day",
        PatternRelationWindow::NextDayReadiness => "to shift next-day",
        PatternRelationWindow::SameNightSleep => "to shift same-night",
    }
}

fn family_label(family: ContextEventFamily) -> &'static str {
    match family {
        ContextEventFamily::Workout => "Workout",
        ContextEventFamily::Tag => "Tag",
        ContextEventFamily::EnhancedTag => "Enhanced tag",
        ContextEventFamily::Session => "Session",
    }
}

fn redact_optional_text(profile: PrivacyProfile, value: Option<&str>) -> Option<String> {
    match profile {
        PrivacyProfile::Full => value.map(ToOwned::to_owned),
        PrivacyProfile::Balanced | PrivacyProfile::Redacted => None,
    }
}

fn count_json_array_items(raw_json: &str) -> usize {
    serde_json::from_str::<Vec<serde_json::Value>>(raw_json)
        .map(|items| items.len())
        .unwrap_or(0)
}

fn sync_status_string(status: &SyncRunStatus) -> String {
    match status {
        SyncRunStatus::Ready => "ready",
        SyncRunStatus::Blocked => "blocked",
        SyncRunStatus::Partial => "partial",
        SyncRunStatus::Success => "success",
        SyncRunStatus::Failed => "failed",
    }
    .to_owned()
}

fn review_sufficiency_string(value: ReviewSufficiency) -> String {
    value.as_str().to_owned()
}

fn average(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(round_metric(
            values.iter().sum::<f64>() / values.len() as f64,
        ))
    }
}

fn round_metric(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| RingmasterError::Config(format!("formatting timestamp failed: {error}")))
}

impl PrivacyProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Redacted => "redacted",
            Self::Balanced => "balanced",
            Self::Full => "full",
        }
    }
}

impl SnapshotSourceMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Demo => "demo",
        }
    }
}

impl TryFrom<crate::cli::PrivacyProfileArg> for PrivacyProfile {
    type Error = RingmasterError;

    fn try_from(value: crate::cli::PrivacyProfileArg) -> Result<Self> {
        Ok(match value {
            crate::cli::PrivacyProfileArg::Redacted => Self::Redacted,
            crate::cli::PrivacyProfileArg::Balanced => Self::Balanced,
            crate::cli::PrivacyProfileArg::Full => Self::Full,
        })
    }
}

impl From<EffectDirection> for String {
    fn from(value: EffectDirection) -> Self {
        value.as_str().to_owned()
    }
}

impl From<PatternMetric> for String {
    fn from(value: PatternMetric) -> Self {
        value.as_str().to_owned()
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::{PrivacyProfile, SNAPSHOT_SCHEMA_VERSION, resolve_scope};
    use crate::config::Config;
    use crate::oura::models::{AuthStatus, CapabilityReport};
    use crate::store::Store;
    use crate::store::queries::{DailyActivityRecord, DailyReadinessRecord, DailySleepRecord};

    fn seed_history(store: &Store) {
        for (day, sleep, readiness, activity) in [
            ("2026-04-06", 81, 74, 69),
            ("2026-04-07", 82, 75, 71),
            ("2026-04-08", 84, 78, 73),
        ] {
            store
                .imports()
                .upsert_daily_sleep(&DailySleepRecord {
                    oura_id: None,
                    day: day.to_owned(),
                    sleep_score: Some(sleep),
                    raw_cache_key: None,
                    updated_at: format!("{day}T06:00:00Z"),
                })
                .unwrap_or_else(|error| panic!("sleep row should seed: {error}"));
            store
                .imports()
                .upsert_daily_readiness(&DailyReadinessRecord {
                    oura_id: None,
                    day: day.to_owned(),
                    readiness_score: Some(readiness),
                    temperature_deviation: None,
                    temperature_trend_deviation: None,
                    raw_cache_key: None,
                    updated_at: format!("{day}T06:01:00Z"),
                })
                .unwrap_or_else(|error| panic!("readiness row should seed: {error}"));
            store
                .imports()
                .upsert_daily_activity(&DailyActivityRecord {
                    oura_id: None,
                    day: day.to_owned(),
                    activity_score: Some(activity),
                    active_calories: 420,
                    steps: 8_400,
                    total_calories: 2_300,
                    raw_cache_key: None,
                    updated_at: format!("{day}T06:02:00Z"),
                })
                .unwrap_or_else(|error| panic!("activity row should seed: {error}"));
        }
    }

    fn auth_status() -> AuthStatus {
        AuthStatus {
            configured: true,
            callback_url: "http://localhost/callback".to_owned(),
            requested_scopes: vec!["daily".to_owned()],
            granted_scopes: vec!["daily".to_owned()],
            missing_fields: Vec::new(),
            capability_report: CapabilityReport::from_scopes(
                &["daily".to_owned()],
                &["daily".to_owned()],
            ),
            auth_timeout_secs: 30,
            secret_backend: "memory".to_owned(),
            access_token_stored: true,
            refresh_token_stored: true,
            access_token_expires_at: Some("2026-04-08T06:10:00Z".to_owned()),
            last_authenticated_at: Some("2026-04-08T06:00:00Z".to_owned()),
            last_refresh_at: Some("2026-04-08T06:05:00Z".to_owned()),
            account_id: Some("user-123".to_owned()),
            account_email: Some("user@example.com".to_owned()),
            last_error: None,
        }
    }

    #[test]
    fn resolves_today_scope_from_latest_source_day() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        seed_history(&store);

        let scope = resolve_scope(&store, "today")
            .unwrap_or_else(|error| panic!("scope should resolve: {error}"));

        assert_eq!(scope.start_day, "2026-04-08");
        assert_eq!(scope.end_day, "2026-04-08");
    }

    #[test]
    fn redacted_export_omits_personal_identifiers() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        seed_history(&store);
        let config = Config::load().unwrap_or_else(|error| panic!("config should load: {error}"));
        let scope = resolve_scope(&store, "day:2026-04-08")
            .unwrap_or_else(|error| panic!("scope should resolve: {error}"));
        let export = super::export_snapshot(
            &config,
            &store,
            &auth_status(),
            super::SnapshotSourceMode::Live,
            None,
            &scope,
            PrivacyProfile::Redacted,
        )
        .unwrap_or_else(|error| panic!("snapshot export should succeed: {error}"));

        assert_eq!(export.bundle.schema_version, SNAPSHOT_SCHEMA_VERSION);
        assert!(!export.pretty_json.contains("user@example.com"));
        assert!(!export.pretty_json.contains("user-123"));
        assert!(!export.pretty_json.contains("callback"));
    }

    #[test]
    fn redacted_catalog_metadata_stays_compact_and_safe() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        seed_history(&store);
        let config = Config::load().unwrap_or_else(|error| panic!("config should load: {error}"));
        let scope = resolve_scope(&store, "day:2026-04-08")
            .unwrap_or_else(|error| panic!("scope should resolve: {error}"));
        let export = super::export_snapshot(
            &config,
            &store,
            &auth_status(),
            super::SnapshotSourceMode::Live,
            None,
            &scope,
            PrivacyProfile::Redacted,
        )
        .unwrap_or_else(|error| panic!("snapshot export should succeed: {error}"));
        store
            .analysis()
            .upsert_snapshot_export(&export.manifest_record, &export.provenance_records)
            .unwrap_or_else(|error| panic!("snapshot export should persist: {error}"));

        let record = store
            .analysis()
            .snapshot_export(&export.bundle.metadata.snapshot_hash)
            .unwrap_or_else(|error| panic!("snapshot export should load: {error}"))
            .unwrap_or_else(|| panic!("snapshot export should exist"));

        assert!(!record.freshness_summary.contains("user@example.com"));
        assert!(!record.capability_summary.contains("user-123"));
        assert!(!record.provenance_summary.contains("callback"));
        assert!(record.freshness_summary.contains("latest_source_day"));
    }
}
