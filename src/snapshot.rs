use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use time::{Date, Duration, OffsetDateTime};

use crate::config::Config;
use crate::error::{Result, RingmasterError};
use crate::evidence::registry::{
    EvidenceDescriptor, PopulationProfile, evidence_registry_version, resolve_evidence_descriptor,
};
use crate::oura::models::{AuthStatus, CapabilityReport};
use crate::review::features::ReviewSufficiency;
use crate::review::registry::{WeeklyAggregation, signal_definition};
use crate::store::Store;
use crate::store::queries::{
    AiArtifactRecord, ContextEventFamily, ContextEventRecord, DailyActivityRecord,
    DailyCardiovascularAgeRecord, DailyOverviewRow, DailyResilienceRecord, DailyStressRecord,
    EffectDirection, PatternMetric, PatternRelationWindow, PatternSummaryRecord, RecordCounts,
    RestModePeriodRecord, ReviewSignalDayRecord, SleepTimeRecord, SnapshotExportRecord,
    SnapshotProvenanceRefRecord, SyncRunStatus, SyncStateRecord, Vo2MaxRecord,
};
use crate::time_utils::current_local_day_string;

pub const SNAPSHOT_SCHEMA_VERSION: &str = "ringmaster.snapshot.v3";
const SUPPORTED_SNAPSHOT_SCHEMA_VERSIONS: &[&str] = &[
    "ringmaster.snapshot.v1",
    "ringmaster.snapshot.v2",
    SNAPSHOT_SCHEMA_VERSION,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
    pub created_at: String,
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

struct SnapshotSourceData {
    sync_states: Vec<SyncStateRecord>,
    capability_report: CapabilityReport,
    daily_history_all: Vec<DailyOverviewRow>,
    daily_history: Vec<DailyOverviewRow>,
    daily_activity: Vec<DailyActivityRecord>,
    sleep_time: Vec<SleepTimeRecord>,
    daily_stress: Vec<DailyStressRecord>,
    daily_resilience: Vec<DailyResilienceRecord>,
    cardiovascular_age: Vec<DailyCardiovascularAgeRecord>,
    vo2_max: Vec<Vo2MaxRecord>,
    rest_mode_periods: Vec<RestModePeriodRecord>,
    context_events: Vec<ContextEventRecord>,
    pattern_summaries: Vec<PatternSummaryRecord>,
    review_signals: Vec<ReviewSignalDayRecord>,
    trend_review_signals: Vec<ReviewSignalDayRecord>,
    heartrate_daily_averages: Vec<DayValuePoint>,
    latest_source_day: Option<String>,
    latest_review_day: Option<String>,
    record_counts: RecordCounts,
    generated_at: String,
    schema_version: u32,
}

struct SnapshotMetricExports {
    daily_scores: Vec<SnapshotDailyScore>,
    activity: Vec<SnapshotActivityDay>,
    heartrate_daily_averages: Vec<SnapshotMetricPoint>,
    sleep_windows: Vec<SnapshotSleepWindow>,
    stress: Vec<SnapshotStressDay>,
    resilience: Vec<SnapshotResilienceDay>,
    cardiovascular_age: Vec<SnapshotMetricPoint>,
    vo2_max: Vec<SnapshotMetricPoint>,
    rest_mode_periods: Vec<SnapshotRestModePeriod>,
    context_events: Vec<SnapshotContextEvent>,
    pattern_summaries: Vec<SnapshotPatternSummary>,
    review_signals: Vec<SnapshotReviewSignal>,
    provenance_records: Vec<SnapshotProvenanceRefRecord>,
}

struct SnapshotManifestContext<'a> {
    compact_json: &'a str,
    fixture_dir: Option<&'a Path>,
    scope: &'a ResolvedSnapshotScope,
    privacy_profile: PrivacyProfile,
    source_mode: SnapshotSourceMode,
    snapshot_hash: &'a str,
    generated_at: &'a str,
    catalog_summary: &'a SnapshotCatalogSummary,
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
    #[serde(default = "default_snapshot_evidence_registry_version")]
    pub evidence_registry_version: String,
    #[serde(default = "default_snapshot_population_profile")]
    pub active_population_profile: PopulationProfile,
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
    #[serde(default)]
    pub sleep_duration_seconds: Option<i64>,
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
    #[serde(default)]
    pub evidence: Option<EvidenceDescriptor>,
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
    #[serde(default)]
    pub evidence: Option<EvidenceDescriptor>,
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
    #[serde(default)]
    pub evidence: Option<EvidenceDescriptor>,
}

fn default_snapshot_evidence_registry_version() -> String {
    evidence_registry_version().to_owned()
}

