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

use cli::{Cli, Command};
use config::Config;
use error::Result;

pub fn run_from<I, S>(args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let cli = Cli::parse(args)?;
    run_cli(cli)
}

pub fn run_cli(cli: Cli) -> Result<String> {
    let config = Config::detect()?;

    match cli.command {
        Command::Help => Ok(cli::help_text()),
        Command::Doctor => Ok(doctor_report(&config)),
        Command::Demo => Ok(tui::render_demo(&config)),
        Command::Tui => Ok(tui::render_placeholder(&config)),
        Command::AuthLogin => Ok(oura::auth::login_scaffold(&config)),
        Command::SyncOnce => Ok(oura::sync::sync_once_scaffold(&config)),
    }
}

pub fn doctor_report(config: &Config) -> String {
    let store_plan = store::db::StorePlan::from_config(config);
    let capability_summary = oura::models::CapabilitySet::bootstrap_default();

    format!(
        "\
ringmaster.rs doctor

app_name: {}
config_dir: {}
state_dir: {}
database_path: {}
oauth_callback: {}
mode: bootstrap
capabilities: {}
notes:
  - compileable skeleton is in place
  - demo mode is available
  - real ratatui / oauth / sqlite integration is the next milestone
",
        config.app_name,
        config.config_dir.display(),
        config.state_dir.display(),
        store_plan.db_path.display(),
        config.oauth_callback,
        capability_summary.render()
    )
}
