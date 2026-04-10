use std::future::Future;
use std::io::{self, IsTerminal, Stdout, stdout};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle as ThreadJoinHandle;
use std::time::Duration;
use std::{collections::HashMap, env};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{CrosstermBackend, TestBackend},
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Block, Paragraph, Tabs},
};
use time::{
    Date, Duration as DateDuration, OffsetDateTime, format_description::well_known::Rfc3339,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle as AsyncJoinHandle;

use crate::action::Action;
use crate::ai::{self, AiRunStatus, GuidedFollowUpKind};
use crate::app::{
    AiBrowserTab, AiLaunchIntent, AiPreflightState, AppState, RunMode, Screen, load_live_snapshot,
};
use crate::cli::{ReportExportArgs, ReportFormatArg};
use crate::components::{
    ai as ai_component, dashboard, explain, ops, patterns, review, timeline, trends,
};
use crate::config::Config;
use crate::error::{Result, RingmasterError};
use crate::eval::parse_persisted_eval_details;
use crate::oura::{auth, sync::SyncOptions, sync::SyncReport, sync::sync_selected};
use crate::refresh::{SyncFamily, due_families, next_wake_duration};
use crate::report;
use crate::resolved_demo_fixture_dir;
use crate::snapshot::{self, LoadedSnapshotArtifact, PrivacyProfile, SnapshotSourceMode};
use crate::store::Store;
use crate::store::queries::{
    AiEvalRunRecord, AiRunRecord, ReportExportRecord, SnapshotCatalogEntry, SnapshotExportRecord,
};
use crate::ui::chrome::{self, PanelKind};
use crate::ui::layout::UiContext;
use crate::ui::theme::{Theme, Tone};

enum WorkerCommand {
    ManualRefresh,
    Shutdown,
}

enum InFlightRefreshResult<T> {
    Completed {
        result: T,
        queued_manual_refresh: bool,
    },
    Shutdown,
}

#[derive(Debug, Clone)]
struct AiPreflightRequest {
    intent: AiLaunchIntent,
    source_screen: Screen,
    selected_day: String,
    privacy_profile: PrivacyProfile,
}

#[derive(Debug, Clone)]
struct SavedRunPreflightOverrides {
    privacy_profile: PrivacyProfile,
    model_override: Option<String>,
    compare_previous_snapshot: bool,
}

#[derive(Debug, Clone)]
enum ReportSourceSelection {
    Snapshot(String),
    AiArtifact(String),
}

static REPORT_EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub async fn run(config: &Config, app: &mut AppState) -> Result<()> {
    if !(stdout().is_terminal() && io::stdin().is_terminal()) {
        return Err(RingmasterError::Ui(
            "interactive TUI mode requires a terminal".to_owned(),
        ));
    }

    let mut session = TerminalSession::start()?;
    let tick_rate = Duration::from_millis(250);
    let (worker_tx, mut worker_actions, worker_handle) = if app.mode == RunMode::Live {
        let (command_tx, action_rx, handle) = spawn_refresh_worker(config.clone());
        (Some(command_tx), Some(action_rx), Some(handle))
    } else {
        (None, None, None)
    };
    let (ai_action_tx, mut ai_actions) = unbounded_channel();
    let mut ai_tasks: HashMap<String, AsyncJoinHandle<()>> = HashMap::new();

    loop {
        if let Some(worker_actions) = worker_actions.as_mut() {
            drain_worker_actions(worker_actions, app);
        }
        drain_worker_actions(&mut ai_actions, app);
        ai_tasks.retain(|_, handle| !handle.is_finished());
        session.draw(app)?;

        if app.should_quit {
            break;
        }

        if event::poll(tick_rate)
            .map_err(|error| RingmasterError::io("polling terminal events", error))?
        {
            let event = event::read()
                .map_err(|error| RingmasterError::io("reading terminal event", error))?;
            if let Some(action) = map_event(app.active_screen, event) {
                let source_screen = app.active_screen;
                let selected_day = app.selected_day_label();
                let current_preflight = app.ai_preflight_state().cloned();
                let selected_tab = app.selected_ai_browser_tab();
                let selected_run = app.selected_ai_run_record();
                let selected_snapshot = app.selected_snapshot_catalog_entry();
                let selected_report = app.selected_report_export_record();
                let selected_eval = app.selected_ai_eval_run_record();
                let request_manual_refresh =
                    matches!(action, Action::RefreshRequested) && matches!(app.mode, RunMode::Live);
                app.handle(action.clone());
                if request_manual_refresh {
                    send_worker_command(&worker_tx, WorkerCommand::ManualRefresh);
                }
                handle_ai_side_effect(
                    config,
                    app.mode,
                    action,
                    source_screen,
                    selected_day,
                    current_preflight,
                    selected_tab,
                    selected_run,
                    selected_snapshot,
                    selected_report,
                    selected_eval,
                    &ai_action_tx,
                    &mut ai_tasks,
                )?;
            }
        } else {
            app.handle(Action::Tick);
        }
    }

    send_worker_command(&worker_tx, WorkerCommand::Shutdown);
    if let Some(worker_handle) = worker_handle {
        worker_handle
            .join()
            .map_err(|_| RingmasterError::Ui("refresh worker panicked".to_owned()))?;
    }
    for (_, handle) in ai_tasks {
        handle.abort();
    }

    Ok(())
}

pub fn render_snapshot(app: &AppState, width: u16, height: u16) -> Result<String> {
    let buffer = render_buffer(app, width, height)?;
    Ok(buffer_to_string(&buffer))
}

pub fn render_buffer(app: &AppState, width: u16, height: u16) -> Result<Buffer> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)
        .map_err(|error| RingmasterError::Ui(format!("building test terminal failed: {error}")))?;
    terminal
        .draw(|frame| draw(frame, app))
        .map_err(|error| RingmasterError::Ui(format!("drawing test terminal failed: {error}")))?;
    Ok(terminal.backend().buffer().clone())
}

fn buffer_to_string(buffer: &Buffer) -> String {
    let mut lines = Vec::new();

    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        lines.push(line.trim_end().to_owned());
    }

    lines.join("\n")
}

fn draw(frame: &mut ratatui::Frame<'_>, app: &AppState) {
    let theme = Theme::default();
    let ui = UiContext::new(frame.area());
    frame.render_widget(Block::default().style(theme.screen()), frame.area());

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if ui.viewport.is_compact() { 4 } else { 5 }),
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let header = Paragraph::new(app.model.title.clone())
        .style(theme.hero())
        .block(chrome::panel(
            &theme,
            Line::from("ringmaster.rs"),
            PanelKind::Hero,
        ));
    frame.render_widget(header, layout[0]);

    let tab_titles = Screen::ALL
        .into_iter()
        .map(|screen| Line::from(screen.title()))
        .collect::<Vec<_>>();
    let tabs = Tabs::new(tab_titles)
        .block(chrome::panel(
            &theme,
            chrome::title_with_badge(&theme, "Views", app.active_screen.title(), Tone::Accent),
            PanelKind::Subtle,
        ))
        .style(theme.annotation())
        .highlight_style(theme.emphasis(Tone::Focus))
        .divider(" ")
        .select(app.active_tab_index());
    frame.render_widget(tabs, layout[1]);

    draw_active_screen(frame, layout[2], app, &ui, &theme);

    let footer = Paragraph::new(app.footer())
        .style(theme.annotation())
        .block(chrome::panel(
            &theme,
            chrome::title_with_badge(&theme, "Keys", "keyboard-first", Tone::Muted),
            PanelKind::Subtle,
        ));
    frame.render_widget(footer, layout[3]);
}

fn draw_active_screen(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    app: &AppState,
    ui: &UiContext,
    theme: &Theme,
) {
    match app.active_screen {
        Screen::Dashboard => dashboard::draw(frame, area, &app.model.dashboard, ui, theme),
        Screen::Timeline => timeline::draw(frame, area, &app.model.timeline, ui, theme),
        Screen::Trends => trends::draw(frame, area, &app.model.trends, ui, theme),
        Screen::Explain => explain::draw(frame, area, &app.model.explain, ui, theme),
        Screen::Patterns => patterns::draw(frame, area, &app.model.patterns, ui, theme),
        Screen::Review => review::draw(frame, area, &app.model.review, ui, theme),
        Screen::Ai => ai_component::draw(frame, area, &app.model.ai, ui, theme),
        Screen::Ops => ops::draw(frame, area, &app.model.ops, ui, theme),
    }
}

fn map_event(active_screen: Screen, event: Event) -> Option<Action> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Some(Action::Quit),
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => Some(Action::NextScreen),
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => Some(Action::PreviousScreen),
            KeyCode::Char('r') => Some(Action::RefreshRequested),
            KeyCode::Char('1') => Some(Action::ShowScreen(Screen::Dashboard)),
            KeyCode::Char('2') => Some(Action::ShowScreen(Screen::Timeline)),
            KeyCode::Char('3') => Some(Action::ShowScreen(Screen::Trends)),
            KeyCode::Char('4') => Some(Action::ShowScreen(Screen::Explain)),
            KeyCode::Char('5') => Some(Action::ShowScreen(Screen::Patterns)),
            KeyCode::Char('6') => Some(Action::ShowScreen(Screen::Review)),
            KeyCode::Char('7') => Some(Action::ShowScreen(Screen::Ai)),
            KeyCode::Char('8') => Some(Action::ShowScreen(Screen::Ops)),
            KeyCode::Char('[') => match active_screen {
                Screen::Dashboard | Screen::Timeline | Screen::Explain | Screen::Review => {
                    Some(Action::PreviousDay)
                }
                Screen::Trends => Some(Action::PreviousTrendWindow),
                Screen::Ai => Some(Action::PreviousAiBrowserTab),
                _ => None,
            },
            KeyCode::Char(']') => match active_screen {
                Screen::Dashboard | Screen::Timeline | Screen::Explain | Screen::Review => {
                    Some(Action::NextDay)
                }
                Screen::Trends => Some(Action::NextTrendWindow),
                Screen::Ai => Some(Action::NextAiBrowserTab),
                _ => None,
            },
            KeyCode::Char(',') if active_screen == Screen::Timeline => {
                Some(Action::PreviousTimelinePoint)
            }
            KeyCode::Char('.') if active_screen == Screen::Timeline => {
                Some(Action::NextTimelinePoint)
            }
            KeyCode::Char('-') if active_screen == Screen::Timeline => {
                Some(Action::TimelineZoomOut)
            }
            KeyCode::Char('=') if active_screen == Screen::Timeline => Some(Action::TimelineZoomIn),
            KeyCode::Char('j') => match active_screen {
                Screen::Timeline | Screen::Explain => Some(Action::NextEvent),
                Screen::Review => Some(Action::NextReviewCard),
                Screen::Ai => Some(Action::NextAiBrowserItem),
                _ => None,
            },
            KeyCode::Char('k') => match active_screen {
                Screen::Timeline | Screen::Explain => Some(Action::PreviousEvent),
                Screen::Review => Some(Action::PreviousReviewCard),
                Screen::Ai => Some(Action::PreviousAiBrowserItem),
                _ => None,
            },
            KeyCode::Char('w')
                if matches!(
                    active_screen,
                    Screen::Timeline | Screen::Explain | Screen::Patterns
                ) =>
            {
                Some(Action::ToggleWorkoutFilter)
            }
            KeyCode::Char('t')
                if matches!(
                    active_screen,
                    Screen::Timeline | Screen::Explain | Screen::Patterns
                ) =>
            {
                Some(Action::ToggleTagFilter)
            }
            KeyCode::Char('s')
                if matches!(
                    active_screen,
                    Screen::Timeline | Screen::Explain | Screen::Patterns
                ) =>
            {
                Some(Action::ToggleSessionFilter)
            }
            KeyCode::Char('m') if active_screen == Screen::Patterns => {
                Some(Action::CyclePatternMetric)
            }
            KeyCode::Char('a')
                if matches!(
                    active_screen,
                    Screen::Dashboard | Screen::Explain | Screen::Review | Screen::Ai
                ) =>
            {
                Some(Action::RequestAiLaunch(AiLaunchIntent::ReviewSelectedDay))
            }
            KeyCode::Char('c')
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(
                        active_screen,
                        Screen::Dashboard | Screen::Patterns | Screen::Review | Screen::Ai
                    ) =>
            {
                Some(Action::RequestAiLaunch(AiLaunchIntent::CompareSelectedWeek))
            }
            KeyCode::Char('n') if active_screen == Screen::Ai => Some(Action::DismissAiPreflight),
            KeyCode::Char('p') if active_screen == Screen::Ai => {
                Some(Action::CycleAiPreflightPrivacyProfile)
            }
            KeyCode::Char('x') if active_screen == Screen::Ai => Some(Action::RequestCancelAiRun),
            KeyCode::Char('e') if active_screen == Screen::Ai => Some(
                Action::RequestAiGuidedFollowUp(GuidedFollowUpKind::ExpandEvidence),
            ),
            KeyCode::Char('y') if active_screen == Screen::Ai => Some(
                Action::RequestAiGuidedFollowUp(GuidedFollowUpKind::ShowCounterevidence),
            ),
            KeyCode::Char('i') if active_screen == Screen::Ai => Some(
                Action::RequestAiGuidedFollowUp(GuidedFollowUpKind::ExplainRanking),
            ),
            KeyCode::Char('d') if active_screen == Screen::Ai => Some(
                Action::RequestAiGuidedFollowUp(GuidedFollowUpKind::SuggestLocalDrilldown),
            ),
            KeyCode::Char('g') if active_screen == Screen::Ai => {
                Some(Action::RequestAiGenerateReport)
            }
            KeyCode::Char('u') if active_screen == Screen::Ai => {
                Some(Action::RequestAiRerunNextPrivacy)
            }
            KeyCode::Char('m') if active_screen == Screen::Ai => {
                Some(Action::RequestAiRerunNextModel)
            }
            KeyCode::Char('b') if active_screen == Screen::Ai => {
                Some(Action::RequestAiComparePreviousSnapshot)
            }
            KeyCode::Char('o') if active_screen == Screen::Ai => {
                Some(Action::RequestJumpToAiEvidence)
            }
            KeyCode::Enter if active_screen == Screen::Ai => Some(Action::ConfirmAiPreflight),
            KeyCode::Char('v') if active_screen == Screen::Review => Some(Action::CycleReviewMode),
            KeyCode::Char('f') if active_screen == Screen::Review => Some(Action::CycleReviewFocus),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Action::Quit)
            }
            _ => None,
        },
        _ => None,
    }
}

