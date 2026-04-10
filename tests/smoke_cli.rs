#![allow(clippy::panic)]

use tempfile::tempdir;

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
    assert!(output.contains("Connection: Connected"));
    assert!(output.contains("Latest sync:"));
    assert!(output.contains("What matters now | 2026-04-08"));
    assert!(output.contains("Capabilities"));
    assert!(output.contains("Drill-down cues"));
    assert!(output.contains("Review"));
    assert!(output.contains("Stress high time is higher than usual."));
}

#[tokio::test]
async fn ui_snapshot_demo_writes_artifacts() {
    let out_dir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
    let out_path = out_dir.path().join("snapshots");
    let out_arg = out_path.to_string_lossy().into_owned();

    let result = ringmaster::run_from([
        "ringmaster",
        "ui",
        "snapshot",
        "--demo",
        "--screen",
        "dashboard",
        "--screen",
        "status",
        "--size",
        "compact",
        "--size",
        "wide",
        "--out-dir",
        &out_arg,
    ])
    .await;
    assert!(result.is_ok(), "ui snapshot should run");

    let output = match result {
        Ok(Some(output)) => output,
        Ok(None) => panic!("ui snapshot should render command output"),
        Err(error) => panic!("unexpected ui snapshot failure: {error}"),
    };

    assert!(output.contains("ringmaster ui snapshot"));
    assert!(output.contains("dashboard"));
    assert!(output.contains("status"));
    assert!(out_path.join("dashboard-compact.txt").exists());
    assert!(out_path.join("status-wide.txt").exists());
}

#[tokio::test]
async fn ui_snapshot_scenario_fixture_root_writes_scenario_tagged_artifacts() {
    let out_dir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
    let out_path = out_dir.path().join("phase7");
    let out_arg = out_path.to_string_lossy().into_owned();

    let result = ringmaster::run_from([
        "ringmaster",
        "ui",
        "snapshot",
        "--fixture-dir",
        "tests/fixtures/phase7",
        "--screen",
        "dashboard",
        "--screen",
        "status",
        "--size",
        "compact",
        "--size",
        "wide",
        "--out-dir",
        &out_arg,
    ])
    .await;
    assert!(result.is_ok(), "scenario fixture ui snapshot should run");

    let output = match result {
        Ok(Some(output)) => output,
        Ok(None) => panic!("scenario fixture ui snapshot should render command output"),
        Err(error) => panic!("unexpected scenario fixture ui snapshot failure: {error}"),
    };

    assert!(output.contains("scenario fixture root"));
    assert!(output.contains("strong, weak, empty, stale, error"));
    assert!(out_path.join("dashboard-strong-compact.txt").exists());
    assert!(out_path.join("dashboard-error-wide.txt").exists());
    assert!(out_path.join("status-stale-compact.txt").exists());
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
