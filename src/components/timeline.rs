use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    symbols,
    text::Span,
    widgets::{Axis, Chart, Dataset, GraphType, List, ListItem, Paragraph, Tabs},
};

use crate::app::{OverlayFamilyGroup, OverlayToggleView, TimelineModel};
use crate::navigation::FocusRegion;
use crate::ui::chrome::{PanelKind, PanelShellSpec, render_panel_shell};
use crate::ui::{
    charts, chrome,
    layout::{DashboardMetrics, UiContext},
    telemetry::{TelemetryAvailability, concise_detail},
    theme::{Theme, Tone},
};

pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &TimelineModel,
    ui: &UiContext,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
) {
    let metrics = DashboardMetrics::for_viewport(ui.viewport);
    let layout = if ui.viewport.is_compact() {
        Layout::default()
            .direction(Direction::Vertical)
            .spacing(metrics.panel_gap_y)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(8),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .spacing(metrics.panel_gap_y)
            .constraints([
                Constraint::Length(5),
                Constraint::Length(3),
                Constraint::Min(13),
                Constraint::Length(7),
                Constraint::Min(10),
            ])
            .split(area)
    };

    let summary = if ui.viewport.is_compact() {
        compact_timeline_summary(model)
    } else {
        format!(
            "{}\n{}\n{}",
            model.summary, model.breadcrumb, model.day_selector
        )
    };
    let summary_shell = render_panel_shell(
        frame,
        layout[0],
        theme,
        metrics,
        PanelShellSpec {
            title: "Timeline",
            status: "DAY",
            status_tone: Tone::Accent,
            focused: false,
            expanded: false,
            kind: PanelKind::Hero,
        },
    );
    frame.render_widget(
        Paragraph::new(concise_detail(
            &summary,
            usize::from(summary_shell.content_area.width),
        ))
        .style(theme.body()),
        summary_shell.content_area,
    );

    draw_controls(
        frame,
        layout[1],
        model,
        theme,
        metrics,
        focused_region,
        expanded_region,
    );
    draw_chart(
        frame,
        layout[2],
        model,
        theme,
        metrics,
        focused_region == FocusRegion::TimelineChart,
        expanded_region == Some(FocusRegion::TimelineChart),
    );
    draw_overlay_lane(
        frame,
        layout[3],
        model,
        theme,
        metrics,
        focused_region == FocusRegion::TimelineLanes,
        expanded_region == Some(FocusRegion::TimelineLanes),
    );

    let bottom = if ui.viewport.is_compact() {
        Layout::default()
            .direction(Direction::Horizontal)
            .spacing(metrics.panel_gap_x)
            .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
            .split(layout[4])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .spacing(metrics.panel_gap_x)
            .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
            .split(layout[4])
    };

    let inspector_shell = render_panel_shell(
        frame,
        bottom[0],
        theme,
        metrics,
        PanelShellSpec {
            title: "Inspector",
            status: if model.selected_event_index.is_some() {
                "LINKED"
            } else {
                "CURSOR"
            },
            status_tone: Tone::Focus,
            focused: focused_region == FocusRegion::TimelineInspector,
            expanded: expanded_region == Some(FocusRegion::TimelineInspector),
            kind: PanelKind::Diagnostic,
        },
    );
    if ui.viewport.is_compact() {
        frame.render_widget(
            Paragraph::new(concise_detail(
                &compact_inspector_summary(model),
                usize::from(inspector_shell.content_area.width),
            )),
            inspector_shell.content_area,
        );
    } else {
        frame.render_widget(
            List::new(
                std::iter::once(model.selected_detail.clone())
                    .chain(model.event_detail_lines.iter().cloned())
                    .map(ListItem::new),
            ),
            inspector_shell.content_area,
        );
    }

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
    let event_shell = render_panel_shell(
        frame,
        bottom[1],
        theme,
        metrics,
        PanelShellSpec {
            title: "Event feed",
            status: if model.events.is_empty() {
                "EMPTY"
            } else {
                "LIVE"
            },
            status_tone: Tone::Info,
            focused: focused_region == FocusRegion::TimelineEvents,
            expanded: expanded_region == Some(FocusRegion::TimelineEvents),
            kind: PanelKind::Section,
        },
    );
    if ui.viewport.is_compact() {
        frame.render_widget(
            Paragraph::new(concise_detail(
                &compact_event_summary(model),
                usize::from(event_shell.content_area.width),
            )),
            event_shell.content_area,
        );
    } else {
        frame.render_widget(List::new(events), event_shell.content_area);
    }
}