fn drain_worker_actions(worker_actions: &mut UnboundedReceiver<Action>, app: &mut AppState) {
    while let Ok(action) = worker_actions.try_recv() {
        app.handle(action);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_ai_side_effect(
    config: &Config,
    run_mode: RunMode,
    action: Action,
    source_screen: Screen,
    selected_day: Option<String>,
    current_preflight: Option<AiPreflightState>,
    selected_tab: AiBrowserTab,
    selected_run: Option<AiRunRecord>,
    selected_snapshot: Option<SnapshotCatalogEntry>,
    _selected_report: Option<ReportExportRecord>,
    selected_eval: Option<AiEvalRunRecord>,
    ai_action_tx: &UnboundedSender<Action>,
    ai_tasks: &mut HashMap<String, AsyncJoinHandle<()>>,
) -> Result<()> {
    if ai_run_controls_require_runs_tab(&action) && selected_tab != AiBrowserTab::Runs {
        let _ = ai_action_tx.send(Action::RefreshFailed {
            message: "Run controls only apply while browsing saved AI runs.".to_owned(),
        });
        return Ok(());
    }
    match action {
        Action::RequestAiLaunch(intent) => {
            let Some(selected_day) = selected_day else {
                let _ = ai_action_tx.send(Action::AiPreflightFailed {
                    message: "AI launches need a selected day from the current live snapshot."
                        .to_owned(),
                });
                return Ok(());
            };
            spawn_ai_preflight_task(
                config.clone(),
                run_mode,
                AiPreflightRequest {
                    intent,
                    source_screen,
                    selected_day,
                    privacy_profile: PrivacyProfile::Redacted,
                },
                ai_action_tx.clone(),
            );
        }
        Action::CycleAiPreflightPrivacyProfile => {
            if let Some(preflight) = current_preflight {
                if let Some(overrides) = saved_run_privacy_cycle_overrides(&preflight) {
                    let Some(run) = selected_run else {
                        let _ = ai_action_tx.send(Action::AiPreflightFailed {
                            message:
                                "The current AI preflight came from a saved run, but that run is no longer selected."
                                    .to_owned(),
                        });
                        return Ok(());
                    };
                    spawn_ai_saved_run_preflight_task(
                        config.clone(),
                        run_mode,
                        source_screen,
                        run,
                        preflight.follow_up_kind,
                        Some(overrides),
                        ai_action_tx.clone(),
                    );
                    return Ok(());
                }
                let Some(selected_day) = selected_day_from_preflight(&preflight) else {
                    let _ = ai_action_tx.send(Action::AiPreflightFailed {
                        message:
                            "The current AI preflight no longer has enough snapshot context to rotate privacy profiles."
                                .to_owned(),
                    });
                    return Ok(());
                };
                spawn_ai_preflight_task(
                    config.clone(),
                    run_mode,
                    AiPreflightRequest {
                        intent: preflight.intent,
                        source_screen: preflight.source_screen,
                        selected_day,
                        privacy_profile: preflight.privacy_profile.next(),
                    },
                    ai_action_tx.clone(),
                );
            }
        }
        Action::ConfirmAiPreflight => {
            let Some(preflight) = current_preflight else {
                return Ok(());
            };
            if !preflight.confirm_enabled {
                return Ok(());
            }
            let queued_record = persist_queued_ai_run(config, &preflight)?;
            let _ = ai_action_tx.send(reload_live_snapshot_action(
                config,
                &format!(
                    "Queued {} with {} privacy.",
                    queued_record.run_kind, queued_record.privacy_profile
                ),
                None,
            )?);
            let run_id = queued_record.run_id.clone();
            let config = config.clone();
            let ai_action_tx = ai_action_tx.clone();
            let handle = tokio::spawn(async move {
                run_ai_job(config, queued_record, preflight, ai_action_tx).await;
            });
            ai_tasks.insert(run_id, handle);
        }
        Action::RequestCancelAiRun => {
            let Some(run) = selected_run else {
                return Ok(());
            };
            if let Some(handle) = ai_tasks.remove(&run.run_id) {
                handle.abort();
            }
            cancel_ai_run(config, &run)?;
            let _ = ai_action_tx.send(reload_live_snapshot_action(
                config,
                &format!("Cancelled AI run {}.", abbreviate_id(&run.run_id, 12)),
                None,
            )?);
        }
        Action::RequestAiGuidedFollowUp(kind) => {
            let Some(run) = selected_run else {
                let _ = ai_action_tx.send(Action::AiPreflightFailed {
                    message: "Select a saved AI run before launching a guided follow-up."
                        .to_owned(),
                });
                return Ok(());
            };
            spawn_ai_saved_run_preflight_task(
                config.clone(),
                run_mode,
                source_screen,
                run,
                Some(kind),
                None,
                ai_action_tx.clone(),
            );
        }
        Action::RequestAiRerunNextPrivacy => {
            let Some(run) = selected_run else {
                return Ok(());
            };
            let next_privacy = next_privacy_profile_from_run(&run)?;
            spawn_ai_saved_run_preflight_task(
                config.clone(),
                run_mode,
                source_screen,
                run,
                None,
                Some(SavedRunPreflightOverrides {
                    privacy_profile: next_privacy,
                    model_override: None,
                    compare_previous_snapshot: false,
                }),
                ai_action_tx.clone(),
            );
        }
        Action::RequestAiRerunNextModel => {
            let Some(run) = selected_run else {
                return Ok(());
            };
            spawn_ai_saved_run_preflight_task(
                config.clone(),
                run_mode,
                source_screen,
                run.clone(),
                None,
                Some(SavedRunPreflightOverrides {
                    privacy_profile: parse_privacy_profile_label(&run.privacy_profile)?,
                    model_override: Some(next_model_choice(config, &run.model)),
                    compare_previous_snapshot: false,
                }),
                ai_action_tx.clone(),
            );
        }
        Action::RequestAiComparePreviousSnapshot => match selected_tab {
            AiBrowserTab::Runs => {
                let Some(run) = selected_run else {
                    return Ok(());
                };
                let overrides = compare_previous_snapshot_overrides(&run)?;
                spawn_ai_saved_run_preflight_task(
                    config.clone(),
                    run_mode,
                    source_screen,
                    run,
                    None,
                    Some(overrides),
                    ai_action_tx.clone(),
                );
            }
            AiBrowserTab::Snapshots => {
                let Some(snapshot) = selected_snapshot else {
                    return Ok(());
                };
                spawn_ai_snapshot_compare_task(
                    config.clone(),
                    run_mode,
                    source_screen,
                    snapshot,
                    ai_action_tx.clone(),
                );
            }
            AiBrowserTab::Reports => {}
            AiBrowserTab::Evals => {
                let _ = ai_action_tx.send(Action::RefreshFailed {
                    message:
                        "Eval entries are read-only history. Compare launches still start from saved runs or snapshots."
                            .to_owned(),
                });
            }
        },
        Action::RequestAiGenerateReport => match selected_tab {
            AiBrowserTab::Runs => {
                let Some(run) = selected_run else {
                    return Ok(());
                };
                let Some(artifact_id) = run.artifact_id else {
                    let _ = ai_action_tx.send(Action::RefreshFailed {
                        message:
                            "The selected AI run does not have a saved structured artifact to export yet."
                                .to_owned(),
                    });
                    return Ok(());
                };
                spawn_report_export_task(
                    config.clone(),
                    run_mode,
                    ReportSourceSelection::AiArtifact(artifact_id),
                    ai_action_tx.clone(),
                );
            }
            AiBrowserTab::Snapshots => {
                let Some(snapshot) = selected_snapshot else {
                    return Ok(());
                };
                spawn_report_export_task(
                    config.clone(),
                    run_mode,
                    ReportSourceSelection::Snapshot(snapshot.snapshot_hash),
                    ai_action_tx.clone(),
                );
            }
            AiBrowserTab::Reports => {}
            AiBrowserTab::Evals => {
                let _ = ai_action_tx.send(Action::RefreshFailed {
                    message:
                        "Eval entries are already exported history; generate reports from saved runs or snapshots instead."
                            .to_owned(),
                });
            }
        },
        Action::RequestJumpToAiEvidence => match selected_tab {
            AiBrowserTab::Runs => {
                let Some(run) = selected_run else {
                    return Ok(());
                };
                if let Some(jump_action) = build_jump_to_evidence_action(config, &run)? {
                    let _ = ai_action_tx.send(jump_action);
                } else {
                    let _ = ai_action_tx.send(Action::RefreshFailed {
                        message:
                            "The selected AI run does not have a resolvable evidence reference yet."
                                .to_owned(),
                    });
                }
            }
            AiBrowserTab::Evals => {
                let Some(eval) = selected_eval else {
                    return Ok(());
                };
                if let Some(jump_action) = build_eval_jump_action(&eval) {
                    let _ = ai_action_tx.send(jump_action);
                } else {
                    let _ = ai_action_tx.send(Action::RefreshFailed {
                            message:
                                "The selected eval does not declare a saved snapshot, AI run, or report link."
                                    .to_owned(),
                        });
                }
            }
            AiBrowserTab::Snapshots | AiBrowserTab::Reports => {}
        },
        _ => {}
    }
    Ok(())
}

fn ai_run_controls_require_runs_tab(action: &Action) -> bool {
    matches!(
        action,
        Action::RequestCancelAiRun
            | Action::RequestAiGuidedFollowUp(_)
            | Action::RequestAiRerunNextPrivacy
            | Action::RequestAiRerunNextModel
    )
}

fn spawn_ai_preflight_task(
    config: Config,
    run_mode: RunMode,
    request: AiPreflightRequest,
    action_tx: UnboundedSender<Action>,
) {
    tokio::spawn(async move {
        let task =
            tokio::task::spawn_blocking(move || prepare_ai_preflight(&config, run_mode, &request));
        match task.await {
            Ok(Ok((snapshot_reload, preflight, status_line))) => {
                let _ = action_tx.send(snapshot_reload);
                let _ = action_tx.send(Action::AiPreflightPrepared {
                    preflight: Box::new(preflight),
                    status_line,
                });
            }
            Ok(Err(error)) => {
                let _ = action_tx.send(Action::AiPreflightFailed {
                    message: format!("AI preflight failed: {error}"),
                });
            }
            Err(error) => {
                let _ = action_tx.send(Action::AiPreflightFailed {
                    message: format!("AI preflight task failed to join: {error}"),
                });
            }
        }
    });
}

fn spawn_ai_saved_run_preflight_task(
    config: Config,
    run_mode: RunMode,
    source_screen: Screen,
    run: AiRunRecord,
    follow_up_kind: Option<GuidedFollowUpKind>,
    overrides: Option<SavedRunPreflightOverrides>,
    action_tx: UnboundedSender<Action>,
) {
    tokio::spawn(async move {
        let task = tokio::task::spawn_blocking(move || {
            prepare_ai_saved_run_preflight(
                &config,
                run_mode,
                source_screen,
                &run,
                follow_up_kind,
                overrides.as_ref(),
            )
        });
        match task.await {
            Ok(Ok((snapshot_reload, preflight, status_line))) => {
                let _ = action_tx.send(snapshot_reload);
                let _ = action_tx.send(Action::AiPreflightPrepared {
                    preflight: Box::new(preflight),
                    status_line,
                });
            }
            Ok(Err(error)) => {
                let _ = action_tx.send(Action::AiPreflightFailed {
                    message: format!("AI preflight failed: {error}"),
                });
            }
            Err(error) => {
                let _ = action_tx.send(Action::AiPreflightFailed {
                    message: format!("AI preflight task failed to join: {error}"),
                });
            }
        }
    });
}

fn spawn_ai_snapshot_compare_task(
    config: Config,
    run_mode: RunMode,
    source_screen: Screen,
    snapshot: SnapshotCatalogEntry,
    action_tx: UnboundedSender<Action>,
) {
    tokio::spawn(async move {
        let task = tokio::task::spawn_blocking(move || {
            prepare_ai_snapshot_compare_preflight(&config, run_mode, source_screen, &snapshot)
        });
        match task.await {
            Ok(Ok((snapshot_reload, preflight, status_line))) => {
                let _ = action_tx.send(snapshot_reload);
                let _ = action_tx.send(Action::AiPreflightPrepared {
                    preflight: Box::new(preflight),
                    status_line,
                });
            }
            Ok(Err(error)) => {
                let _ = action_tx.send(Action::AiPreflightFailed {
                    message: format!("AI compare preflight failed: {error}"),
                });
            }
            Err(error) => {
                let _ = action_tx.send(Action::AiPreflightFailed {
                    message: format!("AI compare preflight task failed to join: {error}"),
                });
            }
        }
    });
}

fn spawn_report_export_task(
    config: Config,
    run_mode: RunMode,
    source: ReportSourceSelection,
    action_tx: UnboundedSender<Action>,
) {
    tokio::spawn(async move {
        let runtime_handle = tokio::runtime::Handle::current();
        let output_path = match report_output_path(&config, &source) {
            Ok(path) => path,
            Err(error) => {
                let _ = action_tx.send(Action::RefreshFailed {
                    message: format!("Could not prepare report export path: {error}"),
                });
                return;
            }
        };

        let export_context = build_report_export_context(&config, run_mode, source, output_path);
        let output_path = export_context.args.out.clone();
        let success_message = format!("Exported report to {}.", output_path.display());

        let export_task = tokio::task::spawn_blocking({
            let config = config.clone();
            let args = export_context.args.clone();
            move || runtime_handle.block_on(report::export_report(&config, args))
        });

        match export_task.await {
            Ok(Ok(_)) => {
                let refresh_task = tokio::task::spawn_blocking({
                    let config = config.clone();
                    let success_message = success_message.clone();
                    move || reload_live_snapshot_action(&config, &success_message, None)
                });

                match refresh_task.await {
                    Ok(Ok(action)) => {
                        let _ = action_tx.send(action);
                    }
                    Ok(Err(error)) => {
                        let _ = action_tx.send(Action::RefreshFailed {
                            message: format!(
                                "Report export succeeded, but the workbench could not refresh: {error}"
                            ),
                        });
                    }
                    Err(error) => {
                        let _ = action_tx.send(Action::RefreshFailed {
                            message: format!(
                                "Report export succeeded, but the refresh worker failed to join: {error}"
                            ),
                        });
                    }
                }
            }
            Ok(Err(error)) => {
                let _ = action_tx.send(Action::RefreshFailed {
                    message: format!("Report export failed: {error}"),
                });
            }
            Err(error) => {
                let _ = action_tx.send(Action::RefreshFailed {
                    message: format!("Report export worker failed to join: {error}"),
                });
            }
        }
    });
}

#[derive(Debug, Clone)]
struct ReportExportContext {
    args: ReportExportArgs,
}

fn build_report_export_context(
    config: &Config,
    run_mode: RunMode,
    source: ReportSourceSelection,
    output_path: PathBuf,
) -> ReportExportContext {
    let demo = run_mode == RunMode::Demo;
    let fixture_dir = resolved_demo_fixture_dir(config, demo, None);

    let args = match source {
        ReportSourceSelection::Snapshot(snapshot_hash) => ReportExportArgs {
            from_snapshot: Some(snapshot_hash),
            from_ai_run: None,
            format: ReportFormatArg::Markdown,
            out: output_path,
            demo,
            fixture_dir,
        },
        ReportSourceSelection::AiArtifact(artifact_id) => ReportExportArgs {
            from_snapshot: None,
            from_ai_run: Some(artifact_id),
            format: ReportFormatArg::Markdown,
            out: output_path,
            demo,
            fixture_dir,
        },
    };

    ReportExportContext { args }
}

fn prepare_ai_preflight(
    config: &Config,
    run_mode: RunMode,
    request: &AiPreflightRequest,
) -> Result<(Action, AiPreflightState, String)> {
    if run_mode != RunMode::Live {
        return Err(RingmasterError::Ui(
            "AI launches require live mode; demo mode keeps the workbench browse-only.".to_owned(),
        ));
    }

    let store = Store::open(config)?;
    let auth_status = auth::inspect_auth(config, &store)?;
    let mut warning_lines = ai_preflight_warning_lines(config);

    let (snapshot_scope, snapshot_paths, request_preview) = match request.intent {
        AiLaunchIntent::ReviewSelectedDay | AiLaunchIntent::ChallengeSelectedDay => {
            let scope_spec = format!("day:{}", request.selected_day);
            let (artifact, path) = export_preflight_snapshot(
                config,
                &store,
                &auth_status,
                &scope_spec,
                request.privacy_profile,
                "review",
            )?;
            let preview = ai::preview_review_request(config, &artifact)?;
            if request.intent == AiLaunchIntent::ChallengeSelectedDay {
                warning_lines.push(
                    "Challenge launches become active from saved AI run detail views; this workbench shortcut stays preview-only for now."
                        .to_owned(),
                );
            }
            (scope_spec, vec![path.display().to_string()], preview)
        }
        AiLaunchIntent::CompareSelectedWeek => {
            let selected_day = parse_day(&request.selected_day)?;
            let current_start = selected_day - DateDuration::days(6);
            let previous_end = current_start - DateDuration::days(1);
            let previous_start = previous_end - DateDuration::days(6);
            let previous_scope = format!(
                "range:{}..{}",
                format_day(previous_start)?,
                format_day(previous_end)?
            );
            let current_scope = format!(
                "range:{}..{}",
                format_day(current_start)?,
                format_day(selected_day)?
            );
            let (snapshot_a, path_a) = export_preflight_snapshot(
                config,
                &store,
                &auth_status,
                &previous_scope,
                request.privacy_profile,
                "compare-a",
            )?;
            let (snapshot_b, path_b) = export_preflight_snapshot(
                config,
                &store,
                &auth_status,
                &current_scope,
                request.privacy_profile,
                "compare-b",
            )?;
            let preview = ai::preview_compare_request(config, &snapshot_a, &snapshot_b)?;
            (
                format!("{current_scope} vs {previous_scope}"),
                vec![path_a.display().to_string(), path_b.display().to_string()],
                preview,
            )
        }
    };

    let confirm_enabled =
        warning_lines.is_empty() && request.intent != AiLaunchIntent::ChallengeSelectedDay;
    let preflight = AiPreflightState {
        intent: request.intent,
        source_screen: request.source_screen,
        snapshot_scope,
        snapshot_paths,
        request_preview,
        privacy_profile: request.privacy_profile,
        model_override: None,
        source_ai_artifact_id: None,
        follow_up_kind: None,
        warning_lines,
        confirm_enabled,
    };
    let status_line = format!(
        "Prepared {} preflight with {} privacy.",
        request.intent.short_label(),
        request.privacy_profile.as_str()
    );
    let snapshot_reload = reload_live_snapshot_action(config, &status_line, Some(&store))?;
    Ok((snapshot_reload, preflight, status_line))
}

fn prepare_ai_saved_run_preflight(
    config: &Config,
    run_mode: RunMode,
    source_screen: Screen,
    run: &AiRunRecord,
    explicit_follow_up_kind: Option<GuidedFollowUpKind>,
    overrides: Option<&SavedRunPreflightOverrides>,
) -> Result<(Action, AiPreflightState, String)> {
    if run_mode != RunMode::Live {
        return Err(RingmasterError::Ui(
            "AI launches require live mode; demo mode keeps the workbench browse-only.".to_owned(),
        ));
    }

    let store = Store::open(config)?;
    let auth_status = auth::inspect_auth(config, &store)?;
    let privacy_profile = overrides.map_or_else(
        || parse_privacy_profile_label(&run.privacy_profile),
        |overrides| Ok(overrides.privacy_profile),
    )?;
    let model_override = overrides.and_then(|overrides| overrides.model_override.clone());
    let compare_previous_snapshot =
        overrides.is_some_and(|overrides| overrides.compare_previous_snapshot);
    let preview_config = config_with_model_override(config, model_override.as_deref());

    let (
        intent,
        snapshot_scope,
        snapshot_paths,
        request_preview,
        follow_up_kind,
        source_ai_artifact_id,
    ) = if compare_previous_snapshot {
        let current_snapshot =
            load_snapshot_export_record(&store, preferred_current_snapshot_hash(run))?;
        let previous_snapshot = previous_similar_snapshot_record(&store, &current_snapshot)?;
        let (snapshot_a, path_a) = materialize_snapshot_for_preflight(
            config,
            &store,
            &auth_status,
            &previous_snapshot,
            privacy_profile,
            "compare-a",
        )?;
        let (snapshot_b, path_b) = materialize_snapshot_for_preflight(
            config,
            &store,
            &auth_status,
            &current_snapshot,
            privacy_profile,
            "compare-b",
        )?;
        (
            AiLaunchIntent::CompareSelectedWeek,
            format!("{} vs {}", current_snapshot.scope, previous_snapshot.scope),
            vec![path_a.display().to_string(), path_b.display().to_string()],
            ai::preview_compare_request(&preview_config, &snapshot_a, &snapshot_b)?,
            None,
            None,
        )
    } else if let Some(follow_up_kind) = explicit_follow_up_kind
        .or_else(|| parse_follow_up_kind_label(run.follow_up_kind.as_deref()))
    {
        let source_ai_artifact_id = resolve_follow_up_source_artifact_id(run)?.to_owned();
        let source_record = store
            .analysis()
            .ai_artifact(&source_ai_artifact_id)?
            .ok_or_else(|| {
                RingmasterError::Ui(format!(
                    "Saved AI artifact `{source_ai_artifact_id}` is no longer present."
                ))
            })?;
        let snapshot_records = load_run_snapshot_records(&store, run)?;
        let mut prepared_snapshots = Vec::new();
        let mut snapshot_paths = Vec::new();
        for (index, snapshot_record) in snapshot_records.iter().enumerate() {
            let (snapshot, path) = materialize_snapshot_for_preflight(
                config,
                &store,
                &auth_status,
                snapshot_record,
                privacy_profile,
                &format!("follow-up-{}", index + 1),
            )?;
            prepared_snapshots.push(snapshot);
            snapshot_paths.push(path.display().to_string());
        }
        (
            AiLaunchIntent::ChallengeSelectedDay,
            run.snapshot_scope.clone(),
            snapshot_paths,
            ai::preview_follow_up_request(
                &preview_config,
                &prepared_snapshots,
                &source_record,
                follow_up_kind,
            )?,
            Some(follow_up_kind),
            Some(source_ai_artifact_id),
        )
    } else if run.run_kind == "compare" {
        let snapshot_records = load_run_snapshot_records(&store, run)?;
        if snapshot_records.len() < 2 {
            return Err(RingmasterError::Ui(
                "Compare reruns need two persisted source snapshots.".to_owned(),
            ));
        }
        let (snapshot_a, path_a) = materialize_snapshot_for_preflight(
            config,
            &store,
            &auth_status,
            &snapshot_records[0],
            privacy_profile,
            "compare-a",
        )?;
        let (snapshot_b, path_b) = materialize_snapshot_for_preflight(
            config,
            &store,
            &auth_status,
            &snapshot_records[1],
            privacy_profile,
            "compare-b",
        )?;
        (
            AiLaunchIntent::CompareSelectedWeek,
            run.snapshot_scope.clone(),
            vec![path_a.display().to_string(), path_b.display().to_string()],
            ai::preview_compare_request(&preview_config, &snapshot_a, &snapshot_b)?,
            None,
            None,
        )
    } else {
        let snapshot_record = load_snapshot_export_record(&store, &run.snapshot_hash_a)?;
        let (snapshot, path) = materialize_snapshot_for_preflight(
            config,
            &store,
            &auth_status,
            &snapshot_record,
            privacy_profile,
            "review",
        )?;
        (
            AiLaunchIntent::ReviewSelectedDay,
            run.snapshot_scope.clone(),
            vec![path.display().to_string()],
            ai::preview_review_request(&preview_config, &snapshot)?,
            None,
            None,
        )
    };

    let warning_lines = ai_preflight_warning_lines(&preview_config);
    let confirm_enabled = warning_lines.is_empty();
    let preflight = AiPreflightState {
        intent,
        source_screen,
        snapshot_scope,
        snapshot_paths,
        request_preview,
        privacy_profile,
        model_override,
        source_ai_artifact_id,
        follow_up_kind,
        warning_lines,
        confirm_enabled,
    };
    let status_line = if let Some(follow_up_kind) = preflight.follow_up_kind {
        format!(
            "Prepared {} follow-up with {} privacy.",
            follow_up_kind.label(),
            preflight.privacy_profile.as_str()
        )
    } else {
        format!(
            "Prepared {} preflight with {} privacy.",
            preflight.intent.short_label(),
            preflight.privacy_profile.as_str()
        )
    };
    let snapshot_reload = reload_live_snapshot_action(config, &status_line, Some(&store))?;
    Ok((snapshot_reload, preflight, status_line))
}

fn prepare_ai_snapshot_compare_preflight(
    config: &Config,
    run_mode: RunMode,
    source_screen: Screen,
    snapshot: &SnapshotCatalogEntry,
) -> Result<(Action, AiPreflightState, String)> {
    if run_mode != RunMode::Live {
        return Err(RingmasterError::Ui(
            "AI launches require live mode; demo mode keeps the workbench browse-only.".to_owned(),
        ));
    }

    let store = Store::open(config)?;
    let auth_status = auth::inspect_auth(config, &store)?;
    let current_snapshot = load_snapshot_export_record(&store, &snapshot.snapshot_hash)?;
    let previous_snapshot = previous_similar_snapshot_record(&store, &current_snapshot)?;
    let privacy_profile = parse_privacy_profile_label(&current_snapshot.privacy_profile)?;
    let (snapshot_a, path_a) = materialize_snapshot_for_preflight(
        config,
        &store,
        &auth_status,
        &previous_snapshot,
        privacy_profile,
        "compare-a",
    )?;
    let (snapshot_b, path_b) = materialize_snapshot_for_preflight(
        config,
        &store,
        &auth_status,
        &current_snapshot,
        privacy_profile,
        "compare-b",
    )?;
    let request_preview = ai::preview_compare_request(config, &snapshot_a, &snapshot_b)?;
    let warning_lines = ai_preflight_warning_lines(config);
    let confirm_enabled = warning_lines.is_empty();
    let preflight = AiPreflightState {
        intent: AiLaunchIntent::CompareSelectedWeek,
        source_screen,
        snapshot_scope: format!("{} vs {}", current_snapshot.scope, previous_snapshot.scope),
        snapshot_paths: vec![path_a.display().to_string(), path_b.display().to_string()],
        request_preview,
        privacy_profile,
        model_override: None,
        source_ai_artifact_id: None,
        follow_up_kind: None,
        warning_lines,
        confirm_enabled,
    };
    let status_line = format!(
        "Prepared compare against the nearest previous snapshot for {}.",
        current_snapshot.anchor_day
    );
    let snapshot_reload = reload_live_snapshot_action(config, &status_line, Some(&store))?;
    Ok((snapshot_reload, preflight, status_line))
}

fn saved_run_privacy_cycle_overrides(
    preflight: &AiPreflightState,
) -> Option<SavedRunPreflightOverrides> {
    if preflight.follow_up_kind.is_none()
        && preflight.source_ai_artifact_id.is_none()
        && preflight.model_override.is_none()
    {
        return None;
    }

    Some(SavedRunPreflightOverrides {
        privacy_profile: preflight.privacy_profile.next(),
        model_override: preflight.model_override.clone(),
        compare_previous_snapshot: false,
    })
}

fn compare_previous_snapshot_overrides(run: &AiRunRecord) -> Result<SavedRunPreflightOverrides> {
    Ok(SavedRunPreflightOverrides {
        privacy_profile: parse_privacy_profile_label(&run.privacy_profile)?,
        model_override: None,
        compare_previous_snapshot: true,
    })
}

fn export_preflight_snapshot(
    config: &Config,
    store: &Store,
    auth_status: &crate::oura::models::AuthStatus,
    scope_spec: &str,
    privacy_profile: PrivacyProfile,
    label: &str,
) -> Result<(LoadedSnapshotArtifact, PathBuf)> {
    let scope = snapshot::resolve_scope(store, scope_spec)?;
    let export = snapshot::export_snapshot(
        config,
        store,
        auth_status,
        SnapshotSourceMode::Live,
        None,
        &scope,
        privacy_profile,
    )?;
    store
        .analysis()
        .upsert_snapshot_export(&export.manifest_record, &export.provenance_records)?;
    let path = ai_snapshot_output_dir(config)?.join(format!(
        "{}-{}-{}.json",
        label,
        scope.anchor_day.replace('-', ""),
        privacy_profile.as_str()
    ));
    snapshot::write_snapshot_artifact(&path, &export.compact_json)?;
    Ok((
        LoadedSnapshotArtifact {
            bundle: export.bundle,
            compact_json: export.compact_json,
        },
        path,
    ))
}

fn load_snapshot_export_record(store: &Store, snapshot_hash: &str) -> Result<SnapshotExportRecord> {
    store
        .analysis()
        .snapshot_export(snapshot_hash)?
        .ok_or_else(|| {
            RingmasterError::Ui(format!(
                "Snapshot `{snapshot_hash}` is no longer present in the local catalog."
            ))
        })
}

fn load_run_snapshot_records(
    store: &Store,
    run: &AiRunRecord,
) -> Result<Vec<SnapshotExportRecord>> {
    let mut records = vec![load_snapshot_export_record(store, &run.snapshot_hash_a)?];
    if let Some(snapshot_hash_b) = &run.snapshot_hash_b {
        records.push(load_snapshot_export_record(store, snapshot_hash_b)?);
    }
    Ok(records)
}

fn materialize_snapshot_for_preflight(
    config: &Config,
    store: &Store,
    auth_status: &crate::oura::models::AuthStatus,
    record: &SnapshotExportRecord,
    privacy_profile: PrivacyProfile,
    label: &str,
) -> Result<(LoadedSnapshotArtifact, PathBuf)> {
    if record.privacy_profile == privacy_profile.as_str() {
        let path = ai_snapshot_output_dir(config)?.join(format!(
            "{}-{}-{}.json",
            label,
            record.anchor_day.replace('-', ""),
            privacy_profile.as_str()
        ));
        snapshot::write_snapshot_artifact(&path, &record.snapshot_json)?;
        return Ok((
            LoadedSnapshotArtifact {
                bundle: snapshot::deserialize_snapshot_bundle(&record.snapshot_json)?,
                compact_json: record.snapshot_json.clone(),
            },
            path,
        ));
    }

    export_preflight_snapshot(
        config,
        store,
        auth_status,
        &record.scope,
        privacy_profile,
        label,
    )
}

fn preferred_current_snapshot_hash(run: &AiRunRecord) -> &str {
    run.snapshot_hash_b
        .as_deref()
        .unwrap_or(&run.snapshot_hash_a)
}

fn previous_similar_snapshot_record(
    store: &Store,
    current_snapshot: &SnapshotExportRecord,
) -> Result<SnapshotExportRecord> {
    let candidates = store.analysis().list_snapshot_exports()?;
    let preferred = candidates
        .iter()
        .filter(|candidate| candidate.snapshot_hash != current_snapshot.snapshot_hash)
        .filter(|candidate| candidate.privacy_profile == current_snapshot.privacy_profile)
        .filter(|candidate| candidate.day_count == current_snapshot.day_count)
        .filter(|candidate| {
            candidate.scope != current_snapshot.scope
                || candidate.anchor_day != current_snapshot.anchor_day
        })
        .find(|candidate| candidate.created_at < current_snapshot.created_at)
        .or_else(|| {
            candidates
                .iter()
                .filter(|candidate| candidate.snapshot_hash != current_snapshot.snapshot_hash)
                .filter(|candidate| candidate.privacy_profile == current_snapshot.privacy_profile)
                .find(|candidate| candidate.created_at < current_snapshot.created_at)
        })
        .ok_or_else(|| {
            RingmasterError::Ui(
                "No earlier similar snapshot is available in the local catalog yet.".to_owned(),
            )
        })?;

    load_snapshot_export_record(store, &preferred.snapshot_hash)
}

fn resolve_follow_up_source_artifact_id(run: &AiRunRecord) -> Result<&str> {
    if run.run_kind == "follow_up" {
        run.source_ai_artifact_id
            .as_deref()
            .or(run.artifact_id.as_deref())
            .ok_or_else(|| {
                RingmasterError::Ui(
                    "Saved follow-up runs need a linked source artifact before they can be rerun."
                        .to_owned(),
                )
            })
    } else {
        run.artifact_id.as_deref().ok_or_else(|| {
            RingmasterError::Ui(
                "The selected AI run does not have a saved structured artifact yet.".to_owned(),
            )
        })
    }
}

fn parse_follow_up_kind_label(value: Option<&str>) -> Option<GuidedFollowUpKind> {
    match value {
        Some("expand_evidence") => Some(GuidedFollowUpKind::ExpandEvidence),
        Some("show_counterevidence") => Some(GuidedFollowUpKind::ShowCounterevidence),
        Some("explain_ranking") => Some(GuidedFollowUpKind::ExplainRanking),
        Some("suggest_local_drilldown") => Some(GuidedFollowUpKind::SuggestLocalDrilldown),
        _ => None,
    }
}

fn parse_privacy_profile_label(value: &str) -> Result<PrivacyProfile> {
    match value {
        "redacted" => Ok(PrivacyProfile::Redacted),
        "balanced" => Ok(PrivacyProfile::Balanced),
        "full" => Ok(PrivacyProfile::Full),
        other => Err(RingmasterError::Ui(format!(
            "Unknown privacy profile `{other}` in the saved AI registry."
        ))),
    }
}

fn next_privacy_profile_from_run(run: &AiRunRecord) -> Result<PrivacyProfile> {
    parse_privacy_profile_label(&run.privacy_profile).map(PrivacyProfile::next)
}

fn next_model_choice(config: &Config, current_model: &str) -> String {
    if config.ai.model != current_model {
        return config.ai.model.clone();
    }

    match current_model {
        "gpt-5" => "gpt-5-mini".to_owned(),
        "gpt-5-mini" => "gpt-5".to_owned(),
        model if model.ends_with("-mini") => model.trim_end_matches("-mini").to_owned(),
        _ => "gpt-5-mini".to_owned(),
    }
}

fn config_with_model_override(config: &Config, model_override: Option<&str>) -> Config {
    let mut cloned = config.clone();
    if let Some(model_override) = model_override {
        model_override.clone_into(&mut cloned.ai.model);
    }
    cloned
}

fn report_output_path(config: &Config, source: &ReportSourceSelection) -> Result<PathBuf> {
    let report_dir = config.paths.cache_dir.join("ai-workbench").join("reports");
    std::fs::create_dir_all(&report_dir)
        .map_err(|error| RingmasterError::io("creating AI workbench report directory", error))?;
    let slug = match source {
        ReportSourceSelection::Snapshot(snapshot_hash) => {
            format!("snapshot-{}", abbreviate_id(snapshot_hash, 16))
        }
        ReportSourceSelection::AiArtifact(artifact_id) => {
            format!("run-{}", abbreviate_id(artifact_id, 16))
        }
    };
    let sequence = REPORT_EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    Ok(report_dir.join(format!(
        "{}-{}-{}.md",
        slug,
        OffsetDateTime::now_utc().unix_timestamp_nanos(),
        sequence
    )))
}

fn ai_snapshot_output_dir(config: &Config) -> Result<PathBuf> {
    let output_dir = config
        .paths
        .cache_dir
        .join("ai-workbench")
        .join("snapshots");
    std::fs::create_dir_all(&output_dir)
        .map_err(|error| RingmasterError::io("creating AI workbench snapshot cache", error))?;
    Ok(output_dir)
}

fn ai_preflight_warning_lines(config: &Config) -> Vec<String> {
    let mut warning_lines = Vec::new();
    if !config.ai.enabled {
        warning_lines.push(
            "The configured AI provider is disabled, so the request stays local until you enable AI."
                .to_owned(),
        );
    }
    if !ai_api_key_ready(config) {
        warning_lines.push(format!(
            "The API key env `{}` is not set, so confirmation remains blocked.",
            config.ai.api_key_env
        ));
    }
    warning_lines
}

fn ai_api_key_ready(config: &Config) -> bool {
    env::var(&config.ai.api_key_env)
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
}

fn selected_day_from_preflight(preflight: &AiPreflightState) -> Option<String> {
    preflight
        .request_preview
        .snapshots
        .iter()
        .map(|snapshot| snapshot.anchor_day.clone())
        .max()
}

fn persist_queued_ai_run(config: &Config, preflight: &AiPreflightState) -> Result<AiRunRecord> {
    let created_at = now_rfc3339()?;
    let run_id = format!(
        "run-{}-{}",
        preflight
            .intent
            .short_label()
            .to_lowercase()
            .replace(' ', "-"),
        OffsetDateTime::now_utc().unix_timestamp_nanos()
    );
    let record = build_ai_run_record(
        &run_id,
        preflight,
        AiRunStatus::Queued,
        &created_at,
        None,
        None,
        None,
        None,
    )?;
    let store = Store::open(config)?;
    store.analysis().upsert_ai_run(&record)?;
    Ok(record)
}

async fn run_ai_job(
    config: Config,
    queued_record: AiRunRecord,
    preflight: AiPreflightState,
    action_tx: UnboundedSender<Action>,
) {
    let mut failed_base_record = queued_record.clone();
    let result = async {
        let started_at = now_rfc3339()?;
        let store = Store::open(&config)?;
        let mut running_record = queued_record.clone();
        AiRunStatus::Running
            .as_str()
            .clone_into(&mut running_record.run_status);
        running_record.started_at = Some(started_at.clone());
        running_record.updated_at = started_at.clone();
        store.analysis().upsert_ai_run(&running_record)?;
        failed_base_record = running_record.clone();
        let _ = action_tx.send(reload_live_snapshot_action(
            &config,
            &format!("Running {}.", running_record.run_kind),
            Some(&store),
        )?);

        let run_config = config_with_model_override(&config, preflight.model_override.as_deref());

        if let Some(follow_up_kind) = preflight.follow_up_kind {
            let source_ai_artifact_id =
                preflight.source_ai_artifact_id.clone().ok_or_else(|| {
                    RingmasterError::Ui(
                        "Follow-up runs need a saved source artifact in preflight.".to_owned(),
                    )
                })?;
            let source_record = store
                .analysis()
                .ai_artifact(&source_ai_artifact_id)?
                .ok_or_else(|| {
                    RingmasterError::Ui(format!(
                        "Saved source artifact `{source_ai_artifact_id}` is no longer present."
                    ))
                })?;
            let snapshots = load_snapshots_from_preflight(&preflight)?;
            let output = ai::follow_up_from_artifact(
                &run_config,
                &snapshots,
                &source_record,
                follow_up_kind,
                false,
                None,
            )
            .await?;
            store.analysis().upsert_ai_artifact(&output.record)?;
            let completed = complete_ai_run_record(
                &running_record,
                &output.request_preview,
                &output.request_fingerprint,
                Some(output.record.artifact_id.clone()),
                Some(source_ai_artifact_id),
                now_rfc3339()?,
            )?;
            store.analysis().upsert_ai_run(&completed)?;
            let _ = action_tx.send(reload_live_snapshot_action(
                &config,
                &format!(
                    "Completed AI {} {}.",
                    completed.run_kind,
                    abbreviate_id(&completed.run_id, 12)
                ),
                Some(&store),
            )?);
        } else {
            match preflight.intent {
                AiLaunchIntent::ReviewSelectedDay | AiLaunchIntent::ChallengeSelectedDay => {
                    let snapshot = load_snapshot_from_preflight(&preflight, 0)?;
                    let output = ai::review_snapshot(&run_config, &snapshot, false, None).await?;
                    store.analysis().upsert_ai_artifact(&output.record)?;
                    let completed = complete_ai_run_record(
                        &running_record,
                        &output.request_preview,
                        &output.request_fingerprint,
                        Some(output.record.artifact_id.clone()),
                        None,
                        now_rfc3339()?,
                    )?;
                    store.analysis().upsert_ai_run(&completed)?;
                    let _ = action_tx.send(reload_live_snapshot_action(
                        &config,
                        &format!(
                            "Completed AI review {}.",
                            abbreviate_id(&completed.run_id, 12)
                        ),
                        Some(&store),
                    )?);
                }
                AiLaunchIntent::CompareSelectedWeek => {
                    let snapshot_a = load_snapshot_from_preflight(&preflight, 0)?;
                    let snapshot_b = load_snapshot_from_preflight(&preflight, 1)?;
                    let output =
                        ai::compare_snapshots(&run_config, &snapshot_a, &snapshot_b, false, None)
                            .await?;
                    store.analysis().upsert_ai_artifact(&output.record)?;
                    let completed = complete_ai_run_record(
                        &running_record,
                        &output.request_preview,
                        &output.request_fingerprint,
                        Some(output.record.artifact_id.clone()),
                        None,
                        now_rfc3339()?,
                    )?;
                    store.analysis().upsert_ai_run(&completed)?;
                    let _ = action_tx.send(reload_live_snapshot_action(
                        &config,
                        &format!(
                            "Completed AI compare {}.",
                            abbreviate_id(&completed.run_id, 12)
                        ),
                        Some(&store),
                    )?);
                }
            }
        }
        Ok::<(), RingmasterError>(())
    }
    .await;

    if let Err(error) = result {
        if let Ok(store) = Store::open(&config) {
            if let Ok(failed_record) =
                failed_ai_run_record(&failed_base_record, AiRunStatus::Failed, error.to_string())
            {
                let _ = store.analysis().upsert_ai_run(&failed_record);
            }
            let _ = action_tx.send(
                reload_live_snapshot_action(
                    &config,
                    &format!(
                        "AI run {} failed: {error}",
                        abbreviate_id(&queued_record.run_id, 12)
                    ),
                    Some(&store),
                )
                .unwrap_or_else(|_| Action::RefreshFailed {
                    message: format!("AI run failed: {error}"),
                }),
            );
        } else {
            let _ = action_tx.send(Action::RefreshFailed {
                message: format!("AI run failed before the local store could be reopened: {error}"),
            });
        }
    }
}

fn load_snapshot_from_preflight(
    preflight: &AiPreflightState,
    index: usize,
) -> Result<LoadedSnapshotArtifact> {
    let path = preflight.snapshot_paths.get(index).ok_or_else(|| {
        RingmasterError::Ui(format!(
            "AI preflight for {} is missing snapshot artifact #{index}.",
            preflight.intent.short_label()
        ))
    })?;
    snapshot::load_snapshot_artifact(Path::new(path))
}

fn load_snapshots_from_preflight(
    preflight: &AiPreflightState,
) -> Result<Vec<LoadedSnapshotArtifact>> {
    preflight
        .snapshot_paths
        .iter()
        .map(|path| snapshot::load_snapshot_artifact(Path::new(path)))
        .collect()
}

fn complete_ai_run_record(
    running_record: &AiRunRecord,
    request_preview: &ai::AiRequestPreview,
    request_fingerprint: &str,
    artifact_id: Option<String>,
    source_ai_artifact_id: Option<String>,
    ended_at: String,
) -> Result<AiRunRecord> {
    let mut completed = running_record.clone();
    AiRunStatus::Succeeded
        .as_str()
        .clone_into(&mut completed.run_status);
    completed.request_preview_json = serde_json::to_string(request_preview)?;
    completed.request_fingerprint = Some(request_fingerprint.to_owned());
    completed.artifact_id = artifact_id;
    completed.source_ai_artifact_id = source_ai_artifact_id;
    completed.error_message = None;
    completed.ended_at = Some(ended_at.clone());
    completed.updated_at = ended_at;
    Ok(completed)
}

fn failed_ai_run_record(
    base_record: &AiRunRecord,
    status: AiRunStatus,
    error_message: String,
) -> Result<AiRunRecord> {
    let ended_at = now_rfc3339()?;
    let mut failed = base_record.clone();
    status.as_str().clone_into(&mut failed.run_status);
    failed.error_message = Some(error_message);
    failed.ended_at = Some(ended_at.clone());
    failed.updated_at = ended_at;
    Ok(failed)
}

fn cancel_ai_run(config: &Config, run: &AiRunRecord) -> Result<()> {
    if !matches!(run.run_status.as_str(), "queued" | "running") {
        return Ok(());
    }
    let store = Store::open(config)?;
    let cancelled = failed_ai_run_record(
        run,
        AiRunStatus::Cancelled,
        "Cancelled from the AI workbench.".to_owned(),
    )?;
    store.analysis().upsert_ai_run(&cancelled)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_ai_run_record(
    run_id: &str,
    preflight: &AiPreflightState,
    status: AiRunStatus,
    created_at: &str,
    started_at: Option<String>,
    ended_at: Option<String>,
    artifact_id: Option<String>,
    error_message: Option<String>,
) -> Result<AiRunRecord> {
    let snapshot_hash_a = preflight
        .request_preview
        .snapshots
        .first()
        .map(|snapshot| snapshot.snapshot_hash.clone())
        .ok_or_else(|| {
            RingmasterError::Ui("AI preflight did not include a primary snapshot hash.".to_owned())
        })?;
    let snapshot_hash_b = preflight
        .request_preview
        .snapshots
        .get(1)
        .map(|snapshot| snapshot.snapshot_hash.clone());
    Ok(AiRunRecord {
        run_id: run_id.to_owned(),
        run_kind: ai_run_kind(preflight).to_owned(),
        run_status: status.as_str().to_owned(),
        provider: preflight.request_preview.provider.clone(),
        model: preflight.request_preview.model.clone(),
        reasoning_effort: None,
        request_mode: preflight.request_preview.request_mode.clone(),
        input_transport: preflight.request_preview.input_transport.clone(),
        run_mode: "real".to_owned(),
        prompt_version: preflight.request_preview.prompt_version.clone(),
        output_schema_version: preflight.request_preview.output_schema_version.clone(),
        privacy_profile: preflight.privacy_profile.as_str().to_owned(),
        snapshot_scope: preflight.snapshot_scope.clone(),
        snapshot_hash_a,
        snapshot_hash_b,
        source_ai_artifact_id: preflight.source_ai_artifact_id.clone(),
        follow_up_kind: preflight
            .follow_up_kind
            .map(|kind| kind.as_str().to_owned()),
        request_fingerprint: Some(preflight.request_preview.request_fingerprint.clone()),
        request_preview_json: serde_json::to_string(&preflight.request_preview)?,
        artifact_id,
        error_message,
        created_at: created_at.to_owned(),
        started_at,
        ended_at,
        updated_at: created_at.to_owned(),
    })
}

fn ai_run_kind(preflight: &AiPreflightState) -> &'static str {
    if preflight.follow_up_kind.is_some() {
        return "follow_up";
    }

    match preflight.intent {
        AiLaunchIntent::ReviewSelectedDay => "review",
        AiLaunchIntent::CompareSelectedWeek => "compare",
        AiLaunchIntent::ChallengeSelectedDay => "challenge",
    }
}

fn parse_day(value: &str) -> Result<Date> {
    Date::parse(
        value,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .map_err(|error| RingmasterError::Config(format!("failed to parse day `{value}`: {error}")))
}

fn format_day(value: Date) -> Result<String> {
    value
        .format(&time::macros::format_description!("[year]-[month]-[day]"))
        .map_err(|error| RingmasterError::Config(format!("failed to format snapshot day: {error}")))
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc().format(&Rfc3339).map_err(|error| {
        RingmasterError::Config(format!(
            "failed to format timestamp for AI orchestration: {error}"
        ))
    })
}

fn build_jump_to_evidence_action(config: &Config, run: &AiRunRecord) -> Result<Option<Action>> {
    let Some(artifact_id) = &run.artifact_id else {
        return Ok(None);
    };
    let store = Store::open(config)?;
    let Some(artifact_record) = store.analysis().ai_artifact(artifact_id)? else {
        return Ok(None);
    };
    let artifact = ai::parse_stored_artifact(&artifact_record)?;
    let Some(export_ref) = first_export_ref_from_artifact(&artifact) else {
        return Ok(None);
    };
    let Some((screen, day)) = resolve_export_ref_destination(&store, run, &export_ref)? else {
        return Ok(None);
    };

    Ok(Some(Action::JumpToDayAndScreen {
        day,
        screen,
        status_line: format!("Opened local evidence for {}.", export_ref),
    }))
}

fn build_eval_jump_action(eval: &AiEvalRunRecord) -> Option<Action> {
    let details = parse_persisted_eval_details(&eval.details_json)?;
    let failing_cases = details
        .cases
        .iter()
        .filter(|case| case.graders.iter().any(|grader| !grader.candidate_passed))
        .collect::<Vec<_>>();
    let candidate_cases = if failing_cases.is_empty() {
        details.cases.iter().collect::<Vec<_>>()
    } else {
        failing_cases
    };

    for case in candidate_cases {
        if let Some(snapshot_hash) = &case.snapshot_hash_a {
            return Some(Action::JumpToAiBrowserRecord {
                tab: AiBrowserTab::Snapshots,
                record_id: snapshot_hash.clone(),
                status_line: format!("Opened snapshot linked from eval case {}.", case.case_id),
            });
        }
        if let Some(ai_run_id) = &case.candidate.lineage.ai_run_id {
            return Some(Action::JumpToAiBrowserRecord {
                tab: AiBrowserTab::Runs,
                record_id: ai_run_id.clone(),
                status_line: format!("Opened AI run linked from eval case {}.", case.case_id),
            });
        }
        if let Some(report_id) = &case.candidate.lineage.report_id {
            return Some(Action::JumpToAiBrowserRecord {
                tab: AiBrowserTab::Reports,
                record_id: report_id.clone(),
                status_line: format!("Opened report linked from eval case {}.", case.case_id),
            });
        }
        if let Some(snapshot_hash) = &case.snapshot_hash_b {
            return Some(Action::JumpToAiBrowserRecord {
                tab: AiBrowserTab::Snapshots,
                record_id: snapshot_hash.clone(),
                status_line: format!(
                    "Opened comparison snapshot linked from eval case {}.",
                    case.case_id
                ),
            });
        }
        if let Some(baseline) = &case.baseline {
            if let Some(ai_run_id) = &baseline.lineage.ai_run_id {
                return Some(Action::JumpToAiBrowserRecord {
                    tab: AiBrowserTab::Runs,
                    record_id: ai_run_id.clone(),
                    status_line: format!(
                        "Opened baseline AI run linked from eval case {}.",
                        case.case_id
                    ),
                });
            }
            if let Some(report_id) = &baseline.lineage.report_id {
                return Some(Action::JumpToAiBrowserRecord {
                    tab: AiBrowserTab::Reports,
                    record_id: report_id.clone(),
                    status_line: format!(
                        "Opened baseline report linked from eval case {}.",
                        case.case_id
                    ),
                });
            }
        }
    }

    None
}

fn first_export_ref_from_artifact(artifact: &ai::StoredArtifact) -> Option<String> {
    match artifact {
        ai::StoredArtifact::Review(review) => review
            .headline_findings
            .iter()
            .chain(review.positive_findings.iter())
            .chain(review.negative_findings.iter())
            .find_map(first_export_ref_from_finding),
        ai::StoredArtifact::Compare(compare) => compare
            .material_differences
            .iter()
            .find_map(first_export_ref_from_finding)
            .or_else(|| {
                compare
                    .supporting_evidence
                    .first()
                    .map(|evidence| evidence.export_ref.clone())
            }),
        ai::StoredArtifact::FollowUp(follow_up) => follow_up
            .focal_findings
            .iter()
            .find_map(first_export_ref_from_finding),
    }
}

fn first_export_ref_from_finding(finding: &ai::ArtifactFinding) -> Option<String> {
    finding
        .evidence_refs
        .first()
        .or_else(|| finding.counterevidence_refs.first())
        .map(|evidence| evidence.export_ref.clone())
}

fn resolve_export_ref_destination(
    store: &Store,
    run: &AiRunRecord,
    export_ref: &str,
) -> Result<Option<(Screen, String)>> {
    if let Some(day) = day_from_export_ref(export_ref) {
        return Ok(Some((screen_for_export_ref(export_ref), day)));
    }

    for snapshot_record in load_run_snapshot_records(store, run)? {
        let bundle = snapshot::deserialize_snapshot_bundle(&snapshot_record.snapshot_json)?;
        if let Some(day) = search_snapshot_for_export_ref(&bundle, export_ref) {
            return Ok(Some((screen_for_export_ref(export_ref), day)));
        }
    }

    Ok(None)
}

fn day_from_export_ref(export_ref: &str) -> Option<String> {
    let mut segments = export_ref.split(':');
    match segments.next()? {
        "daily" | "activity" | "sleep_time" | "stress" | "resilience" | "cardio_age"
        | "heartrate" | "vo2_max" => segments.next().map(str::to_owned),
        "signal" => {
            let _ = segments.next()?;
            segments.next().map(str::to_owned)
        }
        _ => None,
    }
}

fn search_snapshot_for_export_ref(
    bundle: &snapshot::SnapshotBundleV1,
    export_ref: &str,
) -> Option<String> {
    bundle
        .context_events
        .iter()
        .find(|event| event.export_ref == export_ref)
        .map(|event| event.anchor_day.clone())
        .or_else(|| {
            bundle
                .pattern_summaries
                .iter()
                .find(|summary| summary.export_ref == export_ref)
                .map(|_| bundle.metadata.anchor_day.clone())
        })
        .or_else(|| {
            bundle
                .review_signals
                .iter()
                .find(|signal| signal.export_ref == export_ref)
                .map(|signal| signal.day.clone())
        })
        .or_else(|| {
            bundle
                .metrics
                .rest_mode_periods
                .iter()
                .find(|period| period.export_ref == export_ref)
                .map(|period| period.start_day.clone())
        })
}

fn screen_for_export_ref(export_ref: &str) -> Screen {
    if export_ref.starts_with("context:") {
        Screen::Timeline
    } else if export_ref.starts_with("pattern:") {
        Screen::Patterns
    } else {
        Screen::Review
    }
}

fn abbreviate_id(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        value.to_owned()
    } else {
        value.chars().take(max_len).collect()
    }
}

fn send_worker_command(worker_tx: &Option<UnboundedSender<WorkerCommand>>, command: WorkerCommand) {
    if let Some(worker_tx) = worker_tx {
        let _ = worker_tx.send(command);
    }
}

fn spawn_refresh_worker(
    config: Config,
) -> (
    UnboundedSender<WorkerCommand>,
    UnboundedReceiver<Action>,
    ThreadJoinHandle<()>,
) {
    let (command_tx, mut command_rx) = unbounded_channel();
    let (action_tx, action_rx) = unbounded_channel();
    let worker = std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = action_tx.send(Action::RefreshFailed {
                    message: format!("Background refresh could not start its runtime: {error}"),
                });
                return;
            }
        };

        runtime.block_on(async move {
            let store = match Store::open(&config) {
                Ok(store) => store,
                Err(error) => {
                    let _ = action_tx.send(Action::RefreshFailed {
                        message: format!(
                            "Background refresh could not open the local store: {error}"
                        ),
                    });
                    return;
                }
            };
            let mut pending_manual_refresh = false;

            loop {
                let sync_states = match store.sync_state().list() {
                    Ok(sync_states) => sync_states,
                    Err(error) => {
                        let _ = action_tx.send(Action::RefreshFailed {
                            message: format!(
                                "Background refresh could not load sync state: {error}"
                            ),
                        });
                        return;
                    }
                };
                let now = time::OffsetDateTime::now_utc();
                let delay = next_wake_duration(&config, &sync_states, now)
                    .unwrap_or_else(|_| Duration::from_secs(1));

                let refresh_request = if pending_manual_refresh {
                    pending_manual_refresh = false;
                    Some((SyncFamily::ALL.to_vec(), true))
                } else {
                    tokio::select! {
                        command = command_rx.recv() => match command {
                            Some(WorkerCommand::ManualRefresh) => Some((SyncFamily::ALL.to_vec(), true)),
                            Some(WorkerCommand::Shutdown) | None => None,
                        },
                        () = tokio::time::sleep(delay) => {
                            match due_families(&config, &sync_states, time::OffsetDateTime::now_utc(), false) {
                                Ok(families) if !families.is_empty() => Some((families, false)),
                                Ok(_) => continue,
                                Err(error) => {
                                    let _ = action_tx.send(Action::RefreshFailed {
                                        message: format!("Background refresh scheduling failed: {error}"),
                                    });
                                    continue;
                                }
                            }
                        }
                    }
                };

                let Some((families, manual)) = refresh_request else {
                    return;
                };
                let family_labels = families
                    .iter()
                    .map(|family| family.label().to_owned())
                    .collect::<Vec<_>>();
                let _ = action_tx.send(Action::RefreshStarted {
                    families: family_labels,
                    manual,
                });

                let refresh_result = await_inflight_refresh(
                    &mut command_rx,
                    sync_selected(
                        &config,
                        &store,
                        SyncOptions {
                            dry_run: false,
                            fixture_dir: None,
                            families,
                            trigger_source: Some("manual_sync".to_owned()),
                            trigger_detail: Some("tui refresh".to_owned()),
                        },
                    ),
                )
                .await;

                let InFlightRefreshResult::Completed {
                    result,
                    queued_manual_refresh,
                } = refresh_result
                else {
                    return;
                };
                pending_manual_refresh |= queued_manual_refresh;

                match result {
                    Ok(report) => match refresh_snapshot_action(&config, &store, report, manual) {
                        Ok(action) => {
                            let _ = action_tx.send(action);
                        }
                        Err(error) => {
                            let _ = action_tx.send(Action::RefreshFailed {
                                message: format!(
                                    "Background refresh completed but reloading the snapshot failed: {error}"
                                ),
                            });
                        }
                    },
                    Err(error) => {
                        let _ = action_tx.send(Action::RefreshFailed {
                            message: format!("Background refresh failed: {error}"),
                        });
                    }
                }
            }
        });
    });

    (command_tx, action_rx, worker)
}

