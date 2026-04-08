use std::fmt::{Display, Formatter};

use rusqlite::{Connection, OptionalExtension, params};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::error::{OuraProblem, Result, RingmasterError};
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
    pub day: String,
    pub sleep_score: Option<u8>,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DailyReadinessRecord {
    pub day: String,
    pub readiness_score: Option<u8>,
    pub temperature_deviation: Option<f64>,
    pub temperature_trend_deviation: Option<f64>,
    pub raw_cache_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyActivityRecord {
    pub day: String,
    pub activity_score: Option<u8>,
    pub active_calories: i64,
    pub steps: i64,
    pub total_calories: i64,
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
    pub heartrate_samples: u64,
    pub workouts: u64,
    pub tags: u64,
    pub enhanced_tags: u64,
    pub sessions: u64,
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
                next_attempt_after
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(sync_key) DO UPDATE SET
                status = excluded.status,
                cursor = excluded.cursor,
                last_attempted_at = excluded.last_attempted_at,
                last_completed_at = excluded.last_completed_at,
                message = excluded.message,
                granted_scopes = excluded.granted_scopes,
                last_error_json = excluded.last_error_json,
                failure_count = excluded.failure_count,
                next_attempt_after = excluded.next_attempt_after",
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
                    next_attempt_after
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
                next_attempt_after
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
                    next_attempt_after
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
            "INSERT INTO daily_sleep (day, sleep_score, raw_cache_key, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(day) DO UPDATE SET
                sleep_score = excluded.sleep_score,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
                record.day,
                record.sleep_score.map(i64::from),
                record.raw_cache_key,
                record.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn upsert_daily_readiness(&self, record: &DailyReadinessRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO daily_readiness (
                day,
                readiness_score,
                temperature_deviation,
                temperature_trend_deviation,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(day) DO UPDATE SET
                readiness_score = excluded.readiness_score,
                temperature_deviation = excluded.temperature_deviation,
                temperature_trend_deviation = excluded.temperature_trend_deviation,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
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

    pub fn upsert_daily_activity(&self, record: &DailyActivityRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO daily_activity (
                day,
                activity_score,
                active_calories,
                steps,
                total_calories,
                raw_cache_key,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(day) DO UPDATE SET
                activity_score = excluded.activity_score,
                active_calories = excluded.active_calories,
                steps = excluded.steps,
                total_calories = excluded.total_calories,
                raw_cache_key = excluded.raw_cache_key,
                updated_at = excluded.updated_at",
            params![
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

    pub fn record_counts(&self) -> Result<RecordCounts> {
        Ok(RecordCounts {
            raw_payloads: row_count(self.connection, "raw_payload_cache")?,
            personal_info: row_count(self.connection, "personal_info")?,
            daily_sleep: row_count(self.connection, "daily_sleep")?,
            daily_readiness: row_count(self.connection, "daily_readiness")?,
            daily_activity: row_count(self.connection, "daily_activity")?,
            heartrate_samples: row_count(self.connection, "heartrate_samples")?,
            workouts: row_count(self.connection, "workouts")?,
            tags: row_count(self.connection, "tags")?,
            enhanced_tags: row_count(self.connection, "enhanced_tags")?,
            sessions: row_count(self.connection, "sessions")?,
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

fn row_count(connection: &Connection, table: &str) -> Result<u64> {
    let query = format!("SELECT COUNT(*) FROM {table}");
    let count = connection.query_row(&query, [], |row| row.get::<_, i64>(0))?;
    u64::try_from(count).map_err(|error| {
        RingmasterError::Config(format!(
            "negative row count for `{table}` is invalid: {error}"
        ))
    })
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
    use crate::error::OuraProblem;
    use crate::store::Store;
    use crate::store::queries::{
        DailyActivityRecord, DailyReadinessRecord, DailySleepRecord, HeartrateSampleRecord,
        SyncRunStatus, SyncStateRecord,
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
                    day: day.to_owned(),
                    sleep_score: Some(sleep),
                    raw_cache_key: None,
                    updated_at: format!("{day}T06:00:00Z"),
                })
                .unwrap_or_else(|error| panic!("sleep row should seed: {error}"));
            store
                .imports()
                .upsert_daily_readiness(&DailyReadinessRecord {
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
}
