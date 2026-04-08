use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    widgets::{Block, Borders, List, ListItem, Paragraph, Sparkline, Tabs},
};

use crate::app::TrendsModel;

pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &TrendsModel) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(14),
            Constraint::Length(6),
        ])
        .split(area);

    let tabs = Tabs::new(model.windows.iter().map(|window| window.label))
        .block(Block::default().title("Trend Window").borders(Borders::ALL))
        .select(model.selected_window_index);
    frame.render_widget(tabs, layout[0]);

    let metric_constraints = vec![Constraint::Length(4); model.metrics.len()];
    let metric_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(metric_constraints)
        .split(layout[1]);

    for (index, metric) in model.metrics.iter().enumerate() {
        let metric_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(1)])
            .split(metric_areas[index]);
        let sparkline = Sparkline::default()
            .block(
                Block::default()
                    .title(format!(
                        "{} | {} | {}",
                        metric.label, metric.current_value, metric.confidence
                    ))
                    .borders(Borders::ALL),
            )
            .data(&metric.sparkline);
        frame.render_widget(sparkline, metric_layout[0]);
        frame.render_widget(Paragraph::new(metric.summary.clone()), metric_layout[1]);
    }

    let notes = model
        .notes
        .iter()
        .cloned()
        .map(ListItem::new)
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(notes).block(Block::default().title("Notes").borders(Borders::ALL)),
        layout[2],
    );
}
