#![allow(clippy::panic)]

use ringmaster::cli::{AuthCommand, Cli, Command};

#[test]
fn parses_no_command_as_none() {
    let result = Cli::parse_from(["ringmaster"]);
    assert!(result.is_ok(), "cli should parse without a subcommand");

    let cli = match result {
        Ok(cli) => cli,
        Err(error) => panic!("unexpected parse failure: {error}"),
    };
    assert_eq!(cli.command, None);
}

#[test]
fn parses_doctor_command() {
    let result = Cli::parse_from(["ringmaster", "doctor"]);
    assert!(result.is_ok(), "cli should parse doctor");

    let cli = match result {
        Ok(cli) => cli,
        Err(error) => panic!("unexpected parse failure: {error}"),
    };
    assert_eq!(cli.command, Some(Command::Doctor));
}

#[test]
fn parses_nested_auth_login_command() {
    let result = Cli::parse_from(["ringmaster", "auth", "login"]);
    assert!(result.is_ok(), "cli should parse nested auth command");

    let cli = match result {
        Ok(cli) => cli,
        Err(error) => panic!("unexpected parse failure: {error}"),
    };
    assert_eq!(
        cli.command,
        Some(Command::Auth {
            command: AuthCommand::Login
        })
    );
}

#[tokio::test]
async fn demo_output_mentions_dashboard() {
    let result = ringmaster::run_from(["ringmaster", "demo"]).await;
    assert!(result.is_ok(), "demo should run");

    let output = match result {
        Ok(Some(output)) => output,
        Ok(None) => panic!("non-interactive demo should render a snapshot"),
        Err(error) => panic!("unexpected demo failure: {error}"),
    };

    assert!(output.contains("ringmaster.rs"));
    assert!(output.contains("Selected day: 2026-04-08"));
    assert!(output.contains("Capabilities"));
    assert!(output.contains("What Changed"));
    assert!(output.contains("sleep is below normal"));
}
