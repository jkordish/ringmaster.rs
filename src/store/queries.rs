use std::fmt::{Display, Formatter};

use rusqlite::{Connection, OptionalExtension, params};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::error::{Result, RingmasterError};
use crate::store::migrations;

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
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecordCounts {
    pub raw_payloads: u64,
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
            .unwrap_or(migrations::current_version());

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
                granted_scopes
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(sync_key) DO UPDATE SET
                status = excluded.status,
                cursor = excluded.cursor,
                last_attempted_at = excluded.last_attempted_at,
                last_completed_at = excluded.last_completed_at,
                message = excluded.message,
                granted_scopes = excluded.granted_scopes",
            params![
                record.sync_key,
                record.status.as_str(),
                record.cursor,
                record.last_attempted_at,
                record.last_completed_at,
                record.message,
                join_scopes(&record.granted_scopes)
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
                    granted_scopes
                 FROM sync_state
                 ORDER BY last_attempted_at DESC
                 LIMIT 1",
                [],
                |row| {
                    Ok(SyncStateRecord {
                        sync_key: row.get(0)?,
                        status: SyncRunStatus::parse(&row.get::<_, String>(1)?),
                        cursor: row.get(2)?,
                        last_attempted_at: row.get(3)?,
                        last_completed_at: row.get(4)?,
                        message: row.get(5)?,
                        granted_scopes: split_scopes(&row.get::<_, String>(6)?),
                    })
                },
            )
            .optional()
            .map_err(Into::into)
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
                r#"
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
                "#,
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

    pub fn record_counts(&self) -> Result<RecordCounts> {
        Ok(RecordCounts {
            raw_payloads: row_count(self.connection, "raw_payload_cache")?,
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

fn parse_optional_score(value: Option<i64>) -> Option<u8> {
    value.and_then(|score| u8::try_from(score).ok())
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

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| RingmasterError::Config(format!("formatting timestamp failed: {error}")))
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use rusqlite::Connection;

    use crate::store::migrations;
    use crate::store::queries::{
        MetadataStore, SyncRunStatus, SyncStateRecord, SyncStateStore, ViewStore,
    };

    #[test]
    fn round_trips_sync_state() {
        let mut connection = Connection::open_in_memory()
            .unwrap_or_else(|error| panic!("in-memory db should open: {error}"));
        migrations::run_migrations(&mut connection)
            .unwrap_or_else(|error| panic!("migrations should succeed: {error}"));
        let sync_store = SyncStateStore::new(&connection);

        let record = SyncStateRecord {
            sync_key: "oura_poll".to_owned(),
            status: SyncRunStatus::Blocked,
            cursor: None,
            last_attempted_at: "2026-04-08T12:00:00Z".to_owned(),
            last_completed_at: None,
            message: Some("auth required".to_owned()),
            granted_scopes: vec!["daily".to_owned()],
        };

        sync_store
            .upsert(&record)
            .unwrap_or_else(|error| panic!("sync state should persist: {error}"));

        let fetched = sync_store
            .latest()
            .unwrap_or_else(|error| panic!("latest sync state should load: {error}"))
            .unwrap_or_else(|| panic!("latest sync state should exist"));

        assert_eq!(fetched, record);
    }

    #[test]
    fn reports_schema_version_from_metadata_store() {
        let mut connection = Connection::open_in_memory()
            .unwrap_or_else(|error| panic!("in-memory db should open: {error}"));
        migrations::run_migrations(&mut connection)
            .unwrap_or_else(|error| panic!("migrations should succeed: {error}"));
        let metadata = MetadataStore::new(&connection);
        let views = ViewStore::new(&connection);

        assert_eq!(
            metadata.schema_version().unwrap_or_default(),
            migrations::current_version()
        );
        assert_eq!(views.record_counts().unwrap_or_default().raw_payloads, 0);
    }
}
