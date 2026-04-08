use std::io;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, RingmasterError>;

#[derive(Debug, Error)]
pub enum RingmasterError {
    #[error("{0}")]
    Cli(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("I/O error while {context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("config parse error: {0}")]
    ConfigParse(#[from] toml::de::Error),
    #[error("storage error: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("oauth error: {0}")]
    Auth(String),
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("ui error: {0}")]
    Ui(String),
}

impl RingmasterError {
    pub fn io(context: &'static str, source: io::Error) -> Self {
        Self::Io { context, source }
    }
}
