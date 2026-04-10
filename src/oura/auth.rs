use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;

use axum::{
    Router,
    extract::{Query, State},
    response::Html,
    routing::get,
};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
    basic::{BasicClient, BasicErrorResponse},
    reqwest::{Client as OAuthHttpClient, Error as OAuthReqwestError, redirect::Policy},
};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::config::{Config, OuraSecretBackend};
use crate::error::{AuthError, OuraProblem, Result, SecretStoreError};
use crate::oura::models::{AuthStatus, CapabilityReport};
use crate::store::Store;
use crate::store::queries::{AuthSessionRecord, OURA_PROVIDER};

const SECRET_SERVICE_NAME: &str = "ringmaster.rs";
const SECRET_USER_NAME: &str = "oura";
const ACCESS_TOKEN_REFRESH_SKEW_SECS: i64 = 60;

type OAuthClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginReport {
    pub status: LoginStatus,
    pub auth_status: AuthStatus,
    pub authorization_url: Option<String>,
    pub listener_plan: LoopbackListenerPlan,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginStatus {
    ConfigMissing,
    Authorized,
    PartialGrant,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopbackListenerPlan {
    pub bind_address: SocketAddr,
    pub callback_path: String,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct OAuthCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedSession {
    pub access_token: String,
    pub granted_scopes: Vec<String>,
    pub access_token_expires_at: Option<String>,
    pub account_id: Option<String>,
    pub account_email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredTokens {
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExchangedTokenSet {
    access_token: String,
    refresh_token: Option<String>,
    token_type: String,
    granted_scopes: Vec<String>,
    access_token_expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CallbackOutcome {
    AuthorizedCode(String),
    Denied(OuraProblem),
}

#[derive(Debug, Clone)]
struct CallbackState {
    sender: Arc<Mutex<Option<oneshot::Sender<OAuthCallbackQuery>>>>,
}

trait SecretStore: Send + Sync {
    fn backend_label(&self) -> &'static str;
    fn backend_location(&self) -> Option<String> {
        None
    }
    fn read_tokens(&self) -> std::result::Result<Option<StoredTokens>, SecretStoreError>;
    fn write_tokens(&self, tokens: &StoredTokens) -> std::result::Result<(), SecretStoreError>;
}

#[derive(Debug, Default, Clone, Copy)]
struct RuntimeSecretStore;

impl RuntimeSecretStore {
    pub fn from_config(config: &Config) -> SecretBackend {
        match config.oura.secret_backend {
            OuraSecretBackend::Keyring => SecretBackend::Keyring(Self),
            OuraSecretBackend::File => {
                SecretBackend::File(FileSecretStore::new(config.oura.secret_file.clone()))
            }
        }
    }

    fn entry() -> std::result::Result<keyring::Entry, SecretStoreError> {
        keyring::Entry::new(SECRET_SERVICE_NAME, SECRET_USER_NAME).map_err(Into::into)
    }
}

#[derive(Debug, Clone)]
enum SecretBackend {
    Keyring(RuntimeSecretStore),
    File(FileSecretStore),
}

#[derive(Debug, Clone)]
struct FileSecretStore {
    path: PathBuf,
}

impl FileSecretStore {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[cfg(target_os = "linux")]
fn secure_storage_backend_hint() -> &'static str {
    " On Linux, make sure a Secret Service provider such as gnome-keyring or KeePassXC is running and unlocked, or opt into local file storage with `RINGMASTER_OURA_SECRET_BACKEND=file`."
}

#[cfg(not(target_os = "linux"))]
fn secure_storage_backend_hint() -> &'static str {
    ""
}

fn normalize_keyring_read_error(error: keyring::Error) -> SecretStoreError {
    match error {
        keyring::Error::NoStorageAccess(source) => SecretStoreError::BackendUnavailable(format!(
            "secure storage is locked or unavailable: {source}.{}",
            secure_storage_backend_hint()
        )),
        keyring::Error::PlatformFailure(source) => SecretStoreError::BackendUnavailable(format!(
            "secure storage backend failed: {source}.{}",
            secure_storage_backend_hint()
        )),
        other => SecretStoreError::Keyring(other),
    }
}

fn normalize_keyring_write_error(error: keyring::Error) -> SecretStoreError {
    match error {
        keyring::Error::NoEntry => SecretStoreError::BackendUnavailable(format!(
            "secure storage could not create the token entry.{}",
            secure_storage_backend_hint()
        )),
        keyring::Error::NoStorageAccess(source) => SecretStoreError::BackendUnavailable(format!(
            "secure storage is locked or unavailable: {source}.{}",
            secure_storage_backend_hint()
        )),
        keyring::Error::PlatformFailure(source) => SecretStoreError::BackendUnavailable(format!(
            "secure storage backend failed: {source}.{}",
            secure_storage_backend_hint()
        )),
        other => SecretStoreError::Keyring(other),
    }
}

impl SecretStore for RuntimeSecretStore {
    fn backend_label(&self) -> &'static str {
        "keyring"
    }

    fn read_tokens(&self) -> std::result::Result<Option<StoredTokens>, SecretStoreError> {
        let entry = Self::entry()?;
        let payload = match entry.get_password() {
            Ok(value) => value,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(error) => return Err(normalize_keyring_read_error(error)),
        };

        serde_json::from_str(&payload).map(Some).map_err(|error| {
            SecretStoreError::BackendUnavailable(format!(
                "stored session payload is invalid JSON: {error}"
            ))
        })
    }

    fn write_tokens(&self, tokens: &StoredTokens) -> std::result::Result<(), SecretStoreError> {
        let entry = Self::entry()?;
        let payload = serde_json::to_string(tokens).map_err(|error| {
            SecretStoreError::BackendUnavailable(format!(
                "failed to encode stored tokens for secure storage: {error}"
            ))
        })?;
        entry
            .set_password(&payload)
            .map_err(normalize_keyring_write_error)?;
        Ok(())
    }
}

impl SecretStore for SecretBackend {
    fn backend_label(&self) -> &'static str {
        match self {
            Self::Keyring(store) => store.backend_label(),
            Self::File(store) => store.backend_label(),
        }
    }

    fn backend_location(&self) -> Option<String> {
        match self {
            Self::Keyring(store) => store.backend_location(),
            Self::File(store) => store.backend_location(),
        }
    }

    fn read_tokens(&self) -> std::result::Result<Option<StoredTokens>, SecretStoreError> {
        match self {
            Self::Keyring(store) => store.read_tokens(),
            Self::File(store) => store.read_tokens(),
        }
    }

    fn write_tokens(&self, tokens: &StoredTokens) -> std::result::Result<(), SecretStoreError> {
        match self {
            Self::Keyring(store) => store.write_tokens(tokens),
            Self::File(store) => store.write_tokens(tokens),
        }
    }
}

#[cfg(unix)]
fn set_owner_only_permissions(path: &Path, mode: u32) -> std::result::Result<(), SecretStoreError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|source| {
        SecretStoreError::BackendUnavailable(format!(
            "failed to apply private permissions to `{}`: {source}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn set_owner_only_permissions(
    _path: &Path,
    _mode: u32,
) -> std::result::Result<(), SecretStoreError> {
    Ok(())
}

impl SecretStore for FileSecretStore {
    fn backend_label(&self) -> &'static str {
        "file"
    }

    fn backend_location(&self) -> Option<String> {
        Some(self.path.display().to_string())
    }

    fn read_tokens(&self) -> std::result::Result<Option<StoredTokens>, SecretStoreError> {
        match fs::read_to_string(&self.path) {
            Ok(payload) => serde_json::from_str(&payload).map(Some).map_err(|error| {
                SecretStoreError::BackendUnavailable(format!(
                    "stored session payload at `{}` is invalid JSON: {error}",
                    self.path.display()
                ))
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(SecretStoreError::BackendUnavailable(format!(
                "failed to read token file `{}`: {source}",
                self.path.display()
            ))),
        }
    }

    fn write_tokens(&self, tokens: &StoredTokens) -> std::result::Result<(), SecretStoreError> {
        let payload = serde_json::to_string(tokens).map_err(|error| {
            SecretStoreError::BackendUnavailable(format!(
                "failed to encode stored tokens for local file storage: {error}"
            ))
        })?;
        let parent = self.path.parent().ok_or_else(|| {
            SecretStoreError::BackendUnavailable(format!(
                "token file path `{}` does not have a parent directory",
                self.path.display()
            ))
        })?;
        fs::create_dir_all(parent).map_err(|source| {
            SecretStoreError::BackendUnavailable(format!(
                "failed to create token directory `{}`: {source}",
                parent.display()
            ))
        })?;
        set_owner_only_permissions(parent, 0o700)?;

        let mut options = OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&self.path).map_err(|source| {
            SecretStoreError::BackendUnavailable(format!(
                "failed to open token file `{}` for writing: {source}",
                self.path.display()
            ))
        })?;
        file.write_all(payload.as_bytes()).map_err(|source| {
            SecretStoreError::BackendUnavailable(format!(
                "failed to write token file `{}`: {source}",
                self.path.display()
            ))
        })?;
        file.flush().map_err(|source| {
            SecretStoreError::BackendUnavailable(format!(
                "failed to flush token file `{}`: {source}",
                self.path.display()
            ))
        })?;
        set_owner_only_permissions(&self.path, 0o600)?;
        Ok(())
    }
}

pub fn inspect_auth(config: &Config, store: &Store) -> Result<AuthStatus> {
    let secret_store = RuntimeSecretStore::from_config(config);
    inspect_auth_with_secret_store(config, store, &secret_store)
}

pub async fn login(config: &Config, store: &Store) -> Result<LoginReport> {
    let secret_store = RuntimeSecretStore::from_config(config);
    login_with_secret_store(config, store, &secret_store).await
}

pub async fn ensure_authorized_session(
    config: &Config,
    store: &Store,
) -> Result<AuthorizedSession> {
    let secret_store = RuntimeSecretStore::from_config(config);
    ensure_authorized_session_with_secret_store(config, store, &secret_store).await
}

fn inspect_auth_with_secret_store(
    config: &Config,
    store: &Store,
    secret_store: &dyn SecretStore,
) -> Result<AuthStatus> {
    let session = store.auth().get(OURA_PROVIDER)?;
    let mut last_error = session
        .as_ref()
        .and_then(|record| record.last_error.clone());
    let tokens = match secret_store.read_tokens() {
        Ok(tokens) => tokens,
        Err(error) => {
            if last_error.is_none() {
                last_error = Some(secret_store_problem(error.to_string()));
            }
            None
        }
    };
    let granted_scopes = session
        .as_ref()
        .map(|record| record.granted_scopes.clone())
        .unwrap_or_default();

    Ok(AuthStatus {
        configured: config.oura.client_configured(),
        callback_url: config.oura.callback_url(),
        requested_scopes: config.oura.requested_scopes.clone(),
        granted_scopes: granted_scopes.clone(),
        missing_fields: config.oura.missing_fields(),
        capability_report: CapabilityReport::from_scopes(
            &config.oura.requested_scopes,
            &granted_scopes,
        ),
        auth_timeout_secs: config.oura.auth_timeout_secs,
        secret_backend: secret_store.backend_label().to_owned(),
        access_token_stored: tokens
            .as_ref()
            .is_some_and(|stored| !stored.access_token.trim().is_empty()),
        refresh_token_stored: tokens
            .as_ref()
            .and_then(|stored| stored.refresh_token.as_ref())
            .is_some_and(|token| !token.trim().is_empty()),
        access_token_expires_at: session
            .as_ref()
            .and_then(|record| record.access_token_expires_at.clone()),
        last_authenticated_at: session
            .as_ref()
            .and_then(|record| record.last_authenticated_at.clone()),
        last_refresh_at: session
            .as_ref()
            .and_then(|record| record.last_refresh_at.clone()),
        account_id: session
            .as_ref()
            .and_then(|record| record.account_id.clone()),
        account_email: session
            .as_ref()
            .and_then(|record| record.account_email.clone()),
        last_error,
    })
}

async fn login_with_secret_store(
    config: &Config,
    store: &Store,
    secret_store: &dyn SecretStore,
) -> Result<LoginReport> {
    let auth_status = inspect_auth_with_secret_store(config, store, secret_store)?;
    let listener_plan = LoopbackListenerPlan {
        bind_address: config.oura.callback_bind,
        callback_path: config.oura.callback_path.clone(),
        timeout_secs: config.oura.auth_timeout_secs,
    };

    if !config.oura.client_configured() {
        return Ok(LoginReport {
            status: LoginStatus::ConfigMissing,
            auth_status,
            authorization_url: None,
            listener_plan,
            notes: vec![
                "Set `RINGMASTER_OURA_CLIENT_ID` and `RINGMASTER_OURA_CLIENT_SECRET` before starting OAuth."
                    .to_owned(),
                "The client secret is intentionally env-only so it does not need to live in plaintext config."
                    .to_owned(),
            ],
        });
    }

    let listener = TcpListener::bind(config.oura.callback_bind)
        .await
        .map_err(|error| AuthError::CallbackListener(error.to_string()))?;
    let bound_address = listener
        .local_addr()
        .map_err(|error| AuthError::CallbackListener(error.to_string()))?;
    let listener_plan = LoopbackListenerPlan {
        bind_address: bound_address,
        callback_path: config.oura.callback_path.clone(),
        timeout_secs: config.oura.auth_timeout_secs,
    };
    let callback_url = config.oura.callback_url_for_bind_address(bound_address);
    let oauth_client = build_oauth_client(config, &callback_url)?;
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorization_url, csrf_token) = oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scopes(config.oura.requested_scopes.iter().cloned().map(Scope::new))
        .set_pkce_challenge(pkce_challenge)
        .url();

    println!(
        "Open this URL in your browser to authorize ringmaster:\n{}\n",
        authorization_url
    );

    let callback = wait_for_callback(listener, &listener_plan).await?;
    let http_client = oauth_http_client()?;
    let outcome = evaluate_callback(&callback, csrf_token.secret())?;

    match outcome {
        CallbackOutcome::Denied(problem) => {
            persist_problem(store, problem.clone())?;
            let auth_status = inspect_auth_with_secret_store(config, store, secret_store)?;
            Ok(LoginReport {
                status: LoginStatus::Denied,
                auth_status,
                authorization_url: Some(authorization_url.to_string()),
                listener_plan,
                notes: vec![
                    "Authorization was denied, so the local session was left unchanged.".to_owned(),
                    format!("{problem}"),
                ],
            })
        }
        CallbackOutcome::AuthorizedCode(code) => {
            let exchanged = exchange_authorization_code(
                &oauth_client,
                &http_client,
                code,
                pkce_verifier,
                &config.oura.requested_scopes,
            )
            .await?;
            persist_authorized_session(store, secret_store, &exchanged, None, false)?;
            let auth_status = inspect_auth_with_secret_store(config, store, secret_store)?;
            let missing_scopes = auth_status.capability_report.missing_scope_names();
            let status = if missing_scopes.is_empty() {
                LoginStatus::Authorized
            } else {
                LoginStatus::PartialGrant
            };
            let mut notes = vec![
                "OAuth code exchange completed and the session metadata was persisted locally."
                    .to_owned(),
                format!(
                    "Secure token storage backend: {}",
                    auth_status.secret_backend
                ),
            ];
            if let Some(location) = secret_store.backend_location() {
                notes.push(format!("Secure token file: {location}"));
            }
            if missing_scopes.is_empty() {
                notes.push("All requested scopes were granted.".to_owned());
            } else {
                notes.push(format!(
                    "Some requested scopes were not granted: {}.",
                    missing_scopes.join(", ")
                ));
            }

            Ok(LoginReport {
                status,
                auth_status,
                authorization_url: Some(authorization_url.to_string()),
                listener_plan,
                notes,
            })
        }
    }
}

async fn ensure_authorized_session_with_secret_store(
    config: &Config,
    store: &Store,
    secret_store: &dyn SecretStore,
) -> Result<AuthorizedSession> {
    let session = store.auth().get(OURA_PROVIDER)?.ok_or_else(|| {
        AuthError::OAuthFlow("no persisted Oura auth session is available".to_owned())
    })?;
    let stored_tokens = secret_store.read_tokens().map_err(AuthError::from)?;
    let mut tokens = stored_tokens.ok_or(AuthError::MissingAccessToken)?;

    let should_refresh = tokens.refresh_token.is_some()
        && (tokens.access_token.trim().is_empty()
            || access_token_is_stale(session.access_token_expires_at.as_deref())?);

    let session = if should_refresh {
        let refresh_token = tokens
            .refresh_token
            .clone()
            .ok_or(AuthError::MissingRefreshToken)?;
        let oauth_client = build_oauth_client(config, &config.oura.callback_url())?;
        let http_client = oauth_http_client()?;
        let refreshed = refresh_access_token(&oauth_client, &http_client, refresh_token).await?;
        persist_authorized_session(
            store,
            secret_store,
            &refreshed,
            session.account_email.clone(),
            true,
        )?;
        tokens = StoredTokens {
            access_token: refreshed.access_token,
            refresh_token: refreshed.refresh_token,
        };
        store.auth().get(OURA_PROVIDER)?.ok_or_else(|| {
            AuthError::OAuthFlow("auth session disappeared after refresh".to_owned())
        })?
    } else {
        session
    };

    Ok(AuthorizedSession {
        access_token: tokens.access_token,
        granted_scopes: session.granted_scopes,
        access_token_expires_at: session.access_token_expires_at,
        account_id: session.account_id,
        account_email: session.account_email,
    })
}

fn build_oauth_client(config: &Config, callback_url: &str) -> Result<OAuthClient> {
    let client_id = config
        .oura
        .client_id
        .clone()
        .ok_or(AuthError::MissingClientCredentials)?;
    let client_secret = config
        .oura
        .client_secret
        .clone()
        .ok_or(AuthError::MissingClientCredentials)?;
    let auth_url = AuthUrl::new(config.oura.authorize_url.clone()).map_err(|error| {
        AuthError::InvalidOAuthConfig(format!("invalid authorize URL: {error}"))
    })?;
    let token_url = TokenUrl::new(config.oura.token_url.clone())
        .map_err(|error| AuthError::InvalidOAuthConfig(format!("invalid token URL: {error}")))?;
    let redirect_url = RedirectUrl::new(callback_url.to_owned())
        .map_err(|error| AuthError::InvalidOAuthConfig(format!("invalid callback URL: {error}")))?;

    Ok(BasicClient::new(ClientId::new(client_id))
        .set_client_secret(ClientSecret::new(client_secret))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_url))
}

fn oauth_http_client() -> Result<OAuthHttpClient> {
    OAuthHttpClient::builder()
        .redirect(Policy::none())
        .build()
        .map_err(|error| {
            AuthError::OAuthFlow(format!("failed to build OAuth HTTP client: {error}")).into()
        })
}

async fn wait_for_callback(
    listener: TcpListener,
    plan: &LoopbackListenerPlan,
) -> Result<OAuthCallbackQuery> {
    let (callback_sender, callback_receiver) = oneshot::channel::<OAuthCallbackQuery>();
    let (shutdown_sender, shutdown_receiver) = oneshot::channel::<()>();
    let state = CallbackState {
        sender: Arc::new(Mutex::new(Some(callback_sender))),
    };
    let router = Router::new()
        .route(&plan.callback_path, get(callback_handler))
        .with_state(state);
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = shutdown_receiver.await;
            })
            .await
    });

    let callback =
        tokio::time::timeout(StdDuration::from_secs(plan.timeout_secs), callback_receiver)
            .await
            .map_err(|_| AuthError::CallbackTimeout(plan.timeout_secs))?
            .map_err(|_| {
                AuthError::CallbackListener(
                    "loopback callback channel closed unexpectedly".to_owned(),
                )
            })?;

    let _ = shutdown_sender.send(());
    match server.await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => return Err(AuthError::CallbackListener(error.to_string()).into()),
        Err(error) => {
            return Err(AuthError::CallbackListener(format!(
                "loopback callback server join failed: {error}"
            ))
            .into());
        }
    }

    Ok(callback)
}

