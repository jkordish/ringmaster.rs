use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::{Color, Rect, Style},
    symbols,
    text::Span,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, List, ListItem, Paragraph},
};

use crate::app::{OverlayFamilyGroup, TimelineModel};

pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &TimelineModel) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(8),
            Constraint::Length(11),
        ])
        .split(area);

    let summary = Paragraph::new(model.summary.clone())
        .block(Block::default().title("Timeline").borders(Borders::ALL));
    frame.render_widget(summary, layout[0]);

    let toggle_summary = model
        .overlay_toggles
        .iter()
        .map(|toggle| {
            format!(
                "{}={} ({})",
                toggle.label,
                if toggle.enabled { "on" } else { "off" },
                toggle.key_hint
            )
        })
        .collect::<Vec<_>>()
        .join(" | ");
    let day_selector = Paragraph::new(format!("{} | {}", model.day_selector, toggle_summary))
        .block(
            Block::default()
                .title("Date And Filters")
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

        let x_min = f64::from(model.window_start_minute);
        let x_max = f64::from(model.window_end_minute.max(model.window_start_minute + 1));
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
                        Span::raw(format_minutes(model.window_start_minute)),
                        Span::raw(format_minutes(u16::midpoint(
                            model.window_start_minute,
                            model.window_end_minute,
                        ))),
                        Span::raw(format_minutes(model.window_end_minute)),
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

    let overlay_lines = if model.overlay_groups.is_empty() {
        vec![ListItem::new(
            "No workouts, tags, or sessions overlap the selected window.",
        )]
    } else {
        render_overlay_lines(
            layout[3].width.saturating_sub(4),
            model.window_start_minute,
            model.window_end_minute,
            &model.overlay_groups,
        )
        .into_iter()
        .map(ListItem::new)
        .collect()
    };
    frame.render_widget(
        List::new(overlay_lines).block(
            Block::default()
                .title("Overlay Lanes")
                .borders(Borders::ALL),
        ),
        layout[3],
    );

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(44), Constraint::Percentage(56)])
        .split(layout[4]);

    frame.render_widget(
        List::new(
            std::iter::once(model.selected_detail.clone())
                .chain(model.event_detail_lines.iter().cloned())
                .map(ListItem::new),
        )
        .block(
            Block::default()
                .title("Selected Detail")
                .borders(Borders::ALL),
        ),
        bottom[0],
    );

    let events = if model.events.is_empty() {
        vec![ListItem::new(
            "No context events match the current filters.",
        )]
    } else {
        model
            .events
            .iter()
            .map(|event| {
                let prefix = if event.selected { ">" } else { " " };
                let detail = if event.detail.is_empty() {
                    String::new()
                } else {
                    format!(" | {}", event.detail)
                };
                ListItem::new(format!(
                    "{} {} {}{}",
                    prefix, event.glyph, event.headline, detail
                ))
            })
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(events).block(Block::default().title("Day Events").borders(Borders::ALL)),
        bottom[1],
    );
}

fn render_overlay_lines(
    width: u16,
    window_start_minute: u16,
    window_end_minute: u16,
    groups: &[OverlayFamilyGroup],
) -> Vec<String> {
    let usable_width = usize::from(width.max(12));
    let total_span = u32::from(window_end_minute.saturating_sub(window_start_minute).max(1));
    let max_rows = 3usize;
    let mut lines = Vec::new();

    for group in groups {
        lines.push(format!(
            "{} [{}] {} event(s)",
            group.family_label, group.glyph, group.item_count
        ));

        let mut packed_rows: Vec<Vec<(usize, usize, bool)>> = Vec::new();
        for block in &group.blocks {
            let start = usize::from(block.start_minute.saturating_sub(window_start_minute))
                .saturating_mul(usable_width.saturating_sub(1))
                / usize::try_from(total_span).unwrap_or(1);
            let end = usize::from(block.end_minute.saturating_sub(window_start_minute))
                .saturating_mul(usable_width.saturating_sub(1))
                / usize::try_from(total_span).unwrap_or(1);
            let width = end.max(start).saturating_add(1);

            if let Some(row) = packed_rows
                .iter_mut()
                .find(|row| row.last().is_none_or(|(_, last_end, _)| start > *last_end))
            {
                row.push((start, width, block.selected));
            } else {
                packed_rows.push(vec![(start, width, block.selected)]);
            }
        }

        let hidden_rows = packed_rows.len().saturating_sub(max_rows);
        for row in packed_rows.into_iter().take(max_rows) {
            let mut chars = vec![' '; usable_width];
            for (start, width, selected) in row {
                let glyph = if selected { '#' } else { group.glyph };
                let end = start.saturating_add(width).min(usable_width);
                for cell in &mut chars[start..end] {
                    *cell = glyph;
                }
            }
            lines.push(chars.into_iter().collect());
        }
        if hidden_rows > 0 {
            lines.push(format!("+{} overlapping row(s) hidden", hidden_rows));
        }
    }

    lines
}

fn format_minutes(value: u16) -> String {
    let hours = value / 60;
    let minutes = value % 60;
    format!("{hours:02}:{minutes:02}")
}
