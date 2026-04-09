use std::fmt::{Display, Formatter};

use rusqlite::{Connection, OptionalExtension, params};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::error::{OuraProblem, Result, RingmasterError};
use crate::oura::models::{TagRecord, TagSource};
use crate::review::features::ReviewSufficiency;
use crate::store::migrations;

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
    pub status: SyncRunStatus,
    pub cursor: Option<String>,
    pub last_attempted_at: String,
    pub last_completed_at: Option<String>,
    pub message: Option<String>,
    pub granted_scopes: Vec<String>,
    pub last_error: Option<OuraProblem>,
    pub failure_count: u32,
    pub next_attempt_after: Option<String>,
    pub last_trigger_source: Option<String>,
    pub last_trigger_detail: Option<String>,
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
    ActivityScore,
    ReadinessScore,
    SleepScore,
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

#[derive(Debug, Clone, PartialEq)]
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
    pub daily_readiness: u64,
    pub daily_activity: u64,
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

pub struct ViewStore<'connection> {
    connection: &'connection Connection,
}

impl Display for SyncRunStatus {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl SyncRunStatus {
    pub fn as_str(&self) -> &'static str {
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
            "failed" => Self::Failed,
            _ => Self::Failed,
        }
    }
}

impl ContextEventFamily {
    pub fn as_str(self) -> &'static str {
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
    pub fn as_str(self) -> &'static str {
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
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ActivityScore => "activity_score",
            Self::ReadinessScore => "next_day_readiness",
            Self::SleepScore => "same_night_sleep",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ActivityScore => "Activity",
            Self::ReadinessScore => "Next-day readiness",
            Self::SleepScore => "Same-night sleep",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "activity_score" => Some(Self::ActivityScore),
            "next_day_readiness" => Some(Self::ReadinessScore),
            "same_night_sleep" => Some(Self::SleepScore),
            _ => None,
        }
    }
}

impl PatternRelationWindow {
    pub fn as_str(self) -> &'static str {
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
    pub fn as_str(self) -> &'static str {
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
    pub fn as_str(self) -> &'static str {
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
    pub fn new(connection: &'connection Connection) -> Self {
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

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        self.connection
            .query_row(
                "SELECT value FROM app_metadata WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
    }
}

impl<'connection> SyncStateStore<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn upsert(&self, record: &SyncStateRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO sync_state (
                sync_key,
                status,
                cursor,
                last_attempted_at,
                last_completed_at,
                message,
                granted_scopes,
                last_error_json,
                failure_count,
                next_attempt_after,
                last_trigger_source,
                last_trigger_detail
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ON CONFLICT(sync_key) DO UPDATE SET
                status = excluded.status,
                cursor = excluded.cursor,
                last_attempted_at = excluded.last_attempted_at,
                last_completed_at = excluded.last_completed_at,
                message = excluded.message,
                granted_scopes = excluded.granted_scopes,
                last_error_json = excluded.last_error_json,
                failure_count = excluded.failure_count,
                next_attempt_after = excluded.next_attempt_after,
                last_trigger_source = excluded.last_trigger_source,
                last_trigger_detail = excluded.last_trigger_detail",
            params![
                record.sync_key,
                record.status.as_str(),
                record.cursor,
                record.last_attempted_at,
                record.last_completed_at,
                record.message,
                join_scopes(&record.granted_scopes),
                encode_problem(&record.last_error)?,
                i64::from(record.failure_count),
                record.next_attempt_after,
                record.last_trigger_source,
                record.last_trigger_detail,
            ],
        )?;

        Ok(())
    }

    pub fn latest(&self) -> Result<Option<SyncStateRecord>> {
        self.connection
            .query_row(
                "SELECT
                    sync_key,
                    status,
                    cursor,
                    last_attempted_at,
                    last_completed_at,
                    message,
                    granted_scopes,
                    last_error_json,
                    failure_count,
                    next_attempt_after,
                    last_trigger_source,
                    last_trigger_detail
                 FROM sync_state
                 ORDER BY last_attempted_at DESC
                 LIMIT 1",
                [],
                read_sync_state_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list(&self) -> Result<Vec<SyncStateRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                sync_key,
                status,
                cursor,
                last_attempted_at,
                last_completed_at,
                message,
                granted_scopes,
                last_error_json,
                failure_count,
                next_attempt_after,
                last_trigger_source,
                last_trigger_detail
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
                    status,
                    cursor,
                    last_attempted_at,
                    last_completed_at,
                    message,
                    granted_scopes,
                    last_error_json,
                    failure_count,
                    next_attempt_after,
                    last_trigger_source,
                    last_trigger_detail
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
    pub fn new(connection: &'connection Connection) -> Self {
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
                        last_error: decode_problem(row.get(8)?).map_err(json_to_sql_error)?,
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
                encode_problem(&record.last_error)?,
                record.updated_at,
            ],
        )?;

        Ok(())
    }
}

