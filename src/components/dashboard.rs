use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::{Alignment, Rect},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::DashboardModel;

pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &DashboardModel) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(4),
            Constraint::Min(7),
        ])
        .split(area);

    let score_columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(layout[0]);

    for (index, card) in model.scores.iter().enumerate() {
        let text = format!("{}\n{}\n{}", card.label, card.value, card.subtitle);
        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(Block::default().title(card.label).borders(Borders::ALL));
        frame.render_widget(paragraph, score_columns[index]);
    }

    let capability_lines = model
        .capabilities
        .iter()
        .map(|capability| {
            let status = if capability.available {
                "ready"
            } else {
                "waiting"
            };
            ListItem::new(format!(
                "{}: {} ({})",
                capability.label, status, capability.note
            ))
        })
        .collect::<Vec<_>>();

    let capability_list = List::new(capability_lines).block(
        Block::default()
            .title(format!("Capabilities | {}", model.freshness))
            .borders(Borders::ALL),
    );
    frame.render_widget(capability_list, layout[1]);

    let mut details = vec![ListItem::new(model.change_summary.clone())];
    details.extend(model.highlights.iter().cloned().map(ListItem::new));

    let detail_list =
        List::new(details).block(Block::default().title("What Changed").borders(Borders::ALL));
    frame.render_widget(detail_list, layout[2]);
}