const fn default_snapshot_population_profile() -> PopulationProfile {
    PopulationProfile::GeneralAdult
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotFollowUpTarget {
    pub label: String,
    pub command: String,
    pub reason: String,
}

#[must_use]
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

/// # Errors
///
/// Returns an error if the scope syntax is invalid or any referenced day cannot be parsed.
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

/// # Errors
///
/// Returns an error if store reads, derivation, serialization, validation, or manifest construction fails.
pub fn export_snapshot(
    config: &Config,
    store: &Store,
    auth_status: &AuthStatus,
    source_mode: SnapshotSourceMode,
    fixture_dir: Option<&Path>,
    scope: &ResolvedSnapshotScope,
    privacy_profile: PrivacyProfile,
) -> Result<SnapshotExportOutput> {
    let source = load_snapshot_source_data(store, auth_status, scope)?;
    let exports = build_snapshot_metric_exports(
        &source,
        privacy_profile,
        config.guidance.active_population_profile,
    );
    let bundle = build_snapshot_bundle(
        auth_status,
        scope,
        privacy_profile,
        source_mode,
        config.guidance.active_population_profile,
        &source,
        &exports,
    )?;
    finalize_snapshot_output(
        bundle,
        fixture_dir,
        scope,
        privacy_profile,
        source_mode,
        &source,
        exports,
    )
}

fn load_snapshot_source_data(
    store: &Store,
    auth_status: &AuthStatus,
    scope: &ResolvedSnapshotScope,
) -> Result<SnapshotSourceData> {
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
    let derived = crate::derive::derive_review_artifacts_between_days(
        store,
        &scope.start_day,
        &scope.end_day,
    )?;
    let review_signal_start_day = comparison_window_start_day(scope)?;
    let trend_review_signals = if review_signal_start_day == scope.start_day {
        derived.review_signal_days.clone()
    } else {
        crate::derive::derive_review_artifacts_between_days(
            store,
            &review_signal_start_day,
            &scope.end_day,
        )?
        .review_signal_days
    };
    let heartrate_daily_averages =
        load_heartrate_daily_averages(store, &scope.start_day, &scope.end_day)?;
    let latest_source_day = store.views().latest_source_day()?;
    let latest_review_day = store.views().latest_review_day()?;
    let record_counts = store.views().record_counts()?;
    let generated_at = deterministic_generated_at(&GeneratedAtInputs {
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
        context_events: &derived.context_events,
        pattern_summaries: &derived.pattern_summaries,
        review_signals: &derived.review_signal_days,
    })?;

    Ok(SnapshotSourceData {
        sync_states,
        capability_report,
        daily_history_all,
        daily_history,
        daily_activity,
        sleep_time,
        daily_stress,
        daily_resilience,
        cardiovascular_age,
        vo2_max,
        rest_mode_periods,
        context_events: derived.context_events,
        pattern_summaries: derived.pattern_summaries,
        review_signals: derived.review_signal_days,
        trend_review_signals,
        heartrate_daily_averages,
        latest_source_day,
        latest_review_day,
        record_counts,
        generated_at,
        schema_version: store.metadata().schema_version()?,
    })
}

fn build_snapshot_metric_exports(
    source: &SnapshotSourceData,
    privacy_profile: PrivacyProfile,
    active_population: PopulationProfile,
) -> SnapshotMetricExports {
    let generated_at = &source.generated_at;
    let mut provenance_records = Vec::new();
    let daily_scores =
        export_daily_scores(&source.daily_history, generated_at, &mut provenance_records);
    let activity = export_activity_days(
        &source.daily_activity,
        generated_at,
        &mut provenance_records,
    );
    let sleep_windows =
        export_sleep_windows(&source.sleep_time, generated_at, &mut provenance_records);
    let stress = export_stress_days(
        &source.daily_stress,
        privacy_profile,
        generated_at,
        &mut provenance_records,
    );
    let resilience = export_resilience_days(
        &source.daily_resilience,
        generated_at,
        &mut provenance_records,
    );
    let cardiovascular_age = export_cardiovascular_age_points(
        &source.cardiovascular_age,
        generated_at,
        &mut provenance_records,
    );
    let vo2_max = export_vo2_max_points(&source.vo2_max, generated_at, &mut provenance_records);
    let rest_mode_periods = export_rest_mode_periods(
        &source.rest_mode_periods,
        generated_at,
        &mut provenance_records,
    );
    let heartrate_daily_averages = export_heartrate_daily_average_points(
        &source.heartrate_daily_averages,
        generated_at,
        &mut provenance_records,
    );
    let context_events = export_context_events(
        &source.context_events,
        privacy_profile,
        generated_at,
        &mut provenance_records,
    );
    let pattern_summaries = export_pattern_summaries(
        &source.pattern_summaries,
        privacy_profile,
        generated_at,
        &mut provenance_records,
        active_population,
    );
    let review_signals = export_review_signals(
        &source.review_signals,
        privacy_profile,
        generated_at,
        &mut provenance_records,
        active_population,
    );
    provenance_records.sort_by(|left, right| left.export_ref.cmp(&right.export_ref));

    SnapshotMetricExports {
        daily_scores,
        activity,
        heartrate_daily_averages,
        sleep_windows,
        stress,
        resilience,
        cardiovascular_age,
        vo2_max,
        rest_mode_periods,
        context_events,
        pattern_summaries,
        review_signals,
        provenance_records,
    }
}

fn build_snapshot_bundle(
    auth_status: &AuthStatus,
    scope: &ResolvedSnapshotScope,
    privacy_profile: PrivacyProfile,
    source_mode: SnapshotSourceMode,
    active_population: PopulationProfile,
    source: &SnapshotSourceData,
    exports: &SnapshotMetricExports,
) -> Result<SnapshotBundleV1> {
    let baselines = build_baselines(&source.daily_history_all, scope)?;
    let trend_summaries = build_trend_summaries(
        &source.daily_history_all,
        &exports.heartrate_daily_averages,
        &source.trend_review_signals,
        scope,
        active_population,
    )?;
    let follow_up_targets =
        build_follow_up_targets(scope, &baselines, &trend_summaries, &exports.review_signals);
    let warnings = build_freshness_warnings(
        &source.sync_states,
        source.latest_source_day.as_deref(),
        source.latest_review_day.as_deref(),
        &source.capability_report,
    );

    Ok(SnapshotBundleV1 {
        schema_version: SNAPSHOT_SCHEMA_VERSION.to_owned(),
        metadata: SnapshotMetadata {
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            generated_at: source.generated_at.clone(),
            snapshot_hash: String::new(),
            scope: scope.normalized_spec.clone(),
            start_day: scope.start_day.clone(),
            end_day: scope.end_day.clone(),
            anchor_day: scope.anchor_day.clone(),
            privacy_profile,
            source_mode,
            schema_version: source.schema_version,
            evidence_registry_version: evidence_registry_version().to_owned(),
            active_population_profile: active_population,
        },
        freshness: build_snapshot_freshness(source, warnings),
        capabilities: build_snapshot_capabilities(auth_status, &source.capability_report),
        record_counts: build_snapshot_record_counts(&source.record_counts, exports),
        metrics: SnapshotMetrics {
            daily_scores: exports.daily_scores.clone(),
            activity: exports.activity.clone(),
            heartrate_daily_averages: exports.heartrate_daily_averages.clone(),
            sleep_windows: exports.sleep_windows.clone(),
            stress: exports.stress.clone(),
            resilience: exports.resilience.clone(),
            cardiovascular_age: exports.cardiovascular_age.clone(),
            vo2_max: exports.vo2_max.clone(),
            rest_mode_periods: exports.rest_mode_periods.clone(),
        },
        baselines,
        trend_summaries,
        context_events: exports.context_events.clone(),
        pattern_summaries: exports.pattern_summaries.clone(),
        review_signals: exports.review_signals.clone(),
        follow_up_targets,
    })
}

fn finalize_snapshot_output(
    mut bundle: SnapshotBundleV1,
    fixture_dir: Option<&Path>,
    scope: &ResolvedSnapshotScope,
    privacy_profile: PrivacyProfile,
    source_mode: SnapshotSourceMode,
    source: &SnapshotSourceData,
    exports: SnapshotMetricExports,
) -> Result<SnapshotExportOutput> {
    let snapshot_hash = snapshot_hash_for_bundle(&bundle)?;
    bundle.metadata.snapshot_hash.clone_from(&snapshot_hash);

    let bundle = round_trip_snapshot_bundle(&bundle)?;
    let compact_json = serde_json::to_string(&bundle)?;
    let pretty_json = serde_json::to_string_pretty(&bundle)?;
    validate_snapshot_bundle(&bundle)?;
    let provenance_records = attach_snapshot_hash(exports.provenance_records, &snapshot_hash);
    let catalog_summary = summarize_snapshot_bundle(&bundle, &provenance_records);

    Ok(SnapshotExportOutput {
        manifest_record: snapshot_manifest_record(&SnapshotManifestContext {
            compact_json: &compact_json,
            fixture_dir,
            scope,
            privacy_profile,
            source_mode,
            snapshot_hash: &snapshot_hash,
            generated_at: &source.generated_at,
            catalog_summary: &catalog_summary,
        })?,
        bundle,
        compact_json,
        pretty_json,
        provenance_records,
    })
}

fn export_daily_scores(
    rows: &[DailyOverviewRow],
    generated_at: &str,
    provenance_records: &mut Vec<SnapshotProvenanceRefRecord>,
) -> Vec<SnapshotDailyScore> {
    rows.iter()
        .map(|row| {
            let export_ref = format!("daily:{}", row.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "daily_overview",
                &row.day,
                generated_at,
            ));
            SnapshotDailyScore {
                export_ref,
                day: row.day.clone(),
                sleep_score: row.sleep_score,
                sleep_duration_seconds: row.sleep_duration_seconds,
                readiness_score: row.readiness_score,
                activity_score: row.activity_score,
            }
        })
        .collect()
}

fn export_activity_days(
    rows: &[DailyActivityRecord],
    generated_at: &str,
    provenance_records: &mut Vec<SnapshotProvenanceRefRecord>,
) -> Vec<SnapshotActivityDay> {
    rows.iter()
        .map(|row| {
            let export_ref = format!("activity:{}", row.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "daily_activity",
                &row.day,
                generated_at,
            ));
            SnapshotActivityDay {
                export_ref,
                day: row.day.clone(),
                active_calories: row.active_calories,
                steps: row.steps,
                total_calories: row.total_calories,
            }
        })
        .collect()
}

fn export_sleep_windows(
    rows: &[SleepTimeRecord],
    generated_at: &str,
    provenance_records: &mut Vec<SnapshotProvenanceRefRecord>,
) -> Vec<SnapshotSleepWindow> {
    rows.iter()
        .map(|row| {
            let export_ref = format!("sleep_time:{}", row.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "sleep_time",
                &row.day,
                generated_at,
            ));
            SnapshotSleepWindow {
                export_ref,
                day: row.day.clone(),
                status: row.status.clone(),
                recommendation: row.recommendation.clone(),
            }
        })
        .collect()
}

fn export_stress_days(
    rows: &[DailyStressRecord],
    privacy_profile: PrivacyProfile,
    generated_at: &str,
    provenance_records: &mut Vec<SnapshotProvenanceRefRecord>,
) -> Vec<SnapshotStressDay> {
    rows.iter()
        .map(|row| {
            let export_ref = format!("stress:{}", row.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "daily_stress",
                &row.day,
                generated_at,
            ));
            SnapshotStressDay {
                export_ref,
                day: row.day.clone(),
                stress_high: row.stress_high,
                recovery_high: row.recovery_high,
                summary: redact_optional_text(privacy_profile, row.day_summary.as_deref()),
            }
        })
        .collect()
}

