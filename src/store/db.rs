use std::path::PathBuf;

use crate::config::Config;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorePlan {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
}

impl StorePlan {
    pub fn from_config(config: &Config) -> Self {
        Self {
            data_dir: config.state_dir.clone(),
            db_path: config.state_dir.join("ringmaster.db"),
        }
    }
}
