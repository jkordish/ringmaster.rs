#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::module_name_repetitions)]

pub mod action;
pub mod app;
pub mod cli;
pub mod components;
pub mod config;
pub mod error;
pub mod oura;
pub mod store;
pub mod tui;

use std::io::{IsTerminal, stdin, stdout};
use std::sync::OnceLock;

use app::{build_demo_state, build_live_state};
use cli::{AuthCommand, Cli, Command, SyncCommand};
use config::Config;
use error::{Result, RingmasterError};
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
        Command::Tui => run_tui(&config).await,
        Command::Auth {
            command: AuthCommand::Login,
        } => run_auth_login(&config).await,
        Command::Sync {
            command: SyncCommand::Once,
        } => run_sync_once(&config).await,
    }
}

fn run_doctor(config: &Config) -> Result<Option<String>> {
    let store = Store::open(config)?;
    let auth_status = oura::auth::inspect_auth(config);
    let latest_sync = store.sync_state().latest()?;
    let capability_lines = auth_status
        .capability_report
        .entries
        .iter()
        .map(|entry| {
            format!(
                "  - {}: {} ({})",
                entry.kind.label(),
                if entry.available { "ready" } else { "waiting" },
                entry.note
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

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
missing_auth_fields: {}
last_sync: {}
capabilities:
{}
record_counts:
  daily_sleep: {}
  daily_readiness: {}
  daily_activity: {}
  heartrate_samples: {}
  workouts: {}
  tags: {}
  enhanced_tags: {}
  sessions: {}
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
        if auth_status.missing_fields.is_empty() {
            "none".to_owned()
        } else {
            auth_status.missing_fields.join(", ")
        },
        latest_sync
            .map(|sync| format!("{} at {}", sync.status, sync.last_attempted_at))
            .unwrap_or_else(|| "never".to_owned()),
        capability_lines,
        store.views().record_counts()?.daily_sleep,
        store.views().record_counts()?.daily_readiness,
        store.views().record_counts()?.daily_activity,
        store.views().record_counts()?.heartrate_samples,
        store.views().record_counts()?.workouts,
        store.views().record_counts()?.tags,
        store.views().record_counts()?.enhanced_tags,
        store.views().record_counts()?.sessions,
        store.views().record_counts()?.raw_payloads,
    );

    Ok(Some(report))
}

async fn run_demo(config: &Config) -> Result<Option<String>> {
    let mut app = build_demo_state(config);

    if interactive_terminal_available() {
        info!("running demo in interactive terminal mode");
        tui::run(&mut app).await?;
        Ok(None)
    } else {
        warn!("demo ran without a tty; rendering a deterministic snapshot instead");
        tui::render_snapshot(&app, 100, 32).map(Some)
    }
}

async fn run_tui(config: &Config) -> Result<Option<String>> {
    let store = Store::open(config)?;
    let auth_status = oura::auth::inspect_auth(config);
    let mut app = build_live_state(config, &store, &auth_status)?;

    if interactive_terminal_available() {
        info!("running live TUI");
        tui::run(&mut app).await?;
        Ok(None)
    } else {
        warn!("tui ran without a tty; rendering a live snapshot instead");
        tui::render_snapshot(&app, 100, 32).map(Some)
    }
}

async fn run_auth_login(config: &Config) -> Result<Option<String>> {
    let plan = oura::auth::prepare_login(config).await?;
    let authorization_url = plan
        .authorization_url
        .unwrap_or_else(|| "unavailable until client credentials are configured".to_owned());
    let notes = plan
        .notes
        .iter()
        .map(|note| format!("  - {note}"))
        .collect::<Vec<_>>()
        .join("\n");

    let output = format!(
        "\
ringmaster.rs auth login

callback_url: {}
listener_bind: {}
requested_scopes: {}
granted_scopes: {}
authorization_url: {}
notes:
{}
",
        plan.auth_status.callback_url,
        plan.listener_plan.bind_address,
        plan.auth_status.requested_scopes.join(", "),
        if plan.auth_status.granted_scopes.is_empty() {
            "none".to_owned()
        } else {
            plan.auth_status.granted_scopes.join(", ")
        },
        authorization_url,
        notes,
    );

    Ok(Some(output))
}

async fn run_sync_once(config: &Config) -> Result<Option<String>> {
    let store = Store::open(config)?;
    let report = oura::sync::sync_once(config, &store).await?;
    let notes = report
        .notes
        .iter()
        .map(|note| format!("  - {note}"))
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
notes:
{}
",
        report.status,
        report.started_at,
        report.finished_at,
        report.database_path,
        report.capability_report.available_labels().join(", "),
        notes,
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