fn export_resilience_days(
    rows: &[DailyResilienceRecord],
    generated_at: &str,
    provenance_records: &mut Vec<SnapshotProvenanceRefRecord>,
) -> Vec<SnapshotResilienceDay> {
    rows.iter()
        .map(|row| {
            let export_ref = format!("resilience:{}", row.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "daily_resilience",
                &row.day,
                generated_at,
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
        .collect()
}

fn export_cardiovascular_age_points(
    rows: &[DailyCardiovascularAgeRecord],
    generated_at: &str,
    provenance_records: &mut Vec<SnapshotProvenanceRefRecord>,
) -> Vec<SnapshotMetricPoint> {
    rows.iter()
        .map(|row| {
            let export_ref = format!("cardio_age:{}", row.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "daily_cardiovascular_age",
                &row.day,
                generated_at,
            ));
            SnapshotMetricPoint {
                export_ref,
                day: row.day.clone(),
                value: row.vascular_age.map(crate::numeric::i64_to_f64),
            }
        })
        .collect()
}

fn export_vo2_max_points(
    rows: &[Vo2MaxRecord],
    generated_at: &str,
    provenance_records: &mut Vec<SnapshotProvenanceRefRecord>,
) -> Vec<SnapshotMetricPoint> {
    rows.iter()
        .map(|row| {
            let export_ref = format!("vo2_max:{}:{}", row.day, row.recorded_at);
            provenance_records.push(provenance_record(
                &export_ref,
                "vo2_max",
                &format!("{}|{}", row.day, row.recorded_at),
                generated_at,
            ));
            SnapshotMetricPoint {
                export_ref,
                day: row.day.clone(),
                value: row.vo2_max,
            }
        })
        .collect()
}

fn export_rest_mode_periods(
    rows: &[RestModePeriodRecord],
    generated_at: &str,
    provenance_records: &mut Vec<SnapshotProvenanceRefRecord>,
) -> Vec<SnapshotRestModePeriod> {
    rows.iter()
        .map(|row| {
            let export_ref = format!("rest_mode:{}", row.period_id);
            provenance_records.push(provenance_record(
                &export_ref,
                "rest_mode_period",
                &row.period_id,
                generated_at,
            ));
            SnapshotRestModePeriod {
                export_ref,
                start_day: row.start_day.clone(),
                end_day: row.end_day.clone(),
                episode_count: row.episode_count,
                tag_count: count_json_array_items(&row.tags_json),
            }
        })
        .collect()
}

fn export_heartrate_daily_average_points(
    points: &[DayValuePoint],
    generated_at: &str,
    provenance_records: &mut Vec<SnapshotProvenanceRefRecord>,
) -> Vec<SnapshotMetricPoint> {
    points
        .iter()
        .map(|point| {
            let export_ref = format!("heartrate:{}", point.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "heartrate_day",
                &point.day,
                generated_at,
            ));
            SnapshotMetricPoint {
                export_ref,
                day: point.day.clone(),
                value: Some(point.value),
            }
        })
        .collect()
}

fn export_context_events(
    records: &[ContextEventRecord],
    privacy_profile: PrivacyProfile,
    generated_at: &str,
    provenance_records: &mut Vec<SnapshotProvenanceRefRecord>,
) -> Vec<SnapshotContextEvent> {
    records
        .iter()
        .map(|record| {
            let export_ref = format!("context:{}", record.context_event_id);
            provenance_records.push(provenance_record(
                &export_ref,
                "context_event",
                &record.context_event_id,
                generated_at,
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
        .collect()
}

fn export_pattern_summaries(
    records: &[PatternSummaryRecord],
    privacy_profile: PrivacyProfile,
    generated_at: &str,
    provenance_records: &mut Vec<SnapshotProvenanceRefRecord>,
    active_population: PopulationProfile,
) -> Vec<SnapshotPatternSummary> {
    records
        .iter()
        .map(|record| {
            let export_ref = format!("pattern:{}", record.summary_id);
            provenance_records.push(provenance_record(
                &export_ref,
                "pattern_summary",
                &record.summary_id,
                generated_at,
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
                evidence: resolve_evidence_descriptor("pattern_association", active_population),
            }
        })
        .collect()
}

fn export_review_signals(
    records: &[ReviewSignalDayRecord],
    privacy_profile: PrivacyProfile,
    generated_at: &str,
    provenance_records: &mut Vec<SnapshotProvenanceRefRecord>,
    active_population: PopulationProfile,
) -> Vec<SnapshotReviewSignal> {
    records
        .iter()
        .map(|record| {
            let export_ref = format!("signal:{}:{}", record.signal_key, record.day);
            provenance_records.push(provenance_record(
                &export_ref,
                "review_signal",
                &format!("{}|{}", record.signal_key, record.day),
                generated_at,
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
                evidence: resolve_evidence_descriptor(&record.signal_key, active_population),
            }
        })
        .collect()
}

fn build_snapshot_freshness(
    source: &SnapshotSourceData,
    warnings: Vec<String>,
) -> SnapshotFreshness {
    SnapshotFreshness {
        latest_source_day: source.latest_source_day.clone(),
        latest_review_day: source.latest_review_day.clone(),
        warnings,
        sync_states: source
            .sync_states
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
    }
}

fn build_snapshot_capabilities(
    auth_status: &AuthStatus,
    capability_report: &CapabilityReport,
) -> SnapshotCapabilities {
    SnapshotCapabilities {
        requested_scopes: auth_status.requested_scopes.clone(),
        granted_scopes: capability_report
            .granted_scope_names()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        missing_scopes: capability_report
            .missing_scope_names()
            .into_iter()
            .map(str::to_owned)
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
    }
}

fn build_snapshot_record_counts(
    record_counts: &RecordCounts,
    exports: &SnapshotMetricExports,
) -> SnapshotRecordCounts {
    SnapshotRecordCounts {
        daily_history_days: exports.daily_scores.len(),
        heartrate_days: exports.heartrate_daily_averages.len(),
        context_events: exports.context_events.len(),
        pattern_summaries: exports.pattern_summaries.len(),
        review_signals: exports.review_signals.len(),
        raw_tables: build_snapshot_raw_tables(record_counts),
    }
}

fn build_snapshot_raw_tables(record_counts: &RecordCounts) -> BTreeMap<String, u64> {
    BTreeMap::from([
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
    ])
}

fn snapshot_hash_for_bundle(bundle: &SnapshotBundleV1) -> Result<String> {
    match bundle.schema_version.as_str() {
        "ringmaster.snapshot.v1" | "ringmaster.snapshot.v2" => {
            legacy_snapshot_hash_for_bundle(bundle)
        }
        _ => current_snapshot_hash_for_bundle(bundle),
    }
}

fn current_snapshot_hash_for_bundle(bundle: &SnapshotBundleV1) -> Result<String> {
    let mut without_hash = bundle.clone();
    without_hash.metadata.snapshot_hash.clear();
    let serialized_without_hash = serde_json::to_string(&without_hash)?;
    let mut canonical_value = serde_json::from_str::<serde_json::Value>(&serialized_without_hash)?;
    canonicalize_snapshot_hash_value(&mut canonical_value, bundle.schema_version.as_str());
    let canonical_without_hash = serde_json::to_string(&canonical_value)?;
    Ok(hex::encode(Sha256::digest(
        canonical_without_hash.as_bytes(),
    )))
}

fn legacy_snapshot_hash_for_bundle(bundle: &SnapshotBundleV1) -> Result<String> {
    let mut without_hash = bundle.clone();
    without_hash.metadata.snapshot_hash.clear();
    let legacy_json = legacy_snapshot_json_for_bundle(&without_hash)?;
    Ok(hex::encode(Sha256::digest(legacy_json.as_bytes())))
}

fn legacy_snapshot_json_for_bundle(bundle: &SnapshotBundleV1) -> Result<String> {
    let metrics = LegacySnapshotMetricsHashView {
        daily_scores: bundle
            .metrics
            .daily_scores
            .iter()
            .map(|score| LegacySnapshotDailyScoreHashView {
                export_ref: &score.export_ref,
                day: &score.day,
                sleep_score: score.sleep_score,
                readiness_score: score.readiness_score,
                activity_score: score.activity_score,
            })
            .collect(),
        activity: &bundle.metrics.activity,
        heartrate_daily_averages: &bundle.metrics.heartrate_daily_averages,
        sleep_windows: &bundle.metrics.sleep_windows,
        stress: &bundle.metrics.stress,
        resilience: &bundle.metrics.resilience,
        cardiovascular_age: &bundle.metrics.cardiovascular_age,
        vo2_max: &bundle.metrics.vo2_max,
        rest_mode_periods: &bundle.metrics.rest_mode_periods,
    };
    let view = LegacySnapshotBundleHashView {
        schema_version: &bundle.schema_version,
        metadata: LegacySnapshotMetadataHashView {
            app_version: &bundle.metadata.app_version,
            generated_at: &bundle.metadata.generated_at,
            snapshot_hash: &bundle.metadata.snapshot_hash,
            scope: &bundle.metadata.scope,
            start_day: &bundle.metadata.start_day,
            end_day: &bundle.metadata.end_day,
            anchor_day: &bundle.metadata.anchor_day,
            privacy_profile: bundle.metadata.privacy_profile,
            source_mode: bundle.metadata.source_mode,
            schema_version: bundle.metadata.schema_version,
        },
        freshness: &bundle.freshness,
        capabilities: &bundle.capabilities,
        record_counts: &bundle.record_counts,
        metrics,
        baselines: &bundle.baselines,
        trend_summaries: bundle
            .trend_summaries
            .iter()
            .map(|trend| LegacySnapshotTrendSummaryHashView {
                metric_key: &trend.metric_key,
                label: &trend.label,
                direction: &trend.direction,
                summary: &trend.summary,
                current_average: trend.current_average,
                previous_average: trend.previous_average,
            })
            .collect(),
        context_events: &bundle.context_events,
        pattern_summaries: bundle
            .pattern_summaries
            .iter()
            .map(|pattern| LegacySnapshotPatternSummaryHashView {
                export_ref: &pattern.export_ref,
                family: &pattern.family,
                label: &pattern.label,
                metric: &pattern.metric,
                relation_window: &pattern.relation_window,
                sample_count: pattern.sample_count,
                median_delta: pattern.median_delta,
                effect_direction: &pattern.effect_direction,
                confidence: &pattern.confidence,
                summary: &pattern.summary,
            })
            .collect(),
        review_signals: bundle
            .review_signals
            .iter()
            .map(|signal| LegacySnapshotReviewSignalHashView {
                export_ref: &signal.export_ref,
                day: &signal.day,
                signal_key: &signal.signal_key,
                numeric_value: signal.numeric_value,
                text_value: signal.text_value.as_deref(),
                delta: signal.delta,
                z_score: signal.z_score,
                persistence_days: signal.persistence_days,
                sufficiency: &signal.sufficiency,
                stale_days: signal.stale_days,
            })
            .collect(),
        follow_up_targets: &bundle.follow_up_targets,
    };
    serde_json::to_string(&view).map_err(Into::into)
}

#[derive(Serialize)]
struct LegacySnapshotBundleHashView<'a> {
    schema_version: &'a str,
    metadata: LegacySnapshotMetadataHashView<'a>,
    freshness: &'a SnapshotFreshness,
    capabilities: &'a SnapshotCapabilities,
    record_counts: &'a SnapshotRecordCounts,
    metrics: LegacySnapshotMetricsHashView<'a>,
    baselines: &'a [SnapshotBaseline],
    trend_summaries: Vec<LegacySnapshotTrendSummaryHashView<'a>>,
    context_events: &'a [SnapshotContextEvent],
    pattern_summaries: Vec<LegacySnapshotPatternSummaryHashView<'a>>,
    review_signals: Vec<LegacySnapshotReviewSignalHashView<'a>>,
    follow_up_targets: &'a [SnapshotFollowUpTarget],
}

#[derive(Serialize)]
struct LegacySnapshotMetadataHashView<'a> {
    app_version: &'a str,
    generated_at: &'a str,
    snapshot_hash: &'a str,
    scope: &'a str,
    start_day: &'a str,
    end_day: &'a str,
    anchor_day: &'a str,
    privacy_profile: PrivacyProfile,
    source_mode: SnapshotSourceMode,
    schema_version: u32,
}

#[derive(Serialize)]
struct LegacySnapshotMetricsHashView<'a> {
    daily_scores: Vec<LegacySnapshotDailyScoreHashView<'a>>,
    activity: &'a [SnapshotActivityDay],
    heartrate_daily_averages: &'a [SnapshotMetricPoint],
    sleep_windows: &'a [SnapshotSleepWindow],
    stress: &'a [SnapshotStressDay],
    resilience: &'a [SnapshotResilienceDay],
    cardiovascular_age: &'a [SnapshotMetricPoint],
    vo2_max: &'a [SnapshotMetricPoint],
    rest_mode_periods: &'a [SnapshotRestModePeriod],
}

#[derive(Serialize)]
struct LegacySnapshotTrendSummaryHashView<'a> {
    metric_key: &'a str,
    label: &'a str,
    direction: &'a str,
    summary: &'a str,
    current_average: Option<f64>,
    previous_average: Option<f64>,
}

#[derive(Serialize)]
struct LegacySnapshotPatternSummaryHashView<'a> {
    export_ref: &'a str,
    family: &'a str,
    label: &'a str,
    metric: &'a str,
    relation_window: &'a str,
    sample_count: u32,
    median_delta: f64,
    effect_direction: &'a str,
    confidence: &'a str,
    summary: &'a str,
}

#[derive(Serialize)]
struct LegacySnapshotReviewSignalHashView<'a> {
    export_ref: &'a str,
    day: &'a str,
    signal_key: &'a str,
    numeric_value: Option<f64>,
    text_value: Option<&'a str>,
    delta: Option<f64>,
    z_score: Option<f64>,
    persistence_days: u32,
    sufficiency: &'a str,
    stale_days: u32,
}

#[derive(Serialize)]
struct LegacySnapshotDailyScoreHashView<'a> {
    export_ref: &'a str,
    day: &'a str,
    sleep_score: Option<u8>,
    readiness_score: Option<u8>,
    activity_score: Option<u8>,
}

