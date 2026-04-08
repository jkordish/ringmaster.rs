use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    widgets::{Block, Borders, List, ListItem, Paragraph, Sparkline},
};

use crate::app::TrendsModel;

pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &TrendsModel) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(6),
            Constraint::Min(8),
        ])
        .split(area);

    let window_lines = model
        .windows
        .iter()
        .map(|window| ListItem::new(format!("{}  {}", window.label, window.summary)))
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(window_lines).block(
            Block::default()
                .title("Trend Windows")
                .borders(Borders::ALL),
        ),
        layout[0],
    );

    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .title("Trend Sparkline")
                .borders(Borders::ALL),
        )
        .data(&model.sparkline);
    frame.render_widget(sparkline, layout[1]);

    let notes = model.notes.join("\n");
    frame.render_widget(
        Paragraph::new(notes).block(Block::default().title("Notes").borders(Borders::ALL)),
        layout[2],
    );
}
