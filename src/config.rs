use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Result, RingmasterError};

pub const APP_NAME: &str = "ringmaster";

#[derive(Debug, Clone)]
pub struct Config {
    pub app_name: &'static str,
    pub paths: AppPaths,
    pub logging: LoggingConfig,
    pub oura: OuraConfig,
}

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub home_dir: PathBuf,
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub state_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub database_file: PathBuf,
    pub log_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub filter: String,
}

#[derive(Debug, Clone)]
pub struct OuraConfig {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub authorize_url: String,
    pub token_url: String,
    pub api_base_url: String,
    pub callback_bind: SocketAddr,
    pub callback_path: String,
    pub requested_scopes: Vec<String>,
    pub auth_timeout_secs: u64,
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    logging: Option<FileLoggingConfig>,
    oura: Option<FileOuraConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct FileLoggingConfig {
    filter: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FileOuraConfig {
    client_id: Option<String>,
    authorize_url: Option<String>,
    token_url: Option<String>,
    api_base_url: Option<String>,
    callback_bind: Option<String>,
    callback_path: Option<String>,
    requested_scopes: Option<Vec<String>>,
    auth_timeout_secs: Option<u64>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let paths = AppPaths::detect()?;
        let file_config = load_file_config(&paths.config_file)?;

        let logging_filter = env_string("RINGMASTER_LOG_FILTER")
            .or_else(|| {
                file_config
                    .logging
                    .as_ref()
                    .and_then(|logging| logging.filter.clone())
            })
            .unwrap_or_else(|| "ringmaster=info".to_owned());

        let callback_bind = env_string("RINGMASTER_OURA_CALLBACK_BIND")
            .or_else(|| {
                file_config
                    .oura
                    .as_ref()
                    .and_then(|oura| oura.callback_bind.clone())
            })
            .unwrap_or_else(|| "127.0.0.1:8788".to_owned());

        let callback_bind = callback_bind.parse::<SocketAddr>().map_err(|error| {
            RingmasterError::Config(format!(
                "invalid Oura callback bind address `{callback_bind}`: {error}"
            ))
        })?;

        let callback_path = env_string("RINGMASTER_OURA_CALLBACK_PATH")
            .or_else(|| {
                file_config
                    .oura
                    .as_ref()
                    .and_then(|oura| oura.callback_path.clone())
            })
            .unwrap_or_else(|| "/callback".to_owned());

        Ok(Self {
            app_name: APP_NAME,
            paths: paths.clone(),
            logging: LoggingConfig {
                filter: logging_filter,
            },
            oura: OuraConfig {
                client_id: env_string("RINGMASTER_OURA_CLIENT_ID").or_else(|| {
                    file_config
                        .oura
                        .as_ref()
                        .and_then(|oura| oura.client_id.clone())
                }),
                client_secret: env_string("RINGMASTER_OURA_CLIENT_SECRET"),
                authorize_url: env_string("RINGMASTER_OURA_AUTHORIZE_URL")
                    .or_else(|| {
                        file_config
                            .oura
                            .as_ref()
                            .and_then(|oura| oura.authorize_url.clone())
                    })
                    .unwrap_or_else(|| "https://cloud.oura.com/oauth/authorize".to_owned()),
                token_url: env_string("RINGMASTER_OURA_TOKEN_URL")
                    .or_else(|| {
                        file_config
                            .oura
                            .as_ref()
                            .and_then(|oura| oura.token_url.clone())
                    })
                    .unwrap_or_else(|| "https://api.oura.com/oauth/token".to_owned()),
                api_base_url: env_string("RINGMASTER_OURA_API_BASE_URL")
                    .or_else(|| {
                        file_config
                            .oura
                            .as_ref()
                            .and_then(|oura| oura.api_base_url.clone())
                    })
                    .unwrap_or_else(|| "https://api.oura.com".to_owned()),
                callback_bind,
                callback_path,
                requested_scopes: env_csv("RINGMASTER_OURA_REQUESTED_SCOPES").unwrap_or_else(
                    || {
                        file_config
                            .oura
                            .as_ref()
                            .and_then(|oura| oura.requested_scopes.clone())
                            .unwrap_or_else(default_requested_scopes)
                    },
                ),
                auth_timeout_secs: env_string("RINGMASTER_OURA_AUTH_TIMEOUT_SECS")
                    .and_then(|value| value.parse::<u64>().ok())
                    .or_else(|| {
                        file_config
                            .oura
                            .as_ref()
                            .and_then(|oura| oura.auth_timeout_secs)
                    })
                    .unwrap_or(120),
            },
        })
    }
}

impl AppPaths {
    pub fn detect() -> Result<Self> {
        let home_dir = env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| RingmasterError::Config("HOME is not set".to_owned()))?;

