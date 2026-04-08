use std::io::{self, IsTerminal, Stdout, stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{CrosstermBackend, TestBackend},
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Block, Borders, Paragraph, Tabs},
};

use crate::action::Action;
use crate::app::{AppState, Screen};
use crate::components::{dashboard, ops, timeline, trends};
use crate::error::{Result, RingmasterError};

pub async fn run(app: &mut AppState) -> Result<()> {
    if !(stdout().is_terminal() && io::stdin().is_terminal()) {
        return Err(RingmasterError::Ui(
            "interactive TUI mode requires a terminal".to_owned(),
        ));
    }

    let mut session = TerminalSession::start()?;
    let tick_rate = Duration::from_millis(250);

    loop {
        session.draw(app)?;

        if app.should_quit {
            break;
        }

        if event::poll(tick_rate)
            .map_err(|error| RingmasterError::io("polling terminal events", error))?
        {
            let event = event::read()
                .map_err(|error| RingmasterError::io("reading terminal event", error))?;
            if let Some(action) = map_event(event) {
                app.handle(action);
            }
        } else {
            app.handle(Action::Tick);
        }
    }

    Ok(())
}

pub fn render_snapshot(app: &AppState, width: u16, height: u16) -> Result<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)
        .map_err(|error| RingmasterError::Ui(format!("building test terminal failed: {error}")))?;
    terminal
        .draw(|frame| draw(frame, app))
        .map_err(|error| RingmasterError::Ui(format!("drawing test terminal failed: {error}")))?;

    let buffer = terminal.backend().buffer().clone();
    let mut lines = Vec::new();

    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        lines.push(line.trim_end().to_owned());
    }

    Ok(lines.join("\n"))
}

fn draw(frame: &mut ratatui::Frame<'_>, app: &AppState) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let header = Paragraph::new(app.model.title.clone()).block(
        Block::default()
            .title("ringmaster.rs")
            .borders(Borders::ALL),
    );
    frame.render_widget(header, layout[0]);

    let tab_titles = Screen::ALL
        .into_iter()
        .map(|screen| Line::from(screen.title()))
        .collect::<Vec<_>>();
    let tabs = Tabs::new(tab_titles)
        .block(Block::default().title("Screens").borders(Borders::ALL))
        .select(app.active_tab_index());
    frame.render_widget(tabs, layout[1]);

    draw_active_screen(frame, layout[2], app);

    let footer = Paragraph::new(app.footer()).block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, layout[3]);
}

fn draw_active_screen(frame: &mut ratatui::Frame<'_>, area: Rect, app: &AppState) {
    match app.active_screen {
        Screen::Dashboard => dashboard::draw(frame, area, &app.model.dashboard),
        Screen::Timeline => timeline::draw(frame, area, &app.model.timeline),
        Screen::Trends => trends::draw(frame, area, &app.model.trends),
        Screen::Ops => ops::draw(frame, area, &app.model.ops),
    }
}

fn map_event(event: Event) -> Option<Action> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Some(Action::Quit),
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => Some(Action::NextScreen),
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => Some(Action::PreviousScreen),
            KeyCode::Char('r') => Some(Action::RefreshRequested),
            KeyCode::Char('1') => Some(Action::ShowScreen(Screen::Dashboard)),
            KeyCode::Char('2') => Some(Action::ShowScreen(Screen::Timeline)),
            KeyCode::Char('3') => Some(Action::ShowScreen(Screen::Trends)),
            KeyCode::Char('4') => Some(Action::ShowScreen(Screen::Ops)),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Action::Quit)
            }
            _ => None,
        },
        _ => None,
    }
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
    use crate::app::{Screen, build_demo_state, build_live_state};
    use crate::config::{Config, LoggingConfig, OuraConfig};
    use crate::error::OuraProblem;
    use crate::oura::models::{AuthStatus, CapabilityReport};
    use crate::store::Store;
    use crate::store::queries::{
        AuthSessionRecord, DailyActivityRecord, DailyReadinessRecord, DailySleepRecord,
        PersonalInfoRecord, SyncRunStatus, SyncStateRecord,
    };
    use crate::tui::render_snapshot;
    use std::path::PathBuf;

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
                callback_bind: "127.0.0.1:8788"
                    .parse()
                    .unwrap_or_else(|error| panic!("socket address should parse in test: {error}")),
                callback_path: "/callback".to_owned(),
                requested_scopes: vec![
                    "personal".to_owned(),
                    "daily".to_owned(),
                    "heartrate".to_owned(),
                ],
                auth_timeout_secs: 120,
            },
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
                day: "2026-04-08".to_owned(),
                sleep_score: Some(86),
                raw_cache_key: Some("daily_sleep|fixture".to_owned()),
                updated_at: "2026-04-08T03:35:00Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("daily sleep should seed: {error}"));
        store
            .imports()
            .upsert_daily_readiness(&DailyReadinessRecord {
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

        assert!(output.contains("ringmaster.rs demo"));
        assert!(output.contains("Capabilities"));
        assert!(output.contains("What Changed"));
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

        assert!(output.contains("Daily: waiting (missing scope)"));
        assert!(
            output.contains("Missing `daily` scope; dashboard summary rows remain unavailable.")
        );
        assert!(
            output.contains("Missing scopes keep some features unavailable: daily, heartrate.")
        );
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

        assert!(output.contains("Heartrate scope is missing"));
        assert!(output.contains("No cached heartrate samples yet."));
        assert!(output.contains("Workout overlays remain a pure presentation layer"));
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

        assert!(output.contains("Need cached daily rows before 7d baselines can be computed."));
        assert!(output.contains("Imported 5 heartrate samples from fixture history."));
        assert!(output.contains("Trend Sparkline"));
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

        let output = render_snapshot(&app, 120, 42)
            .unwrap_or_else(|error| panic!("ops snapshot should render: {error}"));

        assert!(output.contains("Auth state: authenticated"));
        assert!(output.contains("Secret backend: keyring"));
        assert!(output.contains("Granted scopes: personal, daily, heartrate"));
        assert!(output.contains("Database file: :memory:"));
        assert!(output.contains("oura.heartrate=failed @ 2026-04-08T03:40:00Z"));
        assert!(output.contains("Warnings"));
    }
}
