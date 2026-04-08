use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::ExplainModel;

pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &ExplainModel) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(7),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(model.headline.clone())
            .block(Block::default().title("Explain").borders(Borders::ALL)),
        layout[0],
    );

    frame.render_widget(
        List::new(model.summary_lines.iter().cloned().map(ListItem::new)).block(
            Block::default()
                .title("Selected Day Summary")
                .borders(Borders::ALL),
        ),
        layout[1],
    );

    frame.render_widget(
        List::new(model.measurement_lines.iter().cloned().map(ListItem::new)).block(
            Block::default()
                .title("What Was Measured")
                .borders(Borders::ALL),
        ),
        layout[2],
    );

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
        .split(layout[3]);

    frame.render_widget(
        List::new(model.evidence_lines.iter().cloned().map(ListItem::new))
            .block(Block::default().title("Evidence").borders(Borders::ALL)),
        middle[0],
    );

    frame.render_widget(
        List::new(model.context_lines.iter().cloned().map(ListItem::new)).block(
            Block::default()
                .title("Context Entries")
                .borders(Borders::ALL),
        ),
        middle[1],
    );

    frame.render_widget(
        List::new(model.caveat_lines.iter().cloned().map(ListItem::new))
            .block(Block::default().title("Caveats").borders(Borders::ALL)),
        layout[4],
    );
}