fn draw_controls(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &TimelineModel,
    theme: &Theme,
    metrics: DashboardMetrics,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
) {
    let controls_focused = focused_region == FocusRegion::TimelineControls;
    let controls_expanded = expanded_region == Some(FocusRegion::TimelineControls);
    let controls = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(metrics.panel_gap_x)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
        .split(area);

    let window_shell = render_panel_shell(
        frame,
        controls[0],
        theme,
        metrics,
        PanelShellSpec {
            title: "Window",
            status: model
                .window_presets
                .get(model.selected_window_preset_index)
                .map_or("24H", |preset| preset.label),
            status_tone: Tone::Focus,
            focused: controls_focused,
            expanded: controls_expanded,
            kind: PanelKind::Section,
        },
    );
    frame.render_widget(
        Tabs::new(model.window_presets.iter().map(|preset| preset.label))
            .style(theme.annotation())
            .highlight_style(theme.emphasis(Tone::Focus))
            .divider(" ")
            .select(model.selected_window_preset_index),
        window_shell.content_area,
    );

    draw_overlay_tabs(
        frame,
        controls[1],
        &model.overlay_toggles,
        theme,
        metrics,
        focused_region == FocusRegion::TimelineLanes,
        expanded_region == Some(FocusRegion::TimelineLanes),
    );
}

fn draw_chart(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &TimelineModel,
    theme: &Theme,
    metrics: DashboardMetrics,
    focused: bool,
    expanded: bool,
) {
    if model.heart_rate.is_empty() {
        let shell = render_panel_shell(
            frame,
            area,
            theme,
            metrics,
            PanelShellSpec {
                title: "Heart rate",
                status: TelemetryAvailability::NoData.label(),
                status_tone: Tone::Warning,
                focused,
                expanded,
                kind: PanelKind::Section,
            },
        );
        frame.render_widget(
            Paragraph::new(model.selected_detail.clone()),
            shell.content_area,
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

    let shell = render_panel_shell(
        frame,
        area,
        theme,
        metrics,
        PanelShellSpec {
            title: "Heart rate",
            status: &format!("{}H", model.window_hours),
            status_tone: Tone::Accent,
            focused,
            expanded,
            kind: PanelKind::Section,
        },
    );
    frame.render_widget(
        Chart::new(datasets)
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
        shell.content_area,
    );
}

fn draw_overlay_lane(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &TimelineModel,
    theme: &Theme,
    metrics: DashboardMetrics,
    focused: bool,
    expanded: bool,
) {
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        metrics,
        PanelShellSpec {
            title: "Overlay lanes",
            status: if model.overlay_groups.is_empty() {
                TelemetryAvailability::NoData.label()
            } else {
                "ACTIVE"
            },
            status_tone: Tone::Info,
            focused,
            expanded,
            kind: PanelKind::Section,
        },
    );
    if area.height <= 3 {
        frame.render_widget(
            Paragraph::new(concise_detail(
                &compact_overlay_summary(model),
                usize::from(shell.content_area.width),
            )),
            shell.content_area,
        );
    } else {
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
        frame.render_widget(List::new(overlay_lines), shell.content_area);
    }
}

fn draw_overlay_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    toggles: &[OverlayToggleView],
    theme: &Theme,
    metrics: DashboardMetrics,
    focused: bool,
    expanded: bool,
) {
    let selected_index = toggles
        .iter()
        .position(|toggle| toggle.selected)
        .unwrap_or(0);
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        metrics,
        PanelShellSpec {
            title: "Overlay filters",
            status: "FILTER",
            status_tone: Tone::Info,
            focused,
            expanded,
            kind: PanelKind::Section,
        },
    );
    frame.render_widget(
        Tabs::new(toggles.iter().map(overlay_tab_label))
            .style(theme.annotation())
            .highlight_style(theme.emphasis(Tone::Focus))
            .divider(" ")
            .select(selected_index),
        shell.content_area,
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

fn compact_timeline_summary(model: &TimelineModel) -> String {
    format!("{} | {}", model.selected_day_label, model.breadcrumb)
}

fn compact_overlay_summary(model: &TimelineModel) -> String {
    if model.overlay_groups.is_empty() {
        return "[quiet] No workouts, tags, or sessions in window".to_owned();
    }

    model
        .overlay_groups
        .iter()
        .map(|group| {
            format!(
                "[{}] {} {}",
                group.glyph, group.item_count, group.family_label
            )
        })
        .collect::<Vec<_>>()
        .join(" • ")
}

fn compact_inspector_summary(model: &TimelineModel) -> String {
    if model.selected_event_index.is_some() && !model.event_detail_lines.is_empty() {
        model.event_detail_lines[0].clone()
    } else {
        model.selected_detail.clone()
    }
}

fn compact_event_summary(model: &TimelineModel) -> String {
    if model.events.is_empty() {
        return "[empty] No context events match the current filters".to_owned();
    }

    let selected_index = model
        .events
        .iter()
        .position(|event| event.selected)
        .unwrap_or(0);
    let event = &model.events[selected_index];
    let extra_count = model.events.len().saturating_sub(1);
    if extra_count == 0 {
        format!("[{}] {}", event.glyph, event.headline)
    } else {
        format!("[{}] {} | +{extra_count} more", event.glyph, event.headline)
    }
}
