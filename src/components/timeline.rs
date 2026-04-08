use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::{Color, Rect, Style},
    symbols,
    text::Span,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, List, ListItem, Paragraph},
};

use crate::app::TimelineModel;

pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &TimelineModel) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(6),
        ])
        .split(area);

    let summary = Paragraph::new(model.summary.clone())
        .block(Block::default().title("Timeline").borders(Borders::ALL));
    frame.render_widget(summary, layout[0]);

    let day_selector = Paragraph::new(format!(
        "Days: {} | window={}h",
        model.day_selector, model.window_hours
    ))
    .block(
        Block::default()
            .title("Date Selector")
            .borders(Borders::ALL),
    );
    frame.render_widget(day_selector, layout[1]);

    if model.heart_rate.is_empty() {
        let empty = Paragraph::new(model.selected_detail.clone()).block(
            Block::default()
                .title("Intraday Heartrate")
                .borders(Borders::ALL),
        );
        frame.render_widget(empty, layout[2]);
    } else {
        let mut segments = Vec::new();
        let mut current_segment = Vec::new();

        for point in &model.heart_rate {
            if point.gap_before && !current_segment.is_empty() {
                segments.push(current_segment);
                current_segment = Vec::new();
            }

            current_segment.push((f64::from(point.minute_of_day), f64::from(point.bpm)));
        }

        if !current_segment.is_empty() {
            segments.push(current_segment);
        }

        let selected_dataset = model
            .selected_point_index
            .and_then(|index| model.heart_rate.get(index))
            .map(|point| vec![(f64::from(point.minute_of_day), f64::from(point.bpm))]);

        let mut datasets = segments
            .iter()
            .map(|segment| {
                Dataset::default()
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(Color::Cyan))
                    .data(segment.as_slice())
            })
            .collect::<Vec<_>>();
        if let Some(selected_dataset) = selected_dataset.as_ref() {
            datasets.push(
                Dataset::default()
                    .name("selected")
                    .marker(symbols::Marker::Dot)
                    .graph_type(GraphType::Scatter)
                    .style(Style::default().fg(Color::Yellow))
                    .data(selected_dataset.as_slice()),
            );
        }

        let x_min = model
            .heart_rate
            .first()
            .map_or(0.0, |point| f64::from(point.minute_of_day));
        let x_max = model.heart_rate.last().map_or(24.0 * 60.0, |point| {
            f64::from(point.minute_of_day).max(x_min + 1.0)
        });
        let y_min = model
            .heart_rate
            .iter()
            .map(|point| point.bpm)
            .min()
            .map_or(40.0, |value| f64::from(value.saturating_sub(5)));
        let y_max = model
            .heart_rate
            .iter()
            .map(|point| point.bpm)
            .max()
            .map_or(120.0, |value| f64::from(value.saturating_add(5)));
        let chart = Chart::new(datasets)
            .block(
                Block::default()
                    .title("Intraday Heartrate")
                    .borders(Borders::ALL),
            )
            .x_axis(
                Axis::default()
                    .title("Time")
                    .bounds([x_min, x_max])
                    .labels(vec![
                        Span::raw(format_minutes(x_min)),
                        Span::raw(format_minutes(f64::midpoint(x_min, x_max))),
                        Span::raw(format_minutes(x_max)),
                    ]),
            )
            .y_axis(
                Axis::default()
                    .title("bpm")
                    .bounds([y_min, y_max])
                    .labels(vec![
                        Span::raw(format!("{y_min:.0}")),
                        Span::raw(format!("{:.0}", f64::midpoint(y_min, y_max))),
                        Span::raw(format!("{y_max:.0}")),
                    ]),
            );
        frame.render_widget(chart, layout[2]);
    }

    let overlays = model
        .overlays
        .iter()
        .cloned()
        .map(ListItem::new)
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(overlays).block(Block::default().title("Details").borders(Borders::ALL)),
        layout[3],
    );
}

fn format_minutes(value: f64) -> String {
    let total_minutes = value.round().max(0.0) as u16;
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    format!("{hours:02}:{minutes:02}")
}
