use std::path::PathBuf;

use clap::{Args, CommandFactory, Parser, Subcommand};

use crate::error::{Result, RingmasterError};

#[derive(Debug, Clone, PartialEq, Eq, Parser)]
#[command(
    name = "ringmaster",
    version,
    about = "Local-first Rust terminal app for exploring Oura Cloud data"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Launch the live terminal UI.
    Tui(TuiArgs),
    /// Print paths, config, storage, and health information.
    Doctor,
    /// Launch deterministic demo mode.
    Demo,
    /// Authentication commands.
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Sync commands.
    Sync {
        #[command(subcommand)]
        command: SyncCommand,
    },
    /// Webhook receiver, replay, and subscription management commands.
    Webhook {
        #[command(subcommand)]
        command: WebhookCommand,
    },
    /// Derived read-model and analytics commands.
    Derive {
        #[command(subcommand)]
        command: DeriveCommand,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum AuthCommand {
    /// Start or describe the OAuth login flow.
    Login,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum SyncCommand {
    /// Run one poll-first sync cycle.
    Once(SyncOnceArgs),
    /// Run the poll-first scheduler without the TUI.
    Watch(SyncWatchArgs),
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum DeriveCommand {
    /// Rebuild derived context events and pattern summaries from persisted data.
    Rebuild(DeriveRebuildArgs),
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum WebhookCommand {
    /// Run the Oura webhook receiver.
    Serve(WebhookServeArgs),
    /// Replay fixture-backed or stored webhook deliveries locally.
    Replay(WebhookReplayArgs),
    /// Declarative webhook subscription lifecycle commands.
    Subscriptions {
        #[command(subcommand)]
        command: WebhookSubscriptionCommand,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum WebhookSubscriptionCommand {
    /// List desired and remote webhook subscriptions.
    List(WebhookSubscriptionsListArgs),
    /// Converge remote webhook subscriptions toward desired local config.
    Sync(WebhookSubscriptionsSyncArgs),
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct TuiArgs {
    /// Launch the TUI with deterministic demo data.
    #[arg(long)]
    pub demo: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct SyncOnceArgs {
    /// Fetch and normalize data without mutating `SQLite`.
    #[arg(long)]
    pub dry_run: bool,
    /// Load Oura payloads from a fixture directory instead of the live API.
    #[arg(long)]
    pub fixture_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct SyncWatchArgs {
    /// Fetch and normalize data without mutating `SQLite`.
    #[arg(long)]
    pub dry_run: bool,
    /// Use deterministic fixture-backed sync behavior.
    #[arg(long)]
    pub demo: bool,
    /// Load Oura payloads from a fixture directory instead of the live API.
    #[arg(long)]
    pub fixture_dir: Option<PathBuf>,
    /// Stop after a bounded number of scheduler iterations.
    #[arg(long)]
    pub max_iterations: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct DeriveRebuildArgs {
    /// Seed a temporary demo store from fixture data before rebuilding derived state.
    #[arg(long)]
    pub demo: bool,
    /// Load Oura payloads from a fixture directory when seeding demo derivation.
    #[arg(long)]
    pub fixture_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args, Default)]
pub struct WebhookServeArgs {}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct WebhookReplayArgs {
    /// Replay a captured webhook request fixture from disk.
    #[arg(long)]
    pub fixture: Option<PathBuf>,
    /// Replay a previously accepted delivery by stored delivery id.
    #[arg(long)]
    pub delivery_id: Option<i64>,
    /// Replay the most recent accepted deliveries.
    #[arg(long)]
    pub recent: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct WebhookSubscriptionsListArgs {
    /// Load remote subscriptions from a fixture directory instead of the live API.
    #[arg(long)]
    pub fixture_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct WebhookSubscriptionsSyncArgs {
    /// Print the subscription diff without mutating the remote service.
    #[arg(long)]
    pub dry_run: bool,
    /// Delete remote subscriptions that are clearly out of the desired spec.
    #[arg(long)]
    pub prune: bool,
    /// Load remote subscriptions from a fixture directory instead of the live API.
    #[arg(long)]
    pub fixture_dir: Option<PathBuf>,
}

impl Cli {
    pub fn parse_from<I, T>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::try_parse_from(args).map_err(|error| RingmasterError::Cli(error.to_string()))
    }

    pub fn help_text() -> String {
        let mut command = Self::command();
        let mut buffer = Vec::new();
        match command.write_long_help(&mut buffer) {
            Ok(()) => String::from_utf8_lossy(&buffer).into_owned(),
            Err(_) => "ringmaster help is currently unavailable".to_owned(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::{
        AuthCommand, Cli, Command, DeriveCommand, DeriveRebuildArgs, SyncCommand, SyncOnceArgs,
        SyncWatchArgs, WebhookCommand, WebhookReplayArgs, WebhookSubscriptionCommand,
        WebhookSubscriptionsSyncArgs,
    };

    #[test]
    fn parses_nested_subcommands() {
        let cli = Cli::parse_from(["ringmaster", "auth", "login"]).unwrap_or_else(|error| {
            panic!("expected clap parsing to succeed in test: {error}");
        });

        match cli.command {
            Some(Command::Auth {
                command: AuthCommand::Login,
            }) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_sync_once() {
        let cli = Cli::parse_from(["ringmaster", "sync", "once"]).unwrap_or_else(|error| {
            panic!("expected clap parsing to succeed in test: {error}");
        });

        match cli.command {
            Some(Command::Sync {
                command:
                    SyncCommand::Once(SyncOnceArgs {
                        dry_run: false,
                        fixture_dir: None,
                    }),
            }) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn help_text_mentions_required_commands() {
        let help = Cli::help_text();

        for command in ["tui", "doctor", "auth", "sync", "webhook", "derive", "demo"] {
            assert!(
                help.contains(command),
                "help text should mention `{command}`"
            );
        }
    }

    #[test]
    fn parses_sync_watch_demo_args() {
        let cli = Cli::parse_from([
            "ringmaster",
            "sync",
            "watch",
            "--demo",
            "--max-iterations",
            "1",
        ])
        .unwrap_or_else(|error| {
            panic!("expected clap parsing to succeed in test: {error}");
        });

        match cli.command {
            Some(Command::Sync {
                command:
                    SyncCommand::Watch(SyncWatchArgs {
                        dry_run: false,
                        demo: true,
                        fixture_dir: None,
                        max_iterations: Some(1),
                    }),
            }) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_derive_rebuild_demo_args() {
        let cli = Cli::parse_from(["ringmaster", "derive", "rebuild", "--demo"]).unwrap_or_else(
            |error| {
                panic!("expected clap parsing to succeed in test: {error}");
            },
        );

        match cli.command {
            Some(Command::Derive {
                command:
                    DeriveCommand::Rebuild(DeriveRebuildArgs {
                        demo: true,
                        fixture_dir: None,
                    }),
            }) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_webhook_replay_fixture_args() {
        let cli = Cli::parse_from([
            "ringmaster",
            "webhook",
            "replay",
            "--fixture",
            "tests/fixtures/webhooks/sample.json",
        ])
        .unwrap_or_else(|error| {
            panic!("expected clap parsing to succeed in test: {error}");
        });

        match cli.command {
            Some(Command::Webhook {
                command:
                    WebhookCommand::Replay(WebhookReplayArgs {
                        fixture: Some(path),
                        delivery_id: None,
                        recent: None,
                    }),
            }) => assert!(path.ends_with("tests/fixtures/webhooks/sample.json")),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_webhook_subscriptions_sync_args() {
        let cli = Cli::parse_from([
            "ringmaster",
            "webhook",
            "subscriptions",
            "sync",
            "--dry-run",
            "--prune",
        ])
        .unwrap_or_else(|error| {
            panic!("expected clap parsing to succeed in test: {error}");
        });

        match cli.command {
            Some(Command::Webhook {
                command:
                    WebhookCommand::Subscriptions {
                        command:
                            WebhookSubscriptionCommand::Sync(WebhookSubscriptionsSyncArgs {
                                dry_run: true,
                                prune: true,
                                fixture_dir: None,
                            }),
                    },
            }) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
