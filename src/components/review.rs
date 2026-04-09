use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
};

use crate::app::ReviewModel;

pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &ReviewModel) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(8),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(format!("Selected day: {}", model.selected_day_label))
            .block(Block::default().title("Review").borders(Borders::ALL)),
        layout[0],
    );

    frame.render_widget(
        Tabs::new(model.mode_tabs.iter().map(|tab| tab.label.as_str()))
            .block(Block::default().title("Mode").borders(Borders::ALL))
            .select(model.selected_mode_index),
        layout[1],
    );

    frame.render_widget(
        Tabs::new(model.focus_tabs.iter().map(|tab| tab.label.as_str()))
            .block(
                Block::default()
                    .title("Investigation Focus")
                    .borders(Borders::ALL),
            )
            .select(model.selected_focus_index),
        layout[2],
    );

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(layout[3]);

    let cards = if model.cards.is_empty() {
        vec![ListItem::new(model.empty_message.clone())]
    } else {
        model
            .cards
            .iter()
            .map(|card| {
                let prefix = if card.selected { ">" } else { " " };
                ListItem::new(format!(
                    "{} {} | {} | {}",
                    prefix, card.headline, card.section_label, card.confidence_label
                ))
            })
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(cards).block(Block::default().title("Ranked Cards").borders(Borders::ALL)),
        body[0],
    );

    let details = if model.detail_lines.is_empty() {
        vec![ListItem::new("No review details are available yet.")]
    } else {
        model
            .detail_lines
            .iter()
            .cloned()
            .map(ListItem::new)
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(details).block(Block::default().title("Evidence").borders(Borders::ALL)),
        body[1],
    );

    let warnings = if model.warning_lines.is_empty() {
        vec![ListItem::new("No review warnings.")]
    } else {
        model
            .warning_lines
            .iter()
            .cloned()
            .map(ListItem::new)
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(warnings).block(Block::default().title("Warnings").borders(Borders::ALL)),
        layout[4],
    );
}
