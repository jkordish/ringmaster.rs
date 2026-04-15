use tempfile::tempdir;

use ringmaster::cli::{AuthCommand, Cli, Command};

fn ok<T, E>(result: Result<T, E>, context: &str) -> T
where
    E: std::fmt::Display,
{
    match result {
        Ok(value) => value,
        Err(error) => unreachable!("{context}: {error}"),
    }
}

fn some<T>(option: Option<T>, context: &str) -> T {
    match option {
        Some(value) => value,
        None => unreachable!("{context}"),
    }
}

#[test]
fn parses_no_command_as_none() {
    let result = Cli::parse_from(["ringmaster"]);
    assert!(result.is_ok(), "cli should parse without a subcommand");

    let cli = ok(result, "unexpected parse failure");
    assert_eq!(cli.command, None);
}

#[test]
fn parses_doctor_command() {
    let result = Cli::parse_from(["ringmaster", "doctor"]);
    assert!(result.is_ok(), "cli should parse doctor");

    let cli = ok(result, "unexpected parse failure");
    assert_eq!(cli.command, Some(Command::Doctor));
}

#[test]
fn parses_nested_auth_login_command() {
    let result = Cli::parse_from(["ringmaster", "auth", "login"]);
    assert!(result.is_ok(), "cli should parse nested auth command");

    let cli = ok(result, "unexpected parse failure");
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

    let output = some(
        ok(result, "unexpected demo failure"),
        "non-interactive demo should render a snapshot",
    );

    assert!(output.contains("ringmaster"));
    assert!(output.contains("Conn on"));
    assert!(output.contains("READINESS"));
    assert!(output.contains("READINESS BREAKDOWN"));
    assert!(output.contains("WEEKLY TRENDS"));
    assert!(output.contains("Readiness tile | score 74"));
    assert!(output.contains("Review"));
}

#[tokio::test]
async fn ui_snapshot_demo_writes_artifacts() {
    let out_dir = ok(tempdir(), "tempdir should build");
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

    let output = some(
        ok(result, "unexpected ui snapshot failure"),
        "ui snapshot should render command output",
    );

    assert!(output.contains("ringmaster ui snapshot"));
    assert!(output.contains("dashboard"));
    assert!(output.contains("status"));
    assert!(out_path.join("dashboard-compact.txt").exists());
    assert!(out_path.join("status-wide.txt").exists());
}

#[tokio::test]
async fn ui_snapshot_demo_writes_ansi_sidecars_when_requested() {
    let out_dir = ok(tempdir(), "tempdir should build");
    let out_path = out_dir.path().join("color-snapshots");
    let out_arg = out_path.to_string_lossy().into_owned();

    let result = ringmaster::run_from([
        "ringmaster",
        "ui",
        "snapshot",
        "--demo",
        "--screen",
        "dashboard",
        "--size",
        "compact",
        "--ansi-sidecar",
        "--color-mode",
        "truecolor",
        "--color-mode",
        "mono",
        "--out-dir",
        &out_arg,
    ])
    .await;
    assert!(result.is_ok(), "ansi ui snapshot should run");

    let output = some(
        ok(result, "unexpected ansi ui snapshot failure"),
        "ansi ui snapshot should render command output",
    );

    assert!(output.contains("ansi_sidecars: truecolor, mono"));
    assert!(out_path.join("dashboard-compact.txt").exists());
    assert!(out_path.join("dashboard-compact-truecolor.ansi").exists());
    assert!(out_path.join("dashboard-compact-mono.ansi").exists());
}

#[tokio::test]
async fn ui_snapshot_ai_demo_writes_ai_workbench_artifacts() {
    let out_dir = ok(tempdir(), "tempdir should build");
    let out_path = out_dir.path().join("ai-snapshots");
    let out_arg = out_path.to_string_lossy().into_owned();

    let result = ringmaster::run_from([
        "ringmaster",
        "ui",
        "snapshot",
        "--demo",
        "--screen",
        "ai",
        "--size",
        "compact",
        "--size",
        "wide",
        "--out-dir",
        &out_arg,
    ])
    .await;
    assert!(result.is_ok(), "ai ui snapshot should run");

    let output = some(
        ok(result, "unexpected ai ui snapshot failure"),
        "ai ui snapshot should render command output",
    );

    assert!(output.contains("ringmaster ui snapshot"));
    assert!(output.contains("ai"));
    assert!(out_path.join("ai-compact.txt").exists());
    assert!(out_path.join("ai-wide.txt").exists());
}

