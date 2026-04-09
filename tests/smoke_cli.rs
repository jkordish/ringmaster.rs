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

    assert!(output.contains("ringmaster"));
    assert!(output.contains("Selected day: 2026-04-08"));
    assert!(output.contains("Capabilities"));
    assert!(output.contains("What Changed"));
    assert!(output.contains("Review:"));
    assert!(output.contains("Stress high time is lower than usual."));
}

#[tokio::test]
async fn review_today_demo_renders_ranked_output() {
    let result = ringmaster::run_from(["ringmaster", "review", "today", "--demo"]).await;
    assert!(result.is_ok(), "review today demo should run");

    let output = match result {
        Ok(Some(output)) => output,
        Ok(None) => panic!("review today demo should render text output"),
        Err(error) => panic!("unexpected review today failure: {error}"),
    };

    assert!(output.contains("ringmaster review today"));
    assert!(output.contains("top_observations:"));
    assert!(output.contains("evidence:"));
}

#[tokio::test]
async fn review_week_demo_renders_ranked_output() {
    let result = ringmaster::run_from(["ringmaster", "review", "week", "--demo"]).await;
    assert!(result.is_ok(), "review week demo should run");

    let output = match result {
        Ok(Some(output)) => output,
        Ok(None) => panic!("review week demo should render text output"),
        Err(error) => panic!("unexpected review week failure: {error}"),
    };

    assert!(output.contains("ringmaster review week"));
    assert!(output.contains("top_observations:"));
}

#[tokio::test]
async fn review_investigate_demo_renders_bounded_focus_output() {
    let result = ringmaster::run_from([
        "ringmaster",
        "review",
        "investigate",
        "--focus",
        "readiness",
        "--demo",
    ])
    .await;
    assert!(result.is_ok(), "review investigate demo should run");

    let output = match result {
        Ok(Some(output)) => output,
        Ok(None) => panic!("review investigate demo should render text output"),
        Err(error) => panic!("unexpected review investigate failure: {error}"),
    };

    assert!(output.contains("ringmaster review investigate"));
    assert!(output.contains("focus: readiness"));
    assert!(output.contains("look_at:"));
}
