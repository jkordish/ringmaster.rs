use crate::error::{Result, RingmasterError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cli {
    pub command: Command,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Tui,
    Demo,
    Doctor,
    AuthLogin,
    SyncOnce,
}

impl Cli {
    pub fn parse<I, S>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let collected: Vec<String> = args.into_iter().map(Into::into).collect();

        let mut parts = collected.iter().skip(1).map(String::as_str);

        let command = match parts.next() {
            None => Command::Help,
            Some("help" | "-h" | "--help") => Command::Help,
            Some("tui") => Command::Tui,
            Some("demo") => Command::Demo,
            Some("doctor") => Command::Doctor,
            Some("auth") => match parts.next() {
                Some("login") => Command::AuthLogin,
                Some(other) => {
                    return Err(RingmasterError::Usage(format!(
                        "unknown auth subcommand: {other}\n\n{}",
                        help_text()
                    )));
                }
                None => {
                    return Err(RingmasterError::Usage(format!(
                        "missing auth subcommand\n\n{}",
                        help_text()
                    )));
                }
            },
            Some("sync") => match parts.next() {
                Some("once") => Command::SyncOnce,
                Some(other) => {
                    return Err(RingmasterError::Usage(format!(
                        "unknown sync subcommand: {other}\n\n{}",
                        help_text()
                    )));
                }
                None => {
                    return Err(RingmasterError::Usage(format!(
                        "missing sync subcommand\n\n{}",
                        help_text()
                    )));
                }
            },
            Some(other) => {
                return Err(RingmasterError::Usage(format!(
                    "unknown command: {other}\n\n{}",
                    help_text()
                )));
            }
        };

        Ok(Self { command })
    }
}

pub fn help_text() -> String {
    let text = r#"ringmaster.rs

Usage:
  ringmaster <command>

Commands:
  tui           Launch the terminal UI (placeholder shell today)
  demo          Launch deterministic demo output
  doctor        Print environment and project health information
  auth login    Start or describe the OAuth login flow
  sync once     Run one sync cycle (scaffold)
  help          Show this help text
"#;

    text.to_owned()
}
