#![forbid(unsafe_code)]
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    clippy::perf
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::derive_partial_eq_without_eq,
    clippy::future_not_send,
    clippy::ignored_unit_patterns,
    clippy::map_unwrap_or,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::multiple_crate_versions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_ref_mut,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::ref_option,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::unused_async
)]

pub mod action;
pub mod app;
pub mod cli;
pub mod components;
pub mod config;
pub mod derive;
pub mod error;
pub mod insights;
pub mod oura;
pub mod refresh;
pub mod store;
pub mod tui;
pub mod webhook;

use std::io::{IsTerminal, stdin, stdout};
use std::path::PathBuf;
use std::sync::OnceLock;

use app::{build_demo_state, build_live_state, load_live_snapshot};
use cli::{
    AuthCommand, Cli, Command, DeriveCommand, DeriveRebuildArgs, SyncCommand, SyncOnceArgs,
    SyncWatchArgs, TuiArgs, WebhookCommand, WebhookReplayArgs, WebhookServeArgs,
    WebhookSubscriptionCommand, WebhookSubscriptionsListArgs, WebhookSubscriptionsSyncArgs,
};
use config::Config;
use error::{Result, RingmasterError};
use refresh::{SyncFamily, WatchOptions};
use store::Store;
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

static LOGGING_INIT: OnceLock<std::result::Result<(), String>> = OnceLock::new();

pub async fn run_from<I, T>(args: I) -> Result<Option<String>>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::parse_from(args)?;
    if cli.command.is_none() {
        return Ok(Some(Cli::help_text()));
    }

    run_cli(cli).await
}

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
        Command::Demo => run_demo(&config).await,
        Command::Tui(args) => run_tui(&config, args).await,
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
    }
}

fn run_doctor(config: &Config) -> Result<Option<String>> {
    let store = Store::open(config)?;
    let auth_status = oura::auth::inspect_auth(config, &store)?;
    let snapshot = load_live_snapshot(config, &store, &auth_status)?;
    let capability_lines = auth_status
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
        .join("\n");
    let sync_lines = if snapshot.sync_states.is_empty() {
        "  - none".to_owned()
    } else {
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
    };
    let refresh_lines = SyncFamily::ALL
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
        .join("\n");
    let demo_fixture_dir = config
        .refresh
        .demo_fixture_dir
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "tests/fixtures/phase3".to_owned());
    let record_counts = &snapshot.record_counts;
    let webhook_desired_enabled = snapshot
        .webhook
        .desired_subscriptions
        .iter()
        .filter(|record| record.enabled)
        .count();
    let webhook_remote_healthy = snapshot
        .webhook
        .remote_subscriptions
        .iter()
        .filter(|record| doctor_remote_subscription_healthy(&snapshot, record))
        .count();
    let webhook_renewals_due = snapshot
        .webhook
        .remote_subscriptions
        .iter()
        .filter(|record| doctor_remote_subscription_renewal_due(&snapshot, &record.expiration_time))
        .count();
    let webhook_last_delivery = snapshot.webhook.recent_deliveries.first().map_or_else(
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
    );
    let webhook_last_rejection = snapshot
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
        );
    let webhook_queue_oldest = snapshot
        .webhook
        .pending_invalidations
        .iter()
        .map(|record| record.first_queued_at.as_str())
        .min()
        .unwrap_or("none");
    let webhook_failed_attempts = snapshot
        .webhook
        .recent_processing_attempts
        .iter()
        .filter(|attempt| attempt.outcome == "failed")
        .count();

    let report = format!(
        "\
ringmaster doctor

app_name: {}
config_dir: {}
config_file: {} ({})
state_dir: {}
cache_dir: {}
database_path: {} ({})
log_dir: {}
schema_version: {}
migrations_applied_this_run: {}
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
missing_auth_fields: {}
capabilities:
{}
refresh_policy:
{}
demo_fixture_dir: {}
sync_slices:
{}
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
webhook_failed_attempts: {}
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
  raw_payloads: {}
",
        config.app_name,
        config.paths.config_dir.display(),
        config.paths.config_file.display(),
        if config.paths.config_file_present() {
            "present"
        } else {
            "missing"
        },
        config.paths.state_dir.display(),
        config.paths.cache_dir.display(),
        config.paths.database_file.display(),
        if config.paths.database_present() {
            "present"
        } else {
            "created if needed"
        },
        config.paths.log_dir.display(),
        store.metadata().schema_version()?,
        store.migration_report().applied_versions.len(),
        auth_status.callback_url,
        auth_status.configured,
        if auth_status.access_token_stored || auth_status.refresh_token_stored {
            "authenticated"
        } else if auth_status.configured {
            "configured_without_session"
        } else {
            "unconfigured"
        },
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
        if auth_status.missing_fields.is_empty() {
            "none".to_owned()
        } else {
            auth_status.missing_fields.join(", ")
        },
        capability_lines,
        refresh_lines,
        demo_fixture_dir,
        sync_lines,
        config.webhook.receiver_configured(),
        snapshot
            .webhook
            .callback_url
            .clone()
            .unwrap_or_else(|| "unconfigured".to_owned()),
        snapshot.webhook.verification_token_configured,
        doctor_receiver_status(&snapshot),
        doctor_heartbeat_status(&snapshot, "webhook.receiver"),
        doctor_heartbeat_status(&snapshot, "sync.watch"),
        doctor_runtime_mode(&snapshot),
        snapshot.webhook.callback_url.is_none(),
        webhook_desired_enabled,
        snapshot.webhook.remote_subscriptions.len(),
        webhook_remote_healthy,
        webhook_renewals_due,
        webhook_last_delivery,
        webhook_last_rejection,
        snapshot.webhook.pending_invalidations.len(),
        webhook_queue_oldest,
        webhook_failed_attempts,
        record_counts.personal_info,
        record_counts.daily_sleep,
        record_counts.daily_readiness,
        record_counts.daily_activity,
        record_counts.heartrate_samples,
        record_counts.workouts,
        record_counts.tags,
        record_counts.enhanced_tags,
        record_counts.sessions,
        record_counts.derived_context_events,
        record_counts.derived_pattern_summaries,
        record_counts.raw_payloads,
    );

    Ok(Some(report))
}

