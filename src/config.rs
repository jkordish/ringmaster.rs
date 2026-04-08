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
    pub refresh: RefreshConfig,
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

#[derive(Debug, Clone)]
pub struct RefreshConfig {
    pub personal_interval_secs: u64,
    pub daily_interval_secs: u64,
    pub heartrate_interval_secs: u64,
    pub workout_interval_secs: u64,
    pub enhanced_tag_interval_secs: u64,
    pub session_interval_secs: u64,
    pub personal_stale_after_secs: u64,
    pub daily_stale_after_secs: u64,
    pub heartrate_stale_after_secs: u64,
    pub workout_stale_after_secs: u64,
    pub enhanced_tag_stale_after_secs: u64,
    pub session_stale_after_secs: u64,
    pub daily_history_days: u16,
    pub daily_overlap_days: u16,
    pub heartrate_history_days: u16,
    pub heartrate_overlap_minutes: u16,
    pub workout_history_days: u16,
    pub workout_overlap_days: u16,
    pub enhanced_tag_history_days: u16,
    pub enhanced_tag_overlap_days: u16,
    pub session_history_days: u16,
    pub session_overlap_days: u16,
    pub max_backoff_secs: u64,
    pub demo_fixture_dir: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    logging: Option<FileLoggingConfig>,
    oura: Option<FileOuraConfig>,
    refresh: Option<FileRefreshConfig>,
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

#[derive(Debug, Default, Deserialize)]
struct FileRefreshConfig {
    personal_interval_secs: Option<u64>,
    daily_interval_secs: Option<u64>,
    heartrate_interval_secs: Option<u64>,
    workout_interval_secs: Option<u64>,
    enhanced_tag_interval_secs: Option<u64>,
    session_interval_secs: Option<u64>,
    personal_stale_after_secs: Option<u64>,
    daily_stale_after_secs: Option<u64>,
    heartrate_stale_after_secs: Option<u64>,
    workout_stale_after_secs: Option<u64>,
    enhanced_tag_stale_after_secs: Option<u64>,
    session_stale_after_secs: Option<u64>,
    daily_history_days: Option<u16>,
    daily_overlap_days: Option<u16>,
    heartrate_history_days: Option<u16>,
    heartrate_overlap_minutes: Option<u16>,
    workout_history_days: Option<u16>,
    workout_overlap_days: Option<u16>,
    enhanced_tag_history_days: Option<u16>,
    enhanced_tag_overlap_days: Option<u16>,
    session_history_days: Option<u16>,
    session_overlap_days: Option<u16>,
    max_backoff_secs: Option<u64>,
    demo_fixture_dir: Option<PathBuf>,
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

        let config = Self {
            app_name: APP_NAME,
            paths,
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
            refresh: RefreshConfig {
                personal_interval_secs: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.personal_interval_secs)
                    .unwrap_or(3_600),
                daily_interval_secs: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.daily_interval_secs)
                    .unwrap_or(300),
                heartrate_interval_secs: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.heartrate_interval_secs)
                    .unwrap_or(60),
                workout_interval_secs: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.workout_interval_secs)
                    .unwrap_or(600),
                enhanced_tag_interval_secs: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.enhanced_tag_interval_secs)
                    .unwrap_or(300),
                session_interval_secs: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.session_interval_secs)
                    .unwrap_or(300),
                personal_stale_after_secs: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.personal_stale_after_secs)
                    .unwrap_or(72 * 60 * 60),
                daily_stale_after_secs: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.daily_stale_after_secs)
                    .unwrap_or(12 * 60 * 60),
                heartrate_stale_after_secs: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.heartrate_stale_after_secs)
                    .unwrap_or(15 * 60),
                workout_stale_after_secs: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.workout_stale_after_secs)
                    .unwrap_or(24 * 60 * 60),
                enhanced_tag_stale_after_secs: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.enhanced_tag_stale_after_secs)
                    .unwrap_or(12 * 60 * 60),
                session_stale_after_secs: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.session_stale_after_secs)
                    .unwrap_or(12 * 60 * 60),
                daily_history_days: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.daily_history_days)
                    .unwrap_or(90),
                daily_overlap_days: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.daily_overlap_days)
                    .unwrap_or(2),
                heartrate_history_days: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.heartrate_history_days)
                    .unwrap_or(7),
                heartrate_overlap_minutes: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.heartrate_overlap_minutes)
                    .unwrap_or(60),
                workout_history_days: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.workout_history_days)
                    .unwrap_or(90),
                workout_overlap_days: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.workout_overlap_days)
                    .unwrap_or(2),
                enhanced_tag_history_days: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.enhanced_tag_history_days)
                    .unwrap_or(90),
                enhanced_tag_overlap_days: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.enhanced_tag_overlap_days)
                    .unwrap_or(2),
                session_history_days: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.session_history_days)
                    .unwrap_or(90),
                session_overlap_days: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.session_overlap_days)
                    .unwrap_or(2),
                max_backoff_secs: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.max_backoff_secs)
                    .unwrap_or(60 * 60),
                demo_fixture_dir: file_config
                    .refresh
                    .as_ref()
                    .and_then(|refresh| refresh.demo_fixture_dir.clone())
                    .or_else(|| Some(PathBuf::from("tests/fixtures/phase3"))),
            },
        };

        config.refresh.validate()?;

        Ok(config)
    }
}

