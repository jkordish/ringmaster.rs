use std::fmt::{Display, Formatter};

use rusqlite::{Connection, OptionalExtension, params};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::error::{OuraProblem, Result, RingmasterError};
use crate::oura::models::{TagRecord, TagSource};
use crate::review::features::ReviewSufficiency;
use crate::store::migrations;
use crate::time_utils::current_local_day_string;

pub const OURA_PROVIDER: &str = "oura";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncRunStatus {
    Ready,
    Blocked,
    Partial,
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncStateRecord {
    pub sync_key: String,
    pub family: String,
    pub status: SyncRunStatus,
    pub cursor: Option<String>,
    pub last_successful_sync_end: Option<String>,
    pub last_attempted_at: String,
    pub last_completed_at: Option<String>,
    pub last_reconcile_end: Option<String>,
    pub oldest_recently_reconciled_at: Option<String>,
    pub message: Option<String>,
    pub granted_scopes: Vec<String>,
    pub last_error: Option<OuraProblem>,
    pub last_error_at: Option<String>,
    pub last_error_kind: Option<String>,
    pub last_error_detail: Option<String>,
    pub failure_count: u32,
    pub next_attempt_after: Option<String>,
    pub last_trigger_source: Option<String>,
    pub last_trigger_detail: Option<String>,
    pub updated_at: String,
}

impl Default for SyncStateRecord {
    fn default() -> Self {
        Self {
            sync_key: String::new(),
            family: String::new(),
            status: SyncRunStatus::Ready,
            cursor: None,
            last_successful_sync_end: None,
            last_attempted_at: String::new(),
            last_completed_at: None,
            last_reconcile_end: None,
            oldest_recently_reconciled_at: None,
            message: None,
            granted_scopes: Vec::new(),
            last_error: None,
            last_error_at: None,
            last_error_kind: None,
            last_error_detail: None,
            failure_count: 0,
            next_attempt_after: None,
            last_trigger_source: None,
            last_trigger_detail: None,
            updated_at: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSessionRecord {
    pub provider: String,
    pub account_id: Option<String>,
    pub account_email: Option<String>,
    pub token_type: String,
    pub granted_scopes: Vec<String>,
    pub access_token_expires_at: Option<String>,
    pub last_authenticated_at: Option<String>,
    pub last_refresh_at: Option<String>,
    pub last_error: Option<OuraProblem>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersonalInfoRecord {
    pub profile_id: String,
    pub age: Option<u16>,
    pub weight: Option<f64>,
    pub height: Option<f64>,
    pub biological_sex: Option<String>,
    pub email: Option<String>,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailySleepRecord {
    pub oura_id: Option<String>,
    pub day: String,
    pub sleep_score: Option<u8>,
    pub sleep_duration_seconds: Option<i64>,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SleepPeriodRecord {
    pub oura_id: String,
    pub day: String,
    pub bedtime_start: Option<String>,
    pub bedtime_end: Option<String>,
    pub sleep_type: Option<String>,
    pub average_heart_rate: Option<f64>,
    pub average_hrv: Option<f64>,
    pub average_breath: Option<f64>,
    pub total_sleep_duration: Option<i64>,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DailySpO2Record {
    pub oura_id: Option<String>,
    pub day: String,
    pub average_spo2: Option<f64>,
    pub breathing_disturbance_index: Option<f64>,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DailyReadinessRecord {
    pub oura_id: Option<String>,
    pub day: String,
    pub readiness_score: Option<u8>,
    pub temperature_deviation: Option<f64>,
    pub temperature_trend_deviation: Option<f64>,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyActivityRecord {
    pub oura_id: Option<String>,
    pub day: String,
    pub activity_score: Option<u8>,
    pub active_calories: i64,
    pub steps: i64,
    pub total_calories: i64,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SleepTimeRecord {
    pub oura_id: Option<String>,
    pub day: String,
    pub status: Option<String>,
    pub recommendation: Option<String>,
    pub optimal_bedtime_start_offset: Option<i64>,
    pub optimal_bedtime_end_offset: Option<i64>,
    pub optimal_bedtime_day_tz: Option<i64>,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyStressRecord {
    pub oura_id: Option<String>,
    pub day: String,
    pub stress_high: Option<i64>,
    pub recovery_high: Option<i64>,
    pub day_summary: Option<String>,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DailyResilienceRecord {
    pub oura_id: Option<String>,
    pub day: String,
    pub level: String,
    pub sleep_recovery: f64,
    pub daytime_recovery: f64,
    pub stress: f64,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyCardiovascularAgeRecord {
    pub day: String,
    pub vascular_age: Option<i64>,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Vo2MaxRecord {
    pub oura_id: Option<String>,
    pub day: String,
    pub recorded_at: String,
    pub vo2_max: Option<f64>,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestModePeriodRecord {
    pub period_id: String,
    pub start_day: String,
    pub start_time: Option<String>,
    pub end_day: Option<String>,
    pub end_time: Option<String>,
    pub episode_count: u32,
    pub tags_json: String,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartrateSampleRecord {
    pub recorded_at: String,
    pub bpm: u16,
    pub source_day: Option<String>,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkoutRecord {
    pub workout_id: String,
    pub day: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub timezone: Option<String>,
    pub sport: Option<String>,
    pub activity: Option<String>,
    pub intensity: Option<String>,
    pub title: String,
    pub notes: Option<String>,
    pub source: Option<String>,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnhancedTagRecord {
    pub enhanced_tag_id: String,
    pub day: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub label: String,
    pub subtype: Option<String>,
    pub comment: Option<String>,
    pub intensity: Option<String>,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub session_id: String,
    pub day: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub kind: Option<String>,
    pub state: Option<String>,
    pub score: Option<i64>,
    pub title: String,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContextEventFamily {
    Workout,
    Tag,
    EnhancedTag,
    Session,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeSemantics {
    Interval,
    Point,
    AllDay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEventRecord {
    pub context_event_id: String,
    pub family: ContextEventFamily,
    pub source_id: String,
    pub anchor_day: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub time_semantics: TimeSemantics,
    pub title: String,
    pub subtype: Option<String>,
    pub notes: Option<String>,
    pub intensity: Option<String>,
    pub metadata_json: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PatternMetric {
    Activity,
    Readiness,
    Sleep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PatternRelationWindow {
    SameDayActivity,
    NextDayReadiness,
    SameNightSleep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DataSufficiency {
    Thin,
    Medium,
    Strong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectDirection {
    Higher,
    Lower,
    Flat,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PatternSummaryRecord {
    pub summary_id: String,
    pub family: ContextEventFamily,
    pub normalized_key: String,
    pub relation_window: PatternRelationWindow,
    pub metric: PatternMetric,
    pub sample_count: u32,
    pub median_delta: f64,
    pub effect_direction: EffectDirection,
    pub confidence: DataSufficiency,
    pub metadata_json: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReviewSignalDayRecord {
    pub signal_key: String,
    pub day: String,
    pub numeric_value: Option<f64>,
    pub text_value: Option<String>,
    pub baseline_mean: Option<f64>,
    pub baseline_stddev: Option<f64>,
    pub delta: Option<f64>,
    pub z_score: Option<f64>,
    pub persistence_days: u32,
    pub sufficiency: ReviewSufficiency,
    pub stale_days: u32,
    pub metadata_json: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPayloadRecord {
    pub cache_key: String,
    pub endpoint: String,
    pub requested_at: String,
    pub scope: Option<String>,
    pub etag: Option<String>,
    pub payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecordCounts {
    pub raw_payloads: u64,
    pub personal_info: u64,
    pub daily_sleep: u64,
    pub sleep_periods: u64,
    pub daily_readiness: u64,
    pub daily_activity: u64,
    pub daily_spo2: u64,
    pub sleep_time: u64,
    pub daily_stress: u64,
    pub daily_resilience: u64,
    pub daily_cardiovascular_age: u64,
    pub vo2_max: u64,
    pub rest_mode_periods: u64,
    pub heartrate_samples: u64,
    pub workouts: u64,
    pub tags: u64,
    pub enhanced_tags: u64,
    pub sessions: u64,
    pub derived_context_events: u64,
    pub derived_pattern_summaries: u64,
    pub derived_review_signal_days: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyOverviewRow {
    pub day: String,
    pub sleep_score: Option<u8>,
    pub sleep_duration_seconds: Option<i64>,
    pub readiness_score: Option<u8>,
    pub activity_score: Option<u8>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartRatePoint {
    pub recorded_at: String,
    pub bpm: u16,
    pub source_day: Option<String>,
}

pub struct MetadataStore<'connection> {
    connection: &'connection Connection,
}

pub struct SyncStateStore<'connection> {
    connection: &'connection Connection,
}

pub struct AuthStore<'connection> {
    connection: &'connection Connection,
}

pub struct ImportStore<'connection> {
    connection: &'connection Connection,
}

pub struct DerivedStore<'connection> {
    connection: &'connection Connection,
}

pub struct AnalysisStore<'connection> {
    connection: &'connection Connection,
}

pub struct ViewStore<'connection> {
    connection: &'connection Connection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotExportRecord {
    pub snapshot_hash: String,
    pub schema_version: String,
    pub app_version: String,
    pub generated_at: String,
    pub scope: String,
    pub start_day: String,
    pub end_day: String,
    pub anchor_day: String,
    pub day_count: u32,
    pub privacy_profile: String,
    pub source_mode: String,
    pub fixture_dir: Option<String>,
    pub latest_source_day: Option<String>,
    pub latest_review_day: Option<String>,
    pub freshness_summary: String,
    pub trust_summary: String,
    pub capability_summary: String,
    pub provenance_summary: String,
    pub snapshot_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotProvenanceRefRecord {
    pub snapshot_hash: String,
    pub export_ref: String,
    pub local_kind: String,
    pub local_locator: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiArtifactRecord {
    pub artifact_id: String,
    pub artifact_kind: String,
    pub output_schema_version: String,
    pub prompt_version: String,
    pub provider: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub request_mode: String,
    pub input_transport: String,
    pub run_mode: String,
    pub created_at: String,
    pub snapshot_hash_a: String,
    pub snapshot_hash_b: Option<String>,
    pub privacy_profile: String,
    pub artifact_status: String,
    pub overview: String,
    pub summary_cache: String,
    pub request_fingerprint: Option<String>,
    pub payload_json: String,
    pub rendered_briefing: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiArtifactDaySummaryRecord {
    pub artifact_id: String,
    pub artifact_kind: String,
    pub created_at: String,
    pub provider: String,
    pub model: String,
    pub prompt_version: String,
    pub output_schema_version: String,
    pub privacy_profile: String,
    pub summary_cache: String,
    pub overview: String,
    pub matched_snapshot_hash: String,
    pub peer_snapshot_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRunRecord {
    pub run_id: String,
    pub run_kind: String,
    pub run_status: String,
    pub provider: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub request_mode: String,
    pub input_transport: String,
    pub run_mode: String,
    pub prompt_version: String,
    pub output_schema_version: String,
    pub privacy_profile: String,
    pub snapshot_scope: String,
    pub snapshot_hash_a: String,
    pub snapshot_hash_b: Option<String>,
    pub source_ai_artifact_id: Option<String>,
    pub follow_up_kind: Option<String>,
    pub request_fingerprint: Option<String>,
    pub request_preview_json: String,
    pub artifact_id: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub updated_at: String,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRunRegistryEntry {
    pub run_id: String,
    pub run_kind: String,
    pub run_status: String,
    pub provider: String,
    pub model: String,
    pub prompt_version: String,
    pub output_schema_version: String,
    pub run_mode: String,
    pub privacy_profile: String,
    pub snapshot_scope: String,
    pub snapshot_hash_a: String,
    pub snapshot_hash_b: Option<String>,
    pub source_ai_artifact_id: Option<String>,
    pub follow_up_kind: Option<String>,
    pub artifact_id: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotCatalogEntry {
    pub snapshot_hash: String,
    pub schema_version: String,
    pub generated_at: String,
    pub scope: String,
    pub start_day: String,
    pub end_day: String,
    pub anchor_day: String,
    pub day_count: u32,
    pub privacy_profile: String,
    pub source_mode: String,
    pub fixture_dir: Option<String>,
    pub latest_source_day: Option<String>,
    pub latest_review_day: Option<String>,
    pub freshness_summary: String,
    pub trust_summary: String,
    pub capability_summary: String,
    pub provenance_summary: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRunListEntry {
    pub artifact_id: String,
    pub artifact_kind: String,
    pub artifact_status: String,
    pub provider: String,
    pub model: String,
    pub prompt_version: String,
    pub output_schema_version: String,
    pub run_mode: String,
    pub created_at: String,
    pub snapshot_hash_a: String,
    pub snapshot_hash_b: Option<String>,
    pub privacy_profile: String,
    pub overview: String,
    pub summary_cache: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportExportRecord {
    pub report_id: String,
    pub report_kind: String,
    pub title: String,
    pub format: String,
    pub output_path: String,
    pub content_hash: String,
    pub privacy_profile: String,
    pub created_at: String,
    pub source_snapshot_hash_a: Option<String>,
    pub source_snapshot_hash_b: Option<String>,
    pub source_ai_artifact_id: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub prompt_version: Option<String>,
    pub output_schema_version: Option<String>,
    pub export_status: String,
    pub last_verified_exists: bool,
    pub last_verified_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiEvalRunRecord {
    pub eval_run_id: String,
    pub task_family: String,
    pub fixture_dir: String,
    pub candidate_label: String,
    pub baseline_label: Option<String>,
    pub provider: String,
    pub model: String,
    pub prompt_version: String,
    pub output_schema_version: String,
    pub created_at: String,
    pub total_cases: u32,
    pub passed_cases: u32,
    pub failed_cases: u32,
    pub schema_validity_score: f64,
    pub completeness_score: f64,
    pub overclaiming_score: f64,
    pub medical_safety_score: f64,
    pub privacy_score: f64,
    pub evidence_score: f64,
    pub honesty_score: f64,
    pub regression_summary: String,
    pub details_json: String,
}

impl Display for SyncRunStatus {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl SyncRunStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Blocked => "blocked",
            Self::Partial => "partial",
            Self::Success => "success",
            Self::Failed => "failed",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "ready" => Self::Ready,
            "blocked" => Self::Blocked,
            "partial" => Self::Partial,
            "success" => Self::Success,
            _ => Self::Failed,
        }
    }
}

impl ContextEventFamily {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Workout => "workout",
            Self::Tag => "tag",
            Self::EnhancedTag => "enhanced_tag",
            Self::Session => "session",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "workout" => Some(Self::Workout),
            "tag" => Some(Self::Tag),
            "enhanced_tag" => Some(Self::EnhancedTag),
            "session" => Some(Self::Session),
            _ => None,
        }
    }
}

impl TimeSemantics {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Interval => "interval",
            Self::Point => "point",
            Self::AllDay => "all_day",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "interval" => Some(Self::Interval),
            "point" => Some(Self::Point),
            "all_day" => Some(Self::AllDay),
            _ => None,
        }
    }
}

impl PatternMetric {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Activity => "activity_score",
            Self::Readiness => "next_day_readiness",
            Self::Sleep => "same_night_sleep",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Activity => "Activity",
            Self::Readiness => "Next-day readiness",
            Self::Sleep => "Same-night sleep",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "activity_score" => Some(Self::Activity),
            "next_day_readiness" => Some(Self::Readiness),
            "same_night_sleep" => Some(Self::Sleep),
            _ => None,
        }
    }
}

impl PatternRelationWindow {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SameDayActivity => "same_day_activity",
            Self::NextDayReadiness => "next_day_readiness",
            Self::SameNightSleep => "same_night_sleep",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "same_day_activity" => Some(Self::SameDayActivity),
            "next_day_readiness" => Some(Self::NextDayReadiness),
            "same_night_sleep" => Some(Self::SameNightSleep),
            _ => None,
        }
    }
}

impl DataSufficiency {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Thin => "thin",
            Self::Medium => "medium",
            Self::Strong => "strong",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "thin" => Some(Self::Thin),
            "medium" => Some(Self::Medium),
            "strong" => Some(Self::Strong),
            _ => None,
        }
    }
}

impl EffectDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Higher => "higher",
            Self::Lower => "lower",
            Self::Flat => "flat",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "higher" => Some(Self::Higher),
            "lower" => Some(Self::Lower),
            "flat" => Some(Self::Flat),
            _ => None,
        }
    }
}

impl<'connection> MetadataStore<'connection> {
    pub const fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn schema_version(&self) -> Result<u32> {
        let version = self
            .connection
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get::<_, u32>(0),
            )
            .optional()?
            .unwrap_or_else(migrations::current_version);

        Ok(version)
    }

    pub fn upsert(&self, key: &str, value: &str) -> Result<()> {
        self.connection.execute(
            "INSERT INTO app_metadata (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, now_rfc3339()?],
        )?;

        Ok(())
    }
}

impl<'connection> SyncStateStore<'connection> {
    pub const fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn upsert(&self, record: &SyncStateRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO sync_state (
                sync_key,
                family,
                status,
                cursor,
                last_successful_sync_end,
                last_attempted_at,
                last_completed_at,
                last_reconcile_end,
                oldest_recently_reconciled_at,
                message,
                granted_scopes,
                last_error_json,
                last_error_at,
                last_error_kind,
                last_error_detail,
                failure_count,
                next_attempt_after,
                last_trigger_source,
                last_trigger_detail,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
            ON CONFLICT(sync_key) DO UPDATE SET
                family = excluded.family,
                status = excluded.status,
                cursor = excluded.cursor,
                last_successful_sync_end = excluded.last_successful_sync_end,
                last_attempted_at = excluded.last_attempted_at,
                last_completed_at = excluded.last_completed_at,
                last_reconcile_end = excluded.last_reconcile_end,
                oldest_recently_reconciled_at = excluded.oldest_recently_reconciled_at,
                message = excluded.message,
                granted_scopes = excluded.granted_scopes,
                last_error_json = excluded.last_error_json,
                last_error_at = excluded.last_error_at,
                last_error_kind = excluded.last_error_kind,
                last_error_detail = excluded.last_error_detail,
                failure_count = excluded.failure_count,
                next_attempt_after = excluded.next_attempt_after,
                last_trigger_source = excluded.last_trigger_source,
                last_trigger_detail = excluded.last_trigger_detail,
                updated_at = excluded.updated_at",
            params![
                record.sync_key,
                record.family,
                record.status.as_str(),
                record.cursor,
                record.last_successful_sync_end,
                record.last_attempted_at,
                record.last_completed_at,
                record.last_reconcile_end,
                record.oldest_recently_reconciled_at,
                record.message,
                join_scopes(&record.granted_scopes),
                encode_problem(record.last_error.as_ref())?,
                record.last_error_at,
                record.last_error_kind,
                record.last_error_detail,
                i64::from(record.failure_count),
                record.next_attempt_after,
                record.last_trigger_source,
                record.last_trigger_detail,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn list(&self) -> Result<Vec<SyncStateRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                sync_key,
                family,
                status,
                cursor,
                last_successful_sync_end,
                last_attempted_at,
                last_completed_at,
                last_reconcile_end,
                oldest_recently_reconciled_at,
                message,
                granted_scopes,
                last_error_json,
                last_error_at,
                last_error_kind,
                last_error_detail,
                failure_count,
                next_attempt_after,
                last_trigger_source,
                last_trigger_detail,
                updated_at
             FROM sync_state
             ORDER BY sync_key ASC",
        )?;
        let rows = statement.query_map([], read_sync_state_row)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn get(&self, sync_key: &str) -> Result<Option<SyncStateRecord>> {
        self.connection
            .query_row(
                "SELECT
                    sync_key,
                    family,
                    status,
                    cursor,
                    last_successful_sync_end,
                    last_attempted_at,
                    last_completed_at,
                    last_reconcile_end,
                    oldest_recently_reconciled_at,
                    message,
                    granted_scopes,
                    last_error_json,
                    last_error_at,
                    last_error_kind,
                    last_error_detail,
                    failure_count,
                    next_attempt_after,
                    last_trigger_source,
                    last_trigger_detail,
                    updated_at
                 FROM sync_state
                 WHERE sync_key = ?1",
                params![sync_key],
                read_sync_state_row,
            )
            .optional()
            .map_err(Into::into)
    }
}

impl<'connection> AuthStore<'connection> {
    pub const fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn get(&self, provider: &str) -> Result<Option<AuthSessionRecord>> {
        self.connection
            .query_row(
                "SELECT
                    provider,
                    account_id,
                    account_email,
                    token_type,
                    granted_scopes,
                    access_token_expires_at,
                    last_authenticated_at,
                    last_refresh_at,
                    last_error_json,
                    updated_at
                 FROM auth_session
                 WHERE provider = ?1",
                params![provider],
                |row| {
                    Ok(AuthSessionRecord {
                        provider: row.get(0)?,
                        account_id: row.get(1)?,
                        account_email: row.get(2)?,
                        token_type: row.get(3)?,
                        granted_scopes: split_scopes(&row.get::<_, String>(4)?),
                        access_token_expires_at: row.get(5)?,
                        last_authenticated_at: row.get(6)?,
                        last_refresh_at: row.get(7)?,
                        last_error: decode_problem(row.get::<_, Option<String>>(8)?.as_deref())
                            .map_err(json_to_sql_error)?,
                        updated_at: row.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn upsert(&self, record: &AuthSessionRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO auth_session (
                provider,
                account_id,
                account_email,
                token_type,
                granted_scopes,
                access_token_expires_at,
                last_authenticated_at,
                last_refresh_at,
                last_error_json,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(provider) DO UPDATE SET
                account_id = excluded.account_id,
                account_email = excluded.account_email,
                token_type = excluded.token_type,
                granted_scopes = excluded.granted_scopes,
                access_token_expires_at = excluded.access_token_expires_at,
                last_authenticated_at = excluded.last_authenticated_at,
                last_refresh_at = excluded.last_refresh_at,
                last_error_json = excluded.last_error_json,
                updated_at = excluded.updated_at",
            params![
                record.provider,
                record.account_id,
                record.account_email,
                record.token_type,
                join_scopes(&record.granted_scopes),
                record.access_token_expires_at,
                record.last_authenticated_at,
                record.last_refresh_at,
                encode_problem(record.last_error.as_ref())?,
                record.updated_at,
            ],
        )?;

        Ok(())
    }
}

impl<'connection> ImportStore<'connection> {
    pub const fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn upsert_raw_payload(&self, record: &RawPayloadRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO raw_payload_cache (
                cache_key,
                endpoint,
                requested_at,
                scope,
                etag,
                payload
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(cache_key) DO UPDATE SET
                endpoint = excluded.endpoint,
                requested_at = excluded.requested_at,
                scope = excluded.scope,
                etag = excluded.etag,
                payload = excluded.payload",
            params![
                record.cache_key,
                record.endpoint,
                record.requested_at,
                record.scope,
                record.etag,
                record.payload,
            ],
        )?;

        Ok(())
    }

    pub fn upsert_personal_info(&self, record: &PersonalInfoRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO personal_info (
                profile_id,
                age,
                weight,
                height,
                biological_sex,
                email,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(profile_id) DO UPDATE SET
                age = excluded.age,
                weight = excluded.weight,
                height = excluded.height,
                biological_sex = excluded.biological_sex,
                email = excluded.email,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.profile_id,
                record.age.map(i64::from),
                record.weight,
                record.height,
                record.biological_sex,
                record.email,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn upsert_daily_sleep(&self, record: &DailySleepRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO daily_sleep (
                oura_id,
                day,
                sleep_score,
                sleep_duration_seconds,
                raw_cache_key,
                updated_at
            )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(day) DO UPDATE SET
                oura_id = excluded.oura_id,
                sleep_score = excluded.sleep_score,
                sleep_duration_seconds = excluded.sleep_duration_seconds,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.oura_id,
                record.day,
                record.sleep_score.map(i64::from),
                record.sleep_duration_seconds,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn upsert_sleep_period(&self, record: &SleepPeriodRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO sleep_periods (
                oura_id,
                day,
                bedtime_start,
                bedtime_end,
                sleep_type,
                average_heart_rate,
                average_hrv,
                average_breath,
                total_sleep_duration,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(oura_id) DO UPDATE SET
                day = excluded.day,
                bedtime_start = excluded.bedtime_start,
                bedtime_end = excluded.bedtime_end,
                sleep_type = excluded.sleep_type,
                average_heart_rate = excluded.average_heart_rate,
                average_hrv = excluded.average_hrv,
                average_breath = excluded.average_breath,
                total_sleep_duration = excluded.total_sleep_duration,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.oura_id,
                record.day,
                record.bedtime_start,
                record.bedtime_end,
                record.sleep_type,
                record.average_heart_rate,
                record.average_hrv,
                record.average_breath,
                record.total_sleep_duration,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn delete_sleep_periods_between_days(&self, start_day: &str, end_day: &str) -> Result<()> {
        self.connection.execute(
            "DELETE FROM sleep_periods
             WHERE day >= ?1 AND day <= ?2",
            params![start_day, end_day],
        )?;

        Ok(())
    }

    pub fn delete_daily_sleep(&self, day: &str) -> Result<()> {
        if let Some(day_candidate) = extract_day_suffix(day) {
            self.connection.execute(
                "DELETE FROM daily_sleep
                 WHERE day = ?1
                    OR oura_id = ?1
                    OR day = ?2",
                params![day, day_candidate],
            )?;
        } else {
            self.connection.execute(
                "DELETE FROM daily_sleep WHERE day = ?1 OR oura_id = ?1",
                params![day],
            )?;
        }
        Ok(())
    }

    pub fn upsert_daily_spo2(&self, record: &DailySpO2Record) -> Result<()> {
        self.connection.execute(
            "INSERT INTO daily_spo2 (
                day,
                oura_id,
                average_spo2,
                breathing_disturbance_index,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(day) DO UPDATE SET
                oura_id = excluded.oura_id,
                average_spo2 = excluded.average_spo2,
                breathing_disturbance_index = excluded.breathing_disturbance_index,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.day,
                record.oura_id,
                record.average_spo2,
                record.breathing_disturbance_index,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn upsert_daily_readiness(&self, record: &DailyReadinessRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO daily_readiness (
                oura_id,
                day,
                readiness_score,
                temperature_deviation,
                temperature_trend_deviation,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(day) DO UPDATE SET
                oura_id = excluded.oura_id,
                readiness_score = excluded.readiness_score,
                temperature_deviation = excluded.temperature_deviation,
                temperature_trend_deviation = excluded.temperature_trend_deviation,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.oura_id,
                record.day,
                record.readiness_score.map(i64::from),
                record.temperature_deviation,
                record.temperature_trend_deviation,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn delete_daily_readiness(&self, day: &str) -> Result<()> {
        if let Some(day_candidate) = extract_day_suffix(day) {
            self.connection.execute(
                "DELETE FROM daily_readiness
                 WHERE day = ?1
                    OR oura_id = ?1
                    OR day = ?2",
                params![day, day_candidate],
            )?;
        } else {
            self.connection.execute(
                "DELETE FROM daily_readiness WHERE day = ?1 OR oura_id = ?1",
                params![day],
            )?;
        }
        Ok(())
    }

    pub fn upsert_daily_activity(&self, record: &DailyActivityRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO daily_activity (
                oura_id,
                day,
                activity_score,
                active_calories,
                steps,
                total_calories,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(day) DO UPDATE SET
                oura_id = excluded.oura_id,
                activity_score = excluded.activity_score,
                active_calories = excluded.active_calories,
                steps = excluded.steps,
                total_calories = excluded.total_calories,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.oura_id,
                record.day,
                record.activity_score.map(i64::from),
                record.active_calories,
                record.steps,
                record.total_calories,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn delete_daily_activity(&self, day: &str) -> Result<()> {
        if let Some(day_candidate) = extract_day_suffix(day) {
            self.connection.execute(
                "DELETE FROM daily_activity
                 WHERE day = ?1
                    OR oura_id = ?1
                    OR day = ?2",
                params![day, day_candidate],
            )?;
        } else {
            self.connection.execute(
                "DELETE FROM daily_activity WHERE day = ?1 OR oura_id = ?1",
                params![day],
            )?;
        }
        Ok(())
    }

    pub fn upsert_sleep_time(&self, record: &SleepTimeRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO sleep_time (
                oura_id,
                day,
                status,
                recommendation,
                optimal_bedtime_start_offset,
                optimal_bedtime_end_offset,
                optimal_bedtime_day_tz,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(day) DO UPDATE SET
                oura_id = excluded.oura_id,
                status = excluded.status,
                recommendation = excluded.recommendation,
                optimal_bedtime_start_offset = excluded.optimal_bedtime_start_offset,
                optimal_bedtime_end_offset = excluded.optimal_bedtime_end_offset,
                optimal_bedtime_day_tz = excluded.optimal_bedtime_day_tz,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.oura_id,
                record.day,
                record.status,
                record.recommendation,
                record.optimal_bedtime_start_offset,
                record.optimal_bedtime_end_offset,
                record.optimal_bedtime_day_tz,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn upsert_daily_stress(&self, record: &DailyStressRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO daily_stress (
                oura_id,
                day,
                stress_high,
                recovery_high,
                day_summary,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(day) DO UPDATE SET
                oura_id = excluded.oura_id,
                stress_high = excluded.stress_high,
                recovery_high = excluded.recovery_high,
                day_summary = excluded.day_summary,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.oura_id,
                record.day,
                record.stress_high,
                record.recovery_high,
                record.day_summary,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn upsert_daily_resilience(&self, record: &DailyResilienceRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO daily_resilience (
                oura_id,
                day,
                level,
                sleep_recovery,
                daytime_recovery,
                stress,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(day) DO UPDATE SET
                oura_id = excluded.oura_id,
                level = excluded.level,
                sleep_recovery = excluded.sleep_recovery,
                daytime_recovery = excluded.daytime_recovery,
                stress = excluded.stress,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.oura_id,
                record.day,
                record.level,
                record.sleep_recovery,
                record.daytime_recovery,
                record.stress,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn upsert_daily_cardiovascular_age(
        &self,
        record: &DailyCardiovascularAgeRecord,
    ) -> Result<()> {
        self.connection.execute(
            "INSERT INTO daily_cardiovascular_age (
                day,
                vascular_age,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(day) DO UPDATE SET
                vascular_age = excluded.vascular_age,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.day,
                record.vascular_age,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn upsert_vo2_max(&self, record: &Vo2MaxRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO vo2_max (
                oura_id,
                day,
                recorded_at,
                vo2_max,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(day, recorded_at) DO UPDATE SET
                oura_id = excluded.oura_id,
                vo2_max = excluded.vo2_max,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.oura_id,
                record.day,
                record.recorded_at,
                record.vo2_max,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn upsert_rest_mode_period(&self, record: &RestModePeriodRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO rest_mode_periods (
                period_id,
                start_day,
                start_time,
                end_day,
                end_time,
                episode_count,
                tags_json,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(period_id) DO UPDATE SET
                start_day = excluded.start_day,
                start_time = excluded.start_time,
                end_day = excluded.end_day,
                end_time = excluded.end_time,
                episode_count = excluded.episode_count,
                tags_json = excluded.tags_json,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.period_id,
                record.start_day,
                record.start_time,
                record.end_day,
                record.end_time,
                i64::from(record.episode_count),
                record.tags_json,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn upsert_heartrate_sample(&self, record: &HeartrateSampleRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO heartrate_samples (
                recorded_at,
                bpm,
                source_day,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(recorded_at) DO UPDATE SET
                bpm = excluded.bpm,
                source_day = excluded.source_day,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.recorded_at,
                i64::from(record.bpm),
                record.source_day,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn upsert_workout(&self, record: &WorkoutRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO workouts (
                workout_id,
                day,
                started_at,
                ended_at,
                timezone,
                sport,
                activity,
                intensity,
                title,
                notes,
                source,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(workout_id) DO UPDATE SET
                day = excluded.day,
                started_at = excluded.started_at,
                ended_at = excluded.ended_at,
                timezone = excluded.timezone,
                sport = excluded.sport,
                activity = excluded.activity,
                intensity = excluded.intensity,
                title = excluded.title,
                notes = excluded.notes,
                source = excluded.source,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.workout_id,
                record.day,
                record.started_at,
                record.ended_at,
                record.timezone,
                record.sport,
                record.activity,
                record.intensity,
                record.title,
                record.notes,
                record.source,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn delete_workout(&self, workout_id: &str) -> Result<()> {
        self.connection.execute(
            "DELETE FROM workouts WHERE workout_id = ?1",
            params![workout_id],
        )?;
        Ok(())
    }

    pub fn upsert_enhanced_tag(&self, record: &EnhancedTagRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO enhanced_tags (
                enhanced_tag_id,
                day,
                started_at,
                ended_at,
                label,
                subtype,
                comment,
                intensity,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(enhanced_tag_id) DO UPDATE SET
                day = excluded.day,
                started_at = excluded.started_at,
                ended_at = excluded.ended_at,
                label = excluded.label,
                subtype = excluded.subtype,
                comment = excluded.comment,
                intensity = excluded.intensity,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.enhanced_tag_id,
                record.day,
                record.started_at,
                record.ended_at,
                record.label,
                record.subtype,
                record.comment,
                record.intensity,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn delete_enhanced_tag(&self, enhanced_tag_id: &str) -> Result<()> {
        self.connection.execute(
            "DELETE FROM enhanced_tags WHERE enhanced_tag_id = ?1",
            params![enhanced_tag_id],
        )?;
        Ok(())
    }

    pub fn upsert_session(&self, record: &SessionRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO sessions (
                session_id,
                day,
                started_at,
                ended_at,
                kind,
                state,
                score,
                title,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(session_id) DO UPDATE SET
                day = excluded.day,
                started_at = excluded.started_at,
                ended_at = excluded.ended_at,
                kind = excluded.kind,
                state = excluded.state,
                score = excluded.score,
                title = excluded.title,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.session_id,
                record.day,
                record.started_at,
                record.ended_at,
                record.kind,
                record.state,
                record.score,
                record.title,
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        self.connection.execute(
            "DELETE FROM sessions WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(())
    }
}

impl<'connection> DerivedStore<'connection> {
    pub const fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn replace_context_events(&self, records: &[ContextEventRecord]) -> Result<()> {
        self.connection
            .execute_batch("BEGIN IMMEDIATE TRANSACTION")?;

        let result = (|| -> Result<()> {
            self.connection
                .execute("DELETE FROM derived_context_events", [])?;
            let mut statement = self.connection.prepare(
                "INSERT INTO derived_context_events (
                    context_event_id,
                    family,
                    source_id,
                    anchor_day,
                    start_at,
                    end_at,
                    time_semantics,
                    title,
                    subtype,
                    notes,
                    intensity,
                    metadata_json,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            )?;

            for record in records {
                statement.execute(params![
                    record.context_event_id,
                    record.family.as_str(),
                    record.source_id,
                    record.anchor_day,
                    record.start_at,
                    record.end_at,
                    record.time_semantics.as_str(),
                    record.title,
                    record.subtype,
                    record.notes,
                    record.intensity,
                    record.metadata_json,
                    record.updated_at,
                ])?;
            }

            Ok(())
        })();

        match result {
            Ok(()) => {
                self.connection.execute_batch("COMMIT")?;
                Ok(())
            }
            Err(error) => {
                let _ = self.connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    pub fn replace_pattern_summaries(&self, records: &[PatternSummaryRecord]) -> Result<()> {
        self.connection
            .execute_batch("BEGIN IMMEDIATE TRANSACTION")?;

        let result = (|| -> Result<()> {
            self.connection
                .execute("DELETE FROM derived_pattern_summaries", [])?;
            let mut statement = self.connection.prepare(
                "INSERT INTO derived_pattern_summaries (
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
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;

            for record in records {
                statement.execute(params![
                    record.summary_id,
                    record.family.as_str(),
                    record.normalized_key,
                    record.relation_window.as_str(),
                    record.metric.as_str(),
                    i64::from(record.sample_count),
                    record.median_delta,
                    record.effect_direction.as_str(),
                    record.confidence.as_str(),
                    record.metadata_json,
                    record.updated_at,
                ])?;
            }

            Ok(())
        })();

        match result {
            Ok(()) => {
                self.connection.execute_batch("COMMIT")?;
                Ok(())
            }
            Err(error) => {
                let _ = self.connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    pub fn replace_review_signal_days(&self, records: &[ReviewSignalDayRecord]) -> Result<()> {
        self.connection
            .execute_batch("BEGIN IMMEDIATE TRANSACTION")?;

        let result = (|| -> Result<()> {
            self.connection
                .execute("DELETE FROM derived_review_signal_days", [])?;
            let mut statement = self.connection.prepare(
                "INSERT INTO derived_review_signal_days (
                    signal_key,
                    day,
                    numeric_value,
                    text_value,
                    baseline_mean,
                    baseline_stddev,
                    delta,
                    z_score,
                    persistence_days,
                    sufficiency,
                    stale_days,
                    metadata_json,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            )?;

            for record in records {
                statement.execute(params![
                    record.signal_key,
                    record.day,
                    record.numeric_value,
                    record.text_value,
                    record.baseline_mean,
                    record.baseline_stddev,
                    record.delta,
                    record.z_score,
                    i64::from(record.persistence_days),
                    record.sufficiency.as_str(),
                    i64::from(record.stale_days),
                    record.metadata_json,
                    record.updated_at,
                ])?;
            }

            Ok(())
        })();

        match result {
            Ok(()) => {
                self.connection.execute_batch("COMMIT")?;
                Ok(())
            }
            Err(error) => {
                let _ = self.connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }
}

impl<'connection> AnalysisStore<'connection> {
    pub const fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn upsert_snapshot_export(
        &self,
        record: &SnapshotExportRecord,
        provenance_refs: &[SnapshotProvenanceRefRecord],
    ) -> Result<()> {
        let replace_provenance = !provenance_refs.is_empty();
        self.connection
            .execute_batch("BEGIN IMMEDIATE TRANSACTION")?;

        let result = (|| -> Result<()> {
            self.connection.execute(
                "INSERT INTO snapshot_exports (
                    snapshot_hash,
                    schema_version,
                    app_version,
                    generated_at,
                    scope,
                    start_day,
                    end_day,
                    anchor_day,
                    day_count,
                    privacy_profile,
                    source_mode,
                    fixture_dir,
                    latest_source_day,
                    latest_review_day,
                    freshness_summary,
                    trust_summary,
                    capability_summary,
                    provenance_summary,
                    snapshot_json,
                    created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
                ON CONFLICT(snapshot_hash) DO UPDATE SET
                    schema_version = excluded.schema_version,
                    app_version = excluded.app_version,
                    generated_at = excluded.generated_at,
                    scope = excluded.scope,
                    start_day = excluded.start_day,
                    end_day = excluded.end_day,
                    anchor_day = excluded.anchor_day,
                    day_count = excluded.day_count,
                    privacy_profile = excluded.privacy_profile,
                    source_mode = excluded.source_mode,
                    fixture_dir = COALESCE(excluded.fixture_dir, snapshot_exports.fixture_dir),
                    latest_source_day = excluded.latest_source_day,
                    latest_review_day = excluded.latest_review_day,
                    freshness_summary = excluded.freshness_summary,
                    trust_summary = excluded.trust_summary,
                    capability_summary = excluded.capability_summary,
                    provenance_summary = CASE
                        WHEN ?21 = 1 THEN excluded.provenance_summary
                        ELSE snapshot_exports.provenance_summary
                    END,
                    snapshot_json = excluded.snapshot_json,
                    created_at = snapshot_exports.created_at",
                params![
                    record.snapshot_hash,
                    record.schema_version,
                    record.app_version,
                    record.generated_at,
                    record.scope,
                    record.start_day,
                    record.end_day,
                    record.anchor_day,
                    record.day_count,
                    record.privacy_profile,
                    record.source_mode,
                    record.fixture_dir,
                    record.latest_source_day,
                    record.latest_review_day,
                    record.freshness_summary,
                    record.trust_summary,
                    record.capability_summary,
                    record.provenance_summary,
                    record.snapshot_json,
                    record.created_at,
                    replace_provenance,
                ],
            )?;

            if replace_provenance {
                self.connection.execute(
                    "DELETE FROM snapshot_provenance_refs WHERE snapshot_hash = ?1",
                    params![record.snapshot_hash],
                )?;

                let mut statement = self.connection.prepare(
                    "INSERT INTO snapshot_provenance_refs (
                        snapshot_hash,
                        export_ref,
                        local_kind,
                        local_locator,
                        created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5)",
                )?;
                for provenance_ref in provenance_refs {
                    statement.execute(params![
                        provenance_ref.snapshot_hash,
                        provenance_ref.export_ref,
                        provenance_ref.local_kind,
                        provenance_ref.local_locator,
                        provenance_ref.created_at,
                    ])?;
                }
            }

            Ok(())
        })();

        match result {
            Ok(()) => {
                self.connection.execute_batch("COMMIT")?;
                Ok(())
            }
            Err(error) => {
                let _ = self.connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    pub fn snapshot_export(&self, snapshot_hash: &str) -> Result<Option<SnapshotExportRecord>> {
        self.connection
            .query_row(
                "SELECT
                    snapshot_hash,
                    schema_version,
                    app_version,
                    generated_at,
                    scope,
                    start_day,
                    end_day,
                    anchor_day,
                    day_count,
                    privacy_profile,
                    source_mode,
                    fixture_dir,
                    latest_source_day,
                    latest_review_day,
                    freshness_summary,
                    trust_summary,
                    capability_summary,
                    provenance_summary,
                    snapshot_json,
                    created_at
                 FROM snapshot_exports
                 WHERE snapshot_hash = ?1",
                params![snapshot_hash],
                |row| {
                    Ok(SnapshotExportRecord {
                        snapshot_hash: row.get(0)?,
                        schema_version: row.get(1)?,
                        app_version: row.get(2)?,
                        generated_at: row.get(3)?,
                        scope: row.get(4)?,
                        start_day: row.get(5)?,
                        end_day: row.get(6)?,
                        anchor_day: row.get(7)?,
                        day_count: row.get(8)?,
                        privacy_profile: row.get(9)?,
                        source_mode: row.get(10)?,
                        fixture_dir: row.get(11)?,
                        latest_source_day: row.get(12)?,
                        latest_review_day: row.get(13)?,
                        freshness_summary: row.get(14)?,
                        trust_summary: row.get(15)?,
                        capability_summary: row.get(16)?,
                        provenance_summary: row.get(17)?,
                        snapshot_json: row.get(18)?,
                        created_at: row.get(19)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn snapshot_exports_with_prefix(
        &self,
        snapshot_prefix: &str,
    ) -> Result<Vec<SnapshotExportRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                snapshot_hash,
                schema_version,
                app_version,
                generated_at,
                scope,
                start_day,
                end_day,
                anchor_day,
                day_count,
                privacy_profile,
                source_mode,
                fixture_dir,
                latest_source_day,
                latest_review_day,
                freshness_summary,
                trust_summary,
                capability_summary,
                provenance_summary,
                snapshot_json,
                created_at
             FROM snapshot_exports
             WHERE snapshot_hash LIKE ?1
             ORDER BY created_at DESC, snapshot_hash DESC",
        )?;
        let rows = statement.query_map(params![format!("{snapshot_prefix}%")], |row| {
            Ok(SnapshotExportRecord {
                snapshot_hash: row.get(0)?,
                schema_version: row.get(1)?,
                app_version: row.get(2)?,
                generated_at: row.get(3)?,
                scope: row.get(4)?,
                start_day: row.get(5)?,
                end_day: row.get(6)?,
                anchor_day: row.get(7)?,
                day_count: row.get(8)?,
                privacy_profile: row.get(9)?,
                source_mode: row.get(10)?,
                fixture_dir: row.get(11)?,
                latest_source_day: row.get(12)?,
                latest_review_day: row.get(13)?,
                freshness_summary: row.get(14)?,
                trust_summary: row.get(15)?,
                capability_summary: row.get(16)?,
                provenance_summary: row.get(17)?,
                snapshot_json: row.get(18)?,
                created_at: row.get(19)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn list_snapshot_exports(&self) -> Result<Vec<SnapshotCatalogEntry>> {
        let mut statement = self.connection.prepare(
            "SELECT
                snapshot_hash,
                schema_version,
                generated_at,
                scope,
                start_day,
                end_day,
                anchor_day,
                day_count,
                privacy_profile,
                source_mode,
                fixture_dir,
                latest_source_day,
                latest_review_day,
                freshness_summary,
                trust_summary,
                capability_summary,
                provenance_summary,
                created_at
             FROM snapshot_exports
             ORDER BY created_at DESC, snapshot_hash DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(SnapshotCatalogEntry {
                snapshot_hash: row.get(0)?,
                schema_version: row.get(1)?,
                generated_at: row.get(2)?,
                scope: row.get(3)?,
                start_day: row.get(4)?,
                end_day: row.get(5)?,
                anchor_day: row.get(6)?,
                day_count: row.get(7)?,
                privacy_profile: row.get(8)?,
                source_mode: row.get(9)?,
                fixture_dir: row.get(10)?,
                latest_source_day: row.get(11)?,
                latest_review_day: row.get(12)?,
                freshness_summary: row.get(13)?,
                trust_summary: row.get(14)?,
                capability_summary: row.get(15)?,
                provenance_summary: row.get(16)?,
                created_at: row.get(17)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    pub fn snapshot_provenance_refs(
        &self,
        snapshot_hash: &str,
    ) -> Result<Vec<SnapshotProvenanceRefRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                snapshot_hash,
                export_ref,
                local_kind,
                local_locator,
                created_at
             FROM snapshot_provenance_refs
             WHERE snapshot_hash = ?1
             ORDER BY export_ref ASC",
        )?;
        let rows = statement.query_map(params![snapshot_hash], |row| {
            Ok(SnapshotProvenanceRefRecord {
                snapshot_hash: row.get(0)?,
                export_ref: row.get(1)?,
                local_kind: row.get(2)?,
                local_locator: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    pub fn upsert_ai_artifact(&self, record: &AiArtifactRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO ai_artifacts (
                artifact_id,
                artifact_kind,
                output_schema_version,
                prompt_version,
                provider,
                model,
                reasoning_effort,
                request_mode,
                input_transport,
                run_mode,
                created_at,
                snapshot_hash_a,
                snapshot_hash_b,
                privacy_profile,
                artifact_status,
                overview,
                summary_cache,
                request_fingerprint,
                payload_json,
                rendered_briefing
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
            ON CONFLICT(artifact_id) DO UPDATE SET
                artifact_kind = excluded.artifact_kind,
                output_schema_version = excluded.output_schema_version,
                prompt_version = excluded.prompt_version,
                provider = excluded.provider,
                model = excluded.model,
                reasoning_effort = excluded.reasoning_effort,
                request_mode = excluded.request_mode,
                input_transport = excluded.input_transport,
                run_mode = excluded.run_mode,
                created_at = excluded.created_at,
                snapshot_hash_a = excluded.snapshot_hash_a,
                snapshot_hash_b = excluded.snapshot_hash_b,
                privacy_profile = excluded.privacy_profile,
                artifact_status = excluded.artifact_status,
                overview = excluded.overview,
                summary_cache = excluded.summary_cache,
                request_fingerprint = excluded.request_fingerprint,
                payload_json = excluded.payload_json,
                rendered_briefing = excluded.rendered_briefing",
            params![
                record.artifact_id,
                record.artifact_kind,
                record.output_schema_version,
                record.prompt_version,
                record.provider,
                record.model,
                record.reasoning_effort,
                record.request_mode,
                record.input_transport,
                record.run_mode,
                record.created_at,
                record.snapshot_hash_a,
                record.snapshot_hash_b,
                record.privacy_profile,
                record.artifact_status,
                record.overview,
                record.summary_cache,
                record.request_fingerprint,
                record.payload_json,
                record.rendered_briefing,
            ],
        )?;

        Ok(())
    }

    #[cfg(test)]
    pub fn latest_ai_artifact(
        &self,
        artifact_kind: &str,
        snapshot_hash: &str,
    ) -> Result<Option<AiArtifactRecord>> {
        self.connection
            .query_row(
                "SELECT
                    artifact_id,
                    artifact_kind,
                    output_schema_version,
                    prompt_version,
                    provider,
                    model,
                    reasoning_effort,
                    request_mode,
                    input_transport,
                    run_mode,
                    created_at,
                    snapshot_hash_a,
                    snapshot_hash_b,
                    privacy_profile,
                    artifact_status,
                    overview,
                    summary_cache,
                    request_fingerprint,
                    payload_json,
                    rendered_briefing
                 FROM ai_artifacts
                 WHERE artifact_kind = ?1
                   AND snapshot_hash_a = ?2
                 ORDER BY created_at DESC
                 LIMIT 1",
                params![artifact_kind, snapshot_hash],
                |row| {
                    Ok(AiArtifactRecord {
                        artifact_id: row.get(0)?,
                        artifact_kind: row.get(1)?,
                        output_schema_version: row.get(2)?,
                        prompt_version: row.get(3)?,
                        provider: row.get(4)?,
                        model: row.get(5)?,
                        reasoning_effort: row.get(6)?,
                        request_mode: row.get(7)?,
                        input_transport: row.get(8)?,
                        run_mode: row.get(9)?,
                        created_at: row.get(10)?,
                        snapshot_hash_a: row.get(11)?,
                        snapshot_hash_b: row.get(12)?,
                        privacy_profile: row.get(13)?,
                        artifact_status: row.get(14)?,
                        overview: row.get(15)?,
                        summary_cache: row.get(16)?,
                        request_fingerprint: row.get(17)?,
                        payload_json: row.get(18)?,
                        rendered_briefing: row.get(19)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn latest_ai_artifact_for_anchor_day(
        &self,
        anchor_day: &str,
    ) -> Result<Option<AiArtifactDaySummaryRecord>> {
        self.connection
            .query_row(
                "SELECT
                    ai_artifacts.artifact_id,
                    ai_artifacts.artifact_kind,
                    ai_artifacts.created_at,
                    ai_artifacts.provider,
                    ai_artifacts.model,
                    ai_artifacts.prompt_version,
                    ai_artifacts.output_schema_version,
                    ai_artifacts.privacy_profile,
                    ai_artifacts.summary_cache,
                    ai_artifacts.overview,
                    CASE
                        WHEN snapshot_a.anchor_day = ?1 THEN ai_artifacts.snapshot_hash_a
                        ELSE ai_artifacts.snapshot_hash_b
                    END AS matched_snapshot_hash,
                    CASE
                        WHEN snapshot_a.anchor_day = ?1 THEN ai_artifacts.snapshot_hash_b
                        ELSE ai_artifacts.snapshot_hash_a
                    END AS peer_snapshot_hash
                 FROM ai_artifacts
                 LEFT JOIN snapshot_exports AS snapshot_a
                    ON snapshot_a.snapshot_hash = ai_artifacts.snapshot_hash_a
                 LEFT JOIN snapshot_exports AS snapshot_b
                    ON snapshot_b.snapshot_hash = ai_artifacts.snapshot_hash_b
                 WHERE ai_artifacts.artifact_kind IN ('review', 'compare')
                   AND (snapshot_a.anchor_day = ?1 OR snapshot_b.anchor_day = ?1)
                 ORDER BY ai_artifacts.created_at DESC, ai_artifacts.artifact_id DESC
                 LIMIT 1",
                params![anchor_day],
                |row| {
                    Ok(AiArtifactDaySummaryRecord {
                        artifact_id: row.get(0)?,
                        artifact_kind: row.get(1)?,
                        created_at: row.get(2)?,
                        provider: row.get(3)?,
                        model: row.get(4)?,
                        prompt_version: row.get(5)?,
                        output_schema_version: row.get(6)?,
                        privacy_profile: row.get(7)?,
                        summary_cache: row.get(8)?,
                        overview: row.get(9)?,
                        matched_snapshot_hash: row.get(10)?,
                        peer_snapshot_hash: row.get(11)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn ai_artifact(&self, artifact_id: &str) -> Result<Option<AiArtifactRecord>> {
        self.connection
            .query_row(
                "SELECT
                    artifact_id,
                    artifact_kind,
                    output_schema_version,
                    prompt_version,
                    provider,
                    model,
                    reasoning_effort,
                    request_mode,
                    input_transport,
                    run_mode,
                    created_at,
                    snapshot_hash_a,
                    snapshot_hash_b,
                    privacy_profile,
                    artifact_status,
                    overview,
                    summary_cache,
                    request_fingerprint,
                    payload_json,
                    rendered_briefing
                 FROM ai_artifacts
                 WHERE artifact_id = ?1",
                params![artifact_id],
                |row| {
                    Ok(AiArtifactRecord {
                        artifact_id: row.get(0)?,
                        artifact_kind: row.get(1)?,
                        output_schema_version: row.get(2)?,
                        prompt_version: row.get(3)?,
                        provider: row.get(4)?,
                        model: row.get(5)?,
                        reasoning_effort: row.get(6)?,
                        request_mode: row.get(7)?,
                        input_transport: row.get(8)?,
                        run_mode: row.get(9)?,
                        created_at: row.get(10)?,
                        snapshot_hash_a: row.get(11)?,
                        snapshot_hash_b: row.get(12)?,
                        privacy_profile: row.get(13)?,
                        artifact_status: row.get(14)?,
                        overview: row.get(15)?,
                        summary_cache: row.get(16)?,
                        request_fingerprint: row.get(17)?,
                        payload_json: row.get(18)?,
                        rendered_briefing: row.get(19)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn ai_artifacts_with_prefix(&self, artifact_prefix: &str) -> Result<Vec<AiArtifactRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                artifact_id,
                artifact_kind,
                output_schema_version,
                prompt_version,
                provider,
                model,
                reasoning_effort,
                request_mode,
                input_transport,
                run_mode,
                created_at,
                snapshot_hash_a,
                snapshot_hash_b,
                privacy_profile,
                artifact_status,
                overview,
                summary_cache,
                request_fingerprint,
                payload_json,
                rendered_briefing
             FROM ai_artifacts
             WHERE artifact_id LIKE ?1
             ORDER BY created_at DESC, artifact_id DESC",
        )?;
        let rows = statement.query_map(params![format!("{artifact_prefix}%")], |row| {
            Ok(AiArtifactRecord {
                artifact_id: row.get(0)?,
                artifact_kind: row.get(1)?,
                output_schema_version: row.get(2)?,
                prompt_version: row.get(3)?,
                provider: row.get(4)?,
                model: row.get(5)?,
                reasoning_effort: row.get(6)?,
                request_mode: row.get(7)?,
                input_transport: row.get(8)?,
                run_mode: row.get(9)?,
                created_at: row.get(10)?,
                snapshot_hash_a: row.get(11)?,
                snapshot_hash_b: row.get(12)?,
                privacy_profile: row.get(13)?,
                artifact_status: row.get(14)?,
                overview: row.get(15)?,
                summary_cache: row.get(16)?,
                request_fingerprint: row.get(17)?,
                payload_json: row.get(18)?,
                rendered_briefing: row.get(19)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn upsert_ai_run(&self, record: &AiRunRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO ai_runs (
                run_id,
                run_kind,
                run_status,
                provider,
                model,
                reasoning_effort,
                request_mode,
                input_transport,
                run_mode,
                prompt_version,
                output_schema_version,
                privacy_profile,
                snapshot_scope,
                snapshot_hash_a,
                snapshot_hash_b,
                source_ai_artifact_id,
                follow_up_kind,
                request_fingerprint,
                request_preview_json,
                artifact_id,
                error_message,
                created_at,
                started_at,
                ended_at,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)
            ON CONFLICT(run_id) DO UPDATE SET
                run_kind = excluded.run_kind,
                run_status = excluded.run_status,
                provider = excluded.provider,
                model = excluded.model,
                reasoning_effort = excluded.reasoning_effort,
                request_mode = excluded.request_mode,
                input_transport = excluded.input_transport,
                run_mode = excluded.run_mode,
                prompt_version = excluded.prompt_version,
                output_schema_version = excluded.output_schema_version,
                privacy_profile = excluded.privacy_profile,
                snapshot_scope = excluded.snapshot_scope,
                snapshot_hash_a = excluded.snapshot_hash_a,
                snapshot_hash_b = excluded.snapshot_hash_b,
                source_ai_artifact_id = excluded.source_ai_artifact_id,
                follow_up_kind = excluded.follow_up_kind,
                request_fingerprint = excluded.request_fingerprint,
                request_preview_json = excluded.request_preview_json,
                artifact_id = excluded.artifact_id,
                error_message = excluded.error_message,
                created_at = excluded.created_at,
                started_at = excluded.started_at,
                ended_at = excluded.ended_at,
                updated_at = excluded.updated_at",
            params![
                record.run_id,
                record.run_kind,
                record.run_status,
                record.provider,
                record.model,
                record.reasoning_effort,
                record.request_mode,
                record.input_transport,
                record.run_mode,
                record.prompt_version,
                record.output_schema_version,
                record.privacy_profile,
                record.snapshot_scope,
                record.snapshot_hash_a,
                record.snapshot_hash_b,
                record.source_ai_artifact_id,
                record.follow_up_kind,
                record.request_fingerprint,
                record.request_preview_json,
                record.artifact_id,
                record.error_message,
                record.created_at,
                record.started_at,
                record.ended_at,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn update_ai_run_if_active(&self, record: &AiRunRecord) -> Result<bool> {
        let updated = self.connection.execute(
            "UPDATE ai_runs
             SET
                run_kind = ?2,
                run_status = ?3,
                provider = ?4,
                model = ?5,
                reasoning_effort = ?6,
                request_mode = ?7,
                input_transport = ?8,
                run_mode = ?9,
                prompt_version = ?10,
                output_schema_version = ?11,
                privacy_profile = ?12,
                snapshot_scope = ?13,
                snapshot_hash_a = ?14,
                snapshot_hash_b = ?15,
                source_ai_artifact_id = ?16,
                follow_up_kind = ?17,
                request_fingerprint = ?18,
                request_preview_json = ?19,
                artifact_id = ?20,
                error_message = ?21,
                created_at = ?22,
                started_at = ?23,
                ended_at = ?24,
                updated_at = ?25
             WHERE run_id = ?1
               AND run_status IN ('queued', 'running')",
            params![
                record.run_id,
                record.run_kind,
                record.run_status,
                record.provider,
                record.model,
                record.reasoning_effort,
                record.request_mode,
                record.input_transport,
                record.run_mode,
                record.prompt_version,
                record.output_schema_version,
                record.privacy_profile,
                record.snapshot_scope,
                record.snapshot_hash_a,
                record.snapshot_hash_b,
                record.source_ai_artifact_id,
                record.follow_up_kind,
                record.request_fingerprint,
                record.request_preview_json,
                record.artifact_id,
                record.error_message,
                record.created_at,
                record.started_at,
                record.ended_at,
                record.updated_at,
            ],
        )?;

        Ok(updated > 0)
    }

    pub fn ai_run(&self, run_id: &str) -> Result<Option<AiRunRecord>> {
        self.connection
            .query_row(
                "SELECT
                    run_id,
                    run_kind,
                    run_status,
                    provider,
                    model,
                    reasoning_effort,
                    request_mode,
                    input_transport,
                    run_mode,
                    prompt_version,
                    output_schema_version,
                    privacy_profile,
                    snapshot_scope,
                    snapshot_hash_a,
                    snapshot_hash_b,
                    source_ai_artifact_id,
                    follow_up_kind,
                    request_fingerprint,
                    request_preview_json,
                    artifact_id,
                    error_message,
                    created_at,
                    started_at,
                    ended_at,
                    updated_at
                 FROM ai_runs
                 WHERE run_id = ?1",
                params![run_id],
                |row| {
                    Ok(AiRunRecord {
                        run_id: row.get(0)?,
                        run_kind: row.get(1)?,
                        run_status: row.get(2)?,
                        provider: row.get(3)?,
                        model: row.get(4)?,
                        reasoning_effort: row.get(5)?,
                        request_mode: row.get(6)?,
                        input_transport: row.get(7)?,
                        run_mode: row.get(8)?,
                        prompt_version: row.get(9)?,
                        output_schema_version: row.get(10)?,
                        privacy_profile: row.get(11)?,
                        snapshot_scope: row.get(12)?,
                        snapshot_hash_a: row.get(13)?,
                        snapshot_hash_b: row.get(14)?,
                        source_ai_artifact_id: row.get(15)?,
                        follow_up_kind: row.get(16)?,
                        request_fingerprint: row.get(17)?,
                        request_preview_json: row.get(18)?,
                        artifact_id: row.get(19)?,
                        error_message: row.get(20)?,
                        created_at: row.get(21)?,
                        started_at: row.get(22)?,
                        ended_at: row.get(23)?,
                        updated_at: row.get(24)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_ai_run_records(&self) -> Result<Vec<AiRunRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                run_id,
                run_kind,
                run_status,
                provider,
                model,
                reasoning_effort,
                request_mode,
                input_transport,
                run_mode,
                prompt_version,
                output_schema_version,
                privacy_profile,
                snapshot_scope,
                snapshot_hash_a,
                snapshot_hash_b,
                source_ai_artifact_id,
                follow_up_kind,
                request_fingerprint,
                request_preview_json,
                artifact_id,
                error_message,
                created_at,
                started_at,
                ended_at,
                updated_at
             FROM ai_runs
             ORDER BY created_at DESC, run_id DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AiRunRecord {
                run_id: row.get(0)?,
                run_kind: row.get(1)?,
                run_status: row.get(2)?,
                provider: row.get(3)?,
                model: row.get(4)?,
                reasoning_effort: row.get(5)?,
                request_mode: row.get(6)?,
                input_transport: row.get(7)?,
                run_mode: row.get(8)?,
                prompt_version: row.get(9)?,
                output_schema_version: row.get(10)?,
                privacy_profile: row.get(11)?,
                snapshot_scope: row.get(12)?,
                snapshot_hash_a: row.get(13)?,
                snapshot_hash_b: row.get(14)?,
                source_ai_artifact_id: row.get(15)?,
                follow_up_kind: row.get(16)?,
                request_fingerprint: row.get(17)?,
                request_preview_json: row.get(18)?,
                artifact_id: row.get(19)?,
                error_message: row.get(20)?,
                created_at: row.get(21)?,
                started_at: row.get(22)?,
                ended_at: row.get(23)?,
                updated_at: row.get(24)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    #[cfg(test)]
    pub fn list_ai_runs_for_snapshot(
        &self,
        snapshot_hash: &str,
    ) -> Result<Vec<AiRunRegistryEntry>> {
        let mut statement = self.connection.prepare(
            "SELECT
                run_id,
                run_kind,
                run_status,
                provider,
                model,
                prompt_version,
                output_schema_version,
                run_mode,
                privacy_profile,
                snapshot_scope,
                snapshot_hash_a,
                snapshot_hash_b,
                source_ai_artifact_id,
                follow_up_kind,
                artifact_id,
                error_message,
                created_at,
                started_at,
                ended_at
             FROM ai_runs
             WHERE snapshot_hash_a = ?1 OR snapshot_hash_b = ?1
             ORDER BY created_at DESC, run_id DESC",
        )?;
        let rows = statement.query_map(params![snapshot_hash], |row| {
            Ok(AiRunRegistryEntry {
                run_id: row.get(0)?,
                run_kind: row.get(1)?,
                run_status: row.get(2)?,
                provider: row.get(3)?,
                model: row.get(4)?,
                prompt_version: row.get(5)?,
                output_schema_version: row.get(6)?,
                run_mode: row.get(7)?,
                privacy_profile: row.get(8)?,
                snapshot_scope: row.get(9)?,
                snapshot_hash_a: row.get(10)?,
                snapshot_hash_b: row.get(11)?,
                source_ai_artifact_id: row.get(12)?,
                follow_up_kind: row.get(13)?,
                artifact_id: row.get(14)?,
                error_message: row.get(15)?,
                created_at: row.get(16)?,
                started_at: row.get(17)?,
                ended_at: row.get(18)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    pub fn list_ai_artifacts(&self) -> Result<Vec<AiRunListEntry>> {
        let mut statement = self.connection.prepare(
            "SELECT
                artifact_id,
                artifact_kind,
                artifact_status,
                provider,
                model,
                prompt_version,
                output_schema_version,
                run_mode,
                created_at,
                snapshot_hash_a,
                snapshot_hash_b,
                privacy_profile,
                overview,
                summary_cache
             FROM ai_artifacts
             ORDER BY created_at DESC, artifact_id DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AiRunListEntry {
                artifact_id: row.get(0)?,
                artifact_kind: row.get(1)?,
                artifact_status: row.get(2)?,
                provider: row.get(3)?,
                model: row.get(4)?,
                prompt_version: row.get(5)?,
                output_schema_version: row.get(6)?,
                run_mode: row.get(7)?,
                created_at: row.get(8)?,
                snapshot_hash_a: row.get(9)?,
                snapshot_hash_b: row.get(10)?,
                privacy_profile: row.get(11)?,
                overview: row.get(12)?,
                summary_cache: row.get(13)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    pub fn list_ai_artifact_records(&self) -> Result<Vec<AiArtifactRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                artifact_id,
                artifact_kind,
                artifact_status,
                provider,
                model,
                reasoning_effort,
                request_mode,
                input_transport,
                run_mode,
                prompt_version,
                output_schema_version,
                created_at,
                snapshot_hash_a,
                snapshot_hash_b,
                privacy_profile,
                overview,
                summary_cache,
                request_fingerprint,
                payload_json,
                rendered_briefing
             FROM ai_artifacts
             ORDER BY created_at DESC, artifact_id DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AiArtifactRecord {
                artifact_id: row.get(0)?,
                artifact_kind: row.get(1)?,
                artifact_status: row.get(2)?,
                provider: row.get(3)?,
                model: row.get(4)?,
                reasoning_effort: row.get(5)?,
                request_mode: row.get(6)?,
                input_transport: row.get(7)?,
                run_mode: row.get(8)?,
                prompt_version: row.get(9)?,
                output_schema_version: row.get(10)?,
                created_at: row.get(11)?,
                snapshot_hash_a: row.get(12)?,
                snapshot_hash_b: row.get(13)?,
                privacy_profile: row.get(14)?,
                overview: row.get(15)?,
                summary_cache: row.get(16)?,
                request_fingerprint: row.get(17)?,
                payload_json: row.get(18)?,
                rendered_briefing: row.get(19)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    pub fn list_ai_artifacts_for_snapshot(
        &self,
        snapshot_hash: &str,
    ) -> Result<Vec<AiRunListEntry>> {
        let mut statement = self.connection.prepare(
            "SELECT
                artifact_id,
                artifact_kind,
                artifact_status,
                provider,
                model,
                prompt_version,
                output_schema_version,
                run_mode,
                created_at,
                snapshot_hash_a,
                snapshot_hash_b,
                privacy_profile,
                overview,
                summary_cache
             FROM ai_artifacts
             WHERE snapshot_hash_a = ?1 OR snapshot_hash_b = ?1
             ORDER BY created_at DESC, artifact_id DESC",
        )?;
        let rows = statement.query_map(params![snapshot_hash], |row| {
            Ok(AiRunListEntry {
                artifact_id: row.get(0)?,
                artifact_kind: row.get(1)?,
                artifact_status: row.get(2)?,
                provider: row.get(3)?,
                model: row.get(4)?,
                prompt_version: row.get(5)?,
                output_schema_version: row.get(6)?,
                run_mode: row.get(7)?,
                created_at: row.get(8)?,
                snapshot_hash_a: row.get(9)?,
                snapshot_hash_b: row.get(10)?,
                privacy_profile: row.get(11)?,
                overview: row.get(12)?,
                summary_cache: row.get(13)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    pub fn upsert_report_export(&self, record: &ReportExportRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO report_exports (
                report_id,
                report_kind,
                title,
                format,
                output_path,
                content_hash,
                privacy_profile,
                created_at,
                source_snapshot_hash_a,
                source_snapshot_hash_b,
                source_ai_artifact_id,
                provider,
                model,
                prompt_version,
                output_schema_version,
                export_status,
                last_verified_exists,
                last_verified_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
            ON CONFLICT(report_id) DO UPDATE SET
                report_kind = excluded.report_kind,
                title = excluded.title,
                format = excluded.format,
                output_path = excluded.output_path,
                content_hash = excluded.content_hash,
                privacy_profile = excluded.privacy_profile,
                created_at = excluded.created_at,
                source_snapshot_hash_a = excluded.source_snapshot_hash_a,
                source_snapshot_hash_b = excluded.source_snapshot_hash_b,
                source_ai_artifact_id = excluded.source_ai_artifact_id,
                provider = excluded.provider,
                model = excluded.model,
                prompt_version = excluded.prompt_version,
                output_schema_version = excluded.output_schema_version,
                export_status = excluded.export_status,
                last_verified_exists = excluded.last_verified_exists,
                last_verified_at = excluded.last_verified_at",
            params![
                record.report_id,
                record.report_kind,
                record.title,
                record.format,
                record.output_path,
                record.content_hash,
                record.privacy_profile,
                record.created_at,
                record.source_snapshot_hash_a,
                record.source_snapshot_hash_b,
                record.source_ai_artifact_id,
                record.provider,
                record.model,
                record.prompt_version,
                record.output_schema_version,
                record.export_status,
                i64::from(record.last_verified_exists),
                record.last_verified_at,
            ],
        )?;
        Ok(())
    }

    pub fn report_exports_for_snapshot(
        &self,
        snapshot_hash: &str,
    ) -> Result<Vec<ReportExportRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                report_id,
                report_kind,
                title,
                format,
                output_path,
                content_hash,
                privacy_profile,
                created_at,
                source_snapshot_hash_a,
                source_snapshot_hash_b,
                source_ai_artifact_id,
                provider,
                model,
                prompt_version,
                output_schema_version,
                export_status,
                last_verified_exists,
                last_verified_at
             FROM report_exports
             WHERE source_snapshot_hash_a = ?1 OR source_snapshot_hash_b = ?1
             ORDER BY created_at DESC, report_id DESC",
        )?;
        let rows = statement.query_map(params![snapshot_hash], |row| {
            Ok(ReportExportRecord {
                report_id: row.get(0)?,
                report_kind: row.get(1)?,
                title: row.get(2)?,
                format: row.get(3)?,
                output_path: row.get(4)?,
                content_hash: row.get(5)?,
                privacy_profile: row.get(6)?,
                created_at: row.get(7)?,
                source_snapshot_hash_a: row.get(8)?,
                source_snapshot_hash_b: row.get(9)?,
                source_ai_artifact_id: row.get(10)?,
                provider: row.get(11)?,
                model: row.get(12)?,
                prompt_version: row.get(13)?,
                output_schema_version: row.get(14)?,
                export_status: row.get(15)?,
                last_verified_exists: row.get::<_, i64>(16)? != 0,
                last_verified_at: row.get(17)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    pub fn list_report_exports(&self) -> Result<Vec<ReportExportRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                report_id,
                report_kind,
                title,
                format,
                output_path,
                content_hash,
                privacy_profile,
                created_at,
                source_snapshot_hash_a,
                source_snapshot_hash_b,
                source_ai_artifact_id,
                provider,
                model,
                prompt_version,
                output_schema_version,
                export_status,
                last_verified_exists,
                last_verified_at
             FROM report_exports
             ORDER BY created_at DESC, report_id DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ReportExportRecord {
                report_id: row.get(0)?,
                report_kind: row.get(1)?,
                title: row.get(2)?,
                format: row.get(3)?,
                output_path: row.get(4)?,
                content_hash: row.get(5)?,
                privacy_profile: row.get(6)?,
                created_at: row.get(7)?,
                source_snapshot_hash_a: row.get(8)?,
                source_snapshot_hash_b: row.get(9)?,
                source_ai_artifact_id: row.get(10)?,
                provider: row.get(11)?,
                model: row.get(12)?,
                prompt_version: row.get(13)?,
                output_schema_version: row.get(14)?,
                export_status: row.get(15)?,
                last_verified_exists: row.get::<_, i64>(16)? != 0,
                last_verified_at: row.get(17)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    pub fn report_exports_for_ai_artifact(
        &self,
        artifact_id: &str,
    ) -> Result<Vec<ReportExportRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                report_id,
                report_kind,
                title,
                format,
                output_path,
                content_hash,
                privacy_profile,
                created_at,
                source_snapshot_hash_a,
                source_snapshot_hash_b,
                source_ai_artifact_id,
                provider,
                model,
                prompt_version,
                output_schema_version,
                export_status,
                last_verified_exists,
                last_verified_at
             FROM report_exports
             WHERE source_ai_artifact_id = ?1
             ORDER BY created_at DESC, report_id DESC",
        )?;
        let rows = statement.query_map(params![artifact_id], |row| {
            Ok(ReportExportRecord {
                report_id: row.get(0)?,
                report_kind: row.get(1)?,
                title: row.get(2)?,
                format: row.get(3)?,
                output_path: row.get(4)?,
                content_hash: row.get(5)?,
                privacy_profile: row.get(6)?,
                created_at: row.get(7)?,
                source_snapshot_hash_a: row.get(8)?,
                source_snapshot_hash_b: row.get(9)?,
                source_ai_artifact_id: row.get(10)?,
                provider: row.get(11)?,
                model: row.get(12)?,
                prompt_version: row.get(13)?,
                output_schema_version: row.get(14)?,
                export_status: row.get(15)?,
                last_verified_exists: row.get::<_, i64>(16)? != 0,
                last_verified_at: row.get(17)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    pub fn upsert_ai_eval_run(&self, record: &AiEvalRunRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO ai_eval_runs (
                eval_run_id,
                task_family,
                fixture_dir,
                candidate_label,
                baseline_label,
                provider,
                model,
                prompt_version,
                output_schema_version,
                created_at,
                total_cases,
                passed_cases,
                failed_cases,
                schema_validity_score,
                completeness_score,
                overclaiming_score,
                medical_safety_score,
                privacy_score,
                evidence_score,
                honesty_score,
                regression_summary,
                details_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)
            ON CONFLICT(eval_run_id) DO UPDATE SET
                task_family = excluded.task_family,
                fixture_dir = excluded.fixture_dir,
                candidate_label = excluded.candidate_label,
                baseline_label = excluded.baseline_label,
                provider = excluded.provider,
                model = excluded.model,
                prompt_version = excluded.prompt_version,
                output_schema_version = excluded.output_schema_version,
                created_at = excluded.created_at,
                total_cases = excluded.total_cases,
                passed_cases = excluded.passed_cases,
                failed_cases = excluded.failed_cases,
                schema_validity_score = excluded.schema_validity_score,
                completeness_score = excluded.completeness_score,
                overclaiming_score = excluded.overclaiming_score,
                medical_safety_score = excluded.medical_safety_score,
                privacy_score = excluded.privacy_score,
                evidence_score = excluded.evidence_score,
                honesty_score = excluded.honesty_score,
                regression_summary = excluded.regression_summary,
                details_json = excluded.details_json",
            params![
                record.eval_run_id,
                record.task_family,
                record.fixture_dir,
                record.candidate_label,
                record.baseline_label,
                record.provider,
                record.model,
                record.prompt_version,
                record.output_schema_version,
                record.created_at,
                record.total_cases,
                record.passed_cases,
                record.failed_cases,
                record.schema_validity_score,
                record.completeness_score,
                record.overclaiming_score,
                record.medical_safety_score,
                record.privacy_score,
                record.evidence_score,
                record.honesty_score,
                record.regression_summary,
                record.details_json,
            ],
        )?;
        Ok(())
    }

    pub fn list_ai_eval_runs(&self) -> Result<Vec<AiEvalRunRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                eval_run_id,
                task_family,
                fixture_dir,
                candidate_label,
                baseline_label,
                provider,
                model,
                prompt_version,
                output_schema_version,
                created_at,
                total_cases,
                passed_cases,
                failed_cases,
                schema_validity_score,
                completeness_score,
                overclaiming_score,
                medical_safety_score,
                privacy_score,
                evidence_score,
                honesty_score,
                regression_summary,
                details_json
             FROM ai_eval_runs
             ORDER BY created_at DESC, eval_run_id DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AiEvalRunRecord {
                eval_run_id: row.get(0)?,
                task_family: row.get(1)?,
                fixture_dir: row.get(2)?,
                candidate_label: row.get(3)?,
                baseline_label: row.get(4)?,
                provider: row.get(5)?,
                model: row.get(6)?,
                prompt_version: row.get(7)?,
                output_schema_version: row.get(8)?,
                created_at: row.get(9)?,
                total_cases: row.get(10)?,
                passed_cases: row.get(11)?,
                failed_cases: row.get(12)?,
                schema_validity_score: row.get(13)?,
                completeness_score: row.get(14)?,
                overclaiming_score: row.get(15)?,
                medical_safety_score: row.get(16)?,
                privacy_score: row.get(17)?,
                evidence_score: row.get(18)?,
                honesty_score: row.get(19)?,
                regression_summary: row.get(20)?,
                details_json: row.get(21)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }
}

impl<'connection> ViewStore<'connection> {
    pub const fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn daily_history(&self, limit: usize) -> Result<Vec<DailyOverviewRow>> {
        let bounded_limit = usize::min(limit, 366);
        let mut statement = self.connection.prepare(
            r"
            WITH days AS (
                SELECT day FROM daily_sleep
                UNION
                SELECT day FROM daily_readiness
                UNION
                SELECT day FROM daily_activity
            )
            SELECT
                days.day,
                daily_sleep.sleep_score,
                daily_sleep.sleep_duration_seconds,
                daily_readiness.readiness_score,
                daily_activity.activity_score,
                MAX(
                    COALESCE(daily_sleep.updated_at, ''),
                    COALESCE(daily_readiness.updated_at, ''),
                    COALESCE(daily_activity.updated_at, '')
                )
            FROM days
            LEFT JOIN daily_sleep ON daily_sleep.day = days.day
            LEFT JOIN daily_readiness ON daily_readiness.day = days.day
            LEFT JOIN daily_activity ON daily_activity.day = days.day
            ORDER BY days.day DESC
            LIMIT ?1
            ",
        )?;
        let rows = statement.query_map(
            params![crate::numeric::usize_to_i64(bounded_limit)],
            |row| {
                Ok(DailyOverviewRow {
                    day: row.get(0)?,
                    sleep_score: parse_optional_score(row.get::<_, Option<i64>>(1)?),
                    sleep_duration_seconds: row.get(2)?,
                    readiness_score: parse_optional_score(row.get::<_, Option<i64>>(3)?),
                    activity_score: parse_optional_score(row.get::<_, Option<i64>>(4)?),
                    updated_at: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                })
            },
        )?;

        let mut history = Vec::new();
        for row in rows {
            history.push(row?);
        }
        history.reverse();

        Ok(history)
    }

    pub fn daily_history_all(&self) -> Result<Vec<DailyOverviewRow>> {
        let mut statement = self.connection.prepare(
            "WITH days AS (
                SELECT day FROM daily_sleep
                UNION
                SELECT day FROM daily_readiness
                UNION
                SELECT day FROM daily_activity
            )
            SELECT
                days.day,
                daily_sleep.sleep_score,
                daily_sleep.sleep_duration_seconds,
                daily_readiness.readiness_score,
                daily_activity.activity_score,
                MAX(
                    COALESCE(daily_sleep.updated_at, ''),
                    COALESCE(daily_readiness.updated_at, ''),
                    COALESCE(daily_activity.updated_at, '')
                )
            FROM days
            LEFT JOIN daily_sleep ON daily_sleep.day = days.day
            LEFT JOIN daily_readiness ON daily_readiness.day = days.day
            LEFT JOIN daily_activity ON daily_activity.day = days.day
            ORDER BY days.day ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(DailyOverviewRow {
                day: row.get(0)?,
                sleep_score: parse_optional_score(row.get::<_, Option<i64>>(1)?),
                sleep_duration_seconds: row.get(2)?,
                readiness_score: parse_optional_score(row.get::<_, Option<i64>>(3)?),
                activity_score: parse_optional_score(row.get::<_, Option<i64>>(4)?),
                updated_at: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
            })
        })?;

        let mut history = Vec::new();
        for row in rows {
            history.push(row?);
        }

        Ok(history)
    }

    pub fn daily_history_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<DailyOverviewRow>> {
        let mut statement = self.connection.prepare(
            "WITH days AS (
                SELECT day FROM daily_sleep
                UNION
                SELECT day FROM daily_readiness
                UNION
                SELECT day FROM daily_activity
            )
            SELECT
                days.day,
                daily_sleep.sleep_score,
                daily_sleep.sleep_duration_seconds,
                daily_readiness.readiness_score,
                daily_activity.activity_score,
                MAX(
                    COALESCE(daily_sleep.updated_at, ''),
                    COALESCE(daily_readiness.updated_at, ''),
                    COALESCE(daily_activity.updated_at, '')
                )
            FROM days
            LEFT JOIN daily_sleep ON daily_sleep.day = days.day
            LEFT JOIN daily_readiness ON daily_readiness.day = days.day
            LEFT JOIN daily_activity ON daily_activity.day = days.day
            WHERE days.day >= ?1 AND days.day <= ?2
            ORDER BY days.day ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], |row| {
            Ok(DailyOverviewRow {
                day: row.get(0)?,
                sleep_score: parse_optional_score(row.get::<_, Option<i64>>(1)?),
                sleep_duration_seconds: row.get(2)?,
                readiness_score: parse_optional_score(row.get::<_, Option<i64>>(3)?),
                activity_score: parse_optional_score(row.get::<_, Option<i64>>(4)?),
                updated_at: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
            })
        })?;

        let mut history = Vec::new();
        for row in rows {
            history.push(row?);
        }

        Ok(history)
    }

    pub fn latest_source_day(&self) -> Result<Option<String>> {
        let local_day = current_local_day_string();
        self.connection
            .query_row(
                "SELECT MAX(day) FROM (
                    SELECT day FROM daily_sleep
                    UNION ALL
                    SELECT day FROM daily_readiness
                    UNION ALL
                    SELECT day FROM daily_activity
                    UNION ALL
                    SELECT day FROM sleep_time
                    UNION ALL
                    SELECT day FROM daily_stress
                    UNION ALL
                    SELECT day FROM daily_resilience
                    UNION ALL
                    SELECT day FROM daily_cardiovascular_age
                    UNION ALL
                    SELECT day FROM vo2_max
                    UNION ALL
                    SELECT day FROM workouts
                    UNION ALL
                    SELECT day FROM tags
                    UNION ALL
                    SELECT day FROM enhanced_tags
                    UNION ALL
                    SELECT day FROM sessions
                    UNION ALL
                    SELECT CASE
                        WHEN end_day IS NULL THEN MAX(start_day, ?1)
                        ELSE end_day
                    END AS day
                    FROM rest_mode_periods
                )",
                [local_day],
                |row| row.get::<_, Option<String>>(0),
            )
            .map_err(Into::into)
    }

    pub fn latest_review_day(&self) -> Result<Option<String>> {
        let local_day = current_local_day_string();
        self.connection
            .query_row(
                "SELECT MAX(day) FROM (
                    SELECT day FROM derived_review_signal_days
                    UNION ALL
                    SELECT day FROM sleep_time
                    UNION ALL
                    SELECT CASE
                        WHEN end_day IS NULL THEN MAX(start_day, ?1)
                        ELSE end_day
                    END AS day
                    FROM rest_mode_periods
                )",
                [local_day],
                |row| row.get::<_, Option<String>>(0),
            )
            .map_err(Into::into)
    }

    pub fn latest_personal_info(&self) -> Result<Option<PersonalInfoRecord>> {
        self.connection
            .query_row(
                "SELECT
                    profile_id,
                    age,
                    weight,
                    height,
                    biological_sex,
                    email,
                    raw_cache_key,
                    updated_at
                 FROM personal_info
                 ORDER BY updated_at DESC
                 LIMIT 1",
                [],
                |row| {
                    Ok(PersonalInfoRecord {
                        profile_id: row.get(0)?,
                        age: parse_optional_u16(row.get::<_, Option<i64>>(1)?)?,
                        weight: row.get(2)?,
                        height: row.get(3)?,
                        biological_sex: row.get(4)?,
                        email: row.get(5)?,
                        raw_cache_key: row.get(6)?,
                        updated_at: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn heartrate_for_day(&self, day: &str) -> Result<Vec<HeartRatePoint>> {
        let mut statement = self.connection.prepare(
            "SELECT recorded_at, bpm, source_day
             FROM heartrate_samples
             WHERE source_day = ?1
             ORDER BY recorded_at ASC",
        )?;
        let rows = statement.query_map(params![day], read_heartrate_row)?;

        let mut points = Vec::new();
        for row in rows {
            points.push(row?);
        }

        Ok(points)
    }

    pub fn available_heartrate_days(&self, limit: usize) -> Result<Vec<String>> {
        let bounded_limit = usize::min(limit, 366);
        let mut statement = self.connection.prepare(
            "SELECT DISTINCT source_day
             FROM heartrate_samples
             WHERE source_day IS NOT NULL
             ORDER BY source_day DESC
             LIMIT ?1",
        )?;
        let rows = statement.query_map(
            params![crate::numeric::usize_to_i64(bounded_limit)],
            |row| row.get::<_, Option<String>>(0),
        )?;

        let mut days = Vec::new();
        for row in rows {
            if let Some(day) = row? {
                days.push(day);
            }
        }
        days.reverse();

        Ok(days)
    }

    pub fn workouts_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<WorkoutRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                workout_id,
                day,
                started_at,
                ended_at,
                timezone,
                sport,
                activity,
                intensity,
                title,
                notes,
                source,
                raw_cache_key,
                updated_at
             FROM workouts
             WHERE day >= ?1 AND day <= ?2
             ORDER BY day ASC, started_at ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], read_workout_row)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn daily_activity_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<DailyActivityRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                oura_id,
                day,
                activity_score,
                active_calories,
                steps,
                total_calories,
                raw_cache_key,
                updated_at
             FROM daily_activity
             WHERE day >= ?1 AND day <= ?2
             ORDER BY day ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], |row| {
            Ok(DailyActivityRecord {
                oura_id: row.get(0)?,
                day: row.get(1)?,
                activity_score: parse_optional_score(row.get(2)?),
                active_calories: row.get(3)?,
                steps: row.get(4)?,
                total_calories: row.get(5)?,
                raw_cache_key: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn sleep_periods_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<SleepPeriodRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                oura_id,
                day,
                bedtime_start,
                bedtime_end,
                sleep_type,
                average_heart_rate,
                average_hrv,
                average_breath,
                total_sleep_duration,
                raw_cache_key,
                updated_at
             FROM sleep_periods
             WHERE day >= ?1 AND day <= ?2
             ORDER BY day ASC, COALESCE(bedtime_start, day) ASC, oura_id ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], |row| {
            Ok(SleepPeriodRecord {
                oura_id: row.get(0)?,
                day: row.get(1)?,
                bedtime_start: row.get(2)?,
                bedtime_end: row.get(3)?,
                sleep_type: row.get(4)?,
                average_heart_rate: row.get(5)?,
                average_hrv: row.get(6)?,
                average_breath: row.get(7)?,
                total_sleep_duration: row.get(8)?,
                raw_cache_key: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn daily_spo2_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<DailySpO2Record>> {
        let mut statement = self.connection.prepare(
            "SELECT
                day,
                oura_id,
                average_spo2,
                breathing_disturbance_index,
                raw_cache_key,
                updated_at
             FROM daily_spo2
             WHERE day >= ?1 AND day <= ?2
             ORDER BY day ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], |row| {
            Ok(DailySpO2Record {
                day: row.get(0)?,
                oura_id: row.get(1)?,
                average_spo2: row.get(2)?,
                breathing_disturbance_index: row.get(3)?,
                raw_cache_key: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn daily_readiness_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<DailyReadinessRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                oura_id,
                day,
                readiness_score,
                temperature_deviation,
                temperature_trend_deviation,
                raw_cache_key,
                updated_at
             FROM daily_readiness
             WHERE day >= ?1 AND day <= ?2
             ORDER BY day ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], |row| {
            Ok(DailyReadinessRecord {
                oura_id: row.get(0)?,
                day: row.get(1)?,
                readiness_score: parse_optional_score(row.get(2)?),
                temperature_deviation: row.get(3)?,
                temperature_trend_deviation: row.get(4)?,
                raw_cache_key: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn sleep_time_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<SleepTimeRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                oura_id,
                day,
                status,
                recommendation,
                optimal_bedtime_start_offset,
                optimal_bedtime_end_offset,
                optimal_bedtime_day_tz,
                raw_cache_key,
                updated_at
             FROM sleep_time
             WHERE day >= ?1 AND day <= ?2
             ORDER BY day ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], |row| {
            Ok(SleepTimeRecord {
                oura_id: row.get(0)?,
                day: row.get(1)?,
                status: row.get(2)?,
                recommendation: row.get(3)?,
                optimal_bedtime_start_offset: row.get(4)?,
                optimal_bedtime_end_offset: row.get(5)?,
                optimal_bedtime_day_tz: row.get(6)?,
                raw_cache_key: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn daily_stress_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<DailyStressRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                oura_id,
                day,
                stress_high,
                recovery_high,
                day_summary,
                raw_cache_key,
                updated_at
             FROM daily_stress
             WHERE day >= ?1 AND day <= ?2
             ORDER BY day ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], |row| {
            Ok(DailyStressRecord {
                oura_id: row.get(0)?,
                day: row.get(1)?,
                stress_high: row.get(2)?,
                recovery_high: row.get(3)?,
                day_summary: row.get(4)?,
                raw_cache_key: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn daily_resilience_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<DailyResilienceRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                oura_id,
                day,
                level,
                sleep_recovery,
                daytime_recovery,
                stress,
                raw_cache_key,
                updated_at
             FROM daily_resilience
             WHERE day >= ?1 AND day <= ?2
             ORDER BY day ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], |row| {
            Ok(DailyResilienceRecord {
                oura_id: row.get(0)?,
                day: row.get(1)?,
                level: row.get(2)?,
                sleep_recovery: row.get(3)?,
                daytime_recovery: row.get(4)?,
                stress: row.get(5)?,
                raw_cache_key: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn daily_cardiovascular_age_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<DailyCardiovascularAgeRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                day,
                vascular_age,
                raw_cache_key,
                updated_at
             FROM daily_cardiovascular_age
             WHERE day >= ?1 AND day <= ?2
             ORDER BY day ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], |row| {
            Ok(DailyCardiovascularAgeRecord {
                day: row.get(0)?,
                vascular_age: row.get(1)?,
                raw_cache_key: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn vo2_max_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<Vo2MaxRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                oura_id,
                day,
                recorded_at,
                vo2_max,
                raw_cache_key,
                updated_at
             FROM vo2_max
             WHERE day >= ?1 AND day <= ?2
             ORDER BY day ASC, recorded_at ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], |row| {
            Ok(Vo2MaxRecord {
                oura_id: row.get(0)?,
                day: row.get(1)?,
                recorded_at: row.get(2)?,
                vo2_max: row.get(3)?,
                raw_cache_key: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn rest_mode_periods_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<RestModePeriodRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                period_id,
                start_day,
                start_time,
                end_day,
                end_time,
                episode_count,
                tags_json,
                raw_cache_key,
                updated_at
             FROM rest_mode_periods
             WHERE start_day <= ?2
               AND (end_day IS NULL OR end_day >= ?1)
             ORDER BY start_day ASC, COALESCE(start_time, start_day) ASC, period_id ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], |row| {
            Ok(RestModePeriodRecord {
                period_id: row.get(0)?,
                start_day: row.get(1)?,
                start_time: row.get(2)?,
                end_day: row.get(3)?,
                end_time: row.get(4)?,
                episode_count: parse_u32(row.get::<_, i64>(5)?, 5)?,
                tags_json: row.get(6)?,
                raw_cache_key: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn tags_between_days(&self, start_day: &str, end_day: &str) -> Result<Vec<TagRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                tag_id,
                day,
                label,
                raw_cache_key,
                updated_at
             FROM tags
             WHERE day >= ?1 AND day <= ?2
             ORDER BY day ASC, tag_id ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], |row| {
            Ok(TagRecord {
                tag_id: row.get(0)?,
                day: row.get(1)?,
                label: row.get(2)?,
                source: TagSource::Basic,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn enhanced_tags_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<EnhancedTagRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                enhanced_tag_id,
                day,
                started_at,
                ended_at,
                label,
                subtype,
                comment,
                intensity,
                raw_cache_key,
                updated_at
             FROM enhanced_tags
             WHERE day >= ?1 AND day <= ?2
             ORDER BY day ASC, COALESCE(started_at, day) ASC, enhanced_tag_id ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], read_enhanced_tag_row)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn sessions_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<SessionRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                session_id,
                day,
                started_at,
                ended_at,
                kind,
                state,
                score,
                title,
                raw_cache_key,
                updated_at
             FROM sessions
             WHERE day >= ?1 AND day <= ?2
             ORDER BY day ASC, started_at ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], read_session_row)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    #[cfg(test)]
    pub fn context_events_for_day(&self, day: &str) -> Result<Vec<ContextEventRecord>> {
        let day_start = format!("{day}T00:00:00Z");
        let day_end = format!("{day}T23:59:59Z");
        let mut statement = self.connection.prepare(
            "SELECT
                context_event_id,
                family,
                source_id,
                anchor_day,
                start_at,
                end_at,
                time_semantics,
                title,
                subtype,
                notes,
                intensity,
                metadata_json,
                updated_at
             FROM derived_context_events
             WHERE anchor_day = ?1
                OR (
                    strftime('%s', start_at) <= strftime('%s', ?3)
                    AND strftime('%s', COALESCE(end_at, start_at)) >= strftime('%s', ?2)
                )
             ORDER BY strftime('%s', start_at) ASC, context_event_id ASC",
        )?;
        let rows = statement.query_map(params![day, day_start, day_end], read_context_event_row)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn context_events_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<ContextEventRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                context_event_id,
                family,
                source_id,
                anchor_day,
                start_at,
                end_at,
                time_semantics,
                title,
                subtype,
                notes,
                intensity,
                metadata_json,
                updated_at
             FROM derived_context_events
             WHERE anchor_day >= ?1 AND anchor_day <= ?2
             ORDER BY anchor_day ASC, start_at ASC, context_event_id ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], read_context_event_row)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn pattern_summaries(
        &self,
        family: Option<ContextEventFamily>,
        metric: Option<PatternMetric>,
    ) -> Result<Vec<PatternSummaryRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
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
                updated_at
             FROM derived_pattern_summaries
             WHERE (?1 IS NULL OR family = ?1)
               AND (?2 IS NULL OR metric = ?2)
             ORDER BY
                CASE confidence WHEN 'strong' THEN 3 WHEN 'medium' THEN 2 ELSE 1 END DESC,
                sample_count DESC,
                ABS(median_delta) DESC,
                normalized_key ASC",
        )?;
        let family_filter = family.map(ContextEventFamily::as_str);
        let metric_filter = metric.map(PatternMetric::as_str);
        let rows = statement.query_map(
            params![family_filter, metric_filter],
            read_pattern_summary_row,
        )?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn review_signal_days_between_days(
        &self,
        start_day: &str,
        end_day: &str,
    ) -> Result<Vec<ReviewSignalDayRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                signal_key,
                day,
                numeric_value,
                text_value,
                baseline_mean,
                baseline_stddev,
                delta,
                z_score,
                persistence_days,
                sufficiency,
                stale_days,
                metadata_json,
                updated_at
             FROM derived_review_signal_days
             WHERE day >= ?1 AND day <= ?2
             ORDER BY day ASC, signal_key ASC",
        )?;
        let rows = statement.query_map(params![start_day, end_day], read_review_signal_day_row)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn record_counts(&self) -> Result<RecordCounts> {
        Ok(RecordCounts {
            raw_payloads: row_count(self.connection, "raw_payload_cache")?,
            personal_info: row_count(self.connection, "personal_info")?,
            daily_sleep: row_count(self.connection, "daily_sleep")?,
            sleep_periods: row_count(self.connection, "sleep_periods")?,
            daily_readiness: row_count(self.connection, "daily_readiness")?,
            daily_activity: row_count(self.connection, "daily_activity")?,
            daily_spo2: row_count(self.connection, "daily_spo2")?,
            sleep_time: row_count(self.connection, "sleep_time")?,
            daily_stress: row_count(self.connection, "daily_stress")?,
            daily_resilience: row_count(self.connection, "daily_resilience")?,
            daily_cardiovascular_age: row_count(self.connection, "daily_cardiovascular_age")?,
            vo2_max: row_count(self.connection, "vo2_max")?,
            rest_mode_periods: row_count(self.connection, "rest_mode_periods")?,
            heartrate_samples: row_count(self.connection, "heartrate_samples")?,
            workouts: row_count(self.connection, "workouts")?,
            tags: row_count(self.connection, "tags")?,
            enhanced_tags: row_count(self.connection, "enhanced_tags")?,
            sessions: row_count(self.connection, "sessions")?,
            derived_context_events: row_count(self.connection, "derived_context_events")?,
            derived_pattern_summaries: row_count(self.connection, "derived_pattern_summaries")?,
            derived_review_signal_days: row_count(self.connection, "derived_review_signal_days")?,
        })
    }
}

fn read_sync_state_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SyncStateRecord> {
    let sync_key: String = row.get(0)?;
    let last_attempted_at = row.get::<_, String>(5)?;
    let updated_at = row
        .get::<_, Option<String>>(19)?
        .unwrap_or_else(|| last_attempted_at.clone());
    let family = row
        .get::<_, Option<String>>(1)?
        .unwrap_or_else(|| match sync_key.as_str() {
            "oura.personal" => "personal".to_owned(),
            "oura.daily" => "daily".to_owned(),
            "oura.spo2" => "spo2".to_owned(),
            "oura.heartrate" => "heartrate".to_owned(),
            "oura.workouts" => "workout".to_owned(),
            "oura.enhanced_tags" => "tag".to_owned(),
            "oura.sessions" => "session".to_owned(),
            other => other.to_owned(),
        });

    Ok(SyncStateRecord {
        sync_key,
        family,
        status: SyncRunStatus::parse(&row.get::<_, String>(2)?),
        cursor: row.get(3)?,
        last_successful_sync_end: row.get(4)?,
        last_attempted_at,
        last_completed_at: row.get(6)?,
        last_reconcile_end: row.get(7)?,
        oldest_recently_reconciled_at: row.get(8)?,
        message: row.get(9)?,
        granted_scopes: split_scopes(&row.get::<_, String>(10)?),
        last_error: decode_problem(row.get::<_, Option<String>>(11)?.as_deref())
            .map_err(json_to_sql_error)?,
        last_error_at: row.get(12)?,
        last_error_kind: row.get(13)?,
        last_error_detail: row.get(14)?,
        failure_count: parse_u32(row.get::<_, i64>(15)?, 15)?,
        next_attempt_after: row.get(16)?,
        last_trigger_source: row.get(17)?,
        last_trigger_detail: row.get(18)?,
        updated_at,
    })
}

fn parse_optional_score(value: Option<i64>) -> Option<u8> {
    value.and_then(|score| u8::try_from(score).ok())
}

fn parse_optional_u16(value: Option<i64>) -> rusqlite::Result<Option<u16>> {
    value.map_or_else(
        || Ok(None),
        |value| {
            u16::try_from(value).map(Some).map_err(|_| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Integer,
                    Box::new(std::fmt::Error),
                )
            })
        },
    )
}

fn parse_u32(value: i64, column: usize) -> rusqlite::Result<u32> {
    u32::try_from(value).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Integer,
            Box::new(std::fmt::Error),
        )
    })
}

fn read_heartrate_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<HeartRatePoint> {
    let bpm = row.get::<_, i64>(1)?;
    let bpm = u16::try_from(bpm).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Integer,
            Box::new(std::fmt::Error),
        )
    })?;

    Ok(HeartRatePoint {
        recorded_at: row.get(0)?,
        bpm,
        source_day: row.get(2)?,
    })
}

fn read_workout_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkoutRecord> {
    Ok(WorkoutRecord {
        workout_id: row.get(0)?,
        day: row.get(1)?,
        started_at: row.get(2)?,
        ended_at: row.get(3)?,
        timezone: row.get(4)?,
        sport: row.get(5)?,
        activity: row.get(6)?,
        intensity: row.get(7)?,
        title: row.get(8)?,
        notes: row.get(9)?,
        source: row.get(10)?,
        raw_cache_key: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn read_enhanced_tag_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EnhancedTagRecord> {
    Ok(EnhancedTagRecord {
        enhanced_tag_id: row.get(0)?,
        day: row.get(1)?,
        started_at: row.get(2)?,
        ended_at: row.get(3)?,
        label: row.get(4)?,
        subtype: row.get(5)?,
        comment: row.get(6)?,
        intensity: row.get(7)?,
        raw_cache_key: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn read_session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionRecord> {
    Ok(SessionRecord {
        session_id: row.get(0)?,
        day: row.get(1)?,
        started_at: row.get(2)?,
        ended_at: row.get(3)?,
        kind: row.get(4)?,
        state: row.get(5)?,
        score: row.get(6)?,
        title: row.get(7)?,
        raw_cache_key: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn read_context_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ContextEventRecord> {
    Ok(ContextEventRecord {
        context_event_id: row.get(0)?,
        family: parse_context_event_family(&row.get::<_, String>(1)?, 1)?,
        source_id: row.get(2)?,
        anchor_day: row.get(3)?,
        start_at: row.get(4)?,
        end_at: row.get(5)?,
        time_semantics: parse_time_semantics(&row.get::<_, String>(6)?, 6)?,
        title: row.get(7)?,
        subtype: row.get(8)?,
        notes: row.get(9)?,
        intensity: row.get(10)?,
        metadata_json: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn read_pattern_summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PatternSummaryRecord> {
    Ok(PatternSummaryRecord {
        summary_id: row.get(0)?,
        family: parse_context_event_family(&row.get::<_, String>(1)?, 1)?,
        normalized_key: row.get(2)?,
        relation_window: parse_pattern_relation_window(&row.get::<_, String>(3)?, 3)?,
        metric: parse_pattern_metric(&row.get::<_, String>(4)?, 4)?,
        sample_count: parse_u32(row.get::<_, i64>(5)?, 5)?,
        median_delta: row.get(6)?,
        effect_direction: parse_effect_direction(&row.get::<_, String>(7)?, 7)?,
        confidence: parse_data_sufficiency(&row.get::<_, String>(8)?, 8)?,
        metadata_json: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn read_review_signal_day_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReviewSignalDayRecord> {
    Ok(ReviewSignalDayRecord {
        signal_key: row.get(0)?,
        day: row.get(1)?,
        numeric_value: row.get(2)?,
        text_value: row.get(3)?,
        baseline_mean: row.get(4)?,
        baseline_stddev: row.get(5)?,
        delta: row.get(6)?,
        z_score: row.get(7)?,
        persistence_days: parse_u32(row.get::<_, i64>(8)?, 8)?,
        sufficiency: parse_review_sufficiency(&row.get::<_, String>(9)?, 9)?,
        stale_days: parse_u32(row.get::<_, i64>(10)?, 10)?,
        metadata_json: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn parse_context_event_family(value: &str, column: usize) -> rusqlite::Result<ContextEventFamily> {
    ContextEventFamily::parse(value).ok_or_else(|| invalid_text_enum(column, value))
}

fn parse_time_semantics(value: &str, column: usize) -> rusqlite::Result<TimeSemantics> {
    TimeSemantics::parse(value).ok_or_else(|| invalid_text_enum(column, value))
}

fn parse_pattern_metric(value: &str, column: usize) -> rusqlite::Result<PatternMetric> {
    PatternMetric::parse(value).ok_or_else(|| invalid_text_enum(column, value))
}

fn parse_pattern_relation_window(
    value: &str,
    column: usize,
) -> rusqlite::Result<PatternRelationWindow> {
    PatternRelationWindow::parse(value).ok_or_else(|| invalid_text_enum(column, value))
}

fn parse_data_sufficiency(value: &str, column: usize) -> rusqlite::Result<DataSufficiency> {
    DataSufficiency::parse(value).ok_or_else(|| invalid_text_enum(column, value))
}

fn parse_effect_direction(value: &str, column: usize) -> rusqlite::Result<EffectDirection> {
    EffectDirection::parse(value).ok_or_else(|| invalid_text_enum(column, value))
}

fn parse_review_sufficiency(value: &str, column: usize) -> rusqlite::Result<ReviewSufficiency> {
    match value {
        "missing" => Ok(ReviewSufficiency::Missing),
        "thin" => Ok(ReviewSufficiency::Thin),
        "medium" => Ok(ReviewSufficiency::Medium),
        "strong" => Ok(ReviewSufficiency::Strong),
        _ => Err(invalid_text_enum(column, value)),
    }
}

fn invalid_text_enum(column: usize, value: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid enum value `{value}`"),
        )),
    )
}

fn row_count(connection: &Connection, table: &str) -> Result<u64> {
    let query = format!("SELECT COUNT(*) FROM {table}");
    let count = connection.query_row(&query, [], |row| row.get::<_, i64>(0))?;
    u64::try_from(count).map_err(|error| {
        RingmasterError::Config(format!(
            "negative row count for `{table}` is invalid: {error}"
        ))
    })
}

fn extract_day_suffix(identifier: &str) -> Option<&str> {
    let candidate = identifier.get(identifier.len().checked_sub(10)?..)?;
    let bytes = candidate.as_bytes();
    if bytes.len() == 10
        && bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && bytes[2].is_ascii_digit()
        && bytes[3].is_ascii_digit()
        && bytes[4] == b'-'
        && bytes[5].is_ascii_digit()
        && bytes[6].is_ascii_digit()
        && bytes[7] == b'-'
        && bytes[8].is_ascii_digit()
        && bytes[9].is_ascii_digit()
    {
        Some(candidate)
    } else {
        None
    }
}

fn join_scopes(scopes: &[String]) -> String {
    scopes.join(",")
}

fn split_scopes(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn encode_problem(problem: Option<&OuraProblem>) -> Result<Option<String>> {
    problem
        .map(serde_json::to_string)
        .transpose()
        .map_err(Into::into)
}

fn decode_problem(
    value: Option<&str>,
) -> std::result::Result<Option<OuraProblem>, serde_json::Error> {
    value.map(serde_json::from_str::<OuraProblem>).transpose()
}

fn json_to_sql_error(error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| RingmasterError::Config(format!("formatting timestamp failed: {error}")))
}

#[cfg(test)]
mod tests {
    use super::{AiEvalRunRecord, ReportExportRecord};
    use rusqlite::params;

    use crate::error::OuraProblem;
    use crate::review::features::ReviewSufficiency;
    use crate::store::Store;
    use crate::store::queries::{
        AiArtifactDaySummaryRecord, AiArtifactRecord, AiRunRecord, ContextEventFamily,
        ContextEventRecord, DailyActivityRecord, DailyReadinessRecord, DailySleepRecord,
        HeartrateSampleRecord, RestModePeriodRecord, ReviewSignalDayRecord, SnapshotExportRecord,
        SnapshotProvenanceRefRecord, SyncRunStatus, SyncStateRecord, TimeSemantics, Vo2MaxRecord,
    };

    fn seed_daily_history(store: &Store) {
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
                    sleep_duration_seconds: Some(27_000),
                    raw_cache_key: None,
                    updated_at: format!("{day}T06:00:00Z"),
                })
                .unwrap_or_else(|error| unreachable!("sleep row should seed: {error}"));
            store
                .imports()
                .upsert_daily_readiness(&DailyReadinessRecord {
                    oura_id: None,
                    day: day.to_owned(),
                    readiness_score: Some(readiness),
                    temperature_deviation: None,
                    temperature_trend_deviation: None,
                    raw_cache_key: None,
                    updated_at: format!("{day}T06:05:00Z"),
                })
                .unwrap_or_else(|error| unreachable!("readiness row should seed: {error}"));
            store
                .imports()
                .upsert_daily_activity(&DailyActivityRecord {
                    oura_id: None,
                    day: day.to_owned(),
                    activity_score: Some(activity),
                    active_calories: 400,
                    steps: 8_000,
                    total_calories: 2_300,
                    raw_cache_key: None,
                    updated_at: format!("{day}T06:10:00Z"),
                })
                .unwrap_or_else(|error| unreachable!("activity row should seed: {error}"));
        }
    }

    fn make_snapshot_export(snapshot_hash: &str, anchor_day: &str) -> SnapshotExportRecord {
        SnapshotExportRecord {
            snapshot_hash: snapshot_hash.to_owned(),
            schema_version: "ringmaster.snapshot.v3".to_owned(),
            app_version: "0.1.0".to_owned(),
            generated_at: format!("{anchor_day}T00:00:00Z"),
            scope: "today".to_owned(),
            start_day: anchor_day.to_owned(),
            end_day: anchor_day.to_owned(),
            anchor_day: anchor_day.to_owned(),
            day_count: 1,
            privacy_profile: "redacted".to_owned(),
            source_mode: "demo".to_owned(),
            fixture_dir: None,
            latest_source_day: Some(anchor_day.to_owned()),
            latest_review_day: Some(anchor_day.to_owned()),
            freshness_summary: format!(
                "latest_source_day={anchor_day} latest_review_day={anchor_day} warnings=0"
            ),
            trust_summary: "review_signals=1 strong=1 stale=0 follow_up_targets=1".to_owned(),
            capability_summary: "granted=1 missing=0 requested=1".to_owned(),
            provenance_summary: "refs=0 local_kinds=0".to_owned(),
            snapshot_json: "{\"schema_version\":\"ringmaster.snapshot.v3\"}".to_owned(),
            created_at: format!("{anchor_day}T00:00:01Z"),
        }
    }

    fn make_ai_artifact(
        artifact_id: &str,
        artifact_kind: &str,
        created_at: &str,
        snapshot_hash_a: &str,
        snapshot_hash_b: Option<&str>,
    ) -> AiArtifactRecord {
        AiArtifactRecord {
            artifact_id: artifact_id.to_owned(),
            artifact_kind: artifact_kind.to_owned(),
            output_schema_version: format!("ringmaster.ai.{artifact_kind}.v3"),
            prompt_version: match artifact_kind {
                "review" => "review_prompt_v3".to_owned(),
                "compare" => "compare_prompt_v2".to_owned(),
                "follow_up" => "follow_up_prompt_v2".to_owned(),
                other => format!("{other}_prompt_v2"),
            },
            provider: "dry_run".to_owned(),
            model: "deterministic".to_owned(),
            reasoning_effort: None,
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            run_mode: "dry_run".to_owned(),
            created_at: created_at.to_owned(),
            snapshot_hash_a: snapshot_hash_a.to_owned(),
            snapshot_hash_b: snapshot_hash_b.map(str::to_owned),
            privacy_profile: "redacted".to_owned(),
            artifact_status: "dry_run".to_owned(),
            overview: format!("{artifact_kind} overview"),
            summary_cache: format!("{artifact_kind} summary"),
            request_fingerprint: Some(format!("fingerprint-{artifact_id}")),
            payload_json: format!("{{\"artifact_id\":\"{artifact_id}\"}}"),
            rendered_briefing: format!("{artifact_kind} rendered briefing"),
        }
    }

    fn make_ai_run(
        run_id: &str,
        run_kind: &str,
        run_status: &str,
        snapshot_hash_a: &str,
        snapshot_hash_b: Option<&str>,
    ) -> AiRunRecord {
        AiRunRecord {
            run_id: run_id.to_owned(),
            run_kind: run_kind.to_owned(),
            run_status: run_status.to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-5.1".to_owned(),
            reasoning_effort: Some("medium".to_owned()),
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            run_mode: "real".to_owned(),
            prompt_version: match run_kind {
                "review" => "review_prompt_v3".to_owned(),
                "compare" => "compare_prompt_v2".to_owned(),
                "follow_up" => "follow_up_prompt_v2".to_owned(),
                other => format!("{other}_prompt_v2"),
            },
            output_schema_version: format!("ringmaster.ai.{run_kind}.v3"),
            privacy_profile: "redacted".to_owned(),
            snapshot_scope: "today".to_owned(),
            snapshot_hash_a: snapshot_hash_a.to_owned(),
            snapshot_hash_b: snapshot_hash_b.map(str::to_owned),
            source_ai_artifact_id: None,
            follow_up_kind: None,
            request_fingerprint: Some(format!("fingerprint-{run_id}")),
            request_preview_json: "{\"task_family\":\"review\"}".to_owned(),
            artifact_id: None,
            error_message: None,
            created_at: "2026-04-10T00:01:00Z".to_owned(),
            started_at: Some("2026-04-10T00:01:01Z".to_owned()),
            ended_at: Some("2026-04-10T00:01:02Z".to_owned()),
            updated_at: "2026-04-10T00:01:02Z".to_owned(),
        }
    }

    fn make_report_export(
        report_id: &str,
        snapshot_hash: &str,
        artifact_id: &str,
    ) -> ReportExportRecord {
        ReportExportRecord {
            report_id: report_id.to_owned(),
            report_kind: "ai_review_report".to_owned(),
            title: "AI review report".to_owned(),
            format: "markdown".to_owned(),
            output_path: "/tmp/report.md".to_owned(),
            content_hash: "content-hash".to_owned(),
            privacy_profile: "redacted".to_owned(),
            created_at: "2026-04-10T00:10:00Z".to_owned(),
            source_snapshot_hash_a: Some(snapshot_hash.to_owned()),
            source_snapshot_hash_b: None,
            source_ai_artifact_id: Some(artifact_id.to_owned()),
            provider: Some("dry_run".to_owned()),
            model: Some("deterministic".to_owned()),
            prompt_version: Some("review_prompt_v3".to_owned()),
            output_schema_version: Some("ringmaster.ai.review.v3".to_owned()),
            export_status: "written".to_owned(),
            last_verified_exists: true,
            last_verified_at: "2026-04-10T00:10:01Z".to_owned(),
        }
    }

    fn make_ai_eval_run(eval_run_id: &str) -> AiEvalRunRecord {
        AiEvalRunRecord {
            eval_run_id: eval_run_id.to_owned(),
            task_family: "mixed".to_owned(),
            fixture_dir: "tests/fixtures/ai".to_owned(),
            candidate_label: "candidate".to_owned(),
            baseline_label: Some("baseline".to_owned()),
            provider: "fixture".to_owned(),
            model: "mixed".to_owned(),
            prompt_version: "mixed".to_owned(),
            output_schema_version: "mixed".to_owned(),
            created_at: "2026-04-10T00:11:00Z".to_owned(),
            total_cases: 2,
            passed_cases: 2,
            failed_cases: 0,
            schema_validity_score: 1.0,
            completeness_score: 1.0,
            overclaiming_score: 1.0,
            medical_safety_score: 1.0,
            privacy_score: 1.0,
            evidence_score: 1.0,
            honesty_score: 1.0,
            regression_summary: "Improvements: compare:evidence; regressions: none.".to_owned(),
            details_json: r#"{"fixture_dir":"tests/fixtures/ai","fixture_schema_version":"ringmaster.ai.eval.fixtures.v1","candidate_label":"candidate","baseline_label":"baseline","total_cases":2,"passed_cases":2,"failed_cases":0,"scores":{"schema_validity":1.0,"completeness":1.0,"overclaiming":1.0,"medical_safety":1.0,"privacy":1.0,"evidence":1.0,"honesty":1.0},"regression_summary":"Improvements: compare:evidence; regressions: none.","improvements":["compare:evidence"],"regressions":[],"cases":[]}"#.to_owned(),
        }
    }

    #[test]
    fn sync_state_round_trips_backoff_metadata() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));

        store
            .sync_state()
            .upsert(&SyncStateRecord {
                sync_key: "oura.daily".to_owned(),
                family: "daily".to_owned(),
                status: SyncRunStatus::Failed,
                cursor: Some("2026-04-08".to_owned()),
                last_successful_sync_end: Some("2026-04-08".to_owned()),
                last_attempted_at: "2026-04-08T06:00:00Z".to_owned(),
                last_completed_at: Some("2026-04-08T06:00:05Z".to_owned()),
                last_reconcile_end: Some("2026-04-08".to_owned()),
                oldest_recently_reconciled_at: Some("2026-03-10".to_owned()),
                message: Some("rate limited".to_owned()),
                granted_scopes: vec!["daily".to_owned()],
                last_error: Some(OuraProblem::new(
                    Some(429),
                    "rate limited",
                    Some("retry later".to_owned()),
                )),
                last_error_at: Some("2026-04-08T06:00:05Z".to_owned()),
                last_error_kind: Some("rate_limit".to_owned()),
                last_error_detail: Some("rate limited".to_owned()),
                failure_count: 3,
                next_attempt_after: Some("2026-04-08T06:05:00Z".to_owned()),
                last_trigger_source: Some("periodic_reconcile".to_owned()),
                last_trigger_detail: Some("daily scheduler".to_owned()),
                updated_at: "2026-04-08T06:00:05Z".to_owned(),
            })
            .unwrap_or_else(|error| unreachable!("sync state should persist: {error}"));

        let record = store
            .sync_state()
            .get("oura.daily")
            .unwrap_or_else(|error| unreachable!("sync state should read: {error}"))
            .unwrap_or_else(|| unreachable!("sync state should exist"));

        assert_eq!(record.failure_count, 3);
        assert_eq!(
            record.next_attempt_after.as_deref(),
            Some("2026-04-08T06:05:00Z")
        );
        assert_eq!(record.status, SyncRunStatus::Failed);
    }

    #[test]
    fn sync_state_family_falls_back_from_sync_key_without_re_reading_the_row() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));

        store
            .sync_state()
            .connection
            .execute(
                "INSERT INTO sync_state (
                    sync_key,
                    family,
                    status,
                    cursor,
                    last_successful_sync_end,
                    last_attempted_at,
                    last_completed_at,
                    last_reconcile_end,
                    oldest_recently_reconciled_at,
                    message,
                    granted_scopes,
                    last_error_json,
                    last_error_at,
                    last_error_kind,
                    last_error_detail,
                    failure_count,
                    next_attempt_after,
                    last_trigger_source,
                    last_trigger_detail,
                    updated_at
                ) VALUES (
                    ?1, NULL, ?2, NULL, NULL, ?3, NULL, NULL, NULL, NULL, '', NULL, NULL, NULL, NULL, 0, NULL, NULL, NULL, ?4
                )",
                params![
                    "oura.spo2",
                    SyncRunStatus::Success.as_str(),
                    "2026-04-08T06:00:00Z",
                    "2026-04-08T06:00:00Z",
                ],
            )
            .unwrap_or_else(|error| unreachable!("sync state row should insert: {error}"));

        let record = store
            .sync_state()
            .get("oura.spo2")
            .unwrap_or_else(|error| unreachable!("sync state should read: {error}"))
            .unwrap_or_else(|| unreachable!("sync state should exist"));

        assert_eq!(record.sync_key, "oura.spo2");
        assert_eq!(record.family, "spo2");
    }

    #[test]
    fn daily_history_returns_oldest_to_newest_rows() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));
        seed_daily_history(&store);

        let history = store
            .views()
            .daily_history(30)
            .unwrap_or_else(|error| unreachable!("daily history should load: {error}"));

        assert_eq!(history.len(), 3);
        assert_eq!(
            history.first().map(|row| row.day.as_str()),
            Some("2026-04-06")
        );
        assert_eq!(
            history.last().map(|row| row.day.as_str()),
            Some("2026-04-08")
        );
        assert_eq!(history[2].sleep_score, Some(84));
    }

    #[test]
    fn heartrate_queries_return_per_day_points_and_day_list() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));

        for (timestamp, bpm, day) in [
            ("2026-04-07T08:00:00Z", 58, "2026-04-07"),
            ("2026-04-07T08:05:00Z", 60, "2026-04-07"),
            ("2026-04-08T09:00:00Z", 64, "2026-04-08"),
        ] {
            store
                .imports()
                .upsert_heartrate_sample(&HeartrateSampleRecord {
                    recorded_at: timestamp.to_owned(),
                    bpm,
                    source_day: Some(day.to_owned()),
                    raw_cache_key: None,
                    updated_at: timestamp.to_owned(),
                })
                .unwrap_or_else(|error| unreachable!("heartrate sample should seed: {error}"));
        }

        let days = store
            .views()
            .available_heartrate_days(10)
            .unwrap_or_else(|error| unreachable!("heartrate days should load: {error}"));
        let points = store
            .views()
            .heartrate_for_day("2026-04-07")
            .unwrap_or_else(|error| unreachable!("heartrate day should load: {error}"));

        assert_eq!(days, vec!["2026-04-07".to_owned(), "2026-04-08".to_owned()]);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].bpm, 58);
        assert_eq!(points[1].bpm, 60);
    }

    #[test]
    fn latest_source_day_tracks_newest_persisted_family_day() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));
        seed_daily_history(&store);
        store
            .imports()
            .upsert_rest_mode_period(&RestModePeriodRecord {
                period_id: "rest-1".to_owned(),
                start_day: "2026-04-07".to_owned(),
                start_time: Some("2026-04-07T00:00:00Z".to_owned()),
                end_day: Some("2026-04-09".to_owned()),
                end_time: Some("2026-04-09T23:59:59Z".to_owned()),
                episode_count: 1,
                tags_json: "[]".to_owned(),
                raw_cache_key: None,
                updated_at: "2026-04-09T23:59:59Z".to_owned(),
            })
            .unwrap_or_else(|error| unreachable!("rest mode period should seed: {error}"));

        assert_eq!(
            store
                .views()
                .latest_source_day()
                .unwrap_or_else(|error| unreachable!("latest day should load: {error}"))
                .as_deref(),
            Some("2026-04-09")
        );
    }

    #[test]
    fn latest_source_day_treats_open_rest_mode_as_current() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));
        let current_day = crate::time_utils::current_local_day_string();
        store
            .imports()
            .upsert_rest_mode_period(&RestModePeriodRecord {
                period_id: "rest-open".to_owned(),
                start_day: "2026-04-01".to_owned(),
                start_time: Some("2026-04-01T00:00:00Z".to_owned()),
                end_day: None,
                end_time: None,
                episode_count: 1,
                tags_json: "[]".to_owned(),
                raw_cache_key: None,
                updated_at: "2026-04-01T00:00:00Z".to_owned(),
            })
            .unwrap_or_else(|error| unreachable!("rest mode period should seed: {error}"));

        assert_eq!(
            store
                .views()
                .latest_source_day()
                .unwrap_or_else(|error| unreachable!("latest day should load: {error}"))
                .as_deref(),
            Some(current_day.as_str())
        );
    }

    #[test]
    fn latest_review_day_prefers_reviewable_sources() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));

        store
            .derived()
            .replace_review_signal_days(&[ReviewSignalDayRecord {
                signal_key: "sleep_score".to_owned(),
                day: "2026-04-08".to_owned(),
                numeric_value: Some(82.0),
                text_value: None,
                baseline_mean: Some(80.0),
                baseline_stddev: Some(4.0),
                delta: Some(2.0),
                z_score: Some(0.5),
                persistence_days: 1,
                sufficiency: ReviewSufficiency::Medium,
                stale_days: 0,
                metadata_json: "{}".to_owned(),
                updated_at: "2026-04-08T12:00:00Z".to_owned(),
            }])
            .unwrap_or_else(|error| unreachable!("review signal day should seed: {error}"));
        store
            .imports()
            .upsert_rest_mode_period(&RestModePeriodRecord {
                period_id: "rest-1".to_owned(),
                start_day: "2026-04-07".to_owned(),
                start_time: Some("2026-04-07T00:00:00Z".to_owned()),
                end_day: Some("2026-04-10".to_owned()),
                end_time: Some("2026-04-10T23:59:59Z".to_owned()),
                episode_count: 1,
                tags_json: "[]".to_owned(),
                raw_cache_key: None,
                updated_at: "2026-04-10T23:59:59Z".to_owned(),
            })
            .unwrap_or_else(|error| unreachable!("rest mode period should seed: {error}"));

        assert_eq!(
            store
                .views()
                .latest_review_day()
                .unwrap_or_else(|error| unreachable!("latest review day should load: {error}"))
                .as_deref(),
            Some("2026-04-10")
        );
    }

    #[test]
    fn latest_review_day_treats_open_rest_mode_as_current() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));
        let current_day = crate::time_utils::current_local_day_string();
        store
            .imports()
            .upsert_rest_mode_period(&RestModePeriodRecord {
                period_id: "rest-open".to_owned(),
                start_day: "2026-04-01".to_owned(),
                start_time: Some("2026-04-01T00:00:00Z".to_owned()),
                end_day: None,
                end_time: None,
                episode_count: 1,
                tags_json: "[]".to_owned(),
                raw_cache_key: None,
                updated_at: "2026-04-01T00:00:00Z".to_owned(),
            })
            .unwrap_or_else(|error| unreachable!("rest mode period should seed: {error}"));

        assert_eq!(
            store
                .views()
                .latest_review_day()
                .unwrap_or_else(|error| unreachable!("latest review day should load: {error}"))
                .as_deref(),
            Some(current_day.as_str())
        );
    }

    #[test]
    fn analysis_store_round_trips_snapshot_exports_and_provenance() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));
        let record = SnapshotExportRecord {
            snapshot_hash: "hash-123".to_owned(),
            schema_version: "ringmaster.snapshot.v3".to_owned(),
            app_version: "0.1.0".to_owned(),
            generated_at: "2026-04-10T00:00:00Z".to_owned(),
            scope: "today".to_owned(),
            start_day: "2026-04-10".to_owned(),
            end_day: "2026-04-10".to_owned(),
            anchor_day: "2026-04-10".to_owned(),
            day_count: 1,
            privacy_profile: "redacted".to_owned(),
            source_mode: "demo".to_owned(),
            fixture_dir: Some("tests/fixtures/phase7/strong".to_owned()),
            latest_source_day: Some("2026-04-10".to_owned()),
            latest_review_day: Some("2026-04-10".to_owned()),
            freshness_summary:
                "latest_source_day=2026-04-10 latest_review_day=2026-04-10 warnings=0".to_owned(),
            trust_summary: "review_signals=1 strong=1 stale=0 follow_up_targets=1".to_owned(),
            capability_summary: "granted=3 missing=0 requested=3".to_owned(),
            provenance_summary: "refs=1 local_kinds=1".to_owned(),
            snapshot_json: "{\"schema_version\":\"ringmaster.snapshot.v3\"}".to_owned(),
            created_at: "2026-04-10T00:00:01Z".to_owned(),
        };
        let provenance = vec![SnapshotProvenanceRefRecord {
            snapshot_hash: "hash-123".to_owned(),
            export_ref: "daily:2026-04-10".to_owned(),
            local_kind: "daily_overview".to_owned(),
            local_locator: "2026-04-10".to_owned(),
            created_at: "2026-04-10T00:00:00Z".to_owned(),
        }];

        store
            .analysis()
            .upsert_snapshot_export(&record, &provenance)
            .unwrap_or_else(|error| unreachable!("snapshot export should persist: {error}"));

        let loaded = store
            .analysis()
            .snapshot_export("hash-123")
            .unwrap_or_else(|error| unreachable!("snapshot export should load: {error}"))
            .unwrap_or_else(|| unreachable!("snapshot export should exist"));
        let loaded_provenance = store
            .analysis()
            .snapshot_provenance_refs("hash-123")
            .unwrap_or_else(|error| unreachable!("provenance refs should load: {error}"));

        assert_eq!(loaded.scope, "today");
        assert_eq!(loaded.privacy_profile, "redacted");
        assert_eq!(loaded_provenance.len(), 1);
        assert_eq!(loaded_provenance[0].export_ref, "daily:2026-04-10");
    }

    #[test]
    fn analysis_store_preserves_provenance_on_metadata_only_snapshot_upsert() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));
        let record = SnapshotExportRecord {
            snapshot_hash: "hash-keep".to_owned(),
            schema_version: "ringmaster.snapshot.v3".to_owned(),
            app_version: "0.1.0".to_owned(),
            generated_at: "2026-04-10T00:00:00Z".to_owned(),
            scope: "today".to_owned(),
            start_day: "2026-04-10".to_owned(),
            end_day: "2026-04-10".to_owned(),
            anchor_day: "2026-04-10".to_owned(),
            day_count: 1,
            privacy_profile: "redacted".to_owned(),
            source_mode: "demo".to_owned(),
            fixture_dir: Some("tests/fixtures/phase7/strong".to_owned()),
            latest_source_day: Some("2026-04-10".to_owned()),
            latest_review_day: Some("2026-04-10".to_owned()),
            freshness_summary:
                "latest_source_day=2026-04-10 latest_review_day=2026-04-10 warnings=0".to_owned(),
            trust_summary: "review_signals=1 strong=1 stale=0 follow_up_targets=1".to_owned(),
            capability_summary: "granted=3 missing=0 requested=3".to_owned(),
            provenance_summary: "refs=1 local_kinds=1".to_owned(),
            snapshot_json: "{\"schema_version\":\"ringmaster.snapshot.v3\"}".to_owned(),
            created_at: "2026-04-10T00:00:01Z".to_owned(),
        };
        let provenance = vec![SnapshotProvenanceRefRecord {
            snapshot_hash: "hash-keep".to_owned(),
            export_ref: "daily:2026-04-10".to_owned(),
            local_kind: "daily_overview".to_owned(),
            local_locator: "2026-04-10".to_owned(),
            created_at: "2026-04-10T00:00:00Z".to_owned(),
        }];

        store
            .analysis()
            .upsert_snapshot_export(&record, &provenance)
            .unwrap_or_else(|error| unreachable!("snapshot export should persist: {error}"));

        let metadata_only = SnapshotExportRecord {
            fixture_dir: None,
            freshness_summary:
                "latest_source_day=2026-04-10 latest_review_day=2026-04-10 warnings=1".to_owned(),
            provenance_summary: "refs=0 local_kinds=0".to_owned(),
            ..record
        };

        store
            .analysis()
            .upsert_snapshot_export(&metadata_only, &[])
            .unwrap_or_else(|error| unreachable!("metadata-only upsert should persist: {error}"));

        let loaded = store
            .analysis()
            .snapshot_export("hash-keep")
            .unwrap_or_else(|error| unreachable!("snapshot export should load: {error}"))
            .unwrap_or_else(|| unreachable!("snapshot export should exist"));
        let loaded_provenance = store
            .analysis()
            .snapshot_provenance_refs("hash-keep")
            .unwrap_or_else(|error| unreachable!("provenance refs should load: {error}"));

        assert_eq!(
            loaded.fixture_dir.as_deref(),
            Some("tests/fixtures/phase7/strong")
        );
        assert_eq!(loaded.provenance_summary, "refs=1 local_kinds=1");
        assert_eq!(loaded.freshness_summary, metadata_only.freshness_summary);
        assert_eq!(loaded_provenance.len(), 1);
        assert_eq!(loaded_provenance[0].export_ref, "daily:2026-04-10");
    }

    #[test]
    fn analysis_store_preserves_snapshot_created_at_on_upsert() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));
        let original = SnapshotExportRecord {
            snapshot_hash: "hash-created-at".to_owned(),
            schema_version: "ringmaster.snapshot.v3".to_owned(),
            app_version: "0.1.0".to_owned(),
            generated_at: "2026-04-10T00:00:00Z".to_owned(),
            scope: "today".to_owned(),
            start_day: "2026-04-10".to_owned(),
            end_day: "2026-04-10".to_owned(),
            anchor_day: "2026-04-10".to_owned(),
            day_count: 1,
            privacy_profile: "redacted".to_owned(),
            source_mode: "demo".to_owned(),
            fixture_dir: None,
            latest_source_day: Some("2026-04-10".to_owned()),
            latest_review_day: Some("2026-04-10".to_owned()),
            freshness_summary:
                "latest_source_day=2026-04-10 latest_review_day=2026-04-10 warnings=0".to_owned(),
            trust_summary: "review_signals=1 strong=1 stale=0 follow_up_targets=1".to_owned(),
            capability_summary: "granted=3 missing=0 requested=3".to_owned(),
            provenance_summary: "refs=0 local_kinds=0".to_owned(),
            snapshot_json: "{\"schema_version\":\"ringmaster.snapshot.v3\"}".to_owned(),
            created_at: "2026-04-10T00:00:01Z".to_owned(),
        };

        store
            .analysis()
            .upsert_snapshot_export(&original, &[])
            .unwrap_or_else(|error| unreachable!("snapshot export should persist: {error}"));

        let refreshed = SnapshotExportRecord {
            created_at: "2026-04-09T23:59:59Z".to_owned(),
            freshness_summary:
                "latest_source_day=2026-04-10 latest_review_day=2026-04-10 warnings=1".to_owned(),
            ..original
        };

        store
            .analysis()
            .upsert_snapshot_export(&refreshed, &[])
            .unwrap_or_else(|error| unreachable!("snapshot export should refresh: {error}"));

        let loaded = store
            .analysis()
            .snapshot_export("hash-created-at")
            .unwrap_or_else(|error| unreachable!("snapshot export should load: {error}"))
            .unwrap_or_else(|| unreachable!("snapshot export should exist"));

        assert_eq!(loaded.created_at, "2026-04-10T00:00:01Z");
        assert_eq!(loaded.freshness_summary, refreshed.freshness_summary);
    }

    #[test]
    fn analysis_store_round_trips_ai_artifacts() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));
        let artifact = AiArtifactRecord {
            artifact_id: "artifact-123".to_owned(),
            artifact_kind: "review".to_owned(),
            output_schema_version: "ringmaster.ai.review.v3".to_owned(),
            prompt_version: "review_prompt_v3".to_owned(),
            provider: "dry_run".to_owned(),
            model: "deterministic".to_owned(),
            reasoning_effort: None,
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            run_mode: "dry_run".to_owned(),
            created_at: "2026-04-10T00:05:00Z".to_owned(),
            snapshot_hash_a: "hash-123".to_owned(),
            snapshot_hash_b: None,
            privacy_profile: "redacted".to_owned(),
            artifact_status: "dry_run".to_owned(),
            overview: "Dry-run review for today.".to_owned(),
            summary_cache: "Dry-run review for today.".to_owned(),
            request_fingerprint: Some("fingerprint-123".to_owned()),
            payload_json: "{\"status\":\"dry_run\"}".to_owned(),
            rendered_briefing: "ringmaster ai review".to_owned(),
        };

        store
            .analysis()
            .upsert_ai_artifact(&artifact)
            .unwrap_or_else(|error| unreachable!("ai artifact should persist: {error}"));

        let loaded = store
            .analysis()
            .latest_ai_artifact("review", "hash-123")
            .unwrap_or_else(|error| unreachable!("ai artifact should load: {error}"))
            .unwrap_or_else(|| unreachable!("ai artifact should exist"));

        assert_eq!(loaded.provider, "dry_run");
        assert_eq!(loaded.payload_json, "{\"status\":\"dry_run\"}");
    }

    #[test]
    fn analysis_store_round_trips_ai_runs() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));
        store
            .analysis()
            .upsert_snapshot_export(&make_snapshot_export("hash-123", "2026-04-10"), &[])
            .unwrap_or_else(|error| unreachable!("snapshot export should persist: {error}"));
        let run = make_ai_run("run-123", "review", "queued", "hash-123", None);

        store
            .analysis()
            .upsert_ai_run(&run)
            .unwrap_or_else(|error| unreachable!("ai run should persist: {error}"));

        let loaded = store
            .analysis()
            .ai_run("run-123")
            .unwrap_or_else(|error| unreachable!("ai run should load: {error}"))
            .unwrap_or_else(|| unreachable!("ai run should exist"));

        assert_eq!(loaded.run_status, "queued");
        assert_eq!(loaded.provider, "openai");
        assert_eq!(loaded.snapshot_hash_a, "hash-123");
    }

    #[test]
    fn analysis_store_updates_ai_runs_only_while_active() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));
        store
            .analysis()
            .upsert_snapshot_export(&make_snapshot_export("hash-123", "2026-04-10"), &[])
            .unwrap_or_else(|error| unreachable!("snapshot export should persist: {error}"));

        let queued = make_ai_run("run-conditional", "review", "queued", "hash-123", None);
        store
            .analysis()
            .upsert_ai_run(&queued)
            .unwrap_or_else(|error| unreachable!("queued run should persist: {error}"));

        let mut cancelled = queued;
        cancelled.run_status = "cancelled".to_owned();
        cancelled.error_message = Some("Cancelled from test.".to_owned());
        cancelled.ended_at = Some("2026-04-10T00:02:00Z".to_owned());
        cancelled.updated_at = "2026-04-10T00:02:00Z".to_owned();

        let updated = store
            .analysis()
            .update_ai_run_if_active(&cancelled)
            .unwrap_or_else(|error| unreachable!("active run transition should succeed: {error}"));
        assert!(updated);

        let persisted = store
            .analysis()
            .ai_run("run-conditional")
            .unwrap_or_else(|error| unreachable!("ai run should load: {error}"))
            .unwrap_or_else(|| unreachable!("ai run should exist"));
        assert_eq!(persisted.run_status, "cancelled");

        let mut succeeded = persisted;
        succeeded.run_status = "succeeded".to_owned();
        succeeded.error_message = None;
        succeeded.updated_at = "2026-04-10T00:03:00Z".to_owned();
        store
            .analysis()
            .upsert_ai_run(&succeeded)
            .unwrap_or_else(|error| unreachable!("succeeded run should persist: {error}"));

        let mut interrupted = succeeded;
        interrupted.run_status = "interrupted".to_owned();
        interrupted.error_message = Some("Interrupted after completion.".to_owned());
        interrupted.updated_at = "2026-04-10T00:04:00Z".to_owned();

        let updated = store
            .analysis()
            .update_ai_run_if_active(&interrupted)
            .unwrap_or_else(|error| {
                unreachable!("inactive run transition should not error: {error}")
            });
        assert!(!updated);

        let persisted = store
            .analysis()
            .ai_run("run-conditional")
            .unwrap_or_else(|error| unreachable!("ai run should reload: {error}"))
            .unwrap_or_else(|| unreachable!("ai run should still exist"));
        assert_eq!(persisted.run_status, "succeeded");
        assert!(persisted.error_message.is_none());
    }

    #[test]
    fn list_ai_runs_for_snapshot_includes_compare_runs_on_either_side() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));
        store
            .analysis()
            .upsert_snapshot_export(&make_snapshot_export("hash-left", "2026-04-09"), &[])
            .unwrap_or_else(|error| unreachable!("left snapshot should persist: {error}"));
        store
            .analysis()
            .upsert_snapshot_export(&make_snapshot_export("hash-right", "2026-04-10"), &[])
            .unwrap_or_else(|error| unreachable!("right snapshot should persist: {error}"));

        store
            .analysis()
            .upsert_ai_run(&make_ai_run(
                "run-compare",
                "compare",
                "running",
                "hash-left",
                Some("hash-right"),
            ))
            .unwrap_or_else(|error| unreachable!("compare run should persist: {error}"));

        let right_runs = store
            .analysis()
            .list_ai_runs_for_snapshot("hash-right")
            .unwrap_or_else(|error| unreachable!("ai runs should load: {error}"));

        assert_eq!(right_runs.len(), 1);
        assert_eq!(right_runs[0].run_id, "run-compare");
        assert_eq!(right_runs[0].snapshot_hash_b.as_deref(), Some("hash-right"));
    }

    #[test]
    fn latest_ai_artifact_for_anchor_day_returns_none_when_day_has_no_snapshot_artifact() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));

        assert_eq!(
            store
                .analysis()
                .latest_ai_artifact_for_anchor_day("2026-04-10")
                .unwrap_or_else(|error| unreachable!(
                    "ai artifact day summary should load: {error}"
                )),
            None
        );
    }

    #[test]
    fn latest_ai_artifact_for_anchor_day_prefers_newest_review_for_matching_day() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));

        store
            .analysis()
            .upsert_snapshot_export(&make_snapshot_export("hash-older", "2026-04-10"), &[])
            .unwrap_or_else(|error| unreachable!("older snapshot should persist: {error}"));
        store
            .analysis()
            .upsert_snapshot_export(&make_snapshot_export("hash-newer", "2026-04-10"), &[])
            .unwrap_or_else(|error| unreachable!("newer snapshot should persist: {error}"));
        store
            .analysis()
            .upsert_ai_artifact(&make_ai_artifact(
                "artifact-older",
                "review",
                "2026-04-10T00:05:00Z",
                "hash-older",
                None,
            ))
            .unwrap_or_else(|error| unreachable!("older artifact should persist: {error}"));
        store
            .analysis()
            .upsert_ai_artifact(&make_ai_artifact(
                "artifact-newer",
                "review",
                "2026-04-10T00:06:00Z",
                "hash-newer",
                None,
            ))
            .unwrap_or_else(|error| unreachable!("newer artifact should persist: {error}"));

        let loaded = store
            .analysis()
            .latest_ai_artifact_for_anchor_day("2026-04-10")
            .unwrap_or_else(|error| unreachable!("ai artifact day summary should load: {error}"))
            .unwrap_or_else(|| unreachable!("ai artifact day summary should exist"));

        assert_eq!(
            loaded,
            AiArtifactDaySummaryRecord {
                artifact_id: "artifact-newer".to_owned(),
                artifact_kind: "review".to_owned(),
                created_at: "2026-04-10T00:06:00Z".to_owned(),
                provider: "dry_run".to_owned(),
                model: "deterministic".to_owned(),
                prompt_version: "review_prompt_v3".to_owned(),
                output_schema_version: "ringmaster.ai.review.v3".to_owned(),
                privacy_profile: "redacted".to_owned(),
                summary_cache: "review summary".to_owned(),
                overview: "review overview".to_owned(),
                matched_snapshot_hash: "hash-newer".to_owned(),
                peer_snapshot_hash: None,
            }
        );
    }

    #[test]
    fn latest_ai_artifact_for_anchor_day_matches_compare_runs_on_either_snapshot_side() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));

        store
            .analysis()
            .upsert_snapshot_export(&make_snapshot_export("hash-left", "2026-04-09"), &[])
            .unwrap_or_else(|error| unreachable!("left snapshot should persist: {error}"));
        store
            .analysis()
            .upsert_snapshot_export(&make_snapshot_export("hash-right", "2026-04-10"), &[])
            .unwrap_or_else(|error| unreachable!("right snapshot should persist: {error}"));
        store
            .analysis()
            .upsert_ai_artifact(&make_ai_artifact(
                "artifact-compare",
                "compare",
                "2026-04-10T00:08:00Z",
                "hash-left",
                Some("hash-right"),
            ))
            .unwrap_or_else(|error| unreachable!("compare artifact should persist: {error}"));

        let loaded = store
            .analysis()
            .latest_ai_artifact_for_anchor_day("2026-04-10")
            .unwrap_or_else(|error| unreachable!("ai artifact day summary should load: {error}"))
            .unwrap_or_else(|| unreachable!("compare artifact day summary should exist"));

        assert_eq!(loaded.artifact_kind, "compare");
        assert_eq!(loaded.matched_snapshot_hash, "hash-right");
        assert_eq!(loaded.peer_snapshot_hash.as_deref(), Some("hash-left"));

        let loaded_left = store
            .analysis()
            .latest_ai_artifact_for_anchor_day("2026-04-09")
            .unwrap_or_else(|error| unreachable!("ai artifact day summary should load: {error}"))
            .unwrap_or_else(|| unreachable!("compare artifact day summary should exist"));

        assert_eq!(loaded_left.matched_snapshot_hash, "hash-left");
        assert_eq!(
            loaded_left.peer_snapshot_hash.as_deref(),
            Some("hash-right")
        );
    }

    #[test]
    fn analysis_store_round_trips_report_exports_and_eval_runs() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));
        let snapshot = make_snapshot_export("hash-123", "2026-04-10");
        let ai_artifact = make_ai_artifact(
            "artifact-123",
            "review",
            "2026-04-10T00:05:00Z",
            "hash-123",
            None,
        );
        store
            .analysis()
            .upsert_snapshot_export(&snapshot, &[])
            .unwrap_or_else(|error| unreachable!("snapshot export should persist: {error}"));
        store
            .analysis()
            .upsert_ai_artifact(&ai_artifact)
            .unwrap_or_else(|error| unreachable!("ai artifact should persist: {error}"));

        let report = make_report_export("report-123", "hash-123", "artifact-123");
        store
            .analysis()
            .upsert_report_export(&report)
            .unwrap_or_else(|error| unreachable!("report export should persist: {error}"));

        let eval = make_ai_eval_run("eval-123");
        store
            .analysis()
            .upsert_ai_eval_run(&eval)
            .unwrap_or_else(|error| unreachable!("ai eval run should persist: {error}"));

        let loaded_reports = store
            .analysis()
            .report_exports_for_snapshot("hash-123")
            .unwrap_or_else(|error| unreachable!("report exports should load: {error}"));
        assert_eq!(loaded_reports.len(), 1);
        assert_eq!(loaded_reports[0].report_id, "report-123");
    }

    #[test]
    fn rest_mode_periods_between_days_include_open_periods_started_before_window() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));
        store
            .imports()
            .upsert_rest_mode_period(&RestModePeriodRecord {
                period_id: "rest-open".to_owned(),
                start_day: "2026-04-01".to_owned(),
                start_time: Some("2026-04-01T00:00:00Z".to_owned()),
                end_day: None,
                end_time: None,
                episode_count: 1,
                tags_json: "[]".to_owned(),
                raw_cache_key: None,
                updated_at: "2026-04-01T00:00:00Z".to_owned(),
            })
            .unwrap_or_else(|error| unreachable!("rest mode period should seed: {error}"));

        let periods = store
            .views()
            .rest_mode_periods_between_days("2026-04-03", "2026-04-04")
            .unwrap_or_else(|error| unreachable!("rest mode periods should load: {error}"));

        assert_eq!(periods.len(), 1);
        assert_eq!(periods[0].period_id, "rest-open");
    }

    #[test]
    fn vo2_max_queries_preserve_multiple_measurements_per_day() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));

        for (oura_id, recorded_at, vo2_max) in [
            ("vo2-1", "2026-04-08T08:00:00Z", 41.5),
            ("vo2-2", "2026-04-08T12:00:00Z", 42.0),
        ] {
            store
                .imports()
                .upsert_vo2_max(&Vo2MaxRecord {
                    oura_id: Some(oura_id.to_owned()),
                    day: "2026-04-08".to_owned(),
                    recorded_at: recorded_at.to_owned(),
                    vo2_max: Some(vo2_max),
                    raw_cache_key: None,
                    updated_at: recorded_at.to_owned(),
                })
                .unwrap_or_else(|error| unreachable!("vo2 max row should seed: {error}"));
        }

        let records = store
            .views()
            .vo2_max_between_days("2026-04-08", "2026-04-08")
            .unwrap_or_else(|error| unreachable!("vo2 max rows should load: {error}"));

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].recorded_at, "2026-04-08T08:00:00Z");
        assert_eq!(records[1].recorded_at, "2026-04-08T12:00:00Z");
    }

    #[test]
    fn context_events_for_day_respects_offset_timestamps() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));
        store
            .derived()
            .replace_context_events(&[ContextEventRecord {
                context_event_id: "workout:late-offset".to_owned(),
                family: ContextEventFamily::Workout,
                source_id: "late-offset".to_owned(),
                anchor_day: "2026-04-08".to_owned(),
                start_at: "2026-04-08T23:30:00-07:00".to_owned(),
                end_at: Some("2026-04-08T23:45:00-07:00".to_owned()),
                time_semantics: TimeSemantics::Interval,
                title: "Late workout".to_owned(),
                subtype: Some("running".to_owned()),
                notes: None,
                intensity: Some("moderate".to_owned()),
                metadata_json: "{}".to_owned(),
                updated_at: "2026-04-09T07:00:00Z".to_owned(),
            }])
            .unwrap_or_else(|error| unreachable!("context event should seed: {error}"));

        let events = store
            .views()
            .context_events_for_day("2026-04-08")
            .unwrap_or_else(|error| unreachable!("context events should load: {error}"));

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].context_event_id, "workout:late-offset");
    }

    #[test]
    fn daily_delete_accepts_object_id_suffix_for_legacy_rows() {
        let store = Store::open_test_store()
            .unwrap_or_else(|error| unreachable!("store should open: {error}"));
        store
            .imports()
            .upsert_daily_sleep(&DailySleepRecord {
                oura_id: None,
                day: "2026-04-08".to_owned(),
                sleep_score: Some(88),
                sleep_duration_seconds: Some(28_200),
                raw_cache_key: None,
                updated_at: "2026-04-08T00:00:00Z".to_owned(),
            })
            .unwrap_or_else(|error| unreachable!("legacy daily sleep row should seed: {error}"));

        store
            .imports()
            .delete_daily_sleep("daily_sleep_2026-04-08")
            .unwrap_or_else(|error| {
                unreachable!("legacy delete should resolve by day suffix: {error}")
            });

        assert_eq!(
            store
                .views()
                .record_counts()
                .unwrap_or_else(|error| unreachable!("counts should load: {error}"))
                .daily_sleep,
            0
        );
    }
}
