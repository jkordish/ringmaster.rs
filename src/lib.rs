//! App-first public facade for `ringmaster`.
//!
//! The supported library surface is intentionally small: `run_from`, CLI parsing
//! types, and the top-level error/result types. UI, sync, storage, and webhook
//! internals stay crate-private so the application can evolve without turning
//! every refactor into a semver promise.
//!
//! ```rust
//! use clap::Parser;
//! use ringmaster::cli::{Cli, Command};
//!
//! let cli = Cli::parse_from(["ringmaster", "doctor"]).unwrap();
//! assert!(matches!(cli.command, Some(Command::Doctor)));
//! ```
//!
#![forbid(unsafe_code)]
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    clippy::perf
)]
#![allow(clippy::multiple_crate_versions, clippy::too_many_lines)]

mod action;
mod ai;
mod ai_prompts;
mod app;
pub mod cli;
mod components;
mod config;
mod derive;
mod error;
mod eval;
mod evidence;
mod focus;
mod insights;
mod keybindings;
mod navigation;
mod numeric;
mod oura;
mod refresh;
mod report;
mod review;
mod snapshot;
mod store;
#[cfg(test)]
pub(crate) mod test_support;
mod time_utils;
mod tui;
mod ui;
mod webhook;

pub use error::{Result, RingmasterError};

use std::collections::HashMap;
use std::fs;
use std::io::{IsTerminal, stdin, stdout};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::evidence::{evidence_registry_version, stale_evidence_warnings};
use crate::time_utils::current_local_day_string;
use app::{build_demo_state, build_live_state, build_read_only_live_state, load_live_snapshot};
use cli::{
    AiCommand, AiCompareArgs, AiEvalArgs, AiReviewArgs, AiRunsCommand, AiRunsListArgs,
    AiRunsShowArgs, AuthCommand, Cli, Command, DeriveCommand, DeriveRebuildArgs, ReportCommand,
    ReportExportArgs, ReviewCommand, ReviewFocusArg, ReviewInvestigateArgs, ReviewTodayArgs,
    ReviewWeekArgs, SnapshotColorModeArg, SnapshotCommand, SnapshotExportArgs, SnapshotListArgs,
    SnapshotScreenArg, SnapshotShowArgs, SnapshotSizeArg, SyncCommand, SyncOnceArgs, SyncWatchArgs,
    TuiArgs, UiCommand, UiSnapshotArgs, WebhookCommand, WebhookReplayArgs, WebhookServeArgs,
    WebhookSubscriptionCommand, WebhookSubscriptionsListArgs, WebhookSubscriptionsSyncArgs,
};
use config::{AppPaths, Config};
use refresh::{SyncFamily, WatchOptions};
use review::{
    InvestigationReport, ReviewCard, ReviewDeck, ReviewFocus, ReviewInputs, ReviewMode,
    build_investigation_report, build_review_deck,
};
use store::Store;
use time::{Date, Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use ui::layout::DEFAULT_NON_INTERACTIVE_VIEWPORT;

static LOGGING_INIT: OnceLock<std::result::Result<(), String>> = OnceLock::new();

#[derive(Debug)]
struct TempRootGuard {
    path: PathBuf,
}

const FIXTURE_SNAPSHOT_WEBHOOK_BIND_ADDRESS: &str = "127.0.0.1:8799";
const FIXTURE_SNAPSHOT_WEBHOOK_PATH: &str = "/webhooks/oura";
const FIXTURE_SNAPSHOT_WEBHOOK_CALLBACK_URL: &str = "https://fixture.example.test/webhooks/oura";
const FIXTURE_SNAPSHOT_AUTH_CALLBACK_URL: &str = "http://localhost:8788/callback";
const FIXTURE_SNAPSHOT_AUTH_TIMEOUT_SECS: u64 = 120;
const FIXTURE_SNAPSHOT_SECRET_BACKEND: &str = "fixture-memory";
const FIXTURE_SNAPSHOT_ACCESS_TOKEN_EXPIRES_AT: &str = "2026-04-09T12:45:00Z";
const FIXTURE_SNAPSHOT_LAST_AUTHENTICATED_AT: &str = "2026-04-09T08:45:00Z";
const FIXTURE_SNAPSHOT_LAST_REFRESH_AT: &str = "2026-04-09T11:45:00Z";
const FIXTURE_SNAPSHOT_ACCOUNT_ID: &str = "fixture-user";
const FIXTURE_SNAPSHOT_ACCOUNT_EMAIL: &str = "fixture@example.com";
const FIXTURE_SNAPSHOT_PERSONAL_INTERVAL_SECS: u64 = 3_600;
const FIXTURE_SNAPSHOT_DAILY_INTERVAL_SECS: u64 = 300;
const FIXTURE_SNAPSHOT_HEARTRATE_INTERVAL_SECS: u64 = 60;
const FIXTURE_SNAPSHOT_WORKOUT_INTERVAL_SECS: u64 = 600;
const FIXTURE_SNAPSHOT_ENHANCED_TAG_INTERVAL_SECS: u64 = 300;
const FIXTURE_SNAPSHOT_SESSION_INTERVAL_SECS: u64 = 300;
const FIXTURE_SNAPSHOT_PERSONAL_STALE_AFTER_SECS: u64 = 72 * 60 * 60;
const FIXTURE_SNAPSHOT_DAILY_STALE_AFTER_SECS: u64 = 12 * 60 * 60;
const FIXTURE_SNAPSHOT_HEARTRATE_STALE_AFTER_SECS: u64 = 15 * 60;
const FIXTURE_SNAPSHOT_WORKOUT_STALE_AFTER_SECS: u64 = 24 * 60 * 60;
const FIXTURE_SNAPSHOT_ENHANCED_TAG_STALE_AFTER_SECS: u64 = 12 * 60 * 60;
const FIXTURE_SNAPSHOT_SESSION_STALE_AFTER_SECS: u64 = 12 * 60 * 60;
const FIXTURE_SNAPSHOT_BASE_SYNC_ATTEMPTED_AT: &str = "2026-04-09T11:58:00Z";
const FIXTURE_SNAPSHOT_BASE_SYNC_COMPLETED_AT: &str = "2026-04-09T11:59:00Z";
const FIXTURE_SNAPSHOT_STALE_SYNC_ATTEMPTED_AT: &str = "2026-04-09T05:00:00Z";
const FIXTURE_SNAPSHOT_STALE_SYNC_COMPLETED_AT: &str = "2026-04-09T05:01:00Z";
const FIXTURE_SNAPSHOT_ERROR_SYNC_ATTEMPTED_AT: &str = "2026-04-09T11:45:00Z";
const FIXTURE_SNAPSHOT_ERROR_SYNC_COMPLETED_AT: &str = "2026-04-09T11:46:00Z";

/// Parses CLI arguments and runs the requested command.
///
/// # Errors
///
/// Returns an error when argument parsing fails, configuration or runtime
/// dependencies cannot be initialized, or the selected command fails.
pub fn run_from<I, T>(args: I) -> impl std::future::Future<Output = Result<Option<String>>> + Send
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    async move {
        let cli = cli?;
        if cli.command.is_none() {
            return Ok(Some(Cli::help_text()));
        }

        run_cli(cli).await
    }
}

/// Executes a parsed CLI command.
///
/// # Errors
///
/// Returns an error when configuration cannot be loaded, logging or runtime
/// directories cannot be initialized, or the chosen command fails.
pub async fn run_cli(cli: Cli) -> Result<Option<String>> {
    let config = Config::load()?;
    init_logging(&config.logging.filter)?;
    config.paths.ensure_runtime_dirs()?;

    let command = cli
        .command
        .ok_or_else(|| RingmasterError::Cli(Cli::help_text()))?;
    info!(?command, "starting ringmaster command");

    match command {
        Command::Doctor => run_doctor(&config),
        Command::Demo => run_demo(&config),
        Command::Tui(args) => run_tui(&config, &args),
        Command::Snapshot { command } => match command {
            SnapshotCommand::Export(args) => run_snapshot_export(&config, args).await,
            SnapshotCommand::List(args) => run_snapshot_list(&config, args).await,
            SnapshotCommand::Show(args) => run_snapshot_show(&config, args).await,
        },
        Command::Ui { command } => match command {
            UiCommand::Snapshot(args) => run_ui_snapshot(&config, args).await,
        },
        Command::Auth {
            command: AuthCommand::Login,
        } => run_auth_login(&config).await,
        Command::Sync { command } => match command {
            SyncCommand::Once(args) => run_sync_once(&config, args).await,
            SyncCommand::Watch(args) => run_sync_watch(&config, args).await,
        },
        Command::Webhook { command } => match command {
            WebhookCommand::Serve(args) => run_webhook_serve(&config, args).await,
            WebhookCommand::Replay(args) => run_webhook_replay(&config, args).await,
            WebhookCommand::Subscriptions { command } => match command {
                WebhookSubscriptionCommand::List(args) => {
                    run_webhook_subscriptions_list(&config, args).await
                }
                WebhookSubscriptionCommand::Sync(args) => {
                    run_webhook_subscriptions_sync(&config, args).await
                }
            },
        },
        Command::Derive { command } => match command {
            DeriveCommand::Rebuild(args) => run_derive_rebuild(&config, args).await,
        },
        Command::Review { command } => match command {
            ReviewCommand::Today(args) => run_review_today(&config, args).await,
            ReviewCommand::Week(args) => run_review_week(&config, args).await,
            ReviewCommand::Investigate(args) => run_review_investigate(&config, args).await,
        },
        Command::Ai { command } => match command {
            AiCommand::Review(args) => run_ai_review(&config, args).await,
            AiCommand::Compare(args) => run_ai_compare(&config, args).await,
            AiCommand::Runs { command } => match command {
                AiRunsCommand::List(args) => run_ai_runs_list(&config, args).await,
                AiRunsCommand::Show(args) => run_ai_runs_show(&config, args).await,
            },
            AiCommand::Eval(args) => run_ai_eval(&config, &args),
        },
        Command::Report { command } => match command {
            ReportCommand::Export(args) => run_report_export(&config, args).await,
        },
    }
}

fn run_doctor(config: &Config) -> Result<Option<String>> {
    let store = Store::open(config)?;
    let auth_status = oura::auth::inspect_auth(config, &store)?;
    let snapshot = load_live_snapshot(config, &store, &auth_status)?;
    Ok(Some(render_doctor_report(
        config,
        &store,
        &auth_status,
        &snapshot,
    )?))
}

struct DoctorWebhookStats {
    desired_enabled: usize,
    remote_healthy: usize,
    renewals_due: usize,
    last_delivery: String,
    last_rejection: String,
    queue_oldest: String,
    failed_attempts: usize,
}

fn render_doctor_report(
    config: &Config,
    store: &Store,
    auth_status: &oura::models::AuthStatus,
    snapshot: &app::LiveSnapshot,
) -> Result<String> {
    let webhook_stats = doctor_webhook_stats(snapshot);
    Ok([
        "ringmaster doctor".to_owned(),
        String::new(),
        doctor_runtime_section(config, store)?,
        doctor_auth_section(auth_status),
        doctor_sync_section(config, snapshot),
        doctor_webhook_section(config, snapshot, &webhook_stats),
        doctor_ai_section(snapshot),
        doctor_record_counts_section(snapshot),
    ]
    .join("\n"))
}

fn doctor_runtime_section(config: &Config, store: &Store) -> Result<String> {
    let stale_evidence = stale_evidence_warnings(OffsetDateTime::now_utc().date());
    let stale_entries = if stale_evidence.is_empty() {
        "none".to_owned()
    } else {
        stale_evidence.join(" | ")
    };

    Ok([
        format!("app_name: {}", config.app_name),
        format!("config_dir: {}", config.paths.config_dir.display()),
        format!(
            "config_file: {} ({})",
            config.paths.config_file.display(),
            if config.paths.config_file_present() {
                "present"
            } else {
                "missing"
            }
        ),
        format!("state_dir: {}", config.paths.state_dir.display()),
        format!("cache_dir: {}", config.paths.cache_dir.display()),
        format!(
            "database_path: {} ({})",
            config.paths.database_file.display(),
            if config.paths.database_present() {
                "present"
            } else {
                "created if needed"
            }
        ),
        format!("log_dir: {}", config.paths.log_dir.display()),
        format!("schema_version: {}", store.metadata().schema_version()?),
        format!("evidence_registry_version: {}", evidence_registry_version()),
        format!(
            "migrations_applied_this_run: {}",
            store.migration_report().applied_versions.len()
        ),
        format!(
            "active_population_profile: {} ({})",
            config.guidance.active_population_profile.as_str(),
            config.guidance.source_label()
        ),
        format!("stale_evidence_entries: {stale_entries}"),
    ]
    .join("\n"))
}

fn doctor_auth_section(auth_status: &oura::models::AuthStatus) -> String {
    format!(
        "\
oauth_callback: {}
auth_configured: {}
auth_state: {}
secret_backend: {}
access_token_stored: {}
refresh_token_stored: {}
access_token_expires_at: {}
last_authenticated_at: {}
last_refresh_at: {}
account_id: {}
account_email: {}
auth_last_error: {}
missing_auth_fields: {}
capabilities:
{}",
        auth_status.callback_url,
        auth_status.configured,
        doctor_auth_state(auth_status),
        auth_status.secret_backend,
        auth_status.access_token_stored,
        auth_status.refresh_token_stored,
        auth_status
            .access_token_expires_at
            .clone()
            .unwrap_or_else(|| "unknown".to_owned()),
        auth_status
            .last_authenticated_at
            .clone()
            .unwrap_or_else(|| "never".to_owned()),
        auth_status
            .last_refresh_at
            .clone()
            .unwrap_or_else(|| "never".to_owned()),
        auth_status
            .account_id
            .clone()
            .unwrap_or_else(|| "unknown".to_owned()),
        auth_status
            .account_email
            .clone()
            .unwrap_or_else(|| "unknown".to_owned()),
        auth_status
            .last_error
            .as_ref()
            .map_or_else(|| "none".to_owned(), ToString::to_string),
        if auth_status.missing_fields.is_empty() {
            "none".to_owned()
        } else {
            auth_status.missing_fields.join(", ")
        },
        doctor_capability_lines(auth_status),
    )
}

fn doctor_sync_section(config: &Config, snapshot: &app::LiveSnapshot) -> String {
    format!(
        "\
refresh_policy:
{}
demo_fixture_dir: {}
sync_slices:
{}",
        doctor_refresh_lines(config),
        config.refresh.demo_fixture_dir.as_ref().map_or_else(
            || "tests/fixtures/phase5".to_owned(),
            |path| path.display().to_string()
        ),
        doctor_sync_lines(snapshot),
    )
}

fn doctor_webhook_section(
    config: &Config,
    snapshot: &app::LiveSnapshot,
    stats: &DoctorWebhookStats,
) -> String {
    format!(
        "\
webhook_receiver_configured: {}
webhook_callback_url: {}
webhook_verification_token_configured: {}
webhook_receiver_status: {}
webhook_receiver_heartbeat: {}
webhook_watch_heartbeat: {}
webhook_runtime_mode: {}
webhook_missing_public_prereq: {}
webhook_desired_subscriptions: {}
webhook_remote_subscriptions: {}
webhook_remote_healthy: {}
webhook_remote_renewals_due: {}
webhook_last_delivery: {}
webhook_last_rejection: {}
webhook_queue_depth: {}
webhook_queue_oldest: {}
webhook_failed_attempts: {}",
        config.webhook_receiver_configured(),
        snapshot
            .webhook
            .callback_url
            .clone()
            .unwrap_or_else(|| "unconfigured".to_owned()),
        snapshot.webhook.verification_token_configured,
        doctor_receiver_status(snapshot),
        doctor_heartbeat_status(snapshot, "webhook.receiver"),
        doctor_heartbeat_status(snapshot, "sync.watch"),
        doctor_runtime_mode(snapshot),
        snapshot.webhook.callback_url.is_none(),
        stats.desired_enabled,
        snapshot.webhook.remote_subscriptions.len(),
        stats.remote_healthy,
        stats.renewals_due,
        stats.last_delivery,
        stats.last_rejection,
        snapshot.webhook.pending_invalidations.len(),
        stats.queue_oldest,
        stats.failed_attempts,
    )
}

fn doctor_ai_section(snapshot: &app::LiveSnapshot) -> String {
    format!(
        "\
ai_provider_enabled: {}
ai_provider: {}
ai_api_key_env: {}
ai_api_key_ready: {}
ai_default_model: {}
ai_request_mode: {}
ai_input_transport: {}
ai_prompt_cache: {}
ai_stateless_default: {}
ai_tools_disabled_by_default: {}
ai_prompt_versions: review={} compare={}
ai_last_successful_run: {}
ai_last_failed_run: {}
ai_artifact_registry: snapshots={} runs={} artifacts={} reports={} evals={}",
        snapshot.ai_ops.enabled,
        snapshot.ai_ops.provider,
        snapshot.ai_ops.api_key_env,
        snapshot.ai_ops.api_key_ready,
        snapshot.ai_ops.default_model,
        snapshot.ai_ops.request_mode,
        snapshot.ai_ops.input_transport,
        snapshot.ai_ops.prompt_cache,
        snapshot.ai_ops.request_mode == crate::config::AiRequestMode::Stateless.as_str(),
        snapshot.ai_ops.tools_disabled,
        snapshot.ai_ops.review_prompt_version,
        snapshot.ai_ops.compare_prompt_version,
        snapshot
            .ai_ops
            .last_successful_run
            .clone()
            .unwrap_or_else(|| "none".to_owned()),
        snapshot
            .ai_ops
            .last_failed_run
            .clone()
            .unwrap_or_else(|| "none".to_owned()),
        snapshot.ai_ops.snapshot_catalog_count,
        snapshot.ai_ops.ai_run_count,
        snapshot.ai_ops.ai_artifact_count,
        snapshot.ai_ops.report_export_count,
        snapshot.ai_ops.ai_eval_run_count,
    )
}

fn doctor_record_counts_section(snapshot: &app::LiveSnapshot) -> String {
    let counts = &snapshot.record_counts;
    format!(
        "\
record_counts:
  personal_info: {}
  daily_sleep: {}
  daily_readiness: {}
  daily_activity: {}
  heartrate_samples: {}
  workouts: {}
  tags: {}
  enhanced_tags: {}
  sessions: {}
  derived_context_events: {}
  derived_pattern_summaries: {}
  derived_review_signal_days: {}
  raw_payloads: {}",
        counts.personal_info,
        counts.daily_sleep,
        counts.daily_readiness,
        counts.daily_activity,
        counts.heartrate_samples,
        counts.workouts,
        counts.tags,
        counts.enhanced_tags,
        counts.sessions,
        counts.derived_context_events,
        counts.derived_pattern_summaries,
        counts.derived_review_signal_days,
        counts.raw_payloads,
    )
}

const fn doctor_auth_state(auth_status: &oura::models::AuthStatus) -> &'static str {
    if auth_status.access_token_stored || auth_status.refresh_token_stored {
        "authenticated"
    } else if auth_status.configured {
        "configured_without_session"
    } else {
        "unconfigured"
    }
}

