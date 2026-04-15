use std::path::PathBuf;
use std::time::Duration;

use rusqlite::Connection;

use crate::config::Config;
use crate::error::{Result, RingmasterError};
use crate::store::migrations::{MigrationReport, run_migrations};
use crate::store::queries::{
    AnalysisStore, AuthStore, DerivedStore, ImportStore, MetadataStore, SyncStateStore, ViewStore,
};
use crate::store::webhook_store::WebhookStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorePlan {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
}

#[derive(Debug)]
pub struct Store {
    plan: StorePlan,
    connection: Connection,
    migration_report: MigrationReport,
    #[cfg(test)]
    _temp_dir: Option<tempfile::TempDir>,
}

impl StorePlan {
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self {
            data_dir: config.paths.state_dir.clone(),
            db_path: config.paths.database_file.clone(),
        }
    }

    /// Ensures the parent directory for the store database exists.
    ///
    /// # Errors
    ///
    /// Returns an error when the on-disk data directory cannot be created.
    pub fn ensure_directories(&self) -> Result<()> {
        if self.db_path.as_os_str() == ":memory:" {
            return Ok(());
        }

        std::fs::create_dir_all(&self.data_dir)
            .map_err(|error| RingmasterError::io("creating store data directory", error))
    }
}

impl Store {
    /// Opens the configured store, applies migrations, and records bootstrap metadata.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be opened, configured, migrated,
    /// or seeded with the application metadata.
    pub fn open(config: &Config) -> Result<Self> {
        Self::open_with_plan(StorePlan::from_config(config), config.app_name)
    }

    /// Opens a store from an already computed plan.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be opened, configured, migrated,
    /// or seeded with the application metadata.
    pub(crate) fn open_with_plan(plan: StorePlan, app_name: &str) -> Result<Self> {
        plan.ensure_directories()?;
        let mut connection = Connection::open(&plan.db_path)?;
        configure_connection(&connection)?;
        let migration_report = run_migrations(&mut connection)?;

        let store = Self {
            plan,
            connection,
            migration_report,
            #[cfg(test)]
            _temp_dir: None,
        };
        store.metadata().upsert("app_name", app_name)?;

        Ok(store)
    }

    #[cfg(test)]
    /// Opens an isolated temporary on-disk store for tests.
    ///
    /// # Errors
    ///
    /// Returns an error when the temporary database cannot be opened,
    /// configured, or migrated.
    pub fn open_test_store() -> Result<Self> {
        let temp_dir = tempfile::tempdir()
            .map_err(|error| RingmasterError::io("creating isolated test store", error))?;
        let plan = StorePlan {
            data_dir: temp_dir.path().to_path_buf(),
            db_path: temp_dir.path().join("ringmaster-test.db"),
        };
        plan.ensure_directories()?;
        let mut connection = Connection::open(&plan.db_path)?;
        configure_connection(&connection)?;
        let migration_report = run_migrations(&mut connection)?;

        let store = Self {
            plan,
            connection,
            migration_report,
            _temp_dir: Some(temp_dir),
        };
        store.metadata().upsert("app_name", "ringmaster")?;

        Ok(store)
    }

    #[must_use]
    pub const fn plan(&self) -> &StorePlan {
        &self.plan
    }

    #[must_use]
    pub const fn migration_report(&self) -> &MigrationReport {
        &self.migration_report
    }

    #[must_use]
    pub const fn metadata(&self) -> MetadataStore<'_> {
        MetadataStore::new(&self.connection)
    }

    #[must_use]
    pub const fn sync_state(&self) -> SyncStateStore<'_> {
        SyncStateStore::new(&self.connection)
    }

    #[must_use]
    pub const fn auth(&self) -> AuthStore<'_> {
        AuthStore::new(&self.connection)
    }

    #[must_use]
    pub const fn imports(&self) -> ImportStore<'_> {
        ImportStore::new(&self.connection)
    }

    #[must_use]
    pub const fn derived(&self) -> DerivedStore<'_> {
        DerivedStore::new(&self.connection)
    }

    #[must_use]
    pub const fn analysis(&self) -> AnalysisStore<'_> {
        AnalysisStore::new(&self.connection)
    }

    #[must_use]
    pub const fn views(&self) -> ViewStore<'_> {
        ViewStore::new(&self.connection)
    }

    #[must_use]
    pub const fn webhook(&self) -> WebhookStore<'_> {
        WebhookStore::new(&self.connection)
    }
}

