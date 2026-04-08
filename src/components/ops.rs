use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::OpsModel;

pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &OpsModel) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(6),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(format!("Mode: {}", model.mode_label))
            .block(Block::default().title("Ops Summary").borders(Borders::ALL)),
        layout[0],
    );

    let family_items = model
        .family_statuses
        .iter()
        .map(|status| {
            ListItem::new(format!(
                "{}: {} | {} | last sync {} | {}",
                status.label,
                status.state_label,
                status.scope_label,
                status.last_sync,
                status.detail
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(family_items).block(
            Block::default()
                .title("Family Status")
                .borders(Borders::ALL),
        ),
        layout[1],
    );

    let items = model
        .items
        .iter()
        .map(|item| ListItem::new(format!("{}: {}", item.label, item.value)))
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(items).block(Block::default().title("Diagnostics").borders(Borders::ALL)),
        layout[2],
    );

    let warnings = if model.warnings.is_empty() {
        vec![ListItem::new("No warnings.")]
    } else {
        model
            .warnings
            .iter()
            .cloned()
            .map(ListItem::new)
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(warnings).block(Block::default().title("Warnings").borders(Borders::ALL)),
        layout[3],
    );
}