impl<'connection> ImportStore<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
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
            "INSERT INTO daily_sleep (oura_id, day, sleep_score, raw_cache_key, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(day) DO UPDATE SET
                oura_id = excluded.oura_id,
                sleep_score = excluded.sleep_score,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.oura_id,
                record.day,
                record.sleep_score.map(i64::from),
                record.raw_cache_key,
                record.updated_at,
            ],
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
    pub fn new(connection: &'connection Connection) -> Self {
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

impl<'connection> ViewStore<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn latest_daily_overview(&self) -> Result<Option<DailyOverviewRow>> {
        let row = self
            .connection
            .query_row(
                r"
                WITH latest_day AS (
                    SELECT MAX(day) AS day FROM (
                        SELECT day FROM daily_sleep
                        UNION ALL
                        SELECT day FROM daily_readiness
                        UNION ALL
                        SELECT day FROM daily_activity
                    )
                )
                SELECT
                    latest_day.day,
                    (SELECT sleep_score FROM daily_sleep WHERE day = latest_day.day),
                    (SELECT readiness_score FROM daily_readiness WHERE day = latest_day.day),
                    (SELECT activity_score FROM daily_activity WHERE day = latest_day.day),
                    COALESCE(
                        (SELECT updated_at FROM daily_sleep WHERE day = latest_day.day),
                        (SELECT updated_at FROM daily_readiness WHERE day = latest_day.day),
                        (SELECT updated_at FROM daily_activity WHERE day = latest_day.day)
                    )
                FROM latest_day
                ",
                [],
                |row| {
                    let day = row.get::<_, Option<String>>(0)?;
                    match day {
                        Some(day) => Ok(Some(DailyOverviewRow {
                            day,
                            sleep_score: parse_optional_score(row.get::<_, Option<i64>>(1)?),
                            readiness_score: parse_optional_score(row.get::<_, Option<i64>>(2)?),
                            activity_score: parse_optional_score(row.get::<_, Option<i64>>(3)?),
                            updated_at: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                        })),
                        None => Ok(None),
                    }
                },
            )
            .optional()?;

        Ok(row.flatten())
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
        let rows = statement.query_map(params![bounded_limit as i64], |row| {
            Ok(DailyOverviewRow {
                day: row.get(0)?,
                sleep_score: parse_optional_score(row.get::<_, Option<i64>>(1)?),
                readiness_score: parse_optional_score(row.get::<_, Option<i64>>(2)?),
                activity_score: parse_optional_score(row.get::<_, Option<i64>>(3)?),
                updated_at: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            })
        })?;

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
                readiness_score: parse_optional_score(row.get::<_, Option<i64>>(2)?),
                activity_score: parse_optional_score(row.get::<_, Option<i64>>(3)?),
                updated_at: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
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
                readiness_score: parse_optional_score(row.get::<_, Option<i64>>(2)?),
                activity_score: parse_optional_score(row.get::<_, Option<i64>>(3)?),
                updated_at: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            })
        })?;

        let mut history = Vec::new();
        for row in rows {
            history.push(row?);
        }

        Ok(history)
    }

    pub fn latest_source_day(&self) -> Result<Option<String>> {
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
                        WHEN end_day IS NULL THEN MAX(start_day, DATE('now'))
                        ELSE end_day
                    END AS day
                    FROM rest_mode_periods
                )",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .map_err(Into::into)
    }

    pub fn latest_review_day(&self) -> Result<Option<String>> {
        self.connection
            .query_row(
                "SELECT MAX(day) FROM (
                    SELECT day FROM derived_review_signal_days
                    UNION ALL
                    SELECT day FROM sleep_time
                    UNION ALL
                    SELECT CASE
                        WHEN end_day IS NULL THEN MAX(start_day, DATE('now'))
                        ELSE end_day
                    END AS day
                    FROM rest_mode_periods
                )",
                [],
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

    pub fn recent_heartrate(&self, limit: usize) -> Result<Vec<HeartRatePoint>> {
        let bounded_limit = usize::min(limit, 240);
        let mut statement = self.connection.prepare(
            "SELECT recorded_at, bpm, source_day
             FROM heartrate_samples
             ORDER BY recorded_at DESC
             LIMIT ?1",
        )?;
        let rows = statement.query_map(params![bounded_limit as i64], |row| {
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
        })?;

        let mut points = Vec::new();
        for row in rows {
            points.push(row?);
        }
        points.reverse();

        Ok(points)
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
        let rows = statement.query_map(params![bounded_limit as i64], |row| {
            row.get::<_, Option<String>>(0)
        })?;

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
            daily_readiness: row_count(self.connection, "daily_readiness")?,
            daily_activity: row_count(self.connection, "daily_activity")?,
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
    Ok(SyncStateRecord {
        sync_key: row.get(0)?,
        status: SyncRunStatus::parse(&row.get::<_, String>(1)?),
        cursor: row.get(2)?,
        last_attempted_at: row.get(3)?,
        last_completed_at: row.get(4)?,
        message: row.get(5)?,
        granted_scopes: split_scopes(&row.get::<_, String>(6)?),
        last_error: decode_problem(row.get(7)?).map_err(json_to_sql_error)?,
        failure_count: parse_u32(row.get::<_, i64>(8)?, 8)?,
        next_attempt_after: row.get(9)?,
        last_trigger_source: row.get(10)?,
        last_trigger_detail: row.get(11)?,
    })
}

fn parse_optional_score(value: Option<i64>) -> Option<u8> {
    value.and_then(|score| u8::try_from(score).ok())
}

fn parse_optional_u16(value: Option<i64>) -> rusqlite::Result<Option<u16>> {
    match value {
        Some(value) => u16::try_from(value).map(Some).map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Integer,
                Box::new(std::fmt::Error),
            )
        }),
        None => Ok(None),
    }
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