fn configure_connection(connection: &Connection) -> Result<()> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA synchronous = NORMAL;
         PRAGMA journal_mode = WAL;",
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::store::db::Store;
    use crate::store::queries::{SyncRunStatus, SyncStateRecord};
    use crate::test_support::ok;

    #[test]
    fn opens_isolated_test_store() {
        let store = ok(Store::open_test_store(), "store should open");

        assert_eq!(store.migration_report().current_version, 20);
    }

    #[test]
    fn sync_state_survives_reopen_and_stays_per_family() {
        let tempdir = ok(tempdir(), "tempdir should build");
        let plan = super::StorePlan {
            data_dir: tempdir.path().to_path_buf(),
            db_path: tempdir.path().join("ringmaster-test.db"),
        };
        let store = ok(
            Store::open_with_plan(plan.clone(), "ringmaster"),
            "store should open",
        );

        ok(
            store.sync_state().upsert(&SyncStateRecord {
                sync_key: "oura.daily".to_owned(),
                family: "daily".to_owned(),
                status: SyncRunStatus::Success,
                cursor: Some("2026-04-08".to_owned()),
                last_successful_sync_end: Some("2026-04-08".to_owned()),
                last_attempted_at: "2026-04-08T06:00:00Z".to_owned(),
                last_completed_at: Some("2026-04-08T06:00:05Z".to_owned()),
                last_reconcile_end: Some("2026-04-08".to_owned()),
                oldest_recently_reconciled_at: Some("2026-03-10".to_owned()),
                message: Some("daily sync complete".to_owned()),
                granted_scopes: vec!["daily".to_owned()],
                last_error: None,
                last_error_at: None,
                last_error_kind: None,
                last_error_detail: None,
                failure_count: 0,
                next_attempt_after: None,
                last_trigger_source: Some("periodic_reconcile".to_owned()),
                last_trigger_detail: Some("daily seed".to_owned()),
                updated_at: "2026-04-08T06:00:05Z".to_owned(),
            }),
            "daily sync state should persist",
        );
        ok(
            store.sync_state().upsert(&SyncStateRecord {
                sync_key: "oura.heartrate".to_owned(),
                family: "heartrate".to_owned(),
                status: SyncRunStatus::Success,
                cursor: Some("2026-04-08T05:45:00Z".to_owned()),
                last_successful_sync_end: Some("2026-04-08T05:45:00Z".to_owned()),
                last_attempted_at: "2026-04-08T06:10:00Z".to_owned(),
                last_completed_at: Some("2026-04-08T06:10:05Z".to_owned()),
                last_reconcile_end: Some("2026-04-08T06:10:05Z".to_owned()),
                oldest_recently_reconciled_at: Some("2026-04-01".to_owned()),
                message: Some("heartrate sync complete".to_owned()),
                granted_scopes: vec!["heartrate".to_owned()],
                last_error: None,
                last_error_at: None,
                last_error_kind: None,
                last_error_detail: None,
                failure_count: 0,
                next_attempt_after: None,
                last_trigger_source: Some("startup".to_owned()),
                last_trigger_detail: Some("heartrate seed".to_owned()),
                updated_at: "2026-04-08T06:10:05Z".to_owned(),
            }),
            "heartrate sync state should persist",
        );

        drop(store);

        let reopened = ok(
            Store::open_with_plan(plan, "ringmaster"),
            "store should reopen from the same plan",
        );
        let states = ok(
            reopened.sync_state().list(),
            "reopened sync states should load",
        );
        let daily = states
            .iter()
            .find(|state| state.sync_key == "oura.daily")
            .unwrap_or_else(|| panic!("daily sync state should exist"));
        let heartrate = states
            .iter()
            .find(|state| state.sync_key == "oura.heartrate")
            .unwrap_or_else(|| panic!("heartrate sync state should exist"));

        assert_eq!(daily.family, "daily");
        assert_eq!(
            daily.last_successful_sync_end.as_deref(),
            Some("2026-04-08")
        );
        assert_eq!(heartrate.family, "heartrate");
        assert_eq!(
            heartrate.last_successful_sync_end.as_deref(),
            Some("2026-04-08T05:45:00Z")
        );
    }
}