async fn await_inflight_refresh<F>(
    command_rx: &mut UnboundedReceiver<WorkerCommand>,
    sync_future: F,
) -> InFlightRefreshResult<F::Output>
where
    F: Future,
{
    let mut sync_future = std::pin::pin!(sync_future);
    let mut queued_manual_refresh = false;

    loop {
        tokio::select! {
            result = &mut sync_future => {
                return InFlightRefreshResult::Completed {
                    result,
                    queued_manual_refresh,
                };
            }
            command = command_rx.recv() => match command {
                Some(WorkerCommand::ManualRefresh) => {
                    queued_manual_refresh = true;
                }
                Some(WorkerCommand::Shutdown) | None => return InFlightRefreshResult::Shutdown,
            }
        }
    }
}

fn refresh_snapshot_action(
    config: &Config,
    store: &Store,
    report: SyncReport,
    manual: bool,
) -> Result<Action> {
    reload_live_snapshot_action(config, &refresh_summary(&report, manual), Some(store))
}

fn reload_live_snapshot_action(
    config: &Config,
    summary: &str,
    store_override: Option<&Store>,
) -> Result<Action> {
    let owned_store;
    let store = if let Some(store) = store_override {
        store
    } else {
        owned_store = Store::open(config)?;
        &owned_store
    };
    let auth_status = auth::inspect_auth(config, store)?;
    let snapshot = load_live_snapshot(config, store, &auth_status)?;
    Ok(Action::LiveSnapshotLoaded {
        snapshot: Box::new(snapshot),
        summary: summary.to_owned(),
    })
}

