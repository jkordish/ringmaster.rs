use clap::{CommandFactory, Parser, Subcommand};

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
    Tui,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum AuthCommand {
    /// Start or describe the OAuth login flow.
    Login,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum SyncCommand {
    /// Run one poll-first sync cycle.
    Once,
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
    use super::{AuthCommand, Cli, Command, SyncCommand};

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
                command: SyncCommand::Once,
            }) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn help_text_mentions_required_commands() {
        let help = Cli::help_text();

        for command in ["tui", "doctor", "auth", "sync", "demo"] {
            assert!(
                help.contains(command),
                "help text should mention `{command}`"
            );
        }
    }
}
