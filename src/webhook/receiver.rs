use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::path::PathBuf;

use axum::{
    Router,
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tracing::{info, warn};

use crate::config::Config;
use crate::error::{Result, RingmasterError};
use crate::store::Store;
use crate::store::webhook_store::{
    AcceptedWebhookDeliveryInput, AcceptedWebhookDeliveryRecord, AcceptedWebhookDeliveryResult,
    InvalidationInput, InvalidationRecord, RejectedWebhookDeliveryInput, RuntimeHeartbeatRecord,
    now_rfc3339,
};
use crate::webhook::{WebhookEventType, is_supported_data_type};

type HmacSha256 = Hmac<Sha256>;

const HEALTH_PATH: &str = "/healthz";
const READY_PATH: &str = "/readyz";
const RECEIVER_COMPONENT: &str = "webhook.receiver";
const ACCEPTED_NO_INVALIDATION: &str = "accepted_without_invalidation";
const FIXTURE_NOW_RFC3339: &str = "{{now_rfc3339}}";
const FIXTURE_COMPUTED_SIGNATURE: &str = "{{computed_hmac_sha256}}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookServeReport {
    pub bind_address: SocketAddr,
    pub callback_url: Option<String>,
    pub stopped_at: String,
}

struct RejectionContext<'a> {
    request: &'a InboundWebhookRequest,
    received_at: String,
    reason_code: &'a str,
    detail: String,
    signature_timestamp: Option<String>,
    status: StatusCode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookReplayOptions {
    pub fixture: Option<PathBuf>,
    pub delivery_id: Option<i64>,
    pub recent: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookReplayReport {
    pub entries: Vec<WebhookReplayEntry>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookReplayEntry {
    pub source: String,
    pub status: String,
    pub delivery_id: Option<i64>,
    pub invalidation_id: Option<i64>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiverResponse {
    pub status_code: u16,
    pub body: String,
    pub outcome: ReceiverOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiverOutcome {
    VerificationChallenge,
    Accepted {
        delivery_id: i64,
        duplicate: bool,
        invalidation_id: Option<i64>,
        detail: Option<String>,
    },
    Rejected {
        reason_code: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiverSecurityConfig {
    pub verification_token: String,
    pub signature_secret: String,
    pub signature_tolerance_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundWebhookRequest {
    pub method: String,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReplayFixture {
    pub method: String,
    #[serde(default)]
    pub query: BTreeMap<String, String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub body: Option<String>,
    pub body_json: Option<Value>,
    pub signature_secret: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct DeliveryPayload {
    data_type: Option<String>,
    event_type: Option<String>,
    object_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ReceiverState {
    config: Config,
    security: ReceiverSecurityConfig,
}

pub async fn serve(config: &Config) -> Result<WebhookServeReport> {
    let security = security_from_config(config)?;
    let bind_address = config.webhook.bind;
    let state = ReceiverState {
        config: config.clone(),
        security,
    };
    write_heartbeat(
        config,
        "running",
        Some(format!("listening on {}", bind_address)),
    )?;

    let app = Router::new()
        .route(
            &config.webhook.path,
            get(handle_verification).post(handle_delivery),
        )
        .route(HEALTH_PATH, get(handle_health))
        .route(READY_PATH, get(handle_ready))
        .with_state(state);

    let listener = TcpListener::bind(bind_address)
        .await
        .map_err(|error| RingmasterError::io("binding webhook receiver listener", error))?;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let heartbeat_task = tokio::spawn(heartbeat_loop(config.clone(), bind_address, shutdown_rx));

    info!(bind = %bind_address, path = %config.webhook.path, "starting webhook receiver");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                let _ = shutdown_tx.send(true);
            }
        })
        .await
        .map_err(|error| RingmasterError::io("serving webhook receiver", error))?;

    let _ = heartbeat_task.await;
    write_heartbeat(
        config,
        "stopped",
        Some("receiver shut down cleanly".to_owned()),
    )?;
    Ok(WebhookServeReport {
        bind_address,
        callback_url: config.webhook.callback_url(),
        stopped_at: now_rfc3339()?,
    })
}

pub fn process_inbound_request(
    security: &ReceiverSecurityConfig,
    store: &Store,
    request: InboundWebhookRequest,
) -> Result<ReceiverResponse> {
    let received_at = now_rfc3339()?;
    match request.method.to_ascii_uppercase().as_str() {
        "GET" => handle_challenge(security, store, request, received_at),
        "POST" => handle_signed_delivery(security, store, request, received_at),
        other => reject_request(
            store,
            RejectionContext {
                request: &request,
                received_at,
                reason_code: "method_not_allowed",
                detail: format!("unsupported webhook method `{other}`"),
                signature_timestamp: None,
                status: StatusCode::METHOD_NOT_ALLOWED,
            },
        ),
    }
}

pub async fn replay(
    config: &Config,
    store: &Store,
    options: WebhookReplayOptions,
) -> Result<WebhookReplayReport> {
    let selected_sources = usize::from(options.fixture.is_some())
        + usize::from(options.delivery_id.is_some())
        + usize::from(options.recent.is_some());
    if selected_sources != 1 {
        return Err(RingmasterError::Config(
            "webhook replay requires exactly one of --fixture, --delivery-id, or --recent"
                .to_owned(),
        ));
    }

    if let Some(path) = options.fixture {
        let fixture = load_fixture(&path)?;
        let security = fixture_security(config, &fixture)?;
        let request = fixture_into_request(fixture, Some(security.signature_secret.as_str()))?;
        let response = process_inbound_request(&security, store, request)?;
        return Ok(WebhookReplayReport {
            entries: vec![entry_from_receiver_response(path.display().to_string(), response)],
            notes: vec![
                "Fixture replay exercised the same verification, persistence, and enqueue path as the live receiver."
                    .to_owned(),
            ],
        });
    }

    if let Some(delivery_id) = options.delivery_id {
        let record = store.webhook().get_delivery(delivery_id)?.ok_or_else(|| {
            RingmasterError::Config(format!(
                "webhook delivery {delivery_id} was not found in local storage"
            ))
        })?;
        let entry = replay_stored_delivery(store, &record)?;
        return Ok(WebhookReplayReport {
            entries: vec![entry],
            notes: vec![
                "Stored-delivery replay trusts the already accepted audit log and re-enqueues invalidations without re-verifying the original signature."
                    .to_owned(),
            ],
        });
    }

    let recent_limit = options.recent.unwrap_or(1);
    let mut entries = Vec::new();
    let mut deliveries = store.webhook().list_recent_deliveries(recent_limit)?;
    deliveries.reverse();
    for record in &deliveries {
        entries.push(replay_stored_delivery(store, record)?);
    }

    Ok(WebhookReplayReport {
        entries,
        notes: vec![
            "Stored-delivery replay trusts the already accepted audit log and re-enqueues invalidations without re-verifying the original signature."
                .to_owned(),
        ],
    })
}

pub fn security_from_config(config: &Config) -> Result<ReceiverSecurityConfig> {
    let verification_token = config.webhook.verification_token.clone().ok_or_else(|| {
        RingmasterError::Config(
            "webhook serve requires RINGMASTER_WEBHOOK_VERIFICATION_TOKEN".to_owned(),
        )
    })?;
    let signature_secret = config.oura.client_secret.clone().ok_or_else(|| {
        RingmasterError::Config(
            "webhook serve requires RINGMASTER_OURA_CLIENT_SECRET for signature verification"
                .to_owned(),
        )
    })?;

    Ok(ReceiverSecurityConfig {
        verification_token,
        signature_secret,
        signature_tolerance_secs: config.webhook.signature_tolerance_secs,
    })
}

async fn handle_verification(
    State(state): State<ReceiverState>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let request = InboundWebhookRequest {
        method: "GET".to_owned(),
        query: normalize_query(query),
        headers: normalize_headers(&headers),
        body: "{}".to_owned(),
    };
    match execute_receiver_request(&state, request) {
        Ok(response) => (
            StatusCode::from_u16(response.status_code).unwrap_or(StatusCode::OK),
            response.body,
        ),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("webhook receiver failed: {error}"),
        ),
    }
}

async fn handle_delivery(
    State(state): State<ReceiverState>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let request = InboundWebhookRequest {
        method: "POST".to_owned(),
        query: normalize_query(query),
        headers: normalize_headers(&headers),
        body: String::from_utf8_lossy(&body).into_owned(),
    };
    match execute_receiver_request(&state, request) {
        Ok(response) => (
            StatusCode::from_u16(response.status_code).unwrap_or(StatusCode::OK),
            response.body,
        ),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("webhook receiver failed: {error}"),
        ),
    }
}

async fn handle_health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn handle_ready(State(state): State<ReceiverState>) -> impl IntoResponse {
    match Store::open(&state.config) {
        Ok(_) => (StatusCode::OK, "ready".to_owned()),
        Err(error) => (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("storage unavailable: {error}"),
        ),
    }
}

fn execute_receiver_request(
    state: &ReceiverState,
    request: InboundWebhookRequest,
) -> Result<ReceiverResponse> {
    let store = Store::open(&state.config)?;
    process_inbound_request(&state.security, &store, request)
}

fn handle_challenge(
    security: &ReceiverSecurityConfig,
    store: &Store,
    request: InboundWebhookRequest,
    received_at: String,
) -> Result<ReceiverResponse> {
    let provided_token = request.query.get("verification_token").cloned();
    let challenge = request.query.get("challenge").cloned();
    if challenge.is_none() {
        return reject_request(
            store,
            RejectionContext {
                request: &request,
                received_at,
                reason_code: "missing_challenge",
                detail: "verification challenge requests must include `challenge`".to_owned(),
                signature_timestamp: None,
                status: StatusCode::BAD_REQUEST,
            },
        );
    }
    if provided_token.as_deref() != Some(security.verification_token.as_str()) {
        return reject_request(
            store,
            RejectionContext {
                request: &request,
                received_at,
                reason_code: "verification_token_mismatch",
                detail: "verification token did not match the configured receiver token".to_owned(),
                signature_timestamp: None,
                status: StatusCode::UNAUTHORIZED,
            },
        );
    }

    Ok(ReceiverResponse {
        status_code: StatusCode::OK.as_u16(),
        body: challenge.unwrap_or_default(),
        outcome: ReceiverOutcome::VerificationChallenge,
    })
}

fn handle_signed_delivery(
    security: &ReceiverSecurityConfig,
    store: &Store,
    request: InboundWebhookRequest,
    received_at: String,
) -> Result<ReceiverResponse> {
    let signature = match request.headers.get("x-oura-signature") {
        Some(signature) => signature.clone(),
        None => {
            return reject_request(
                store,
                RejectionContext {
                    request: &request,
                    received_at,
                    reason_code: "missing_signature",
                    detail: "webhook delivery did not include x-oura-signature".to_owned(),
                    signature_timestamp: request.headers.get("x-oura-timestamp").cloned(),
                    status: StatusCode::UNAUTHORIZED,
                },
            );
        }
    };
    let signature_timestamp = match request.headers.get("x-oura-timestamp") {
        Some(value) => value.clone(),
        None => {
            return reject_request(
                store,
                RejectionContext {
                    request: &request,
                    received_at,
                    reason_code: "missing_timestamp",
                    detail: "webhook delivery did not include x-oura-timestamp".to_owned(),
                    signature_timestamp: None,
                    status: StatusCode::UNAUTHORIZED,
                },
            );
        }
    };

    let signature_time = match parse_signature_timestamp(&signature_timestamp) {
        Ok(signature_time) => signature_time,
        Err(detail) => {
            return reject_request(
                store,
                RejectionContext {
                    request: &request,
                    received_at,
                    reason_code: "invalid_timestamp",
                    detail,
                    signature_timestamp: Some(signature_timestamp),
                    status: StatusCode::BAD_REQUEST,
                },
            );
        }
    };

    if !timestamp_is_fresh(signature_time, security.signature_tolerance_secs)? {
        return reject_request(
            store,
            RejectionContext {
                request: &request,
                received_at,
                reason_code: "stale_timestamp",
                detail: "webhook delivery timestamp is outside the configured tolerance window"
                    .to_owned(),
                signature_timestamp: Some(signature_timestamp),
                status: StatusCode::UNAUTHORIZED,
            },
        );
    }

    if !verify_signature(
        &security.signature_secret,
        &signature_timestamp,
        &request.body,
        &signature,
    )? {
        return reject_request(
            store,
            RejectionContext {
                request: &request,
                received_at,
                reason_code: "signature_mismatch",
                detail: "webhook signature verification failed".to_owned(),
                signature_timestamp: Some(signature_timestamp),
                status: StatusCode::UNAUTHORIZED,
            },
        );
    }

    let payload: DeliveryPayload = match serde_json::from_str(&request.body) {
        Ok(payload) => payload,
        Err(error) => {
            return reject_request(
                store,
                RejectionContext {
                    request: &request,
                    received_at,
                    reason_code: "invalid_payload_json",
                    detail: format!(
                        "webhook payload decode failed after signature verification: {error}"
                    ),
                    signature_timestamp: Some(signature_timestamp),
                    status: StatusCode::BAD_REQUEST,
                },
            );
        }
    };
    let event_type = payload
        .event_type
        .as_deref()
        .and_then(WebhookEventType::parse);
    let payload_json = payload_wrapper(&request.body);
    let accepted = store
        .webhook()
        .insert_accepted_delivery(&AcceptedWebhookDeliveryInput {
            delivery_fingerprint: delivery_fingerprint(&signature_timestamp, &request.body),
            received_at,
            signature_timestamp: Some(signature_timestamp.clone()),
            data_type: payload.data_type.clone(),
            event_type,
            object_id: payload.object_id,
            payload_json,
            headers_json: sanitize_headers(&request.headers),
            query_json: sanitize_query(&request.query),
        })?;

    let (delivery, duplicate) = match accepted {
        AcceptedWebhookDeliveryResult::Inserted(record) => (record, false),
        AcceptedWebhookDeliveryResult::Duplicate(record) => (record, true),
    };
    let invalidation = if duplicate {
        None
    } else {
        enqueue_delivery_invalidation(
            store,
            delivery.delivery_id,
            delivery.received_at.clone(),
            delivery.data_type.clone(),
            delivery.event_type,
            delivery.object_id.clone(),
        )?
    };
    let detail = if duplicate {
        Some("duplicate delivery already persisted; skipped invalidation enqueue".to_owned())
    } else if invalidation.is_none() {
        Some(ACCEPTED_NO_INVALIDATION.to_owned())
    } else {
        None
    };

    Ok(ReceiverResponse {
        status_code: StatusCode::OK.as_u16(),
        body: "accepted".to_owned(),
        outcome: ReceiverOutcome::Accepted {
            delivery_id: delivery.delivery_id,
            duplicate,
            invalidation_id: invalidation.map(|record| record.invalidation_id),
            detail,
        },
    })
}

fn reject_request(store: &Store, rejection: RejectionContext<'_>) -> Result<ReceiverResponse> {
    let _ = store
        .webhook()
        .insert_rejected_delivery(&RejectedWebhookDeliveryInput {
            received_at: rejection.received_at,
            reason_code: rejection.reason_code.to_owned(),
            detail: rejection.detail.clone(),
            signature_timestamp: rejection.signature_timestamp,
            payload_json: payload_wrapper(&rejection.request.body),
            headers_json: sanitize_headers(&rejection.request.headers),
            query_json: sanitize_query(&rejection.request.query),
        })?;

    Ok(ReceiverResponse {
        status_code: rejection.status.as_u16(),
        body: rejection.detail,
        outcome: ReceiverOutcome::Rejected {
            reason_code: rejection.reason_code.to_owned(),
        },
    })
}

fn enqueue_delivery_invalidation(
    store: &Store,
    delivery_id: i64,
    queued_at: String,
    data_type: Option<String>,
    event_type: Option<WebhookEventType>,
    object_id: Option<String>,
) -> Result<Option<InvalidationRecord>> {
    let Some(data_type) = data_type else {
        return Ok(None);
    };
    let Some(event_type) = event_type else {
        return Ok(None);
    };
    if !is_supported_data_type(&data_type) {
        return Ok(None);
    }

    let queue_key = derive_queue_key(&data_type, event_type, object_id.as_deref());
    store
        .webhook()
        .enqueue_invalidation(&InvalidationInput {
            queue_key,
            data_type,
            event_type,
            object_id,
            delivery_id,
            queued_at: queued_at.clone(),
            available_at: queued_at,
        })
        .map(Some)
}

fn derive_queue_key(
    data_type: &str,
    event_type: WebhookEventType,
    object_id: Option<&str>,
) -> String {
    match object_id {
        Some(object_id) => format!("{data_type}:{}:{object_id}", event_type.as_str()),
        None => format!("{data_type}:{}:*", event_type.as_str()),
    }
}

fn delivery_fingerprint(signature_timestamp: &str, body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(signature_timestamp.as_bytes());
    hasher.update(body.as_bytes());
    hex::encode(hasher.finalize())
}

fn compute_signature(secret: &str, timestamp: &str, body: &str) -> Result<String> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|error| {
        RingmasterError::Config(format!(
            "webhook signature secret could not initialize hmac: {error}"
        ))
    })?;
    mac.update(timestamp.as_bytes());
    mac.update(body.as_bytes());
    Ok(hex::encode_upper(mac.finalize().into_bytes()))
}

fn verify_signature(secret: &str, timestamp: &str, body: &str, signature: &str) -> Result<bool> {
    let Ok(signature_bytes) = hex::decode(signature.trim()) else {
        return Ok(false);
    };
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|error| {
        RingmasterError::Config(format!(
            "webhook signature secret could not initialize hmac: {error}"
        ))
    })?;
    mac.update(timestamp.as_bytes());
    mac.update(body.as_bytes());
    Ok(mac.verify_slice(&signature_bytes).is_ok())
}

fn parse_signature_timestamp(timestamp: &str) -> std::result::Result<OffsetDateTime, String> {
    if let Ok(seconds) = timestamp.parse::<i64>() {
        OffsetDateTime::from_unix_timestamp(seconds).map_err(|error| {
            format!("webhook timestamp `{timestamp}` was not a valid unix timestamp: {error}")
        })
    } else {
        OffsetDateTime::parse(timestamp, &Rfc3339).map_err(|error| {
            format!("webhook timestamp `{timestamp}` was not valid RFC3339: {error}")
        })
    }
}

fn timestamp_is_fresh(parsed: OffsetDateTime, tolerance_secs: u64) -> Result<bool> {
    let tolerance = Duration::seconds(i64::try_from(tolerance_secs).map_err(|error| {
        RingmasterError::Config(format!("invalid webhook signature tolerance: {error}"))
    })?);
    let age = OffsetDateTime::now_utc() - parsed;
    Ok(age.abs() <= tolerance)
}

fn normalize_headers(headers: &HeaderMap) -> BTreeMap<String, String> {
    headers
        .iter()
        .filter_map(|(key, value)| {
            value
                .to_str()
                .ok()
                .map(|text| (key.as_str().to_ascii_lowercase(), text.to_owned()))
        })
        .collect()
}

fn normalize_query(query: HashMap<String, String>) -> BTreeMap<String, String> {
    query
        .into_iter()
        .map(|(key, value)| (key.to_ascii_lowercase(), value))
        .collect()
}

fn sanitize_headers(headers: &BTreeMap<String, String>) -> String {
    let sanitized = headers
        .iter()
        .map(|(key, value)| {
            let sanitized_value = if key.contains("signature")
                || key.contains("secret")
                || key.contains("authorization")
                || key.contains("cookie")
            {
                "[redacted]".to_owned()
            } else {
                value.clone()
            };
            (key.clone(), sanitized_value)
        })
        .collect::<BTreeMap<_, _>>();
    serde_json::to_string(&sanitized).unwrap_or_else(|_| "{}".to_owned())
}

fn sanitize_query(query: &BTreeMap<String, String>) -> String {
    let sanitized = query
        .iter()
        .map(|(key, value)| {
            let sanitized_value = if key.contains("token") {
                "[redacted]".to_owned()
            } else {
                value.clone()
            };
            (key.clone(), sanitized_value)
        })
        .collect::<BTreeMap<_, _>>();
    serde_json::to_string(&sanitized).unwrap_or_else(|_| "{}".to_owned())
}

fn payload_wrapper(raw_body: &str) -> String {
    if serde_json::from_str::<Value>(raw_body).is_ok() {
        raw_body.to_owned()
    } else {
        json!({ "raw_body": raw_body }).to_string()
    }
}

fn load_fixture(path: &PathBuf) -> Result<ReplayFixture> {
    let payload = std::fs::read_to_string(path)
        .map_err(|error| RingmasterError::io("reading webhook replay fixture", error))?;
    serde_json::from_str(&payload).map_err(Into::into)
}

fn fixture_security(config: &Config, fixture: &ReplayFixture) -> Result<ReceiverSecurityConfig> {
    let verification_token = config
        .webhook
        .verification_token
        .clone()
        .unwrap_or_else(|| "fixture-verification-token".to_owned());
    let signature_secret = fixture
        .signature_secret
        .clone()
        .or_else(|| config.oura.client_secret.clone())
        .ok_or_else(|| {
            RingmasterError::Config(
                "fixture replay requires either signature_secret in the fixture or RINGMASTER_OURA_CLIENT_SECRET"
                    .to_owned(),
            )
        })?;

    Ok(ReceiverSecurityConfig {
        verification_token,
        signature_secret,
        signature_tolerance_secs: config.webhook.signature_tolerance_secs,
    })
}

fn fixture_into_request(
    fixture: ReplayFixture,
    computed_signature_secret: Option<&str>,
) -> Result<InboundWebhookRequest> {
    let body = match (fixture.body, fixture.body_json) {
        (Some(body), None) => body,
        (None, Some(body_json)) => serde_json::to_string(&body_json)?,
        (Some(_), Some(_)) => {
            return Err(RingmasterError::Config(
                "webhook replay fixture must provide either `body` or `body_json`, not both"
                    .to_owned(),
            ));
        }
        (None, None) => "{}".to_owned(),
    };
    let mut headers = fixture
        .headers
        .into_iter()
        .map(|(key, value)| (key.to_ascii_lowercase(), value))
        .collect::<BTreeMap<_, _>>();
    if headers
        .get("x-oura-timestamp")
        .is_some_and(|value| value == FIXTURE_NOW_RFC3339)
    {
        headers.insert("x-oura-timestamp".to_owned(), now_rfc3339()?);
    }
    if headers
        .get("x-oura-signature")
        .is_some_and(|value| value == FIXTURE_COMPUTED_SIGNATURE)
    {
        let signature_secret = fixture
            .signature_secret
            .as_deref()
            .or(computed_signature_secret)
            .ok_or_else(|| {
                RingmasterError::Config(
                    "webhook replay fixtures using computed signatures must include signature_secret or a configured Oura client secret"
                        .to_owned(),
                )
            })?;
        let timestamp = headers.get("x-oura-timestamp").cloned().ok_or_else(|| {
            RingmasterError::Config(
                "webhook replay fixtures using computed signatures must include x-oura-timestamp"
                    .to_owned(),
            )
        })?;
        headers.insert(
            "x-oura-signature".to_owned(),
            compute_signature(signature_secret, &timestamp, &body)?,
        );
    }

    Ok(InboundWebhookRequest {
        method: fixture.method,
        query: fixture.query,
        headers,
        body,
    })
}

fn replay_stored_delivery(
    store: &Store,
    record: &AcceptedWebhookDeliveryRecord,
) -> Result<WebhookReplayEntry> {
    let invalidation = enqueue_delivery_invalidation(
        store,
        record.delivery_id,
        now_rfc3339()?,
        record.data_type.clone(),
        record.event_type,
        record.object_id.clone(),
    )?;
    Ok(WebhookReplayEntry {
        source: format!("delivery:{}", record.delivery_id),
        status: if invalidation.is_some() {
            "requeued".to_owned()
        } else {
            ACCEPTED_NO_INVALIDATION.to_owned()
        },
        delivery_id: Some(record.delivery_id),
        invalidation_id: invalidation.map(|entry| entry.invalidation_id),
        detail: None,
    })
}

fn entry_from_receiver_response(source: String, response: ReceiverResponse) -> WebhookReplayEntry {
    match response.outcome {
        ReceiverOutcome::VerificationChallenge => WebhookReplayEntry {
            source,
            status: "verification_challenge".to_owned(),
            delivery_id: None,
            invalidation_id: None,
            detail: Some(response.body),
        },
        ReceiverOutcome::Accepted {
            delivery_id,
            duplicate,
            invalidation_id,
            detail,
        } => WebhookReplayEntry {
            source,
            status: if duplicate {
                "duplicate".to_owned()
            } else {
                "accepted".to_owned()
            },
            delivery_id: Some(delivery_id),
            invalidation_id,
            detail,
        },
        ReceiverOutcome::Rejected { reason_code } => WebhookReplayEntry {
            source,
            status: "rejected".to_owned(),
            delivery_id: None,
            invalidation_id: None,
            detail: Some(reason_code),
        },
    }
}

fn write_heartbeat(config: &Config, mode: &str, detail: Option<String>) -> Result<()> {
    let store = Store::open(config)?;
    store
        .webhook()
        .upsert_runtime_heartbeat(&RuntimeHeartbeatRecord {
            component: RECEIVER_COMPONENT.to_owned(),
            mode: mode.to_owned(),
            bind_address: Some(config.webhook.bind.to_string()),
            public_base_url: config.webhook.public_base_url.clone(),
            detail,
            last_seen_at: now_rfc3339()?,
        })
}

async fn heartbeat_loop(
    config: Config,
    bind_address: SocketAddr,
    mut shutdown: watch::Receiver<bool>,
) {
    let interval = std::time::Duration::from_secs(config.webhook.heartbeat_secs);
    loop {
        if let Err(error) = write_heartbeat(
            &config,
            "running",
            Some(format!("listening on {}", bind_address)),
        ) {
            warn!("failed to update webhook receiver heartbeat: {error}");
        }
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_ok() && *shutdown.borrow() {
                    break;
                }
            }
            _ = tokio::time::sleep(interval) => {}
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        InboundWebhookRequest, ReceiverOutcome, ReceiverSecurityConfig, ReplayFixture,
        fixture_into_request, process_inbound_request, replay,
    };
    use crate::config::Config;
    use crate::store::Store;
    use crate::webhook::receiver::{WebhookReplayOptions, delivery_fingerprint};
    use axum::http::StatusCode;
    use hmac::{Hmac, Mac};
    use serde_json::json;
    use sha2::Sha256;
    use tempfile::tempdir;
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    type TestHmacSha256 = Hmac<Sha256>;

    fn sign(timestamp: &str, body: &str, secret: &str) -> String {
        let mut mac = TestHmacSha256::new_from_slice(secret.as_bytes())
            .unwrap_or_else(|error| panic!("hmac init should succeed in test: {error}"));
        mac.update(timestamp.as_bytes());
        mac.update(body.as_bytes());
        hex::encode_upper(mac.finalize().into_bytes())
    }

    fn security_config() -> ReceiverSecurityConfig {
        ReceiverSecurityConfig {
            verification_token: "fixture-verification-token".to_owned(),
            signature_secret: "fixture-secret".to_owned(),
            signature_tolerance_secs: 300,
        }
    }

    #[test]
    fn accepts_valid_signed_delivery_and_enqueues_invalidation() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let timestamp = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|error| panic!("timestamp should format: {error}"));
        let body = r#"{"data_type":"daily_sleep","event_type":"create","object_id":"sleep_123"}"#;
        let response = process_inbound_request(
            &security_config(),
            &store,
            InboundWebhookRequest {
                method: "POST".to_owned(),
                query: BTreeMap::new(),
                headers: BTreeMap::from([
                    (
                        "x-oura-signature".to_owned(),
                        sign(&timestamp, body, "fixture-secret"),
                    ),
                    ("x-oura-timestamp".to_owned(), timestamp.clone()),
                ]),
                body: body.to_owned(),
            },
        )
        .unwrap_or_else(|error| panic!("delivery should be accepted: {error}"));

        match response.outcome {
            ReceiverOutcome::Accepted {
                duplicate,
                invalidation_id,
                ..
            } => {
                assert!(!duplicate);
                assert!(invalidation_id.is_some());
            }
            other => panic!("unexpected receiver outcome: {other:?}"),
        }
        assert_eq!(
            store
                .webhook()
                .list_pending_invalidations()
                .unwrap_or_else(|error| { panic!("pending invalidations should load: {error}") })
                .len(),
            1
        );
    }

    #[test]
    fn rejects_invalid_verification_challenge_token() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let response = process_inbound_request(
            &security_config(),
            &store,
            InboundWebhookRequest {
                method: "GET".to_owned(),
                query: BTreeMap::from([
                    ("challenge".to_owned(), "abc123".to_owned()),
                    ("verification_token".to_owned(), "wrong".to_owned()),
                ]),
                headers: BTreeMap::new(),
                body: "{}".to_owned(),
            },
        )
        .unwrap_or_else(|error| panic!("challenge should produce response: {error}"));

        match response.outcome {
            ReceiverOutcome::Rejected { reason_code } => {
                assert_eq!(reason_code, "verification_token_mismatch");
            }
            other => panic!("unexpected receiver outcome: {other:?}"),
        }
        assert!(
            store
                .webhook()
                .latest_rejected_delivery()
                .unwrap_or_else(|error| panic!("latest rejection should load: {error}"))
                .is_some()
        );
    }

    #[test]
    fn rejects_malformed_timestamp_instead_of_returning_internal_error() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let body = r#"{"data_type":"daily_sleep","event_type":"create","object_id":"sleep_123"}"#;
        let response = process_inbound_request(
            &security_config(),
            &store,
            InboundWebhookRequest {
                method: "POST".to_owned(),
                query: BTreeMap::new(),
                headers: BTreeMap::from([
                    (
                        "x-oura-signature".to_owned(),
                        sign("not-a-time", body, "fixture-secret"),
                    ),
                    ("x-oura-timestamp".to_owned(), "not-a-time".to_owned()),
                ]),
                body: body.to_owned(),
            },
        )
        .unwrap_or_else(|error| panic!("malformed timestamp should produce a rejection: {error}"));

        assert_eq!(response.status_code, StatusCode::BAD_REQUEST.as_u16());
        match response.outcome {
            ReceiverOutcome::Rejected { reason_code } => {
                assert_eq!(reason_code, "invalid_timestamp");
            }
            other => panic!("unexpected receiver outcome: {other:?}"),
        }
        let rejection = store
            .webhook()
            .latest_rejected_delivery()
            .unwrap_or_else(|error| panic!("latest rejection should load: {error}"))
            .unwrap_or_else(|| panic!("a rejected delivery should be recorded"));
        assert_eq!(rejection.reason_code, "invalid_timestamp");
    }

    #[test]
    fn dedupes_duplicate_signed_delivery() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let timestamp = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|error| panic!("timestamp should format: {error}"));
        let body = r#"{"data_type":"daily_sleep","event_type":"create","object_id":"sleep_123"}"#;
        let headers = BTreeMap::from([
            (
                "x-oura-signature".to_owned(),
                sign(&timestamp, body, "fixture-secret"),
            ),
            ("x-oura-timestamp".to_owned(), timestamp.clone()),
        ]);
        let first = process_inbound_request(
            &security_config(),
            &store,
            InboundWebhookRequest {
                method: "POST".to_owned(),
                query: BTreeMap::new(),
                headers: headers.clone(),
                body: body.to_owned(),
            },
        )
        .unwrap_or_else(|error| panic!("first delivery should succeed: {error}"));
        let second = process_inbound_request(
            &security_config(),
            &store,
            InboundWebhookRequest {
                method: "POST".to_owned(),
                query: BTreeMap::new(),
                headers,
                body: body.to_owned(),
            },
        )
        .unwrap_or_else(|error| panic!("second delivery should succeed: {error}"));

        let first_delivery_id = match first.outcome {
            ReceiverOutcome::Accepted { delivery_id, .. } => delivery_id,
            other => panic!("unexpected first outcome: {other:?}"),
        };
        match second.outcome {
            ReceiverOutcome::Accepted {
                duplicate,
                delivery_id,
                ..
            } => {
                assert!(duplicate);
                assert_eq!(delivery_id, first_delivery_id);
            }
            other => panic!("unexpected second outcome: {other:?}"),
        }
        assert_eq!(
            delivery_fingerprint(&timestamp, body),
            store
                .webhook()
                .list_recent_deliveries(1)
                .unwrap_or_else(|error| panic!("recent deliveries should load: {error}"))[0]
                .delivery_fingerprint
        );
        assert_eq!(
            store
                .webhook()
                .list_pending_invalidations()
                .unwrap_or_else(|error| panic!("pending invalidations should load: {error}"))
                .len(),
            1
        );
    }

    #[test]
    fn rejects_invalid_json_after_signature_verification() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let timestamp = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|error| panic!("timestamp should format: {error}"));
        let body = "{not-json";
        let response = process_inbound_request(
            &security_config(),
            &store,
            InboundWebhookRequest {
                method: "POST".to_owned(),
                query: BTreeMap::new(),
                headers: BTreeMap::from([
                    (
                        "x-oura-signature".to_owned(),
                        sign(&timestamp, body, "fixture-secret"),
                    ),
                    ("x-oura-timestamp".to_owned(), timestamp),
                ]),
                body: body.to_owned(),
            },
        )
        .unwrap_or_else(|error| panic!("invalid payload should produce a rejection: {error}"));

        match response.outcome {
            ReceiverOutcome::Rejected { reason_code } => {
                assert_eq!(reason_code, "invalid_payload_json");
            }
            other => panic!("unexpected receiver outcome: {other:?}"),
        }
        let rejection = store
            .webhook()
            .latest_rejected_delivery()
            .unwrap_or_else(|error| panic!("latest rejection should load: {error}"))
            .unwrap_or_else(|| panic!("a rejected delivery should be recorded"));
        assert_eq!(rejection.reason_code, "invalid_payload_json");
    }

    #[test]
    fn duplicate_signed_delivery_does_not_requeue_completed_invalidation() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let timestamp = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|error| panic!("timestamp should format: {error}"));
        let body = r#"{"data_type":"daily_sleep","event_type":"create","object_id":"sleep_123"}"#;
        let headers = BTreeMap::from([
            (
                "x-oura-signature".to_owned(),
                sign(&timestamp, body, "fixture-secret"),
            ),
            ("x-oura-timestamp".to_owned(), timestamp.clone()),
        ]);

        let first = process_inbound_request(
            &security_config(),
            &store,
            InboundWebhookRequest {
                method: "POST".to_owned(),
                query: BTreeMap::new(),
                headers: headers.clone(),
                body: body.to_owned(),
            },
        )
        .unwrap_or_else(|error| panic!("first delivery should succeed: {error}"));
        let invalidation_id = match first.outcome {
            ReceiverOutcome::Accepted {
                invalidation_id: Some(invalidation_id),
                ..
            } => invalidation_id,
            other => panic!("unexpected first outcome: {other:?}"),
        };
        let attempt = store
            .webhook()
            .start_processing_attempt(invalidation_id, &timestamp)
            .unwrap_or_else(|error| panic!("attempt should start: {error}"));
        store
            .webhook()
            .complete_processing_attempt_success(
                invalidation_id,
                attempt.attempt_id,
                &timestamp,
                Some("processed"),
            )
            .unwrap_or_else(|error| panic!("attempt should complete: {error}"));
        assert!(
            store
                .webhook()
                .list_pending_invalidations()
                .unwrap_or_else(|error| panic!("pending invalidations should load: {error}"))
                .is_empty()
        );

        let duplicate = process_inbound_request(
            &security_config(),
            &store,
            InboundWebhookRequest {
                method: "POST".to_owned(),
                query: BTreeMap::new(),
                headers,
                body: body.to_owned(),
            },
        )
        .unwrap_or_else(|error| panic!("duplicate delivery should succeed: {error}"));

        match duplicate.outcome {
            ReceiverOutcome::Accepted {
                duplicate,
                invalidation_id,
                detail,
                ..
            } => {
                assert!(duplicate);
                assert!(invalidation_id.is_none());
                assert_eq!(
                    detail.as_deref(),
                    Some("duplicate delivery already persisted; skipped invalidation enqueue")
                );
            }
            other => panic!("unexpected duplicate outcome: {other:?}"),
        }
        assert!(
            store
                .webhook()
                .list_pending_invalidations()
                .unwrap_or_else(|error| panic!("pending invalidations should load: {error}"))
                .is_empty()
        );
    }

    #[test]
    fn fixture_body_json_serializes_into_request_body() {
        let fixture = ReplayFixture {
            method: "POST".to_owned(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: None,
            body_json: Some(json!({"hello":"world"})),
            signature_secret: Some("fixture-secret".to_owned()),
        };

        let request = fixture_into_request(fixture, None)
            .unwrap_or_else(|error| panic!("fixture should convert: {error}"));
        assert_eq!(request.body, r#"{"hello":"world"}"#);
    }

    #[test]
    fn fixture_placeholders_materialize_timestamp_and_signature() {
        let request = fixture_into_request(
            ReplayFixture {
                method: "POST".to_owned(),
                query: BTreeMap::new(),
                headers: BTreeMap::from([
                    (
                        "x-oura-signature".to_owned(),
                        "{{computed_hmac_sha256}}".to_owned(),
                    ),
                    ("x-oura-timestamp".to_owned(), "{{now_rfc3339}}".to_owned()),
                ]),
                body: Some(
                    r#"{"data_type":"daily_sleep","event_type":"create","object_id":"sleep_123"}"#
                        .to_owned(),
                ),
                body_json: None,
                signature_secret: Some("fixture-secret".to_owned()),
            },
            None,
        )
        .unwrap_or_else(|error| panic!("fixture should materialize: {error}"));

        assert_ne!(
            request
                .headers
                .get("x-oura-timestamp")
                .unwrap_or_else(|| panic!("timestamp header should exist")),
            "{{now_rfc3339}}"
        );
        assert_ne!(
            request
                .headers
                .get("x-oura-signature")
                .unwrap_or_else(|| panic!("signature header should exist")),
            "{{computed_hmac_sha256}}"
        );
    }

    #[test]
    fn fixture_placeholders_use_config_secret_when_fixture_secret_is_absent() {
        let request = fixture_into_request(
            ReplayFixture {
                method: "POST".to_owned(),
                query: BTreeMap::new(),
                headers: BTreeMap::from([
                    (
                        "x-oura-signature".to_owned(),
                        "{{computed_hmac_sha256}}".to_owned(),
                    ),
                    ("x-oura-timestamp".to_owned(), "{{now_rfc3339}}".to_owned()),
                ]),
                body: Some(
                    r#"{"data_type":"daily_sleep","event_type":"create","object_id":"sleep_123"}"#
                        .to_owned(),
                ),
                body_json: None,
                signature_secret: None,
            },
            Some("fixture-secret"),
        )
        .unwrap_or_else(|error| panic!("fixture should materialize with config secret: {error}"));

        assert_ne!(
            request
                .headers
                .get("x-oura-signature")
                .unwrap_or_else(|| panic!("signature header should exist")),
            "{{computed_hmac_sha256}}"
        );
    }

    #[tokio::test]
    async fn replay_fixture_uses_embedded_signature_secret() {
        let fixture_dir =
            tempdir().unwrap_or_else(|error| panic!("tempdir should succeed: {error}"));
        let timestamp = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|error| panic!("timestamp should format: {error}"));
        let body = r#"{"data_type":"daily_sleep","event_type":"create","object_id":"sleep_123"}"#;
        std::fs::write(
            fixture_dir.path().join("sample.json"),
            format!(
                r#"{{
  "method": "POST",
  "headers": {{
    "x-oura-signature": "{signature}",
    "x-oura-timestamp": "{timestamp}"
  }},
  "body": "{body}",
  "signature_secret": "fixture-secret"
}}"#,
                signature = sign(&timestamp, body, "fixture-secret"),
                timestamp = timestamp,
                body = body.replace('"', "\\\""),
            ),
        )
        .unwrap_or_else(|error| panic!("fixture write should succeed: {error}"));

        let config = Config::load().unwrap_or_else(|error| panic!("config should load: {error}"));
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let report = replay(
            &config,
            &store,
            WebhookReplayOptions {
                fixture: Some(fixture_dir.path().join("sample.json")),
                delivery_id: None,
                recent: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("fixture replay should succeed: {error}"));

        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].status, "accepted");
    }

    #[tokio::test]
    async fn replay_fixture_uses_config_signature_secret_when_fixture_omits_it() {
        let fixture_dir =
            tempdir().unwrap_or_else(|error| panic!("tempdir should succeed: {error}"));
        std::fs::write(
            fixture_dir.path().join("sample.json"),
            r#"{
  "method": "POST",
  "headers": {
    "x-oura-signature": "{{computed_hmac_sha256}}",
    "x-oura-timestamp": "{{now_rfc3339}}"
  },
  "body": "{\"data_type\":\"daily_sleep\",\"event_type\":\"create\",\"object_id\":\"sleep_123\"}"
}"#,
        )
        .unwrap_or_else(|error| panic!("fixture write should succeed: {error}"));

        let mut config =
            Config::load().unwrap_or_else(|error| panic!("config should load: {error}"));
        config.oura.client_secret = Some("fixture-secret".to_owned());
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let report = replay(
            &config,
            &store,
            WebhookReplayOptions {
                fixture: Some(fixture_dir.path().join("sample.json")),
                delivery_id: None,
                recent: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("fixture replay should succeed: {error}"));

        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].status, "accepted");
    }
}
