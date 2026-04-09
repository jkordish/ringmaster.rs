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

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "bootstrap_schema",
        sql: r"
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
    ",
    },
    Migration {
        version: 2,
        name: "phase1_foundation",
        sql: r"
        CREATE TABLE IF NOT EXISTS auth_session (
            provider TEXT PRIMARY KEY,
            account_id TEXT,
            account_email TEXT,
            token_type TEXT NOT NULL,
            granted_scopes TEXT NOT NULL,
            access_token_expires_at TEXT,
            last_authenticated_at TEXT,
            last_refresh_at TEXT,
            last_error_json TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS personal_info (
            profile_id TEXT PRIMARY KEY,
            age INTEGER,
            weight REAL,
            height REAL,
            biological_sex TEXT,
            email TEXT,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL
        );

        ALTER TABLE sync_state ADD COLUMN last_error_json TEXT;
        ALTER TABLE daily_readiness ADD COLUMN temperature_deviation REAL;
        ALTER TABLE daily_readiness ADD COLUMN temperature_trend_deviation REAL;
        ALTER TABLE daily_activity ADD COLUMN active_calories INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE daily_activity ADD COLUMN steps INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE daily_activity ADD COLUMN total_calories INTEGER NOT NULL DEFAULT 0;

        CREATE INDEX IF NOT EXISTS idx_sync_state_attempted_at
            ON sync_state(last_attempted_at DESC);
        ",
    },
    Migration {
        version: 3,
        name: "phase1_sync_state_cleanup",
        sql: r"
        DELETE FROM sync_state
        WHERE sync_key NOT IN ('oura.personal', 'oura.daily', 'oura.heartrate');
        ",
    },
    Migration {
        version: 4,
        name: "phase2_refresh_metadata",
        sql: r"
        ALTER TABLE sync_state ADD COLUMN failure_count INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE sync_state ADD COLUMN next_attempt_after TEXT;
        ",
    },
    Migration {
        version: 5,
        name: "phase3_context_family_expansion",
        sql: r"
        ALTER TABLE workouts ADD COLUMN day TEXT;
        ALTER TABLE workouts ADD COLUMN timezone TEXT;
        ALTER TABLE workouts ADD COLUMN activity TEXT;
        ALTER TABLE workouts ADD COLUMN intensity TEXT;
        ALTER TABLE workouts ADD COLUMN title TEXT;
        ALTER TABLE workouts ADD COLUMN notes TEXT;
        ALTER TABLE workouts ADD COLUMN source TEXT;

        ALTER TABLE enhanced_tags ADD COLUMN started_at TEXT;
        ALTER TABLE enhanced_tags ADD COLUMN ended_at TEXT;
        ALTER TABLE enhanced_tags ADD COLUMN subtype TEXT;
        ALTER TABLE enhanced_tags ADD COLUMN comment TEXT;
        ALTER TABLE enhanced_tags ADD COLUMN intensity TEXT;

        ALTER TABLE sessions ADD COLUMN day TEXT;
        ALTER TABLE sessions ADD COLUMN state TEXT;
        ALTER TABLE sessions ADD COLUMN score INTEGER;
        ALTER TABLE sessions ADD COLUMN title TEXT;

        UPDATE workouts
        SET
            day = COALESCE(day, substr(started_at, 1, 10)),
            title = COALESCE(NULLIF(title, ''), NULLIF(sport, ''), 'Workout')
        WHERE day IS NULL
            OR title IS NULL
            OR title = '';

        UPDATE sessions
        SET
            day = COALESCE(day, substr(started_at, 1, 10)),
            title = COALESCE(NULLIF(title, ''), NULLIF(kind, ''), 'Session')
        WHERE day IS NULL
            OR title IS NULL
            OR title = '';

        CREATE INDEX IF NOT EXISTS idx_workouts_day
            ON workouts(day);
        CREATE INDEX IF NOT EXISTS idx_workouts_day_started_at
            ON workouts(day, started_at);
        CREATE INDEX IF NOT EXISTS idx_enhanced_tags_day_started_at
            ON enhanced_tags(day, started_at);
        CREATE INDEX IF NOT EXISTS idx_sessions_day
            ON sessions(day);
        CREATE INDEX IF NOT EXISTS idx_sessions_day_started_at
            ON sessions(day, started_at);
        ",
    },
    Migration {
        version: 6,
        name: "phase3_derived_context_and_patterns",
        sql: r"
        CREATE TABLE IF NOT EXISTS derived_context_events (
            context_event_id TEXT PRIMARY KEY,
            family TEXT NOT NULL,
            source_id TEXT NOT NULL,
            anchor_day TEXT NOT NULL,
            start_at TEXT NOT NULL,
            end_at TEXT,
            time_semantics TEXT NOT NULL,
            title TEXT NOT NULL,
            subtype TEXT,
            notes TEXT,
            intensity TEXT,
            metadata_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS derived_pattern_summaries (
            summary_id TEXT PRIMARY KEY,
            family TEXT NOT NULL,
            normalized_key TEXT NOT NULL,
            relation_window TEXT NOT NULL,
            metric TEXT NOT NULL,
            sample_count INTEGER NOT NULL,
            median_delta REAL NOT NULL,
            effect_direction TEXT NOT NULL,
            confidence TEXT NOT NULL,
            metadata_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_derived_context_events_day_family_start
            ON derived_context_events(anchor_day, family, start_at);
        CREATE INDEX IF NOT EXISTS idx_derived_context_events_family_subtype_day
            ON derived_context_events(family, subtype, anchor_day);
        CREATE INDEX IF NOT EXISTS idx_derived_context_events_start_end
            ON derived_context_events(start_at, end_at);

        CREATE UNIQUE INDEX IF NOT EXISTS idx_derived_pattern_unique
            ON derived_pattern_summaries(family, normalized_key, relation_window, metric);
        CREATE INDEX IF NOT EXISTS idx_derived_pattern_metric_confidence
            ON derived_pattern_summaries(metric, confidence, sample_count);
        ",
    },
    Migration {
        version: 7,
        name: "phase4_webhook_freshness_and_ops_foundation",
        sql: r"
        ALTER TABLE sync_state ADD COLUMN last_trigger_source TEXT;
        ALTER TABLE sync_state ADD COLUMN last_trigger_detail TEXT;

        ALTER TABLE webhook_subscriptions RENAME TO webhook_subscriptions_legacy;

        CREATE TABLE IF NOT EXISTS webhook_desired_subscriptions (
            data_type TEXT NOT NULL,
            event_type TEXT NOT NULL,
            enabled INTEGER NOT NULL,
            callback_url TEXT,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (data_type, event_type)
        );

        CREATE TABLE IF NOT EXISTS webhook_remote_subscriptions (
            subscription_id TEXT PRIMARY KEY,
            callback_url TEXT NOT NULL,
            event_type TEXT NOT NULL,
            data_type TEXT NOT NULL,
            expiration_time TEXT NOT NULL,
            drift_status TEXT NOT NULL,
            last_seen_at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        INSERT INTO webhook_remote_subscriptions (
            subscription_id,
            callback_url,
            event_type,
            data_type,
            expiration_time,
            drift_status,
            last_seen_at,
            created_at,
            updated_at
        )
        SELECT
            subscription_id,
            COALESCE(delivery_mode, ''),
            event_type,
            'unknown',
            COALESCE(updated_at, created_at),
            COALESCE(status, 'unknown'),
            COALESCE(updated_at, created_at),
            created_at,
            updated_at
        FROM webhook_subscriptions_legacy;

        DROP TABLE webhook_subscriptions_legacy;

        CREATE TABLE IF NOT EXISTS webhook_deliveries (
            delivery_id INTEGER PRIMARY KEY AUTOINCREMENT,
            delivery_fingerprint TEXT NOT NULL UNIQUE,
            received_at TEXT NOT NULL,
            signature_timestamp TEXT,
            data_type TEXT,
            event_type TEXT,
            object_id TEXT,
            payload_json TEXT NOT NULL,
            headers_json TEXT NOT NULL,
            query_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS webhook_delivery_rejections (
            rejection_id INTEGER PRIMARY KEY AUTOINCREMENT,
            received_at TEXT NOT NULL,
            reason_code TEXT NOT NULL,
            detail TEXT NOT NULL,
            signature_timestamp TEXT,
            payload_json TEXT NOT NULL,
            headers_json TEXT NOT NULL,
            query_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS webhook_invalidations (
            invalidation_id INTEGER PRIMARY KEY AUTOINCREMENT,
            queue_key TEXT NOT NULL UNIQUE,
            data_type TEXT NOT NULL,
            event_type TEXT NOT NULL,
            object_id TEXT,
            delivery_id INTEGER NOT NULL,
            first_queued_at TEXT NOT NULL,
            last_queued_at TEXT NOT NULL,
            available_at TEXT NOT NULL,
            leased_at TEXT,
            lease_owner TEXT,
            attempt_count INTEGER NOT NULL DEFAULT 0,
            last_error TEXT,
            FOREIGN KEY(delivery_id) REFERENCES webhook_deliveries(delivery_id)
        );

        CREATE TABLE IF NOT EXISTS webhook_processing_attempts (
            attempt_id INTEGER PRIMARY KEY AUTOINCREMENT,
            invalidation_id INTEGER NOT NULL,
            started_at TEXT NOT NULL,
            finished_at TEXT,
            outcome TEXT NOT NULL,
            detail TEXT,
            FOREIGN KEY(invalidation_id) REFERENCES webhook_invalidations(invalidation_id)
        );

        CREATE TABLE IF NOT EXISTS webhook_runtime_heartbeats (
            component TEXT PRIMARY KEY,
            mode TEXT NOT NULL,
            bind_address TEXT,
            public_base_url TEXT,
            detail TEXT,
            last_seen_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_webhook_remote_subscriptions_kind
            ON webhook_remote_subscriptions(data_type, event_type);
        CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_received_at
            ON webhook_deliveries(received_at DESC);
        CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_kind
            ON webhook_deliveries(data_type, event_type, object_id);
        CREATE INDEX IF NOT EXISTS idx_webhook_rejections_received_at
            ON webhook_delivery_rejections(received_at DESC);
        CREATE INDEX IF NOT EXISTS idx_webhook_invalidations_available_at
            ON webhook_invalidations(available_at ASC, invalidation_id ASC);
        CREATE INDEX IF NOT EXISTS idx_webhook_attempts_invalidation
            ON webhook_processing_attempts(invalidation_id, started_at DESC);
        ",
    },
    Migration {
        version: 8,
        name: "webhook_invalidation_completion",
        sql: r"
        ALTER TABLE webhook_invalidations ADD COLUMN completed_at TEXT;

        CREATE INDEX IF NOT EXISTS idx_webhook_invalidations_pending
            ON webhook_invalidations(completed_at, available_at ASC, invalidation_id ASC);
        ",
    },
    Migration {
        version: 9,
        name: "daily_summary_oura_id_mapping",
        sql: r"
        ALTER TABLE daily_sleep ADD COLUMN oura_id TEXT;
        ALTER TABLE daily_readiness ADD COLUMN oura_id TEXT;
        ALTER TABLE daily_activity ADD COLUMN oura_id TEXT;

        CREATE INDEX IF NOT EXISTS idx_daily_sleep_oura_id
            ON daily_sleep(oura_id);
        CREATE INDEX IF NOT EXISTS idx_daily_readiness_oura_id
            ON daily_readiness(oura_id);
        CREATE INDEX IF NOT EXISTS idx_daily_activity_oura_id
            ON daily_activity(oura_id);
        ",
    },
    Migration {
        version: 10,
        name: "phase5_review_family_tables",
        sql: r"
        CREATE TABLE IF NOT EXISTS sleep_time (
            day TEXT PRIMARY KEY,
            oura_id TEXT,
            status TEXT,
            recommendation TEXT,
            optimal_bedtime_start_offset INTEGER,
            optimal_bedtime_end_offset INTEGER,
            optimal_bedtime_day_tz INTEGER,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS daily_stress (
            day TEXT PRIMARY KEY,
            oura_id TEXT,
            stress_high INTEGER,
            recovery_high INTEGER,
            day_summary TEXT,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS daily_resilience (
            day TEXT PRIMARY KEY,
            oura_id TEXT,
            level TEXT NOT NULL,
            sleep_recovery REAL NOT NULL,
            daytime_recovery REAL NOT NULL,
            stress REAL NOT NULL,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS daily_cardiovascular_age (
            day TEXT PRIMARY KEY,
            vascular_age INTEGER,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS vo2_max (
            day TEXT PRIMARY KEY,
            oura_id TEXT,
            recorded_at TEXT NOT NULL,
            vo2_max REAL,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS rest_mode_periods (
            period_id TEXT PRIMARY KEY,
            start_day TEXT NOT NULL,
            start_time TEXT,
            end_day TEXT,
            end_time TEXT,
            episode_count INTEGER NOT NULL,
            tags_json TEXT NOT NULL,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_sleep_time_oura_id
            ON sleep_time(oura_id);
        CREATE INDEX IF NOT EXISTS idx_sleep_time_day
            ON sleep_time(day);
        CREATE INDEX IF NOT EXISTS idx_daily_stress_oura_id
            ON daily_stress(oura_id);
        CREATE INDEX IF NOT EXISTS idx_daily_stress_day
            ON daily_stress(day);
        CREATE INDEX IF NOT EXISTS idx_daily_resilience_oura_id
            ON daily_resilience(oura_id);
        CREATE INDEX IF NOT EXISTS idx_daily_resilience_day
            ON daily_resilience(day);
        CREATE INDEX IF NOT EXISTS idx_daily_cardiovascular_age_day
            ON daily_cardiovascular_age(day);
        CREATE INDEX IF NOT EXISTS idx_vo2_max_oura_id
            ON vo2_max(oura_id);
        CREATE INDEX IF NOT EXISTS idx_vo2_max_day
            ON vo2_max(day);
        CREATE INDEX IF NOT EXISTS idx_rest_mode_periods_start_day
            ON rest_mode_periods(start_day);
        CREATE INDEX IF NOT EXISTS idx_rest_mode_periods_end_day
            ON rest_mode_periods(end_day);
        ",
    },
    Migration {
        version: 11,
        name: "phase5_review_signal_days",
        sql: r"
        CREATE TABLE IF NOT EXISTS derived_review_signal_days (
            signal_key TEXT NOT NULL,
            day TEXT NOT NULL,
            numeric_value REAL,
            text_value TEXT,
            baseline_mean REAL,
            baseline_stddev REAL,
            delta REAL,
            z_score REAL,
            persistence_days INTEGER NOT NULL,
            sufficiency TEXT NOT NULL,
            stale_days INTEGER NOT NULL,
            metadata_json TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (signal_key, day)
        );

        CREATE INDEX IF NOT EXISTS idx_review_signal_days_day
            ON derived_review_signal_days(day);
        CREATE INDEX IF NOT EXISTS idx_review_signal_days_signal
            ON derived_review_signal_days(signal_key, day);
        ",
    },
    Migration {
        version: 12,
        name: "phase5_vo2_max_history_and_keys",
        sql: r"
        ALTER TABLE vo2_max RENAME TO vo2_max_legacy;

        CREATE TABLE vo2_max (
            day TEXT NOT NULL,
            oura_id TEXT,
            recorded_at TEXT NOT NULL,
            vo2_max REAL,
            raw_cache_key TEXT,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (day, recorded_at)
        );

        INSERT INTO vo2_max (
            day,
            oura_id,
            recorded_at,
            vo2_max,
            raw_cache_key,
            updated_at
        )
        SELECT
            day,
            oura_id,
            recorded_at,
            vo2_max,
            raw_cache_key,
            updated_at
        FROM vo2_max_legacy;

        DROP TABLE vo2_max_legacy;

        CREATE INDEX IF NOT EXISTS idx_vo2_max_oura_id
            ON vo2_max(oura_id);
        CREATE INDEX IF NOT EXISTS idx_vo2_max_day
            ON vo2_max(day);
        CREATE INDEX IF NOT EXISTS idx_vo2_max_recorded_at
            ON vo2_max(recorded_at);
        ",
    },
];

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
    use rusqlite::{Connection, params};

    use super::{MIGRATIONS, current_version, run_migrations};

    #[test]
    fn applies_bootstrap_schema() {
        let mut connection = Connection::open_in_memory()
            .unwrap_or_else(|error| panic!("in-memory db should open: {error}"));
        let report = run_migrations(&mut connection)
            .unwrap_or_else(|error| panic!("migrations should succeed: {error}"));

        assert_eq!(report.current_version, current_version());
        assert_eq!(
            report.applied_versions,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
        );
    }

    #[test]
    fn phase3_migration_backfills_existing_workout_and_session_rows() {
        let mut connection = Connection::open_in_memory()
            .unwrap_or_else(|error| panic!("in-memory db should open: {error}"));
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                );",
            )
            .unwrap_or_else(|error| panic!("schema migrations table should exist: {error}"));

        for migration in &MIGRATIONS[..4] {
            connection
                .execute_batch(migration.sql)
                .unwrap_or_else(|error| panic!("phase-2 migration should apply: {error}"));
            connection
                .execute(
                    "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                    params![migration.version, migration.name, "2026-04-08T00:00:00Z"],
                )
                .unwrap_or_else(|error| panic!("migration marker should insert: {error}"));
        }

        connection
            .execute(
                "INSERT INTO workouts (
                    workout_id,
                    started_at,
                    ended_at,
                    sport,
                    raw_cache_key,
                    updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "legacy-workout",
                    "2026-04-07T23:30:00-07:00",
                    "2026-04-08T00:15:00-07:00",
                    "running",
                    "cache-workout",
                    "2026-04-08T00:00:00Z"
                ],
            )
            .unwrap_or_else(|error| panic!("legacy workout should insert: {error}"));
        connection
            .execute(
                "INSERT INTO sessions (
                    session_id,
                    started_at,
                    ended_at,
                    kind,
                    raw_cache_key,
                    updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "legacy-session",
                    "2026-04-08T06:30:00+02:00",
                    "2026-04-08T06:50:00+02:00",
                    "breathing",
                    "cache-session",
                    "2026-04-08T00:00:00Z"
                ],
            )
            .unwrap_or_else(|error| panic!("legacy session should insert: {error}"));

        let report = run_migrations(&mut connection)
            .unwrap_or_else(|error| panic!("phase-3 migrations should succeed: {error}"));
        assert_eq!(report.applied_versions, vec![5, 6, 7, 8, 9, 10, 11, 12]);

        let (workout_day, workout_title): (String, String) = connection
            .query_row(
                "SELECT day, title FROM workouts WHERE workout_id = ?1",
                params!["legacy-workout"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or_else(|error| panic!("backfilled workout should load: {error}"));
        assert_eq!(workout_day, "2026-04-07");
        assert_eq!(workout_title, "running");

        let (session_day, session_title): (String, String) = connection
            .query_row(
                "SELECT day, title FROM sessions WHERE session_id = ?1",
                params!["legacy-session"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or_else(|error| panic!("backfilled session should load: {error}"));
        assert_eq!(session_day, "2026-04-08");
        assert_eq!(session_title, "breathing");
    }

    #[test]
    fn phase4_migration_rehomes_legacy_webhook_subscriptions() {
        let mut connection = Connection::open_in_memory()
            .unwrap_or_else(|error| panic!("in-memory db should open: {error}"));
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                );",
            )
            .unwrap_or_else(|error| panic!("schema migrations table should exist: {error}"));

        for migration in &MIGRATIONS[..6] {
            connection
                .execute_batch(migration.sql)
                .unwrap_or_else(|error| panic!("pre-phase4 migration should apply: {error}"));
            connection
                .execute(
                    "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                    params![migration.version, migration.name, "2026-04-08T00:00:00Z"],
                )
                .unwrap_or_else(|error| panic!("migration marker should insert: {error}"));
        }

        connection
            .execute(
                "INSERT INTO webhook_subscriptions (
                    subscription_id,
                    event_type,
                    delivery_mode,
                    status,
                    created_at,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "legacy-subscription",
                    "create",
                    "https://example.test/webhooks/oura",
                    "active",
                    "2026-04-07T00:00:00Z",
                    "2026-04-08T00:00:00Z"
                ],
            )
            .unwrap_or_else(|error| panic!("legacy webhook subscription should insert: {error}"));

        let report = run_migrations(&mut connection)
            .unwrap_or_else(|error| panic!("phase4 migrations should succeed: {error}"));
        assert_eq!(report.applied_versions, vec![7, 8, 9, 10, 11, 12]);

        let row: (String, String, String) = connection
            .query_row(
                "SELECT subscription_id, callback_url, drift_status
                 FROM webhook_remote_subscriptions
                 WHERE subscription_id = ?1",
                params!["legacy-subscription"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap_or_else(|error| panic!("rehomed webhook subscription should load: {error}"));
        assert_eq!(row.0, "legacy-subscription");
        assert_eq!(row.1, "https://example.test/webhooks/oura");
        assert_eq!(row.2, "active");
    }

    #[test]
    fn phase4_migration_adds_daily_summary_oura_ids() {
        let mut connection = Connection::open_in_memory()
            .unwrap_or_else(|error| panic!("in-memory db should open: {error}"));
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                );",
            )
            .unwrap_or_else(|error| panic!("schema migrations table should exist: {error}"));

        for migration in &MIGRATIONS[..8] {
            connection
                .execute_batch(migration.sql)
                .unwrap_or_else(|error| panic!("pre-phase5 migration should apply: {error}"));
            connection
                .execute(
                    "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                    params![migration.version, migration.name, "2026-04-08T00:00:00Z"],
                )
                .unwrap_or_else(|error| panic!("migration marker should insert: {error}"));
        }

        let report = run_migrations(&mut connection)
            .unwrap_or_else(|error| panic!("phase5 migration should succeed: {error}"));
        assert_eq!(report.applied_versions, vec![9, 10, 11, 12]);

        let daily_sleep_columns: Vec<String> = {
            let mut statement = connection
                .prepare("PRAGMA table_info(daily_sleep)")
                .unwrap_or_else(|error| panic!("daily_sleep schema should prepare: {error}"));
            let rows = statement
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap_or_else(|error| panic!("daily_sleep schema should query: {error}"));
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .unwrap_or_else(|error| panic!("daily_sleep columns should load: {error}"))
        };
        assert!(daily_sleep_columns.iter().any(|column| column == "oura_id"));
    }

    #[test]
    fn phase5_migration_creates_review_family_tables() {
        let mut connection = Connection::open_in_memory()
            .unwrap_or_else(|error| panic!("in-memory db should open: {error}"));
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                );",
            )
            .unwrap_or_else(|error| panic!("schema migrations table should exist: {error}"));

        for migration in &MIGRATIONS[..9] {
            connection
                .execute_batch(migration.sql)
                .unwrap_or_else(|error| panic!("pre-phase5 migration should apply: {error}"));
            connection
                .execute(
                    "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                    params![migration.version, migration.name, "2026-04-09T00:00:00Z"],
                )
                .unwrap_or_else(|error| panic!("migration marker should insert: {error}"));
        }

        let report = run_migrations(&mut connection)
            .unwrap_or_else(|error| panic!("phase5 migration should succeed: {error}"));
        assert_eq!(report.applied_versions, vec![10, 11, 12]);

        let table_names: Vec<String> = {
            let mut statement = connection
                .prepare(
                    "SELECT name
                     FROM sqlite_master
                     WHERE type = 'table'
                       AND name IN (
                           'sleep_time',
                           'daily_stress',
                           'daily_resilience',
                           'daily_cardiovascular_age',
                           'vo2_max',
                           'rest_mode_periods'
                       )
                     ORDER BY name ASC",
                )
                .unwrap_or_else(|error| panic!("schema query should prepare: {error}"));
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap_or_else(|error| panic!("schema query should run: {error}"));
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .unwrap_or_else(|error| panic!("table names should load: {error}"))
        };

        assert_eq!(
            table_names,
            vec![
                "daily_cardiovascular_age".to_owned(),
                "daily_resilience".to_owned(),
                "daily_stress".to_owned(),
                "rest_mode_periods".to_owned(),
                "sleep_time".to_owned(),
                "vo2_max".to_owned(),
            ]
        );
    }

    #[test]
    fn phase5_review_signal_migration_creates_snapshot_table() {
        let mut connection = Connection::open_in_memory()
            .unwrap_or_else(|error| panic!("in-memory db should open: {error}"));
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                );",
            )
            .unwrap_or_else(|error| panic!("schema migrations table should exist: {error}"));

        for migration in &MIGRATIONS[..10] {
            connection
                .execute_batch(migration.sql)
                .unwrap_or_else(|error| {
                    panic!("pre-review-signal migration should apply: {error}")
                });
            connection
                .execute(
                    "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                    params![migration.version, migration.name, "2026-04-09T00:00:00Z"],
                )
                .unwrap_or_else(|error| panic!("migration marker should insert: {error}"));
        }

        let report = run_migrations(&mut connection)
            .unwrap_or_else(|error| panic!("review signal migration should succeed: {error}"));
        assert_eq!(report.applied_versions, vec![11, 12]);

        let table_names: Vec<String> = {
            let mut statement = connection
                .prepare(
                    "SELECT name
                     FROM sqlite_master
                     WHERE type = 'table'
                       AND name = 'derived_review_signal_days'",
                )
                .unwrap_or_else(|error| panic!("schema query should prepare: {error}"));
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap_or_else(|error| panic!("schema query should run: {error}"));
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .unwrap_or_else(|error| panic!("table names should load: {error}"))
        };

        assert_eq!(table_names, vec!["derived_review_signal_days".to_owned()]);
    }

    #[test]
    fn phase5_vo2_max_history_migration_preserves_rows_and_composite_key() {
        let mut connection = Connection::open_in_memory()
            .unwrap_or_else(|error| panic!("in-memory db should open: {error}"));
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                );",
            )
            .unwrap_or_else(|error| panic!("schema migrations table should exist: {error}"));

        for migration in &MIGRATIONS[..11] {
            connection
                .execute_batch(migration.sql)
                .unwrap_or_else(|error| panic!("pre-vo2 history migration should apply: {error}"));
            connection
                .execute(
                    "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                    params![migration.version, migration.name, "2026-04-09T00:00:00Z"],
                )
                .unwrap_or_else(|error| panic!("migration marker should insert: {error}"));
        }

        connection
            .execute(
                "INSERT INTO vo2_max (
                    oura_id,
                    day,
                    recorded_at,
                    vo2_max,
                    raw_cache_key,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "vo2-1",
                    "2026-04-08",
                    "2026-04-08T08:00:00Z",
                    42.5_f64,
                    "raw-1",
                    "2026-04-08T09:00:00Z"
                ],
            )
            .unwrap_or_else(|error| panic!("legacy vo2 row should seed: {error}"));

        let report = run_migrations(&mut connection)
            .unwrap_or_else(|error| panic!("vo2 history migration should succeed: {error}"));
        assert_eq!(report.applied_versions, vec![12]);

        let primary_key_columns: Vec<String> = {
            let mut statement = connection
                .prepare("PRAGMA table_info(vo2_max)")
                .unwrap_or_else(|error| panic!("vo2_max schema should prepare: {error}"));
            let rows = statement
                .query_map([], |row| {
                    let name = row.get::<_, String>(1)?;
                    let pk_position = row.get::<_, i64>(5)?;
                    Ok((name, pk_position))
                })
                .unwrap_or_else(|error| panic!("vo2_max schema should query: {error}"));
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .unwrap_or_else(|error| panic!("vo2_max columns should load: {error}"))
                .into_iter()
                .filter(|(_, pk_position)| *pk_position > 0)
                .map(|(name, _)| name)
                .collect()
        };

        assert_eq!(
            primary_key_columns,
            vec!["day".to_owned(), "recorded_at".to_owned()]
        );

        let row_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM vo2_max", [], |row| row.get(0))
            .unwrap_or_else(|error| panic!("migrated vo2 rows should count: {error}"));
        assert_eq!(row_count, 1);
    }
}
