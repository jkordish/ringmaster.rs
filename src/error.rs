use std::fmt::{Display, Formatter};
use std::io;

use serde::{Deserialize, Serialize};
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
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("storage error: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error(transparent)]
    OuraApi(#[from] OuraApiError),
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("ui error: {0}")]
    Ui(String),
}

#[derive(Debug, Error)]
pub enum SecretStoreError {
    #[error("secret backend is unavailable: {0}")]
    BackendUnavailable(String),
    #[error("secret `{0}` is not stored")]
    MissingSecret(&'static str),
    #[error("keyring error: {0}")]
    Keyring(#[from] keyring::Error),
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Oura client credentials are not configured")]
    MissingClientCredentials,
    #[error("failed to build the OAuth flow: {0}")]
    InvalidOAuthConfig(String),
    #[error("failed to start the loopback callback listener: {0}")]
    CallbackListener(String),
    #[error("auth callback timed out after {0}s")]
    CallbackTimeout(u64),
    #[error("authorization was denied by the user")]
    AccessDenied,
    #[error("the auth callback did not include an authorization code")]
    MissingAuthorizationCode,
    #[error("the auth callback state did not match the active login session")]
    StateMismatch,
    #[error("the refresh token is not available in the secret store")]
    MissingRefreshToken,
    #[error("the access token is not available in the secret store")]
    MissingAccessToken,
    #[error(transparent)]
    SecretStore(#[from] SecretStoreError),
    #[error(transparent)]
    Problem(#[from] OuraProblem),
    #[error("oauth flow error: {0}")]
    OAuthFlow(String),
}

#[derive(Debug, Error)]
pub enum OuraApiError {
    #[error(transparent)]
    Problem(#[from] OuraProblem),
    #[error("unexpected Oura API response: {0}")]
    UnexpectedResponse(String),
    #[error("failed to decode Oura API payload for `{endpoint}`: {source}")]
    Decode {
        endpoint: &'static str,
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OuraProblem {
    pub status: Option<u16>,
    pub title: String,
    pub detail: Option<String>,
    pub oauth_error: Option<String>,
    pub oauth_error_description: Option<String>,
}

impl RingmasterError {
    pub fn io(context: &'static str, source: io::Error) -> Self {
        Self::Io { context, source }
    }
}

impl OuraProblem {
    pub fn new(status: Option<u16>, title: impl Into<String>, detail: Option<String>) -> Self {
        Self {
            status,
            title: title.into(),
            detail,
            oauth_error: None,
            oauth_error_description: None,
        }
    }

    pub fn oauth(
        status: Option<u16>,
        title: impl Into<String>,
        detail: Option<String>,
        oauth_error: Option<String>,
        oauth_error_description: Option<String>,
    ) -> Self {
        Self {
            status,
            title: title.into(),
            detail,
            oauth_error,
            oauth_error_description,
        }
    }
}

impl Display for OuraProblem {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(status) = self.status {
            write!(formatter, "Oura API problem {status}: {}", self.title)?;
        } else {
            write!(formatter, "Oura API problem: {}", self.title)?;
        }

        if let Some(detail) = &self.detail {
            write!(formatter, " ({detail})")?;
        }

        if let Some(error) = &self.oauth_error {
            write!(formatter, " [oauth_error={error}]")?;
        }

        Ok(())
    }
}

impl std::error::Error for OuraProblem {}
