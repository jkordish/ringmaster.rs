use std::env;
use std::path::PathBuf;

use crate::error::{Result, RingmasterError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub app_name: &'static str,
    pub config_dir: PathBuf,
    pub state_dir: PathBuf,
    pub oauth_callback: String,
}

impl Config {
    pub fn detect() -> Result<Self> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        let config_root = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));

        let state_root = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local").join("state"));

        if config_root.as_os_str().is_empty() || state_root.as_os_str().is_empty() {
            return Err(RingmasterError::Config(
                "resolved config or state directory was empty".to_owned(),
            ));
        }

        Ok(Self {
            app_name: "ringmaster",
            config_dir: config_root.join("ringmaster"),
            state_dir: state_root.join("ringmaster"),
            oauth_callback: "http://127.0.0.1:8788/callback".to_owned(),
        })
    }
}
