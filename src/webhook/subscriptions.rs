use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::config::Config;
use crate::error::{OuraApiError, OuraProblem, Result, RingmasterError};
use crate::store::Store;
use crate::store::webhook_store::{
    DesiredWebhookSubscriptionRecord, RemoteWebhookSubscriptionRecord, now_rfc3339,
};
use crate::webhook::WebhookEventType;

const FIXTURE_REMOTE_FILE: &str = "subscriptions.remote.json";
const FIXTURE_CONTEXT_FILE: &str = "subscriptions.context.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionSyncOptions {
    pub dry_run: bool,
    pub prune: bool,
    pub fixture_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteWebhookSubscription {
    pub id: String,
    pub callback_url: String,
    pub event_type: WebhookEventType,
    pub data_type: String,
    pub expiration_time: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredWebhookSubscriptionTarget {
    pub data_type: String,
    pub event_type: WebhookEventType,
    pub callback_url: String,
    pub verification_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionListReport {
    pub callback_url: Option<String>,
    pub verification_token_configured: bool,
    pub fixture_dir: Option<PathBuf>,
    pub desired: Vec<DesiredWebhookSubscriptionTarget>,
    pub remote: Vec<RemoteWebhookSubscription>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionSyncPlan {
    pub create: Vec<DesiredWebhookSubscriptionTarget>,
    pub update: Vec<SubscriptionUpdate>,
    pub renew: Vec<RemoteWebhookSubscription>,
    pub prune: Vec<RemoteWebhookSubscription>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionUpdate {
    pub existing: RemoteWebhookSubscription,
    pub desired: DesiredWebhookSubscriptionTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionSyncReport {
    pub dry_run: bool,
    pub prune_requested: bool,
    pub fixture_dir: Option<PathBuf>,
    pub callback_url: String,
    pub plan: SubscriptionSyncPlan,
    pub remote_before: Vec<RemoteWebhookSubscription>,
    pub remote_after: Vec<RemoteWebhookSubscription>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct FixtureSubscriptionContext {
    callback_url: Option<String>,
    verification_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct SubscriptionKey {
    data_type: String,
    event_type: WebhookEventType,
}

#[derive(Debug, Clone)]
struct SyncContext {
    callback_url: String,
    verification_token: String,
}

#[derive(Debug, Clone)]
struct LiveWebhookAdminClient {
    http: HttpClient,
    api_base_url: String,
    client_id: String,
    client_secret: String,
}

#[derive(Debug, Clone)]
struct FixtureWebhookAdminClient {
    subscriptions: Vec<RemoteWebhookSubscription>,
}

#[derive(Debug, Clone)]
enum WebhookAdminClient {
    Live(LiveWebhookAdminClient),
    Fixture(FixtureWebhookAdminClient),
}

#[derive(Debug, Serialize)]
struct CreateWebhookSubscriptionRequest<'a> {
    callback_url: &'a str,
    verification_token: &'a str,
    event_type: WebhookEventType,
    data_type: &'a str,
}

#[derive(Debug, Serialize)]
struct UpdateWebhookSubscriptionRequest<'a> {
    verification_token: &'a str,
    callback_url: Option<&'a str>,
    event_type: Option<WebhookEventType>,
    data_type: Option<&'a str>,
}

pub async fn list_subscriptions(
    config: &Config,
    store: &Store,
    fixture_dir: Option<PathBuf>,
) -> Result<SubscriptionListReport> {
    let desired = desired_targets(config, fixture_dir.as_deref())?;
    persist_desired_snapshot(config, store, &desired)?;

    let mut client = admin_client(config, fixture_dir.clone())?;
    let remote = client.list().await?;
    persist_remote_snapshot(config, store, &desired, &remote)?;

    let mut notes = Vec::new();
    if fixture_dir.is_some() {
        notes.push("Loaded remote subscriptions from fixture data.".to_owned());
    } else {
        notes.push("Loaded remote subscriptions from the Oura admin API.".to_owned());
    }

    Ok(SubscriptionListReport {
        callback_url: desired.first().map(|entry| entry.callback_url.clone()),
        verification_token_configured: config.webhook.verification_token.is_some(),
        fixture_dir,
        desired,
        remote,
        notes,
    })
}

pub async fn sync_subscriptions(
    config: &Config,
    store: &Store,
    options: SubscriptionSyncOptions,
) -> Result<SubscriptionSyncReport> {
    let desired = desired_targets(config, options.fixture_dir.as_deref())?;
    persist_desired_snapshot(config, store, &desired)?;

    let mut client = admin_client(config, options.fixture_dir.clone())?;
    let remote_before = client.list().await?;
    let plan = build_sync_plan(
        &desired,
        &remote_before,
        config.webhook.renewal_lead_secs,
        options.prune,
    )?;

    let remote_after = if options.dry_run {
        remote_before.clone()
    } else {
        client.apply(&plan).await?;
        client.list().await?
    };
    persist_remote_snapshot(config, store, &desired, &remote_after)?;

    let mut notes = Vec::new();
    if options.dry_run {
        notes.push(
            "Dry-run mode reported the webhook diff without mutating the remote service."
                .to_owned(),
        );
    } else if options.fixture_dir.is_some() {
        notes.push(
            "Applied webhook subscription changes against fixture-backed remote state.".to_owned(),
        );
    } else {
        notes.push("Converged remote webhook subscriptions toward local desired state.".to_owned());
    }
    if options.prune {
        notes.push("Prune mode was enabled for out-of-spec remote subscriptions.".to_owned());
    }

    Ok(SubscriptionSyncReport {
        dry_run: options.dry_run,
        prune_requested: options.prune,
        fixture_dir: options.fixture_dir,
        callback_url: desired
            .first()
            .map(|entry| entry.callback_url.clone())
            .ok_or_else(|| {
                RingmasterError::Config(
                    "webhook sync requires at least one enabled desired subscription".to_owned(),
                )
            })?,
        plan,
        remote_before,
        remote_after,
        notes,
    })
}

pub fn build_sync_plan(
    desired: &[DesiredWebhookSubscriptionTarget],
    remote: &[RemoteWebhookSubscription],
    renewal_lead_secs: u64,
    prune_requested: bool,
) -> Result<SubscriptionSyncPlan> {
    let now = OffsetDateTime::now_utc();
    let renewal_lead = Duration::seconds(i64::try_from(renewal_lead_secs).map_err(|error| {
        RingmasterError::Config(format!("invalid webhook renewal lead seconds: {error}"))
    })?);
    let desired_by_key = desired
        .iter()
        .cloned()
        .map(|entry| {
            (
                SubscriptionKey {
                    data_type: entry.data_type.clone(),
                    event_type: entry.event_type,
                },
                entry,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let remote_by_key = remote.iter().cloned().fold(
        BTreeMap::<SubscriptionKey, Vec<RemoteWebhookSubscription>>::new(),
        |mut groups, subscription| {
            groups
                .entry(SubscriptionKey {
                    data_type: subscription.data_type.clone(),
                    event_type: subscription.event_type,
                })
                .or_default()
                .push(subscription);
            groups
        },
    );

    let mut create = Vec::new();
    let mut update = Vec::new();
    let mut renew = Vec::new();
    let mut prune = Vec::new();
    let mut notes = Vec::new();

    for (key, desired_entry) in &desired_by_key {
        let mut matches = remote_by_key.get(key).cloned().unwrap_or_default();
        if matches.is_empty() {
            create.push(desired_entry.clone());
            continue;
        }

        matches.sort_by(|left, right| left.id.cmp(&right.id));
        let canonical = matches
            .iter()
            .find(|subscription| subscription.callback_url == desired_entry.callback_url)
            .cloned()
            .unwrap_or_else(|| matches[0].clone());

        if canonical.callback_url != desired_entry.callback_url {
            update.push(SubscriptionUpdate {
                existing: canonical.clone(),
                desired: desired_entry.clone(),
            });
        } else if expires_within(&canonical.expiration_time, now, renewal_lead)? {
            renew.push(canonical.clone());
        }

        for extra in matches {
            if extra.id == canonical.id {
                continue;
            }
            if prune_requested {
                prune.push(extra);
            } else {
                notes.push(format!(
                    "Remote subscription {} duplicates desired {}:{}; rerun with --prune to remove it.",
                    extra.id, key.data_type, key.event_type.as_str()
                ));
            }
        }
    }

    for (key, subscriptions) in remote_by_key {
        if desired_by_key.contains_key(&key) {
            continue;
        }
        for subscription in subscriptions {
            if prune_requested {
                prune.push(subscription);
            } else {
                notes.push(format!(
                    "Remote subscription {} for {}:{} is outside the desired config; rerun with --prune to remove it.",
                    subscription.id, key.data_type, key.event_type.as_str()
                ));
            }
        }
    }

    Ok(SubscriptionSyncPlan {
        create,
        update,
        renew,
        prune,
        notes,
    })
}

impl WebhookAdminClient {
    async fn list(&mut self) -> Result<Vec<RemoteWebhookSubscription>> {
        match self {
            Self::Live(client) => client.list().await,
            Self::Fixture(client) => client.list().await,
        }
    }

    async fn apply(&mut self, plan: &SubscriptionSyncPlan) -> Result<()> {
        match self {
            Self::Live(client) => client.apply(plan).await,
            Self::Fixture(client) => client.apply(plan).await,
        }
    }
}

impl LiveWebhookAdminClient {
    fn new(config: &Config) -> Result<Self> {
        let client_id = config.oura.client_id.clone().ok_or_else(|| {
            RingmasterError::Config(
                "webhook subscription management requires RINGMASTER_OURA_CLIENT_ID".to_owned(),
            )
        })?;
        let client_secret = config.oura.client_secret.clone().ok_or_else(|| {
            RingmasterError::Config(
                "webhook subscription management requires RINGMASTER_OURA_CLIENT_SECRET".to_owned(),
            )
        })?;
        let http = HttpClient::builder()
            .user_agent("ringmaster.rs/phase4")
            .redirect(reqwest::redirect::Policy::none())
            .build()?;

        Ok(Self {
            http,
            api_base_url: config.oura.api_base_url.trim_end_matches('/').to_owned(),
            client_id,
            client_secret,
        })
    }

    async fn list(&self) -> Result<Vec<RemoteWebhookSubscription>> {
        self.send(
            reqwest::Method::GET,
            "/v2/webhook/subscription",
            None::<&()>,
        )
        .await
    }

    async fn create(&self, desired: &DesiredWebhookSubscriptionTarget) -> Result<()> {
        self.send(
            reqwest::Method::POST,
            "/v2/webhook/subscription",
            Some(&CreateWebhookSubscriptionRequest {
                callback_url: &desired.callback_url,
                verification_token: &desired.verification_token,
                event_type: desired.event_type,
                data_type: &desired.data_type,
            }),
        )
        .await
    }

    async fn update(&self, update: &SubscriptionUpdate) -> Result<()> {
        self.send(
            reqwest::Method::PUT,
            &format!("/v2/webhook/subscription/{}", update.existing.id),
            Some(&UpdateWebhookSubscriptionRequest {
                verification_token: &update.desired.verification_token,
                callback_url: Some(&update.desired.callback_url),
                event_type: Some(update.desired.event_type),
                data_type: Some(&update.desired.data_type),
            }),
        )
        .await
    }

    async fn renew(&self, subscription: &RemoteWebhookSubscription) -> Result<()> {
        self.send_empty(
            reqwest::Method::PUT,
            &format!("/v2/webhook/subscription/renew/{}", subscription.id),
        )
        .await
    }

    async fn delete(&self, subscription: &RemoteWebhookSubscription) -> Result<()> {
        self.send_empty(
            reqwest::Method::DELETE,
            &format!("/v2/webhook/subscription/{}", subscription.id),
        )
        .await
    }

    async fn apply(&self, plan: &SubscriptionSyncPlan) -> Result<()> {
        for desired in &plan.create {
            self.create(desired).await?;
        }
        for update in &plan.update {
            self.update(update).await?;
        }
        for subscription in &plan.renew {
            self.renew(subscription).await?;
        }
        for subscription in &plan.prune {
            self.delete(subscription).await?;
        }
        Ok(())
    }

    async fn send<T, B>(&self, method: reqwest::Method, path: &str, body: Option<&B>) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
        B: Serialize + ?Sized,
    {
        let url = format!("{}{}", self.api_base_url, path);
        let builder = self
            .http
            .request(method, url)
            .header("x-client-id", &self.client_id)
            .header("x-client-secret", &self.client_secret);
        let response = if let Some(body) = body {
            builder.json(body).send().await?
        } else {
            builder.send().await?
        };
        let status = response.status();
        if status == reqwest::StatusCode::NO_CONTENT {
            let payload = serde_json::from_str("null")
                .map_err(|error| RingmasterError::Config(format!("null decode failed: {error}")))?;
            return Ok(payload);
        }
        let text = response.text().await?;
        if !status.is_success() {
            return Err(OuraApiError::Problem(parse_api_problem(status, &text)).into());
        }
        serde_json::from_str(&text).map_err(|error| {
            OuraApiError::UnexpectedResponse(format!(
                "failed to decode webhook admin response: {error}"
            ))
            .into()
        })
    }

    async fn send_empty(&self, method: reqwest::Method, path: &str) -> Result<()> {
        let url = format!("{}{}", self.api_base_url, path);
        let response = self
            .http
            .request(method, url)
            .header("x-client-id", &self.client_id)
            .header("x-client-secret", &self.client_secret)
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(OuraApiError::Problem(parse_api_problem(status, &text)).into());
        }
        Ok(())
    }
}

impl FixtureWebhookAdminClient {
    fn load(fixture_dir: PathBuf) -> Result<Self> {
        let remote_path = fixture_dir.join(FIXTURE_REMOTE_FILE);
        let payload = std::fs::read_to_string(&remote_path)
            .map_err(|error| RingmasterError::io("reading webhook subscription fixture", error))?;
        let subscriptions = serde_json::from_str(&payload)?;

        Ok(Self { subscriptions })
    }

    async fn list(&self) -> Result<Vec<RemoteWebhookSubscription>> {
        Ok(self.subscriptions.clone())
    }

    async fn apply(&mut self, plan: &SubscriptionSyncPlan) -> Result<()> {
        for desired in &plan.create {
            self.subscriptions.push(RemoteWebhookSubscription {
                id: format!(
                    "fixture-{}-{}",
                    desired.data_type,
                    desired.event_type.as_str()
                ),
                callback_url: desired.callback_url.clone(),
                event_type: desired.event_type,
                data_type: desired.data_type.clone(),
                expiration_time: one_year_from_now()?,
            });
        }

        for update in &plan.update {
            if let Some(existing) = self
                .subscriptions
                .iter_mut()
                .find(|candidate| candidate.id == update.existing.id)
            {
                existing
                    .callback_url
                    .clone_from(&update.desired.callback_url);
                existing.event_type = update.desired.event_type;
                existing.data_type.clone_from(&update.desired.data_type);
            }
        }

        for renew in &plan.renew {
            if let Some(existing) = self
                .subscriptions
                .iter_mut()
                .find(|candidate| candidate.id == renew.id)
            {
                existing.expiration_time = one_year_from_now()?;
            }
        }

        for prune in &plan.prune {
            self.subscriptions
                .retain(|candidate| candidate.id != prune.id);
        }

        Ok(())
    }
}

fn admin_client(config: &Config, fixture_dir: Option<PathBuf>) -> Result<WebhookAdminClient> {
    match fixture_dir {
        Some(path) => Ok(WebhookAdminClient::Fixture(
            FixtureWebhookAdminClient::load(path)?,
        )),
        None => Ok(WebhookAdminClient::Live(LiveWebhookAdminClient::new(
            config,
        )?)),
    }
}

fn desired_targets(
    config: &Config,
    fixture_dir: Option<&Path>,
) -> Result<Vec<DesiredWebhookSubscriptionTarget>> {
    let context = fixture_dir.map(load_fixture_context).transpose()?.flatten();
    let sync_context = SyncContext {
        callback_url: resolve_callback_url(config, context.as_ref())?,
        verification_token: resolve_verification_token(config, context.as_ref())?,
    };

    let mut desired = Vec::new();
    for subscription in &config.webhook.subscriptions {
        if !subscription.enabled {
            continue;
        }
        for event_type in subscription.normalized_event_types() {
            desired.push(DesiredWebhookSubscriptionTarget {
                data_type: subscription.data_type.clone(),
                event_type,
                callback_url: sync_context.callback_url.clone(),
                verification_token: sync_context.verification_token.clone(),
            });
        }
    }

    if desired.is_empty() {
        return Err(RingmasterError::Config(
            "webhook sync requires at least one enabled desired subscription".to_owned(),
        ));
    }

    desired.sort_by(|left, right| {
        left.data_type
            .cmp(&right.data_type)
            .then(left.event_type.cmp(&right.event_type))
    });
    Ok(desired)
}

fn persist_desired_snapshot(
    config: &Config,
    store: &Store,
    desired: &[DesiredWebhookSubscriptionTarget],
) -> Result<()> {
    let updated_at = now_rfc3339()?;
    let callback_url = config.webhook.callback_url();
    let records = desired
        .iter()
        .map(|entry| DesiredWebhookSubscriptionRecord {
            data_type: entry.data_type.clone(),
            event_type: entry.event_type,
            enabled: true,
            callback_url: callback_url
                .clone()
                .or_else(|| Some(entry.callback_url.clone())),
            updated_at: updated_at.clone(),
        })
        .collect::<Vec<_>>();
    store.webhook().replace_desired_subscriptions(&records)
}

fn persist_remote_snapshot(
    config: &Config,
    store: &Store,
    desired: &[DesiredWebhookSubscriptionTarget],
    remote: &[RemoteWebhookSubscription],
) -> Result<()> {
    let desired_by_key = desired
        .iter()
        .map(|entry| {
            (
                SubscriptionKey {
                    data_type: entry.data_type.clone(),
                    event_type: entry.event_type,
                },
                entry,
            )
        })
        .collect::<HashMap<_, _>>();
    let duplicate_counts = remote.iter().fold(
        HashMap::<SubscriptionKey, usize>::new(),
        |mut counts, subscription| {
            *counts
                .entry(SubscriptionKey {
                    data_type: subscription.data_type.clone(),
                    event_type: subscription.event_type,
                })
                .or_default() += 1;
            counts
        },
    );
    let seen_at = now_rfc3339()?;
    let records = remote
        .iter()
        .map(|subscription| {
            let key = SubscriptionKey {
                data_type: subscription.data_type.clone(),
                event_type: subscription.event_type,
            };
            let drift_status = if duplicate_counts.get(&key).copied().unwrap_or_default() > 1 {
                "duplicate"
            } else if let Some(desired_entry) = desired_by_key.get(&key) {
                if subscription.callback_url == desired_entry.callback_url {
                    "matched"
                } else {
                    "diverged"
                }
            } else if config
                .webhook
                .subscriptions
                .iter()
                .any(|entry| entry.data_type == subscription.data_type && !entry.enabled)
            {
                "disabled"
            } else {
                "unexpected"
            };

            RemoteWebhookSubscriptionRecord {
                subscription_id: subscription.id.clone(),
                callback_url: subscription.callback_url.clone(),
                event_type: subscription.event_type,
                data_type: subscription.data_type.clone(),
                expiration_time: subscription.expiration_time.clone(),
                drift_status: drift_status.to_owned(),
                last_seen_at: seen_at.clone(),
                created_at: seen_at.clone(),
                updated_at: seen_at.clone(),
            }
        })
        .collect::<Vec<_>>();
    store.webhook().replace_remote_subscriptions(&records)
}

fn resolve_callback_url(
    config: &Config,
    fixture_context: Option<&FixtureSubscriptionContext>,
) -> Result<String> {
    config
        .webhook
        .callback_url()
        .or_else(|| fixture_context.and_then(|context| context.callback_url.clone()))
        .ok_or_else(|| {
            RingmasterError::Config(
                "webhook subscription sync requires webhook.public_base_url or a fixture context callback_url"
                    .to_owned(),
            )
        })
}

fn resolve_verification_token(
    config: &Config,
    fixture_context: Option<&FixtureSubscriptionContext>,
) -> Result<String> {
    config
        .webhook
        .verification_token
        .clone()
        .or_else(|| fixture_context.and_then(|context| context.verification_token.clone()))
        .ok_or_else(|| {
            RingmasterError::Config(
                "webhook subscription sync requires webhook.verification_token or a fixture context verification_token"
                    .to_owned(),
            )
        })
}

fn load_fixture_context(fixture_dir: &Path) -> Result<Option<FixtureSubscriptionContext>> {
    let path = fixture_dir.join(FIXTURE_CONTEXT_FILE);
    if !path.is_file() {
        return Ok(None);
    }
    let payload = std::fs::read_to_string(&path).map_err(|error| {
        RingmasterError::io("reading webhook subscription fixture context", error)
    })?;
    let context = serde_json::from_str(&payload)?;
    Ok(Some(context))
}

fn expires_within(
    expiration_time: &str,
    now: OffsetDateTime,
    renewal_lead: Duration,
) -> Result<bool> {
    let expiration = OffsetDateTime::parse(expiration_time, &Rfc3339).map_err(|error| {
        RingmasterError::Config(format!(
            "invalid webhook expiration timestamp `{expiration_time}`: {error}"
        ))
    })?;
    Ok(expiration <= now + renewal_lead)
}

fn one_year_from_now() -> Result<String> {
    OffsetDateTime::now_utc()
        .checked_add(Duration::days(365))
        .ok_or_else(|| RingmasterError::Config("webhook renewal timestamp overflowed".to_owned()))?
        .format(&Rfc3339)
        .map_err(|error| RingmasterError::Config(format!("formatting timestamp failed: {error}")))
}

fn parse_api_problem(status: reqwest::StatusCode, payload: &str) -> OuraProblem {
    serde_json::from_str::<OuraProblem>(payload).unwrap_or_else(|_| {
        OuraProblem::new(
            Some(status.as_u16()),
            "Webhook admin request failed",
            Some(payload.trim().to_owned()),
        )
    })
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::{
        DesiredWebhookSubscriptionTarget, RemoteWebhookSubscription, SubscriptionSyncOptions,
        build_sync_plan, list_subscriptions, sync_subscriptions,
    };
    use crate::config::Config;
    use crate::store::Store;
    use crate::webhook::WebhookEventType;
    use tempfile::tempdir;

    #[test]
    fn builds_create_update_renew_and_prune_plan() {
        let desired = vec![
            DesiredWebhookSubscriptionTarget {
                data_type: "daily_sleep".to_owned(),
                event_type: WebhookEventType::Create,
                callback_url: "https://example.test/webhooks/oura".to_owned(),
                verification_token: "token".to_owned(),
            },
            DesiredWebhookSubscriptionTarget {
                data_type: "workout".to_owned(),
                event_type: WebhookEventType::Update,
                callback_url: "https://example.test/webhooks/oura".to_owned(),
                verification_token: "token".to_owned(),
            },
        ];
        let remote = vec![
            RemoteWebhookSubscription {
                id: "sub-1".to_owned(),
                callback_url: "https://other.test/webhooks/oura".to_owned(),
                event_type: WebhookEventType::Create,
                data_type: "daily_sleep".to_owned(),
                expiration_time: "2099-01-01T00:00:00Z".to_owned(),
            },
            RemoteWebhookSubscription {
                id: "sub-2".to_owned(),
                callback_url: "https://example.test/webhooks/oura".to_owned(),
                event_type: WebhookEventType::Update,
                data_type: "workout".to_owned(),
                expiration_time: "2000-01-01T00:00:00Z".to_owned(),
            },
            RemoteWebhookSubscription {
                id: "sub-3".to_owned(),
                callback_url: "https://unexpected.test/webhooks/oura".to_owned(),
                event_type: WebhookEventType::Delete,
                data_type: "session".to_owned(),
                expiration_time: "2099-01-01T00:00:00Z".to_owned(),
            },
        ];

        let plan = build_sync_plan(&desired, &remote, 60, true)
            .unwrap_or_else(|error| panic!("plan should build: {error}"));

        assert_eq!(plan.create.len(), 0);
        assert_eq!(plan.update.len(), 1);
        assert_eq!(plan.renew.len(), 1);
        assert_eq!(plan.prune.len(), 1);
        assert_eq!(plan.update[0].existing.id, "sub-1");
        assert_eq!(plan.renew[0].id, "sub-2");
        assert_eq!(plan.prune[0].id, "sub-3");
    }

    #[tokio::test]
    async fn fixture_backed_sync_dry_run_persists_snapshots() {
        let fixture_dir =
            tempdir().unwrap_or_else(|error| panic!("tempdir should succeed: {error}"));
        std::fs::write(
            fixture_dir.path().join("subscriptions.context.json"),
            r#"{"callback_url":"https://fixture.test/webhooks/oura","verification_token":"fixture-token"}"#,
        )
        .unwrap_or_else(|error| panic!("context fixture write should succeed: {error}"));
        std::fs::write(
            fixture_dir.path().join("subscriptions.remote.json"),
            r#"[{"id":"sub-1","callback_url":"https://fixture.test/webhooks/oura","event_type":"create","data_type":"daily_sleep","expiration_time":"2099-01-01T00:00:00Z"}]"#,
        )
        .unwrap_or_else(|error| panic!("remote fixture write should succeed: {error}"));

        let config = Config::load().unwrap_or_else(|error| panic!("config should load: {error}"));
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let report = sync_subscriptions(
            &config,
            &store,
            SubscriptionSyncOptions {
                dry_run: true,
                prune: false,
                fixture_dir: Some(fixture_dir.path().to_path_buf()),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("fixture sync should succeed: {error}"));

        assert!(report.dry_run);
        assert_eq!(report.remote_before.len(), 1);
        assert!(
            !store
                .webhook()
                .list_desired_subscriptions()
                .unwrap_or_else(|error| panic!("desired snapshot should load: {error}"))
                .is_empty()
        );
        assert_eq!(
            store
                .webhook()
                .list_remote_subscriptions()
                .unwrap_or_else(|error| panic!("remote snapshot should load: {error}"))
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn fixture_list_uses_context_when_local_config_is_incomplete() {
        let fixture_dir =
            tempdir().unwrap_or_else(|error| panic!("tempdir should succeed: {error}"));
        std::fs::write(
            fixture_dir.path().join("subscriptions.context.json"),
            r#"{"callback_url":"https://fixture.test/webhooks/oura","verification_token":"fixture-token"}"#,
        )
        .unwrap_or_else(|error| panic!("context fixture write should succeed: {error}"));
        std::fs::write(fixture_dir.path().join("subscriptions.remote.json"), r"[]")
            .unwrap_or_else(|error| panic!("remote fixture write should succeed: {error}"));

        let config = Config::load().unwrap_or_else(|error| panic!("config should load: {error}"));
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let report = list_subscriptions(&config, &store, Some(fixture_dir.path().to_path_buf()))
            .await
            .unwrap_or_else(|error| panic!("fixture list should succeed: {error}"));

        assert_eq!(
            report.callback_url.as_deref(),
            Some("https://fixture.test/webhooks/oura")
        );
        assert!(report.remote.is_empty());
        assert!(!report.desired.is_empty());
    }
}
