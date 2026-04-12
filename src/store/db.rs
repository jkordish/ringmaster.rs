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
    use crate::store::db::Store;
    use crate::test_support::ok;

    #[test]
    fn opens_isolated_test_store() {
        let store = ok(Store::open_test_store(), "store should open");

        assert_eq!(store.migration_report().current_version, 17);
    }
}
