use std::path::PathBuf;
use std::time::Duration;

use rusqlite::Connection;

use crate::config::Config;
use crate::error::{Result, RingmasterError};
use crate::store::migrations::{MigrationReport, run_migrations};
use crate::store::queries::{
    AuthStore, DerivedStore, ImportStore, MetadataStore, SyncStateStore, ViewStore,
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
}

impl StorePlan {
    pub fn from_config(config: &Config) -> Self {
        Self {
            data_dir: config.paths.state_dir.clone(),
            db_path: config.paths.database_file.clone(),
        }
    }

    pub fn ensure_directories(&self) -> Result<()> {
        if self.db_path.as_os_str() == ":memory:" {
            return Ok(());
        }

        std::fs::create_dir_all(&self.data_dir)
            .map_err(|error| RingmasterError::io("creating store data directory", error))
    }
}

impl Store {
    pub fn open(config: &Config) -> Result<Self> {
        let plan = StorePlan::from_config(config);
        plan.ensure_directories()?;
        let mut connection = Connection::open(&plan.db_path)?;
        configure_connection(&mut connection)?;
        let migration_report = run_migrations(&mut connection)?;

        let store = Self {
            plan,
            connection,
            migration_report,
        };
        store.metadata().upsert("app_name", config.app_name)?;

        Ok(store)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let mut connection = Connection::open_in_memory()?;
        configure_connection(&mut connection)?;
        let migration_report = run_migrations(&mut connection)?;

        Ok(Self {
            plan: StorePlan {
                data_dir: PathBuf::from("."),
                db_path: PathBuf::from(":memory:"),
            },
            connection,
            migration_report,
        })
    }

    pub fn plan(&self) -> &StorePlan {
        &self.plan
    }

    pub fn migration_report(&self) -> &MigrationReport {
        &self.migration_report
    }

    pub fn metadata(&self) -> MetadataStore<'_> {
        MetadataStore::new(&self.connection)
    }

    pub fn sync_state(&self) -> SyncStateStore<'_> {
        SyncStateStore::new(&self.connection)
    }

    pub fn auth(&self) -> AuthStore<'_> {
        AuthStore::new(&self.connection)
    }

    pub fn imports(&self) -> ImportStore<'_> {
        ImportStore::new(&self.connection)
    }

    pub fn derived(&self) -> DerivedStore<'_> {
        DerivedStore::new(&self.connection)
    }

    pub fn views(&self) -> ViewStore<'_> {
        ViewStore::new(&self.connection)
    }

    pub fn webhook(&self) -> WebhookStore<'_> {
        WebhookStore::new(&self.connection)
    }
}

fn configure_connection(connection: &mut Connection) -> Result<()> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA synchronous = NORMAL;
         PRAGMA journal_mode = WAL;",
    )?;

    Ok(())
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use crate::store::db::Store;

    #[test]
    fn opens_in_memory_store() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));

        assert_eq!(store.migration_report().current_version, 9);
    }
}