fn encode_problem(problem: &Option<OuraProblem>) -> Result<Option<String>> {
    problem
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(Into::into)
}

fn decode_problem(
    value: Option<String>,
) -> std::result::Result<Option<OuraProblem>, serde_json::Error> {
    value
        .as_deref()
        .map(serde_json::from_str::<OuraProblem>)
        .transpose()
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
#[allow(clippy::panic)]
mod tests {
    use time::OffsetDateTime;

    use crate::error::OuraProblem;
    use crate::review::features::ReviewSufficiency;
    use crate::store::Store;
    use crate::store::queries::{
        ContextEventFamily, ContextEventRecord, DailyActivityRecord, DailyReadinessRecord,
        DailySleepRecord, HeartrateSampleRecord, RestModePeriodRecord, ReviewSignalDayRecord,
        SyncRunStatus, SyncStateRecord, TimeSemantics, Vo2MaxRecord,
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
                    updated_at: format!("{day}T06:05:00Z"),
                })
                .unwrap_or_else(|error| panic!("readiness row should seed: {error}"));
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
                .unwrap_or_else(|error| panic!("activity row should seed: {error}"));
        }
    }

    #[test]
    fn sync_state_round_trips_backoff_metadata() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));

        store
            .sync_state()
            .upsert(&SyncStateRecord {
                sync_key: "oura.daily".to_owned(),
                status: SyncRunStatus::Failed,
                cursor: Some("2026-04-08".to_owned()),
                last_attempted_at: "2026-04-08T06:00:00Z".to_owned(),
                last_completed_at: Some("2026-04-08T06:00:05Z".to_owned()),
                message: Some("rate limited".to_owned()),
                granted_scopes: vec!["daily".to_owned()],
                last_error: Some(OuraProblem::new(
                    Some(429),
                    "rate limited",
                    Some("retry later".to_owned()),
                )),
                failure_count: 3,
                next_attempt_after: Some("2026-04-08T06:05:00Z".to_owned()),
                last_trigger_source: Some("periodic_reconcile".to_owned()),
                last_trigger_detail: Some("daily scheduler".to_owned()),
            })
            .unwrap_or_else(|error| panic!("sync state should persist: {error}"));

        let record = store
            .sync_state()
            .get("oura.daily")
            .unwrap_or_else(|error| panic!("sync state should read: {error}"))
            .unwrap_or_else(|| panic!("sync state should exist"));

        assert_eq!(record.failure_count, 3);
        assert_eq!(
            record.next_attempt_after.as_deref(),
            Some("2026-04-08T06:05:00Z")
        );
        assert_eq!(record.status, SyncRunStatus::Failed);
    }

    #[test]
    fn daily_history_returns_oldest_to_newest_rows() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        seed_daily_history(&store);

        let history = store
            .views()
            .daily_history(30)
            .unwrap_or_else(|error| panic!("daily history should load: {error}"));

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
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));

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
                .unwrap_or_else(|error| panic!("heartrate sample should seed: {error}"));
        }

        let days = store
            .views()
            .available_heartrate_days(10)
            .unwrap_or_else(|error| panic!("heartrate days should load: {error}"));
        let points = store
            .views()
            .heartrate_for_day("2026-04-07")
            .unwrap_or_else(|error| panic!("heartrate day should load: {error}"));

        assert_eq!(days, vec!["2026-04-07".to_owned(), "2026-04-08".to_owned()]);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].bpm, 58);
        assert_eq!(points[1].bpm, 60);
    }

    #[test]
    fn latest_source_day_tracks_newest_persisted_family_day() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
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
            .unwrap_or_else(|error| panic!("rest mode period should seed: {error}"));

        assert_eq!(
            store
                .views()
                .latest_source_day()
                .unwrap_or_else(|error| panic!("latest day should load: {error}"))
                .as_deref(),
            Some("2026-04-09")
        );
    }

    #[test]
    fn latest_source_day_treats_open_rest_mode_as_current() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let current_day = OffsetDateTime::now_utc().date().to_string();
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
            .unwrap_or_else(|error| panic!("rest mode period should seed: {error}"));

        assert_eq!(
            store
                .views()
                .latest_source_day()
                .unwrap_or_else(|error| panic!("latest day should load: {error}"))
                .as_deref(),
            Some(current_day.as_str())
        );
    }

    #[test]
    fn latest_review_day_prefers_reviewable_sources() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));

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
            .unwrap_or_else(|error| panic!("review signal day should seed: {error}"));
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
            .unwrap_or_else(|error| panic!("rest mode period should seed: {error}"));

        assert_eq!(
            store
                .views()
                .latest_review_day()
                .unwrap_or_else(|error| panic!("latest review day should load: {error}"))
                .as_deref(),
            Some("2026-04-10")
        );
    }

    #[test]
    fn latest_review_day_treats_open_rest_mode_as_current() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let current_day = OffsetDateTime::now_utc().date().to_string();
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
            .unwrap_or_else(|error| panic!("rest mode period should seed: {error}"));

        assert_eq!(
            store
                .views()
                .latest_review_day()
                .unwrap_or_else(|error| panic!("latest review day should load: {error}"))
                .as_deref(),
            Some(current_day.as_str())
        );
    }

    #[test]
    fn rest_mode_periods_between_days_include_open_periods_started_before_window() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
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
            .unwrap_or_else(|error| panic!("rest mode period should seed: {error}"));

        let periods = store
            .views()
            .rest_mode_periods_between_days("2026-04-03", "2026-04-04")
            .unwrap_or_else(|error| panic!("rest mode periods should load: {error}"));

        assert_eq!(periods.len(), 1);
        assert_eq!(periods[0].period_id, "rest-open");
    }

    #[test]
    fn vo2_max_queries_preserve_multiple_measurements_per_day() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));

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
                .unwrap_or_else(|error| panic!("vo2 max row should seed: {error}"));
        }

        let records = store
            .views()
            .vo2_max_between_days("2026-04-08", "2026-04-08")
            .unwrap_or_else(|error| panic!("vo2 max rows should load: {error}"));

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].recorded_at, "2026-04-08T08:00:00Z");
        assert_eq!(records[1].recorded_at, "2026-04-08T12:00:00Z");
    }

    #[test]
    fn context_events_for_day_respects_offset_timestamps() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
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
            .unwrap_or_else(|error| panic!("context event should seed: {error}"));

        let events = store
            .views()
            .context_events_for_day("2026-04-08")
            .unwrap_or_else(|error| panic!("context events should load: {error}"));

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].context_event_id, "workout:late-offset");
    }

    #[test]
    fn daily_delete_accepts_object_id_suffix_for_legacy_rows() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        store
            .imports()
            .upsert_daily_sleep(&DailySleepRecord {
                oura_id: None,
                day: "2026-04-08".to_owned(),
                sleep_score: Some(88),
                raw_cache_key: None,
                updated_at: "2026-04-08T00:00:00Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("legacy daily sleep row should seed: {error}"));

        store
            .imports()
            .delete_daily_sleep("daily_sleep_2026-04-08")
            .unwrap_or_else(|error| panic!("legacy delete should resolve by day suffix: {error}"));

        assert_eq!(
            store
                .views()
                .record_counts()
                .unwrap_or_else(|error| panic!("counts should load: {error}"))
                .daily_sleep,
            0
        );
    }
}
