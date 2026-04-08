use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::TimelineModel;

pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &TimelineModel) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(6),
        ])
        .split(area);

    let summary = Paragraph::new(model.summary.clone()).block(
        Block::default()
            .title("Timeline Summary")
            .borders(Borders::ALL),
    );
    frame.render_widget(summary, layout[0]);

    let points = if model.heart_rate.is_empty() {
        vec![ListItem::new("No cached heartrate samples yet.")]
    } else {
        model
            .heart_rate
            .iter()
            .map(|point| ListItem::new(format!("{}  {} bpm", point.label, point.bpm)))
            .collect()
    };
    frame.render_widget(
        List::new(points).block(Block::default().title("Heartrate").borders(Borders::ALL)),
        layout[1],
    );

    let overlays = model
        .overlays
        .iter()
        .cloned()
        .map(ListItem::new)
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(overlays).block(Block::default().title("Overlays").borders(Borders::ALL)),
        layout[2],
    );
}