fn canonicalize_snapshot_hash_value(value: &mut serde_json::Value, schema_version: &str) {
    if !matches!(
        schema_version,
        "ringmaster.snapshot.v1" | "ringmaster.snapshot.v2"
    ) {
        return;
    }

    let Some(bundle) = value.as_object_mut() else {
        return;
    };
    if let Some(metadata) = bundle
        .get_mut("metadata")
        .and_then(serde_json::Value::as_object_mut)
    {
        metadata.remove("evidence_registry_version");
        metadata.remove("active_population_profile");
    }
    if let Some(metrics) = bundle
        .get_mut("metrics")
        .and_then(serde_json::Value::as_object_mut)
        && let Some(daily_scores) = metrics
            .get_mut("daily_scores")
            .and_then(serde_json::Value::as_array_mut)
    {
        for score in daily_scores {
            if let Some(score_object) = score.as_object_mut() {
                score_object.remove("sleep_duration_seconds");
            }
        }
    }
    for field in ["trend_summaries", "pattern_summaries", "review_signals"] {
        if let Some(entries) = bundle
            .get_mut(field)
            .and_then(serde_json::Value::as_array_mut)
        {
            for entry in entries {
                if let Some(entry_object) = entry.as_object_mut() {
                    entry_object.remove("evidence");
                }
            }
        }
    }
}

fn round_trip_snapshot_bundle(bundle: &SnapshotBundleV1) -> Result<SnapshotBundleV1> {
    let serialized_with_hash = serde_json::to_string(bundle)?;
    serde_json::from_str::<SnapshotBundleV1>(&serialized_with_hash).map_err(Into::into)
}

fn attach_snapshot_hash(
    provenance_records: Vec<SnapshotProvenanceRefRecord>,
    snapshot_hash: &str,
) -> Vec<SnapshotProvenanceRefRecord> {
    provenance_records
        .into_iter()
        .map(|mut record| {
            snapshot_hash.clone_into(&mut record.snapshot_hash);
            record
        })
        .collect()
}

fn snapshot_manifest_record(context: &SnapshotManifestContext<'_>) -> Result<SnapshotExportRecord> {
    Ok(SnapshotExportRecord {
        snapshot_hash: context.snapshot_hash.to_owned(),
        schema_version: SNAPSHOT_SCHEMA_VERSION.to_owned(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        generated_at: context.generated_at.to_owned(),
        scope: context.scope.normalized_spec.clone(),
        start_day: context.scope.start_day.clone(),
        end_day: context.scope.end_day.clone(),
        anchor_day: context.scope.anchor_day.clone(),
        day_count: u32::try_from(context.scope.day_count).unwrap_or(u32::MAX),
        privacy_profile: context.privacy_profile.as_str().to_owned(),
        source_mode: context.source_mode.as_str().to_owned(),
        fixture_dir: context.fixture_dir.map(|path| path.display().to_string()),
        latest_source_day: context.catalog_summary.latest_source_day.clone(),
        latest_review_day: context.catalog_summary.latest_review_day.clone(),
        freshness_summary: context.catalog_summary.freshness_summary.clone(),
        trust_summary: context.catalog_summary.trust_summary.clone(),
        capability_summary: context.catalog_summary.capability_summary.clone(),
        provenance_summary: context.catalog_summary.provenance_summary.clone(),
        snapshot_json: context.compact_json.to_owned(),
        created_at: now_rfc3339()?,
    })
}

/// # Errors
///
/// Returns an error if the artifact file cannot be read or the snapshot payload is invalid.
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

#[must_use]
pub fn rebuild_follow_up_targets(bundle: &SnapshotBundleV1) -> Vec<SnapshotFollowUpTarget> {
    let scope = ResolvedSnapshotScope {
        raw_spec: bundle.metadata.scope.clone(),
        normalized_spec: bundle.metadata.scope.clone(),
        start_day: bundle.metadata.start_day.clone(),
        end_day: bundle.metadata.end_day.clone(),
        anchor_day: bundle.metadata.anchor_day.clone(),
        day_count: 0,
    };
    build_follow_up_targets(
        &scope,
        &bundle.baselines,
        &bundle.trend_summaries,
        &bundle.review_signals,
    )
}

#[must_use]
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

fn validate_snapshot_schema_version(schema_version: &str) -> Result<()> {
    if !SUPPORTED_SNAPSHOT_SCHEMA_VERSIONS.contains(&schema_version) {
        return Err(RingmasterError::Config(format!(
            "unsupported snapshot schema version `{schema_version}`"
        )));
    }
    Ok(())
}

fn validated_snapshot_hash(bundle: &SnapshotBundleV1) -> Result<&str> {
    let observed_hash = bundle.metadata.snapshot_hash.trim();
    if observed_hash.is_empty() {
        return Err(RingmasterError::Config(
            "snapshot artifact is missing metadata.snapshot_hash".to_owned(),
        ));
    }
    Ok(observed_hash)
}

/// # Errors
///
/// Returns an error if the JSON cannot be decoded or the decoded bundle fails validation.
pub fn deserialize_snapshot_bundle(raw_json: &str) -> Result<SnapshotBundleV1> {
    let bundle = serde_json::from_str::<SnapshotBundleV1>(raw_json)?;
    validate_snapshot_bundle(&bundle)?;
    Ok(bundle)
}

/// # Errors
///
/// Returns an error if the bundle is invalid or cannot be serialized canonically.
pub fn canonicalize_snapshot_bundle(bundle: &SnapshotBundleV1) -> Result<String> {
    validate_snapshot_bundle(bundle)?;
    serde_json::to_string(bundle).map_err(Into::into)
}

/// # Errors
///
/// Returns an error if the artifact directory cannot be created or the artifact cannot be written.
pub fn write_snapshot_artifact(path: &Path, compact_json: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| RingmasterError::io("creating snapshot artifact directory", error))?;
    }
    fs::write(path, compact_json)
        .map_err(|error| RingmasterError::io("writing snapshot artifact", error))?;
    Ok(())
}

/// # Errors
///
/// Returns an error if the bundle schema version or content hash is invalid.
pub fn validate_snapshot_bundle(bundle: &SnapshotBundleV1) -> Result<()> {
    validate_snapshot_schema_version(&bundle.schema_version)?;

    let observed_hash = validated_snapshot_hash(bundle)?;
    let expected_hash = snapshot_hash_for_bundle(bundle)?;
    if observed_hash != expected_hash {
        return Err(RingmasterError::Config(format!(
            "snapshot hash mismatch: expected `{expected_hash}` but found `{observed_hash}`"
        )));
    }

    Ok(())
}

/// # Errors
///
/// Returns an error if the artifact payload, overview, or rendered briefing cannot be normalized.
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
        created_at: input.created_at,
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

fn comparison_window_start_day(scope: &ResolvedSnapshotScope) -> Result<String> {
    let previous_end = parse_day(&scope.start_day)? - Duration::days(1);
    let comparison_days = i64::try_from(scope.day_count.max(1)).map_err(|error| {
        RingmasterError::Config(format!("snapshot day count is too large: {error}"))
    })?;
    Ok((previous_end - Duration::days(comparison_days - 1)).to_string())
}