fn refresh_summary(report: &SyncReport, manual: bool) -> String {
    let prefix = if manual {
        "Manual refresh"
    } else {
        "Background refresh"
    };
    let family_statuses = report
        .slice_reports
        .iter()
        .map(|slice| format!("{}={}", slice.sync_key, slice.status))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{prefix} finished: {family_statuses}.")
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn start() -> Result<Self> {
        enable_raw_mode().map_err(|error| RingmasterError::io("enabling raw mode", error))?;
        let mut output = stdout();
        execute!(output, EnterAlternateScreen)
            .map_err(|error| RingmasterError::io("entering alternate screen", error))?;
        let backend = CrosstermBackend::new(output);
        let terminal = Terminal::new(backend)
            .map_err(|error| RingmasterError::Ui(format!("creating terminal failed: {error}")))?;

        Ok(Self { terminal })
    }

    fn draw(&mut self, app: &AppState) -> Result<()> {
        self.terminal
            .draw(|frame| draw(frame, app))
            .map_err(|error| RingmasterError::Ui(format!("drawing terminal failed: {error}")))?;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use std::collections::HashMap;
    use std::future::pending;
    use std::time::Duration;
    use tokio::sync::mpsc::unbounded_channel;

    use crate::action::Action;
    use crate::ai::{AiRequestPreview, AiRequestPreviewSnapshot, GuidedFollowUpKind};
    use crate::app::{
        AiBrowserTab, AiLaunchIntent, AiPreflightState, RunMode, Screen, build_demo_state,
        build_live_state,
    };
    use crate::build_scenario_fixture_snapshot_apps_for_tests;
    use crate::config::{Config, LoggingConfig, OuraConfig, RefreshConfig, WebhookConfig};
    use crate::error::OuraProblem;
    use crate::oura::models::{AuthStatus, CapabilityReport};
    use crate::snapshot::PrivacyProfile;
    use crate::store::Store;
    use crate::store::queries::{
        AiRunRecord, AuthSessionRecord, DailyActivityRecord, DailyReadinessRecord,
        DailySleepRecord, PersonalInfoRecord, SyncRunStatus, SyncStateRecord,
    };
    use crate::tui::render_snapshot;
    use crate::webhook::default_desired_subscriptions;
    use std::path::{Path, PathBuf};

    fn sample_request_preview(snapshot_hash: &str) -> AiRequestPreview {
        AiRequestPreview {
            task_family: "review".to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-5-mini".to_owned(),
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            prompt_cache: "auto".to_owned(),
            prompt_version: "review_prompt_v1".to_owned(),
            output_schema_version: "ringmaster.ai.review.v1".to_owned(),
            snapshots: vec![AiRequestPreviewSnapshot {
                label: "primary".to_owned(),
                snapshot_hash: snapshot_hash.to_owned(),
                scope: "day:2026-04-08".to_owned(),
                anchor_day: "2026-04-08".to_owned(),
                privacy_profile: PrivacyProfile::Redacted,
                day_count: 1,
            }],
            snapshot_bytes: 32_768,
            approximate_input_tokens: 8_192,
            stateless: true,
            tools_disabled: true,
            includes_notes_or_free_text: true,
            content_classes: vec![
                "summary".to_owned(),
                "review_signals".to_owned(),
                "context_events".to_owned(),
            ],
            prefix_fingerprint: "preview-prefix".to_owned(),
            payload_fingerprint: "preview-payload".to_owned(),
            request_fingerprint: "preview-request".to_owned(),
        }
    }

    fn sample_ai_run_record() -> AiRunRecord {
        AiRunRecord {
            run_id: "run-review-1".to_owned(),
            run_kind: "review".to_owned(),
            run_status: "succeeded".to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-5-mini".to_owned(),
            reasoning_effort: None,
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            run_mode: "real".to_owned(),
            prompt_version: "review_prompt_v1".to_owned(),
            output_schema_version: "ringmaster.ai.review.v1".to_owned(),
            privacy_profile: PrivacyProfile::Redacted.as_str().to_owned(),
            snapshot_scope: "day:2026-04-08".to_owned(),
            snapshot_hash_a: "demo-snapshot-20260408".to_owned(),
            snapshot_hash_b: None,
            source_ai_artifact_id: Some("run-demo-review-20260408".to_owned()),
            follow_up_kind: Some(GuidedFollowUpKind::ExpandEvidence.as_str().to_owned()),
            request_fingerprint: Some("preview-request".to_owned()),
            request_preview_json: serde_json::to_string(&sample_request_preview(
                "demo-snapshot-20260408",
            ))
            .unwrap_or_else(|error| panic!("sample request preview should serialize: {error}")),
            artifact_id: Some("artifact-review-1".to_owned()),
            error_message: None,
            created_at: "2026-04-10T00:00:00Z".to_owned(),
            started_at: Some("2026-04-10T00:01:00Z".to_owned()),
            ended_at: Some("2026-04-10T00:02:00Z".to_owned()),
            updated_at: "2026-04-10T00:02:00Z".to_owned(),
        }
    }

    fn test_config() -> Config {
        Config {
            app_name: "ringmaster",
            paths: crate::config::AppPaths::from_roots(
                PathBuf::from("/home/tester"),
                PathBuf::from("/tmp/config"),
                PathBuf::from("/tmp/state"),
                PathBuf::from("/tmp/cache"),
            )
            .unwrap_or_else(|error| panic!("paths should resolve: {error}")),
            logging: LoggingConfig {
                filter: "ringmaster=info".to_owned(),
            },
            oura: OuraConfig {
                client_id: Some("test-client".to_owned()),
                client_secret: None,
                authorize_url: "https://example.invalid/auth".to_owned(),
                token_url: "https://example.invalid/token".to_owned(),
                api_base_url: "https://example.invalid/api".to_owned(),
                secret_backend: crate::config::OuraSecretBackend::Keyring,
                secret_file: PathBuf::from("/tmp/state/ringmaster/secrets/oura-tokens.json"),
                callback_bind: "127.0.0.1:8788"
                    .parse()
                    .unwrap_or_else(|error| panic!("socket address should parse in test: {error}")),
                callback_path: "/callback".to_owned(),
                requested_scopes: vec![
                    "personal".to_owned(),
                    "daily".to_owned(),
                    "heartrate".to_owned(),
                    "workout".to_owned(),
                    "tag".to_owned(),
                    "session".to_owned(),
                ],
                auth_timeout_secs: 120,
            },
            refresh: RefreshConfig {
                personal_interval_secs: 3_600,
                daily_interval_secs: 300,
                heartrate_interval_secs: 60,
                workout_interval_secs: 600,
                enhanced_tag_interval_secs: 300,
                session_interval_secs: 300,
                personal_stale_after_secs: 72 * 60 * 60,
                daily_stale_after_secs: 12 * 60 * 60,
                heartrate_stale_after_secs: 15 * 60,
                workout_stale_after_secs: 24 * 60 * 60,
                enhanced_tag_stale_after_secs: 12 * 60 * 60,
                session_stale_after_secs: 12 * 60 * 60,
                daily_history_days: 90,
                daily_overlap_days: 2,
                heartrate_history_days: 7,
                heartrate_overlap_minutes: 60,
                workout_history_days: 90,
                workout_overlap_days: 2,
                enhanced_tag_history_days: 90,
                enhanced_tag_overlap_days: 2,
                session_history_days: 90,
                session_overlap_days: 2,
                max_backoff_secs: 60 * 60,
                demo_fixture_dir: None,
            },
            webhook: WebhookConfig {
                bind: "127.0.0.1:8799".parse().unwrap(),
                path: "/webhooks/oura".to_owned(),
                public_base_url: Some("https://example.test".to_owned()),
                verification_token: Some("verify-me".to_owned()),
                signature_tolerance_secs: 300,
                heartbeat_secs: 15,
                renewal_lead_secs: 7 * 24 * 60 * 60,
                subscriptions: default_desired_subscriptions(),
            },
            ai: crate::config::AiConfig::default(),
        }
    }

    fn test_auth_status(config: &Config, granted_scopes: Vec<String>) -> AuthStatus {
        AuthStatus {
            configured: true,
            callback_url: config.oura.callback_url(),
            requested_scopes: config.oura.requested_scopes.clone(),
            granted_scopes: granted_scopes.clone(),
            missing_fields: vec!["client_secret"],
            capability_report: CapabilityReport::from_scopes(
                &config.oura.requested_scopes,
                &granted_scopes,
            ),
            auth_timeout_secs: config.oura.auth_timeout_secs,
            secret_backend: "keyring".to_owned(),
            access_token_stored: true,
            refresh_token_stored: true,
            access_token_expires_at: Some("2026-04-08T08:00:00Z".to_owned()),
            last_authenticated_at: Some("2026-04-08T03:00:00Z".to_owned()),
            last_refresh_at: Some("2026-04-08T03:30:00Z".to_owned()),
            account_id: Some("user_123".to_owned()),
            account_email: Some("fixture-user@example.com".to_owned()),
            last_error: None,
        }
    }

    fn seed_sync_state(
        store: &Store,
        sync_key: &str,
        status: SyncRunStatus,
        message: &str,
        granted_scopes: &[&str],
        last_error: Option<OuraProblem>,
    ) {
        store
            .sync_state()
            .upsert(&SyncStateRecord {
                sync_key: sync_key.to_owned(),
                status,
                cursor: None,
                last_attempted_at: "2026-04-08T03:40:00Z".to_owned(),
                last_completed_at: Some("2026-04-08T03:41:00Z".to_owned()),
                message: Some(message.to_owned()),
                granted_scopes: granted_scopes
                    .iter()
                    .map(|scope| (*scope).to_owned())
                    .collect(),
                last_error,
                failure_count: 0,
                next_attempt_after: None,
                last_trigger_source: Some("periodic_reconcile".to_owned()),
                last_trigger_detail: Some("fixture seed".to_owned()),
            })
            .unwrap_or_else(|error| panic!("sync state should seed: {error}"));
    }

    fn seed_live_rows(store: &Store) {
        store
            .imports()
            .upsert_personal_info(&PersonalInfoRecord {
                profile_id: "user_123".to_owned(),
                age: Some(34),
                weight: Some(72.4),
                height: Some(178.0),
                biological_sex: Some("male".to_owned()),
                email: Some("fixture-user@example.com".to_owned()),
                raw_cache_key: Some("personal|fixture".to_owned()),
                updated_at: "2026-04-08T03:35:00Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("personal info should seed: {error}"));
        store
            .imports()
            .upsert_daily_sleep(&DailySleepRecord {
                oura_id: None,
                day: "2026-04-08".to_owned(),
                sleep_score: Some(86),
                raw_cache_key: Some("daily_sleep|fixture".to_owned()),
                updated_at: "2026-04-08T03:35:00Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("daily sleep should seed: {error}"));
        store
            .imports()
            .upsert_daily_readiness(&DailyReadinessRecord {
                oura_id: None,
                day: "2026-04-08".to_owned(),
                readiness_score: Some(83),
                temperature_deviation: Some(0.11),
                temperature_trend_deviation: Some(0.06),
                raw_cache_key: Some("daily_readiness|fixture".to_owned()),
                updated_at: "2026-04-08T03:35:00Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("daily readiness should seed: {error}"));
        store
            .imports()
            .upsert_daily_activity(&DailyActivityRecord {
                oura_id: None,
                day: "2026-04-08".to_owned(),
                activity_score: Some(78),
                active_calories: 601,
                steps: 12_590,
                total_calories: 2_564,
                raw_cache_key: Some("daily_activity|fixture".to_owned()),
                updated_at: "2026-04-08T03:35:00Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("daily activity should seed: {error}"));
        store
            .auth()
            .upsert(&AuthSessionRecord {
                provider: "oura".to_owned(),
                account_id: Some("user_123".to_owned()),
                account_email: Some("fixture-user@example.com".to_owned()),
                token_type: "Bearer".to_owned(),
                granted_scopes: vec![
                    "personal".to_owned(),
                    "daily".to_owned(),
                    "heartrate".to_owned(),
                ],
                access_token_expires_at: Some("2026-04-08T08:00:00Z".to_owned()),
                last_authenticated_at: Some("2026-04-08T03:00:00Z".to_owned()),
                last_refresh_at: Some("2026-04-08T03:30:00Z".to_owned()),
                last_error: None,
                updated_at: "2026-04-08T03:35:00Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("auth session should seed: {error}"));
    }

    #[test]
    fn renders_demo_snapshot() {
        let config = test_config();
        let mut app = build_demo_state(&config);
        app.active_screen = Screen::Dashboard;

        let output = render_snapshot(&app, 100, 32)
            .unwrap_or_else(|error| panic!("snapshot should render: {error}"));

        assert!(output.contains("ringmaster"));
        assert!(output.contains("Connection: Connected"));
        assert!(output.contains("Latest sync:"));
        assert!(output.contains("What matters now | 2026-04-08"));
        assert!(output.contains("Capabilities"));
        assert!(output.contains("Drill-down cues"));
    }

    #[test]
    fn renders_dashboard_missing_capability_state() {
        let config = test_config();
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        seed_sync_state(
            &store,
            "oura.daily",
            SyncRunStatus::Blocked,
            "Missing `daily` scope; dashboard summary rows remain unavailable.",
            &["personal"],
            None,
        );
        let auth_status = test_auth_status(&config, vec!["personal".to_owned()]);
        let mut app = build_live_state(&config, &store, &auth_status)
            .unwrap_or_else(|error| panic!("live state should build: {error}"));
        app.active_screen = Screen::Dashboard;

        let output = render_snapshot(&app, 100, 32)
            .unwrap_or_else(|error| panic!("dashboard snapshot should render: {error}"));

        assert!(output.contains("missing scope"));
        assert!(output.contains("sleep normal is still forming."));
    }

    #[test]
    fn renders_timeline_missing_heartrate_state() {
        let config = test_config();
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        seed_sync_state(
            &store,
            "oura.heartrate",
            SyncRunStatus::Blocked,
            "Missing `heartrate` scope; timeline and trends remain stale.",
            &["personal", "daily"],
            None,
        );
        let auth_status =
            test_auth_status(&config, vec!["personal".to_owned(), "daily".to_owned()]);
        let mut app = build_live_state(&config, &store, &auth_status)
            .unwrap_or_else(|error| panic!("live state should build: {error}"));
        app.active_screen = Screen::Timeline;

        let output = render_snapshot(&app, 100, 32)
            .unwrap_or_else(|error| panic!("timeline snapshot should render: {error}"));

        assert!(output.contains("No context event is selected"));
    }

    #[test]
    fn renders_trends_empty_data_state() {
        let config = test_config();
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        seed_sync_state(
            &store,
            "oura.heartrate",
            SyncRunStatus::Success,
            "Imported 5 heartrate samples from fixture history.",
            &["personal", "daily", "heartrate"],
            None,
        );
        let auth_status = test_auth_status(
            &config,
            vec![
                "personal".to_owned(),
                "daily".to_owned(),
                "heartrate".to_owned(),
            ],
        );
        let mut app = build_live_state(&config, &store, &auth_status)
            .unwrap_or_else(|error| panic!("live state should build: {error}"));
        app.active_screen = Screen::Trends;

        let output = render_snapshot(&app, 100, 32)
            .unwrap_or_else(|error| panic!("trends snapshot should render: {error}"));

        assert!(output.contains("Analyst notes"));
        assert!(output.contains("confidence: thin"));
    }

    #[test]
    fn renders_explain_screen_with_evidence_and_caveats() {
        let config = test_config();
        let mut app = build_demo_state(&config);
        app.active_screen = Screen::Explain;

        let output = render_snapshot(&app, 110, 38)
            .unwrap_or_else(|error| panic!("explain snapshot should render: {error}"));

        assert!(output.contains("Day story for 2026-04-08"));
        assert!(output.contains("Supporting evidence"));
        assert!(output.contains("Uncertainty"));
        assert!(output.contains("Press 2 to open Timeline"));
    }

    #[test]
    fn renders_patterns_insufficient_data_state() {
        let config = test_config();
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let auth_status = test_auth_status(
            &config,
            vec![
                "personal".to_owned(),
                "daily".to_owned(),
                "heartrate".to_owned(),
                "workout".to_owned(),
                "tag".to_owned(),
                "session".to_owned(),
            ],
        );
        let mut app = build_live_state(&config, &store, &auth_status)
            .unwrap_or_else(|error| panic!("live state should build: {error}"));
        app.active_screen = Screen::Patterns;

        let output = render_snapshot(&app, 110, 32)
            .unwrap_or_else(|error| panic!("patterns snapshot should render: {error}"));

        assert!(output.contains("Not enough data yet"));
        assert!(output.contains("Patterns stay descriptive on purpose."));
    }

    #[test]
    fn renders_review_screen_with_ranked_cards() {
        let config = test_config();
        let mut app = build_demo_state(&config);
        app.active_screen = Screen::Review;

        let output = render_snapshot(&app, 120, 40)
            .unwrap_or_else(|error| panic!("review snapshot should render: {error}"));

        assert!(output.contains("Ranked observations"));
        assert!(output.contains("AI artifact"));
        assert!(output.contains("AI artifact: available"));
        assert!(output.contains("Provider / model: openai / gpt-4o-2024-08-06"));
        assert!(output.contains("Briefing detail"));
        assert!(output.contains("Readiness score"));
    }

    #[test]
    fn renders_review_screen_without_ai_artifact_when_none_is_saved() {
        let config = test_config();
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let auth_status = test_auth_status(
            &config,
            vec![
                "personal".to_owned(),
                "daily".to_owned(),
                "heartrate".to_owned(),
                "workout".to_owned(),
                "tag".to_owned(),
                "session".to_owned(),
            ],
        );
        let mut app = build_live_state(&config, &store, &auth_status)
            .unwrap_or_else(|error| panic!("live state should build: {error}"));
        app.active_screen = Screen::Review;

        let output = render_snapshot(&app, 120, 40)
            .unwrap_or_else(|error| panic!("review snapshot should render: {error}"));

        assert!(output.contains("AI artifact"));
        assert!(output.contains("AI artifact: none"));
        assert!(output.contains("No saved AI artifact is linked to this day yet."));
    }

    #[test]
    fn renders_ops_auth_and_sync_metadata() {
        let config = test_config();
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        seed_live_rows(&store);
        seed_sync_state(
            &store,
            "oura.personal",
            SyncRunStatus::Success,
            "Imported personal info for profile user_123.",
            &["personal", "daily", "heartrate"],
            None,
        );
        seed_sync_state(
            &store,
            "oura.daily",
            SyncRunStatus::Success,
            "Imported 3 daily summary rows from fixture history.",
            &["personal", "daily", "heartrate"],
            None,
        );
        seed_sync_state(
            &store,
            "oura.heartrate",
            SyncRunStatus::Failed,
            "Heartrate sync failed after a partial import.",
            &["personal", "daily", "heartrate"],
            Some(OuraProblem::new(
                Some(429),
                "rate limit reached",
                Some("retry after the minute window resets".to_owned()),
            )),
        );
        let auth_status = test_auth_status(
            &config,
            vec![
                "personal".to_owned(),
                "daily".to_owned(),
                "heartrate".to_owned(),
            ],
        );
        let mut app = build_live_state(&config, &store, &auth_status)
            .unwrap_or_else(|error| panic!("live state should build: {error}"));
        app.active_screen = Screen::Ops;

        let output = render_snapshot(&app, 120, 44)
            .unwrap_or_else(|error| panic!("ops snapshot should render: {error}"));

        assert!(output.contains("Auth state: authenticated"));
        assert!(output.contains("Secret backend: keyring"));
        assert!(output.contains("Granted scopes: personal, daily, heartrate"));
        assert!(output.contains("Database path: :memory:"));
        assert!(output.contains("Warnings [operator attention]"));
        assert!(output.contains("Warnings"));
    }

    #[test]
    fn compact_status_snapshot_keeps_auth_and_queue_diagnostics_visible() {
        let config = test_config();
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        seed_live_rows(&store);
        seed_sync_state(
            &store,
            "oura.personal",
            SyncRunStatus::Success,
            "Imported personal info for profile user_123.",
            &["personal", "daily", "heartrate"],
            None,
        );
        seed_sync_state(
            &store,
            "oura.daily",
            SyncRunStatus::Success,
            "Imported 3 daily summary rows from fixture history.",
            &["personal", "daily", "heartrate"],
            None,
        );
        seed_sync_state(
            &store,
            "oura.heartrate",
            SyncRunStatus::Failed,
            "Heartrate sync failed after a partial import.",
            &["personal", "daily", "heartrate"],
            Some(OuraProblem::new(
                Some(429),
                "rate limit reached",
                Some("retry after the minute window resets".to_owned()),
            )),
        );
        let auth_status = test_auth_status(
            &config,
            vec![
                "personal".to_owned(),
                "daily".to_owned(),
                "heartrate".to_owned(),
            ],
        );
        let mut app = build_live_state(&config, &store, &auth_status)
            .unwrap_or_else(|error| panic!("live state should build: {error}"));
        app.active_screen = Screen::Ops;

        let output = render_snapshot(&app, 90, 28)
            .unwrap_or_else(|error| panic!("compact status snapshot should render: {error}"));

        assert!(output.contains("Auth state: authenticated"));
        assert!(output.contains("Granted scopes: personal, daily, heartrate"));
        assert!(output.contains("Receiver heartbeat: missing"));
        assert!(output.contains("Invalidation queue: pending=0"));
    }

    #[test]
    fn dashboard_compact_and_wide_snapshots_have_distinct_reading_paths() {
        let config = test_config();
        let mut app = build_demo_state(&config);
        app.active_screen = Screen::Dashboard;

        let compact = render_snapshot(&app, 90, 28)
            .unwrap_or_else(|error| panic!("compact snapshot should render: {error}"));
        let wide = render_snapshot(&app, 160, 44)
            .unwrap_or_else(|error| panic!("wide snapshot should render: {error}"));

        assert!(compact.contains("Secondary detail"));
        assert!(wide.contains("Drill-down cues"));
    }

    #[test]
    fn review_snapshot_marks_selected_card_without_relying_on_color() {
        let config = test_config();
        let mut app = build_demo_state(&config);
        app.active_screen = Screen::Review;

        let output = render_snapshot(&app, 160, 44)
            .unwrap_or_else(|error| panic!("review snapshot should render: {error}"));

        assert!(output.contains("> #1"));
        assert!(output.contains("Warnings and caveats"));
    }

    #[test]
    fn compact_review_snapshot_keeps_tabs_and_multiple_cards_visible() {
        let config = test_config();
        let mut app = build_demo_state(&config);
        app.active_screen = Screen::Review;

        let output = render_snapshot(&app, 90, 28)
            .unwrap_or_else(|error| panic!("compact review snapshot should render: {error}"));

        assert!(output.contains("Today   Week   Investigate"));
        assert!(output.contains("Readiness   Sleep   Recovery"));
        assert!(output.contains("> #1"));
        assert!(output.contains("#2"));
        assert!(output.contains("AI artifact: available"));
    }

    #[test]
    fn ai_workbench_snapshot_shows_launch_points_and_trust_surface() {
        let config = test_config();
        let mut app = build_demo_state(&config);
        app.active_screen = Screen::Ai;

        let compact = render_snapshot(&app, 90, 28)
            .unwrap_or_else(|error| panic!("compact ai snapshot should render: {error}"));
        let wide = render_snapshot(&app, 160, 44)
            .unwrap_or_else(|error| panic!("wide ai snapshot should render: {error}"));

        assert!(compact.contains("AI workbench"));
        assert!(compact.contains("Launch points"));
        assert!(compact.contains("Preflight defaults"));
        assert!(wide.contains("AI workbench"));
        assert!(wide.contains("saved artifacts"));
        assert!(wide.contains("trust surface"));
    }

    #[test]
    fn ai_workbench_smoke_path_covers_disabled_preflight_and_saved_run_detail() {
        let mut disabled_config = test_config();
        disabled_config.ai.enabled = false;
        let mut disabled_app = build_demo_state(&disabled_config);
        disabled_app.active_screen = Screen::Ai;

        let disabled_output = render_snapshot(&disabled_app, 160, 44)
            .unwrap_or_else(|error| panic!("disabled ai snapshot should render: {error}"));
        assert!(disabled_output.contains("Provider is disabled."));

        let config = test_config();
        let mut preflight_app = build_demo_state(&config);
        preflight_app.active_screen = Screen::Ai;
        preflight_app.handle(Action::AiPreflightPrepared {
            preflight: Box::new(AiPreflightState {
                intent: AiLaunchIntent::ReviewSelectedDay,
                source_screen: Screen::Review,
                snapshot_scope: "day:2026-04-08".to_owned(),
                snapshot_paths: vec![
                    "/tmp/cache/ringmaster/ai-workbench/snapshots/review-20260408-redacted.json"
                        .to_owned(),
                ],
                request_preview: sample_request_preview("demo-snapshot-20260408"),
                privacy_profile: PrivacyProfile::Redacted,
                model_override: Some("gpt-5-mini".to_owned()),
                source_ai_artifact_id: Some("run-demo-review-20260408".to_owned()),
                follow_up_kind: Some(crate::ai::GuidedFollowUpKind::ExpandEvidence),
                warning_lines: Vec::new(),
                confirm_enabled: true,
            }),
            status_line: "Prepared review preflight.".to_owned(),
        });

        let preflight_output = render_snapshot(&preflight_app, 160, 44)
            .unwrap_or_else(|error| panic!("preflight ai snapshot should render: {error}"));
        assert!(preflight_output.contains("Preflight | Review"));
        assert!(preflight_output.contains("model override: gpt-5-mini"));
        assert!(preflight_output.contains("follow_up_kind: expand_evidence"));
        assert!(
            preflight_output.contains("confirm with Enter | cancel with n | cycle privacy with p")
        );

        let mut saved_run_app = build_demo_state(&config);
        saved_run_app.active_screen = Screen::Ai;

        let saved_output = render_snapshot(&saved_run_app, 160, 44)
            .unwrap_or_else(|error| panic!("saved-run ai snapshot should render: {error}"));
        assert!(saved_output.contains("Saved AI run"));
        assert!(saved_output.contains("kind: review | status: succeeded"));
    }

    #[test]
    fn ai_workbench_browser_tabs_render_snapshot_report_and_eval_details() {
        let config = test_config();
        let mut app = build_demo_state(&config);
        app.active_screen = Screen::Ai;

        app.handle(Action::NextAiBrowserTab);
        let snapshot_output = render_snapshot(&app, 160, 44)
            .unwrap_or_else(|error| panic!("snapshot browser should render: {error}"));
        assert!(snapshot_output.contains("Snapshot artifact"));
        assert!(snapshot_output.contains("trust:"));

        app.handle(Action::NextAiBrowserTab);
        let report_output = render_snapshot(&app, 160, 44)
            .unwrap_or_else(|error| panic!("report browser should render: {error}"));
        assert!(report_output.contains("Report export"));
        assert!(report_output.contains("Daily review briefing"));

        app.handle(Action::NextAiBrowserTab);
        let eval_output = render_snapshot(&app, 160, 44)
            .unwrap_or_else(|error| panic!("eval browser should render: {error}"));
        assert!(eval_output.contains("Eval run"));
        assert!(eval_output.contains("fixture_manifest: tests/fixtures/ai"));
        assert!(eval_output.contains("baseline_vs_candidate: regressions=1"));
    }

    #[test]
    fn ops_snapshot_renders_eval_health() {
        let config = test_config();
        let mut app = build_demo_state(&config);
        app.active_screen = Screen::Ops;

        let output = render_snapshot(&app, 160, 44)
            .unwrap_or_else(|error| panic!("ops snapshot should render: {error}"));
        assert!(output.contains("Latest eval"));
        assert!(output.contains("Eval health"));
        assert!(output.contains("failed_cases=1 regressions=1 improvements=1"));
    }

    #[tokio::test]
    async fn renders_scenario_fixture_matrix_across_compact_and_wide() {
        let config = test_config();
        let states = build_scenario_fixture_snapshot_apps_for_tests(
            &config,
            Path::new("tests/fixtures/phase7"),
        )
        .await
        .unwrap_or_else(|error| panic!("scenario fixture apps should build: {error}"));

        for (scenario, mut app) in states {
            let scenario_marker = format!("Scenario fixture `{}`", scenario.label());

            for (screen, compact_marker, wide_marker) in [
                (Screen::Dashboard, "Now |", "What matters now"),
                (
                    Screen::Timeline,
                    "Timeline instrument",
                    "Timeline instrument",
                ),
                (Screen::Trends, "Trend windows", "Trend windows"),
                (
                    Screen::Explain,
                    "Supporting evidence",
                    "Supporting evidence",
                ),
                (Screen::Patterns, "Patterns browser", "Patterns browser"),
                (Screen::Review, "Review digest", "Review digest"),
                (Screen::Ai, "AI workbench", "AI workbench"),
                (Screen::Ops, "Status console", "Status console"),
            ] {
                app.active_screen = screen;

                for (width, height) in [(90, 28), (160, 44)] {
                    let output = render_snapshot(&app, width, height)
                        .unwrap_or_else(|error| panic!("matrix snapshot should render: {error}"));
                    let marker = if width == 90 {
                        compact_marker
                    } else {
                        wide_marker
                    };

                    assert!(
                        output.contains(&scenario_marker),
                        "scenario marker missing for {:?} {:?} {}x{}",
                        scenario,
                        screen,
                        width,
                        height
                    );
                    assert!(
                        output.contains(marker),
                        "screen marker `{marker}` missing for {:?} {:?} {}x{}",
                        scenario,
                        screen,
                        width,
                        height
                    );
                }
            }
        }
    }

    #[test]
    fn maps_contextual_keys_to_screen_actions() {
        let press = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));

        assert_eq!(
            super::map_event(Screen::Timeline, press(KeyCode::Char('['))),
            Some(Action::PreviousDay)
        );
        assert_eq!(
            super::map_event(Screen::Timeline, press(KeyCode::Char('.'))),
            Some(Action::NextTimelinePoint)
        );
        assert_eq!(
            super::map_event(Screen::Trends, press(KeyCode::Char(']'))),
            Some(Action::NextTrendWindow)
        );
        assert_eq!(
            super::map_event(Screen::Dashboard, press(KeyCode::Char('['))),
            Some(Action::PreviousDay)
        );
        assert_eq!(
            super::map_event(Screen::Dashboard, press(KeyCode::Char('a'))),
            Some(Action::RequestAiLaunch(AiLaunchIntent::ReviewSelectedDay))
        );
        assert_eq!(
            super::map_event(Screen::Dashboard, press(KeyCode::Char('c'))),
            Some(Action::RequestAiLaunch(AiLaunchIntent::CompareSelectedWeek))
        );
        assert_eq!(
            super::map_event(Screen::Patterns, press(KeyCode::Char('m'))),
            Some(Action::CyclePatternMetric)
        );
        assert_eq!(
            super::map_event(Screen::Patterns, press(KeyCode::Char('c'))),
            Some(Action::RequestAiLaunch(AiLaunchIntent::CompareSelectedWeek))
        );
        assert_eq!(
            super::map_event(Screen::Explain, press(KeyCode::Char('j'))),
            Some(Action::NextEvent)
        );
        assert_eq!(
            super::map_event(Screen::Explain, press(KeyCode::Char('a'))),
            Some(Action::RequestAiLaunch(AiLaunchIntent::ReviewSelectedDay))
        );
        assert_eq!(
            super::map_event(Screen::Review, press(KeyCode::Char('6'))),
            Some(Action::ShowScreen(Screen::Review))
        );
        assert_eq!(
            super::map_event(Screen::Review, press(KeyCode::Char('7'))),
            Some(Action::ShowScreen(Screen::Ai))
        );
        assert_eq!(
            super::map_event(Screen::Review, press(KeyCode::Char('8'))),
            Some(Action::ShowScreen(Screen::Ops))
        );
        assert_eq!(
            super::map_event(Screen::Review, press(KeyCode::Char('v'))),
            Some(Action::CycleReviewMode)
        );
        assert_eq!(
            super::map_event(Screen::Review, press(KeyCode::Char('f'))),
            Some(Action::CycleReviewFocus)
        );
        assert_eq!(
            super::map_event(Screen::Review, press(KeyCode::Char('j'))),
            Some(Action::NextReviewCard)
        );
        assert_eq!(
            super::map_event(Screen::Review, press(KeyCode::Char('a'))),
            Some(Action::RequestAiLaunch(AiLaunchIntent::ReviewSelectedDay))
        );
        assert_eq!(
            super::map_event(Screen::Review, press(KeyCode::Char('c'))),
            Some(Action::RequestAiLaunch(AiLaunchIntent::CompareSelectedWeek))
        );
        assert_eq!(
            super::map_event(Screen::Ai, press(KeyCode::Char('['))),
            Some(Action::PreviousAiBrowserTab)
        );
        assert_eq!(
            super::map_event(Screen::Ai, press(KeyCode::Char(']'))),
            Some(Action::NextAiBrowserTab)
        );
        assert_eq!(
            super::map_event(Screen::Ai, press(KeyCode::Char('j'))),
            Some(Action::NextAiBrowserItem)
        );
        assert_eq!(
            super::map_event(Screen::Ai, press(KeyCode::Char('k'))),
            Some(Action::PreviousAiBrowserItem)
        );
        assert_eq!(
            super::map_event(Screen::Ai, press(KeyCode::Char('a'))),
            Some(Action::RequestAiLaunch(AiLaunchIntent::ReviewSelectedDay))
        );
        assert_eq!(
            super::map_event(Screen::Ai, press(KeyCode::Char('c'))),
            Some(Action::RequestAiLaunch(AiLaunchIntent::CompareSelectedWeek))
        );
        assert_eq!(
            super::map_event(Screen::Ai, press(KeyCode::Char('n'))),
            Some(Action::DismissAiPreflight)
        );
        assert_eq!(
            super::map_event(Screen::Ai, press(KeyCode::Char('p'))),
            Some(Action::CycleAiPreflightPrivacyProfile)
        );
        assert_eq!(
            super::map_event(Screen::Ai, press(KeyCode::Char('x'))),
            Some(Action::RequestCancelAiRun)
        );
        assert_eq!(
            super::map_event(Screen::Ai, press(KeyCode::Enter)),
            Some(Action::ConfirmAiPreflight)
        );
    }

    #[test]
    fn ctrl_c_keeps_quit_priority_over_compare_shortcut() {
        let ctrl_c = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

        for screen in [
            Screen::Dashboard,
            Screen::Patterns,
            Screen::Review,
            Screen::Ai,
        ] {
            assert_eq!(super::map_event(screen, ctrl_c.clone()), Some(Action::Quit));
        }
    }

    #[test]
    fn report_export_paths_are_unique_per_request() {
        let config = test_config();
        let source = super::ReportSourceSelection::Snapshot("snapshot-hash".to_owned());

        let first = super::report_output_path(&config, &source)
            .unwrap_or_else(|error| panic!("first report path should build: {error}"));
        let second = super::report_output_path(&config, &source)
            .unwrap_or_else(|error| panic!("second report path should build: {error}"));

        assert_ne!(first, second);
    }

    #[test]
    fn failed_run_record_preserves_started_at_from_running_state() {
        let preflight = AiPreflightState {
            intent: AiLaunchIntent::ReviewSelectedDay,
            source_screen: Screen::Review,
            snapshot_scope: "day:2026-04-08".to_owned(),
            snapshot_paths: vec!["/tmp/review-20260408-redacted.json".to_owned()],
            request_preview: sample_request_preview("demo-snapshot-20260408"),
            privacy_profile: PrivacyProfile::Redacted,
            model_override: None,
            source_ai_artifact_id: None,
            follow_up_kind: None,
            warning_lines: Vec::new(),
            confirm_enabled: true,
        };
        let running = super::build_ai_run_record(
            "run-review-1",
            &preflight,
            super::AiRunStatus::Running,
            "2026-04-10T00:00:00Z",
            Some("2026-04-10T00:01:00Z".to_owned()),
            None,
            None,
            None,
        )
        .unwrap_or_else(|error| panic!("running record should build: {error}"));

        let failed =
            super::failed_ai_run_record(&running, super::AiRunStatus::Failed, "boom".to_owned())
                .unwrap_or_else(|error| panic!("failed record should build: {error}"));

        assert_eq!(failed.started_at.as_deref(), Some("2026-04-10T00:01:00Z"));
        assert_eq!(failed.error_message.as_deref(), Some("boom"));
        assert_eq!(failed.run_status, "failed");
        assert!(failed.ended_at.is_some());
    }

    #[test]
    fn saved_run_privacy_cycle_overrides_preserve_follow_up_context() {
        let preflight = AiPreflightState {
            intent: AiLaunchIntent::ChallengeSelectedDay,
            source_screen: Screen::Ai,
            snapshot_scope: "day:2026-04-08".to_owned(),
            snapshot_paths: vec!["/tmp/follow-up-20260408-redacted.json".to_owned()],
            request_preview: sample_request_preview("demo-snapshot-20260408"),
            privacy_profile: PrivacyProfile::Redacted,
            model_override: Some("gpt-5-mini".to_owned()),
            source_ai_artifact_id: Some("run-demo-review-20260408".to_owned()),
            follow_up_kind: Some(GuidedFollowUpKind::ExpandEvidence),
            warning_lines: Vec::new(),
            confirm_enabled: true,
        };

        let overrides = super::saved_run_privacy_cycle_overrides(&preflight)
            .unwrap_or_else(|| panic!("saved-run follow-up preflight should preserve overrides"));

        assert_eq!(overrides.privacy_profile, PrivacyProfile::Balanced);
        assert_eq!(overrides.model_override.as_deref(), Some("gpt-5-mini"));
        assert!(!overrides.compare_previous_snapshot);
    }

    #[test]
    fn compare_previous_snapshot_overrides_keep_current_privacy_profile() {
        let run = sample_ai_run_record();

        let overrides = super::compare_previous_snapshot_overrides(&run)
            .unwrap_or_else(|error| panic!("compare-previous overrides should build: {error}"));

        assert_eq!(overrides.privacy_profile, PrivacyProfile::Redacted);
        assert!(overrides.model_override.is_none());
        assert!(overrides.compare_previous_snapshot);
    }

    #[test]
    fn report_export_context_tracks_demo_fixture_scope() {
        let mut config = test_config();
        config.refresh.demo_fixture_dir = Some(PathBuf::from("tests/fixtures/ai"));

        let demo_context = super::build_report_export_context(
            &config,
            RunMode::Demo,
            super::ReportSourceSelection::Snapshot("snapshot-123".to_owned()),
            PathBuf::from("/tmp/demo-report.md"),
        );
        assert!(demo_context.args.demo);
        assert_eq!(
            demo_context.args.fixture_dir,
            Some(PathBuf::from("tests/fixtures/ai"))
        );
        assert_eq!(
            demo_context.args.from_snapshot.as_deref(),
            Some("snapshot-123")
        );
        assert!(demo_context.args.from_ai_run.is_none());

        let live_context = super::build_report_export_context(
            &config,
            RunMode::Live,
            super::ReportSourceSelection::AiArtifact("artifact-123".to_owned()),
            PathBuf::from("/tmp/live-report.md"),
        );
        assert!(!live_context.args.demo);
        assert_eq!(live_context.args.fixture_dir, None);
        assert_eq!(
            live_context.args.from_ai_run.as_deref(),
            Some("artifact-123")
        );
        assert!(live_context.args.from_snapshot.is_none());
    }

    #[test]
    fn run_controls_require_runs_tab_and_fail_closed_on_other_tabs() {
        assert!(super::ai_run_controls_require_runs_tab(
            &Action::RequestCancelAiRun
        ));
        assert!(super::ai_run_controls_require_runs_tab(
            &Action::RequestAiGuidedFollowUp(GuidedFollowUpKind::ExpandEvidence)
        ));
        assert!(super::ai_run_controls_require_runs_tab(
            &Action::RequestAiRerunNextPrivacy
        ));
        assert!(super::ai_run_controls_require_runs_tab(
            &Action::RequestAiRerunNextModel
        ));
        assert!(!super::ai_run_controls_require_runs_tab(
            &Action::RequestAiGenerateReport
        ));

        let (ai_action_tx, mut ai_action_rx) = unbounded_channel();
        let mut ai_tasks = HashMap::new();
        super::handle_ai_side_effect(
            &test_config(),
            RunMode::Live,
            Action::RequestCancelAiRun,
            Screen::Ai,
            Some("2026-04-08".to_owned()),
            None,
            AiBrowserTab::Snapshots,
            Some(sample_ai_run_record()),
            None,
            None,
            None,
            &ai_action_tx,
            &mut ai_tasks,
        )
        .unwrap_or_else(|error| panic!("non-run tab guard should not fail: {error}"));

        assert!(ai_tasks.is_empty());
        match ai_action_rx.try_recv() {
            Ok(Action::RefreshFailed { message }) => {
                assert_eq!(
                    message,
                    "Run controls only apply while browsing saved AI runs."
                );
            }
            other => panic!("expected non-runs-tab guard message, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn shutdown_preempts_inflight_refresh() {
        let (command_tx, mut command_rx) = unbounded_channel();
        let send_result = command_tx.send(super::WorkerCommand::Shutdown);
        assert!(send_result.is_ok());

        let result = super::await_inflight_refresh(&mut command_rx, pending::<usize>()).await;
        assert!(matches!(result, super::InFlightRefreshResult::Shutdown));
    }

    #[tokio::test]
    async fn manual_refresh_is_queued_while_sync_is_inflight() {
        let (command_tx, mut command_rx) = unbounded_channel();
        let delayed_tx = command_tx.clone();
        let sender = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let _ = delayed_tx.send(super::WorkerCommand::ManualRefresh);
        });

        let result = super::await_inflight_refresh(&mut command_rx, async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            7usize
        })
        .await;

        sender
            .await
            .unwrap_or_else(|error| panic!("sender task should complete: {error}"));

        match result {
            super::InFlightRefreshResult::Completed {
                result,
                queued_manual_refresh,
            } => {
                assert_eq!(result, 7);
                assert!(queued_manual_refresh);
            }
            super::InFlightRefreshResult::Shutdown => {
                panic!("refresh should complete instead of shutting down")
            }
        }
    }
}
