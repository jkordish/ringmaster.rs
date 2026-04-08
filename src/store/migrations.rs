use rusqlite::params;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    pub applied_versions: Vec<u32>,
    pub current_version: u32,
}

pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "bootstrap_schema",
    sql: r#"
        CREATE TABLE IF NOT EXISTS app_metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sync_state (
            sync_key TEXT PRIMARY KEY,
            status TEXT NOT NULL,
            cursor TEXT,
            last_attempted_at TEXT NOT NULL,
            last_completed_at TEXT,
            message TEXT,
            granted_scopes TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS raw_payload_cache (
            cache_key TEXT PRIMARY KEY,
            endpoint TEXT NOT NULL,
            requested_at TEXT NOT NULL,
            scope TEXT,
            etag TEXT,
            payload TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS daily_sleep (
            day TEXT PRIMARY KEY,
            sleep_score INTEGER,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS daily_readiness (
            day TEXT PRIMARY KEY,
            readiness_score INTEGER,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS daily_activity (
            day TEXT PRIMARY KEY,
            activity_score INTEGER,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS heartrate_samples (
            recorded_at TEXT PRIMARY KEY,
            bpm INTEGER NOT NULL,
            source_day TEXT,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS workouts (
            workout_id TEXT PRIMARY KEY,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            sport TEXT,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tags (
            tag_id TEXT PRIMARY KEY,
            day TEXT NOT NULL,
            label TEXT NOT NULL,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS enhanced_tags (
            enhanced_tag_id TEXT PRIMARY KEY,
            day TEXT NOT NULL,
            label TEXT NOT NULL,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            kind TEXT,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS webhook_subscriptions (
            subscription_id TEXT PRIMARY KEY,
            event_type TEXT NOT NULL,
            delivery_mode TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_heartrate_samples_source_day
            ON heartrate_samples(source_day);

        CREATE INDEX IF NOT EXISTS idx_workouts_started_at
            ON workouts(started_at);

        CREATE INDEX IF NOT EXISTS idx_sessions_started_at
            ON sessions(started_at);

        CREATE INDEX IF NOT EXISTS idx_tags_day
            ON tags(day);

        CREATE INDEX IF NOT EXISTS idx_enhanced_tags_day
            ON enhanced_tags(day);
    "#,
}];

pub fn run_migrations(connection: &mut rusqlite::Connection) -> Result<MigrationReport> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL
        );",
    )?;

    let applied_versions = read_applied_versions(connection)?;
    let applied_set: std::collections::BTreeSet<u32> = applied_versions.into_iter().collect();
    let mut applied_now = Vec::new();

    for migration in MIGRATIONS {
        if applied_set.contains(&migration.version) {
            continue;
        }

        let applied_at = now_rfc3339()?;
        let transaction = connection.transaction()?;
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
            params![migration.version, migration.name, applied_at],
        )?;
        transaction.commit()?;
        applied_now.push(migration.version);
    }

    Ok(MigrationReport {
        applied_versions: applied_now,
        current_version: current_version(),
    })
}

pub fn current_version() -> u32 {
    MIGRATIONS
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or_default()
}

fn read_applied_versions(connection: &rusqlite::Connection) -> Result<Vec<u32>> {
    let mut statement =
        connection.prepare("SELECT version FROM schema_migrations ORDER BY version ASC")?;
    let rows = statement.query_map([], |row| row.get::<_, u32>(0))?;
    let mut versions = Vec::new();

    for row in rows {
        versions.push(row?);
    }

    Ok(versions)
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc().format(&Rfc3339).map_err(|error| {
        crate::error::RingmasterError::Config(format!("formatting timestamp failed: {error}"))
    })
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use rusqlite::Connection;

    use super::{current_version, run_migrations};

    #[test]
    fn applies_bootstrap_schema() {
        let mut connection = Connection::open_in_memory()
            .unwrap_or_else(|error| panic!("in-memory db should open: {error}"));
        let report = run_migrations(&mut connection)
            .unwrap_or_else(|error| panic!("migrations should succeed: {error}"));

        assert_eq!(report.current_version, current_version());
        assert_eq!(report.applied_versions, vec![1]);
    }
}