fn doctor_receiver_status(snapshot: &app::LiveSnapshot) -> String {
    if snapshot.webhook.callback_url.is_none() || !snapshot.webhook.verification_token_configured {
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

async fn run_demo(config: &Config) -> Result<Option<String>> {
    run_tui(config, TuiArgs { demo: true }).await
}

async fn run_tui(config: &Config, args: TuiArgs) -> Result<Option<String>> {
    let mut app = if args.demo {
        build_demo_state(config)
    } else {
        let store = Store::open(config)?;
        let auth_status = oura::auth::inspect_auth(config, &store)?;
        build_live_state(config, &store, &auth_status)?
    };

    if interactive_terminal_available() {
        if args.demo {
            info!("running demo TUI");
        } else {
            info!("running live TUI");
        }
        tui::run(config, &mut app).await?;
        Ok(None)
    } else {
        if args.demo {
            warn!("demo TUI ran without a tty; rendering a deterministic snapshot instead");
        } else {
            warn!("tui ran without a tty; rendering a live snapshot instead");
        }
        tui::render_snapshot(&app, 100, 32).map(Some)
    }
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
    let last_status = report
        .last_report
        .as_ref()
        .map(|sync_report| sync_report.status.to_string())
        .unwrap_or_else(|| "not-run".to_owned());
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
    )
    .await?;
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
{}
notes:
{}
",
        entries, notes
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
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "live".to_owned()),
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
    let output = format!(
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
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "live".to_owned()),
        report.remote_before.len(),
        report.remote_after.len(),
        if plan_lines.is_empty() {
            "  - no changes".to_owned()
        } else {
            plan_lines.join("\n")
        },
        if report.notes.is_empty() {
            "  - none".to_owned()
        } else {
            report
                .notes
                .iter()
                .map(|note| format!("  - {note}"))
                .collect::<Vec<_>>()
                .join("\n")
        },
    );

    Ok(Some(output))
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
notes:
{}
",
        report.database_path, report.context_event_count, report.pattern_summary_count, notes,
    );

    Ok(Some(output))
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
#[allow(clippy::panic)]
mod tests {
    use super::{run_doctor, run_webhook_replay};
    use crate::cli::WebhookReplayArgs;
    use crate::config::{
        AppPaths, Config, LoggingConfig, OuraConfig, RefreshConfig, WebhookConfig,
    };
    use crate::store::Store;
    use crate::store::webhook_store::{
        AcceptedWebhookDeliveryInput, DesiredWebhookSubscriptionRecord, InvalidationInput,
        RemoteWebhookSubscriptionRecord, RuntimeHeartbeatRecord, now_rfc3339,
    };
    use crate::webhook::{WebhookEventType, default_desired_subscriptions};
    use tempfile::tempdir;
    use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

    fn test_config(
        public_base_url: Option<&str>,
        verification_token: Option<&str>,
    ) -> (tempfile::TempDir, Config) {
        let tempdir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
        let root = tempdir.path();
        let config_root = root.join("config");
        let state_root = root.join("state");
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
                    callback_bind: "127.0.0.1:8788".parse().unwrap_or_else(|error| {
                        panic!("socket address should parse in doctor test: {error}")
                    }),
                    callback_path: "/callback".to_owned(),
                    requested_scopes: vec![
                        "personal".to_owned(),
                        "daily".to_owned(),
                        "heartrate".to_owned(),
                        "workout".to_owned(),
                        "enhanced_tag".to_owned(),
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
            },
        )
    }

    fn future_rfc3339(days: i64) -> String {
        (OffsetDateTime::now_utc() + Duration::days(days))
            .format(&Rfc3339)
            .unwrap_or_else(|error| panic!("future timestamp should format in test: {error}"))
    }

    #[test]
    fn doctor_reports_incomplete_webhook_readiness() {
        let (_tempdir, config) = test_config(None, None);

        let report = run_doctor(&config)
            .unwrap_or_else(|error| panic!("doctor should run: {error}"))
            .unwrap_or_else(|| panic!("doctor should return output"));

        assert!(report.contains("webhook_receiver_configured: false"));
        assert!(report.contains("webhook_receiver_status: config incomplete"));
        assert!(report.contains("webhook_missing_public_prereq: true"));
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
}