async fn callback_handler(
    State(state): State<CallbackState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Html<String> {
    if let Ok(mut sender) = state.sender.lock()
        && let Some(sender) = sender.take()
    {
        let _ = sender.send(query.clone());
    }

    let body = match query.error.as_deref() {
        Some("access_denied") => {
            "ringmaster authorization was denied. You can return to the terminal.".to_owned()
        }
        Some(error) => format!(
            "ringmaster OAuth callback returned an error: {} ({})",
            error,
            query
                .error_description
                .as_deref()
                .unwrap_or("no description provided")
        ),
        None if query.code.is_some() => {
            "ringmaster captured the authorization code. You can return to the terminal.".to_owned()
        }
        None => "ringmaster reached the callback path without an authorization code.".to_owned(),
    };

    Html(body)
}

fn evaluate_callback(query: &OAuthCallbackQuery, expected_state: &str) -> Result<CallbackOutcome> {
    if let Some(error) = &query.error {
        return Ok(CallbackOutcome::Denied(OuraProblem::oauth(
            None,
            "authorization was denied",
            query.error_description.clone(),
            Some(error.clone()),
            query.error_description.clone(),
        )));
    }

    let state = query.state.as_deref().ok_or(AuthError::StateMismatch)?;
    if state != expected_state {
        return Err(AuthError::StateMismatch.into());
    }

    let code = query
        .code
        .clone()
        .ok_or(AuthError::MissingAuthorizationCode)?;
    Ok(CallbackOutcome::AuthorizedCode(code))
}

async fn exchange_authorization_code(
    oauth_client: &OAuthClient,
    http_client: &OAuthHttpClient,
    code: String,
    pkce_verifier: PkceCodeVerifier,
    requested_scopes: &[String],
) -> Result<ExchangedTokenSet> {
    let token_response = oauth_client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(http_client)
        .await
        .map_err(map_oauth_exchange_error)?;

    exchanged_token_set(token_response, requested_scopes)
}

async fn refresh_access_token(
    oauth_client: &OAuthClient,
    http_client: &OAuthHttpClient,
    refresh_token: String,
) -> Result<ExchangedTokenSet> {
    let requested_scopes = Vec::new();
    let token_response = oauth_client
        .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token))
        .request_async(http_client)
        .await
        .map_err(map_oauth_exchange_error)?;

    let exchanged = exchanged_token_set(token_response, &requested_scopes)?;
    if exchanged.refresh_token.is_none() {
        return Err(AuthError::OAuthFlow(
            "refresh response did not include a replacement refresh token".to_owned(),
        )
        .into());
    }

    Ok(exchanged)
}

