use ringmaster::cli::{Cli, Command};

#[test]
fn parses_help_when_no_command_is_given() {
    let cli = Cli::parse(["ringmaster"]).expect("cli should parse");
    assert_eq!(cli.command, Command::Help);
}

#[test]
fn parses_doctor_command() {
    let cli = Cli::parse(["ringmaster", "doctor"]).expect("cli should parse");
    assert_eq!(cli.command, Command::Doctor);
}

#[test]
fn parses_nested_auth_login_command() {
    let cli = Cli::parse(["ringmaster", "auth", "login"]).expect("cli should parse");
    assert_eq!(cli.command, Command::AuthLogin);
}

#[test]
fn demo_output_mentions_dashboard() {
    let output = ringmaster::run_from(["ringmaster", "demo"]).expect("demo should run");
    assert!(output.contains("[Dashboard]"));
    assert!(output.contains("[Trends]"));
}