#[tokio::test]
async fn ui_snapshot_demo_writes_telemetry_screen_artifacts() {
    let out_dir = ok(tempdir(), "tempdir should build");
    let out_path = out_dir.path().join("telemetry-snapshots");
    let out_arg = out_path.to_string_lossy().into_owned();

    let result = ringmaster::run_from([
        "ringmaster",
        "ui",
        "snapshot",
        "--demo",
        "--screen",
        "explain",
        "--screen",
        "patterns",
        "--screen",
        "review",
        "--size",
        "compact",
        "--size",
        "wide",
        "--out-dir",
        &out_arg,
    ])
    .await;
    assert!(result.is_ok(), "telemetry ui snapshot should run");

    let output = some(
        ok(result, "unexpected telemetry ui snapshot failure"),
        "telemetry ui snapshot should render command output",
    );

    assert!(output.contains("explain"));
    assert!(output.contains("patterns"));
    assert!(output.contains("review"));
    assert!(out_path.join("explain-compact.txt").exists());
    assert!(out_path.join("patterns-wide.txt").exists());
    assert!(out_path.join("review-compact.txt").exists());
}

#[tokio::test]
async fn ui_snapshot_scenario_fixture_root_writes_scenario_tagged_artifacts() {
    let out_dir = ok(tempdir(), "tempdir should build");
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

    let output = some(
        ok(result, "unexpected scenario fixture ui snapshot failure"),
        "scenario fixture ui snapshot should render command output",
    );

    assert!(output.contains("scenario fixture root"));
    assert!(output.contains("dense-history"));
    assert!(output.contains("strong, weak, empty"));
    assert!(out_path.join("dashboard-strong-compact.txt").exists());
    assert!(out_path.join("dashboard-dense-history-wide.txt").exists());
    assert!(out_path.join("dashboard-error-wide.txt").exists());
    assert!(out_path.join("status-stale-compact.txt").exists());
}

#[tokio::test]
async fn review_today_demo_renders_ranked_output() {
    let result = ringmaster::run_from(["ringmaster", "review", "today", "--demo"]).await;
    assert!(result.is_ok(), "review today demo should run");

    let output = some(
        ok(result, "unexpected review today failure"),
        "review today demo should render text output",
    );

    assert!(output.contains("ringmaster review today"));
    assert!(output.contains("top_observations:"));
    assert!(output.contains("evidence:"));
}