fn parse_day(value: &str) -> Result<Date> {
    Date::parse(
        value,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .map_err(|error| RingmasterError::Config(format!("invalid day `{value}`: {error}")))
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
        let average = sum / crate::numeric::usize_to_f64(samples.len());
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

fn deterministic_generated_at(inputs: &GeneratedAtInputs<'_>) -> Result<String> {
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
    review_signals: &[ReviewSignalDayRecord],
    scope: &ResolvedSnapshotScope,
    active_population: PopulationProfile,
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

    let mut summaries = vec![
        build_metric_trend(
            "sleep_score",
            "Sleep score",
            &scope_values
                .iter()
                .filter_map(|row| row.sleep_score.map(f64::from))
                .collect::<Vec<_>>(),
            &previous_values
                .iter()
                .filter_map(|row| row.sleep_score.map(f64::from))
                .collect::<Vec<_>>(),
            active_population,
        ),
        build_metric_trend(
            "readiness_score",
            "Readiness score",
            &scope_values
                .iter()
                .filter_map(|row| row.readiness_score.map(f64::from))
                .collect::<Vec<_>>(),
            &previous_values
                .iter()
                .filter_map(|row| row.readiness_score.map(f64::from))
                .collect::<Vec<_>>(),
            active_population,
        ),
        build_metric_trend(
            "activity_score",
            "Activity score",
            &scope_values
                .iter()
                .filter_map(|row| row.activity_score.map(f64::from))
                .collect::<Vec<_>>(),
            &previous_values
                .iter()
                .filter_map(|row| row.activity_score.map(f64::from))
                .collect::<Vec<_>>(),
            active_population,
        ),
        build_metric_trend(
            "sleep_duration",
            "Sleep duration",
            &scope_values
                .iter()
                .filter_map(|row| row.sleep_duration_seconds.map(crate::numeric::i64_to_f64))
                .map(|seconds| seconds / 3600.0)
                .collect::<Vec<_>>(),
            &previous_values
                .iter()
                .filter_map(|row| row.sleep_duration_seconds.map(crate::numeric::i64_to_f64))
                .map(|seconds| seconds / 3600.0)
                .collect::<Vec<_>>(),
            active_population,
        ),
    ];
    for signal_key in ["weekly_activity_minutes", "weekly_activity_distribution"] {
        if let Some(summary) = build_signal_trend_from_review_signals(
            review_signals,
            scope,
            signal_key,
            active_population,
        )? {
            summaries.push(summary);
        }
    }

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
            &current,
            &previous,
            active_population,
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
    current_values: &[f64],
    previous_values: &[f64],
    active_population: PopulationProfile,
) -> SnapshotTrendSummary {
    let current_average = average(current_values);
    let previous_average = average(previous_values);
    let direction =
        current_average
            .zip(previous_average)
            .map_or("insufficient", |(current, previous)| {
                if (current - previous).abs() < 0.5 {
                    "flat"
                } else if current > previous {
                    "higher"
                } else {
                    "lower"
                }
            });
    let summary = match current_average.zip(previous_average) {
        Some((current, previous)) => format!(
            "{label} averaged {current:.1} in-scope versus {previous:.1} in the comparison window."
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
        evidence: resolve_evidence_descriptor(metric_key, active_population),
    }
}

fn build_signal_trend_from_review_signals(
    review_signals: &[ReviewSignalDayRecord],
    scope: &ResolvedSnapshotScope,
    signal_key: &str,
    active_population: PopulationProfile,
) -> Result<Option<SnapshotTrendSummary>> {
    let previous_end = parse_day(&scope.start_day)? - Duration::days(1);
    let scope_days = i64::try_from(scope.day_count).map_err(|error| {
        RingmasterError::Config(format!("snapshot day count is too large: {error}"))
    })?;
    let previous_start = previous_end - Duration::days(scope_days - 1);

    let current_values = review_signals
        .iter()
        .filter(|signal| signal.signal_key == signal_key)
        .filter(|signal| {
            signal.day.as_str() >= scope.start_day.as_str()
                && signal.day.as_str() <= scope.end_day.as_str()
        })
        .filter_map(|signal| signal.numeric_value)
        .collect::<Vec<_>>();
    let previous_values = review_signals
        .iter()
        .filter(|signal| signal.signal_key == signal_key)
        .filter(|signal| {
            parse_day(&signal.day)
                .ok()
                .is_some_and(|day| day >= previous_start && day <= previous_end)
        })
        .filter_map(|signal| signal.numeric_value)
        .collect::<Vec<_>>();

    if current_values.is_empty() && previous_values.is_empty() {
        return Ok(None);
    }

    let definition = signal_definition(signal_key).ok_or_else(|| {
        RingmasterError::Config(format!(
            "snapshot trend summary is missing a review signal definition for `{signal_key}`"
        ))
    })?;
    let current_aggregate = aggregate_signal_window(definition.weekly_aggregation, &current_values);
    let previous_aggregate =
        aggregate_signal_window(definition.weekly_aggregation, &previous_values);
    let label = resolve_evidence_descriptor(signal_key, active_population).map_or_else(
        || prettify_metric_key(signal_key),
        |descriptor| descriptor.label,
    );
    Ok(Some(build_aggregated_signal_trend(
        signal_key,
        &label,
        current_aggregate,
        previous_aggregate,
        definition.weekly_aggregation,
        active_population,
    )))
}

fn aggregate_signal_window(aggregation: WeeklyAggregation, values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }

    match aggregation {
        WeeklyAggregation::Mean => {
            Some(values.iter().sum::<f64>() / crate::numeric::usize_to_f64(values.len()))
        }
        WeeklyAggregation::Sum | WeeklyAggregation::Count => Some(values.iter().sum()),
        WeeklyAggregation::Latest => values.last().copied(),
    }
}

fn build_aggregated_signal_trend(
    metric_key: &str,
    label: &str,
    current_value: Option<f64>,
    previous_value: Option<f64>,
    aggregation: WeeklyAggregation,
    active_population: PopulationProfile,
) -> SnapshotTrendSummary {
    let direction = current_value.zip(previous_value).map_or_else(
        || "insufficient".to_owned(),
        |(current, previous)| {
            if (current - previous).abs() < 0.5 {
                "flat".to_owned()
            } else if current > previous {
                "higher".to_owned()
            } else {
                "lower".to_owned()
            }
        },
    );
    let summary = match current_value.zip(previous_value) {
        Some((current, previous)) => match aggregation {
            WeeklyAggregation::Mean => format!(
                "{label} averaged {current:.1} in-scope versus {previous:.1} in the comparison window."
            ),
            WeeklyAggregation::Sum => format!(
                "{label} totaled {current:.1} in-scope versus {previous:.1} in the comparison window."
            ),
            WeeklyAggregation::Count => format!(
                "{label} covered {current:.0} days in-scope versus {previous:.0} in the comparison window."
            ),
            WeeklyAggregation::Latest => format!(
                "{label} ended at {current:.1} in-scope versus {previous:.1} in the comparison window."
            ),
        },
        None => format!("Not enough {label} samples were available to compare windows."),
    };

    SnapshotTrendSummary {
        metric_key: metric_key.to_owned(),
        label: label.to_owned(),
        direction,
        summary,
        current_average: current_value,
        previous_average: previous_value,
        evidence: resolve_evidence_descriptor(metric_key, active_population),
    }
}

fn prettify_metric_key(metric_key: &str) -> String {
    metric_key
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            format!(
                "{}{}",
                first.to_ascii_uppercase(),
                chars.as_str().to_ascii_lowercase()
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
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
    baselines: &[SnapshotBaseline],
    trend_summaries: &[SnapshotTrendSummary],
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
    targets.extend(
        ranked_signal_targets(review_signals)
            .or_else(|| ranked_baseline_targets(scope, baselines, trend_summaries))
            .unwrap_or_else(|| ranked_missing_signal_targets(review_signals)),
    );
    targets
}

fn signal_focus(signal_key: &str) -> Option<&'static str> {
    if signal_key.contains("stress") {
        Some("stress")
    } else if signal_key.contains("recovery") {
        Some("recovery")
    } else if signal_key.contains("sleep") {
        Some("sleep")
    } else if signal_key.contains("activity")
        || signal_key.contains("calorie")
        || signal_key.contains("step")
        || signal_key.contains("workout")
    {
        Some("activity")
    } else if signal_key.contains("readiness")
        || signal_key.contains("temperature")
        || signal_key.contains("cardio")
        || signal_key.contains("vascular")
        || signal_key.contains("vo2")
        || signal_key.contains("resilience")
    {
        Some("readiness")
    } else {
        None
    }
}

fn metric_focus(metric_key: &str) -> Option<&'static str> {
    match metric_key {
        "sleep_score" => Some("sleep"),
        "readiness_score" => Some("readiness"),
        "activity_score" => Some("activity"),
        _ => None,
    }
}

fn sufficiency_rank(sufficiency: &str) -> u8 {
    match sufficiency {
        "strong" => 3,
        "medium" => 2,
        "thin" => 1,
        _ => 0,
    }
}

fn ranked_signal_targets(
    review_signals: &[SnapshotReviewSignal],
) -> Option<Vec<SnapshotFollowUpTarget>> {
    let mut ranked = review_signals
        .iter()
        .filter_map(|signal| {
            let focus = signal_focus(&signal.signal_key)?;
            let sufficiency = sufficiency_rank(&signal.sufficiency);
            (sufficiency > 0).then_some((
                focus,
                sufficiency,
                signal.z_score.map_or(0.0, f64::abs),
                signal.delta.map_or(0.0, f64::abs),
                signal.persistence_days,
                signal.stale_days,
                signal,
            ))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.total_cmp(&left.2))
            .then_with(|| right.3.total_cmp(&left.3))
            .then_with(|| right.4.cmp(&left.4))
            .then_with(|| left.5.cmp(&right.5))
            .then_with(|| left.6.signal_key.cmp(&right.6.signal_key))
    });

    let mut seen_focuses = BTreeSet::new();
    let targets = ranked
        .into_iter()
        .filter_map(|(focus, _, _, _, _, _, signal)| {
            seen_focuses
                .insert(focus)
                .then_some(SnapshotFollowUpTarget {
                    label: format!("Investigate {}", signal.signal_key),
                    command: format!(
                        "review investigate --focus {focus} --anchor-day {}",
                        signal.day
                    ),
                    reason:
                        "Follow the strongest structured signal back into local review tooling."
                            .to_owned(),
                })
        })
        .take(3)
        .collect::<Vec<_>>();

    (!targets.is_empty()).then_some(targets)
}

