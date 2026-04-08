use std::net::SocketAddr;

use axum::{Router, extract::Query, response::Html, routing::get};
use oauth2::{
    AuthUrl, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl, Scope, TokenUrl,
    basic::BasicClient,
};
use serde::Deserialize;

use crate::config::Config;
use crate::error::{Result, RingmasterError};
use crate::oura::models::{AuthStatus, CapabilityReport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginPlan {
    pub auth_status: AuthStatus,
    pub authorization_url: Option<String>,
    pub listener_plan: LoopbackListenerPlan,
    pub notes: Vec<String>,
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

pub fn inspect_auth(config: &Config) -> AuthStatus {
    AuthStatus {
        configured: config.oura.client_configured(),
        callback_url: config.oura.callback_url(),
        requested_scopes: config.oura.requested_scopes.clone(),
        granted_scopes: config.oura.granted_scopes.clone(),
        missing_fields: config.oura.missing_fields(),
        capability_report: CapabilityReport::from_scopes(
            &config.oura.requested_scopes,
            &config.oura.granted_scopes,
        ),
        auth_timeout_secs: config.oura.auth_timeout_secs,
    }
}

pub async fn prepare_login(config: &Config) -> Result<LoginPlan> {
    let auth_status = inspect_auth(config);
    let listener_plan = LoopbackListenerPlan {
        bind_address: config.oura.callback_bind,
        callback_path: config.oura.callback_path.clone(),
        timeout_secs: config.oura.auth_timeout_secs,
    };

    if !auth_status.configured {
        return Ok(LoginPlan {
            auth_status,
            authorization_url: None,
            listener_plan,
            notes: vec![
                "Set `oura.client_id` and `oura.client_secret` in config.toml or environment."
                    .to_owned(),
                "The loopback callback path is scaffolded, but token persistence is intentionally deferred."
                    .to_owned(),
            ],
        });
    }

    let client_id = config
        .oura
        .client_id
        .clone()
        .ok_or_else(|| RingmasterError::Auth("missing Oura client_id".to_owned()))?;
    let client_secret = config
        .oura
        .client_secret
        .clone()
        .ok_or_else(|| RingmasterError::Auth("missing Oura client_secret".to_owned()))?;
    let auth_url = AuthUrl::new(config.oura.authorize_url.clone())
        .map_err(|error| RingmasterError::Auth(format!("invalid authorize URL: {error}")))?;
    let token_url = TokenUrl::new(config.oura.token_url.clone())
        .map_err(|error| RingmasterError::Auth(format!("invalid token URL: {error}")))?;
    let redirect_url = RedirectUrl::new(config.oura.callback_url())
        .map_err(|error| RingmasterError::Auth(format!("invalid callback URL: {error}")))?;

    let oauth_client = BasicClient::new(ClientId::new(client_id))
        .set_client_secret(ClientSecret::new(client_secret))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_url);
    let (pkce_challenge, _pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorization_url, _csrf_token) = oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scopes(config.oura.requested_scopes.iter().cloned().map(Scope::new))
        .set_pkce_challenge(pkce_challenge)
        .url();

    Ok(LoginPlan {
        auth_status,
        authorization_url: Some(authorization_url.to_string()),
        listener_plan,
        notes: vec![
            "The OAuth authorization URL is ready.".to_owned(),
            "A loopback callback router is scaffolded for the configured callback path.".to_owned(),
            "Token exchange and secure token persistence remain follow-up work for the next milestone."
                .to_owned(),
        ],
    })
}

pub fn build_loopback_router(plan: &LoopbackListenerPlan) -> Router {
    Router::new().route(&plan.callback_path, get(callback_handler))
}

async fn callback_handler(Query(query): Query<OAuthCallbackQuery>) -> Html<String> {
    let body = match query.error {
        Some(error) => {
            let description = query
                .error_description
                .as_deref()
                .unwrap_or("no description provided");
            format!("ringmaster.rs OAuth callback received an error: {error} ({description})")
        }
        None => {
            let code_status = if query.code.is_some() {
                "authorization code captured"
            } else {
                "callback reached without an authorization code"
            };

            format!(
                "ringmaster.rs OAuth callback scaffold reached successfully: {code_status}. You can return to the terminal."
            )
        }
    };

    Html(body)
}