impl RefreshConfig {
    fn validate(&self) -> Result<()> {
        for (label, value) in [
            ("refresh.daily_history_days", self.daily_history_days),
            (
                "refresh.heartrate_history_days",
                self.heartrate_history_days,
            ),
            ("refresh.workout_history_days", self.workout_history_days),
            (
                "refresh.enhanced_tag_history_days",
                self.enhanced_tag_history_days,
            ),
            ("refresh.session_history_days", self.session_history_days),
        ] {
            if value == 0 {
                return Err(RingmasterError::Config(format!(
                    "{label} must be at least 1"
                )));
            }
        }

        if self.max_backoff_secs == 0 {
            return Err(RingmasterError::Config(
                "refresh.max_backoff_secs must be at least 1".to_owned(),
            ));
        }

        Ok(())
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
    [
        "personal",
        "daily",
        "heartrate",
        "workout",
        "session",
        "enhanced_tag",
    ]
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

    use super::{AppPaths, Config, OuraConfig, RefreshConfig, default_requested_scopes};

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

    #[test]
    fn loads_responsive_refresh_defaults() {
        let config = Config::load().unwrap_or_else(|error| {
            panic!("config load should succeed with repo defaults: {error}");
        });

        assert_eq!(config.refresh.heartrate_interval_secs, 60);
        assert_eq!(config.refresh.daily_interval_secs, 300);
        assert_eq!(config.refresh.personal_interval_secs, 3_600);
        assert_eq!(config.refresh.workout_interval_secs, 600);
        assert_eq!(config.refresh.enhanced_tag_interval_secs, 300);
        assert_eq!(config.refresh.session_interval_secs, 300);
    }

    #[test]
    fn rejects_zero_daily_history_days() {
        let refresh = RefreshConfig {
            personal_interval_secs: 3_600,
            daily_interval_secs: 300,
            heartrate_interval_secs: 60,
            workout_interval_secs: 600,
            enhanced_tag_interval_secs: 300,
            session_interval_secs: 300,
            personal_stale_after_secs: 72 * 60 * 60,
            daily_stale_after_secs: 12 * 60 * 60,
            heartrate_stale_after_secs: 15 * 60,
            workout_stale_after_secs: 24 * 60 * 60,
            enhanced_tag_stale_after_secs: 12 * 60 * 60,
            session_stale_after_secs: 12 * 60 * 60,
            daily_history_days: 0,
            daily_overlap_days: 2,
            heartrate_history_days: 7,
            heartrate_overlap_minutes: 60,
            workout_history_days: 90,
            workout_overlap_days: 2,
            enhanced_tag_history_days: 90,
            enhanced_tag_overlap_days: 2,
            session_history_days: 90,
            session_overlap_days: 2,
            max_backoff_secs: 60 * 60,
            demo_fixture_dir: None,
        };

        let error = refresh
            .validate()
            .err()
            .unwrap_or_else(|| panic!("zero daily history days should be rejected"));

        assert!(
            error
                .to_string()
                .contains("refresh.daily_history_days must be at least 1")
        );
    }
}
