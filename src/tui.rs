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
    use crate::app::{Screen, build_demo_state};
    use crate::config::{Config, LoggingConfig, OuraConfig};
    use crate::tui::render_snapshot;
    use std::path::PathBuf;

    #[test]
    fn renders_demo_snapshot() {
        let config = Config {
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
                client_id: None,
                client_secret: None,
                authorize_url: "https://example.invalid/auth".to_owned(),
                token_url: "https://example.invalid/token".to_owned(),
                callback_bind: "127.0.0.1:8788"
                    .parse()
                    .unwrap_or_else(|error| panic!("socket address should parse in test: {error}")),
                callback_path: "/callback".to_owned(),
                requested_scopes: vec!["daily".to_owned()],
                granted_scopes: Vec::new(),
                auth_timeout_secs: 120,
            },
        };
        let mut app = build_demo_state(&config);
        app.active_screen = Screen::Dashboard;

        let output = render_snapshot(&app, 100, 32)
            .unwrap_or_else(|error| panic!("snapshot should render: {error}"));

        assert!(output.contains("ringmaster.rs demo"));
        assert!(output.contains("Capabilities"));
        assert!(output.contains("What Changed"));
    }
}
