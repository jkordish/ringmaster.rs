use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    symbols,
    text::{Line, Span},
    widgets::{Axis, Chart, Dataset, GraphType, List, ListItem, Paragraph, Tabs},
};

use crate::app::{OverlayFamilyGroup, OverlayToggleView, TimelineModel};
use crate::ui::{
    charts,
    chrome::{self, PanelKind},
    layout::UiContext,
    theme::{Theme, Tone},
};

pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &TimelineModel,
    ui: &UiContext,
    theme: &Theme,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if ui.viewport.is_compact() { 4 } else { 5 }),
            Constraint::Length(3),
            Constraint::Min(if ui.viewport.is_compact() { 6 } else { 13 }),
            Constraint::Length(if ui.viewport.is_compact() { 4 } else { 7 }),
            Constraint::Min(if ui.viewport.is_compact() { 5 } else { 10 }),
        ])
        .split(area);

    let summary = if ui.viewport.is_compact() {
        format!("{}\n{}", model.selected_day_label, model.breadcrumb)
    } else {
        format!(
            "{}\n{}\n{}",
            model.summary, model.breadcrumb, model.day_selector
        )
    };
    frame.render_widget(
        Paragraph::new(summary)
            .style(theme.body())
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    "Timeline instrument",
                    &model.selected_day_label,
                    Tone::Accent,
                ),
                PanelKind::Hero,
            )),
        layout[0],
    );

    draw_controls(frame, layout[1], model, theme);
    draw_chart(frame, layout[2], model, theme);
    draw_overlay_lane(frame, layout[3], model, theme);

    let bottom = if ui.viewport.is_compact() {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(2)])
            .split(layout[4])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
            .split(layout[4])
    };

    frame.render_widget(
        List::new(
            std::iter::once(model.selected_detail.clone())
                .chain(model.event_detail_lines.iter().cloned())
                .map(ListItem::new),
        )
        .block(chrome::panel(
            theme,
            chrome::title_with_badge(
                theme,
                "Selected detail",
                if model.selected_event_index.is_some() {
                    "linked"
                } else {
                    "cursor"
                },
                Tone::Focus,
            ),
            PanelKind::Section,
        )),
        bottom[0],
    );

    let events = if model.events.is_empty() {
        vec![ListItem::new(
            "[empty] No context events match the current filters.",
        )]
    } else {
        model
            .events
            .iter()
            .map(|event| {
                let prefix = chrome::focus_prefix(event.selected);
                let detail = if event.detail.is_empty() {
                    String::new()
                } else {
                    format!(" | {}", event.detail)
                };
                ListItem::new(format!(
                    "{} [{}] {}{}",
                    prefix, event.glyph, event.headline, detail
                ))
            })
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(events).block(chrome::panel(
            theme,
            Line::from("Day events"),
            PanelKind::Section,
        )),
        bottom[1],
    );
}

fn draw_controls(frame: &mut Frame<'_>, area: Rect, model: &TimelineModel, theme: &Theme) {
    let controls = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
        .split(area);

    frame.render_widget(
        Tabs::new(model.window_presets.iter().map(|preset| preset.label))
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    "Window presets",
                    model
                        .window_presets
                        .get(model.selected_window_preset_index)
                        .map_or("24h", |preset| preset.label),
                    Tone::Focus,
                ),
                PanelKind::Section,
            ))
            .style(theme.annotation())
            .highlight_style(theme.emphasis(Tone::Focus))
            .divider(" ")
            .select(model.selected_window_preset_index),
        controls[0],
    );

    draw_overlay_tabs(frame, controls[1], &model.overlay_toggles, theme);
}

fn draw_chart(frame: &mut Frame<'_>, area: Rect, model: &TimelineModel, theme: &Theme) {
    if model.heart_rate.is_empty() {
        frame.render_widget(
            Paragraph::new(model.selected_detail.clone()).block(chrome::panel(
                theme,
                chrome::title_with_badge(theme, "Heartrate", "no data", Tone::Warning),
                PanelKind::Hero,
            )),
            area,
        );
        return;
    }

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
                .style(charts::line_style(theme))
                .data(segment.as_slice())
        })
        .collect::<Vec<_>>();
    if let Some(selected_dataset) = selected_dataset.as_ref() {
        datasets.push(
            Dataset::default()
                .name("selected")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Scatter)
                .style(charts::selected_point_style(theme))
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

    frame.render_widget(
        Chart::new(datasets)
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    "Heartrate",
                    &format!("{}h window", model.window_hours),
                    Tone::Accent,
                ),
                PanelKind::Hero,
            ))
            .x_axis(
                Axis::default()
                    .title("time")
                    .style(charts::baseline_style(theme))
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
                    .style(charts::baseline_style(theme))
                    .bounds([y_min, y_max])
                    .labels(vec![
                        Span::raw(format!("{y_min:.0}")),
                        Span::raw(format!("{:.0}", f64::midpoint(y_min, y_max))),
                        Span::raw(format!("{y_max:.0}")),
                    ]),
            ),
        area,
    );
}

fn draw_overlay_lane(frame: &mut Frame<'_>, area: Rect, model: &TimelineModel, theme: &Theme) {
    let overlay_lines = if model.overlay_groups.is_empty() {
        vec![ListItem::new(
            "[quiet] No workouts, tags, or sessions overlap the selected window.",
        )]
    } else {
        render_overlay_lines(
            area.width.saturating_sub(4),
            model.window_start_minute,
            model.window_end_minute,
            &model.overlay_groups,
        )
        .into_iter()
        .map(ListItem::new)
        .collect()
    };
    frame.render_widget(
        List::new(overlay_lines).block(chrome::panel(
            theme,
            chrome::title_with_badge(theme, "Overlay lanes", "temporal context", Tone::Info),
            PanelKind::Subtle,
        )),
        area,
    );
}

fn draw_overlay_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    toggles: &[OverlayToggleView],
    theme: &Theme,
) {
    let selected_index = toggles
        .iter()
        .position(|toggle| toggle.selected)
        .unwrap_or(0);
    frame.render_widget(
        Tabs::new(toggles.iter().map(overlay_tab_label))
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    "Overlay filters",
                    toggles
                        .get(selected_index)
                        .map_or("Workouts", |toggle| toggle.label),
                    Tone::Info,
                ),
                PanelKind::Subtle,
            ))
            .style(theme.annotation())
            .highlight_style(theme.emphasis(Tone::Focus))
            .divider(" ")
            .select(selected_index),
        area,
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
            "[{}] {} event(s) | {}",
            group.glyph, group.item_count, group.family_label
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
            lines.push(chars.into_iter().collect::<String>());
        }
        if hidden_rows > 0 {
            lines.push(format!("... {hidden_rows} additional lane row(s)"));
        }
    }

    lines
}

fn format_minutes(value: u16) -> String {
    let hours = value / 60;
    let minutes = value % 60;
    format!("{hours:02}:{minutes:02}")
}

fn overlay_tab_label(toggle: &OverlayToggleView) -> String {
    format!(
        "{} {}",
        toggle.label,
        if toggle.enabled { "on" } else { "off" }
    )
}