fn exchanged_token_set(
    token_response: oauth2::basic::BasicTokenResponse,
    requested_scopes: &[String],
) -> Result<ExchangedTokenSet> {
    let granted_scopes = token_response
        .scopes()
        .map(|scopes| {
            scopes
                .iter()
                .map(|scope| scope.as_ref().to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| requested_scopes.to_vec());

    Ok(ExchangedTokenSet {
        access_token: token_response.access_token().secret().to_owned(),
        refresh_token: token_response
            .refresh_token()
            .map(|token| token.secret().to_owned()),
        token_type: token_response.token_type().as_ref().to_owned(),
        granted_scopes,
        access_token_expires_at: expires_at_rfc3339(token_response.expires_in())?,
    })
}

fn persist_problem(store: &Store, problem: OuraProblem) -> Result<()> {
    let existing = store.auth().get(OURA_PROVIDER)?;
    let updated_at = now_rfc3339()?;
    let record = existing.unwrap_or_else(|| AuthSessionRecord {
        provider: OURA_PROVIDER.to_owned(),
        account_id: None,
        account_email: None,
        token_type: "Bearer".to_owned(),
        granted_scopes: Vec::new(),
        access_token_expires_at: None,
        last_authenticated_at: None,
        last_refresh_at: None,
        last_error: None,
        updated_at: updated_at.clone(),
    });

    store.auth().upsert(&AuthSessionRecord {
        last_error: Some(problem),
        updated_at,
        ..record
    })?;
    Ok(())
}

fn persist_authorized_session(
    store: &Store,
    secret_store: &dyn SecretStore,
    exchanged: &ExchangedTokenSet,
    account_email: Option<String>,
    refreshed: bool,
) -> Result<()> {
    secret_store
        .write_tokens(&StoredTokens {
            access_token: exchanged.access_token.clone(),
            refresh_token: exchanged.refresh_token.clone(),
        })
        .map_err(AuthError::from)?;
    let now = now_rfc3339()?;
    let existing = store.auth().get(OURA_PROVIDER)?;
    let last_authenticated_at = existing
        .as_ref()
        .and_then(|record| record.last_authenticated_at.clone())
        .or_else(|| Some(now.clone()));
    let account_id = existing
        .as_ref()
        .and_then(|record| record.account_id.clone());
    let account_email = account_email.or_else(|| {
        existing
            .as_ref()
            .and_then(|record| record.account_email.clone())
    });
    store.auth().upsert(&AuthSessionRecord {
        provider: OURA_PROVIDER.to_owned(),
        account_id,
        account_email,
        token_type: exchanged.token_type.clone(),
        granted_scopes: exchanged.granted_scopes.clone(),
        access_token_expires_at: exchanged.access_token_expires_at.clone(),
        last_authenticated_at: if refreshed {
            last_authenticated_at
        } else {
            Some(now.clone())
        },
        last_refresh_at: if refreshed { Some(now.clone()) } else { None },
        last_error: None,
        updated_at: now,
    })?;
    Ok(())
}

fn access_token_is_stale(expires_at: Option<&str>) -> Result<bool> {
    let Some(expires_at) = expires_at else {
        return Ok(false);
    };
    let expires_at = OffsetDateTime::parse(expires_at, &Rfc3339).map_err(|error| {
        AuthError::OAuthFlow(format!("stored access token expiry is invalid: {error}"))
    })?;
    Ok(expires_at <= OffsetDateTime::now_utc() + Duration::seconds(ACCESS_TOKEN_REFRESH_SKEW_SECS))
}

fn expires_at_rfc3339(expires_in: Option<StdDuration>) -> Result<Option<String>> {
    let Some(expires_in) = expires_in else {
        return Ok(None);
    };
    let seconds = i64::try_from(expires_in.as_secs()).map_err(|error| {
        AuthError::OAuthFlow(format!("token expiry duration overflowed: {error}"))
    })?;
    let expires_at = OffsetDateTime::now_utc() + Duration::seconds(seconds);
    expires_at.format(&Rfc3339).map(Some).map_err(|error| {
        AuthError::OAuthFlow(format!("failed to format token expiry: {error}")).into()
    })
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc().format(&Rfc3339).map_err(|error| {
        AuthError::OAuthFlow(format!("failed to format auth timestamp: {error}")).into()
    })
}

fn secret_store_problem(detail: String) -> OuraProblem {
    OuraProblem::new(None, "secure token storage is unavailable", Some(detail))
}

fn map_oauth_exchange_error(
    error: oauth2::RequestTokenError<
        oauth2::HttpClientError<OAuthReqwestError>,
        BasicErrorResponse,
    >,
) -> crate::error::RingmasterError {
    match error {
        oauth2::RequestTokenError::ServerResponse(server_error) => {
            AuthError::Problem(OuraProblem::oauth(
                None,
                "oauth server returned an error",
                server_error.error_description().cloned(),
                Some(server_error.error().to_string()),
                server_error.error_description().cloned(),
            ))
            .into()
        }
        oauth2::RequestTokenError::Request(error) => match error {
            oauth2::HttpClientError::Reqwest(error) => {
                AuthError::OAuthFlow(format!("OAuth HTTP client request failed: {error}")).into()
            }
            oauth2::HttpClientError::Http(error) => AuthError::OAuthFlow(format!(
                "OAuth HTTP client could not build a valid request: {error}"
            ))
            .into(),
            oauth2::HttpClientError::Io(error) => AuthError::OAuthFlow(format!(
                "OAuth HTTP client encountered an I/O failure: {error}"
            ))
            .into(),
            oauth2::HttpClientError::Other(message) => AuthError::OAuthFlow(message).into(),
            _ => AuthError::OAuthFlow(
                "OAuth HTTP client returned an unsupported transport error".to_owned(),
            )
            .into(),
        },
        oauth2::RequestTokenError::Parse(error, body) => AuthError::OAuthFlow(format!(
            "failed to parse OAuth server response: {error}; body={}",
            String::from_utf8_lossy(&body)
        ))
        .into(),
        oauth2::RequestTokenError::Other(message) => AuthError::OAuthFlow(message).into(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::collections::VecDeque;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use axum::{Json, extract::Form, routing::post};
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;
    use crate::config::{
        AppPaths, DEFAULT_OURA_API_BASE_URL, DEFAULT_OURA_AUTHORIZE_URL, LoggingConfig, OuraConfig,
        RefreshConfig, WebhookConfig,
    };
    use crate::webhook::default_desired_subscriptions;

    #[derive(Debug, Default)]
    struct MemorySecretStore {
        tokens: Mutex<Option<StoredTokens>>,
    }

    impl SecretStore for MemorySecretStore {
        fn backend_label(&self) -> &'static str {
            "memory"
        }

        fn read_tokens(&self) -> std::result::Result<Option<StoredTokens>, SecretStoreError> {
            Ok(self
                .tokens
                .lock()
                .map_err(|_| {
                    SecretStoreError::BackendUnavailable(
                        "memory secret store lock poisoned".to_owned(),
                    )
                })?
                .clone())
        }

        fn write_tokens(&self, tokens: &StoredTokens) -> std::result::Result<(), SecretStoreError> {
            *self.tokens.lock().map_err(|_| {
                SecretStoreError::BackendUnavailable("memory secret store lock poisoned".to_owned())
            })? = Some(tokens.clone());
            Ok(())
        }
    }

    fn test_config(token_url: String) -> Config {
        Config {
            app_name: "ringmaster",
            paths: AppPaths::from_roots(
                PathBuf::from("/home/tester"),
                PathBuf::from("/tmp/config"),
                PathBuf::from("/tmp/state"),
                PathBuf::from("/tmp/cache"),
            )
            .unwrap(),
            logging: LoggingConfig {
                filter: "ringmaster=debug".to_owned(),
            },
            oura: OuraConfig {
                client_id: Some("test-client".to_owned()),
                client_secret: Some("test-secret".to_owned()),
                authorize_url: DEFAULT_OURA_AUTHORIZE_URL.to_owned(),
                token_url,
                api_base_url: DEFAULT_OURA_API_BASE_URL.to_owned(),
                secret_backend: crate::config::OuraSecretBackend::Keyring,
                secret_file: PathBuf::from("/tmp/state/ringmaster/secrets/oura-tokens.json"),
                callback_bind: "127.0.0.1:0".parse().unwrap(),
                callback_path: "/callback".to_owned(),
                requested_scopes: vec![
                    "personal".to_owned(),
                    "daily".to_owned(),
                    "heartrate".to_owned(),
                    "workout".to_owned(),
                    "tag".to_owned(),
                    "session".to_owned(),
                ],
                auth_timeout_secs: 5,
            },
            refresh: RefreshConfig {
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
                daily_history_days: 90,
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
            },
            webhook: WebhookConfig {
                bind: "127.0.0.1:8799".parse().unwrap(),
                path: "/webhooks/oura".to_owned(),
                public_base_url: Some("https://example.test".to_owned()),
                verification_token: Some("verify-me".to_owned()),
                signature_tolerance_secs: 300,
                heartbeat_secs: 15,
                renewal_lead_secs: 7 * 24 * 60 * 60,
                subscriptions: default_desired_subscriptions(),
            },
            ai: crate::config::AiConfig::default(),
        }
    }

    #[test]
    fn callback_denial_is_reported_without_panicking() {
        let result = evaluate_callback(
            &OAuthCallbackQuery {
                code: None,
                state: Some("abc".to_owned()),
                error: Some("access_denied".to_owned()),
                error_description: Some("user clicked cancel".to_owned()),
            },
            "abc",
        )
        .unwrap();

        assert_eq!(
            result,
            CallbackOutcome::Denied(OuraProblem::oauth(
                None,
                "authorization was denied",
                Some("user clicked cancel".to_owned()),
                Some("access_denied".to_owned()),
                Some("user clicked cancel".to_owned()),
            ))
        );
    }

    #[test]
    fn callback_state_mismatch_is_rejected() {
        let error = evaluate_callback(
            &OAuthCallbackQuery {
                code: Some("auth-code".to_owned()),
                state: Some("wrong".to_owned()),
                error: None,
                error_description: None,
            },
            "expected",
        )
        .expect_err("mismatched state should fail");

        assert!(matches!(
            error,
            crate::error::RingmasterError::Auth(AuthError::StateMismatch)
        ));
    }

    #[tokio::test]
    async fn authorization_code_exchange_persists_session_metadata() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let app = Router::new().route(
                "/token",
                post(|| async move {
                    Json(json!({
                        "access_token": "access-1",
                        "refresh_token": "refresh-1",
                        "token_type": "Bearer",
                        "expires_in": 3600,
                        "scope": "personal daily heartrate"
                    }))
                }),
            );
            axum::serve(listener, app).await.unwrap();
        });

        let config = test_config(format!("http://{address}/token"));
        let oauth_client =
            build_oauth_client(&config, "http://127.0.0.1:8788/callback").expect("oauth client");
        let http_client = oauth_http_client().expect("http client");
        let exchanged = exchange_authorization_code(
            &oauth_client,
            &http_client,
            "code-1".to_owned(),
            PkceCodeVerifier::new("verifier".to_owned()),
            &config.oura.requested_scopes,
        )
        .await
        .expect("code exchange should succeed");

        let store = Store::open_in_memory().expect("store should open");
        let secrets = MemorySecretStore::default();
        persist_authorized_session(&store, &secrets, &exchanged, None, false)
            .expect("session should persist");
        let auth_status =
            inspect_auth_with_secret_store(&config, &store, &secrets).expect("inspect auth");

        assert!(auth_status.access_token_stored);
        assert!(auth_status.refresh_token_stored);
        assert_eq!(
            auth_status.granted_scopes,
            vec![
                "personal".to_owned(),
                "daily".to_owned(),
                "heartrate".to_owned()
            ]
        );

        server.abort();
    }

    #[tokio::test]
    async fn refresh_rotation_replaces_stored_tokens() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let grants = Arc::new(Mutex::new(VecDeque::from([json!({
            "access_token": "access-2",
            "refresh_token": "refresh-2",
            "token_type": "Bearer",
            "expires_in": 3600
        })])));
        let grants_clone = Arc::clone(&grants);
        let server = tokio::spawn(async move {
            let app = Router::new().route(
                "/token",
                post(
                    move |Form(_): Form<std::collections::HashMap<String, String>>| {
                        let grants_clone = Arc::clone(&grants_clone);
                        async move {
                            let payload = grants_clone
                                .lock()
                                .unwrap()
                                .pop_front()
                                .expect("refresh response should exist");
                            Json(payload)
                        }
                    },
                ),
            );
            axum::serve(listener, app).await.unwrap();
        });

        let config = test_config(format!("http://{address}/token"));
        let store = Store::open_in_memory().expect("store should open");
        let secrets = MemorySecretStore::default();
        secrets
            .write_tokens(&StoredTokens {
                access_token: "expired-access".to_owned(),
                refresh_token: Some("refresh-1".to_owned()),
            })
            .expect("seed tokens");
        store
            .auth()
            .upsert(&AuthSessionRecord {
                provider: OURA_PROVIDER.to_owned(),
                account_id: None,
                account_email: None,
                token_type: "Bearer".to_owned(),
                granted_scopes: vec!["daily".to_owned(), "heartrate".to_owned()],
                access_token_expires_at: Some("2020-01-01T00:00:00Z".to_owned()),
                last_authenticated_at: Some("2020-01-01T00:00:00Z".to_owned()),
                last_refresh_at: None,
                last_error: None,
                updated_at: "2020-01-01T00:00:00Z".to_owned(),
            })
            .expect("seed auth session");
        let session = ensure_authorized_session_with_secret_store(&config, &store, &secrets)
            .await
            .expect("refresh should succeed");

        assert_eq!(session.access_token, "access-2");
        let stored = secrets
            .read_tokens()
            .expect("tokens should read")
            .expect("tokens stored");
        assert_eq!(stored.refresh_token.as_deref(), Some("refresh-2"));
        let auth = store
            .auth()
            .get(OURA_PROVIDER)
            .expect("session read")
            .expect("session present");
        assert!(auth.last_refresh_at.is_some());

        server.abort();
    }

    #[test]
    fn keyring_write_no_entry_maps_to_backend_unavailable() {
        let error = normalize_keyring_write_error(keyring::Error::NoEntry);

        match error {
            SecretStoreError::BackendUnavailable(detail) => {
                assert!(detail.contains("could not create the token entry"));
            }
            other => panic!("expected backend unavailable error, got {other}"),
        }
    }

    #[test]
    fn keyring_read_no_storage_access_maps_to_backend_unavailable() {
        let error = normalize_keyring_read_error(keyring::Error::NoStorageAccess(Box::new(
            std::io::Error::other("locked"),
        )));

        match error {
            SecretStoreError::BackendUnavailable(detail) => {
                assert!(detail.contains("locked or unavailable"));
            }
            other => panic!("expected backend unavailable error, got {other}"),
        }
    }

    #[test]
    fn file_secret_store_round_trips_tokens_with_private_permissions() {
        let tempdir = tempdir()
            .unwrap_or_else(|error| panic!("tempdir should succeed for file backend: {error}"));
        let path = tempdir.path().join("secrets").join("oura-tokens.json");
        let store = FileSecretStore::new(path.clone());
        let tokens = StoredTokens {
            access_token: "access-token".to_owned(),
            refresh_token: Some("refresh-token".to_owned()),
        };

        store
            .write_tokens(&tokens)
            .unwrap_or_else(|error| panic!("writing token file should succeed: {error}"));

        let loaded = store
            .read_tokens()
            .unwrap_or_else(|error| panic!("reading token file should succeed: {error}"))
            .unwrap_or_else(|| panic!("token file should contain a payload"));
        assert_eq!(loaded, tokens);

        #[cfg(unix)]
        {
            let file_mode = fs::metadata(&path)
                .unwrap_or_else(|error| panic!("metadata should be readable: {error}"))
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(file_mode, 0o600);
        }

        #[cfg(unix)]
        {
            let parent_mode = fs::metadata(
                path.parent()
                    .unwrap_or_else(|| panic!("token file should have a parent directory")),
            )
            .unwrap_or_else(|error| panic!("parent directory metadata should be readable: {error}"))
            .permissions()
            .mode()
                & 0o777;
            assert_eq!(parent_mode, 0o700);
        }
    }
}