fn ranked_missing_signal_targets(
    review_signals: &[SnapshotReviewSignal],
) -> Vec<SnapshotFollowUpTarget> {
    let mut ranked = review_signals
        .iter()
        .filter_map(|signal| {
            let focus = signal_focus(&signal.signal_key)?;
            Some((
                focus,
                signal.z_score.map_or(0.0, f64::abs),
                signal.delta.map_or(0.0, f64::abs),
                signal.persistence_days,
                signal.stale_days,
                signal,
            ))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| right.2.total_cmp(&left.2))
            .then_with(|| right.3.cmp(&left.3))
            .then_with(|| left.4.cmp(&right.4))
            .then_with(|| left.5.signal_key.cmp(&right.5.signal_key))
    });

    let mut seen_focuses = BTreeSet::new();
    ranked
        .into_iter()
        .filter_map(|(focus, _, _, _, _, signal)| {
            seen_focuses.insert(focus).then_some(SnapshotFollowUpTarget {
                label: format!("Investigate {}", signal.signal_key),
                command: format!(
                    "review investigate --focus {focus} --anchor-day {}",
                    signal.day
                ),
                reason: "Fall back to the best available sparse review signal when stronger local comparisons are unavailable."
                    .to_owned(),
            })
        })
        .take(3)
        .collect()
}

fn ranked_baseline_targets(
    scope: &ResolvedSnapshotScope,
    baselines: &[SnapshotBaseline],
    trend_summaries: &[SnapshotTrendSummary],
) -> Option<Vec<SnapshotFollowUpTarget>> {
    let mut candidates = BTreeMap::<&'static str, (f64, String)>::new();

    for baseline in baselines {
        let Some(focus) = metric_focus(&baseline.metric_key) else {
            continue;
        };
        let Some(delta) = baseline.delta else {
            continue;
        };
        let delta_abs = delta.abs();
        let reason = format!(
            "Follow the largest local {} delta back into review tooling.",
            baseline.label.to_ascii_lowercase()
        );
        upsert_baseline_candidate(&mut candidates, focus, delta_abs, reason);
    }

    for trend in trend_summaries {
        let Some(focus) = metric_focus(&trend.metric_key) else {
            continue;
        };
        let Some(delta_abs) = trend
            .current_average
            .zip(trend.previous_average)
            .map(|(current, previous)| (current - previous).abs())
        else {
            continue;
        };
        let reason = format!(
            "Follow the clearest local {} trend back into review tooling.",
            trend.label.to_ascii_lowercase()
        );
        upsert_baseline_candidate(&mut candidates, focus, delta_abs, reason);
    }

    let mut ranked = candidates
        .into_iter()
        .map(|(focus, (delta_abs, reason))| (focus, delta_abs, reason))
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1).then_with(|| left.0.cmp(right.0)));

    let targets = ranked
        .into_iter()
        .take(3)
        .map(|(focus, _, reason)| SnapshotFollowUpTarget {
            label: format!("Investigate {focus}"),
            command: format!(
                "review investigate --focus {focus} --anchor-day {}",
                scope.anchor_day
            ),
            reason,
        })
        .collect::<Vec<_>>();

    (!targets.is_empty()).then_some(targets)
}

fn upsert_baseline_candidate(
    candidates: &mut BTreeMap<&'static str, (f64, String)>,
    focus: &'static str,
    delta_abs: f64,
    reason: String,
) {
    match candidates.get_mut(focus) {
        Some((current_delta, current_reason)) if delta_abs > *current_delta => {
            *current_delta = delta_abs;
            *current_reason = reason;
        }
        None => {
            candidates.insert(focus, (delta_abs, reason));
        }
        Some(_) => {}
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

const fn relation_verb(window: PatternRelationWindow) -> &'static str {
    match window {
        PatternRelationWindow::SameDayActivity => "to shift same-day",
        PatternRelationWindow::NextDayReadiness => "to shift next-day",
        PatternRelationWindow::SameNightSleep => "to shift same-night",
    }
}

const fn family_label(family: ContextEventFamily) -> &'static str {
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
            values.iter().sum::<f64>() / crate::numeric::usize_to_f64(values.len()),
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
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Redacted => "redacted",
            Self::Balanced => "balanced",
            Self::Full => "full",
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Redacted => Self::Balanced,
            Self::Balanced => Self::Full,
            Self::Full => Self::Redacted,
        }
    }
}

impl SnapshotSourceMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
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
mod tests {
    use super::{
        PrivacyProfile, ResolvedSnapshotScope, SNAPSHOT_SCHEMA_VERSION, SnapshotBaseline,
        SnapshotBundleV1, SnapshotCapabilities, SnapshotDailyScore, SnapshotFollowUpTarget,
        SnapshotFreshness, SnapshotMetadata, SnapshotMetrics, SnapshotPatternSummary,
        SnapshotRecordCounts, SnapshotReviewSignal, SnapshotSourceMode, SnapshotTrendSummary,
        build_follow_up_targets, canonicalize_snapshot_bundle, deserialize_snapshot_bundle,
        legacy_snapshot_json_for_bundle, resolve_scope, snapshot_hash_for_bundle,
    };
    use crate::config::Config;
    use crate::evidence::registry::resolve_evidence_descriptor;
    use crate::oura::models::{AuthStatus, CapabilityReport};
    use crate::store::Store;
    use crate::store::queries::{
        DailyActivityRecord, DailyReadinessRecord, DailySleepRecord, WorkoutRecord,
    };
    use crate::test_support::{ok, some};