#[tokio::test]
async fn review_week_demo_renders_ranked_output() {
    let result = ringmaster::run_from(["ringmaster", "review", "week", "--demo"]).await;
    assert!(result.is_ok(), "review week demo should run");

    let output = some(
        ok(result, "unexpected review week failure"),
        "review week demo should render text output",
    );

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

#[tokio::test]
async fn snapshot_list_demo_renders_catalog_output() {
    let result = ringmaster::run_from(["ringmaster", "snapshot", "list", "--demo"]).await;
    assert!(result.is_ok(), "snapshot list demo should run");

    let output = match result {
        Ok(Some(output)) => output,
        Ok(None) => panic!("snapshot list demo should render text output"),
        Err(error) => panic!("unexpected snapshot list failure: {error}"),
    };

    assert!(output.contains("ringmaster snapshot list"));
    assert!(output.contains("snapshots:"));
    assert!(output.contains("profile=redacted"));
}

#[tokio::test]
async fn snapshot_show_path_renders_snapshot_detail() {
    let out_dir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
    let snapshot_path = out_dir.path().join("snapshot.json");
    let snapshot_arg = snapshot_path.to_string_lossy().into_owned();

    let export_result = ringmaster::run_from([
        "ringmaster",
        "snapshot",
        "export",
        "--demo",
        "--profile",
        "redacted",
        "--out",
        &snapshot_arg,
    ])
    .await;
    assert!(export_result.is_ok(), "snapshot export should run");

    let result = ringmaster::run_from(["ringmaster", "snapshot", "show", &snapshot_arg]).await;
    assert!(result.is_ok(), "snapshot show should run");

    let output = match result {
        Ok(Some(output)) => output,
        Ok(None) => panic!("snapshot show should render text output"),
        Err(error) => panic!("unexpected snapshot show failure: {error}"),
    };

    assert!(output.contains("ringmaster snapshot show"));
    assert!(output.contains("source: file:"));
    assert!(output.contains("snapshot_hash:"));
}

#[tokio::test]
async fn ai_runs_list_demo_renders_registry_output() {
    let result = ringmaster::run_from(["ringmaster", "ai", "runs", "list", "--demo"]).await;
    assert!(result.is_ok(), "ai runs list demo should run");

    let output = match result {
        Ok(Some(output)) => output,
        Ok(None) => panic!("ai runs list demo should render text output"),
        Err(error) => panic!("unexpected ai runs list failure: {error}"),
    };

    assert!(output.contains("ringmaster ai runs list"));
    assert!(output.contains("runs:"));
    assert!(output.contains("kind=review"));
}

#[tokio::test]
async fn ai_runs_show_demo_accepts_id_listed_by_previous_demo_invocation() {
    let list_result = ringmaster::run_from(["ringmaster", "ai", "runs", "list", "--demo"]).await;
    assert!(list_result.is_ok(), "ai runs list demo should run");

    let list_output = match list_result {
        Ok(Some(output)) => output,
        Ok(None) => panic!("ai runs list demo should render text output"),
        Err(error) => panic!("unexpected ai runs list failure: {error}"),
    };
    let listed_id = list_output
        .lines()
        .find_map(|line| {
            line.trim_start()
                .strip_prefix("- ")
                .and_then(|value| value.split(" | ").next())
        })
        .unwrap_or_else(|| panic!("expected at least one demo AI run id in list output"));

    let show_result =
        ringmaster::run_from(["ringmaster", "ai", "runs", "show", "--demo", listed_id]).await;
    assert!(
        show_result.is_ok(),
        "ai runs show demo should resolve an id listed by a previous invocation"
    );

    let show_output = match show_result {
        Ok(Some(output)) => output,
        Ok(None) => panic!("ai runs show demo should render text output"),
        Err(error) => panic!("unexpected ai runs show failure: {error}"),
    };

    assert!(show_output.contains("ringmaster ai runs show"));
    assert!(show_output.contains("artifact_id:"));
}

#[tokio::test]
async fn report_export_from_snapshot_writes_markdown_report() {
    let out_dir = tempdir().unwrap_or_else(|error| panic!("tempdir should build: {error}"));
    let snapshot_path = out_dir.path().join("snapshot.json");
    let snapshot_arg = snapshot_path.to_string_lossy().into_owned();
    let report_path = out_dir.path().join("report.md");
    let report_arg = report_path.to_string_lossy().into_owned();

    let export_result = ringmaster::run_from([
        "ringmaster",
        "snapshot",
        "export",
        "--demo",
        "--profile",
        "redacted",
        "--out",
        &snapshot_arg,
    ])
    .await;
    assert!(export_result.is_ok(), "snapshot export should run");

    let result = ringmaster::run_from([
        "ringmaster",
        "report",
        "export",
        "--from-snapshot",
        &snapshot_arg,
        "--format",
        "markdown",
        "--out",
        &report_arg,
    ])
    .await;
    assert!(result.is_ok(), "report export should run");

    let output = match result {
        Ok(Some(output)) => output,
        Ok(None) => panic!("report export should render text output"),
        Err(error) => panic!("unexpected report export failure: {error}"),
    };

    let report_text = std::fs::read_to_string(&report_path)
        .unwrap_or_else(|error| panic!("report should read: {error}"));
    assert!(output.contains("ringmaster report export"));
    assert!(output.contains("format: markdown"));
    assert!(report_text.contains("# Snapshot report:"));
}

#[tokio::test]
async fn ai_eval_fixture_dir_renders_summary() {
    let result = ringmaster::run_from([
        "ringmaster",
        "ai",
        "eval",
        "--fixture-dir",
        "tests/fixtures/ai",
    ])
    .await;
    assert!(result.is_ok(), "ai eval should run");

    let output = match result {
        Ok(Some(output)) => output,
        Ok(None) => panic!("ai eval should render text output"),
        Err(error) => panic!("unexpected ai eval failure: {error}"),
    };

    assert!(output.contains("ringmaster ai eval"));
    assert!(output.contains("candidate_label: candidate"));
    assert!(output.contains("baseline_label: baseline"));
    assert!(output.contains("scores:"));
}