fn doctor_capability_lines(auth_status: &oura::models::AuthStatus) -> String {
    auth_status
        .capability_report
        .entries
        .iter()
        .map(|entry| {
            format!(
                "  - {}: {} ({})",
                entry.kind.label(),
                if entry.granted {
                    "granted"
                } else if entry.requested {
                    "missing"
                } else {
                    "not-requested"
                },
                entry.note
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn doctor_refresh_lines(config: &Config) -> String {
    SyncFamily::ALL
        .into_iter()
        .map(|family| {
            format!(
                "  - {}: interval={}s stale_after={}s scope={}",
                family.label(),
                family.interval_secs(&config.refresh),
                family.stale_after_secs(&config.refresh),
                family.capability_kind().scope_name()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn doctor_sync_lines(snapshot: &app::LiveSnapshot) -> String {
    if snapshot.sync_states.is_empty() {
        return "  - none".to_owned();
    }

    snapshot
        .sync_states
        .iter()
        .map(|sync| {
            let error = sync
                .last_error
                .as_ref()
                .map(|problem| format!(" | error={problem}"))
                .unwrap_or_default();
            let backoff = sync
                .next_attempt_after
                .as_deref()
                .map(|value| format!(" | next_attempt_after={value}"))
                .unwrap_or_default();
            format!(
                "  - {}: {} at {} | failures={}{}{}",
                sync.sync_key,
                sync.status,
                sync.last_attempted_at,
                sync.failure_count,
                backoff,
                error
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn doctor_webhook_stats(snapshot: &app::LiveSnapshot) -> DoctorWebhookStats {
    DoctorWebhookStats {
        desired_enabled: snapshot
            .webhook
            .desired_subscriptions
            .iter()
            .filter(|record| record.enabled)
            .count(),
        remote_healthy: snapshot
            .webhook
            .remote_subscriptions
            .iter()
            .filter(|record| doctor_remote_subscription_healthy(snapshot, record))
            .count(),
        renewals_due: snapshot
            .webhook
            .remote_subscriptions
            .iter()
            .filter(|record| {
                doctor_remote_subscription_renewal_due(snapshot, &record.expiration_time)
            })
            .count(),
        last_delivery: snapshot.webhook.recent_deliveries.first().map_or_else(
            || "none".to_owned(),
            |record| {
                format!(
                    "{} {} {}",
                    record.data_type.as_deref().unwrap_or("unknown"),
                    record
                        .event_type
                        .map_or("unknown", |event_type| event_type.as_str()),
                    record.received_at
                )
            },
        ),
        last_rejection: snapshot
            .webhook
            .latest_rejected_delivery
            .as_ref()
            .map_or_else(
                || "none".to_owned(),
                |record| {
                    format!(
                        "{} {} ({})",
                        record.reason_code, record.received_at, record.detail
                    )
                },
            ),
        queue_oldest: snapshot
            .webhook
            .pending_invalidations
            .iter()
            .map(|record| record.first_queued_at.as_str())
            .min()
            .unwrap_or("none")
            .to_owned(),
        failed_attempts: snapshot
            .webhook
            .recent_processing_attempts
            .iter()
            .filter(|attempt| attempt.outcome == "failed")
            .count(),
    }
}

fn doctor_receiver_status(snapshot: &app::LiveSnapshot) -> String {
    if !doctor_receiver_config_complete(snapshot) {
        return "config incomplete".to_owned();
    }

    let Some(record) = snapshot
        .webhook
        .runtime_heartbeats
        .iter()
        .find(|record| record.component == "webhook.receiver")
    else {
        return "missing heartbeat".to_owned();
    };

    if doctor_heartbeat_active(snapshot, record) {
        "healthy".to_owned()
    } else if record.mode == "stopped" {
        format!("stopped ({})", record.last_seen_at)
    } else {
        format!("stale heartbeat ({})", record.last_seen_at)
    }
}

fn doctor_receiver_config_complete(snapshot: &app::LiveSnapshot) -> bool {
    snapshot.webhook.callback_url.is_some()
        && snapshot.webhook.verification_token_configured
        && !snapshot
            .auth_status
            .missing_fields
            .contains(&"client_secret")
}

fn doctor_runtime_mode(snapshot: &app::LiveSnapshot) -> &'static str {
    let receiver = snapshot
        .webhook
        .runtime_heartbeats
        .iter()
        .find(|record| record.component == "webhook.receiver")
        .is_some_and(|record| doctor_heartbeat_active(snapshot, record));
    let watcher = snapshot
        .webhook
        .runtime_heartbeats
        .iter()
        .find(|record| record.component == "sync.watch")
        .is_some_and(|record| doctor_heartbeat_active(snapshot, record));

    match (receiver, watcher) {
        (true, true) => "full hybrid",
        (true, false) => "receiver only",
        _ => "scheduler only",
    }
}

fn doctor_heartbeat_status(snapshot: &app::LiveSnapshot, component: &str) -> String {
    snapshot
        .webhook
        .runtime_heartbeats
        .iter()
        .find(|record| record.component == component)
        .map_or_else(
            || "missing".to_owned(),
            |record| {
                let health = if doctor_heartbeat_active(snapshot, record) {
                    "healthy"
                } else if record.mode == "stopped" {
                    "stopped"
                } else {
                    "stale"
                };
                format!(
                    "{} | mode={} | last_seen={}",
                    health, record.mode, record.last_seen_at
                )
            },
        )
}

fn doctor_heartbeat_active(
    snapshot: &app::LiveSnapshot,
    record: &crate::store::webhook_store::RuntimeHeartbeatRecord,
) -> bool {
    record.mode != "stopped" && doctor_heartbeat_healthy(snapshot, &record.last_seen_at)
}

fn doctor_heartbeat_healthy(snapshot: &app::LiveSnapshot, last_seen_at: &str) -> bool {
    let Some(last_seen_at) = doctor_parse_timestamp(last_seen_at) else {
        return false;
    };
    let now = doctor_parse_timestamp(&snapshot.captured_at).unwrap_or_else(OffsetDateTime::now_utc);
    let age = (now - last_seen_at).whole_seconds().max(0);
    let max_age = i64::try_from(snapshot.webhook.heartbeat_secs)
        .unwrap_or_default()
        .saturating_mul(3);
    age <= max_age
}

fn doctor_remote_subscription_healthy(
    snapshot: &app::LiveSnapshot,
    record: &crate::store::webhook_store::RemoteWebhookSubscriptionRecord,
) -> bool {
    if record.drift_status != "matched" {
        return false;
    }
    let Some(expiration_time) = doctor_parse_timestamp(&record.expiration_time) else {
        return false;
    };
    let now = doctor_parse_timestamp(&snapshot.captured_at).unwrap_or_else(OffsetDateTime::now_utc);
    expiration_time > now
}

fn doctor_remote_subscription_renewal_due(
    snapshot: &app::LiveSnapshot,
    expiration_time: &str,
) -> bool {
    let Some(expiration_time) = doctor_parse_timestamp(expiration_time) else {
        return true;
    };
    let now = doctor_parse_timestamp(&snapshot.captured_at).unwrap_or_else(OffsetDateTime::now_utc);
    let renewal_lead =
        Duration::seconds(i64::try_from(snapshot.webhook.renewal_lead_secs).unwrap_or_default());
    expiration_time <= now + renewal_lead
}

fn doctor_parse_timestamp(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339).ok()
}

fn run_demo(config: &Config) -> Result<Option<String>> {
    let args = TuiArgs { demo: true };
    run_tui(config, &args)
}

fn run_tui(config: &Config, args: &TuiArgs) -> Result<Option<String>> {
    let interactive = interactive_terminal_available();
    let mut app = if args.demo {
        build_demo_state(config)
    } else {
        let store = Store::open(config)?;
        let auth_status = oura::auth::inspect_auth(config, &store)?;
        if interactive {
            build_live_state(config, &store, &auth_status)?
        } else {
            build_read_only_live_state(config, &store, &auth_status)?
        }
    };

    if interactive {
        if args.demo {
            info!("running demo TUI");
        } else {
            info!("running live TUI");
        }
        tui::run(config, &mut app)?;
        Ok(None)
    } else {
        if args.demo {
            warn!("demo TUI ran without a tty; rendering a deterministic snapshot instead");
        } else {
            warn!("tui ran without a tty; rendering a live snapshot instead");
        }
        let (width, height) = DEFAULT_NON_INTERACTIVE_VIEWPORT.named_dimensions();
        tui::render_snapshot(&app, width, height).map(Some)
    }
}

async fn run_ui_snapshot(config: &Config, args: UiSnapshotArgs) -> Result<Option<String>> {
    let screens = snapshot_screens(&args);
    let sizes = snapshot_sizes(&args);
    let render_source =
        build_snapshot_render_source(config, args.demo, args.fixture_dir.clone()).await?;
    let scenario_list = render_source.scenarios();
    let color_modes = snapshot_color_modes(&args);
    let requests = ui::snapshot::build_requests(
        &screens,
        &sizes,
        if scenario_list.is_empty() {
            None
        } else {
            Some(scenario_list.as_slice())
        },
    );
    let artifact_paths =
        ui::snapshot::write_snapshots(&args.out_dir, &requests, &color_modes, |request| {
            render_source.app_for_request(request)
        })?;

    let artifacts = artifact_paths
        .iter()
        .map(|path| format!("  - {}", path.display()))
        .collect::<Vec<_>>()
        .join("\n");
    let scenarios = if scenario_list.is_empty() {
        "single source".to_owned()
    } else {
        scenario_list
            .iter()
            .map(|scenario| scenario.label())
            .collect::<Vec<_>>()
            .join(", ")
    };

    Ok(Some(format!(
        "\
ringmaster ui snapshot

source: {}
scenarios: {}
screens: {}
sizes: {}
ansi_sidecars: {}
out_dir: {}
artifacts:
{}
",
        render_source.label(),
        scenarios,
        screens
            .iter()
            .map(|screen| screen.title().to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join(", "),
        sizes
            .iter()
            .map(|size| size.label().to_owned())
            .collect::<Vec<_>>()
            .join(", "),
        if color_modes.is_empty() {
            "disabled".to_owned()
        } else {
            color_modes
                .iter()
                .map(|mode| mode.label().to_owned())
                .collect::<Vec<_>>()
                .join(", ")
        },
        args.out_dir.display(),
        artifacts
    )))
}

fn snapshot_screens(args: &UiSnapshotArgs) -> Vec<app::Screen> {
    if args.screen.is_empty() {
        app::Screen::ALL.to_vec()
    } else {
        args.screen
            .iter()
            .map(|screen| match screen {
                SnapshotScreenArg::Dashboard => app::Screen::Dashboard,
                SnapshotScreenArg::Timeline => app::Screen::Timeline,
                SnapshotScreenArg::Trends => app::Screen::Trends,
                SnapshotScreenArg::Explain => app::Screen::Explain,
                SnapshotScreenArg::Patterns => app::Screen::Patterns,
                SnapshotScreenArg::Review => app::Screen::Review,
                SnapshotScreenArg::Ai => app::Screen::Ai,
                SnapshotScreenArg::Status => app::Screen::Ops,
            })
            .collect()
    }
}

fn snapshot_sizes(args: &UiSnapshotArgs) -> Vec<ui::snapshot::SnapshotSize> {
    if args.size.is_empty() {
        ui::snapshot::SnapshotSize::ALL.to_vec()
    } else {
        args.size
            .iter()
            .map(|size| match size {
                SnapshotSizeArg::Compact => ui::snapshot::SnapshotSize::Compact,
                SnapshotSizeArg::Medium => ui::snapshot::SnapshotSize::Medium,
                SnapshotSizeArg::Wide => ui::snapshot::SnapshotSize::Wide,
            })
            .collect()
    }
}

fn snapshot_color_modes(args: &UiSnapshotArgs) -> Vec<ui::snapshot::SnapshotColorMode> {
    if !args.ansi_sidecar {
        return Vec::new();
    }

    if args.color_mode.is_empty() {
        return vec![
            ui::snapshot::SnapshotColorMode::Current,
            ui::snapshot::SnapshotColorMode::Mono,
        ];
    }

    let mut modes = Vec::new();
    for mode in args.color_mode.iter().map(|mode| match mode {
        SnapshotColorModeArg::Current => ui::snapshot::SnapshotColorMode::Current,
        SnapshotColorModeArg::Truecolor => ui::snapshot::SnapshotColorMode::TrueColor,
        SnapshotColorModeArg::Ansi256 => ui::snapshot::SnapshotColorMode::Ansi256,
        SnapshotColorModeArg::Ansi16 => ui::snapshot::SnapshotColorMode::Ansi16,
        SnapshotColorModeArg::Mono => ui::snapshot::SnapshotColorMode::Mono,
    }) {
        if !modes.contains(&mode) {
            modes.push(mode);
        }
    }
    modes
}

#[derive(Debug, Clone)]
struct SnapshotScenarioState {
    scenario: ui::snapshot::SnapshotScenario,
    app: app::AppState,
}

#[derive(Debug, Clone)]
enum SnapshotRenderSource {
    Single {
        label: String,
        app: Box<app::AppState>,
    },
    ScenarioMatrix {
        label: String,
        states: Vec<SnapshotScenarioState>,
    },
}

impl SnapshotRenderSource {
    fn label(&self) -> &str {
        match self {
            Self::Single { label, .. } | Self::ScenarioMatrix { label, .. } => label,
        }
    }

    fn scenarios(&self) -> Vec<ui::snapshot::SnapshotScenario> {
        match self {
            Self::Single { .. } => Vec::new(),
            Self::ScenarioMatrix { states, .. } => {
                states.iter().map(|state| state.scenario).collect()
            }
        }
    }

    fn app_for_request(&self, request: ui::snapshot::SnapshotRequest) -> Result<app::AppState> {
        match self {
            Self::Single { app, .. } => Ok((**app).clone()),
            Self::ScenarioMatrix { states, .. } => states
                .iter()
                .find(|state| Some(state.scenario) == request.scenario)
                .map(|state| state.app.clone())
                .ok_or_else(|| {
                    RingmasterError::Ui(format!(
                        "snapshot scenario {:?} was not prepared",
                        request.scenario
                    ))
                }),
        }
    }
}

async fn build_snapshot_render_source(
    config: &Config,
    demo: bool,
    fixture_dir: Option<PathBuf>,
) -> Result<SnapshotRenderSource> {
    if let Some(fixture_dir) = fixture_dir {
        if ui::snapshot::is_scenario_fixture_root(&fixture_dir) {
            let states = build_scenario_fixture_snapshot_states(config, &fixture_dir).await?;
            return Ok(SnapshotRenderSource::ScenarioMatrix {
                label: format!("scenario fixture root {}", fixture_dir.display()),
                states,
            });
        }

        let app = build_fixture_snapshot_app(
            config,
            fixture_dir.clone(),
            "Fixture-backed snapshot is reading from a seeded local store.",
        )
        .await?;
        return Ok(SnapshotRenderSource::Single {
            label: format!("fixture {}", fixture_dir.display()),
            app: Box::new(app),
        });
    }

    if demo {
        return Ok(SnapshotRenderSource::Single {
            label: "demo".to_owned(),
            app: Box::new(build_demo_state(config)),
        });
    }

    let store = Store::open(config)?;
    let auth_status = oura::auth::inspect_auth(config, &store)?;
    Ok(SnapshotRenderSource::Single {
        label: "live store".to_owned(),
        app: Box::new(build_read_only_live_state(config, &store, &auth_status)?),
    })
}

async fn build_fixture_snapshot_app(
    config: &Config,
    fixture_dir: PathBuf,
    status_line: &str,
) -> Result<app::AppState> {
    let snapshot = load_fixture_snapshot(config, fixture_dir).await?;
    Ok(app::build_state_from_snapshot(
        app::RunMode::Live,
        status_line,
        snapshot,
    ))
}

async fn load_fixture_snapshot(config: &Config, fixture_dir: PathBuf) -> Result<app::LiveSnapshot> {
    let temp_root = TempRootGuard::new("ui-snapshot");
    let mut temp_config = config.clone();
    temp_config.paths = AppPaths::from_roots(
        config.paths.home_dir.clone(),
        temp_root.path().join("config"),
        temp_root.path().join("state"),
        temp_root.path().join("cache"),
    )?;

    let store = Store::open(&temp_config)?;
    oura::sync::sync_once(
        &temp_config,
        &store,
        oura::sync::SyncOptions {
            dry_run: false,
            fixture_dir: Some(fixture_dir.clone()),
            families: SyncFamily::ALL.to_vec(),
            trigger_source: Some("periodic_reconcile".to_owned()),
            trigger_detail: Some("ui snapshot fixture seed".to_owned()),
        },
    )
    .await?;
    derive::rebuild_store(&store)?;
    let auth_status = oura::auth::inspect_auth(&temp_config, &store)?;
    let mut snapshot = load_live_snapshot(&temp_config, &store, &auth_status)?;
    apply_fixture_snapshot_overlay(&mut snapshot, &fixture_dir);
    Ok(snapshot)
}

async fn build_scenario_fixture_snapshot_states(
    config: &Config,
    fixture_root: &std::path::Path,
) -> Result<Vec<SnapshotScenarioState>> {
    let mut states = Vec::new();
    let mut snapshot_cache: HashMap<PathBuf, app::LiveSnapshot> = HashMap::new();
    for scenario in ui::snapshot::SnapshotScenario::ALL {
        let fixture_dir = scenario_fixture_seed_dir(fixture_root, scenario);
        let mut snapshot = if let Some(snapshot) = snapshot_cache.get(&fixture_dir) {
            snapshot.clone()
        } else {
            let snapshot = load_fixture_snapshot(config, fixture_dir.clone()).await?;
            snapshot_cache.insert(fixture_dir, snapshot.clone());
            snapshot
        };
        apply_scenario_fixture_snapshot_overlay(&mut snapshot, fixture_root, scenario);
        states.push(SnapshotScenarioState {
            scenario,
            app: app::build_state_from_snapshot(
                app::RunMode::Live,
                format!(
                    "Scenario fixture `{}` is driving this deterministic snapshot.",
                    scenario.label()
                ),
                snapshot,
            ),
        });
    }
    Ok(states)
}

#[cfg(test)]
pub(crate) async fn build_scenario_fixture_snapshot_apps_for_tests(
    config: &Config,
    fixture_root: &std::path::Path,
) -> Result<Vec<(ui::snapshot::SnapshotScenario, app::AppState)>> {
    build_scenario_fixture_snapshot_states(config, fixture_root)
        .await
        .map(|states| {
            states
                .into_iter()
                .map(|state| (state.scenario, state.app))
                .collect()
        })
}

fn scenario_fixture_seed_dir(
    fixture_root: &std::path::Path,
    scenario: ui::snapshot::SnapshotScenario,
) -> PathBuf {
    let fixture_label = match scenario {
        ui::snapshot::SnapshotScenario::Strong
        | ui::snapshot::SnapshotScenario::Weak
        | ui::snapshot::SnapshotScenario::Empty
        | ui::snapshot::SnapshotScenario::DenseHistory => scenario.label(),
        ui::snapshot::SnapshotScenario::Stale
        | ui::snapshot::SnapshotScenario::Error
        | ui::snapshot::SnapshotScenario::MissingScope
        | ui::snapshot::SnapshotScenario::RateLimited => "strong",
    };
    fixture_root.join(fixture_label)
}

fn apply_fixture_snapshot_overlay(snapshot: &mut app::LiveSnapshot, fixture_dir: &std::path::Path) {
    let fixture_display_path = fixture_snapshot_display_path(fixture_dir);

    "2026-04-09T12:00:00Z".clone_into(&mut snapshot.captured_at);
    snapshot.config_path = format!("{fixture_display_path}/config.toml");
    snapshot.database_path = format!("{fixture_display_path}/ringmaster.db");
    normalize_fixture_snapshot_refresh_policy(&mut snapshot.refresh_policy);
    normalize_fixture_snapshot_auth_status(&mut snapshot.auth_status, fixture_dir);
    FIXTURE_SNAPSHOT_WEBHOOK_BIND_ADDRESS.clone_into(&mut snapshot.webhook.bind_address);
    FIXTURE_SNAPSHOT_WEBHOOK_PATH.clone_into(&mut snapshot.webhook.path);
    snapshot.webhook.callback_url = Some(FIXTURE_SNAPSHOT_WEBHOOK_CALLBACK_URL.to_owned());
    snapshot.webhook.verification_token_configured = true;
    snapshot.webhook.signature_tolerance_secs = 300;
    snapshot.webhook.heartbeat_secs = 15;
    snapshot.webhook.renewal_lead_secs = 7 * 24 * 60 * 60;
    snapshot.webhook.desired_subscriptions = fixture_snapshot_desired_subscription_records();
    snapshot.webhook.remote_subscriptions = fixture_snapshot_remote_subscription_records(
        &snapshot.webhook.desired_subscriptions,
        "matched",
        "2026-04-15T12:00:00Z",
        "2026-04-09T11:55:00Z",
    );
    normalize_fixture_snapshot_sync_state_timestamps(
        &mut snapshot.sync_states,
        FIXTURE_SNAPSHOT_BASE_SYNC_ATTEMPTED_AT,
        FIXTURE_SNAPSHOT_BASE_SYNC_COMPLETED_AT,
    );
}

const fn normalize_fixture_snapshot_refresh_policy(
    refresh_policy: &mut crate::app::RefreshPolicySnapshot,
) {
    *refresh_policy = crate::app::RefreshPolicySnapshot {
        personal_interval_secs: FIXTURE_SNAPSHOT_PERSONAL_INTERVAL_SECS,
        daily_interval_secs: FIXTURE_SNAPSHOT_DAILY_INTERVAL_SECS,
        heartrate_interval_secs: FIXTURE_SNAPSHOT_HEARTRATE_INTERVAL_SECS,
        workout_interval_secs: FIXTURE_SNAPSHOT_WORKOUT_INTERVAL_SECS,
        enhanced_tag_interval_secs: FIXTURE_SNAPSHOT_ENHANCED_TAG_INTERVAL_SECS,
        session_interval_secs: FIXTURE_SNAPSHOT_SESSION_INTERVAL_SECS,
        personal_stale_after_secs: FIXTURE_SNAPSHOT_PERSONAL_STALE_AFTER_SECS,
        daily_stale_after_secs: FIXTURE_SNAPSHOT_DAILY_STALE_AFTER_SECS,
        heartrate_stale_after_secs: FIXTURE_SNAPSHOT_HEARTRATE_STALE_AFTER_SECS,
        workout_stale_after_secs: FIXTURE_SNAPSHOT_WORKOUT_STALE_AFTER_SECS,
        enhanced_tag_stale_after_secs: FIXTURE_SNAPSHOT_ENHANCED_TAG_STALE_AFTER_SECS,
        session_stale_after_secs: FIXTURE_SNAPSHOT_SESSION_STALE_AFTER_SECS,
    };
}

fn normalize_fixture_snapshot_auth_status(
    auth_status: &mut crate::oura::models::AuthStatus,
    fixture_dir: &std::path::Path,
) {
    let granted_scopes = fixture_snapshot_granted_scopes(fixture_dir);
    auth_status.configured = true;
    FIXTURE_SNAPSHOT_AUTH_CALLBACK_URL.clone_into(&mut auth_status.callback_url);
    auth_status.requested_scopes.clone_from(&granted_scopes);
    auth_status.granted_scopes.clone_from(&granted_scopes);
    auth_status.missing_fields.clear();
    auth_status.capability_report =
        crate::oura::models::CapabilityReport::from_scopes(&granted_scopes, &granted_scopes);
    auth_status.auth_timeout_secs = FIXTURE_SNAPSHOT_AUTH_TIMEOUT_SECS;
    FIXTURE_SNAPSHOT_SECRET_BACKEND.clone_into(&mut auth_status.secret_backend);
    auth_status.access_token_stored = true;
    auth_status.refresh_token_stored = true;
    auth_status.access_token_expires_at = Some(FIXTURE_SNAPSHOT_ACCESS_TOKEN_EXPIRES_AT.to_owned());
    auth_status.last_authenticated_at = Some(FIXTURE_SNAPSHOT_LAST_AUTHENTICATED_AT.to_owned());
    auth_status.last_refresh_at = Some(FIXTURE_SNAPSHOT_LAST_REFRESH_AT.to_owned());
    auth_status.account_id = Some(FIXTURE_SNAPSHOT_ACCOUNT_ID.to_owned());
    auth_status.account_email = Some(FIXTURE_SNAPSHOT_ACCOUNT_EMAIL.to_owned());
    auth_status.last_error = None;
}

fn fixture_snapshot_granted_scopes(fixture_dir: &std::path::Path) -> Vec<String> {
    let mut scopes = Vec::new();
    if fixture_dir.join("personal_info.json").is_file() {
        scopes.push("personal".to_owned());
    }
    let daily_files = [
        fixture_dir.join("daily_sleep.json"),
        fixture_dir.join("sleep.json"),
        fixture_dir.join("daily_readiness.json"),
        fixture_dir.join("daily_activity.json"),
        fixture_dir.join("sleep_time.json"),
        fixture_dir.join("rest_mode_periods.json"),
        fixture_dir.join("daily_stress.json"),
        fixture_dir.join("daily_resilience.json"),
        fixture_dir.join("daily_cardiovascular_age.json"),
        fixture_dir.join("vo2_max.json"),
    ];
    if daily_files.iter().any(|path| path.is_file()) {
        scopes.push("daily".to_owned());
    }
    let stress_files = [
        fixture_dir.join("sleep_time.json"),
        fixture_dir.join("rest_mode_periods.json"),
        fixture_dir.join("daily_stress.json"),
    ];
    if stress_files.iter().any(|path| path.is_file()) {
        scopes.push("stress".to_owned());
    }
    let heart_health_files = [
        fixture_dir.join("daily_resilience.json"),
        fixture_dir.join("daily_cardiovascular_age.json"),
        fixture_dir.join("vo2_max.json"),
    ];
    if heart_health_files.iter().any(|path| path.is_file()) {
        scopes.push("heart_health".to_owned());
    }
    if fixture_dir.join("daily_spo2.json").is_file() {
        scopes.push("spo2".to_owned());
    }
    if fixture_dir.join("heartrate.json").is_file() {
        scopes.push("heartrate".to_owned());
    }
    if fixture_dir.join("workouts.json").is_file() {
        scopes.push("workout".to_owned());
    }
    if fixture_dir.join("enhanced_tags.json").is_file() {
        scopes.push("tag".to_owned());
    }
    if fixture_dir.join("sessions.json").is_file() {
        scopes.push("session".to_owned());
    }

    scopes
}

fn apply_scenario_fixture_snapshot_overlay(
    snapshot: &mut app::LiveSnapshot,
    fixture_root: &std::path::Path,
    scenario: ui::snapshot::SnapshotScenario,
) {
    apply_fixture_snapshot_baseline(snapshot, fixture_root, scenario);

    match scenario {
        ui::snapshot::SnapshotScenario::Strong => apply_strong_scenario_overlay(snapshot),
        ui::snapshot::SnapshotScenario::Weak => apply_weak_scenario_overlay(snapshot),
        ui::snapshot::SnapshotScenario::Empty => apply_empty_scenario_overlay(snapshot),
        ui::snapshot::SnapshotScenario::DenseHistory => {
            apply_strong_scenario_overlay(snapshot);
        }
        ui::snapshot::SnapshotScenario::Stale => apply_stale_scenario_overlay(snapshot),
        ui::snapshot::SnapshotScenario::Error => apply_error_scenario_overlay(snapshot),
        ui::snapshot::SnapshotScenario::MissingScope => {
            apply_missing_scope_scenario_overlay(snapshot);
        }
        ui::snapshot::SnapshotScenario::RateLimited => {
            apply_rate_limited_scenario_overlay(snapshot);
        }
    }
}

fn apply_fixture_snapshot_baseline(
    snapshot: &mut app::LiveSnapshot,
    fixture_root: &std::path::Path,
    scenario: ui::snapshot::SnapshotScenario,
) {
    snapshot.config_path = scenario_fixture_config_path(fixture_root, scenario);
    snapshot.database_path = scenario_fixture_database_path(fixture_root, scenario);
    FIXTURE_SNAPSHOT_WEBHOOK_BIND_ADDRESS.clone_into(&mut snapshot.webhook.bind_address);
    FIXTURE_SNAPSHOT_WEBHOOK_PATH.clone_into(&mut snapshot.webhook.path);
    snapshot.webhook.callback_url = Some(FIXTURE_SNAPSHOT_WEBHOOK_CALLBACK_URL.to_owned());
    snapshot.webhook.verification_token_configured = true;
    snapshot.webhook.signature_tolerance_secs = 300;
    snapshot.webhook.heartbeat_secs = 15;
    snapshot.webhook.renewal_lead_secs = 7 * 24 * 60 * 60;
    snapshot.webhook.desired_subscriptions = fixture_snapshot_desired_subscription_records();
    snapshot.webhook.remote_subscriptions = fixture_snapshot_remote_subscription_records(
        &snapshot.webhook.desired_subscriptions,
        "matched",
        "2026-04-15T12:00:00Z",
        "2026-04-09T11:55:00Z",
    );
    snapshot.webhook.recent_deliveries =
        vec![crate::store::webhook_store::AcceptedWebhookDeliveryRecord {
            delivery_id: 101,
            delivery_fingerprint: format!("scenario-{}", scenario.label()),
            received_at: "2026-04-09T11:54:00Z".to_owned(),
            signature_timestamp: Some("2026-04-09T11:54:00Z".to_owned()),
            data_type: Some("daily_sleep".to_owned()),
            event_type: Some(crate::webhook::WebhookEventType::Update),
            object_id: Some("daily_sleep_2026-04-08".to_owned()),
            payload_json: "{}".to_owned(),
            headers_json: "{}".to_owned(),
            query_json: "{}".to_owned(),
        }];
    snapshot.webhook.latest_rejected_delivery = None;
    snapshot.webhook.pending_invalidations.clear();
    snapshot.webhook.recent_processing_attempts.clear();
    snapshot.webhook.runtime_heartbeats = vec![
        crate::store::webhook_store::RuntimeHeartbeatRecord {
            component: "webhook.receiver".to_owned(),
            mode: "running".to_owned(),
            bind_address: Some(snapshot.webhook.bind_address.clone()),
            public_base_url: snapshot.webhook.callback_url.clone(),
            detail: Some("scenario fixture".to_owned()),
            last_seen_at: "2026-04-09T11:59:30Z".to_owned(),
        },
        crate::store::webhook_store::RuntimeHeartbeatRecord {
            component: "sync.watch".to_owned(),
            mode: "running".to_owned(),
            bind_address: None,
            public_base_url: None,
            detail: Some("scenario fixture".to_owned()),
            last_seen_at: "2026-04-09T11:59:30Z".to_owned(),
        },
    ];
}

fn apply_strong_scenario_overlay(snapshot: &mut app::LiveSnapshot) {
    "2026-04-09T12:00:00Z".clone_into(&mut snapshot.captured_at);
    normalize_fixture_snapshot_sync_state_timestamps(
        &mut snapshot.sync_states,
        FIXTURE_SNAPSHOT_BASE_SYNC_ATTEMPTED_AT,
        FIXTURE_SNAPSHOT_BASE_SYNC_COMPLETED_AT,
    );
}

fn apply_weak_scenario_overlay(snapshot: &mut app::LiveSnapshot) {
    apply_strong_scenario_overlay(snapshot);
    for state in &mut snapshot.sync_states {
        if state.message.is_none() {
            state.message = Some("Thin local history keeps comparisons tentative.".to_owned());
        }
    }
}

fn apply_empty_scenario_overlay(snapshot: &mut app::LiveSnapshot) {
    "2026-04-09T12:00:00Z".clone_into(&mut snapshot.captured_at);
    snapshot.personal_info = None;
    snapshot.daily_history.clear();
    snapshot.heartrate_days.clear();
    snapshot.heartrate_daily_averages.clear();
    snapshot.context_events.clear();
    snapshot.pattern_summaries.clear();
    snapshot.review_signal_days.clear();
    snapshot.sleep_time.clear();
    snapshot.rest_mode_periods.clear();
    snapshot.record_counts = crate::store::queries::RecordCounts::default();
    for state in &mut snapshot.sync_states {
        state.status = crate::store::queries::SyncRunStatus::Success;
        state.message = Some("Scope granted, but the local cache is still empty.".to_owned());
        state.last_error = None;
        state.failure_count = 0;
        state.next_attempt_after = None;
    }
    normalize_fixture_snapshot_sync_state_timestamps(
        &mut snapshot.sync_states,
        FIXTURE_SNAPSHOT_BASE_SYNC_ATTEMPTED_AT,
        FIXTURE_SNAPSHOT_BASE_SYNC_COMPLETED_AT,
    );
}

fn apply_stale_scenario_overlay(snapshot: &mut app::LiveSnapshot) {
    "2026-04-11T12:00:00Z".clone_into(&mut snapshot.captured_at);
    snapshot.webhook.remote_subscriptions = fixture_snapshot_remote_subscription_records(
        &snapshot.webhook.desired_subscriptions,
        "drifted",
        "2026-04-10T08:00:00Z",
        "2026-04-09T08:00:00Z",
    );
    snapshot.webhook.runtime_heartbeats = vec![
        crate::store::webhook_store::RuntimeHeartbeatRecord {
            component: "webhook.receiver".to_owned(),
            mode: "running".to_owned(),
            bind_address: Some(snapshot.webhook.bind_address.clone()),
            public_base_url: snapshot.webhook.callback_url.clone(),
            detail: Some("stale heartbeat".to_owned()),
            last_seen_at: "2026-04-09T07:30:00Z".to_owned(),
        },
        crate::store::webhook_store::RuntimeHeartbeatRecord {
            component: "sync.watch".to_owned(),
            mode: "running".to_owned(),
            bind_address: None,
            public_base_url: None,
            detail: Some("lagging".to_owned()),
            last_seen_at: "2026-04-09T07:35:00Z".to_owned(),
        },
    ];
    snapshot.webhook.latest_rejected_delivery =
        Some(crate::store::webhook_store::RejectedWebhookDeliveryRecord {
            rejection_id: 7,
            received_at: "2026-04-10T05:00:00Z".to_owned(),
            reason_code: "signature_stale".to_owned(),
            detail: "delivery arrived outside the tolerance window".to_owned(),
            signature_timestamp: Some("2026-04-10T04:40:00Z".to_owned()),
            payload_json: "{}".to_owned(),
            headers_json: "{}".to_owned(),
            query_json: "{}".to_owned(),
        });
    snapshot.webhook.pending_invalidations =
        vec![crate::store::webhook_store::InvalidationRecord {
            invalidation_id: 14,
            queue_key: "daily_sleep:update:2026-04-08".to_owned(),
            data_type: "daily_sleep".to_owned(),
            event_type: crate::webhook::WebhookEventType::Update,
            object_id: Some("daily_sleep_2026-04-08".to_owned()),
            delivery_id: 101,
            first_queued_at: "2026-04-10T05:10:00Z".to_owned(),
            last_queued_at: "2026-04-10T05:10:00Z".to_owned(),
            available_at: "2026-04-10T05:10:00Z".to_owned(),
            leased_at: None,
            lease_owner: None,
            attempt_count: 2,
            last_error: Some("receiver heartbeat went stale before processing".to_owned()),
            completed_at: None,
        }];
    snapshot.webhook.recent_processing_attempts =
        vec![crate::store::webhook_store::ProcessingAttemptRecord {
            attempt_id: 3,
            invalidation_id: 14,
            started_at: "2026-04-10T05:11:00Z".to_owned(),
            finished_at: Some("2026-04-10T05:12:00Z".to_owned()),
            outcome: "failed".to_owned(),
            detail: Some("watch loop was offline".to_owned()),
        }];
    for state in &mut snapshot.sync_states {
        state.failure_count = 2;
        state.next_attempt_after = Some("2026-04-11T12:05:00Z".to_owned());
        state.message = Some(
            "Persisted data is present, but freshness is now outside the expected window."
                .to_owned(),
        );
    }
    normalize_fixture_snapshot_sync_state_timestamps(
        &mut snapshot.sync_states,
        FIXTURE_SNAPSHOT_STALE_SYNC_ATTEMPTED_AT,
        FIXTURE_SNAPSHOT_STALE_SYNC_COMPLETED_AT,
    );
}

fn apply_error_scenario_overlay(snapshot: &mut app::LiveSnapshot) {
    "2026-04-09T12:00:00Z".clone_into(&mut snapshot.captured_at);
    let granted_scopes = vec!["personal".to_owned(), "daily".to_owned()];
    snapshot
        .auth_status
        .granted_scopes
        .clone_from(&granted_scopes);
    snapshot.auth_status.capability_report = crate::oura::models::CapabilityReport::from_scopes(
        &snapshot.auth_status.requested_scopes,
        &granted_scopes,
    );
    snapshot.auth_status.access_token_stored = false;
    snapshot.auth_status.refresh_token_stored = false;
    snapshot.auth_status.last_error = Some(crate::error::OuraProblem::new(
        Some(401),
        "session missing",
        Some("run `ringmaster auth login` to restore the local session".to_owned()),
    ));
    snapshot.heartrate_days.clear();
    snapshot.heartrate_daily_averages.clear();
    snapshot.context_events.clear();
    snapshot.pattern_summaries.clear();
    snapshot.review_signal_days.clear();
    snapshot.sleep_time.clear();
    snapshot.rest_mode_periods.clear();
    snapshot.record_counts.heartrate_samples = 0;
    snapshot.record_counts.workouts = 0;
    snapshot.record_counts.enhanced_tags = 0;
    snapshot.record_counts.sessions = 0;
    snapshot.record_counts.derived_context_events = 0;
    snapshot.record_counts.derived_pattern_summaries = 0;
    snapshot.record_counts.derived_review_signal_days = 0;
    snapshot.webhook.callback_url = None;
    snapshot.webhook.verification_token_configured = false;
    snapshot.webhook.remote_subscriptions.clear();
    snapshot.webhook.runtime_heartbeats.clear();
    snapshot.webhook.latest_rejected_delivery =
        Some(crate::store::webhook_store::RejectedWebhookDeliveryRecord {
            rejection_id: 9,
            received_at: "2026-04-09T11:40:00Z".to_owned(),
            reason_code: "verification_failed".to_owned(),
            detail: "receiver is missing the verification token".to_owned(),
            signature_timestamp: Some("2026-04-09T11:39:00Z".to_owned()),
            payload_json: "{}".to_owned(),
            headers_json: "{}".to_owned(),
            query_json: "{}".to_owned(),
        });
    for state in &mut snapshot.sync_states {
        state.failure_count = 3;
        state.next_attempt_after = Some("2026-04-09T12:05:00Z".to_owned());
        match state.sync_key.as_str() {
            "oura.daily" => {
                state.status = crate::store::queries::SyncRunStatus::Failed;
                state.message = Some(
                    "The last daily sync failed because the local auth session is missing."
                        .to_owned(),
                );
                state.last_error = Some(crate::error::OuraProblem::new(
                    Some(401),
                    "auth required",
                    Some("run `ringmaster auth login` before trying another sync".to_owned()),
                ));
            }
            "oura.heartrate" => {
                state.status = crate::store::queries::SyncRunStatus::Blocked;
                state.message = Some(
                    "Missing `heartrate` scope; timeline and trends cannot refresh.".to_owned(),
                );
                state.last_error = None;
            }
            "oura.workouts" => {
                state.status = crate::store::queries::SyncRunStatus::Blocked;
                state.message =
                    Some("Missing `workout` scope; workout context stays unavailable.".to_owned());
                state.last_error = None;
            }
            "oura.enhanced_tags" => {
                state.status = crate::store::queries::SyncRunStatus::Blocked;
                state.message =
                    Some("Missing `tag` scope; tag context stays unavailable.".to_owned());
                state.last_error = None;
            }
            "oura.sessions" => {
                state.status = crate::store::queries::SyncRunStatus::Blocked;
                state.message =
                    Some("Missing `session` scope; session context stays unavailable.".to_owned());
                state.last_error = None;
            }
            _ => {}
        }
    }
    normalize_fixture_snapshot_sync_state_timestamps(
        &mut snapshot.sync_states,
        FIXTURE_SNAPSHOT_ERROR_SYNC_ATTEMPTED_AT,
        FIXTURE_SNAPSHOT_ERROR_SYNC_COMPLETED_AT,
    );
}

fn apply_missing_scope_scenario_overlay(snapshot: &mut app::LiveSnapshot) {
    apply_strong_scenario_overlay(snapshot);
    let granted_scopes = vec!["personal".to_owned(), "daily".to_owned()];
    snapshot
        .auth_status
        .granted_scopes
        .clone_from(&granted_scopes);
    snapshot.auth_status.capability_report = crate::oura::models::CapabilityReport::from_scopes(
        &snapshot.auth_status.requested_scopes,
        &granted_scopes,
    );
    snapshot.heartrate_days.clear();
    snapshot.heartrate_daily_averages.clear();
    snapshot.context_events.clear();
    snapshot.pattern_summaries.clear();
    snapshot.review_signal_days.clear();
    snapshot.record_counts.heartrate_samples = 0;
    snapshot.record_counts.workouts = 0;
    snapshot.record_counts.enhanced_tags = 0;
    snapshot.record_counts.sessions = 0;
    snapshot.record_counts.derived_context_events = 0;
    snapshot.record_counts.derived_pattern_summaries = 0;
    snapshot.record_counts.derived_review_signal_days = 0;
    for state in &mut snapshot.sync_states {
        match state.sync_key.as_str() {
            "oura.heartrate" => {
                state.status = crate::store::queries::SyncRunStatus::Blocked;
                state.message = Some(
                    "Missing `heartrate` scope; heart-rate panels stay unavailable.".to_owned(),
                );
                state.last_error = None;
                state.failure_count = 0;
                state.next_attempt_after = None;
            }
            "oura.workouts" => {
                state.status = crate::store::queries::SyncRunStatus::Blocked;
                state.message =
                    Some("Missing `workout` scope; activity context stays partial.".to_owned());
                state.last_error = None;
                state.failure_count = 0;
                state.next_attempt_after = None;
            }
            "oura.enhanced_tags" => {
                state.status = crate::store::queries::SyncRunStatus::Blocked;
                state.message =
                    Some("Missing `tag` scope; tagged context stays unavailable.".to_owned());
                state.last_error = None;
                state.failure_count = 0;
                state.next_attempt_after = None;
            }
            "oura.sessions" => {
                state.status = crate::store::queries::SyncRunStatus::Blocked;
                state.message = Some(
                    "Missing `session` scope; guided-session context stays unavailable.".to_owned(),
                );
                state.last_error = None;
                state.failure_count = 0;
                state.next_attempt_after = None;
            }
            _ => {}
        }
    }
}

fn apply_rate_limited_scenario_overlay(snapshot: &mut app::LiveSnapshot) {
    apply_strong_scenario_overlay(snapshot);
    for state in &mut snapshot.sync_states {
        if state.sync_key == "oura.heartrate" {
            state.status = crate::store::queries::SyncRunStatus::Failed;
            state.message = Some(
                "Heartrate backfill is rate limited; retry after the minute window resets."
                    .to_owned(),
            );
            state.last_error = Some(crate::error::OuraProblem::new(
                Some(429),
                "rate limit reached",
                Some("retry after the minute window resets".to_owned()),
            ));
            state.failure_count = 1;
            state.next_attempt_after = Some("2026-04-09T12:05:00Z".to_owned());
        }
    }
    normalize_fixture_snapshot_sync_state_timestamps(
        &mut snapshot.sync_states,
        FIXTURE_SNAPSHOT_ERROR_SYNC_ATTEMPTED_AT,
        FIXTURE_SNAPSHOT_BASE_SYNC_COMPLETED_AT,
    );
}

fn scenario_fixture_config_path(
    fixture_root: &std::path::Path,
    scenario: ui::snapshot::SnapshotScenario,
) -> String {
    format!(
        "{}/config.toml",
        fixture_snapshot_display_path(&fixture_root.join(scenario.label()))
    )
}

fn scenario_fixture_database_path(
    fixture_root: &std::path::Path,
    scenario: ui::snapshot::SnapshotScenario,
) -> String {
    format!(
        "{}/ringmaster.db",
        fixture_snapshot_display_path(&fixture_root.join(scenario.label()))
    )
}

fn fixture_snapshot_display_path(fixture_dir: &std::path::Path) -> String {
    let current_dir = std::env::current_dir().ok();
    current_dir
        .as_ref()
        .and_then(|cwd| fixture_dir.strip_prefix(cwd).ok())
        .unwrap_or(fixture_dir)
        .display()
        .to_string()
}

fn normalize_fixture_snapshot_sync_state_timestamps(
    sync_states: &mut [crate::store::queries::SyncStateRecord],
    attempted_at: &str,
    completed_at: &str,
) {
    for state in sync_states {
        attempted_at.clone_into(&mut state.last_attempted_at);
        state.last_completed_at = if matches!(
            state.status,
            crate::store::queries::SyncRunStatus::Failed
                | crate::store::queries::SyncRunStatus::Blocked
        ) {
            None
        } else {
            Some(completed_at.to_owned())
        };
    }
}

fn fixture_snapshot_desired_subscription_records()
-> Vec<crate::store::webhook_store::DesiredWebhookSubscriptionRecord> {
    let updated_at = "2026-04-09T11:50:00Z".to_owned();
    let callback_url = Some(FIXTURE_SNAPSHOT_WEBHOOK_CALLBACK_URL.to_owned());
    crate::webhook::default_desired_subscriptions()
        .into_iter()
        .filter(|subscription| subscription.enabled)
        .flat_map(|subscription| {
            let callback_url = callback_url.clone();
            let updated_at = updated_at.clone();
            subscription
                .normalized_event_types()
                .into_iter()
                .map(move |event_type| {
                    crate::store::webhook_store::DesiredWebhookSubscriptionRecord {
                        data_type: subscription.data_type.clone(),
                        event_type,
                        enabled: true,
                        callback_url: callback_url.clone(),
                        updated_at: updated_at.clone(),
                    }
                })
        })
        .collect()
}

fn fixture_snapshot_remote_subscription_records(
    desired: &[crate::store::webhook_store::DesiredWebhookSubscriptionRecord],
    drift_status: &str,
    expiration_time: &str,
    last_seen_at: &str,
) -> Vec<crate::store::webhook_store::RemoteWebhookSubscriptionRecord> {
    desired
        .iter()
        .enumerate()
        .map(
            |(index, desired)| crate::store::webhook_store::RemoteWebhookSubscriptionRecord {
                subscription_id: format!("sub-{:02}", index + 1),
                callback_url: desired
                    .callback_url
                    .clone()
                    .unwrap_or_else(|| "https://fixture.example.test/webhooks/oura".to_owned()),
                event_type: desired.event_type,
                data_type: desired.data_type.clone(),
                expiration_time: expiration_time.to_owned(),
                drift_status: drift_status.to_owned(),
                last_seen_at: last_seen_at.to_owned(),
                created_at: "2026-04-08T12:00:00Z".to_owned(),
                updated_at: last_seen_at.to_owned(),
            },
        )
        .collect()
}

async fn run_auth_login(config: &Config) -> Result<Option<String>> {
    let store = Store::open(config)?;
    let report = oura::auth::login(config, &store).await?;
    let authorization_url = report
        .authorization_url
        .unwrap_or_else(|| "unavailable until client credentials are configured".to_owned());
    let notes = report
        .notes
        .iter()
        .map(|note| format!("  - {note}"))
        .collect::<Vec<_>>()
        .join("\n");

    let output = format!(
        "\
ringmaster auth login

status: {:?}
callback_url: {}
listener_bind: {}
requested_scopes: {}
granted_scopes: {}
authorization_url: {}
notes:
{}
",
        report.status,
        report.auth_status.callback_url,
        report.listener_plan.bind_address,
        report.auth_status.requested_scopes.join(", "),
        if report.auth_status.granted_scopes.is_empty() {
            "none".to_owned()
        } else {
            report.auth_status.granted_scopes.join(", ")
        },
        authorization_url,
        notes,
    );

    Ok(Some(output))
}

async fn run_sync_once(config: &Config, args: SyncOnceArgs) -> Result<Option<String>> {
    let store = Store::open(config)?;
    let report = oura::sync::sync_once(
        config,
        &store,
        oura::sync::SyncOptions {
            dry_run: args.dry_run,
            fixture_dir: args.fixture_dir,
            families: SyncFamily::ALL.to_vec(),
            trigger_source: Some("manual_sync".to_owned()),
            trigger_detail: Some("ringmaster sync once".to_owned()),
        },
    )
    .await?;
    let notes = report
        .notes
        .iter()
        .map(|note| format!("  - {note}"))
        .collect::<Vec<_>>()
        .join("\n");
    let slices = report
        .slice_reports
        .iter()
        .map(|slice| {
            format!(
                "  - {}: {} rows={} watermark={} {}",
                slice.sync_key,
                slice.status,
                slice.imported_rows,
                slice.watermark.as_deref().unwrap_or("n/a"),
                slice.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let output = format!(
        "\
ringmaster sync once

status: {}
started_at: {}
finished_at: {}
database_path: {}
available_capabilities: {}
slice_reports:
{}
notes:
{}
",
        report.status,
        report.started_at,
        report.finished_at,
        report.database_path,
        report.capability_report.available_labels().join(", "),
        slices,
        notes,
    );

    Ok(Some(output))
}

async fn run_sync_watch(config: &Config, args: SyncWatchArgs) -> Result<Option<String>> {
    let report = refresh::run_watch(
        config,
        WatchOptions {
            dry_run: args.dry_run,
            demo: args.demo,
            fixture_dir: args.fixture_dir,
            max_iterations: args.max_iterations,
        },
    )
    .await?;
    let last_status = report.last_report.as_ref().map_or_else(
        || "not-run".to_owned(),
        |sync_report| sync_report.status.to_string(),
    );
    let notes = if report.notes.is_empty() {
        "  - none".to_owned()
    } else {
        report
            .notes
            .iter()
            .map(|note| format!("  - {note}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let output = format!(
        "\
ringmaster sync watch

iterations: {}
dry_run: {}
demo: {}
database_path: {}
last_status: {}
notes:
{}
",
        report.iterations, report.dry_run, report.demo, report.database_path, last_status, notes,
    );

    Ok(Some(output))
}

async fn run_webhook_serve(config: &Config, _args: WebhookServeArgs) -> Result<Option<String>> {
    let report = webhook::receiver::serve(config).await?;
    let output = format!(
        "\
ringmaster webhook serve

bind: {}
callback_url: {}
stopped_at: {}
",
        report.bind_address,
        report
            .callback_url
            .unwrap_or_else(|| "unconfigured".to_owned()),
        report.stopped_at,
    );

    Ok(Some(output))
}

async fn run_webhook_replay(config: &Config, args: WebhookReplayArgs) -> Result<Option<String>> {
    let replay_uses_fixture = args.fixture.is_some();
    let store = Store::open(config)?;
    let mut report = webhook::receiver::replay(
        config,
        &store,
        webhook::receiver::WebhookReplayOptions {
            fixture: args.fixture,
            delivery_id: args.delivery_id,
            recent: args.recent,
        },
    )?;
    let replay_processing_fixture_dir = replay_processing_fixture_dir(config);
    if report
        .entries
        .iter()
        .any(|entry| entry.invalidation_id.is_some())
    {
        if replay_uses_fixture {
            if let Some(fixture_dir) = replay_processing_fixture_dir {
                let processing_report = refresh::process_pending_invalidations_once(
                    config,
                    &store,
                    true,
                    Some(fixture_dir.clone()),
                )
                .await?;
                if let Some(sync_report) = processing_report.sync_report {
                    report.notes.push(format!(
                        "Previewed {} invalidation(s) via fixture-backed sync from {} without writing to the local store (status={}).",
                        processing_report.claimed_invalidations,
                        fixture_dir.display(),
                        sync_report.status
                    ));
                    report.notes.extend(processing_report.notes);
                }
            } else {
                report.notes.push(
                    "Skipped bounded invalidation processing because no local fixture directory was available."
                        .to_owned(),
                );
            }
        } else {
            report.notes.push(
                "Stored-delivery replay re-enqueued invalidations without auto-running a fixture-backed sync."
                    .to_owned(),
            );
        }
    }
    let entries = if report.entries.is_empty() {
        "  - none".to_owned()
    } else {
        report
            .entries
            .iter()
            .map(|entry| {
                format!(
                    "  - {}: {} delivery_id={} invalidation_id={} {}",
                    entry.source,
                    entry.status,
                    entry
                        .delivery_id
                        .map_or_else(|| "n/a".to_owned(), |value| value.to_string()),
                    entry
                        .invalidation_id
                        .map_or_else(|| "n/a".to_owned(), |value| value.to_string()),
                    entry.detail.as_deref().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let notes = if report.notes.is_empty() {
        "  - none".to_owned()
    } else {
        report
            .notes
            .iter()
            .map(|note| format!("  - {note}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let output = format!(
        "\
ringmaster webhook replay

entries:
{entries}
notes:
{notes}
"
    );

    Ok(Some(output))
}

fn replay_processing_fixture_dir(config: &Config) -> Option<PathBuf> {
    let candidate = config
        .refresh
        .demo_fixture_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("tests/fixtures/phase3"));
    candidate.exists().then_some(candidate)
}

async fn run_webhook_subscriptions_list(
    config: &Config,
    args: WebhookSubscriptionsListArgs,
) -> Result<Option<String>> {
    let store = Store::open(config)?;
    let report =
        webhook::subscriptions::list_subscriptions(config, &store, args.fixture_dir).await?;
    let desired = report
        .desired
        .iter()
        .map(|entry| {
            format!(
                "  - desired {}:{} -> {}",
                entry.data_type,
                entry.event_type.as_str(),
                entry.callback_url
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let remote = if report.remote.is_empty() {
        "  - none".to_owned()
    } else {
        report
            .remote
            .iter()
            .map(|entry| {
                format!(
                    "  - remote {}:{} id={} callback={} expires_at={}",
                    entry.data_type,
                    entry.event_type.as_str(),
                    entry.id,
                    entry.callback_url,
                    entry.expiration_time
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let notes = if report.notes.is_empty() {
        "  - none".to_owned()
    } else {
        report
            .notes
            .iter()
            .map(|note| format!("  - {note}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let output = format!(
        "\
ringmaster webhook subscriptions list

callback_url: {}
verification_token_configured: {}
fixture_dir: {}
desired:
{}
remote:
{}
notes:
{}
",
        report
            .callback_url
            .unwrap_or_else(|| "unconfigured".to_owned()),
        report.verification_token_configured,
        report
            .fixture_dir
            .as_ref()
            .map_or_else(|| "live".to_owned(), |path| path.display().to_string()),
        desired,
        remote,
        notes,
    );

    Ok(Some(output))
}

async fn run_webhook_subscriptions_sync(
    config: &Config,
    args: WebhookSubscriptionsSyncArgs,
) -> Result<Option<String>> {
    let store = Store::open(config)?;
    let report = webhook::subscriptions::sync_subscriptions(
        config,
        &store,
        webhook::subscriptions::SubscriptionSyncOptions {
            dry_run: args.dry_run,
            prune: args.prune,
            fixture_dir: args.fixture_dir,
        },
    )
    .await?;
    Ok(Some(render_webhook_subscription_sync_report(&report)))
}

fn render_webhook_subscription_sync_report(
    report: &crate::webhook::subscriptions::SubscriptionSyncReport,
) -> String {
    format!(
        "\
ringmaster webhook subscriptions sync

dry_run: {}
prune_requested: {}
callback_url: {}
fixture_dir: {}
remote_before: {}
remote_after: {}
plan:
{}
notes:
{}
",
        report.dry_run,
        report.prune_requested,
        report.callback_url,
        report
            .fixture_dir
            .as_ref()
            .map_or_else(|| "live".to_owned(), |path| path.display().to_string()),
        report.remote_before.len(),
        report.remote_after.len(),
        render_webhook_subscription_plan(report),
        render_webhook_subscription_notes(report),
    )
}

fn render_webhook_subscription_plan(
    report: &crate::webhook::subscriptions::SubscriptionSyncReport,
) -> String {
    let plan_lines = [
        report
            .plan
            .create
            .iter()
            .map(|entry| {
                format!(
                    "  - create {}:{} -> {}",
                    entry.data_type,
                    entry.event_type.as_str(),
                    entry.callback_url
                )
            })
            .collect::<Vec<_>>(),
        report
            .plan
            .update
            .iter()
            .map(|entry| {
                format!(
                    "  - update {}:{} id={} {} -> {}",
                    entry.desired.data_type,
                    entry.desired.event_type.as_str(),
                    entry.existing.id,
                    entry.existing.callback_url,
                    entry.desired.callback_url
                )
            })
            .collect::<Vec<_>>(),
        report
            .plan
            .renew
            .iter()
            .map(|entry| {
                format!(
                    "  - renew {}:{} id={} expires_at={}",
                    entry.data_type,
                    entry.event_type.as_str(),
                    entry.id,
                    entry.expiration_time
                )
            })
            .collect::<Vec<_>>(),
        report
            .plan
            .prune
            .iter()
            .map(|entry| {
                format!(
                    "  - prune {}:{} id={}",
                    entry.data_type,
                    entry.event_type.as_str(),
                    entry.id
                )
            })
            .collect::<Vec<_>>(),
        report
            .plan
            .notes
            .iter()
            .map(|note| format!("  - note: {note}"))
            .collect::<Vec<_>>(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    if plan_lines.is_empty() {
        "  - no changes".to_owned()
    } else {
        plan_lines.join("\n")
    }
}

fn render_webhook_subscription_notes(
    report: &crate::webhook::subscriptions::SubscriptionSyncReport,
) -> String {
    if report.notes.is_empty() {
        "  - none".to_owned()
    } else {
        report
            .notes
            .iter()
            .map(|note| format!("  - {note}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

async fn run_derive_rebuild(config: &Config, args: DeriveRebuildArgs) -> Result<Option<String>> {
    let report = derive::rebuild(
        config,
        derive::DeriveOptions {
            demo: args.demo,
            fixture_dir: args.fixture_dir,
        },
    )
    .await?;
    let notes = if report.notes.is_empty() {
        "  - none".to_owned()
    } else {
        report
            .notes
            .iter()
            .map(|note| format!("  - {note}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let output = format!(
        "\
ringmaster derive rebuild

database_path: {}
derived_context_events: {}
derived_pattern_summaries: {}
derived_review_signal_days: {}
notes:
{}
",
        report.database_path,
        report.context_event_count,
        report.pattern_summary_count,
        report.review_signal_day_count,
        notes,
    );

    Ok(Some(output))
}

#[derive(Debug)]
pub(crate) struct SnapshotCommandContext {
    guard: Option<TempRootGuard>,
    pub(crate) config: Config,
    pub(crate) store: Store,
    pub(crate) auth_status: oura::models::AuthStatus,
}

async fn run_snapshot_export(config: &Config, args: SnapshotExportArgs) -> Result<Option<String>> {
    let privacy_profile = snapshot::PrivacyProfile::try_from(args.profile)?;
    let context =
        load_snapshot_command_context(config, args.demo, args.fixture_dir.clone()).await?;
    let scope = snapshot::resolve_scope(&context.store, &args.scope)?;
    let export = snapshot::export_snapshot(
        &context.config,
        &context.store,
        &context.auth_status,
        if args.demo {
            snapshot::SnapshotSourceMode::Demo
        } else {
            snapshot::SnapshotSourceMode::Live
        },
        args.fixture_dir.as_deref(),
        &scope,
        privacy_profile,
    )?;
    context
        .store
        .analysis()
        .upsert_snapshot_export(&export.manifest_record, &export.provenance_records)?;

    let rendered_json = if args.compact {
        export.compact_json.clone()
    } else {
        export.pretty_json.clone()
    };
    if let Some(out_path) = args.out {
        write_text_file(&out_path, &rendered_json, "writing snapshot export")?;
        return Ok(Some(format!(
            "\
ringmaster snapshot export

snapshot_hash: {}
scope: {}
privacy_profile: {}
source_mode: {}
out: {}
",
            export.bundle.metadata.snapshot_hash,
            export.bundle.metadata.scope,
            export.bundle.metadata.privacy_profile.as_str(),
            export.bundle.metadata.source_mode.as_str(),
            out_path.display(),
        )));
    }

    Ok(Some(rendered_json))
}

async fn run_snapshot_list(config: &Config, args: SnapshotListArgs) -> Result<Option<String>> {
    let fixture_dir = resolved_demo_fixture_dir(config, args.demo, args.fixture_dir.clone());
    let context =
        load_library_command_context(config, args.demo, fixture_dir.clone(), false).await?;
    let records = context.store.analysis().list_snapshot_exports()?;
    let mut lines = vec!["ringmaster snapshot list".to_owned(), String::new()];
    if records.is_empty() {
        lines.push("snapshots: none".to_owned());
    } else {
        lines.push("snapshots:".to_owned());
        lines.extend(records.iter().map(|record| {
            format!(
                "  - {} | {} | scope={} | profile={} | {}",
                short_id(&record.snapshot_hash, 12),
                record.created_at,
                record.scope,
                record.privacy_profile,
                record.freshness_summary
            )
        }));
    }

    Ok(Some(lines.join("\n")))
}

async fn run_snapshot_show(config: &Config, args: SnapshotShowArgs) -> Result<Option<String>> {
    let path = PathBuf::from(&args.snapshot);
    let fixture_dir = resolved_demo_fixture_dir(config, args.demo, args.fixture_dir.clone());
    let context =
        load_library_command_context(config, args.demo, fixture_dir.clone(), false).await?;

    let (bundle, provenance_records, source_label) = if path.exists() {
        let artifact = snapshot::load_snapshot_artifact(&path)?;
        let provenance_records = context
            .store
            .analysis()
            .snapshot_provenance_refs(&artifact.bundle.metadata.snapshot_hash)
            .unwrap_or_default();
        (
            artifact.bundle,
            provenance_records,
            format!("file: {}", path.display()),
        )
    } else {
        let Some(record) = resolve_snapshot_record(&context.store, &args.snapshot)? else {
            return Err(RingmasterError::Ui(format!(
                "snapshot `{}` was not found in the local catalog and is not a readable file path",
                args.snapshot
            )));
        };
        let provenance_records = context
            .store
            .analysis()
            .snapshot_provenance_refs(&record.snapshot_hash)?;
        (
            snapshot::deserialize_snapshot_bundle(&record.snapshot_json)?,
            provenance_records,
            "catalog".to_owned(),
        )
    };

    let summary = snapshot::summarize_snapshot_bundle(&bundle, &provenance_records);
    let linked_runs = context
        .store
        .analysis()
        .list_ai_artifacts_for_snapshot(&bundle.metadata.snapshot_hash)?;
    let linked_reports = context
        .store
        .analysis()
        .report_exports_for_snapshot(&bundle.metadata.snapshot_hash)?;

    Ok(Some(render_snapshot_show_output(
        &bundle,
        &summary,
        &provenance_records,
        &linked_runs,
        &linked_reports,
        &source_label,
    )))
}

async fn run_ai_review(config: &Config, args: AiReviewArgs) -> Result<Option<String>> {
    let snapshot_artifact = snapshot::load_snapshot_artifact(&args.snapshot_path)?;
    let show_request_preview = args.dry_run || args.fixture.is_some();
    let output = ai::review_snapshot(
        config,
        &snapshot_artifact,
        args.dry_run,
        args.fixture.as_deref(),
    )
    .await?;
    let store = Store::open(config)?;
    ensure_snapshot_catalog_entry(
        &store,
        &snapshot_artifact,
        args.fixture.as_deref().and_then(|path| path.parent()),
    )?;
    store.analysis().upsert_ai_artifact(&output.record)?;
    store
        .analysis()
        .upsert_ai_run(&completed_ai_run_record(&CompletedAiRunRecordInput {
            run_kind: "review",
            snapshot_scope: &snapshot_artifact.bundle.metadata.scope,
            snapshot_hash_a: &snapshot_artifact.bundle.metadata.snapshot_hash,
            snapshot_hash_b: None,
            request_preview: &output.request_preview,
            request_fingerprint: &output.request_fingerprint,
            artifact_record: &output.record,
            source_ai_artifact_id: None,
            follow_up_kind: None,
        })?)?;
    let artifact_output = if let Some(out_path) = args.out {
        write_text_file(
            &out_path,
            &output.payload_json,
            "writing AI review artifact",
        )?;
        format!(
            "{}\n\nartifact_json_path: {}\nartifact_id: {}",
            output.rendered_briefing,
            out_path.display(),
            output.record.artifact_id
        )
    } else {
        format!("{}\n\n{}", output.rendered_briefing, output.payload_json)
    };
    let rendered = if show_request_preview {
        format!(
            "{}\n{}",
            ai::render_request_preview(&output.request_preview).trim_end(),
            artifact_output
        )
    } else {
        artifact_output
    };

    Ok(Some(rendered))
}

async fn run_ai_compare(config: &Config, args: AiCompareArgs) -> Result<Option<String>> {
    let snapshot_a = snapshot::load_snapshot_artifact(&args.snapshot_a)?;
    let snapshot_b = snapshot::load_snapshot_artifact(&args.snapshot_b)?;
    let show_request_preview = args.dry_run || args.fixture.is_some();
    let output = ai::compare_snapshots(
        config,
        &snapshot_a,
        &snapshot_b,
        args.dry_run,
        args.fixture.as_deref(),
    )
    .await?;
    let store = Store::open(config)?;
    ensure_snapshot_catalog_entry(
        &store,
        &snapshot_a,
        args.fixture.as_deref().and_then(|path| path.parent()),
    )?;
    ensure_snapshot_catalog_entry(
        &store,
        &snapshot_b,
        args.fixture.as_deref().and_then(|path| path.parent()),
    )?;
    store.analysis().upsert_ai_artifact(&output.record)?;
    store
        .analysis()
        .upsert_ai_run(&completed_ai_run_record(&CompletedAiRunRecordInput {
            run_kind: "compare",
            snapshot_scope: &snapshot_b.bundle.metadata.scope,
            snapshot_hash_a: &snapshot_a.bundle.metadata.snapshot_hash,
            snapshot_hash_b: Some(&snapshot_b.bundle.metadata.snapshot_hash),
            request_preview: &output.request_preview,
            request_fingerprint: &output.request_fingerprint,
            artifact_record: &output.record,
            source_ai_artifact_id: None,
            follow_up_kind: None,
        })?)?;
    let artifact_output = if let Some(out_path) = args.out {
        write_text_file(
            &out_path,
            &output.payload_json,
            "writing AI compare artifact",
        )?;
        format!(
            "{}\n\nartifact_json_path: {}\nartifact_id: {}",
            output.rendered_briefing,
            out_path.display(),
            output.record.artifact_id
        )
    } else {
        format!("{}\n\n{}", output.rendered_briefing, output.payload_json)
    };
    let rendered = if show_request_preview {
        format!(
            "{}\n{}",
            ai::render_request_preview(&output.request_preview).trim_end(),
            artifact_output
        )
    } else {
        artifact_output
    };

    Ok(Some(rendered))
}

struct CompletedAiRunRecordInput<'a> {
    run_kind: &'a str,
    snapshot_scope: &'a str,
    snapshot_hash_a: &'a str,
    snapshot_hash_b: Option<&'a str>,
    request_preview: &'a ai::AiRequestPreview,
    request_fingerprint: &'a str,
    artifact_record: &'a store::queries::AiArtifactRecord,
    source_ai_artifact_id: Option<&'a str>,
    follow_up_kind: Option<&'a str>,
}

fn completed_ai_run_record(
    input: &CompletedAiRunRecordInput<'_>,
) -> Result<store::queries::AiRunRecord> {
    Ok(store::queries::AiRunRecord {
        run_id: format!("run-{}", input.artifact_record.artifact_id),
        run_kind: input.run_kind.to_owned(),
        run_status: ai::AiRunStatus::Succeeded.as_str().to_owned(),
        provider: input.artifact_record.provider.clone(),
        model: input.artifact_record.model.clone(),
        reasoning_effort: input.artifact_record.reasoning_effort.clone(),
        request_mode: input.artifact_record.request_mode.clone(),
        input_transport: input.artifact_record.input_transport.clone(),
        run_mode: input.artifact_record.run_mode.clone(),
        prompt_version: input.artifact_record.prompt_version.clone(),
        output_schema_version: input.artifact_record.output_schema_version.clone(),
        privacy_profile: input.artifact_record.privacy_profile.clone(),
        snapshot_scope: input.snapshot_scope.to_owned(),
        snapshot_hash_a: input.snapshot_hash_a.to_owned(),
        snapshot_hash_b: input.snapshot_hash_b.map(ToOwned::to_owned),
        source_ai_artifact_id: input.source_ai_artifact_id.map(ToOwned::to_owned),
        follow_up_kind: input.follow_up_kind.map(ToOwned::to_owned),
        request_fingerprint: Some(input.request_fingerprint.to_owned()),
        request_preview_json: serde_json::to_string(input.request_preview)?,
        artifact_id: Some(input.artifact_record.artifact_id.clone()),
        error_message: None,
        created_at: input.artifact_record.created_at.clone(),
        started_at: Some(input.artifact_record.created_at.clone()),
        ended_at: Some(input.artifact_record.created_at.clone()),
        updated_at: input.artifact_record.created_at.clone(),
    })
}

async fn run_ai_runs_list(config: &Config, args: AiRunsListArgs) -> Result<Option<String>> {
    let fixture_dir = resolved_demo_fixture_dir(config, args.demo, args.fixture_dir.clone());
    let context = load_library_command_context(config, args.demo, fixture_dir, true).await?;
    let records = context.store.analysis().list_ai_artifacts()?;
    let mut lines = vec!["ringmaster ai runs list".to_owned(), String::new()];
    if records.is_empty() {
        lines.push("runs: none".to_owned());
    } else {
        lines.push("runs:".to_owned());
        lines.extend(records.iter().map(|record| {
            format!(
                "  - {} | kind={} | status={} | provider={} | model={} | {}",
                short_id(&record.artifact_id, 12),
                record.artifact_kind,
                record.artifact_status,
                record.provider,
                record.model,
                record.summary_cache
            )
        }));
    }

    Ok(Some(lines.join("\n")))
}

async fn run_ai_runs_show(config: &Config, args: AiRunsShowArgs) -> Result<Option<String>> {
    let fixture_dir = resolved_demo_fixture_dir(config, args.demo, args.fixture_dir.clone());
    let context = load_library_command_context(config, args.demo, fixture_dir, true).await?;
    let Some(record) = resolve_ai_run_record(&context.store, &args.run_id)? else {
        return Err(RingmasterError::Ui(format!(
            "AI run `{}` was not found in the local registry",
            args.run_id
        )));
    };
    let linked_reports = context
        .store
        .analysis()
        .report_exports_for_ai_artifact(&record.artifact_id)?;
    let snapshot_a = context
        .store
        .analysis()
        .snapshot_export(&record.snapshot_hash_a)?;
    let snapshot_b = record
        .snapshot_hash_b
        .as_deref()
        .map(|snapshot_hash| context.store.analysis().snapshot_export(snapshot_hash))
        .transpose()?
        .flatten();

    Ok(Some(render_ai_run_show_output(
        &record,
        snapshot_a.as_ref(),
        snapshot_b.as_ref(),
        &linked_reports,
    )))
}

async fn run_report_export(config: &Config, args: ReportExportArgs) -> Result<Option<String>> {
    report::export_report(config, args).await
}

fn run_ai_eval(config: &Config, args: &AiEvalArgs) -> Result<Option<String>> {
    eval::run_eval(config, args)
}

#[derive(Debug, Clone)]
struct ReviewStoreSnapshot {
    auth_status: oura::models::AuthStatus,
    active_population_profile: evidence::PopulationProfile,
    signal_days: Vec<store::queries::ReviewSignalDayRecord>,
    context_events: Vec<store::queries::ContextEventRecord>,
    pattern_summaries: Vec<store::queries::PatternSummaryRecord>,
    sleep_time: Vec<store::queries::SleepTimeRecord>,
    rest_mode_periods: Vec<store::queries::RestModePeriodRecord>,
}

const REVIEW_SIGNAL_LOOKBACK_DAYS: i64 = 60;
const REVIEW_CONTEXT_LOOKBACK_DAYS: i64 = 90;
const REVIEW_SLEEP_LOOKBACK_DAYS: i64 = 60;
const REVIEW_REST_MODE_LOOKBACK_DAYS: i64 = 180;
const REVIEW_CONTEXT_FORWARD_DAYS: i64 = 7;
const EMPTY_REVIEW_ANCHOR_DAY: &str = "none";

async fn run_review_today(config: &Config, args: ReviewTodayArgs) -> Result<Option<String>> {
    let requested_day = args.day.clone();
    let (_guard, snapshot) = load_review_store_snapshot(
        config,
        args.demo,
        args.fixture_dir,
        requested_day.as_deref(),
    )
    .await?;
    let Some(anchor_day) = requested_day.or_else(|| latest_review_day(&snapshot)) else {
        let deck = empty_review_deck(
            ReviewMode::Today,
            "No reviewable days are available yet. Sync data or use --demo to seed a bounded review snapshot.",
        );
        if args.json {
            return Ok(Some(serde_json::to_string_pretty(&deck)?));
        }
        return Ok(Some(render_review_deck("ringmaster review today", &deck)));
    };
    let deck = build_review_deck(ReviewMode::Today, &anchor_day, &review_inputs(&snapshot))?;
    if args.json {
        return Ok(Some(serde_json::to_string_pretty(&deck)?));
    }

    Ok(Some(render_review_deck("ringmaster review today", &deck)))
}

async fn run_review_week(config: &Config, args: ReviewWeekArgs) -> Result<Option<String>> {
    let requested_day = args.end_day.clone();
    let (_guard, snapshot) = load_review_store_snapshot(
        config,
        args.demo,
        args.fixture_dir,
        requested_day.as_deref(),
    )
    .await?;
    let Some(anchor_day) = requested_day.or_else(|| latest_review_day(&snapshot)) else {
        let deck = empty_review_deck(
            ReviewMode::Week,
            "No reviewable days are available yet. Sync data or use --demo to seed a bounded review snapshot.",
        );
        if args.json {
            return Ok(Some(serde_json::to_string_pretty(&deck)?));
        }
        return Ok(Some(render_review_deck("ringmaster review week", &deck)));
    };
    let deck = build_review_deck(ReviewMode::Week, &anchor_day, &review_inputs(&snapshot))?;
    if args.json {
        return Ok(Some(serde_json::to_string_pretty(&deck)?));
    }

    Ok(Some(render_review_deck("ringmaster review week", &deck)))
}

async fn run_review_investigate(
    config: &Config,
    args: ReviewInvestigateArgs,
) -> Result<Option<String>> {
    let requested_day = args.anchor_day.clone();
    let (_guard, snapshot) = load_review_store_snapshot(
        config,
        args.demo,
        args.fixture_dir,
        requested_day.as_deref(),
    )
    .await?;
    let focus = map_review_focus(args.focus);
    let Some(anchor_day) = requested_day.or_else(|| latest_review_day(&snapshot)) else {
        let report = empty_investigation_report(
            focus,
            "No reviewable days are available yet. Sync data or use --demo before running investigations.",
        );
        if args.json {
            return Ok(Some(serde_json::to_string_pretty(&report)?));
        }
        return Ok(Some(render_investigation_report(&report)));
    };
    let report = build_investigation_report(focus, &anchor_day, &review_inputs(&snapshot))?;
    if args.json {
        return Ok(Some(serde_json::to_string_pretty(&report)?));
    }

    Ok(Some(render_investigation_report(&report)))
}

async fn load_review_store_snapshot(
    config: &Config,
    demo: bool,
    fixture_dir: Option<PathBuf>,
    requested_anchor_day: Option<&str>,
) -> Result<(Option<TempRootGuard>, ReviewStoreSnapshot)> {
    if demo {
        let fixture_dir = fixture_dir
            .or_else(|| config.refresh.demo_fixture_dir.clone())
            .unwrap_or_else(|| PathBuf::from("tests/fixtures/phase5"));
        let temp_root = TempRootGuard::new("review");
        let mut temp_config = config.clone();
        temp_config.paths = AppPaths::from_roots(
            config.paths.home_dir.clone(),
            temp_root.path().join("config"),
            temp_root.path().join("state"),
            temp_root.path().join("cache"),
        )?;
        let store = Store::open(&temp_config)?;
        oura::sync::sync_once(
            &temp_config,
            &store,
            oura::sync::SyncOptions {
                dry_run: false,
                fixture_dir: Some(fixture_dir.clone()),
                families: SyncFamily::ALL.to_vec(),
                trigger_source: Some("review_demo".to_owned()),
                trigger_detail: Some("review seed sync".to_owned()),
            },
        )
        .await?;
        derive::rebuild_store(&store)?;
        let auth_status = oura::auth::inspect_auth(&temp_config, &store)?;
        let snapshot = load_review_snapshot_from_artifacts(
            &store,
            &auth_status,
            config.guidance.active_population_profile,
            requested_anchor_day,
            None,
        )?;
        return Ok((Some(temp_root), snapshot));
    }

    let store = Store::open(config)?;
    let auth_status = oura::auth::inspect_auth(config, &store)?;
    let derived =
        derive::derive_review_artifacts_for_anchor_day(&store, config, requested_anchor_day)?;
    let snapshot = load_review_snapshot_from_artifacts(
        &store,
        &auth_status,
        config.guidance.active_population_profile,
        requested_anchor_day,
        derived.as_ref(),
    )?;
    Ok((None, snapshot))
}

async fn load_snapshot_command_context(
    config: &Config,
    demo: bool,
    fixture_dir: Option<PathBuf>,
) -> Result<SnapshotCommandContext> {
    if demo {
        let fixture_dir = fixture_dir
            .or_else(|| config.refresh.demo_fixture_dir.clone())
            .unwrap_or_else(|| PathBuf::from("tests/fixtures/phase5"));
        let temp_root = TempRootGuard::new("snapshot-export");
        let mut temp_config = config.clone();
        temp_config.paths = AppPaths::from_roots(
            config.paths.home_dir.clone(),
            temp_root.path().join("config"),
            temp_root.path().join("state"),
            temp_root.path().join("cache"),
        )?;
        let store = Store::open(&temp_config)?;
        oura::sync::sync_once(
            &temp_config,
            &store,
            oura::sync::SyncOptions {
                dry_run: false,
                fixture_dir: Some(fixture_dir),
                families: SyncFamily::ALL.to_vec(),
                trigger_source: Some("snapshot_export_demo".to_owned()),
                trigger_detail: Some("snapshot export seed sync".to_owned()),
            },
        )
        .await?;
        derive::rebuild_store(&store)?;
        let auth_status = oura::auth::inspect_auth(&temp_config, &store)?;
        return Ok(SnapshotCommandContext {
            guard: Some(temp_root),
            config: temp_config,
            store,
            auth_status,
        });
    }

    let store = Store::open(config)?;
    let auth_status = oura::auth::inspect_auth(config, &store)?;
    Ok(SnapshotCommandContext {
        guard: None,
        config: config.clone(),
        store,
        auth_status,
    })
}

pub(crate) fn resolved_demo_fixture_dir(
    config: &Config,
    demo: bool,
    fixture_dir: Option<PathBuf>,
) -> Option<PathBuf> {
    if demo {
        Some(
            fixture_dir
                .or_else(|| config.refresh.demo_fixture_dir.clone())
                .unwrap_or_else(|| PathBuf::from("tests/fixtures/phase5")),
        )
    } else {
        fixture_dir
    }
}

pub(crate) async fn load_library_command_context(
    config: &Config,
    demo: bool,
    fixture_dir: Option<PathBuf>,
    include_ai_runs: bool,
) -> Result<SnapshotCommandContext> {
    if !demo {
        return load_snapshot_command_context(config, demo, fixture_dir).await;
    }

    let context = load_snapshot_command_context(config, demo, fixture_dir.clone()).await?;
    let SnapshotCommandContext {
        guard,
        config,
        store,
        auth_status,
    } = context;
    let store_plan = store.plan().clone();
    drop(store);
    seed_demo_library_artifacts(
        &config,
        store_plan.clone(),
        auth_status.clone(),
        fixture_dir.as_deref(),
        include_ai_runs,
    )
    .await?;
    let store = Store::open_with_plan(store_plan, config.app_name)?;
    Ok(SnapshotCommandContext {
        guard,
        config,
        store,
        auth_status,
    })
}

pub(crate) async fn seed_demo_library_artifacts(
    config: &Config,
    store_plan: store::StorePlan,
    auth_status: oura::models::AuthStatus,
    fixture_dir: Option<&std::path::Path>,
    include_ai_runs: bool,
) -> Result<()> {
    const DEMO_REVIEW_RUN_ID: &str = "demo-review";
    const DEMO_COMPARE_RUN_ID: &str = "demo-compare";
    const DEMO_REVIEW_RUN_CREATED_AT: &str = "2026-04-10T00:00:00Z";
    const DEMO_COMPARE_RUN_CREATED_AT: &str = "2026-04-10T00:00:01Z";

    let store = Store::open_with_plan(store_plan.clone(), config.app_name)?;
    let today_scope = snapshot::resolve_scope(&store, "today")?;
    let week_scope = snapshot::resolve_scope(&store, "week")?;
    let today_export = snapshot::export_snapshot(
        config,
        &store,
        &auth_status,
        snapshot::SnapshotSourceMode::Demo,
        fixture_dir,
        &today_scope,
        snapshot::PrivacyProfile::Redacted,
    )?;
    store.analysis().upsert_snapshot_export(
        &today_export.manifest_record,
        &today_export.provenance_records,
    )?;

    let week_export = snapshot::export_snapshot(
        config,
        &store,
        &auth_status,
        snapshot::SnapshotSourceMode::Demo,
        fixture_dir,
        &week_scope,
        snapshot::PrivacyProfile::Redacted,
    )?;
    store.analysis().upsert_snapshot_export(
        &week_export.manifest_record,
        &week_export.provenance_records,
    )?;
    drop(store);

    if include_ai_runs {
        let today_artifact = snapshot::LoadedSnapshotArtifact {
            bundle: today_export.bundle.clone(),
            compact_json: today_export.compact_json.clone(),
        };
        let week_artifact = snapshot::LoadedSnapshotArtifact {
            bundle: week_export.bundle.clone(),
            compact_json: week_export.compact_json.clone(),
        };
        let mut review = ai::review_snapshot_with_run_identity(
            config,
            &today_artifact,
            true,
            None,
            Some(DEMO_REVIEW_RUN_CREATED_AT),
        )
        .await?;
        DEMO_REVIEW_RUN_ID.clone_into(&mut review.record.artifact_id);
        let persist_store = Store::open_with_plan(store_plan.clone(), config.app_name)?;
        persist_store
            .analysis()
            .upsert_ai_artifact(&review.record)?;
        let review_run = completed_ai_run_record(&CompletedAiRunRecordInput {
            run_kind: "review",
            snapshot_scope: &today_artifact.bundle.metadata.scope,
            snapshot_hash_a: &today_artifact.bundle.metadata.snapshot_hash,
            snapshot_hash_b: None,
            request_preview: &review.request_preview,
            request_fingerprint: &review.request_fingerprint,
            artifact_record: &review.record,
            source_ai_artifact_id: None,
            follow_up_kind: None,
        })?;
        persist_store.analysis().upsert_ai_run(&review_run)?;
        let mut compare = ai::compare_snapshots_with_run_identity(
            config,
            &today_artifact,
            &week_artifact,
            true,
            None,
            Some(DEMO_COMPARE_RUN_CREATED_AT),
        )
        .await?;
        DEMO_COMPARE_RUN_ID.clone_into(&mut compare.record.artifact_id);
        let persist_store = Store::open_with_plan(store_plan, config.app_name)?;
        persist_store
            .analysis()
            .upsert_ai_artifact(&compare.record)?;
        let compare_run = completed_ai_run_record(&CompletedAiRunRecordInput {
            run_kind: "compare",
            snapshot_scope: &week_artifact.bundle.metadata.scope,
            snapshot_hash_a: &today_artifact.bundle.metadata.snapshot_hash,
            snapshot_hash_b: Some(&week_artifact.bundle.metadata.snapshot_hash),
            request_preview: &compare.request_preview,
            request_fingerprint: &compare.request_fingerprint,
            artifact_record: &compare.record,
            source_ai_artifact_id: None,
            follow_up_kind: None,
        })?;
        persist_store.analysis().upsert_ai_run(&compare_run)?;
    }

    Ok(())
}

fn load_review_snapshot_from_artifacts(
    store: &Store,
    auth_status: &oura::models::AuthStatus,
    active_population_profile: evidence::PopulationProfile,
    requested_anchor_day: Option<&str>,
    derived: Option<&derive::DerivedReviewArtifacts>,
) -> Result<ReviewStoreSnapshot> {
    let materialized_pattern_summaries = store.views().pattern_summaries(None, None)?;
    let Some(anchor_day) = resolve_review_anchor_day(store, requested_anchor_day, derived)? else {
        return Ok(ReviewStoreSnapshot {
            auth_status: auth_status.clone(),
            active_population_profile,
            signal_days: Vec::new(),
            context_events: Vec::new(),
            pattern_summaries: derived.map_or(materialized_pattern_summaries, |artifacts| {
                artifacts.pattern_summaries.clone()
            }),
            sleep_time: Vec::new(),
            rest_mode_periods: Vec::new(),
        });
    };

    let (signal_start_day, signal_end_day) =
        review_day_range(&anchor_day, REVIEW_SIGNAL_LOOKBACK_DAYS, 0)?;
    let (context_start_day, context_end_day) = review_day_range(
        &anchor_day,
        REVIEW_CONTEXT_LOOKBACK_DAYS,
        REVIEW_CONTEXT_FORWARD_DAYS,
    )?;
    let (sleep_start_day, sleep_end_day) =
        review_day_range(&anchor_day, REVIEW_SLEEP_LOOKBACK_DAYS, 0)?;
    let (rest_mode_start_day, rest_mode_end_day) = review_day_range(
        &anchor_day,
        REVIEW_REST_MODE_LOOKBACK_DAYS,
        REVIEW_CONTEXT_FORWARD_DAYS,
    )?;
    let signal_days = if let Some(artifacts) = derived {
        artifacts
            .review_signal_days
            .iter()
            .filter(|row| row.day >= signal_start_day && row.day <= signal_end_day)
            .cloned()
            .collect()
    } else {
        store
            .views()
            .review_signal_days_between_days(&signal_start_day, &signal_end_day)?
    };
    let context_events = if let Some(artifacts) = derived {
        artifacts
            .context_events
            .iter()
            .filter(|row| row.anchor_day >= context_start_day && row.anchor_day <= context_end_day)
            .cloned()
            .collect()
    } else {
        store
            .views()
            .context_events_between_days(&context_start_day, &context_end_day)?
    };

    Ok(ReviewStoreSnapshot {
        auth_status: auth_status.clone(),
        active_population_profile,
        signal_days,
        context_events,
        pattern_summaries: derived
            .map(|artifacts| artifacts.pattern_summaries.clone())
            .unwrap_or(materialized_pattern_summaries),
        sleep_time: store
            .views()
            .sleep_time_between_days(&sleep_start_day, &sleep_end_day)?,
        rest_mode_periods: store
            .views()
            .rest_mode_periods_between_days(&rest_mode_start_day, &rest_mode_end_day)?,
    })
}

fn latest_review_day(snapshot: &ReviewStoreSnapshot) -> Option<String> {
    let current_day = current_local_day_string();
    snapshot
        .signal_days
        .iter()
        .map(|row| row.day.clone())
        .max()
        .or_else(|| snapshot.sleep_time.iter().map(|row| row.day.clone()).max())
        .or_else(|| {
            snapshot
                .rest_mode_periods
                .iter()
                .map(|row| row.end_day.clone().unwrap_or_else(|| current_day.clone()))
                .max()
        })
}

fn resolve_review_anchor_day(
    store: &Store,
    requested_anchor_day: Option<&str>,
    derived: Option<&derive::DerivedReviewArtifacts>,
) -> Result<Option<String>> {
    if let Some(day) = requested_anchor_day {
        let _ = parse_review_day(day)?;
        Ok(Some(day.to_owned()))
    } else {
        if let Some(derived) = derived {
            let latest_derived_day = derived
                .review_signal_days
                .iter()
                .map(|row| row.day.clone())
                .max();
            if latest_derived_day.is_some() {
                return Ok(latest_derived_day);
            }
        }

        store
            .views()
            .latest_review_day()?
            .map_or_else(|| store.views().latest_source_day(), |day| Ok(Some(day)))
    }
}

fn review_day_range(
    anchor_day: &str,
    lookback_days: i64,
    forward_days: i64,
) -> Result<(String, String)> {
    let anchor_date = parse_review_day(anchor_day)?;
    let start_day = anchor_date
        .checked_sub(Duration::days(lookback_days))
        .unwrap_or(anchor_date);
    let end_day = anchor_date
        .checked_add(Duration::days(forward_days))
        .unwrap_or(anchor_date);
    Ok((start_day.to_string(), end_day.to_string()))
}

fn parse_review_day(day: &str) -> Result<Date> {
    Date::parse(
        day,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .map_err(|error| {
        RingmasterError::Config(format!("failed to parse review day `{day}`: {error}"))
    })
}

fn empty_review_deck(mode: ReviewMode, warning: &str) -> ReviewDeck {
    ReviewDeck {
        mode,
        anchor_day: EMPTY_REVIEW_ANCHOR_DAY.to_owned(),
        observations: Vec::new(),
        positive_changes: Vec::new(),
        negative_drifts: Vec::new(),
        unresolved_anomalies: Vec::new(),
        warnings: vec![warning.to_owned()],
    }
}

fn empty_investigation_report(focus: ReviewFocus, warning: &str) -> InvestigationReport {
    InvestigationReport {
        focus,
        anchor_day: EMPTY_REVIEW_ANCHOR_DAY.to_owned(),
        headline: format!(
            "{} investigation has limited direct evidence.",
            focus.label()
        ),
        summary: warning.to_owned(),
        confidence: review::ReviewConfidence::Low,
        sufficiency: review::features::ReviewSufficiency::Missing,
        evidence: Vec::new(),
        counterevidence: Vec::new(),
        warnings: vec![warning.to_owned()],
        look_at: vec![
            "Run sync once or use --demo to seed reviewable data.".to_owned(),
            "Open Doctor to confirm local auth, sync, and freshness state.".to_owned(),
        ],
    }
}

fn review_inputs(snapshot: &ReviewStoreSnapshot) -> ReviewInputs<'_> {
    ReviewInputs {
        auth_status: &snapshot.auth_status,
        active_population_profile: snapshot.active_population_profile,
        signal_days: &snapshot.signal_days,
        context_events: &snapshot.context_events,
        pattern_summaries: &snapshot.pattern_summaries,
        sleep_time: &snapshot.sleep_time,
        rest_mode_periods: &snapshot.rest_mode_periods,
    }
}

const fn map_review_focus(focus: ReviewFocusArg) -> ReviewFocus {
    match focus {
        ReviewFocusArg::Readiness => ReviewFocus::Readiness,
        ReviewFocusArg::Sleep => ReviewFocus::Sleep,
        ReviewFocusArg::Recovery => ReviewFocus::Recovery,
        ReviewFocusArg::Stress => ReviewFocus::Stress,
        ReviewFocusArg::Activity => ReviewFocus::Activity,
    }
}

fn render_review_deck(title: &str, deck: &ReviewDeck) -> String {
    let mut lines = vec![
        title.to_owned(),
        String::new(),
        format!("anchor_day: {}", deck.anchor_day),
    ];
    if !deck.warnings.is_empty() {
        lines.push("warnings:".to_owned());
        lines.extend(deck.warnings.iter().map(|warning| format!("  - {warning}")));
    }
    lines.push("top_observations:".to_owned());
    lines.extend(render_review_cards(&deck.observations));
    if !deck.positive_changes.is_empty() {
        lines.push("positive_changes:".to_owned());
        lines.extend(render_review_cards(&deck.positive_changes));
    }
    if !deck.negative_drifts.is_empty() {
        lines.push("negative_drifts:".to_owned());
        lines.extend(render_review_cards(&deck.negative_drifts));
    }
    if !deck.unresolved_anomalies.is_empty() {
        lines.push("unresolved_anomalies:".to_owned());
        lines.extend(render_review_cards(&deck.unresolved_anomalies));
    }
    lines.join("\n")
}

fn render_review_cards(cards: &[ReviewCard]) -> Vec<String> {
    cards
        .iter()
        .enumerate()
        .flat_map(render_review_card)
        .collect()
}

fn render_review_card(index: (usize, &ReviewCard)) -> Vec<String> {
    let (index, card) = index;
    let mut lines = vec![
        format!("  {}. {}", index + 1, card.headline),
        format!("     {}", card.confidence_label),
        format!("     {}", card.summary),
        format!("     {}", card.why_this_is_shown),
    ];
    if !card.evidence.is_empty() {
        lines.push("     evidence:".to_owned());
        lines.extend(card.evidence.iter().map(|line| format!("       - {line}")));
    }
    if !card.counterevidence.is_empty() {
        lines.push("     counterevidence:".to_owned());
        lines.extend(
            card.counterevidence
                .iter()
                .map(|line| format!("       - {line}")),
        );
    }
    if !card.warnings.is_empty() {
        lines.push("     warnings:".to_owned());
        lines.extend(card.warnings.iter().map(|line| format!("       - {line}")));
    }
    lines
}

fn render_investigation_report(report: &InvestigationReport) -> String {
    let mut lines = vec![
        "ringmaster review investigate".to_owned(),
        String::new(),
        format!("focus: {}", report.focus.as_str()),
        format!("anchor_day: {}", report.anchor_day),
        format!("headline: {}", report.headline),
        format!(
            "confidence: {} / {} data",
            report.confidence.label(),
            report.sufficiency.label()
        ),
        format!("summary: {}", report.summary),
    ];
    if !report.evidence.is_empty() {
        lines.push("evidence:".to_owned());
        lines.extend(report.evidence.iter().map(|line| format!("  - {line}")));
    }
    if !report.counterevidence.is_empty() {
        lines.push("counterevidence:".to_owned());
        lines.extend(
            report
                .counterevidence
                .iter()
                .map(|line| format!("  - {line}")),
        );
    }
    if !report.warnings.is_empty() {
        lines.push("warnings:".to_owned());
        lines.extend(report.warnings.iter().map(|line| format!("  - {line}")));
    }
    lines.push("look_at:".to_owned());
    lines.extend(report.look_at.iter().map(|line| format!("  - {line}")));
    lines.join("\n")
}

fn write_text_file(path: &std::path::Path, contents: &str, context: &'static str) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| RingmasterError::io(context, error))?;
    }
    fs::write(path, contents).map_err(|error| RingmasterError::io(context, error))
}

fn short_id(value: &str, max_len: usize) -> String {
    value.chars().take(max_len).collect()
}

fn render_snapshot_show_output(
    bundle: &snapshot::SnapshotBundleV1,
    summary: &snapshot::SnapshotCatalogSummary,
    provenance_records: &[store::queries::SnapshotProvenanceRefRecord],
    linked_runs: &[store::queries::AiRunListEntry],
    linked_reports: &[store::queries::ReportExportRecord],
    source_label: &str,
) -> String {
    let mut lines = vec![
        "ringmaster snapshot show".to_owned(),
        String::new(),
        format!("source: {source_label}"),
        format!("snapshot_hash: {}", bundle.metadata.snapshot_hash),
        format!("scope: {}", bundle.metadata.scope),
        format!(
            "days: {}..{} (anchor={} count={})",
            bundle.metadata.start_day,
            bundle.metadata.end_day,
            bundle.metadata.anchor_day,
            day_span_count(&bundle.metadata.start_day, &bundle.metadata.end_day),
        ),
        format!(
            "privacy_profile: {}",
            bundle.metadata.privacy_profile.as_str()
        ),
        format!("schema_version: {}", bundle.schema_version),
        format!("generated_at: {}", bundle.metadata.generated_at),
        format!("freshness: {}", summary.freshness_summary),
        format!("trust: {}", summary.trust_summary),
        format!("capabilities: {}", summary.capability_summary),
        format!("provenance: {}", summary.provenance_summary),
    ];
    if !provenance_records.is_empty() {
        lines.push("provenance_refs:".to_owned());
        lines.extend(provenance_records.iter().map(|record| {
            format!(
                "  - {} => {}:{}",
                record.export_ref, record.local_kind, record.local_locator
            )
        }));
    }
    if !linked_runs.is_empty() {
        lines.push("linked_ai_runs:".to_owned());
        lines.extend(linked_runs.iter().map(|record| {
            format!(
                "  - {} | kind={} | status={} | {}",
                short_id(&record.artifact_id, 12),
                record.artifact_kind,
                record.artifact_status,
                record.summary_cache
            )
        }));
    }
    if !linked_reports.is_empty() {
        lines.push("linked_reports:".to_owned());
        lines.extend(linked_reports.iter().map(|record| {
            format!(
                "  - {} | {} | {}",
                short_id(&record.report_id, 12),
                record.format,
                record.output_path
            )
        }));
    }
    lines.join("\n")
}

fn render_ai_run_show_output(
    record: &store::queries::AiArtifactRecord,
    snapshot_a: Option<&store::queries::SnapshotExportRecord>,
    snapshot_b: Option<&store::queries::SnapshotExportRecord>,
    linked_reports: &[store::queries::ReportExportRecord],
) -> String {
    let mut lines = vec![
        "ringmaster ai runs show".to_owned(),
        String::new(),
        format!("artifact_id: {}", record.artifact_id),
        format!("artifact_kind: {}", record.artifact_kind),
        format!("status: {}", record.artifact_status),
        format!("provider: {}", record.provider),
        format!("model: {}", record.model),
        format!("prompt_version: {}", record.prompt_version),
        format!("output_schema_version: {}", record.output_schema_version),
        format!("run_mode: {}", record.run_mode),
        format!("privacy_profile: {}", record.privacy_profile),
        format!("created_at: {}", record.created_at),
        format!("overview: {}", record.overview),
    ];
    if let Some(request_fingerprint) = &record.request_fingerprint {
        lines.push(format!(
            "request_fingerprint: {}",
            short_id(request_fingerprint, 16)
        ));
    }
    if let Some(snapshot_a) = snapshot_a {
        lines.push(format!(
            "snapshot_a: {} ({})",
            snapshot_a.snapshot_hash, snapshot_a.scope
        ));
    } else {
        lines.push(format!("snapshot_a: {}", record.snapshot_hash_a));
    }
    if let Some(snapshot_b_hash) = &record.snapshot_hash_b {
        if let Some(snapshot_b) = snapshot_b {
            lines.push(format!(
                "snapshot_b: {} ({})",
                snapshot_b.snapshot_hash, snapshot_b.scope
            ));
        } else {
            lines.push(format!("snapshot_b: {snapshot_b_hash}"));
        }
    }
    lines.push(String::new());
    lines.push(record.rendered_briefing.clone());
    if !linked_reports.is_empty() {
        lines.push(String::new());
        lines.push("linked_reports:".to_owned());
        lines.extend(linked_reports.iter().map(|report| {
            format!(
                "  - {} | {} | {}",
                short_id(&report.report_id, 12),
                report.format,
                report.output_path
            )
        }));
    }
    lines.join("\n")
}

fn resolve_snapshot_record(
    store: &Store,
    snapshot_ref: &str,
) -> Result<Option<store::queries::SnapshotExportRecord>> {
    if let Some(record) = store.analysis().snapshot_export(snapshot_ref)? {
        return Ok(Some(record));
    }

    let matches = store
        .analysis()
        .snapshot_exports_with_prefix(snapshot_ref)?;
    if matches.len() > 1 {
        return Err(RingmasterError::Ui(format!(
            "snapshot `{snapshot_ref}` matched multiple catalog entries; use a longer prefix"
        )));
    }
    Ok(matches.into_iter().next())
}

fn ensure_snapshot_catalog_entry(
    store: &Store,
    artifact: &snapshot::LoadedSnapshotArtifact,
    fixture_dir: Option<&Path>,
) -> Result<()> {
    let record = snapshot::catalog_record_from_loaded_artifact(artifact, fixture_dir);
    store.analysis().upsert_snapshot_export(&record, &[])?;
    Ok(())
}

fn resolve_ai_run_record(
    store: &Store,
    run_ref: &str,
) -> Result<Option<store::queries::AiArtifactRecord>> {
    if let Some(record) = store.analysis().ai_artifact(run_ref)? {
        return Ok(Some(record));
    }

    let matches = store.analysis().ai_artifacts_with_prefix(run_ref)?;
    if matches.len() > 1 {
        return Err(RingmasterError::Ui(format!(
            "AI run `{run_ref}` matched multiple registry entries; use a longer prefix"
        )));
    }
    Ok(matches.into_iter().next())
}

fn day_span_count(start_day: &str, end_day: &str) -> i64 {
    let parse = |value: &str| {
        Date::parse(
            value,
            &time::macros::format_description!("[year]-[month]-[day]"),
        )
    };
    match (parse(start_day), parse(end_day)) {
        (Ok(start), Ok(end)) => (end - start).whole_days() + 1,
        _ => 0,
    }
}

impl TempRootGuard {
    fn new(prefix: &str) -> Self {
        let timestamp = OffsetDateTime::now_utc().unix_timestamp_nanos().to_string();
        let path = std::env::temp_dir().join(format!("ringmaster-{prefix}-{timestamp}"));
        let _ = std::fs::create_dir_all(&path);
        Self { path }
    }

    const fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for TempRootGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn init_logging(filter: &str) -> Result<()> {
    let result = LOGGING_INIT.get_or_init(|| {
        let env_filter =
            EnvFilter::try_new(filter).unwrap_or_else(|_| EnvFilter::new("ringmaster=info"));
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(true)
            .compact()
            .try_init()
            .map_err(|error| error.to_string())
    });

    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(RingmasterError::Config(format!(
            "failed to initialize tracing subscriber: {error}"
        ))),
    }
}

fn interactive_terminal_available() -> bool {
    stdout().is_terminal() && stdin().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::{
        run_ai_compare, run_ai_eval, run_ai_review, run_ai_runs_show, run_doctor,
        run_report_export, run_review_investigate, run_review_today, run_review_week,
        run_snapshot_export, run_snapshot_show, run_webhook_replay,
    };
    use crate::cli::{
        AiCompareArgs, AiEvalArgs, AiReviewArgs, AiRunsShowArgs, ReportExportArgs, ReportFormatArg,
        ReviewFocusArg, ReviewInvestigateArgs, ReviewTodayArgs, ReviewWeekArgs, SnapshotExportArgs,
        SnapshotShowArgs, WebhookReplayArgs,
    };
    use crate::config::{
        AppPaths, Config, LoggingConfig, OuraConfig, RefreshConfig, WebhookConfig,
    };
    use crate::error::OuraProblem;
    use crate::evidence::evidence_registry_version;
    use crate::store::Store;
    use crate::store::queries::{
        AiRunRecord, AuthSessionRecord, DailyActivityRecord, DailyReadinessRecord,
        DailySleepRecord, RestModePeriodRecord, SnapshotExportRecord,
    };
    use crate::store::webhook_store::{
        AcceptedWebhookDeliveryInput, DesiredWebhookSubscriptionRecord, InvalidationInput,
        RemoteWebhookSubscriptionRecord, RuntimeHeartbeatRecord, now_rfc3339,
    };
    use crate::webhook::{WebhookEventType, default_desired_subscriptions};
    use tempfile::tempdir;
    use time::{Date, Duration, Month, OffsetDateTime, format_description::well_known::Rfc3339};

    fn copy_dir_recursive(source: &std::path::Path, destination: &std::path::Path) {
        std::fs::create_dir_all(destination).unwrap_or_else(|error| {
            panic!(
                "destination directory {} should exist: {error}",
                destination.display()
            )
        });
        for entry in std::fs::read_dir(source).unwrap_or_else(|error| {
            panic!("source directory {} should read: {error}", source.display())
        }) {
            let entry =
                entry.unwrap_or_else(|error| panic!("source entry should load cleanly: {error}"));
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            if source_path.is_dir() {
                copy_dir_recursive(&source_path, &destination_path);
            } else {
                std::fs::copy(&source_path, &destination_path).unwrap_or_else(|error| {
                    panic!(
                        "fixture file {} should copy to {}: {error}",
                        source_path.display(),
                        destination_path.display()
                    )
                });
            }
        }
    }

    fn test_config(
        public_base_url: Option<&str>,
        verification_token: Option<&str>,
    ) -> (tempfile::TempDir, Config) {
        let tempdir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
        let root = tempdir.path();
        let config_root = root.join("config");
        let state_root = root.join("state");
        let secret_file = state_root
            .join("ringmaster")
            .join("secrets")
            .join("oura-tokens.json");
        let cache_root = root.join("cache");
        let paths = AppPaths::from_roots(root.to_path_buf(), config_root, state_root, cache_root)
            .unwrap_or_else(|error| panic!("paths should resolve: {error}"));

        (
            tempdir,
            Config {
                app_name: "ringmaster",
                paths,
                logging: LoggingConfig {
                    filter: "ringmaster=info".to_owned(),
                },
                oura: OuraConfig {
                    client_id: Some("doctor-client".to_owned()),
                    client_secret: Some("doctor-secret".to_owned()),
                    authorize_url: "https://example.invalid/auth".to_owned(),
                    token_url: "https://example.invalid/token".to_owned(),
                    api_base_url: "https://example.invalid/api".to_owned(),
                    secret_backend: crate::config::OuraSecretBackend::Keyring,
                    secret_file,
                    callback_bind: "127.0.0.1:8788".parse().unwrap_or_else(|error| {
                        panic!("socket address should parse in doctor test: {error}")
                    }),
                    callback_path: "/callback".to_owned(),
                    requested_scopes: vec![
                        "personal".to_owned(),
                        "daily".to_owned(),
                        "heartrate".to_owned(),
                        "workout".to_owned(),
                        "tag".to_owned(),
                        "session".to_owned(),
                    ],
                    auth_timeout_secs: 120,
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
                    demo_fixture_dir: Some(std::path::PathBuf::from("tests/fixtures/phase3")),
                },
                webhook: WebhookConfig {
                    bind: "127.0.0.1:8799".parse().unwrap_or_else(|error| {
                        panic!("webhook socket address should parse in doctor test: {error}")
                    }),
                    path: "/webhooks/oura".to_owned(),
                    public_base_url: public_base_url.map(ToOwned::to_owned),
                    verification_token: verification_token.map(ToOwned::to_owned),
                    signature_tolerance_secs: 300,
                    heartbeat_secs: 15,
                    renewal_lead_secs: 7 * 24 * 60 * 60,
                    subscriptions: default_desired_subscriptions(),
                },
                guidance: crate::config::GuidanceConfig::default(),
                ai: crate::config::AiConfig::default(),
            },
        )
    }

    #[test]
    fn snapshot_color_modes_dedupe_duplicate_cli_flags_in_order() {
        let args = crate::cli::UiSnapshotArgs {
            demo: true,
            fixture_dir: None,
            screen: Vec::new(),
            size: Vec::new(),
            ansi_sidecar: true,
            color_mode: vec![
                crate::cli::SnapshotColorModeArg::Truecolor,
                crate::cli::SnapshotColorModeArg::Mono,
                crate::cli::SnapshotColorModeArg::Truecolor,
                crate::cli::SnapshotColorModeArg::Ansi16,
                crate::cli::SnapshotColorModeArg::Mono,
            ],
            out_dir: std::path::PathBuf::from("/tmp/snapshots"),
        };

        assert_eq!(
            super::snapshot_color_modes(&args),
            vec![
                crate::ui::snapshot::SnapshotColorMode::TrueColor,
                crate::ui::snapshot::SnapshotColorMode::Mono,
                crate::ui::snapshot::SnapshotColorMode::Ansi16,
            ]
        );
    }

    fn future_rfc3339(days: i64) -> String {
        (OffsetDateTime::now_utc() + Duration::days(days))
            .format(&Rfc3339)
            .unwrap_or_else(|error| panic!("future timestamp should format in test: {error}"))
    }

    fn seed_snapshot_export(store: &Store, snapshot_hash: &str, anchor_day: &str) {
        let record = SnapshotExportRecord {
            snapshot_hash: snapshot_hash.to_owned(),
            schema_version: crate::snapshot::SNAPSHOT_SCHEMA_VERSION.to_owned(),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            generated_at: format!("{anchor_day}T08:00:00Z"),
            scope: "today".to_owned(),
            start_day: anchor_day.to_owned(),
            end_day: anchor_day.to_owned(),
            anchor_day: anchor_day.to_owned(),
            day_count: 1,
            privacy_profile: "redacted".to_owned(),
            source_mode: "live".to_owned(),
            fixture_dir: None,
            latest_source_day: Some(anchor_day.to_owned()),
            latest_review_day: Some(anchor_day.to_owned()),
            freshness_summary: format!(
                "latest_source_day={anchor_day} latest_review_day={anchor_day} warnings=0"
            ),
            trust_summary: "explicit snapshot export, redacted profile".to_owned(),
            capability_summary: "granted=0 missing=0 requested=0".to_owned(),
            provenance_summary: "refs=0 local_kinds=0".to_owned(),
            snapshot_json: format!(
                "{{\"schema_version\":\"{}\",\"metadata\":{{\"snapshot_hash\":\"{snapshot_hash}\"}}}}",
                crate::snapshot::SNAPSHOT_SCHEMA_VERSION,
            ),
            created_at: format!("{anchor_day}T08:00:01Z"),
        };

        store
            .analysis()
            .upsert_snapshot_export(&record, &[])
            .unwrap_or_else(|error| panic!("snapshot export should seed: {error}"));
    }

    fn seed_active_ai_run(store: &Store, run_id: &str, run_status: &str) {
        let snapshot_hash = "snapshot-hash-active";
        seed_snapshot_export(store, snapshot_hash, "2026-04-10");

        let record = AiRunRecord {
            run_id: run_id.to_owned(),
            run_kind: "review".to_owned(),
            run_status: run_status.to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-5.1".to_owned(),
            reasoning_effort: Some("medium".to_owned()),
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            run_mode: "real".to_owned(),
            prompt_version: "review_prompt_v3".to_owned(),
            output_schema_version: "ringmaster.ai.review.v3".to_owned(),
            privacy_profile: "redacted".to_owned(),
            snapshot_scope: "today".to_owned(),
            snapshot_hash_a: snapshot_hash.to_owned(),
            snapshot_hash_b: None,
            source_ai_artifact_id: None,
            follow_up_kind: None,
            request_fingerprint: Some(format!("fingerprint-{run_id}")),
            request_preview_json: format!("{{\"run_id\":\"{run_id}\"}}"),
            artifact_id: None,
            error_message: None,
            created_at: "2026-04-10T09:00:00Z".to_owned(),
            started_at: Some("2026-04-10T09:00:05Z".to_owned()),
            ended_at: None,
            updated_at: "2026-04-10T09:00:05Z".to_owned(),
        };

        store
            .analysis()
            .upsert_ai_run(&record)
            .unwrap_or_else(|error| panic!("active ai run should seed: {error}"));
    }

    fn seed_historical_review_days(store: &Store) {
        let imports = store.imports();
        let updated_at = "2026-04-08T12:00:00Z";
        let anchor_day = "2025-10-08";
        let start_day = Date::from_calendar_date(2025, Month::September, 1)
            .unwrap_or_else(|error| panic!("historical start day should parse: {error}"));

        for offset in 0_i64..38_i64 {
            let day = start_day
                .checked_add(Duration::days(offset))
                .unwrap_or_else(|| panic!("historical seed day should stay in range"))
                .to_string();
            let is_anchor_day = day == anchor_day;

            imports
                .upsert_daily_sleep(&DailySleepRecord {
                    oura_id: None,
                    day: day.clone(),
                    sleep_score: Some(if is_anchor_day { 60 } else { 82 }),
                    sleep_duration_seconds: Some(if is_anchor_day { 22_500 } else { 27_000 }),
                    raw_cache_key: None,
                    updated_at: updated_at.to_owned(),
                })
                .unwrap_or_else(|error| panic!("sleep seed row should insert: {error}"));
            imports
                .upsert_daily_readiness(&DailyReadinessRecord {
                    oura_id: None,
                    day: day.clone(),
                    readiness_score: Some(if is_anchor_day { 55 } else { 80 }),
                    temperature_deviation: None,
                    temperature_trend_deviation: None,
                    raw_cache_key: None,
                    updated_at: updated_at.to_owned(),
                })
                .unwrap_or_else(|error| panic!("readiness seed row should insert: {error}"));
            imports
                .upsert_daily_activity(&DailyActivityRecord {
                    oura_id: None,
                    day,
                    activity_score: Some(70),
                    active_calories: 350,
                    steps: 8_000,
                    total_calories: 2_000,
                    raw_cache_key: None,
                    updated_at: updated_at.to_owned(),
                })
                .unwrap_or_else(|error| panic!("activity seed row should insert: {error}"));
        }

        imports
            .upsert_daily_sleep(&DailySleepRecord {
                oura_id: None,
                day: "2026-04-08".to_owned(),
                sleep_score: Some(83),
                sleep_duration_seconds: Some(27_600),
                raw_cache_key: None,
                updated_at: updated_at.to_owned(),
            })
            .unwrap_or_else(|error| panic!("latest sleep seed row should insert: {error}"));
        imports
            .upsert_daily_readiness(&DailyReadinessRecord {
                oura_id: None,
                day: "2026-04-08".to_owned(),
                readiness_score: Some(81),
                temperature_deviation: None,
                temperature_trend_deviation: None,
                raw_cache_key: None,
                updated_at: updated_at.to_owned(),
            })
            .unwrap_or_else(|error| panic!("latest readiness seed row should insert: {error}"));
        imports
            .upsert_daily_activity(&DailyActivityRecord {
                oura_id: None,
                day: "2026-04-08".to_owned(),
                activity_score: Some(72),
                active_calories: 360,
                steps: 8_300,
                total_calories: 2_050,
                raw_cache_key: None,
                updated_at: updated_at.to_owned(),
            })
            .unwrap_or_else(|error| panic!("latest activity seed row should insert: {error}"));
    }

    #[test]
    fn doctor_reports_incomplete_webhook_readiness() {
        let (_tempdir, config) = test_config(None, None);

        let report = run_doctor(&config)
            .unwrap_or_else(|error| panic!("doctor should run: {error}"))
            .unwrap_or_else(|| panic!("doctor should return output"));

        assert!(report.contains(&format!(
            "evidence_registry_version: {}",
            evidence_registry_version()
        )));
        assert!(report.contains("webhook_receiver_configured: false"));
        assert!(report.contains("webhook_receiver_status: config incomplete"));
        assert!(report.contains("webhook_missing_public_prereq: true"));
    }

    #[test]
    fn doctor_requires_client_secret_for_receiver_readiness() {
        let (_tempdir, mut config) =
            test_config(Some("https://example.test"), Some("verify-token"));
        config.oura.client_secret = None;

        let report = run_doctor(&config)
            .unwrap_or_else(|error| panic!("doctor should run without client secret: {error}"))
            .unwrap_or_else(|| panic!("doctor should return output"));

        assert!(report.contains("webhook_receiver_configured: false"));
        assert!(report.contains("webhook_receiver_status: config incomplete"));
    }

    #[test]
    fn doctor_reports_webhook_queue_and_heartbeats() {
        let (_tempdir, config) = test_config(Some("https://example.test"), Some("verify-token"));
        let store = Store::open(&config)
            .unwrap_or_else(|error| panic!("store should open for doctor test: {error}"));
        let received_at = now_rfc3339()
            .unwrap_or_else(|error| panic!("timestamp should format for doctor test: {error}"));

        store
            .webhook()
            .replace_desired_subscriptions(&[DesiredWebhookSubscriptionRecord {
                data_type: "daily_sleep".to_owned(),
                event_type: WebhookEventType::Update,
                enabled: true,
                callback_url: Some("https://example.test/webhooks/oura".to_owned()),
                updated_at: received_at.clone(),
            }])
            .unwrap_or_else(|error| panic!("desired subscriptions should seed: {error}"));
        store
            .webhook()
            .replace_remote_subscriptions(&[RemoteWebhookSubscriptionRecord {
                subscription_id: "sub_123".to_owned(),
                callback_url: "https://example.test/webhooks/oura".to_owned(),
                event_type: WebhookEventType::Update,
                data_type: "daily_sleep".to_owned(),
                expiration_time: future_rfc3339(14),
                drift_status: "matched".to_owned(),
                last_seen_at: received_at.clone(),
                created_at: received_at.clone(),
                updated_at: received_at.clone(),
            }])
            .unwrap_or_else(|error| panic!("remote subscriptions should seed: {error}"));
        let delivery = store
            .webhook()
            .insert_accepted_delivery(&AcceptedWebhookDeliveryInput {
                delivery_fingerprint: "fingerprint".to_owned(),
                received_at: received_at.clone(),
                signature_timestamp: Some(received_at.clone()),
                data_type: Some("daily_sleep".to_owned()),
                event_type: Some(WebhookEventType::Update),
                object_id: Some("daily_sleep_2026-04-08".to_owned()),
                payload_json: "{}".to_owned(),
                headers_json: "{}".to_owned(),
                query_json: "{}".to_owned(),
            })
            .unwrap_or_else(|error| panic!("accepted delivery should seed: {error}"));
        let delivery_id = match delivery {
            crate::store::webhook_store::AcceptedWebhookDeliveryResult::Inserted(record)
            | crate::store::webhook_store::AcceptedWebhookDeliveryResult::Duplicate(record) => {
                record.delivery_id
            }
        };
        store
            .webhook()
            .enqueue_invalidation(&InvalidationInput {
                queue_key: "daily_sleep:update:daily_sleep_2026-04-08".to_owned(),
                data_type: "daily_sleep".to_owned(),
                event_type: WebhookEventType::Update,
                object_id: Some("daily_sleep_2026-04-08".to_owned()),
                delivery_id,
                queued_at: received_at.clone(),
                available_at: received_at.clone(),
            })
            .unwrap_or_else(|error| panic!("invalidation should seed: {error}"));
        for component in ["webhook.receiver", "sync.watch"] {
            store
                .webhook()
                .upsert_runtime_heartbeat(&RuntimeHeartbeatRecord {
                    component: component.to_owned(),
                    mode: "running".to_owned(),
                    bind_address: Some("127.0.0.1:8799".to_owned()),
                    public_base_url: Some("https://example.test".to_owned()),
                    detail: Some("healthy".to_owned()),
                    last_seen_at: received_at.clone(),
                })
                .unwrap_or_else(|error| panic!("heartbeat should seed: {error}"));
        }

        let report = run_doctor(&config)
            .unwrap_or_else(|error| panic!("doctor should run with webhook state: {error}"))
            .unwrap_or_else(|| panic!("doctor should return output"));

        assert!(report.contains("webhook_receiver_status: healthy"));
        assert!(report.contains("webhook_runtime_mode: full hybrid"));
        assert!(report.contains("webhook_queue_depth: 1"));
        assert!(report.contains("webhook_remote_healthy: 1"));
    }

    #[test]
    fn doctor_reports_auth_last_error() {
        let (_tempdir, config) = test_config(None, None);
        let store = Store::open(&config)
            .unwrap_or_else(|error| panic!("store should open for doctor auth test: {error}"));

        store
            .auth()
            .upsert(&AuthSessionRecord {
                provider: crate::store::queries::OURA_PROVIDER.to_owned(),
                account_id: Some("fixture-user".to_owned()),
                account_email: Some("fixture@example.com".to_owned()),
                token_type: "Bearer".to_owned(),
                granted_scopes: vec!["daily".to_owned()],
                access_token_expires_at: None,
                last_authenticated_at: Some("2026-04-08T12:00:00Z".to_owned()),
                last_refresh_at: None,
                last_error: Some(OuraProblem::new(
                    Some(503),
                    "Secret backend unavailable",
                    Some("unlock the keychain".to_owned()),
                )),
                updated_at: "2026-04-08T12:00:00Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("auth session should seed: {error}"));

        let report = run_doctor(&config)
            .unwrap_or_else(|error| panic!("doctor should run with auth error: {error}"))
            .unwrap_or_else(|| panic!("doctor should return output"));

        assert!(
            report.contains(
                "auth_last_error: Oura API problem 503: Secret backend unavailable (unlock the keychain)"
            )
        );
    }

    #[test]
    fn doctor_treats_stopped_watch_heartbeat_as_inactive() {
        let (_tempdir, config) = test_config(Some("https://example.test"), Some("verify-token"));
        let store = Store::open(&config).unwrap_or_else(|error| {
            panic!("store should open for stopped heartbeat test: {error}")
        });
        let received_at = now_rfc3339().unwrap_or_else(|error| {
            panic!("timestamp should format for stopped heartbeat test: {error}")
        });

        store
            .webhook()
            .upsert_runtime_heartbeat(&RuntimeHeartbeatRecord {
                component: "sync.watch".to_owned(),
                mode: "stopped".to_owned(),
                bind_address: None,
                public_base_url: Some("https://example.test".to_owned()),
                detail: Some("watch loop stopped after 1 bounded iteration(s)".to_owned()),
                last_seen_at: received_at,
            })
            .unwrap_or_else(|error| panic!("stopped heartbeat should seed: {error}"));

        let report = run_doctor(&config)
            .unwrap_or_else(|error| {
                panic!("doctor should run with stopped watch heartbeat: {error}")
            })
            .unwrap_or_else(|| panic!("doctor should return output"));

        assert!(report.contains("webhook_watch_heartbeat: stopped | mode=stopped"));
        assert!(report.contains("webhook_runtime_mode: scheduler only"));
    }

    #[test]
    fn doctor_excludes_drifted_remote_subscriptions_from_healthy_total() {
        let (_tempdir, config) = test_config(Some("https://example.test"), Some("verify-token"));
        let store = Store::open(&config).unwrap_or_else(|error| {
            panic!("store should open for drifted subscription doctor test: {error}")
        });
        let received_at = now_rfc3339().unwrap_or_else(|error| {
            panic!("timestamp should format for drifted subscription doctor test: {error}")
        });

        store
            .webhook()
            .replace_remote_subscriptions(&[
                RemoteWebhookSubscriptionRecord {
                    subscription_id: "sub-matched".to_owned(),
                    callback_url: "https://example.test/webhooks/oura".to_owned(),
                    event_type: WebhookEventType::Update,
                    data_type: "daily_sleep".to_owned(),
                    expiration_time: future_rfc3339(14),
                    drift_status: "matched".to_owned(),
                    last_seen_at: received_at.clone(),
                    created_at: received_at.clone(),
                    updated_at: received_at.clone(),
                },
                RemoteWebhookSubscriptionRecord {
                    subscription_id: "sub-diverged".to_owned(),
                    callback_url: "https://other.test/webhooks/oura".to_owned(),
                    event_type: WebhookEventType::Update,
                    data_type: "workout".to_owned(),
                    expiration_time: future_rfc3339(14),
                    drift_status: "diverged".to_owned(),
                    last_seen_at: received_at.clone(),
                    created_at: received_at.clone(),
                    updated_at: received_at,
                },
            ])
            .unwrap_or_else(|error| panic!("remote subscriptions should seed: {error}"));

        let report = run_doctor(&config)
            .unwrap_or_else(|error| {
                panic!("doctor should run with drifted remote subscriptions: {error}")
            })
            .unwrap_or_else(|| panic!("doctor should return output"));

        assert!(report.contains("webhook_remote_subscriptions: 2"));
        assert!(report.contains("webhook_remote_healthy: 1"));
    }

    #[tokio::test]
    async fn review_week_handles_empty_store_without_unknown_anchor() {
        let (_tempdir, config) = test_config(Some("https://example.test"), Some("verify-token"));

        let output = run_review_week(
            &config,
            ReviewWeekArgs {
                end_day: None,
                json: false,
                demo: false,
                fixture_dir: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("review week should not fail on an empty store: {error}"))
        .unwrap_or_else(|| panic!("review week should render text output"));

        assert!(output.contains("ringmaster review week"));
        assert!(output.contains("No reviewable days are available yet"));
        assert!(!output.contains("unknown"));
    }

    #[tokio::test]
    async fn review_investigate_handles_empty_store_without_unknown_anchor() {
        let (_tempdir, config) = test_config(Some("https://example.test"), Some("verify-token"));

        let output = run_review_investigate(
            &config,
            ReviewInvestigateArgs {
                focus: ReviewFocusArg::Readiness,
                anchor_day: None,
                json: false,
                demo: false,
                fixture_dir: None,
            },
        )
        .await
        .unwrap_or_else(|error| {
            panic!("review investigate should not fail on an empty store: {error}")
        })
        .unwrap_or_else(|| panic!("review investigate should render text output"));

        assert!(output.contains("ringmaster review investigate"));
        assert!(output.contains("No reviewable days are available yet"));
        assert!(!output.contains("unknown"));
    }

    #[tokio::test]
    async fn review_today_rebuilds_around_requested_historical_day() {
        let (_tempdir, config) = test_config(Some("https://example.test"), Some("verify-token"));
        let store = Store::open(&config)
            .unwrap_or_else(|error| panic!("store should open for review test: {error}"));
        seed_historical_review_days(&store);

        let output = run_review_today(
            &config,
            ReviewTodayArgs {
                day: Some("2025-10-08".to_owned()),
                json: false,
                demo: false,
                fixture_dir: None,
            },
        )
        .await
        .unwrap_or_else(|error| {
            panic!("review today should build from a requested historical day: {error}")
        })
        .unwrap_or_else(|| panic!("review today should render text output"));

        assert!(output.contains("anchor_day: 2025-10-08"));
        assert!(output.contains("top_observations:"));
        assert!(output.contains("\n  1. "));
        assert!(!output.contains("No reviewable days are available yet"));
    }

    #[tokio::test]
    async fn review_today_requested_day_does_not_mutate_materialized_derived_tables() {
        let (_tempdir, config) = test_config(Some("https://example.test"), Some("verify-token"));
        let store = Store::open(&config)
            .unwrap_or_else(|error| panic!("store should open for review mutation test: {error}"));
        seed_historical_review_days(&store);
        crate::derive::rebuild_store(&store).unwrap_or_else(|error| {
            panic!(
                "full derive rebuild should materialize review data before the read test: {error}"
            )
        });

        let counts_before = store.views().record_counts().unwrap_or_else(|error| {
            panic!("record counts should load before review read test: {error}")
        });
        let latest_review_before = store.views().latest_review_day().unwrap_or_else(|error| {
            panic!("latest review day should load before review read test: {error}")
        });

        let output = run_review_today(
            &config,
            ReviewTodayArgs {
                day: Some("2024-01-01".to_owned()),
                json: false,
                demo: false,
                fixture_dir: None,
            },
        )
        .await
        .unwrap_or_else(|error| {
            panic!("review today should stay read-only for out-of-window historical days: {error}")
        })
        .unwrap_or_else(|| panic!("review today should render text output"));

        assert!(output.contains("anchor_day: 2024-01-01"));

        let counts_after = store.views().record_counts().unwrap_or_else(|error| {
            panic!("record counts should load after review read test: {error}")
        });
        let latest_review_after = store.views().latest_review_day().unwrap_or_else(|error| {
            panic!("latest review day should load after review read test: {error}")
        });

        assert_eq!(
            counts_after.derived_context_events,
            counts_before.derived_context_events
        );
        assert_eq!(
            counts_after.derived_pattern_summaries,
            counts_before.derived_pattern_summaries
        );
        assert_eq!(
            counts_after.derived_review_signal_days,
            counts_before.derived_review_signal_days
        );
        assert_eq!(latest_review_after, latest_review_before);
    }

    #[test]
    fn latest_review_day_treats_open_rest_mode_as_current() {
        let current_day = crate::time_utils::current_local_day_string();
        let snapshot = super::ReviewStoreSnapshot {
            auth_status: crate::oura::models::AuthStatus {
                configured: false,
                callback_url: "http://localhost/callback".to_owned(),
                requested_scopes: Vec::new(),
                granted_scopes: Vec::new(),
                missing_fields: Vec::new(),
                capability_report: crate::oura::models::CapabilityReport::from_scopes(&[], &[]),
                auth_timeout_secs: 300,
                secret_backend: "test".to_owned(),
                access_token_stored: false,
                refresh_token_stored: false,
                access_token_expires_at: None,
                last_authenticated_at: None,
                last_refresh_at: None,
                account_id: None,
                account_email: None,
                last_error: None,
            },
            active_population_profile: crate::evidence::PopulationProfile::GeneralAdult,
            signal_days: Vec::new(),
            context_events: Vec::new(),
            pattern_summaries: Vec::new(),
            sleep_time: Vec::new(),
            rest_mode_periods: vec![RestModePeriodRecord {
                period_id: "rest-open".to_owned(),
                start_day: "2026-04-01".to_owned(),
                start_time: Some("2026-04-01T02:00:00+00:00".to_owned()),
                end_day: None,
                end_time: None,
                episode_count: 1,
                tags_json: "[]".to_owned(),
                raw_cache_key: None,
                updated_at: "2026-04-09T10:00:00Z".to_owned(),
            }],
        };

        assert_eq!(
            super::latest_review_day(&snapshot).as_deref(),
            Some(current_day.as_str())
        );
    }

    #[test]
    fn build_read_only_live_state_preserves_active_ai_runs() {
        let (_tempdir, config) = test_config(None, None);
        let store = Store::open(&config)
            .unwrap_or_else(|error| panic!("store should open for read-only state test: {error}"));
        seed_active_ai_run(&store, "queued-read-only", "queued");
        let auth_status = crate::oura::auth::inspect_auth(&config, &store)
            .unwrap_or_else(|error| panic!("auth status should resolve: {error}"));

        crate::app::build_read_only_live_state(&config, &store, &auth_status)
            .unwrap_or_else(|error| panic!("read-only live state should build: {error}"));

        let record = store
            .analysis()
            .ai_run("queued-read-only")
            .unwrap_or_else(|error| panic!("seeded ai run should load: {error}"))
            .unwrap_or_else(|| panic!("seeded ai run should exist after read-only build"));
        assert_eq!(record.run_status, "queued");
        assert_eq!(record.error_message, None);
        assert_eq!(record.ended_at, None);
    }

    #[test]
    fn build_live_state_interrupts_active_ai_runs() {
        let (_tempdir, config) = test_config(None, None);
        let store = Store::open(&config)
            .unwrap_or_else(|error| panic!("store should open for live state test: {error}"));
        seed_active_ai_run(&store, "queued-live", "queued");
        let auth_status = crate::oura::auth::inspect_auth(&config, &store)
            .unwrap_or_else(|error| panic!("auth status should resolve: {error}"));

        crate::app::build_live_state(&config, &store, &auth_status)
            .unwrap_or_else(|error| panic!("interactive live state should build: {error}"));

        let record = store
            .analysis()
            .ai_run("queued-live")
            .unwrap_or_else(|error| panic!("seeded ai run should load: {error}"))
            .unwrap_or_else(|| panic!("seeded ai run should exist after live build"));
        assert_eq!(record.run_status, "interrupted");
        assert!(record.error_message.is_some());
        assert!(record.ended_at.is_some());
    }

    #[tokio::test]
    async fn build_snapshot_render_source_live_store_preserves_active_ai_runs() {
        let (_tempdir, config) = test_config(None, None);
        let store = Store::open(&config)
            .unwrap_or_else(|error| panic!("store should open for snapshot render test: {error}"));
        seed_active_ai_run(&store, "running-snapshot", "running");

        super::build_snapshot_render_source(&config, false, None)
            .await
            .unwrap_or_else(|error| panic!("live snapshot render source should build: {error}"));

        let record = store
            .analysis()
            .ai_run("running-snapshot")
            .unwrap_or_else(|error| panic!("seeded ai run should load: {error}"))
            .unwrap_or_else(|| panic!("seeded ai run should exist after snapshot builder"));
        assert_eq!(record.run_status, "running");
        assert_eq!(record.error_message, None);
        assert_eq!(record.ended_at, None);
    }

    #[tokio::test]
    async fn stored_delivery_replay_does_not_run_fixture_backed_processing() {
        let (_tempdir, config) = test_config(Some("https://example.test"), Some("verify-token"));
        let store = Store::open(&config)
            .unwrap_or_else(|error| panic!("store should open for replay test: {error}"));
        let received_at = now_rfc3339()
            .unwrap_or_else(|error| panic!("timestamp should format for replay test: {error}"));
        let delivery_id = match store
            .webhook()
            .insert_accepted_delivery(&AcceptedWebhookDeliveryInput {
                delivery_fingerprint: "stored-replay".to_owned(),
                received_at: received_at.clone(),
                signature_timestamp: Some(received_at),
                data_type: Some("daily_sleep".to_owned()),
                event_type: Some(WebhookEventType::Create),
                object_id: Some("sleep_fixture_002".to_owned()),
                payload_json: "{\"data_type\":\"daily_sleep\",\"event_type\":\"create\",\"object_id\":\"sleep_fixture_002\"}".to_owned(),
                headers_json: "{}".to_owned(),
                query_json: "{}".to_owned(),
            })
            .unwrap_or_else(|error| panic!("accepted delivery should seed replay test: {error}"))
        {
            crate::store::webhook_store::AcceptedWebhookDeliveryResult::Inserted(record)
            | crate::store::webhook_store::AcceptedWebhookDeliveryResult::Duplicate(record) => {
                record.delivery_id
            }
        };

        let output = run_webhook_replay(
            &config,
            WebhookReplayArgs {
                fixture: None,
                delivery_id: Some(delivery_id),
                recent: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("stored replay should succeed: {error}"))
        .unwrap_or_else(|| panic!("stored replay should render output"));

        let counts = store.views().record_counts().unwrap_or_else(|error| {
            panic!("record counts should load after stored replay: {error}")
        });
        let pending = store
            .webhook()
            .list_pending_invalidations()
            .unwrap_or_else(|error| {
                panic!("pending invalidations should load after stored replay: {error}")
            });

        assert_eq!(counts.daily_sleep, 0);
        assert_eq!(pending.len(), 1);
        assert!(output.contains(
            "Stored-delivery replay re-enqueued invalidations without auto-running a fixture-backed sync."
        ));
    }

    #[tokio::test]
    async fn snapshot_export_demo_writes_redacted_bundle() {
        let (_tempdir, mut config) =
            test_config(Some("https://example.test"), Some("verify-token"));
        config.refresh.demo_fixture_dir =
            Some(std::path::PathBuf::from("tests/fixtures/phase7/strong"));
        let out_dir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
        let out_path = out_dir.path().join("snapshot.json");

        let output = run_snapshot_export(
            &config,
            SnapshotExportArgs {
                demo: true,
                fixture_dir: None,
                scope: "today".to_owned(),
                profile: crate::cli::PrivacyProfileArg::Redacted,
                out: Some(out_path.clone()),
                compact: false,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("snapshot export should succeed: {error}"))
        .unwrap_or_else(|| panic!("snapshot export should render output"));

        let raw_snapshot = std::fs::read_to_string(&out_path)
            .unwrap_or_else(|error| panic!("snapshot artifact should read: {error}"));
        let artifact = crate::snapshot::load_snapshot_artifact(&out_path)
            .unwrap_or_else(|error| panic!("snapshot artifact should load: {error}"));

        assert!(output.contains("ringmaster snapshot export"));
        assert!(output.contains("privacy_profile: redacted"));
        assert!(!raw_snapshot.contains("fixture@example.com"));
        assert_eq!(
            artifact.bundle.metadata.privacy_profile,
            crate::snapshot::PrivacyProfile::Redacted
        );
    }

    #[tokio::test]
    async fn ai_review_dry_run_persists_local_artifact() {
        let (_tempdir, mut config) =
            test_config(Some("https://example.test"), Some("verify-token"));
        config.refresh.demo_fixture_dir =
            Some(std::path::PathBuf::from("tests/fixtures/phase7/strong"));
        let out_dir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
        let snapshot_path = out_dir.path().join("snapshot.json");
        run_snapshot_export(
            &config,
            SnapshotExportArgs {
                demo: true,
                fixture_dir: None,
                scope: "today".to_owned(),
                profile: crate::cli::PrivacyProfileArg::Redacted,
                out: Some(snapshot_path.clone()),
                compact: false,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("snapshot export should succeed: {error}"));
        let snapshot_artifact = crate::snapshot::load_snapshot_artifact(&snapshot_path)
            .unwrap_or_else(|error| panic!("snapshot artifact should load: {error}"));

        let output = run_ai_review(
            &config,
            AiReviewArgs {
                snapshot_path: snapshot_path.clone(),
                dry_run: true,
                fixture: None,
                out: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("dry-run review should succeed: {error}"))
        .unwrap_or_else(|| panic!("dry-run review should render output"));

        let store = Store::open(&config)
            .unwrap_or_else(|error| panic!("store should open for ai review test: {error}"));
        let persisted = store
            .analysis()
            .latest_ai_artifact("review", &snapshot_artifact.bundle.metadata.snapshot_hash)
            .unwrap_or_else(|error| panic!("persisted ai review should load: {error}"))
            .unwrap_or_else(|| panic!("persisted ai review should exist"));

        assert!(output.contains("ringmaster ai review"));
        assert_eq!(persisted.run_mode, "dry_run");
    }

    #[tokio::test]
    async fn ai_review_fixture_path_uses_fixture_payload() {
        let (_tempdir, mut config) =
            test_config(Some("https://example.test"), Some("verify-token"));
        config.refresh.demo_fixture_dir =
            Some(std::path::PathBuf::from("tests/fixtures/phase7/strong"));
        let out_dir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
        let snapshot_path = out_dir.path().join("snapshot.json");
        run_snapshot_export(
            &config,
            SnapshotExportArgs {
                demo: true,
                fixture_dir: None,
                scope: "today".to_owned(),
                profile: crate::cli::PrivacyProfileArg::Redacted,
                out: Some(snapshot_path.clone()),
                compact: false,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("snapshot export should succeed: {error}"));

        let output = run_ai_review(
            &config,
            AiReviewArgs {
                snapshot_path,
                dry_run: false,
                fixture: Some(std::path::PathBuf::from(
                    "tests/fixtures/phase7/ai-review-fixture.json",
                )),
                out: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("fixture review should succeed: {error}"))
        .unwrap_or_else(|| panic!("fixture review should render output"));

        assert!(output.contains("Fixture-backed review for regression testing."));
    }

    #[tokio::test]
    async fn ai_compare_dry_run_and_fixture_paths_render() {
        let (_tempdir, mut config) =
            test_config(Some("https://example.test"), Some("verify-token"));
        config.refresh.demo_fixture_dir =
            Some(std::path::PathBuf::from("tests/fixtures/phase7/strong"));
        let out_dir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
        let snapshot_a = out_dir.path().join("snapshot-a.json");
        let snapshot_b = out_dir.path().join("snapshot-b.json");

        run_snapshot_export(
            &config,
            SnapshotExportArgs {
                demo: true,
                fixture_dir: None,
                scope: "today".to_owned(),
                profile: crate::cli::PrivacyProfileArg::Redacted,
                out: Some(snapshot_a.clone()),
                compact: false,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("snapshot export should succeed: {error}"));
        run_snapshot_export(
            &config,
            SnapshotExportArgs {
                demo: true,
                fixture_dir: None,
                scope: "week".to_owned(),
                profile: crate::cli::PrivacyProfileArg::Redacted,
                out: Some(snapshot_b.clone()),
                compact: false,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("second snapshot export should succeed: {error}"));

        let dry_run_output = run_ai_compare(
            &config,
            AiCompareArgs {
                snapshot_a: snapshot_a.clone(),
                snapshot_b: snapshot_b.clone(),
                dry_run: true,
                fixture: None,
                out: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("dry-run compare should succeed: {error}"))
        .unwrap_or_else(|| panic!("dry-run compare should render output"));
        let fixture_output = run_ai_compare(
            &config,
            AiCompareArgs {
                snapshot_a,
                snapshot_b,
                dry_run: false,
                fixture: Some(std::path::PathBuf::from(
                    "tests/fixtures/phase7/ai-compare-fixture.json",
                )),
                out: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("fixture compare should succeed: {error}"))
        .unwrap_or_else(|| panic!("fixture compare should render output"));

        assert!(dry_run_output.contains("ringmaster ai compare"));
        assert!(fixture_output.contains("Fixture-backed comparison for regression testing."));
    }

    #[tokio::test]
    async fn ai_runs_show_resolves_prefix_and_links_report_exports() {
        let (_tempdir, mut config) =
            test_config(Some("https://example.test"), Some("verify-token"));
        config.refresh.demo_fixture_dir =
            Some(std::path::PathBuf::from("tests/fixtures/phase7/strong"));
        let out_dir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
        let snapshot_path = out_dir.path().join("snapshot.json");
        let report_path = out_dir.path().join("report.md");

        run_snapshot_export(
            &config,
            SnapshotExportArgs {
                demo: true,
                fixture_dir: None,
                scope: "today".to_owned(),
                profile: crate::cli::PrivacyProfileArg::Redacted,
                out: Some(snapshot_path.clone()),
                compact: false,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("snapshot export should succeed: {error}"));

        let review_output = run_ai_review(
            &config,
            AiReviewArgs {
                snapshot_path: snapshot_path.clone(),
                dry_run: true,
                fixture: None,
                out: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("dry-run review should succeed: {error}"))
        .unwrap_or_else(|| panic!("dry-run review should render output"));
        assert!(review_output.contains("ringmaster ai request preview"));

        let store = Store::open(&config)
            .unwrap_or_else(|error| panic!("store should open for ai runs test: {error}"));
        let snapshot_artifact = crate::snapshot::load_snapshot_artifact(&snapshot_path)
            .unwrap_or_else(|error| panic!("snapshot artifact should load: {error}"));
        let persisted = store
            .analysis()
            .latest_ai_artifact("review", &snapshot_artifact.bundle.metadata.snapshot_hash)
            .unwrap_or_else(|error| panic!("persisted ai review should load: {error}"))
            .unwrap_or_else(|| panic!("persisted ai review should exist"));

        run_report_export(
            &config,
            ReportExportArgs {
                from_snapshot: None,
                from_ai_run: Some(persisted.artifact_id.clone()),
                format: ReportFormatArg::Markdown,
                out: report_path,
                demo: false,
                fixture_dir: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("report export should succeed: {error}"));

        let prefix = persisted.artifact_id[..12].to_owned();
        let output = run_ai_runs_show(
            &config,
            AiRunsShowArgs {
                run_id: prefix,
                demo: false,
                fixture_dir: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("ai runs show should succeed: {error}"))
        .unwrap_or_else(|| panic!("ai runs show should render output"));

        assert!(output.contains("ringmaster ai runs show"));
        assert!(output.contains("linked_reports:"));
        assert!(output.contains("request_fingerprint:"));
    }

    #[tokio::test]
    async fn report_export_from_ai_run_accepts_unique_prefixes() {
        let (_tempdir, mut config) =
            test_config(Some("https://example.test"), Some("verify-token"));
        config.refresh.demo_fixture_dir =
            Some(std::path::PathBuf::from("tests/fixtures/phase7/strong"));
        let out_dir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
        let snapshot_path = out_dir.path().join("snapshot.json");
        let report_path = out_dir.path().join("report.md");

        run_snapshot_export(
            &config,
            SnapshotExportArgs {
                demo: true,
                fixture_dir: None,
                scope: "today".to_owned(),
                profile: crate::cli::PrivacyProfileArg::Redacted,
                out: Some(snapshot_path.clone()),
                compact: false,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("snapshot export should succeed: {error}"));

        run_ai_review(
            &config,
            AiReviewArgs {
                snapshot_path: snapshot_path.clone(),
                dry_run: true,
                fixture: None,
                out: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("dry-run review should succeed: {error}"));

        let store = Store::open(&config)
            .unwrap_or_else(|error| panic!("store should open for ai runs test: {error}"));
        let snapshot_artifact = crate::snapshot::load_snapshot_artifact(&snapshot_path)
            .unwrap_or_else(|error| panic!("snapshot artifact should load: {error}"));
        let persisted = store
            .analysis()
            .latest_ai_artifact("review", &snapshot_artifact.bundle.metadata.snapshot_hash)
            .unwrap_or_else(|error| panic!("persisted ai review should load: {error}"))
            .unwrap_or_else(|| panic!("persisted ai review should exist"));
        let prefix = persisted.artifact_id[..12].to_owned();

        run_report_export(
            &config,
            ReportExportArgs {
                from_snapshot: None,
                from_ai_run: Some(prefix),
                format: ReportFormatArg::Markdown,
                out: report_path.clone(),
                demo: false,
                fixture_dir: None,
            },
        )
        .await
        .unwrap_or_else(|error| {
            panic!("report export should succeed with a unique prefix: {error}")
        });

        let report_contents = std::fs::read_to_string(&report_path)
            .unwrap_or_else(|error| panic!("report output should be readable: {error}"));
        assert!(report_contents.contains("AI review report"));
        assert!(report_contents.contains(&persisted.artifact_id[..12]));
    }

    #[tokio::test]
    async fn report_export_from_snapshot_accepts_unique_prefixes() {
        let (_tempdir, mut config) =
            test_config(Some("https://example.test"), Some("verify-token"));
        config.refresh.demo_fixture_dir =
            Some(std::path::PathBuf::from("tests/fixtures/phase7/strong"));
        let out_dir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
        let snapshot_path = out_dir.path().join("snapshot.json");
        let report_path = out_dir.path().join("report.md");

        run_snapshot_export(
            &config,
            SnapshotExportArgs {
                demo: true,
                fixture_dir: None,
                scope: "today".to_owned(),
                profile: crate::cli::PrivacyProfileArg::Redacted,
                out: Some(snapshot_path.clone()),
                compact: false,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("snapshot export should succeed: {error}"));

        let snapshot_artifact = crate::snapshot::load_snapshot_artifact(&snapshot_path)
            .unwrap_or_else(|error| panic!("snapshot artifact should load: {error}"));
        let store = Store::open(&config)
            .unwrap_or_else(|error| panic!("store should open for snapshot prefix test: {error}"));
        let record = crate::snapshot::catalog_record_from_loaded_artifact(&snapshot_artifact, None);
        store
            .analysis()
            .upsert_snapshot_export(&record, &[])
            .unwrap_or_else(|error| panic!("snapshot export should persist: {error}"));
        let prefix = snapshot_artifact.bundle.metadata.snapshot_hash[..12].to_owned();

        run_report_export(
            &config,
            ReportExportArgs {
                from_snapshot: Some(prefix),
                from_ai_run: None,
                format: ReportFormatArg::Markdown,
                out: report_path.clone(),
                demo: false,
                fixture_dir: None,
            },
        )
        .await
        .unwrap_or_else(|error| {
            panic!("report export should succeed with a unique snapshot prefix: {error}")
        });

        let report_contents = std::fs::read_to_string(&report_path)
            .unwrap_or_else(|error| panic!("report output should be readable: {error}"));
        assert!(report_contents.contains("Snapshot report"));
        assert!(report_contents.contains(&snapshot_artifact.bundle.metadata.snapshot_hash[..12]));
    }

    #[tokio::test]
    async fn snapshot_show_resolves_catalog_prefix() {
        let (_tempdir, mut config) =
            test_config(Some("https://example.test"), Some("verify-token"));
        config.refresh.demo_fixture_dir =
            Some(std::path::PathBuf::from("tests/fixtures/phase7/strong"));
        let store = Store::open(&config)
            .unwrap_or_else(|error| panic!("store should open for snapshot show test: {error}"));
        let fixture_dir = std::path::PathBuf::from("tests/fixtures/phase7/strong");
        crate::oura::sync::sync_once(
            &config,
            &store,
            crate::oura::sync::SyncOptions {
                dry_run: false,
                fixture_dir: Some(fixture_dir.clone()),
                families: crate::SyncFamily::ALL.to_vec(),
                trigger_source: Some("snapshot_show_test".to_owned()),
                trigger_detail: Some("seed store for snapshot prefix lookup".to_owned()),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("fixture sync should succeed: {error}"));
        crate::derive::rebuild_store(&store)
            .unwrap_or_else(|error| panic!("derive rebuild should succeed: {error}"));
        let auth_status = crate::oura::auth::inspect_auth(&config, &store)
            .unwrap_or_else(|error| panic!("auth status should resolve: {error}"));
        let scope = crate::snapshot::resolve_scope(&store, "today")
            .unwrap_or_else(|error| panic!("scope should resolve: {error}"));
        let export = crate::snapshot::export_snapshot(
            &config,
            &store,
            &auth_status,
            crate::snapshot::SnapshotSourceMode::Demo,
            Some(fixture_dir.as_path()),
            &scope,
            crate::snapshot::PrivacyProfile::Redacted,
        )
        .unwrap_or_else(|error| panic!("snapshot export should succeed: {error}"));
        store
            .analysis()
            .upsert_snapshot_export(&export.manifest_record, &export.provenance_records)
            .unwrap_or_else(|error| panic!("snapshot export should persist: {error}"));

        let output = run_snapshot_show(
            &config,
            SnapshotShowArgs {
                snapshot: export.manifest_record.snapshot_hash[..12].to_owned(),
                demo: false,
                fixture_dir: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("snapshot show should succeed: {error}"))
        .unwrap_or_else(|| panic!("snapshot show should render output"));

        assert!(output.contains("ringmaster snapshot show"));
        assert!(output.contains("source: catalog"));
        assert!(output.contains("snapshot_hash:"));
    }

    #[tokio::test]
    async fn report_export_from_snapshot_and_ai_eval_render_outputs() {
        let (_tempdir, mut config) =
            test_config(Some("https://example.test"), Some("verify-token"));
        config.refresh.demo_fixture_dir =
            Some(std::path::PathBuf::from("tests/fixtures/phase7/strong"));
        let out_dir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
        let snapshot_path = out_dir.path().join("snapshot.json");
        let report_path = out_dir.path().join("report.html");
        let eval_export_path = out_dir.path().join("eval.json");

        run_snapshot_export(
            &config,
            SnapshotExportArgs {
                demo: true,
                fixture_dir: None,
                scope: "today".to_owned(),
                profile: crate::cli::PrivacyProfileArg::Redacted,
                out: Some(snapshot_path.clone()),
                compact: false,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("snapshot export should succeed: {error}"));

        let report_output = run_report_export(
            &config,
            ReportExportArgs {
                from_snapshot: Some(snapshot_path.display().to_string()),
                from_ai_run: None,
                format: ReportFormatArg::Html,
                out: report_path.clone(),
                demo: false,
                fixture_dir: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("report export should succeed: {error}"))
        .unwrap_or_else(|| panic!("report export should render output"));

        let eval_args = AiEvalArgs {
            fixture_dir: std::path::PathBuf::from("tests/fixtures/ai"),
            candidate: None,
            baseline: None,
            export: Some(eval_export_path.clone()),
        };
        let eval_output = run_ai_eval(&config, &eval_args)
            .unwrap_or_else(|error| panic!("ai eval should succeed: {error}"))
            .unwrap_or_else(|| panic!("ai eval should render output"));

        assert!(report_output.contains("ringmaster report export"));
        assert!(
            std::fs::read_to_string(&report_path)
                .unwrap_or_else(|error| panic!("report should read: {error}"))
                .contains("<!DOCTYPE html>")
        );
        assert!(eval_output.contains("ringmaster ai eval"));
        assert!(eval_export_path.exists());
    }

    #[tokio::test]
    async fn ai_review_fails_cleanly_when_provider_is_disabled() {
        let (_tempdir, mut config) =
            test_config(Some("https://example.test"), Some("verify-token"));
        config.refresh.demo_fixture_dir =
            Some(std::path::PathBuf::from("tests/fixtures/phase7/strong"));
        let out_dir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
        let snapshot_path = out_dir.path().join("snapshot.json");
        run_snapshot_export(
            &config,
            SnapshotExportArgs {
                demo: true,
                fixture_dir: None,
                scope: "today".to_owned(),
                profile: crate::cli::PrivacyProfileArg::Redacted,
                out: Some(snapshot_path.clone()),
                compact: false,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("snapshot export should succeed: {error}"));

        let error = run_ai_review(
            &config,
            AiReviewArgs {
                snapshot_path,
                dry_run: false,
                fixture: None,
                out: None,
            },
        )
        .await
        .err()
        .unwrap_or_else(|| panic!("ai review should fail when provider is disabled"));

        assert!(error.to_string().contains("AI provider is disabled"));
    }

    #[tokio::test]
    async fn fixture_replay_previews_processing_without_writing_demo_rows() {
        let (_tempdir, config) = test_config(Some("https://example.test"), Some("verify-token"));
        let store = Store::open(&config)
            .unwrap_or_else(|error| panic!("store should open for fixture replay test: {error}"));
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/webhooks/sample.json");

        let output = run_webhook_replay(
            &config,
            WebhookReplayArgs {
                fixture: Some(fixture),
                delivery_id: None,
                recent: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("fixture replay should succeed: {error}"))
        .unwrap_or_else(|| panic!("fixture replay should render output"));

        let counts = store.views().record_counts().unwrap_or_else(|error| {
            panic!("record counts should load after fixture replay: {error}")
        });
        let pending = store
            .webhook()
            .list_pending_invalidations()
            .unwrap_or_else(|error| {
                panic!("pending invalidations should load after fixture replay: {error}")
            });

        assert_eq!(counts.daily_sleep, 0);
        assert_eq!(counts.daily_readiness, 0);
        assert_eq!(counts.daily_activity, 0);
        assert_eq!(pending.len(), 1);
        assert!(output.contains(
            "Previewed 1 invalidation(s) via fixture-backed sync from tests/fixtures/phase3 without writing to the local store"
        ));
    }

    #[tokio::test]
    async fn scenario_fixture_status_snapshots_are_stable_across_host_config_and_temp_roots() {
        let fixture_root = std::path::Path::new("tests/fixtures/phase7");
        let (_first_tempdir, first_config) =
            test_config(Some("https://host-one.example.test"), Some("verify-one"));
        let (_second_tempdir, second_config) = test_config(None, None);

        let mut first_app =
            super::build_scenario_fixture_snapshot_apps_for_tests(&first_config, fixture_root)
                .await
                .unwrap_or_else(|error| panic!("first scenario fixture apps should build: {error}"))
                .into_iter()
                .find_map(|(scenario, app)| {
                    (scenario == crate::ui::snapshot::SnapshotScenario::Stale).then_some(app)
                })
                .unwrap_or_else(|| panic!("stale scenario fixture app should exist"));
        let mut second_app =
            super::build_scenario_fixture_snapshot_apps_for_tests(&second_config, fixture_root)
                .await
                .unwrap_or_else(|error| {
                    panic!("second scenario fixture apps should build: {error}")
                })
                .into_iter()
                .find_map(|(scenario, app)| {
                    (scenario == crate::ui::snapshot::SnapshotScenario::Stale).then_some(app)
                })
                .unwrap_or_else(|| panic!("stale scenario fixture app should exist"));

        first_app.active_screen = crate::app::Screen::Ops;
        second_app.active_screen = crate::app::Screen::Ops;

        let first_snapshot = crate::tui::render_snapshot(&first_app, 160, 44)
            .unwrap_or_else(|error| panic!("first status snapshot should render: {error}"));
        let second_snapshot = crate::tui::render_snapshot(&second_app, 160, 44)
            .unwrap_or_else(|error| panic!("second status snapshot should render: {error}"));

        assert_eq!(
            first_snapshot, second_snapshot,
            "scenario fixture Status snapshots should not vary with host webhook config or temp paths"
        );
        assert!(first_snapshot.contains("Webhook callback: https://fixture.example.test"));
        assert!(first_snapshot.contains("Latest sync: oura.workouts"));
        assert!(first_snapshot.contains("tests/fixtures/phase7/stale/ringmaster.db"));
    }

    #[tokio::test]
    async fn scenario_fixture_status_snapshots_report_the_actual_fixture_root() {
        let copied_root_tempdir =
            tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
        let copied_root = copied_root_tempdir.path().join("scenario-fixtures");
        copy_dir_recursive(std::path::Path::new("tests/fixtures/phase7"), &copied_root);
        let (_tempdir, config) = test_config(Some("https://example.test"), Some("verify-token"));

        let mut stale_app =
            super::build_scenario_fixture_snapshot_apps_for_tests(&config, copied_root.as_path())
                .await
                .unwrap_or_else(|error| {
                    panic!("copied scenario fixture apps should build: {error}")
                })
                .into_iter()
                .find_map(|(scenario, app)| {
                    (scenario == crate::ui::snapshot::SnapshotScenario::Stale).then_some(app)
                })
                .unwrap_or_else(|| panic!("stale scenario fixture app should exist"));
        stale_app.active_screen = crate::app::Screen::Ops;

        let snapshot = crate::tui::render_snapshot(&stale_app, 160, 44)
            .unwrap_or_else(|error| panic!("status snapshot should render: {error}"));

        assert!(snapshot.contains("scenario-fixtures"));
        assert!(snapshot.contains("Config path: "));
        assert!(snapshot.contains("Database path: "));
        assert!(!snapshot.contains("tests/fixtures/phase7/stale/ringmaster.db"));
    }

    #[tokio::test]
    async fn scenario_fixture_overlays_cover_missing_scope_and_rate_limited_states() {
        let (_tempdir, config) = test_config(Some("https://example.test"), Some("verify-token"));
        let states = super::build_scenario_fixture_snapshot_apps_for_tests(
            &config,
            std::path::Path::new("tests/fixtures/phase7"),
        )
        .await
        .unwrap_or_else(|error| panic!("scenario fixture apps should build: {error}"));

        let mut missing_scope_app = states
            .iter()
            .find_map(|(scenario, app)| {
                (scenario == &crate::ui::snapshot::SnapshotScenario::MissingScope)
                    .then_some(app.clone())
            })
            .unwrap_or_else(|| panic!("missing-scope scenario should exist"));
        missing_scope_app.active_screen = crate::app::Screen::Ops;

        let missing_scope_snapshot = crate::tui::render_snapshot(&missing_scope_app, 120, 36)
            .unwrap_or_else(|error| panic!("missing-scope snapshot should render: {error}"));
        assert!(missing_scope_snapshot.contains("heartrate  [SCOPE]"));
        assert!(missing_scope_snapshot.contains("heartrate scope was not granted"));

        let mut rate_limited_app = states
            .iter()
            .find_map(|(scenario, app)| {
                (scenario == &crate::ui::snapshot::SnapshotScenario::RateLimited)
                    .then_some(app.clone())
            })
            .unwrap_or_else(|| panic!("rate-limited scenario should exist"));
        rate_limited_app.active_screen = crate::app::Screen::Ops;

        let rate_limited_snapshot = crate::tui::render_snapshot(&rate_limited_app, 120, 36)
            .unwrap_or_else(|error| panic!("rate-limited snapshot should render: {error}"));
        assert!(rate_limited_snapshot.contains("heartrate  [429]"));
        assert!(rate_limited_snapshot.contains("rate limited"));
    }

    #[tokio::test]
    async fn single_fixture_status_snapshots_are_stable_across_host_config_and_temp_roots() {
        let fixture_dir = std::path::PathBuf::from("tests/fixtures/phase3");
        let (_first_tempdir, mut first_config) =
            test_config(Some("https://host-one.example.test"), Some("verify-one"));
        let (_second_tempdir, mut second_config) = test_config(None, None);
        first_config.oura.client_secret = None;
        first_config.oura.callback_bind = "127.0.0.1:9999"
            .parse()
            .unwrap_or_else(|error| panic!("test callback bind should parse: {error}"));
        first_config.oura.callback_path = "/custom-callback".to_owned();
        first_config.oura.requested_scopes = vec!["personal".to_owned()];
        first_config.refresh.daily_interval_secs = 999;
        first_config.refresh.heartrate_interval_secs = 777;
        first_config.refresh.personal_stale_after_secs = 9_999;
        second_config.oura.requested_scopes = vec![
            "personal".to_owned(),
            "daily".to_owned(),
            "heartrate".to_owned(),
            "workout".to_owned(),
            "tag".to_owned(),
            "session".to_owned(),
        ];
        second_config.refresh.daily_interval_secs = 1;
        second_config.refresh.heartrate_interval_secs = 2;
        second_config.refresh.personal_stale_after_secs = 3;

        let mut first_app = super::build_fixture_snapshot_app(
            &first_config,
            fixture_dir.clone(),
            "Fixture-backed status snapshot.",
        )
        .await
        .unwrap_or_else(|error| panic!("first single-fixture app should build: {error}"));
        let mut second_app = super::build_fixture_snapshot_app(
            &second_config,
            fixture_dir,
            "Fixture-backed status snapshot.",
        )
        .await
        .unwrap_or_else(|error| panic!("second single-fixture app should build: {error}"));

        first_app.active_screen = crate::app::Screen::Ops;
        second_app.active_screen = crate::app::Screen::Ops;

        let first_snapshot = crate::tui::render_snapshot(&first_app, 160, 44)
            .unwrap_or_else(|error| panic!("first single-fixture snapshot should render: {error}"));
        let second_snapshot =
            crate::tui::render_snapshot(&second_app, 160, 44).unwrap_or_else(|error| {
                panic!("second single-fixture snapshot should render: {error}")
            });

        assert_eq!(
            first_snapshot, second_snapshot,
            "single-fixture Status snapshots should not vary with host webhook config or temp paths"
        );
        assert!(first_snapshot.contains("Webhook callback: https://fixture.example.test"));
        assert!(first_snapshot.contains("Latest sync: oura.workouts"));
        assert!(first_snapshot.contains("Auth state: authenticated"));
        assert!(
            first_snapshot.contains("Granted scopes: personal, daily, heartrate, workout, tag")
        );
        assert!(first_snapshot.contains("tests/fixtures/phase3/ringmaster.db"));
    }

    #[tokio::test]
    async fn single_fixture_snapshots_normalize_auth_state_from_fixture_data() {
        let fixture_dir = std::path::PathBuf::from("tests/fixtures/phase3");
        let (_tempdir, mut config) = test_config(None, None);
        config.oura.client_secret = None;
        config.oura.callback_bind = "127.0.0.1:9999"
            .parse()
            .unwrap_or_else(|error| panic!("test callback bind should parse: {error}"));
        config.oura.callback_path = "/custom-callback".to_owned();
        config.oura.requested_scopes = vec!["personal".to_owned()];

        let snapshot = super::load_fixture_snapshot(&config, fixture_dir)
            .await
            .unwrap_or_else(|error| panic!("single-fixture snapshot should load: {error}"));

        assert!(snapshot.auth_status.configured);
        assert_eq!(
            snapshot.auth_status.callback_url,
            super::FIXTURE_SNAPSHOT_AUTH_CALLBACK_URL
        );
        assert_eq!(
            snapshot.auth_status.requested_scopes,
            vec![
                "personal".to_owned(),
                "daily".to_owned(),
                "heartrate".to_owned(),
                "workout".to_owned(),
                "tag".to_owned(),
                "session".to_owned(),
            ]
        );
        assert_eq!(
            snapshot.auth_status.granted_scopes,
            snapshot.auth_status.requested_scopes
        );
        assert!(snapshot.auth_status.missing_fields.is_empty());
        assert_eq!(
            snapshot.auth_status.secret_backend,
            super::FIXTURE_SNAPSHOT_SECRET_BACKEND
        );
        assert!(snapshot.auth_status.access_token_stored);
        assert!(snapshot.auth_status.refresh_token_stored);
        assert_eq!(
            snapshot.auth_status.access_token_expires_at.as_deref(),
            Some(super::FIXTURE_SNAPSHOT_ACCESS_TOKEN_EXPIRES_AT)
        );
        assert_eq!(
            snapshot.auth_status.last_authenticated_at.as_deref(),
            Some(super::FIXTURE_SNAPSHOT_LAST_AUTHENTICATED_AT)
        );
        assert_eq!(
            snapshot.auth_status.last_refresh_at.as_deref(),
            Some(super::FIXTURE_SNAPSHOT_LAST_REFRESH_AT)
        );
        assert_eq!(
            snapshot.auth_status.account_id.as_deref(),
            Some(super::FIXTURE_SNAPSHOT_ACCOUNT_ID)
        );
        assert_eq!(
            snapshot.auth_status.account_email.as_deref(),
            Some(super::FIXTURE_SNAPSHOT_ACCOUNT_EMAIL)
        );
        assert!(snapshot.auth_status.last_error.is_none());
        assert_eq!(
            snapshot.refresh_policy.personal_interval_secs,
            super::FIXTURE_SNAPSHOT_PERSONAL_INTERVAL_SECS
        );
        assert_eq!(
            snapshot.refresh_policy.daily_interval_secs,
            super::FIXTURE_SNAPSHOT_DAILY_INTERVAL_SECS
        );
        assert_eq!(
            snapshot.refresh_policy.heartrate_interval_secs,
            super::FIXTURE_SNAPSHOT_HEARTRATE_INTERVAL_SECS
        );
        assert_eq!(
            snapshot.refresh_policy.personal_stale_after_secs,
            super::FIXTURE_SNAPSHOT_PERSONAL_STALE_AFTER_SECS
        );
    }

    #[test]
    fn demo_status_snapshots_are_stable_across_host_config() {
        let (_first_tempdir, mut first_config) =
            test_config(Some("https://host-one.example.test"), Some("verify-one"));
        let (_second_tempdir, mut second_config) = test_config(None, None);
        first_config.oura.client_secret = None;
        first_config.oura.callback_bind = "127.0.0.1:9999"
            .parse()
            .unwrap_or_else(|error| panic!("test callback bind should parse: {error}"));
        first_config.oura.callback_path = "/custom-callback".to_owned();
        first_config.oura.requested_scopes = vec!["personal".to_owned()];
        first_config.refresh.daily_interval_secs = 999;
        first_config.refresh.heartrate_interval_secs = 777;
        second_config.oura.requested_scopes = vec![
            "personal".to_owned(),
            "daily".to_owned(),
            "heartrate".to_owned(),
            "workout".to_owned(),
            "tag".to_owned(),
            "session".to_owned(),
        ];
        second_config.refresh.daily_interval_secs = 1;
        second_config.refresh.heartrate_interval_secs = 2;

        let mut first_app = crate::app::build_demo_state(&first_config);
        let mut second_app = crate::app::build_demo_state(&second_config);
        let first_refresh_policy = first_app
            .model
            .ops
            .items
            .iter()
            .find(|item| item.label == "Refresh policy")
            .map_or_else(
                || panic!("first demo app should expose refresh policy"),
                |item| item.value.clone(),
            );
        let second_refresh_policy = second_app
            .model
            .ops
            .items
            .iter()
            .find(|item| item.label == "Refresh policy")
            .map_or_else(
                || panic!("second demo app should expose refresh policy"),
                |item| item.value.clone(),
            );
        first_app.active_screen = crate::app::Screen::Ops;
        second_app.active_screen = crate::app::Screen::Ops;

        let first_snapshot = crate::tui::render_snapshot(&first_app, 160, 44)
            .unwrap_or_else(|error| panic!("first demo status snapshot should render: {error}"));
        let second_snapshot = crate::tui::render_snapshot(&second_app, 160, 44)
            .unwrap_or_else(|error| panic!("second demo status snapshot should render: {error}"));

        assert_eq!(
            first_snapshot, second_snapshot,
            "demo status snapshots should not vary with host auth or refresh config"
        );
        assert!(first_snapshot.contains("Auth state: authenticated"));
        assert!(
            first_snapshot.contains("Granted scopes: email, personal, daily, heartrate, tag"),
            "status snapshot should surface the expanded Oura scope line"
        );
        assert_eq!(
            first_refresh_policy,
            "personal=3600s daily=300s heartrate=60s workouts=600s tags=300s sessions=300s"
        );
        assert_eq!(first_refresh_policy, second_refresh_policy);
    }
}
