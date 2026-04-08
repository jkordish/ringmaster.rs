use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::PatternsModel;

pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &PatternsModel) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(6),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(model.header.clone())
            .block(Block::default().title("Patterns").borders(Borders::ALL)),
        layout[0],
    );

    frame.render_widget(
        Paragraph::new(model.filter_summary.clone())
            .block(Block::default().title("Filters").borders(Borders::ALL)),
        layout[1],
    );

    let rows = if model.rows.is_empty() {
        vec![ListItem::new(model.empty_message.clone())]
    } else {
        model
            .rows
            .iter()
            .map(|row| ListItem::new(format!("{} | {}", row.headline, row.detail)))
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(rows).block(Block::default().title("Associations").borders(Borders::ALL)),
        layout[2],
    );

    frame.render_widget(
        List::new(model.notes.iter().cloned().map(ListItem::new))
            .block(Block::default().title("Notes").borders(Borders::ALL)),
        layout[3],
    );
}
