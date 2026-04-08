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

use std::io::{IsTerminal, stdin, stdout};
use std::sync::OnceLock;

use app::{build_demo_state, build_live_state};
use cli::{
    AuthCommand, Cli, Command, DeriveCommand, DeriveRebuildArgs, SyncCommand, SyncOnceArgs,
    SyncWatchArgs, TuiArgs,
};
use config::Config;
use error::{Result, RingmasterError};
use refresh::{SyncFamily, WatchOptions};
use store::Store;
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
        Command::Derive { command } => match command {
            DeriveCommand::Rebuild(args) => run_derive_rebuild(&config, args).await,
        },
    }
}

fn run_doctor(config: &Config) -> Result<Option<String>> {
    let store = Store::open(config)?;
    let auth_status = oura::auth::inspect_auth(config, &store)?;
    let sync_states = store.sync_state().list()?;
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
    let sync_lines = if sync_states.is_empty() {
        "  - none".to_owned()
    } else {
        sync_states
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

    let report = format!(
        "\
ringmaster.rs doctor

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
        store.views().record_counts()?.personal_info,
        store.views().record_counts()?.daily_sleep,
        store.views().record_counts()?.daily_readiness,
        store.views().record_counts()?.daily_activity,
        store.views().record_counts()?.heartrate_samples,
        store.views().record_counts()?.workouts,
        store.views().record_counts()?.tags,
        store.views().record_counts()?.enhanced_tags,
        store.views().record_counts()?.sessions,
        store.views().record_counts()?.derived_context_events,
        store.views().record_counts()?.derived_pattern_summaries,
        store.views().record_counts()?.raw_payloads,
    );

    Ok(Some(report))
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
ringmaster.rs auth login

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
ringmaster.rs sync once

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
ringmaster.rs sync watch

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
ringmaster.rs derive rebuild

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
