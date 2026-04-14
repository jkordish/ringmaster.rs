use std::path::PathBuf;

use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};

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
    /// Snapshot export commands.
    Snapshot {
        #[command(subcommand)]
        command: SnapshotCommand,
    },
    /// Non-interactive UI tooling.
    Ui {
        #[command(subcommand)]
        command: UiCommand,
    },
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
    /// Deterministic review and investigation commands.
    Review {
        #[command(subcommand)]
        command: ReviewCommand,
    },
    /// Snapshot-based AI review and compare commands.
    Ai {
        #[command(subcommand)]
        command: AiCommand,
    },
    /// Report export commands.
    Report {
        #[command(subcommand)]
        command: ReportCommand,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum AuthCommand {
    /// Start or describe the OAuth login flow.
    Login,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum UiCommand {
    /// Render deterministic screen snapshots for visual QA.
    Snapshot(UiSnapshotArgs),
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum SnapshotCommand {
    /// Export a versioned snapshot artifact for local inspection or AI analysis.
    Export(SnapshotExportArgs),
    /// List saved snapshot artifacts from the local catalog.
    List(SnapshotListArgs),
    /// Show one saved snapshot artifact or a snapshot JSON file.
    Show(SnapshotShowArgs),
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
pub enum ReviewCommand {
    /// Print a ranked daily brief for the selected or latest day.
    Today(ReviewTodayArgs),
    /// Print a ranked weekly review ending on the selected or latest day.
    Week(ReviewWeekArgs),
    /// Run a bounded evidence-backed investigation for a fixed focus.
    Investigate(ReviewInvestigateArgs),
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum AiCommand {
    /// Run a structured AI review against a local snapshot artifact.
    Review(AiReviewArgs),
    /// Run a structured AI comparison between two local snapshot artifacts.
    Compare(AiCompareArgs),
    /// Browse persisted AI runs.
    Runs {
        #[command(subcommand)]
        command: AiRunsCommand,
    },
    /// Run deterministic local evaluations for snapshot-based AI behavior.
    Eval(AiEvalArgs),
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum AiRunsCommand {
    /// List saved AI runs from the local registry.
    List(AiRunsListArgs),
    /// Show one saved AI run.
    Show(AiRunsShowArgs),
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum ReportCommand {
    /// Export a human-readable report from a snapshot or AI run.
    Export(ReportExportArgs),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SnapshotScreenArg {
    Dashboard,
    Timeline,
    Trends,
    Explain,
    Patterns,
    Review,
    Ai,
    #[value(alias = "ops")]
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SnapshotSizeArg {
    Compact,
    Medium,
    Wide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SnapshotColorModeArg {
    Current,
    Truecolor,
    Ansi256,
    Ansi16,
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PrivacyProfileArg {
    Redacted,
    Balanced,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ReportFormatArg {
    Markdown,
    Html,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct UiSnapshotArgs {
    /// Use deterministic demo-mode screen data.
    #[arg(long)]
    pub demo: bool,
    /// Seed a temporary local store from fixture payloads before rendering snapshots.
    #[arg(long)]
    pub fixture_dir: Option<PathBuf>,
    /// Limit snapshot generation to specific screens. Defaults to all primary screens.
    #[arg(long, value_enum)]
    pub screen: Vec<SnapshotScreenArg>,
    /// Limit snapshot generation to specific terminal sizes. Defaults to compact, medium, and wide.
    #[arg(long, value_enum)]
    pub size: Vec<SnapshotSizeArg>,
    /// Emit ANSI sidecar artifacts alongside stable text snapshots.
    #[arg(long)]
    pub ansi_sidecar: bool,
    /// Color modes to export for ANSI sidecars. Defaults to `current` and `mono`; prefer explicit `truecolor` plus `mono` for regression QA.
    #[arg(long, value_enum)]
    pub color_mode: Vec<SnapshotColorModeArg>,
    /// Output directory for deterministic snapshot artifacts.
    #[arg(long)]
    pub out_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct SnapshotExportArgs {
    /// Export a deterministic fixture-backed snapshot instead of using the live store.
    #[arg(long)]
    pub demo: bool,
    /// Load Oura payloads from a fixture directory when exporting a demo snapshot.
    #[arg(long)]
    pub fixture_dir: Option<PathBuf>,
    /// Snapshot scope selector such as `today`, `week`, `day:2026-04-08`, or `range:2026-04-01..2026-04-07`.
    #[arg(long, default_value = "today")]
    pub scope: String,
    /// Privacy profile to apply to the exported artifact.
    #[arg(long, value_enum, default_value_t = PrivacyProfileArg::Redacted)]
    pub profile: PrivacyProfileArg,
    /// Output path for the exported snapshot JSON.
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Emit compact JSON instead of pretty JSON.
    #[arg(long)]
    pub compact: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct SnapshotListArgs {
    /// List deterministic demo snapshots instead of reading the live local catalog.
    #[arg(long)]
    pub demo: bool,
    /// Load Oura payloads from a fixture directory when seeding demo snapshots.
    #[arg(long)]
    pub fixture_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct SnapshotShowArgs {
    /// Snapshot hash from the local catalog, or a path to a local snapshot JSON artifact.
    pub snapshot: String,
    /// Resolve demo snapshots instead of reading the live local catalog.
    #[arg(long)]
    pub demo: bool,
    /// Load Oura payloads from a fixture directory when seeding demo snapshots.
    #[arg(long)]
    pub fixture_dir: Option<PathBuf>,
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

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ReviewTodayArgs {
    /// Review a specific closeout day instead of the latest available day.
    #[arg(long)]
    pub day: Option<String>,
    /// Render JSON instead of terminal text.
    #[arg(long)]
    pub json: bool,
    /// Seed a temporary review store from deterministic fixture data.
    #[arg(long)]
    pub demo: bool,
    /// Load Oura payloads from a fixture directory when running in demo mode.
    #[arg(long)]
    pub fixture_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ReviewWeekArgs {
    /// End the weekly review on a specific closeout day instead of the latest available day.
    #[arg(long)]
    pub end_day: Option<String>,
    /// Render JSON instead of terminal text.
    #[arg(long)]
    pub json: bool,
    /// Seed a temporary review store from deterministic fixture data.
    #[arg(long)]
    pub demo: bool,
    /// Load Oura payloads from a fixture directory when running in demo mode.
    #[arg(long)]
    pub fixture_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ReviewFocusArg {
    Readiness,
    Sleep,
    Recovery,
    Stress,
    Activity,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ReviewInvestigateArgs {
    /// Investigation focus. This pass supports only bounded deterministic focuses.
    #[arg(long, value_enum)]
    pub focus: ReviewFocusArg,
    /// Anchor the investigation on a specific closeout day instead of the latest available day.
    #[arg(long)]
    pub anchor_day: Option<String>,
    /// Render JSON instead of terminal text.
    #[arg(long)]
    pub json: bool,
    /// Seed a temporary review store from deterministic fixture data.
    #[arg(long)]
    pub demo: bool,
    /// Load Oura payloads from a fixture directory when running in demo mode.
    #[arg(long)]
    pub fixture_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct AiReviewArgs {
    /// Path to a local snapshot JSON artifact.
    pub snapshot_path: PathBuf,
    /// Skip remote API calls and render a deterministic dry-run artifact instead.
    #[arg(long)]
    pub dry_run: bool,
    /// Load the AI result from a fixture JSON artifact instead of a live provider.
    #[arg(long)]
    pub fixture: Option<PathBuf>,
    /// Optional output path for the persisted AI artifact JSON.
    #[arg(long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct AiCompareArgs {
    /// Path to the base snapshot JSON artifact.
    pub snapshot_a: PathBuf,
    /// Path to the comparison snapshot JSON artifact.
    pub snapshot_b: PathBuf,
    /// Skip remote API calls and render a deterministic dry-run artifact instead.
    #[arg(long)]
    pub dry_run: bool,
    /// Load the AI result from a fixture JSON artifact instead of a live provider.
    #[arg(long)]
    pub fixture: Option<PathBuf>,
    /// Optional output path for the persisted AI artifact JSON.
    #[arg(long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct AiRunsListArgs {
    /// List deterministic demo AI runs instead of reading the live local registry.
    #[arg(long)]
    pub demo: bool,
    /// Load Oura payloads from a fixture directory when seeding demo AI runs.
    #[arg(long)]
    pub fixture_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct AiRunsShowArgs {
    /// Saved AI run id.
    pub run_id: String,
    /// Resolve demo AI runs instead of reading the live local registry.
    #[arg(long)]
    pub demo: bool,
    /// Load Oura payloads from a fixture directory when seeding demo AI runs.
    #[arg(long)]
    pub fixture_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ReportExportArgs {
    /// Export a report directly from a snapshot hash or local snapshot artifact path.
    #[arg(long)]
    pub from_snapshot: Option<String>,
    /// Export a report from a saved AI artifact id or unique prefix from the local AI registry.
    #[arg(long)]
    pub from_ai_run: Option<String>,
    /// Report format to render.
    #[arg(long, value_enum)]
    pub format: ReportFormatArg,
    /// Output path for the rendered report.
    #[arg(long)]
    pub out: PathBuf,
    /// Resolve snapshot or AI run inputs against deterministic demo data.
    #[arg(long)]
    pub demo: bool,
    /// Load Oura payloads from a fixture directory when seeding demo artifacts.
    #[arg(long)]
    pub fixture_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct AiEvalArgs {
    /// Fixture directory containing local eval cases.
    #[arg(long)]
    pub fixture_dir: PathBuf,
    /// Candidate label to annotate in persisted eval summaries.
    #[arg(long)]
    pub candidate: Option<String>,
    /// Baseline label to compare against in the eval summary.
    #[arg(long)]
    pub baseline: Option<String>,
    /// Optional path for writing the detailed eval result JSON.
    #[arg(long)]
    pub export: Option<PathBuf>,
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
    /// Parses command-line arguments into the typed CLI model.
    ///
    /// # Errors
    ///
    /// Returns a CLI error string when clap rejects the provided argument list.
    pub fn parse_from<I, T>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::try_parse_from(args).map_err(|error| RingmasterError::Cli(error.to_string()))
    }

    #[must_use]
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
mod tests {
    use std::path::PathBuf;

    use super::{
        AiCommand, AiCompareArgs, AiEvalArgs, AiReviewArgs, AiRunsCommand, AiRunsListArgs,
        AiRunsShowArgs, AuthCommand, Cli, Command, DeriveCommand, DeriveRebuildArgs,
        PrivacyProfileArg, ReportCommand, ReportExportArgs, ReportFormatArg, ReviewCommand,
        ReviewFocusArg, ReviewInvestigateArgs, ReviewTodayArgs, SnapshotColorModeArg,
        SnapshotCommand, SnapshotExportArgs, SnapshotListArgs, SnapshotScreenArg, SnapshotShowArgs,
        SnapshotSizeArg, SyncCommand, SyncOnceArgs, SyncWatchArgs, UiCommand, UiSnapshotArgs,
        WebhookCommand, WebhookReplayArgs, WebhookSubscriptionCommand,
        WebhookSubscriptionsSyncArgs,
    };
    use crate::test_support::ok;

    #[test]
    fn parses_nested_subcommands() {
        let cli = ok(
            Cli::parse_from(["ringmaster", "auth", "login"]),
            "expected clap parsing to succeed in test",
        );

        match cli.command {
            Some(Command::Auth {
                command: AuthCommand::Login,
            }) => {}
            other => unreachable!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_sync_once() {
        let cli = ok(
            Cli::parse_from(["ringmaster", "sync", "once"]),
            "expected clap parsing to succeed in test",
        );

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

        for command in [
            "tui", "snapshot", "ui", "doctor", "auth", "sync", "webhook", "derive", "review", "ai",
            "demo",
        ] {
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

    #[test]
    fn parses_review_investigate_focus() {
        let cli = Cli::parse_from([
            "ringmaster",
            "review",
            "investigate",
            "--focus",
            "readiness",
            "--demo",
        ])
        .unwrap_or_else(|error| {
            panic!("expected clap parsing to succeed in test: {error}");
        });

        match cli.command {
            Some(Command::Review {
                command:
                    ReviewCommand::Investigate(ReviewInvestigateArgs {
                        focus: ReviewFocusArg::Readiness,
                        anchor_day: None,
                        json: false,
                        demo: true,
                        fixture_dir: None,
                    }),
            }) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_review_today_json() {
        let cli =
            Cli::parse_from(["ringmaster", "review", "today", "--json"]).unwrap_or_else(|error| {
                panic!("expected clap parsing to succeed in test: {error}");
            });

        match cli.command {
            Some(Command::Review {
                command:
                    ReviewCommand::Today(ReviewTodayArgs {
                        day: None,
                        json: true,
                        demo: false,
                        fixture_dir: None,
                    }),
            }) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_ui_snapshot_args() {
        let cli = Cli::parse_from([
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
            "/tmp/ringmaster-snapshots",
        ])
        .unwrap_or_else(|error| {
            panic!("expected clap parsing to succeed in test: {error}");
        });

        match cli.command {
            Some(Command::Ui {
                command:
                    UiCommand::Snapshot(UiSnapshotArgs {
                        demo: true,
                        fixture_dir: None,
                        screen,
                        size,
                        ansi_sidecar: false,
                        color_mode,
                        out_dir,
                    }),
            }) => {
                assert_eq!(
                    screen,
                    vec![SnapshotScreenArg::Dashboard, SnapshotScreenArg::Status]
                );
                assert_eq!(size, vec![SnapshotSizeArg::Compact, SnapshotSizeArg::Wide]);
                assert!(color_mode.is_empty());
                assert_eq!(out_dir, PathBuf::from("/tmp/ringmaster-snapshots"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_ui_snapshot_ansi_sidecar_modes() {
        let cli = Cli::parse_from([
            "ringmaster",
            "ui",
            "snapshot",
            "--ansi-sidecar",
            "--color-mode",
            "current",
            "--color-mode",
            "mono",
            "--out-dir",
            "/tmp/ringmaster-snapshots",
        ])
        .unwrap_or_else(|error| {
            panic!("expected clap parsing to succeed in test: {error}");
        });

        match cli.command {
            Some(Command::Ui {
                command:
                    UiCommand::Snapshot(UiSnapshotArgs {
                        ansi_sidecar,
                        color_mode,
                        ..
                    }),
            }) => {
                assert!(ansi_sidecar);
                assert_eq!(
                    color_mode,
                    vec![SnapshotColorModeArg::Current, SnapshotColorModeArg::Mono]
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_ui_snapshot_status_alias() {
        let cli = Cli::parse_from([
            "ringmaster",
            "ui",
            "snapshot",
            "--screen",
            "ops",
            "--out-dir",
            "/tmp/ringmaster-snapshots",
        ])
        .unwrap_or_else(|error| {
            panic!("expected clap parsing to succeed in test: {error}");
        });

        match cli.command {
            Some(Command::Ui {
                command:
                    UiCommand::Snapshot(UiSnapshotArgs {
                        screen, out_dir, ..
                    }),
            }) => {
                assert_eq!(screen, vec![SnapshotScreenArg::Status]);
                assert_eq!(out_dir, PathBuf::from("/tmp/ringmaster-snapshots"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_snapshot_export_args() {
        let cli = Cli::parse_from([
            "ringmaster",
            "snapshot",
            "export",
            "--demo",
            "--scope",
            "week",
            "--profile",
            "balanced",
            "--out",
            "/tmp/ringmaster-snapshot.json",
        ])
        .unwrap_or_else(|error| {
            panic!("expected clap parsing to succeed in test: {error}");
        });

        match cli.command {
            Some(Command::Snapshot {
                command:
                    SnapshotCommand::Export(SnapshotExportArgs {
                        demo: true,
                        fixture_dir: None,
                        scope,
                        profile,
                        out,
                        compact: false,
                    }),
            }) => {
                assert_eq!(scope, "week");
                assert_eq!(profile, PrivacyProfileArg::Balanced);
                assert_eq!(out, Some(PathBuf::from("/tmp/ringmaster-snapshot.json")));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_snapshot_list_args() {
        let cli =
            Cli::parse_from(["ringmaster", "snapshot", "list", "--demo"]).unwrap_or_else(|error| {
                panic!("expected clap parsing to succeed in test: {error}");
            });

        match cli.command {
            Some(Command::Snapshot {
                command:
                    SnapshotCommand::List(SnapshotListArgs {
                        demo: true,
                        fixture_dir: None,
                    }),
            }) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_snapshot_show_args() {
        let cli = Cli::parse_from(["ringmaster", "snapshot", "show", "snapshot-hash", "--demo"])
            .unwrap_or_else(|error| {
                panic!("expected clap parsing to succeed in test: {error}");
            });

        match cli.command {
            Some(Command::Snapshot {
                command:
                    SnapshotCommand::Show(SnapshotShowArgs {
                        snapshot,
                        demo: true,
                        fixture_dir: None,
                    }),
            }) => assert_eq!(snapshot, "snapshot-hash"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_ai_review_args() {
        let cli = Cli::parse_from([
            "ringmaster",
            "ai",
            "review",
            "/tmp/snapshot.json",
            "--dry-run",
        ])
        .unwrap_or_else(|error| {
            panic!("expected clap parsing to succeed in test: {error}");
        });

        match cli.command {
            Some(Command::Ai {
                command:
                    AiCommand::Review(AiReviewArgs {
                        snapshot_path,
                        dry_run: true,
                        fixture: None,
                        out: None,
                    }),
            }) => assert_eq!(snapshot_path, PathBuf::from("/tmp/snapshot.json")),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_ai_compare_args() {
        let cli = Cli::parse_from([
            "ringmaster",
            "ai",
            "compare",
            "/tmp/a.json",
            "/tmp/b.json",
            "--fixture",
            "tests/fixtures/ai/compare.json",
        ])
        .unwrap_or_else(|error| {
            panic!("expected clap parsing to succeed in test: {error}");
        });

        match cli.command {
            Some(Command::Ai {
                command:
                    AiCommand::Compare(AiCompareArgs {
                        snapshot_a,
                        snapshot_b,
                        dry_run: false,
                        fixture,
                        out: None,
                    }),
            }) => {
                assert_eq!(snapshot_a, PathBuf::from("/tmp/a.json"));
                assert_eq!(snapshot_b, PathBuf::from("/tmp/b.json"));
                assert_eq!(
                    fixture,
                    Some(PathBuf::from("tests/fixtures/ai/compare.json"))
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_ai_runs_list_args() {
        let cli = Cli::parse_from(["ringmaster", "ai", "runs", "list", "--demo"]).unwrap_or_else(
            |error| {
                panic!("expected clap parsing to succeed in test: {error}");
            },
        );

        match cli.command {
            Some(Command::Ai {
                command:
                    AiCommand::Runs {
                        command:
                            AiRunsCommand::List(AiRunsListArgs {
                                demo: true,
                                fixture_dir: None,
                            }),
                    },
            }) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_ai_runs_show_args() {
        let cli = Cli::parse_from(["ringmaster", "ai", "runs", "show", "run-123"]).unwrap_or_else(
            |error| {
                panic!("expected clap parsing to succeed in test: {error}");
            },
        );

        match cli.command {
            Some(Command::Ai {
                command:
                    AiCommand::Runs {
                        command:
                            AiRunsCommand::Show(AiRunsShowArgs {
                                run_id,
                                demo: false,
                                fixture_dir: None,
                            }),
                    },
            }) => assert_eq!(run_id, "run-123"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_report_export_args() {
        let cli = Cli::parse_from([
            "ringmaster",
            "report",
            "export",
            "--from-snapshot",
            "snapshot-123",
            "--format",
            "markdown",
            "--out",
            "/tmp/report.md",
        ])
        .unwrap_or_else(|error| {
            panic!("expected clap parsing to succeed in test: {error}");
        });

        match cli.command {
            Some(Command::Report {
                command:
                    ReportCommand::Export(ReportExportArgs {
                        from_snapshot,
                        from_ai_run,
                        format,
                        out,
                        demo: false,
                        fixture_dir: None,
                    }),
            }) => {
                assert_eq!(from_snapshot, Some("snapshot-123".to_owned()));
                assert_eq!(from_ai_run, None);
                assert_eq!(format, ReportFormatArg::Markdown);
                assert_eq!(out, PathBuf::from("/tmp/report.md"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_ai_eval_args() {
        let cli = Cli::parse_from([
            "ringmaster",
            "ai",
            "eval",
            "--fixture-dir",
            "tests/fixtures/ai",
            "--candidate",
            "candidate",
            "--baseline",
            "baseline",
            "--export",
            "/tmp/eval.json",
        ])
        .unwrap_or_else(|error| {
            panic!("expected clap parsing to succeed in test: {error}");
        });

        match cli.command {
            Some(Command::Ai {
                command:
                    AiCommand::Eval(AiEvalArgs {
                        fixture_dir,
                        candidate,
                        baseline,
                        export,
                    }),
            }) => {
                assert_eq!(fixture_dir, PathBuf::from("tests/fixtures/ai"));
                assert_eq!(candidate, Some("candidate".to_owned()));
                assert_eq!(baseline, Some("baseline".to_owned()));
                assert_eq!(export, Some(PathBuf::from("/tmp/eval.json")));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