    fn seed_history(store: &Store) {
        for (day, sleep, readiness, activity) in [
            ("2026-04-06", 81, 74, 69),
            ("2026-04-07", 82, 75, 71),
            ("2026-04-08", 84, 78, 73),
        ] {
            ok(
                store.imports().upsert_daily_sleep(&DailySleepRecord {
                    oura_id: None,
                    day: day.to_owned(),
                    sleep_score: Some(sleep),
                    sleep_duration_seconds: Some(27_000),
                    raw_cache_key: None,
                    updated_at: format!("{day}T06:00:00Z"),
                }),
                "sleep row should seed",
            );
            ok(
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
                    }),
                "readiness row should seed",
            );
            ok(
                store.imports().upsert_daily_activity(&DailyActivityRecord {
                    oura_id: None,
                    day: day.to_owned(),
                    activity_score: Some(activity),
                    active_calories: 420,
                    steps: 8_400,
                    total_calories: 2_300,
                    raw_cache_key: None,
                    updated_at: format!("{day}T06:02:00Z"),
                }),
                "activity row should seed",
            );
        }
    }

    fn seed_wide_history(store: &Store) {
        let updated_at = "2026-04-08T12:00:00Z";
        for (day, sleep, readiness, activity) in [
            ("2026-01-01", 82, 79, 60),
            ("2026-01-02", 83, 80, 63),
            ("2026-01-03", 84, 81, 58),
            ("2026-01-04", 85, 82, 64),
            ("2026-01-05", 83, 79, 59),
            ("2026-01-06", 86, 83, 65),
            ("2026-01-07", 82, 78, 57),
            ("2026-01-08", 87, 84, 66),
            ("2026-04-08", 80, 76, 61),
        ] {
            ok(
                store.imports().upsert_daily_sleep(&DailySleepRecord {
                    oura_id: None,
                    day: day.to_owned(),
                    sleep_score: Some(sleep),
                    sleep_duration_seconds: Some(26_400),
                    raw_cache_key: None,
                    updated_at: updated_at.to_owned(),
                }),
                "sleep row should seed",
            );
            ok(
                store
                    .imports()
                    .upsert_daily_readiness(&DailyReadinessRecord {
                        oura_id: None,
                        day: day.to_owned(),
                        readiness_score: Some(readiness),
                        temperature_deviation: None,
                        temperature_trend_deviation: None,
                        raw_cache_key: None,
                        updated_at: updated_at.to_owned(),
                    }),
                "readiness row should seed",
            );
            ok(
                store.imports().upsert_daily_activity(&DailyActivityRecord {
                    oura_id: None,
                    day: day.to_owned(),
                    activity_score: Some(activity),
                    active_calories: 420,
                    steps: 8_400,
                    total_calories: 2_300,
                    raw_cache_key: None,
                    updated_at: updated_at.to_owned(),
                }),
                "activity row should seed",
            );
        }

        for (index, day) in ["2026-01-02", "2026-01-04", "2026-01-06", "2026-01-08"]
            .into_iter()
            .enumerate()
        {
            ok(
                store.imports().upsert_workout(&WorkoutRecord {
                    workout_id: format!("workout-{index}"),
                    day: day.to_owned(),
                    started_at: format!("{day}T18:00:00Z"),
                    ended_at: Some(format!("{day}T18:30:00Z")),
                    timezone: Some("UTC".to_owned()),
                    sport: Some("running".to_owned()),
                    activity: Some("cardio".to_owned()),
                    intensity: Some("moderate".to_owned()),
                    title: "Run".to_owned(),
                    notes: None,
                    source: Some("manual".to_owned()),
                    raw_cache_key: None,
                    updated_at: updated_at.to_owned(),
                }),
                "workout row should seed",
            );
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
        let store = ok(Store::open_test_store(), "store should open");
        seed_history(&store);

        let scope = ok(resolve_scope(&store, "today"), "scope should resolve");

        assert_eq!(scope.start_day, "2026-04-08");
        assert_eq!(scope.end_day, "2026-04-08");
    }

    #[test]
    fn redacted_export_omits_personal_identifiers() {
        let store = ok(Store::open_test_store(), "store should open");
        seed_history(&store);
        let config = ok(Config::load(), "config should load");
        let scope = ok(
            resolve_scope(&store, "day:2026-04-08"),
            "scope should resolve",
        );
        let export = ok(
            super::export_snapshot(
                &config,
                &store,
                &auth_status(),
                super::SnapshotSourceMode::Live,
                None,
                &scope,
                PrivacyProfile::Redacted,
            ),
            "snapshot export should succeed",
        );

        assert_eq!(export.bundle.schema_version, SNAPSHOT_SCHEMA_VERSION);
        assert!(!export.pretty_json.contains("user@example.com"));
        assert!(!export.pretty_json.contains("user-123"));
        assert!(!export.pretty_json.contains("callback"));
    }

    #[test]
    fn redacted_catalog_metadata_stays_compact_and_safe() {
        let store = ok(Store::open_test_store(), "store should open");
        seed_history(&store);
        let config = ok(Config::load(), "config should load");
        let scope = ok(
            resolve_scope(&store, "day:2026-04-08"),
            "scope should resolve",
        );
        let export = ok(
            super::export_snapshot(
                &config,
                &store,
                &auth_status(),
                super::SnapshotSourceMode::Live,
                None,
                &scope,
                PrivacyProfile::Redacted,
            ),
            "snapshot export should succeed",
        );
        ok(
            store
                .analysis()
                .upsert_snapshot_export(&export.manifest_record, &export.provenance_records),
            "snapshot export should persist",
        );

        let record = some(
            ok(
                store
                    .analysis()
                    .snapshot_export(&export.bundle.metadata.snapshot_hash),
                "snapshot export should load",
            ),
            "snapshot export should exist",
        );

        assert!(!record.freshness_summary.contains("user@example.com"));
        assert!(!record.capability_summary.contains("user-123"));
        assert!(!record.provenance_summary.contains("callback"));
        assert!(record.freshness_summary.contains("latest_source_day"));
    }

    #[test]
    fn deserialize_snapshot_bundle_accepts_legacy_v2_artifacts_with_defaulted_fields() {
        let mut bundle = SnapshotBundleV1 {
            schema_version: "ringmaster.snapshot.v2".to_owned(),
            metadata: SnapshotMetadata {
                app_version: "0.1.0".to_owned(),
                generated_at: "2026-04-08T00:00:00Z".to_owned(),
                snapshot_hash: String::new(),
                scope: "day:2026-04-08".to_owned(),
                start_day: "2026-04-08".to_owned(),
                end_day: "2026-04-08".to_owned(),
                anchor_day: "2026-04-08".to_owned(),
                privacy_profile: PrivacyProfile::Redacted,
                source_mode: SnapshotSourceMode::Live,
                schema_version: 17,
                evidence_registry_version: crate::evidence::registry::evidence_registry_version()
                    .to_owned(),
                active_population_profile:
                    crate::evidence::registry::PopulationProfile::GeneralAdult,
            },
            freshness: SnapshotFreshness {
                latest_source_day: Some("2026-04-08".to_owned()),
                latest_review_day: Some("2026-04-08".to_owned()),
                warnings: Vec::new(),
                sync_states: Vec::new(),
            },
            capabilities: SnapshotCapabilities {
                requested_scopes: vec!["daily".to_owned()],
                granted_scopes: vec!["daily".to_owned()],
                missing_scopes: Vec::new(),
                entries: Vec::new(),
            },
            record_counts: SnapshotRecordCounts {
                daily_history_days: 1,
                heartrate_days: 0,
                context_events: 0,
                pattern_summaries: 0,
                review_signals: 0,
                raw_tables: std::collections::BTreeMap::new(),
            },
            metrics: SnapshotMetrics {
                daily_scores: vec![SnapshotDailyScore {
                    export_ref: "daily:2026-04-08".to_owned(),
                    day: "2026-04-08".to_owned(),
                    sleep_score: Some(84),
                    sleep_duration_seconds: None,
                    readiness_score: Some(78),
                    activity_score: Some(73),
                }],
                activity: Vec::new(),
                heartrate_daily_averages: Vec::new(),
                sleep_windows: Vec::new(),
                stress: Vec::new(),
                resilience: Vec::new(),
                cardiovascular_age: Vec::new(),
                vo2_max: Vec::new(),
                rest_mode_periods: Vec::new(),
            },
            baselines: Vec::new(),
            trend_summaries: vec![SnapshotTrendSummary {
                metric_key: "sleep_score".to_owned(),
                label: "Sleep score".to_owned(),
                direction: "higher".to_owned(),
                summary: "Sleep score improved week over week.".to_owned(),
                current_average: Some(86.0),
                previous_average: Some(80.0),
                evidence: resolve_evidence_descriptor(
                    "sleep_score",
                    crate::evidence::PopulationProfile::GeneralAdult,
                ),
            }],
            context_events: Vec::new(),
            pattern_summaries: vec![SnapshotPatternSummary {
                export_ref: "pattern:2026-04-10:sleep".to_owned(),
                family: "sleep".to_owned(),
                label: "Sleep regularity".to_owned(),
                metric: "sleep_score".to_owned(),
                relation_window: "daily".to_owned(),
                sample_count: 3,
                median_delta: 4.0,
                effect_direction: "positive".to_owned(),
                confidence: "medium".to_owned(),
                summary: "Sleep score tends to rise after consistent bedtimes.".to_owned(),
                evidence: resolve_evidence_descriptor(
                    "pattern_association",
                    crate::evidence::PopulationProfile::GeneralAdult,
                ),
            }],
            review_signals: vec![SnapshotReviewSignal {
                export_ref: "review_signal:2026-04-10:sleep_score".to_owned(),
                day: "2026-04-10".to_owned(),
                signal_key: "sleep_score".to_owned(),
                numeric_value: Some(86.0),
                text_value: None,
                delta: Some(6.0),
                z_score: Some(1.2),
                persistence_days: 2,
                sufficiency: "medium".to_owned(),
                stale_days: 0,
                evidence: resolve_evidence_descriptor(
                    "sleep_score",
                    crate::evidence::PopulationProfile::GeneralAdult,
                ),
            }],
            follow_up_targets: Vec::new(),
        };
        bundle.metadata.snapshot_hash = ok(
            snapshot_hash_for_bundle(&bundle),
            "legacy snapshot hash should compute",
        );

        let legacy_json = ok(
            legacy_snapshot_json_for_bundle(&bundle),
            "legacy snapshot json should serialize",
        );
        let loaded = ok(
            deserialize_snapshot_bundle(&legacy_json),
            "legacy v2 snapshot should deserialize",
        );

        assert_eq!(loaded.schema_version, "ringmaster.snapshot.v2");
        assert_eq!(
            loaded.metadata.evidence_registry_version,
            crate::evidence::registry::evidence_registry_version()
        );
        assert_eq!(
            loaded.metadata.active_population_profile,
            crate::evidence::registry::PopulationProfile::GeneralAdult
        );
        assert_eq!(loaded.metrics.daily_scores[0].sleep_duration_seconds, None);

        let canonical = ok(
            canonicalize_snapshot_bundle(&loaded),
            "legacy snapshot should canonicalize",
        );
        assert!(canonical.contains("\"evidence_registry_version\""));
        let reparsed = ok(
            deserialize_snapshot_bundle(&canonical),
            "canonicalized legacy snapshot should still deserialize",
        );
        assert_eq!(
            reparsed.metadata.snapshot_hash,
            loaded.metadata.snapshot_hash
        );
    }

    #[test]
    fn snapshot_export_derives_artifacts_across_requested_range() {
        let store = ok(Store::open_test_store(), "store should open");
        seed_wide_history(&store);
        let config = ok(Config::load(), "config should load");
        let scope = ok(
            resolve_scope(&store, "range:2026-01-01..2026-04-08"),
            "scope should resolve",
        );

        let export = ok(
            super::export_snapshot(
                &config,
                &store,
                &auth_status(),
                super::SnapshotSourceMode::Live,
                None,
                &scope,
                PrivacyProfile::Redacted,
            ),
            "snapshot export should succeed",
        );

        assert!(
            export
                .bundle
                .context_events
                .iter()
                .any(|event| event.anchor_day == "2026-01-02"),
            "wide-range export should include early derived context events"
        );
        assert!(
            !export.bundle.pattern_summaries.is_empty(),
            "wide-range export should include pattern summaries derived from the full range"
        );
        assert!(
            export
                .bundle
                .review_signals
                .iter()
                .any(|signal| signal.day == "2026-01-01"),
            "wide-range export should include early review signals from the requested range"
        );
    }

    #[test]
    fn snapshot_export_weekly_activity_trends_include_previous_window_history_without_leaking_it() {
        let store = ok(Store::open_test_store(), "store should open");
        let updated_at = "2026-04-08T12:00:00Z";
        for day in [
            "2026-03-25",
            "2026-03-26",
            "2026-03-27",
            "2026-03-28",
            "2026-03-29",
            "2026-03-30",
            "2026-03-31",
            "2026-04-01",
            "2026-04-02",
            "2026-04-03",
            "2026-04-04",
            "2026-04-05",
            "2026-04-06",
            "2026-04-07",
        ] {
            ok(
                store.imports().upsert_daily_sleep(&DailySleepRecord {
                    oura_id: None,
                    day: day.to_owned(),
                    sleep_score: Some(80),
                    sleep_duration_seconds: Some(27_000),
                    raw_cache_key: None,
                    updated_at: updated_at.to_owned(),
                }),
                "sleep row should seed",
            );
            ok(
                store
                    .imports()
                    .upsert_daily_readiness(&DailyReadinessRecord {
                        oura_id: None,
                        day: day.to_owned(),
                        readiness_score: Some(78),
                        temperature_deviation: None,
                        temperature_trend_deviation: None,
                        raw_cache_key: None,
                        updated_at: updated_at.to_owned(),
                    }),
                "readiness row should seed",
            );
            ok(
                store.imports().upsert_daily_activity(&DailyActivityRecord {
                    oura_id: None,
                    day: day.to_owned(),
                    activity_score: Some(72),
                    active_calories: 420,
                    steps: 8_000,
                    total_calories: 2_250,
                    raw_cache_key: None,
                    updated_at: updated_at.to_owned(),
                }),
                "activity row should seed",
            );
        }
        for (index, (day, ended_at)) in [
            ("2026-03-25", "2026-03-25T18:30:00Z"),
            ("2026-03-27", "2026-03-27T18:30:00Z"),
            ("2026-04-02", "2026-04-02T18:45:00Z"),
            ("2026-04-04", "2026-04-04T18:45:00Z"),
        ]
        .into_iter()
        .enumerate()
        {
            ok(
                store.imports().upsert_workout(&WorkoutRecord {
                    workout_id: format!("trend-workout-{index}"),
                    day: day.to_owned(),
                    started_at: format!("{day}T18:00:00Z"),
                    ended_at: Some(ended_at.to_owned()),
                    timezone: Some("UTC".to_owned()),
                    sport: Some("running".to_owned()),
                    activity: Some("cardio".to_owned()),
                    intensity: Some("moderate".to_owned()),
                    title: "Run".to_owned(),
                    notes: None,
                    source: Some("manual".to_owned()),
                    raw_cache_key: None,
                    updated_at: updated_at.to_owned(),
                }),
                "workout row should seed",
            );
        }

        let config = ok(Config::load(), "config should load");
        let scope = ok(
            resolve_scope(&store, "range:2026-04-01..2026-04-07"),
            "scope should resolve",
        );
        let export = ok(
            super::export_snapshot(
                &config,
                &store,
                &auth_status(),
                super::SnapshotSourceMode::Live,
                None,
                &scope,
                PrivacyProfile::Redacted,
            ),
            "snapshot export should succeed",
        );

        let minutes_trend = export
            .bundle
            .trend_summaries
            .iter()
            .find(|summary| summary.metric_key == "weekly_activity_minutes")
            .unwrap_or_else(|| panic!("weekly activity minutes trend should exist"));
        let distribution_trend = export
            .bundle
            .trend_summaries
            .iter()
            .find(|summary| summary.metric_key == "weekly_activity_distribution")
            .unwrap_or_else(|| panic!("weekly activity distribution trend should exist"));

        assert_eq!(minutes_trend.previous_average, Some(60.0));
        assert_eq!(minutes_trend.current_average, Some(90.0));
        assert_eq!(distribution_trend.previous_average, Some(2.0));
        assert_eq!(distribution_trend.current_average, Some(2.0));
        assert_eq!(distribution_trend.direction, "flat");
        assert!(
            export
                .bundle
                .review_signals
                .iter()
                .all(|signal| signal.day.as_str() >= "2026-04-01"),
            "snapshot payload should not leak previous-window review signals"
        );
    }

    #[test]
    fn follow_up_targets_route_stress_and_recovery_signals_to_matching_focus() {
        let scope = ResolvedSnapshotScope {
            raw_spec: "today".to_owned(),
            normalized_spec: "day:2026-04-10".to_owned(),
            start_day: "2026-04-10".to_owned(),
            end_day: "2026-04-10".to_owned(),
            anchor_day: "2026-04-10".to_owned(),
            day_count: 1,
        };
        let signals = vec![
            SnapshotReviewSignal {
                export_ref: "signal:stress".to_owned(),
                day: "2026-04-10".to_owned(),
                signal_key: "stress_high".to_owned(),
                numeric_value: Some(1.0),
                text_value: None,
                delta: Some(5.0),
                z_score: Some(2.0),
                persistence_days: 2,
                sufficiency: "strong".to_owned(),
                stale_days: 0,
                evidence: None,
            },
            SnapshotReviewSignal {
                export_ref: "signal:recovery".to_owned(),
                day: "2026-04-10".to_owned(),
                signal_key: "recovery_high".to_owned(),
                numeric_value: Some(1.0),
                text_value: None,
                delta: Some(4.0),
                z_score: Some(1.5),
                persistence_days: 2,
                sufficiency: "strong".to_owned(),
                stale_days: 0,
                evidence: None,
            },
            SnapshotReviewSignal {
                export_ref: "signal:sleep-recovery".to_owned(),
                day: "2026-04-10".to_owned(),
                signal_key: "sleep_recovery".to_owned(),
                numeric_value: Some(78.0),
                text_value: None,
                delta: Some(7.0),
                z_score: Some(1.2),
                persistence_days: 2,
                sufficiency: "medium".to_owned(),
                stale_days: 0,
                evidence: None,
            },
        ];

        let targets = build_follow_up_targets(&scope, &[], &[], &signals);
        let investigate = targets
            .into_iter()
            .skip(2)
            .map(|target: SnapshotFollowUpTarget| target.command)
            .collect::<Vec<_>>();

        assert_eq!(
            investigate,
            vec![
                "review investigate --focus stress --anchor-day 2026-04-10".to_owned(),
                "review investigate --focus recovery --anchor-day 2026-04-10".to_owned(),
            ]
        );
    }

    #[test]
    fn follow_up_targets_backfill_from_score_deltas_when_review_signals_are_sparse() {
        let scope = ResolvedSnapshotScope {
            raw_spec: "today".to_owned(),
            normalized_spec: "day:2026-04-10".to_owned(),
            start_day: "2026-04-10".to_owned(),
            end_day: "2026-04-10".to_owned(),
            anchor_day: "2026-04-10".to_owned(),
            day_count: 1,
        };
        let signals = vec![
            SnapshotReviewSignal {
                export_ref: "signal:active-calories".to_owned(),
                day: "2026-04-10".to_owned(),
                signal_key: "active_calories".to_owned(),
                numeric_value: Some(251.0),
                text_value: None,
                delta: Some(-5.0),
                z_score: Some(-0.3),
                persistence_days: 1,
                sufficiency: "missing".to_owned(),
                stale_days: 0,
                evidence: None,
            },
            SnapshotReviewSignal {
                export_ref: "signal:cardio".to_owned(),
                day: "2026-04-10".to_owned(),
                signal_key: "cardiovascular_age".to_owned(),
                numeric_value: Some(43.0),
                text_value: None,
                delta: Some(1.0),
                z_score: Some(0.2),
                persistence_days: 1,
                sufficiency: "missing".to_owned(),
                stale_days: 0,
                evidence: None,
            },
        ];
        let baselines = vec![
            SnapshotBaseline {
                metric_key: "sleep_score".to_owned(),
                label: "Sleep score".to_owned(),
                scope_average: Some(86.0),
                baseline_average: Some(63.0),
                delta: Some(23.0),
                scope_samples: 1,
                baseline_samples: 14,
            },
            SnapshotBaseline {
                metric_key: "readiness_score".to_owned(),
                label: "Readiness score".to_owned(),
                scope_average: Some(70.0),
                baseline_average: Some(76.0),
                delta: Some(-6.0),
                scope_samples: 1,
                baseline_samples: 14,
            },
        ];
        let trends = vec![SnapshotTrendSummary {
            metric_key: "activity_score".to_owned(),
            label: "Activity score".to_owned(),
            direction: "lower".to_owned(),
            summary: "Activity score dipped slightly.".to_owned(),
            current_average: Some(75.0),
            previous_average: Some(77.0),
            evidence: None,
        }];

        let targets = build_follow_up_targets(&scope, &baselines, &trends, &signals);
        let investigate = targets
            .into_iter()
            .skip(2)
            .map(|target: SnapshotFollowUpTarget| target.command)
            .collect::<Vec<_>>();

        assert_eq!(
            investigate,
            vec![
                "review investigate --focus sleep --anchor-day 2026-04-10".to_owned(),
                "review investigate --focus readiness --anchor-day 2026-04-10".to_owned(),
                "review investigate --focus activity --anchor-day 2026-04-10".to_owned(),
            ]
        );
    }
}