        let config_root = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir.join(".config"));
        let state_root = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir.join(".local").join("state"));
        let cache_root = env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir.join(".cache"));

        Self::from_roots(home_dir, config_root, state_root, cache_root)
    }

    pub fn from_roots(
        home_dir: PathBuf,
        config_root: PathBuf,
        state_root: PathBuf,
        cache_root: PathBuf,
    ) -> Result<Self> {
        if is_empty_path(&config_root) || is_empty_path(&state_root) || is_empty_path(&cache_root) {
            return Err(RingmasterError::Config(
                "resolved XDG paths must not be empty".to_owned(),
            ));
        }

        let config_dir = config_root.join(APP_NAME);
        let state_dir = state_root.join(APP_NAME);
        let cache_dir = cache_root.join(APP_NAME);
        let log_dir = state_dir.join("logs");

        Ok(Self {
            home_dir,
            config_dir: config_dir.clone(),
            config_file: config_dir.join("config.toml"),
            state_dir: state_dir.clone(),
            cache_dir,
            database_file: state_dir.join("ringmaster.db"),
            log_dir,
        })
    }

    pub fn ensure_runtime_dirs(&self) -> Result<()> {
        for path in [
            &self.config_dir,
            &self.state_dir,
            &self.cache_dir,
            &self.log_dir,
        ] {
            create_dir_all(path)?;
        }

        Ok(())
    }

    pub fn config_file_present(&self) -> bool {
        self.config_file.is_file()
    }

    pub fn database_present(&self) -> bool {
        self.database_file.is_file()
    }
}

impl OuraConfig {
    pub fn callback_url(&self) -> String {
        format!("http://{}{}", self.callback_bind, self.callback_path)
    }

    pub fn client_configured(&self) -> bool {
        self.client_id.is_some() && self.client_secret.is_some()
    }

    pub fn missing_fields(&self) -> Vec<&'static str> {
        let mut fields = Vec::new();

        if self.client_id.is_none() {
            fields.push("client_id");
        }
        if self.client_secret.is_none() {
            fields.push("client_secret");
        }

        fields
    }
}

fn env_string(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|value| {
        let trimmed = value.trim().to_owned();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn env_csv(key: &str) -> Option<Vec<String>> {
    env_string(key).map(|value| split_csv(&value))
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn default_requested_scopes() -> Vec<String> {
    ["personal", "daily", "heartrate"]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

fn load_file_config(path: &Path) -> Result<FileConfig> {
    if !path.exists() {
        return Ok(FileConfig::default());
    }

    let content = fs::read_to_string(path)
        .map_err(|error| RingmasterError::io("reading config file", error))?;
    toml::from_str(&content).map_err(Into::into)
}

fn create_dir_all(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .map_err(|error| RingmasterError::io("creating runtime directory", error))
}

fn is_empty_path(path: &Path) -> bool {
    path.as_os_str().is_empty()
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use std::path::PathBuf;

    use super::{AppPaths, OuraConfig, default_requested_scopes};

    #[test]
    fn builds_xdg_paths_from_roots() {
        let paths = AppPaths::from_roots(
            PathBuf::from("/home/tester"),
            PathBuf::from("/tmp/config"),
            PathBuf::from("/tmp/state"),
            PathBuf::from("/tmp/cache"),
        )
        .unwrap_or_else(|error| panic!("expected path resolution to succeed: {error}"));

        assert_eq!(
            paths.config_file,
            PathBuf::from("/tmp/config/ringmaster/config.toml")
        );
        assert_eq!(
            paths.database_file,
            PathBuf::from("/tmp/state/ringmaster/ringmaster.db")
        );
    }

    #[test]
    fn reports_missing_oura_credentials() {
        let config = OuraConfig {
            client_id: None,
            client_secret: None,
            authorize_url: String::new(),
            token_url: String::new(),
            api_base_url: String::new(),
            callback_bind: "127.0.0.1:8788"
                .parse()
                .unwrap_or_else(|error| panic!("test socket addr should parse: {error}")),
            callback_path: "/callback".to_owned(),
            requested_scopes: default_requested_scopes(),
            auth_timeout_secs: 120,
        };

        assert_eq!(config.missing_fields(), vec!["client_id", "client_secret"]);
    }
}
