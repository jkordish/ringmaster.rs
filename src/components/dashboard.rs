use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::{Alignment, Line, Rect, Span},
    widgets::{Clear, Paragraph, Wrap},
};

use crate::app::{
    DashboardBreakdownPanel, DashboardBreakdownRail, DashboardDeltaState, DashboardHistogramPanel,
    DashboardJudgedState, DashboardModel, DashboardScoreBand, DashboardScoreTile,
    DashboardSleepTile, DashboardThermometerPanel, DashboardTileState, DashboardTrendPanel,
    DashboardWeeklyHeatmap,
};
use crate::navigation::FocusRegion;
use crate::ui::{
    chrome::{PanelKind, PanelShellSpec, render_panel_shell},
    layout::{
        BreakdownLayout, DashboardChartMetrics, DashboardMetrics, OverlayLayoutSpec, UiContext,
        ViewportClass, WeeklyHeatmapMode, WeeklyTrendsLayout, content_fit_overlay_layout,
        panel_content_metrics,
    },
    telemetry::{
        MetricPanelState, TelemetryAvailability, heatmap_day_label, meter_bar, micro_histogram,
        segmented_bar, spark_strip, stacked_profile_rows,
    },
    text_fit::{
        concise_detail, concise_text, fit_badge_label, fit_breakdown_delta, fit_breakdown_label,
        fit_day_header, fit_single_line_with, fit_weekly_group_label, measure_one_line,
        support_lane_text, support_lane_text_with,
    },
    theme::{Theme, Tone},
};

#[derive(Debug, Clone, Copy)]
struct PanelRenderState {
    focused: bool,
    expanded: bool,
    metrics: DashboardMetrics,
    chart_metrics: DashboardChartMetrics,
}

#[derive(Debug, Clone, Copy)]
struct DashboardDrawContext<'a> {
    theme: &'a Theme,
    viewport: ViewportClass,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
    metrics: DashboardMetrics,
}

const fn panel_state(
    focused: bool,
    expanded: bool,
    metrics: DashboardMetrics,
    viewport: ViewportClass,
) -> PanelRenderState {
    PanelRenderState {
        focused,
        expanded,
        metrics,
        chart_metrics: DashboardChartMetrics::for_viewport(viewport),
    }
}

pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &DashboardModel,
    ui: &UiContext,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
) {
    let metrics = DashboardMetrics::for_viewport(ui.viewport);
    let ctx = DashboardDrawContext {
        theme,
        viewport: ui.viewport,
        focused_region,
        expanded_region,
        metrics,
    };
    match ui.viewport {
        ViewportClass::Compact => {
            draw_compact(frame, area, model, ctx);
        }
        ViewportClass::Medium => {
            draw_medium(frame, area, model, ctx);
        }
        ViewportClass::Wide => {
            draw_wide(frame, area, model, ctx);
        }
    }
}

pub fn draw_detail_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &DashboardModel,
    region: FocusRegion,
    theme: &Theme,
) {
    let viewport = ViewportClass::from_width(area.width);
    let metrics = DashboardMetrics::for_viewport(viewport);
    let overlay = content_fit_overlay_layout(
        area,
        metrics,
        OverlayLayoutSpec::new(78, 24)
            .with_min_size(54, 12)
            .with_content_hints(66, 14),
    );
    let detail = dashboard_detail_spec(model, region, usize::from(overlay.content_area.width));
    frame.render_widget(Clear, overlay.bounds);
    let shell = render_panel_shell(
        frame,
        overlay.bounds,
        theme,
        metrics,
        PanelShellSpec {
            title: &detail.title,
            status: &detail.status,
            status_tone: detail.status_tone,
            focused: true,
            expanded: false,
            kind: if matches!(
                region,
                FocusRegion::DashboardReadiness | FocusRegion::DashboardSleep
            ) {
                PanelKind::Hero
            } else {
                PanelKind::Section
            },
        },
    );
    frame.render_widget(
        Paragraph::new(detail.lines)
            .style(theme.body())
            .wrap(Wrap { trim: false })
            .alignment(Alignment::Left),
        shell.content_area,
    );
}

fn draw_wide(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &DashboardModel,
    ctx: DashboardDrawContext<'_>,
) {
    let DashboardDrawContext {
        theme,
        viewport,
        focused_region,
        expanded_region,
        metrics,
    } = ctx;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([
            Constraint::Length(14),
            Constraint::Length(8),
            Constraint::Min(11),
        ])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(metrics.panel_gap_x)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(38),
            Constraint::Percentage(32),
        ])
        .split(rows[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(top[2]);
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(metrics.panel_gap_x)
        .constraints([
            Constraint::Percentage(22),
            Constraint::Percentage(16),
            Constraint::Percentage(34),
            Constraint::Percentage(28),
        ])
        .split(rows[1]);
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(metrics.panel_gap_x)
        .constraints([Constraint::Percentage(64), Constraint::Percentage(36)])
        .split(rows[2]);

    render_score_tile(
        frame,
        top[0],
        "Readiness",
        &model.readiness,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardReadiness,
            expanded_region == Some(FocusRegion::DashboardReadiness),
            metrics,
            viewport,
        ),
    );
    render_sleep_tile(
        frame,
        top[1],
        &model.sleep,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardSleep,
            expanded_region == Some(FocusRegion::DashboardSleep),
            metrics,
            viewport,
        ),
    );
    render_score_tile(
        frame,
        right[0],
        "Activity",
        &model.activity,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardActivity,
            expanded_region == Some(FocusRegion::DashboardActivity),
            metrics,
            viewport,
        ),
    );

    render_trend_panel(
        frame,
        middle[0],
        "HRV Trend",
        &model.hrv,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardHrv,
            expanded_region == Some(FocusRegion::DashboardHrv),
            metrics,
            viewport,
        ),
    );
    render_temp_panel(
        frame,
        middle[1],
        &model.body_temp,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardTemp,
            expanded_region == Some(FocusRegion::DashboardTemp),
            metrics,
            viewport,
        ),
    );
    render_trend_panel(
        frame,
        middle[2],
        "Heart Rate",
        &model.heart_rate,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardHeartRate,
            expanded_region == Some(FocusRegion::DashboardHeartRate),
            metrics,
            viewport,
        ),
    );
    render_trend_panel(
        frame,
        middle[3],
        "SpO2",
        &model.spo2,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardSpo2,
            expanded_region == Some(FocusRegion::DashboardSpo2),
            metrics,
            viewport,
        ),
    );
    render_histogram_panel(
        frame,
        right[1],
        "Resp Rate",
        &model.respiratory_rate,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardRespRate,
            expanded_region == Some(FocusRegion::DashboardRespRate),
            metrics,
            viewport,
        ),
    );

    render_breakdown_panel(
        frame,
        bottom[0],
        &model.breakdown,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardBreakdown,
            expanded_region == Some(FocusRegion::DashboardBreakdown),
            metrics,
            viewport,
        ),
    );
    render_heatmap_panel(
        frame,
        bottom[1],
        &model.weekly,
        theme,
        viewport,
        panel_state(
            focused_region == FocusRegion::DashboardHeatmap,
            expanded_region == Some(FocusRegion::DashboardHeatmap),
            metrics,
            viewport,
        ),
    );
}

const fn dashboard_tile_tone(state: DashboardTileState) -> Tone {
    match state {
        DashboardTileState::Fresh => Tone::Fresh,
        DashboardTileState::BaselineOnly => Tone::Info,
        DashboardTileState::Stale => Tone::Stale,
        DashboardTileState::Unavailable => Tone::Unavailable,
    }
}

#[derive(Clone, Copy)]
struct ExplicitTileText<'a> {
    primary: &'a str,
    primary_compact: &'a str,
    secondary: &'a str,
    secondary_compact: &'a str,
    primary_tone: Tone,
}

struct DashboardDetailSpec {
    title: String,
    status: String,
    status_tone: Tone,
    lines: Vec<Line<'static>>,
}

fn render_explicit_tile_state(
    frame: &mut Frame<'_>,
    area: Rect,
    text: ExplicitTileText<'_>,
    theme: &Theme,
    alignment: Alignment,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let width = usize::from(area.width);
    let primary_line = fit_single_line_with(text.primary, width, &[text.primary_compact]).text;
    let secondary_line =
        fit_single_line_with(text.secondary, width, &[text.secondary_compact]).text;

    let mut lines = vec![Line::from(Span::styled(
        primary_line,
        theme.dominant_metric(text.primary_tone),
    ))];
    if area.height > 1 && !text.secondary.is_empty() {
        lines.push(Line::from(Span::styled(secondary_line, theme.annotation())));
    }

    render_panel_lines(
        frame,
        centered_body_area(area, lines.len()),
        lines,
        theme,
        alignment,
    );
}

fn dashboard_detail_spec(
    model: &DashboardModel,
    region: FocusRegion,
    width: usize,
) -> DashboardDetailSpec {
    let instrument_width = if width == 0 { 1 } else { width.min(42) };
    match region {
        FocusRegion::DashboardReadiness => {
            let tile = &model.readiness;
            let overlay_title = "Readiness Detail".to_owned();
            let status = tile.tile_state.badge_label().to_owned();
            let status_tone = dashboard_tile_tone(tile.tile_state);
            let summary = if matches!(
                tile.tile_state,
                DashboardTileState::Fresh | DashboardTileState::Stale
            ) {
                format!(
                    "Score {} | {}",
                    tile.primary_value,
                    score_band_label(tile.score_band)
                )
            } else {
                tile.fallback.primary.clone()
            };
            let compare = if matches!(tile.tile_state, DashboardTileState::Fresh) {
                tile.delta_label.clone()
            } else if matches!(tile.tile_state, DashboardTileState::Stale) {
                "Cached daily score while sync catches up.".to_owned()
            } else {
                tile.fallback.secondary.clone()
            };
            let context = tile
                .secondary_lines
                .iter()
                .find(|line| !line.ends_with("--"))
                .cloned()
                .unwrap_or_else(|| tile.note.clone());
            DashboardDetailSpec {
                title: overlay_title,
                status,
                status_tone,
                lines: vec![
                    detail_heading_line("Summary"),
                    detail_body_line(summary),
                    detail_body_line(meter_bar(tile.ring_fill_percent, instrument_width)),
                    detail_body_line(spark_strip(&tile.trend, instrument_width)),
                    detail_heading_line("Compare"),
                    detail_body_line(compare),
                    detail_heading_line("Context"),
                    detail_body_line(context),
                    detail_heading_line("Provenance"),
                    detail_body_line(tile.note.clone()),
                    detail_body_line(
                        "Esc closes this overlay; top nav still drives deeper screens.",
                    ),
                ],
            }
        }
        FocusRegion::DashboardSleep => {
            let tile = &model.sleep;
            let overlay_title = "Sleep Detail".to_owned();
            let status = tile.tile_state.badge_label().to_owned();
            let status_tone = dashboard_tile_tone(tile.tile_state);
            let (summary, support) = sleep_detail_summary(tile);
            let mut lines = vec![detail_heading_line("Summary"), detail_body_line(summary)];
            if matches!(tile.tile_state, DashboardTileState::Fresh) {
                lines.extend(
                    stacked_profile_rows(&tile.trend, instrument_width, 3)
                        .into_iter()
                        .map(detail_body_line),
                );
                lines.push(detail_body_line(spark_strip(&tile.trend, instrument_width)));
            }
            lines.push(detail_heading_line("Compare"));
            lines.push(detail_body_line(support));
            lines.push(detail_heading_line("Provenance"));
            lines.push(detail_body_line(tile.strip_note.clone()));
            lines.push(detail_body_line(
                "Esc closes this overlay; timeline and trends remain available from the top nav.",
            ));
            DashboardDetailSpec {
                title: overlay_title,
                status,
                status_tone,
                lines,
            }
        }
        FocusRegion::DashboardActivity => {
            let tile = &model.activity;
            let overlay_title = "Activity Detail".to_owned();
            let status = tile.tile_state.badge_label().to_owned();
            let status_tone = dashboard_tile_tone(tile.tile_state);
            let summary = if matches!(
                tile.tile_state,
                DashboardTileState::Fresh | DashboardTileState::Stale
            ) {
                format!("Today {} | {}", tile.primary_value, tile.delta_label)
            } else {
                tile.fallback.primary.clone()
            };
            let context = tile
                .secondary_lines
                .iter()
                .find(|line| !line.ends_with("--"))
                .cloned()
                .unwrap_or_else(|| tile.fallback.secondary.clone());
            DashboardDetailSpec {
                title: overlay_title,
                status,
                status_tone,
                lines: vec![
                    detail_heading_line("Summary"),
                    detail_body_line(summary),
                    detail_body_line(meter_bar(tile.ring_fill_percent, instrument_width)),
                    detail_body_line(spark_strip(&tile.trend, instrument_width)),
                    detail_heading_line("Context"),
                    detail_body_line(context),
                    detail_heading_line("Interpretation"),
                    detail_body_line(tile.note.clone()),
                    detail_body_line(
                        "Incomplete days stay visible here; the footer keeps the quick scan compact.",
                    ),
                ],
            }
        }
        FocusRegion::DashboardHrv => trend_detail_spec(
            "HRV Detail",
            &model.hrv,
            instrument_width,
            "HRV overlay keeps the recent strip visible while the footer stays terse.",
        ),
        FocusRegion::DashboardTemp => temp_detail_spec(&model.body_temp, instrument_width),
        FocusRegion::DashboardHeartRate => trend_detail_spec(
            "Heart Rate Detail",
            &model.heart_rate,
            instrument_width,
            "Heart rate now uses a quieter strip in-card so it does not outrank readiness.",
        ),
        FocusRegion::DashboardSpo2 => trend_detail_spec(
            "SpO2 Detail",
            &model.spo2,
            instrument_width,
            "Baseline-only SpO2 stays deliberate in-card and explains itself here.",
        ),
        FocusRegion::DashboardRespRate => histogram_detail_spec(
            "Respiratory Rate Detail",
            &model.respiratory_rate,
            instrument_width,
        ),
        FocusRegion::DashboardBreakdown => {
            breakdown_detail_spec(&model.breakdown, instrument_width)
        }
        FocusRegion::DashboardHeatmap => heatmap_detail_spec(&model.weekly),
        _ => DashboardDetailSpec {
            title: "Dashboard Detail".to_owned(),
            status: "DETAIL".to_owned(),
            status_tone: Tone::Focus,
            lines: vec![detail_body_line(
                "No dashboard detail is available for this region.",
            )],
        },
    }
}

fn detail_heading_line(text: impl Into<String>) -> Line<'static> {
    let text = text.into();
    Line::from(format!("[{}]", text.to_ascii_uppercase()))
}

fn detail_body_line(text: impl Into<String>) -> Line<'static> {
    Line::from(format!("  {}", text.into()))
}

fn sleep_detail_summary(tile: &DashboardSleepTile) -> (String, String) {
    let has_duration = tile.duration_label != "--";
    let has_score = tile.score_label != "score --";
    match (tile.tile_state, has_duration, has_score) {
        (DashboardTileState::Fresh, true, true) => (
            format!("{} | {}", tile.duration_label, tile.score_label),
            tile.strip_note.clone(),
        ),
        (DashboardTileState::Fresh, true, false) => (
            format!("{} | score pending", tile.duration_label),
            "Duration is present; score will fill once the overnight summary closes.".to_owned(),
        ),
        (DashboardTileState::Fresh, false, true) => (
            format!("{} | duration pending", tile.score_label),
            "The score arrived first; duration is still pending for this sleep window.".to_owned(),
        ),
        (DashboardTileState::Stale, _, _) => (
            format!("{} | {}", tile.duration_label, tile.score_label),
            "Cached sleep summary while sync catches up.".to_owned(),
        ),
        _ => (
            tile.fallback.primary.clone(),
            tile.fallback.secondary.clone(),
        ),
    }
}

fn trend_detail_spec(
    title: &str,
    panel: &DashboardTrendPanel,
    instrument_width: usize,
    note: &str,
) -> DashboardDetailSpec {
    let status = panel.tile_state.badge_label().to_owned();
    let status_tone = dashboard_tile_tone(panel.tile_state);
    let summary = if matches!(
        panel.tile_state,
        DashboardTileState::Fresh | DashboardTileState::Stale
    ) {
        panel.primary_label.clone()
    } else {
        panel.fallback.primary.clone()
    };
    let compare = if matches!(panel.tile_state, DashboardTileState::Fresh) {
        format!("{} | {}", panel.baseline_label, panel.range_label)
    } else if matches!(panel.tile_state, DashboardTileState::Stale) {
        panel.note.clone()
    } else {
        panel.fallback.secondary.clone()
    };
    let mut lines = vec![detail_heading_line("Summary"), detail_body_line(summary)];
    if matches!(panel.tile_state, DashboardTileState::Fresh) {
        lines.push(detail_body_line(spark_strip(
            &panel.values,
            instrument_width,
        )));
        lines.push(detail_body_line(micro_histogram(
            &panel.values,
            instrument_width,
        )));
    }
    lines.push(detail_heading_line("Compare"));
    lines.push(detail_body_line(compare));
    lines.push(detail_heading_line("Interpretation"));
    lines.push(detail_body_line(panel.note.clone()));
    lines.push(detail_body_line(note));
    DashboardDetailSpec {
        title: title.to_owned(),
        status,
        status_tone,
        lines,
    }
}

fn temp_detail_spec(
    panel: &DashboardThermometerPanel,
    instrument_width: usize,
) -> DashboardDetailSpec {
    let status = panel.tile_state.badge_label().to_owned();
    let status_tone = dashboard_tile_tone(panel.tile_state);
    let summary = if matches!(
        panel.tile_state,
        DashboardTileState::Fresh | DashboardTileState::Stale
    ) {
        panel.value_label.clone()
    } else {
        panel.fallback.primary.clone()
    };
    let mut lines = vec![detail_heading_line("Summary"), detail_body_line(summary)];
    if matches!(panel.tile_state, DashboardTileState::Fresh) {
        lines.extend(
            compact_temperature_lines(panel.deviation_tenths, instrument_width)
                .into_iter()
                .map(detail_body_line),
        );
    }
    lines.push(detail_heading_line("Interpretation"));
    lines.push(detail_body_line(panel.note.clone()));
    DashboardDetailSpec {
        title: "Body Temperature Detail".to_owned(),
        status,
        status_tone,
        lines,
    }
}

fn histogram_detail_spec(
    title: &str,
    panel: &DashboardHistogramPanel,
    instrument_width: usize,
) -> DashboardDetailSpec {
    let status = panel.tile_state.badge_label().to_owned();
    let status_tone = dashboard_tile_tone(panel.tile_state);
    let summary = if matches!(
        panel.tile_state,
        DashboardTileState::Fresh | DashboardTileState::Stale
    ) {
        panel.primary_label.clone()
    } else {
        panel.fallback.primary.clone()
    };
    let compare = if matches!(panel.tile_state, DashboardTileState::Fresh) {
        format!("{} | {}", panel.delta_label, panel.range_label)
    } else if matches!(panel.tile_state, DashboardTileState::Stale) {
        panel.note.clone()
    } else {
        panel.fallback.secondary.clone()
    };
    let mut lines = vec![detail_heading_line("Summary"), detail_body_line(summary)];
    if matches!(panel.tile_state, DashboardTileState::Fresh) {
        lines.push(detail_body_line(meter_bar(
            fill_percent(&panel.bars),
            instrument_width,
        )));
        lines.push(detail_body_line(micro_histogram(
            &panel.bars,
            instrument_width,
        )));
    }
    lines.push(detail_heading_line("Compare"));
    lines.push(detail_body_line(compare));
    lines.push(detail_heading_line("Interpretation"));
    lines.push(detail_body_line(panel.note.clone()));
    DashboardDetailSpec {
        title: title.to_owned(),
        status,
        status_tone,
        lines,
    }
}

fn breakdown_detail_spec(
    panel: &DashboardBreakdownPanel,
    instrument_width: usize,
) -> DashboardDetailSpec {
    let status = panel.availability.label().to_owned();
    let status_tone = panel.availability.tone();
    let selected = panel
        .rails
        .iter()
        .find(|rail| rail.selected)
        .or_else(|| panel.rails.first());
    let mut lines = vec![
        detail_heading_line("Summary"),
        detail_body_line(panel.note.clone()),
    ];
    if let Some(rail) = selected {
        lines.push(detail_heading_line("Selected driver"));
        lines.push(detail_body_line(format!(
            "{} | {}",
            rail.label, rail.delta_label
        )));
        lines.push(detail_body_line(segmented_bar(
            rail.fill_percent,
            instrument_width.clamp(10, 24),
        )));
        lines.push(detail_body_line(rail.note.clone()));
    }
    lines.push(detail_body_line(
        "Close this overlay to move between drivers with the normal dashboard focus order.",
    ));
    DashboardDetailSpec {
        title: "Readiness Breakdown Detail".to_owned(),
        status,
        status_tone,
        lines,
    }
}

fn heatmap_detail_spec(panel: &DashboardWeeklyHeatmap) -> DashboardDetailSpec {
    let status = panel.availability.label().to_owned();
    let status_tone = panel.availability.tone();
    let grid = panel.grid_for_viewport(ViewportClass::Wide);
    let mut lines = vec![
        detail_heading_line("Window"),
        detail_body_line(if panel.window_page_label.is_empty() {
            panel.window_label.clone()
        } else {
            format!("{} | {}", panel.window_label, panel.window_page_label)
        }),
        detail_heading_line("Selected day"),
        detail_body_line(panel.selected_summary_for_viewport(ViewportClass::Wide)),
        detail_heading_line("Rows"),
    ];
    lines.extend(
        panel
            .row_labels
            .iter()
            .enumerate()
            .map(|(row_index, label)| {
                let values = grid.rows.get(row_index).map_or(&[][..], Vec::as_slice);
                detail_body_line(format!(
                    "{label:<6} {}",
                    compact_heatmap_row(values, grid.selected_cell)
                ))
            }),
    );
    lines.push(detail_body_line(
        "The dashboard stays on seven-day pages; use the focused heatmap region to page windows.",
    ));
    DashboardDetailSpec {
        title: "Weekly Trends Detail".to_owned(),
        status,
        status_tone,
        lines,
    }
}

fn draw_medium(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &DashboardModel,
    ctx: DashboardDrawContext<'_>,
) {
    let DashboardDrawContext {
        theme,
        viewport,
        focused_region,
        expanded_region,
        metrics,
    } = ctx;
    let dashboard_rows = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([
            Constraint::Length(9),
            Constraint::Length(6),
            Constraint::Min(14),
        ])
        .split(area);

    let hero_row = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(metrics.panel_gap_x)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(dashboard_rows[0]);
    let vitals_row = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(metrics.panel_gap_x)
        .constraints([
            Constraint::Percentage(24),
            Constraint::Percentage(18),
            Constraint::Percentage(30),
            Constraint::Percentage(28),
        ])
        .split(dashboard_rows[1]);
    let detail_row = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(metrics.panel_gap_x)
        .constraints([Constraint::Percentage(64), Constraint::Percentage(36)])
        .split(dashboard_rows[2]);
    let right_stack = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([Constraint::Length(3), Constraint::Min(6)])
        .split(detail_row[1]);

    render_score_tile(
        frame,
        hero_row[0],
        "Readiness",
        &model.readiness,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardReadiness,
            expanded_region == Some(FocusRegion::DashboardReadiness),
            metrics,
            viewport,
        ),
    );
    render_sleep_tile(
        frame,
        hero_row[1],
        &model.sleep,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardSleep,
            expanded_region == Some(FocusRegion::DashboardSleep),
            metrics,
            viewport,
        ),
    );
    render_score_tile(
        frame,
        hero_row[2],
        "Activity",
        &model.activity,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardActivity,
            expanded_region == Some(FocusRegion::DashboardActivity),
            metrics,
            viewport,
        ),
    );

    render_trend_panel(
        frame,
        vitals_row[0],
        "HRV Trend",
        &model.hrv,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardHrv,
            expanded_region == Some(FocusRegion::DashboardHrv),
            metrics,
            viewport,
        ),
    );
    render_temp_panel(
        frame,
        vitals_row[1],
        &model.body_temp,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardTemp,
            expanded_region == Some(FocusRegion::DashboardTemp),
            metrics,
            viewport,
        ),
    );
    render_trend_panel(
        frame,
        vitals_row[2],
        "Heart Rate",
        &model.heart_rate,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardHeartRate,
            expanded_region == Some(FocusRegion::DashboardHeartRate),
            metrics,
            viewport,
        ),
    );
    render_trend_panel(
        frame,
        vitals_row[3],
        "SpO2",
        &model.spo2,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardSpo2,
            expanded_region == Some(FocusRegion::DashboardSpo2),
            metrics,
            viewport,
        ),
    );
    render_histogram_panel(
        frame,
        right_stack[0],
        "Resp Rate",
        &model.respiratory_rate,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardRespRate,
            expanded_region == Some(FocusRegion::DashboardRespRate),
            metrics,
            viewport,
        ),
    );

    render_breakdown_panel(
        frame,
        detail_row[0],
        &model.breakdown,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardBreakdown,
            expanded_region == Some(FocusRegion::DashboardBreakdown),
            metrics,
            viewport,
        ),
    );
    render_heatmap_panel(
        frame,
        right_stack[1],
        &model.weekly,
        theme,
        viewport,
        panel_state(
            focused_region == FocusRegion::DashboardHeatmap,
            expanded_region == Some(FocusRegion::DashboardHeatmap),
            metrics,
            viewport,
        ),
    );
}

fn draw_compact(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &DashboardModel,
    ctx: DashboardDrawContext<'_>,
) {
    let DashboardDrawContext {
        theme,
        viewport,
        focused_region,
        expanded_region,
        metrics,
    } = ctx;
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .spacing(0)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(5),
        ])
        .split(area);

    render_score_tile(
        frame,
        layout[0],
        "Readiness",
        &model.readiness,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardReadiness,
            expanded_region == Some(FocusRegion::DashboardReadiness),
            metrics,
            viewport,
        ),
    );
    render_sleep_tile(
        frame,
        layout[1],
        &model.sleep,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardSleep,
            expanded_region == Some(FocusRegion::DashboardSleep),
            metrics,
            viewport,
        ),
    );
    render_score_tile(
        frame,
        layout[2],
        "Activity",
        &model.activity,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardActivity,
            expanded_region == Some(FocusRegion::DashboardActivity),
            metrics,
            viewport,
        ),
    );

    let phys1 = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(metrics.panel_gap_x)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(30),
            Constraint::Percentage(35),
        ])
        .split(layout[3]);
    let phys2 = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(metrics.panel_gap_x)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[4]);
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(metrics.panel_gap_x)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(layout[5]);

    render_trend_panel(
        frame,
        phys1[0],
        "HRV Trend",
        &model.hrv,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardHrv,
            expanded_region == Some(FocusRegion::DashboardHrv),
            metrics,
            viewport,
        ),
    );
    render_temp_panel(
        frame,
        phys1[1],
        &model.body_temp,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardTemp,
            expanded_region == Some(FocusRegion::DashboardTemp),
            metrics,
            viewport,
        ),
    );
    render_trend_panel(
        frame,
        phys1[2],
        "SpO2",
        &model.spo2,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardSpo2,
            expanded_region == Some(FocusRegion::DashboardSpo2),
            metrics,
            viewport,
        ),
    );
    render_trend_panel(
        frame,
        phys2[0],
        "Heart Rate",
        &model.heart_rate,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardHeartRate,
            expanded_region == Some(FocusRegion::DashboardHeartRate),
            metrics,
            viewport,
        ),
    );
    render_histogram_panel(
        frame,
        phys2[1],
        "Resp Rate",
        &model.respiratory_rate,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardRespRate,
            expanded_region == Some(FocusRegion::DashboardRespRate),
            metrics,
            viewport,
        ),
    );
    render_breakdown_panel(
        frame,
        bottom[0],
        &model.breakdown,
        theme,
        panel_state(
            focused_region == FocusRegion::DashboardBreakdown,
            expanded_region == Some(FocusRegion::DashboardBreakdown),
            metrics,
            viewport,
        ),
    );
    render_heatmap_panel(
        frame,
        bottom[1],
        &model.weekly,
        theme,
        viewport,
        panel_state(
            focused_region == FocusRegion::DashboardHeatmap,
            expanded_region == Some(FocusRegion::DashboardHeatmap),
            metrics,
            viewport,
        ),
    );
}

fn render_score_tile(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    score_tile: &DashboardScoreTile,
    theme: &Theme,
    state: PanelRenderState,
) {
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        state.metrics,
        PanelShellSpec {
            title,
            status: score_tile.tile_state.badge_label(),
            status_tone: dashboard_tile_tone(score_tile.tile_state),
            focused: state.focused,
            expanded: state.expanded,
            kind: PanelKind::Hero,
        },
    );
    if shell.content_area.width == 0 || shell.content_area.height == 0 {
        return;
    }

    if area.height <= 3 {
        if !matches!(score_tile.tile_state, DashboardTileState::Fresh) {
            let (primary, primary_compact, secondary, secondary_compact, tone) =
                if matches!(score_tile.tile_state, DashboardTileState::Stale) {
                    (
                        score_tile.primary_value.as_str(),
                        score_tile.primary_value.as_str(),
                        score_tile.note.as_str(),
                        "Cached value",
                        Tone::Muted,
                    )
                } else {
                    (
                        score_tile.fallback.primary.as_str(),
                        score_tile.fallback.primary_compact.as_str(),
                        score_tile.fallback.secondary.as_str(),
                        score_tile.fallback.secondary_compact.as_str(),
                        dashboard_tile_tone(score_tile.tile_state),
                    )
                };
            render_explicit_tile_state(
                frame,
                shell.content_area,
                ExplicitTileText {
                    primary,
                    primary_compact,
                    secondary,
                    secondary_compact,
                    primary_tone: tone,
                },
                theme,
                Alignment::Center,
            );
            return;
        }
        let cue_tone = score_band_cue_tone(score_tile.score_band);
        render_panel_lines(
            frame,
            shell.content_area,
            vec![Line::from(vec![
                Span::styled(
                    score_tile.primary_value.clone(),
                    theme.dominant_metric(score_band_primary_tone(score_tile.score_band)),
                ),
                Span::raw(" "),
                Span::styled(
                    score_band_label(score_tile.score_band).to_owned(),
                    theme.badge(cue_tone),
                ),
            ])],
            theme,
            Alignment::Center,
        );
        return;
    }

    if !matches!(score_tile.tile_state, DashboardTileState::Fresh) {
        let (primary, primary_compact, secondary, secondary_compact, tone) =
            if matches!(score_tile.tile_state, DashboardTileState::Stale) {
                (
                    score_tile.primary_value.as_str(),
                    score_tile.primary_value.as_str(),
                    score_tile.note.as_str(),
                    "Cached value",
                    Tone::Muted,
                )
            } else {
                (
                    score_tile.fallback.primary.as_str(),
                    score_tile.fallback.primary_compact.as_str(),
                    score_tile.fallback.secondary.as_str(),
                    score_tile.fallback.secondary_compact.as_str(),
                    dashboard_tile_tone(score_tile.tile_state),
                )
            };
        render_explicit_tile_state(
            frame,
            shell.content_area,
            ExplicitTileText {
                primary,
                primary_compact,
                secondary,
                secondary_compact,
                primary_tone: tone,
            },
            theme,
            Alignment::Center,
        );
        return;
    }

    let width = usize::from(shell.content_area.width);
    let cue_tone = score_band_cue_tone(score_tile.score_band);
    let trend_width = clamped_instrument_width(width, state.metrics, 14, 24);
    let capacity = usize::from(shell.content_area.height).max(1);
    let note_slot = usize::from(state.focused || state.expanded);
    let instrument_rows = if capacity >= 6 { 2 } else { 1 };
    let secondary_budget = capacity
        .saturating_sub(instrument_rows + 2 + note_slot)
        .min(if width >= 24 { 2 } else { 1 });

    let mut lines = Vec::new();
    if instrument_rows == 2 {
        lines.push(Line::from(Span::styled(
            centered_line(width, spark_strip(&score_tile.trend, trend_width)),
            theme.status_marker(cue_tone),
        )));
    }
    let band_label = score_band_label(score_tile.score_band).to_owned();
    lines.push(Line::from(Span::styled(
        centered_line(width, meter_bar(score_tile.ring_fill_percent, trend_width)),
        theme.status_marker(cue_tone),
    )));
    lines.push(Line::from(Span::styled(
        centered_line(width, concise_text(&score_tile.primary_value, width)),
        theme.dominant_metric(score_band_primary_tone(score_tile.score_band)),
    )));
    lines.push(Line::from(vec![
        Span::styled(centered_line(width / 2, band_label), theme.badge(cue_tone)),
        Span::styled(
            centered_line(
                width.saturating_sub(width / 2),
                concise_text(&score_tile.delta_label, width / 2),
            ),
            theme.status_marker(cue_tone),
        ),
    ]));
    lines.extend(
        score_tile
            .secondary_lines
            .iter()
            .take(secondary_budget)
            .map(|line| Line::from(centered_line(width, concise_text(line, width)))),
    );
    if state.focused || state.expanded {
        lines.push(Line::from(Span::styled(
            centered_line(width, concise_detail(&score_tile.note, width)),
            theme.annotation(),
        )));
    }
    render_panel_lines(
        frame,
        centered_body_area(shell.content_area, lines.len()),
        lines,
        theme,
        Alignment::Center,
    );
}

fn render_sleep_tile(
    frame: &mut Frame<'_>,
    area: Rect,
    tile: &DashboardSleepTile,
    theme: &Theme,
    state: PanelRenderState,
) {
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        state.metrics,
        PanelShellSpec {
            title: "Sleep",
            status: tile.tile_state.badge_label(),
            status_tone: dashboard_tile_tone(tile.tile_state),
            focused: state.focused,
            expanded: state.expanded,
            kind: PanelKind::Hero,
        },
    );
    if shell.content_area.width == 0 || shell.content_area.height == 0 {
        return;
    }

    if area.height <= 3 {
        if !matches!(tile.tile_state, DashboardTileState::Fresh) {
            let (primary, primary_compact, secondary, secondary_compact, tone) =
                if matches!(tile.tile_state, DashboardTileState::Stale) {
                    (
                        tile.duration_label.as_str(),
                        tile.duration_label.as_str(),
                        tile.strip_note.as_str(),
                        "Cached sleep",
                        Tone::Muted,
                    )
                } else {
                    (
                        tile.fallback.primary.as_str(),
                        tile.fallback.primary_compact.as_str(),
                        tile.fallback.secondary.as_str(),
                        tile.fallback.secondary_compact.as_str(),
                        dashboard_tile_tone(tile.tile_state),
                    )
                };
            render_explicit_tile_state(
                frame,
                shell.content_area,
                ExplicitTileText {
                    primary,
                    primary_compact,
                    secondary,
                    secondary_compact,
                    primary_tone: tone,
                },
                theme,
                Alignment::Center,
            );
            return;
        }
        let (summary, support) = sleep_detail_summary(tile);
        render_panel_lines(
            frame,
            shell.content_area,
            vec![
                Line::from(Span::styled(
                    concise_text(&summary, usize::from(shell.content_area.width)),
                    theme.dominant_metric(score_band_primary_tone(tile.score_band)),
                )),
                Line::from(Span::styled(
                    concise_detail(&support, usize::from(shell.content_area.width)),
                    theme.annotation(),
                )),
            ],
            theme,
            Alignment::Center,
        );
        return;
    }

    if !matches!(tile.tile_state, DashboardTileState::Fresh) {
        if matches!(tile.tile_state, DashboardTileState::Stale) {
            let primary = format!("duration {}", tile.duration_label);
            render_explicit_tile_state(
                frame,
                shell.content_area,
                ExplicitTileText {
                    primary: &primary,
                    primary_compact: &tile.duration_label,
                    secondary: &tile.strip_note,
                    secondary_compact: "Cached sleep",
                    primary_tone: Tone::Muted,
                },
                theme,
                Alignment::Center,
            );
        } else {
            render_explicit_tile_state(
                frame,
                shell.content_area,
                ExplicitTileText {
                    primary: &tile.fallback.primary,
                    primary_compact: &tile.fallback.primary_compact,
                    secondary: &tile.fallback.secondary,
                    secondary_compact: &tile.fallback.secondary_compact,
                    primary_tone: dashboard_tile_tone(tile.tile_state),
                },
                theme,
                Alignment::Center,
            );
        }
        return;
    }

    let content = panel_content_metrics(
        shell.content_area,
        measured_panel_support_lane(shell.content_area, state.chart_metrics),
    );
    let width = usize::from(content.chart.area.width);
    let (summary, support) = sleep_detail_summary(tile);
    let cue_tone = score_band_cue_tone(tile.score_band);
    let band_width = clamped_instrument_width(width, state.metrics, 12, width.max(12));
    let capacity = usize::from(content.chart.area.height).max(2);
    let mut lines = vec![Line::from(Span::styled(
        centered_line(width, concise_text(&summary, width)),
        theme.dominant_metric(score_band_primary_tone(tile.score_band)),
    ))];
    lines.push(Line::from(Span::styled(
        centered_line(width, meter_bar(tile.score_fill_percent, band_width)),
        theme.status_marker(cue_tone),
    )));
    if capacity >= 3 {
        lines.push(Line::from(Span::styled(
            centered_line(width, spark_strip(&tile.trend, band_width)),
            theme.chart_ramp(3, 4),
        )));
    }
    render_panel_lines(
        frame,
        centered_body_area(content.chart.area, lines.len()),
        lines,
        theme,
        Alignment::Center,
    );
    render_support_lane(
        frame,
        content.support.area,
        &support,
        theme,
        Alignment::Center,
    );
}

fn render_trend_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    panel: &DashboardTrendPanel,
    theme: &Theme,
    state: PanelRenderState,
) {
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        state.metrics,
        PanelShellSpec {
            title,
            status: panel.tile_state.badge_label(),
            status_tone: dashboard_tile_tone(panel.tile_state),
            focused: state.focused,
            expanded: state.expanded,
            kind: PanelKind::Section,
        },
    );
    if shell.content_area.width == 0 || shell.content_area.height == 0 {
        return;
    }

    if area.height <= 3 {
        if matches!(panel.tile_state, DashboardTileState::Fresh) {
            render_panel_lines(
                frame,
                shell.content_area,
                vec![Line::from(vec![
                    Span::styled(
                        concise_text(
                            &panel.primary_label,
                            usize::from(shell.content_area.width) / 2,
                        ),
                        theme.dominant_metric(judged_primary_tone(panel.judged_state)),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        concise_text(
                            &panel.baseline_label,
                            usize::from(shell.content_area.width).saturating_div(2),
                        ),
                        theme.status_marker(delta_state_tone(panel.delta_state)),
                    ),
                ])],
                theme,
                Alignment::Left,
            );
        } else {
            let (primary, primary_compact, secondary, secondary_compact, tone) =
                if matches!(panel.tile_state, DashboardTileState::Stale) {
                    (
                        panel.primary_label.as_str(),
                        panel.primary_label.as_str(),
                        panel.note.as_str(),
                        "Cached value",
                        Tone::Muted,
                    )
                } else {
                    (
                        panel.fallback.primary.as_str(),
                        panel.fallback.primary_compact.as_str(),
                        panel.fallback.secondary.as_str(),
                        panel.fallback.secondary_compact.as_str(),
                        dashboard_tile_tone(panel.tile_state),
                    )
                };
            render_explicit_tile_state(
                frame,
                shell.content_area,
                ExplicitTileText {
                    primary,
                    primary_compact,
                    secondary,
                    secondary_compact,
                    primary_tone: tone,
                },
                theme,
                Alignment::Left,
            );
        }
        return;
    }

    if !matches!(panel.tile_state, DashboardTileState::Fresh) {
        let (primary, primary_compact, secondary, secondary_compact, tone) =
            if matches!(panel.tile_state, DashboardTileState::Stale) {
                (
                    panel.primary_label.as_str(),
                    panel.primary_label.as_str(),
                    panel.note.as_str(),
                    "Cached value",
                    Tone::Muted,
                )
            } else {
                (
                    panel.fallback.primary.as_str(),
                    panel.fallback.primary_compact.as_str(),
                    panel.fallback.secondary.as_str(),
                    panel.fallback.secondary_compact.as_str(),
                    dashboard_tile_tone(panel.tile_state),
                )
            };
        render_explicit_tile_state(
            frame,
            shell.content_area,
            ExplicitTileText {
                primary,
                primary_compact,
                secondary,
                secondary_compact,
                primary_tone: tone,
            },
            theme,
            Alignment::Left,
        );
        return;
    }

    let content = panel_content_metrics(
        shell.content_area,
        measured_panel_support_lane(shell.content_area, state.chart_metrics),
    );
    let width = usize::from(content.chart.area.width);
    let capacity = usize::from(content.chart.area.height).max(2);
    let compare_line = fit_single_line_with(
        &format!("{} | {}", panel.baseline_label, panel.range_label),
        width,
        &[&panel.baseline_label, &panel.range_label, "baseline --"],
    )
    .text;
    let primary_line = if panel.judged_state.is_some() {
        Line::from(vec![
            Span::styled(
                concise_text(&panel.primary_label, width.saturating_sub(10)),
                theme.dominant_metric(judged_primary_tone(panel.judged_state)),
            ),
            Span::raw(" "),
            Span::styled(
                judged_state_label(panel.judged_state).to_owned(),
                theme.badge(judged_badge_tone(panel.judged_state)),
            ),
        ])
    } else {
        Line::from(Span::styled(
            concise_text(&panel.primary_label, width),
            theme.dominant_metric(judged_primary_tone(panel.judged_state)),
        ))
    };
    let mut lines = vec![primary_line];
    let instrument_lines = trend_instrument_lines(title, panel, width);
    if capacity <= 2 {
        lines.push(Line::from(Span::styled(
            compare_line,
            theme.status_marker(delta_state_tone(panel.delta_state)),
        )));
    } else {
        let instrument_budget = capacity.saturating_sub(2);
        lines.extend(
            instrument_lines
                .into_iter()
                .take(instrument_budget.max(1))
                .map(|line| {
                    Line::from(Span::styled(
                        line,
                        theme.status_marker(delta_state_tone(panel.delta_state)),
                    ))
                }),
        );
        lines.push(Line::from(Span::styled(compare_line, theme.annotation())));
    }
    render_panel_lines(frame, content.chart.area, lines, theme, Alignment::Left);
    render_support_lane(
        frame,
        content.support.area,
        &panel.note,
        theme,
        Alignment::Left,
    );
}

fn render_temp_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    panel: &DashboardThermometerPanel,
    theme: &Theme,
    state: PanelRenderState,
) {
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        state.metrics,
        PanelShellSpec {
            title: if area.width < 24 { "Temp" } else { "Body Temp" },
            status: panel.tile_state.badge_label(),
            status_tone: dashboard_tile_tone(panel.tile_state),
            focused: state.focused,
            expanded: state.expanded,
            kind: PanelKind::Section,
        },
    );
    if shell.content_area.width == 0 || shell.content_area.height == 0 {
        return;
    }

    if area.height <= 3 {
        if !matches!(panel.tile_state, DashboardTileState::Fresh) {
            let (primary, primary_compact, secondary, secondary_compact, tone) =
                if matches!(panel.tile_state, DashboardTileState::Stale) {
                    (
                        panel.value_label.as_str(),
                        panel.value_label.as_str(),
                        panel.note.as_str(),
                        "Cached temp",
                        Tone::Muted,
                    )
                } else {
                    (
                        panel.fallback.primary.as_str(),
                        panel.fallback.primary_compact.as_str(),
                        panel.fallback.secondary.as_str(),
                        panel.fallback.secondary_compact.as_str(),
                        dashboard_tile_tone(panel.tile_state),
                    )
                };
            render_explicit_tile_state(
                frame,
                shell.content_area,
                ExplicitTileText {
                    primary,
                    primary_compact,
                    secondary,
                    secondary_compact,
                    primary_tone: tone,
                },
                theme,
                Alignment::Center,
            );
            return;
        }
        render_panel_lines(
            frame,
            shell.content_area,
            vec![Line::from(vec![
                Span::styled(
                    concise_text(&panel.value_label, usize::from(shell.content_area.width)),
                    theme.dominant_metric(delta_state_tone(panel.delta_state)),
                ),
                Span::raw(" "),
                Span::styled(
                    judged_state_label(panel.judged_state).to_owned(),
                    theme.badge(judged_badge_tone(panel.judged_state)),
                ),
            ])],
            theme,
            Alignment::Center,
        );
        return;
    }

    if !matches!(panel.tile_state, DashboardTileState::Fresh) {
        let (primary, primary_compact, secondary, secondary_compact, tone) =
            if matches!(panel.tile_state, DashboardTileState::Stale) {
                (
                    panel.value_label.as_str(),
                    panel.value_label.as_str(),
                    panel.note.as_str(),
                    "Cached temp",
                    Tone::Muted,
                )
            } else {
                (
                    panel.fallback.primary.as_str(),
                    panel.fallback.primary_compact.as_str(),
                    panel.fallback.secondary.as_str(),
                    panel.fallback.secondary_compact.as_str(),
                    dashboard_tile_tone(panel.tile_state),
                )
            };
        render_explicit_tile_state(
            frame,
            shell.content_area,
            ExplicitTileText {
                primary,
                primary_compact,
                secondary,
                secondary_compact,
                primary_tone: tone,
            },
            theme,
            Alignment::Center,
        );
        return;
    }

    let content = panel_content_metrics(
        shell.content_area,
        measured_panel_support_lane(shell.content_area, state.chart_metrics),
    );
    let width = usize::from(content.chart.area.width);
    let mut lines = vec![Line::from(vec![
        Span::styled(
            centered_line(
                width.saturating_sub(8),
                concise_text(&panel.value_label, width),
            ),
            theme.dominant_metric(delta_state_tone(panel.delta_state)),
        ),
        Span::raw(" "),
        Span::styled(
            judged_state_label(panel.judged_state).to_owned(),
            theme.badge(judged_badge_tone(panel.judged_state)),
        ),
    ])];
    lines.extend(
        compact_temperature_lines(panel.deviation_tenths, width)
            .into_iter()
            .map(|line| {
                Line::from(Span::styled(
                    line,
                    theme.status_marker(delta_state_tone(panel.delta_state)),
                ))
            }),
    );
    render_panel_lines(
        frame,
        centered_body_area(content.chart.area, lines.len()),
        lines,
        theme,
        Alignment::Center,
    );
    render_support_lane(
        frame,
        content.support.area,
        &panel.note,
        theme,
        Alignment::Center,
    );
}

fn render_histogram_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    panel: &DashboardHistogramPanel,
    theme: &Theme,
    state: PanelRenderState,
) {
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        state.metrics,
        PanelShellSpec {
            title,
            status: panel.tile_state.badge_label(),
            status_tone: dashboard_tile_tone(panel.tile_state),
            focused: state.focused,
            expanded: state.expanded,
            kind: PanelKind::Section,
        },
    );
    if shell.content_area.width == 0 || shell.content_area.height == 0 {
        return;
    }

    if area.height <= 3 {
        if matches!(panel.tile_state, DashboardTileState::Fresh) {
            render_panel_lines(
                frame,
                shell.content_area,
                vec![Line::from(vec![
                    Span::styled(
                        concise_text(
                            &panel.primary_label,
                            usize::from(shell.content_area.width) / 2,
                        ),
                        theme.dominant_metric(judged_primary_tone(panel.judged_state)),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        concise_text(
                            &panel.delta_label,
                            usize::from(shell.content_area.width).saturating_div(2),
                        ),
                        theme.status_marker(delta_state_tone(panel.delta_state)),
                    ),
                ])],
                theme,
                Alignment::Center,
            );
        } else {
            let (primary, primary_compact, secondary, secondary_compact, tone) =
                if matches!(panel.tile_state, DashboardTileState::Stale) {
                    (
                        panel.primary_label.as_str(),
                        panel.primary_label.as_str(),
                        panel.note.as_str(),
                        "Cached value",
                        Tone::Muted,
                    )
                } else {
                    (
                        panel.fallback.primary.as_str(),
                        panel.fallback.primary_compact.as_str(),
                        panel.fallback.secondary.as_str(),
                        panel.fallback.secondary_compact.as_str(),
                        dashboard_tile_tone(panel.tile_state),
                    )
                };
            render_explicit_tile_state(
                frame,
                shell.content_area,
                ExplicitTileText {
                    primary,
                    primary_compact,
                    secondary,
                    secondary_compact,
                    primary_tone: tone,
                },
                theme,
                Alignment::Center,
            );
        }
        return;
    }

    if !matches!(panel.tile_state, DashboardTileState::Fresh) {
        let (primary, primary_compact, secondary, secondary_compact, tone) =
            if matches!(panel.tile_state, DashboardTileState::Stale) {
                (
                    panel.primary_label.as_str(),
                    panel.primary_label.as_str(),
                    panel.note.as_str(),
                    "Cached value",
                    Tone::Muted,
                )
            } else {
                (
                    panel.fallback.primary.as_str(),
                    panel.fallback.primary_compact.as_str(),
                    panel.fallback.secondary.as_str(),
                    panel.fallback.secondary_compact.as_str(),
                    dashboard_tile_tone(panel.tile_state),
                )
            };
        render_explicit_tile_state(
            frame,
            shell.content_area,
            ExplicitTileText {
                primary,
                primary_compact,
                secondary,
                secondary_compact,
                primary_tone: tone,
            },
            theme,
            Alignment::Center,
        );
        return;
    }

    let content = panel_content_metrics(
        shell.content_area,
        measured_panel_support_lane(shell.content_area, state.chart_metrics),
    );
    let width = usize::from(content.chart.area.width);
    let capacity = usize::from(content.chart.area.height).max(2);
    let instrument_width = clamped_instrument_width(width, state.metrics, 10, width.max(10));
    let primary_line = if panel.judged_state.is_some() {
        Line::from(vec![
            Span::styled(
                concise_text(&panel.primary_label, width.saturating_sub(10)),
                theme.dominant_metric(judged_primary_tone(panel.judged_state)),
            ),
            Span::raw(" "),
            Span::styled(
                judged_state_label(panel.judged_state).to_owned(),
                theme.badge(judged_badge_tone(panel.judged_state)),
            ),
        ])
    } else {
        Line::from(Span::styled(
            concise_text(&panel.primary_label, width),
            theme.dominant_metric(judged_primary_tone(panel.judged_state)),
        ))
    };
    let compare_line = histogram_compare_line(title, panel, width);
    let mut lines = vec![primary_line];
    if capacity > 2 && title == "Resp Rate" {
        lines.push(Line::from(Span::styled(
            micro_histogram(&panel.bars, instrument_width.max(6)),
            theme.chart_ramp(3, 4),
        )));
        if capacity >= 4 {
            lines.push(Line::from(Span::styled(
                concise_text(&panel.range_label, width),
                theme.annotation(),
            )));
        }
    } else if capacity > 2 {
        lines.push(Line::from(Span::styled(
            concise_text(&panel.range_label, width),
            theme.annotation(),
        )));
        if capacity >= 4 {
            lines.push(Line::from(Span::styled(
                meter_bar(fill_percent(&panel.bars), instrument_width),
                theme.status_marker(delta_state_tone(panel.delta_state)),
            )));
        }
        if capacity >= 5 {
            lines.push(Line::from(Span::styled(
                micro_histogram(&panel.bars, instrument_width.max(6)),
                theme.chart_ramp(3, 4),
            )));
        }
    }
    lines.push(Line::from(Span::styled(
        compare_line,
        theme.status_marker(delta_state_tone(panel.delta_state)),
    )));
    render_panel_lines(frame, content.chart.area, lines, theme, Alignment::Left);
    render_support_lane(
        frame,
        content.support.area,
        &panel.note,
        theme,
        Alignment::Left,
    );
}

fn render_breakdown_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    panel: &DashboardBreakdownPanel,
    theme: &Theme,
    state: PanelRenderState,
) {
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        state.metrics,
        PanelShellSpec {
            title: "Readiness Breakdown",
            status: panel.availability.label(),
            status_tone: panel.availability.tone(),
            focused: state.focused,
            expanded: state.expanded,
            kind: PanelKind::Section,
        },
    );
    if shell.content_area.width == 0 || shell.content_area.height == 0 {
        return;
    }

    if area.height <= 3 {
        if let Some(rail) = panel.rails.iter().find(|rail| rail.selected) {
            render_panel_lines(
                frame,
                shell.content_area,
                vec![breakdown_line(
                    rail,
                    usize::from(shell.content_area.width),
                    theme,
                    8,
                    6,
                )],
                theme,
                Alignment::Left,
            );
        } else {
            render_panel_text(
                frame,
                shell.content_area,
                concise_detail(&panel.note, usize::from(shell.content_area.width)),
                theme,
                Alignment::Left,
            );
        }
        return;
    }

    if !metric_panel_has_reading(panel.availability) {
        render_explicit_tile_state(
            frame,
            shell.content_area,
            ExplicitTileText {
                primary: "No readiness breakdown",
                primary_compact: "No breakdown",
                secondary: &panel.note,
                secondary_compact: "No breakdown today",
                primary_tone: Tone::Unavailable,
            },
            theme,
            Alignment::Left,
        );
        return;
    }

    let preferred_label_width = panel
        .rails
        .iter()
        .map(|rail| measure_one_line(&rail.label).width.saturating_add(2))
        .max()
        .unwrap_or(14);
    let preferred_delta_width = panel
        .rails
        .iter()
        .map(|rail| {
            measure_one_line(&rail.delta_label)
                .width
                .max(rail.availability.label().len())
        })
        .max()
        .unwrap_or(10);
    let layout = BreakdownLayout::for_panel(
        shell.content_area,
        state.chart_metrics,
        panel.rails.len(),
        u16::try_from(preferred_label_width).unwrap_or(u16::MAX),
        u16::try_from(preferred_delta_width).unwrap_or(u16::MAX),
    );
    let focus_note = panel
        .rails
        .iter()
        .find(|rail| rail.selected)
        .map_or_else(|| panel.note.clone(), |rail| rail.note.clone());

    render_line_in_area(
        frame,
        layout.label_cell(layout.header_area),
        Line::from(Span::styled(
            format!(
                "{:<width$}",
                "driver",
                width = usize::from(layout.label_column_width)
            ),
            theme.section_title(Tone::Muted),
        )),
        theme,
        Alignment::Left,
    );
    render_line_in_area(
        frame,
        layout.signal_cell(layout.header_area),
        Line::from(Span::styled("rail", theme.annotation())),
        theme,
        Alignment::Left,
    );
    render_line_in_area(
        frame,
        layout.delta_cell(layout.header_area),
        Line::from(Span::styled("Δ", theme.section_title(Tone::Muted))),
        theme,
        Alignment::Right,
    );

    for (index, rail) in panel.rails.iter().enumerate() {
        let row = layout.row_line_area(index);
        let (marker_label, cue_text, cue_tone) = breakdown_rail_cue(rail);
        let label_width = usize::from(layout.label_column_width);
        let rail_label = fit_breakdown_label(&rail.label, label_width.saturating_sub(2));
        let prefix = if rail.selected { ">" } else { " " };
        render_line_in_area(
            frame,
            layout.label_cell(row),
            Line::from(Span::styled(
                format!(
                    "{prefix} {rail_label:<width$}",
                    width = label_width.saturating_sub(2)
                ),
                if rail.selected {
                    theme.section_title(Tone::Focus)
                } else {
                    theme.body()
                },
            )),
            theme,
            Alignment::Left,
        );
        render_line_in_area(
            frame,
            layout.signal_badge_cell(row),
            Line::from(Span::styled(
                format!(
                    "[{marker_label:<width$}]",
                    width = usize::from(layout.signal_badge_width).saturating_sub(2),
                    marker_label = fit_badge_label(
                        &marker_label,
                        usize::from(layout.signal_badge_width).saturating_sub(2),
                    )
                ),
                theme.badge(cue_tone),
            )),
            theme,
            Alignment::Left,
        );
        render_line_in_area(
            frame,
            layout.signal_track_cell(row),
            Line::from(Span::styled(
                segmented_bar(rail.fill_percent, usize::from(layout.bar_viewport_width)),
                theme.annotation(),
            )),
            theme,
            Alignment::Left,
        );
        render_line_in_area(
            frame,
            layout.delta_cell(row),
            Line::from(Span::styled(
                fit_breakdown_delta(&cue_text, usize::from(layout.delta_column_width)),
                theme.status_marker(cue_tone),
            )),
            theme,
            Alignment::Right,
        );
    }

    if layout.support_lane.height > 0 {
        render_line_in_area(
            frame,
            layout.support_lane,
            Line::from(Span::styled(
                support_lane_text_with(
                    &focus_note,
                    usize::from(layout.support_lane.width),
                    &["Use footer for exact driver evidence"],
                ),
                theme.annotation(),
            )),
            theme,
            Alignment::Left,
        );
    }
}

fn render_heatmap_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    panel: &DashboardWeeklyHeatmap,
    theme: &Theme,
    viewport: ViewportClass,
    state: PanelRenderState,
) {
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        state.metrics,
        PanelShellSpec {
            title: "Weekly Trends",
            status: panel.availability.label(),
            status_tone: panel.availability.tone(),
            focused: state.focused,
            expanded: state.expanded,
            kind: PanelKind::Section,
        },
    );
    if shell.content_area.width == 0 || shell.content_area.height == 0 {
        return;
    }

    if area.height <= 3 {
        let row = if metric_panel_has_reading(panel.availability) {
            panel.recent.rows.first().map_or_else(
                || concise_detail(&panel.note, usize::from(shell.content_area.width)),
                |values| compact_heatmap_row(values, panel.recent.selected_cell),
            )
        } else {
            fit_single_line_with(
                &format!("{} | {}", panel.window_label, panel.note),
                usize::from(shell.content_area.width),
                &[&panel.window_label, "No weekly data"],
            )
            .text
        };
        render_panel_text(frame, shell.content_area, row, theme, Alignment::Left);
        return;
    }

    if !metric_panel_has_reading(panel.availability) {
        render_explicit_tile_state(
            frame,
            shell.content_area,
            ExplicitTileText {
                primary: "No weekly trend data",
                primary_compact: "No weekly data",
                secondary: &panel.note,
                secondary_compact: "No weekly data",
                primary_tone: Tone::Unavailable,
            },
            theme,
            Alignment::Left,
        );
        return;
    }

    let grid = panel.grid_for_viewport(viewport);
    let mode = WeeklyHeatmapMode::Standard;
    let layout = WeeklyTrendsLayout::for_panel(
        shell.content_area,
        state.chart_metrics,
        mode,
        grid.day_labels.len(),
        panel.row_labels.len(),
    );
    if layout.grid_viewport.height == 0 || layout.grid_viewport.width == 0 {
        render_panel_text(
            frame,
            shell.content_area,
            concise_detail(&panel.note, usize::from(shell.content_area.width)),
            theme,
            Alignment::Left,
        );
        return;
    }

    let last_group_index = panel.row_labels.len().saturating_sub(1);
    let last_subrow = layout.subrow_area(last_group_index, 1);
    let content_bottom = shell
        .content_area
        .y
        .saturating_add(shell.content_area.height);
    if last_subrow.y.saturating_add(last_subrow.height) > content_bottom {
        render_compact_grouped_heatmap_panel(
            frame,
            shell.content_area,
            panel,
            grid,
            theme,
            mode,
            layout,
        );
        return;
    }

    render_line_in_area(
        frame,
        layout.header_grid_area,
        heatmap_header_line(theme, grid, mode, layout),
        theme,
        Alignment::Left,
    );

    let selected_column = grid.selected_cell.map(|(_, column_index)| column_index);
    for (group_index, label) in panel.row_labels.iter().enumerate() {
        let label_line = Line::from(Span::styled(
            format!(
                "{:<width$}",
                fit_weekly_group_label(label, usize::from(layout.label_column_width)),
                width = usize::from(layout.label_column_width)
            ),
            theme.section_title(Tone::Muted),
        ));
        render_line_in_area(
            frame,
            layout.group_label_line_area(group_index),
            label_line,
            theme,
            Alignment::Left,
        );
        let values = grid.rows.get(group_index);
        let top_line = values.map_or_else(
            || heatmap_subrow_line(theme, &[], selected_column, layout, 0),
            |row| heatmap_subrow_line(theme, row, selected_column, layout, 0),
        );
        render_line_in_area(
            frame,
            layout.subrow_area(group_index, 0),
            top_line,
            theme,
            Alignment::Left,
        );
        let bottom_line = values.map_or_else(
            || heatmap_subrow_line(theme, &[], selected_column, layout, 1),
            |row| heatmap_subrow_line(theme, row, selected_column, layout, 1),
        );
        render_line_in_area(
            frame,
            layout.subrow_area(group_index, 1),
            bottom_line,
            theme,
            Alignment::Left,
        );
    }

    if layout.legend_area.height > 0 && layout.legend_area.width > 0 {
        render_line_in_area(
            frame,
            layout.legend_area,
            heatmap_legend_line(theme, layout),
            theme,
            Alignment::Left,
        );
    }
    if layout.summary_area.height > 0 && layout.summary_area.width > 0 {
        render_line_in_area(
            frame,
            layout.summary_area,
            heatmap_summary_line(
                theme,
                grid,
                &panel.row_labels,
                &panel.note,
                &panel.window_label,
                &panel.window_page_label,
                layout,
            ),
            theme,
            Alignment::Left,
        );
    }
}

fn render_compact_grouped_heatmap_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    panel: &DashboardWeeklyHeatmap,
    grid: &crate::app::DashboardHeatmapGrid,
    theme: &Theme,
    mode: WeeklyHeatmapMode,
    layout: WeeklyTrendsLayout,
) {
    let selected_column = grid.selected_cell.map(|(_, column_index)| column_index);
    let mut lines = Vec::new();
    lines.push(heatmap_compact_header_line(theme, grid, mode, layout));
    for (group_index, label) in panel.row_labels.iter().enumerate() {
        let values = grid.rows.get(group_index).map_or(&[][..], Vec::as_slice);
        lines.push(heatmap_compact_group_line(
            theme,
            label,
            values,
            selected_column,
            layout,
        ));
    }
    if usize::from(area.height) > lines.len() {
        lines.push(heatmap_compact_legend_line(theme, layout));
    }
    if usize::from(area.height) > lines.len() {
        lines.push(heatmap_compact_summary_line(
            theme,
            grid,
            &panel.row_labels,
            &panel.note,
            heatmap_window_summary(&panel.window_label, &panel.window_page_label).as_deref(),
            layout,
            usize::from(area.width),
        ));
    }
    render_panel_lines(frame, area, lines, theme, Alignment::Left);
}

const fn metric_panel_has_reading(state: MetricPanelState) -> bool {
    state.has_current_sample()
}

const fn availability_has_reading(availability: TelemetryAvailability) -> bool {
    matches!(
        availability,
        TelemetryAvailability::Fresh | TelemetryAvailability::Stale
    )
}

fn selected_heatmap_panel_summary(
    grid: &crate::app::DashboardHeatmapGrid,
    note: &str,
) -> Option<(String, Option<DashboardScoreBand>)> {
    grid.selected_cell
        .and_then(|(row_index, column_index)| {
            let row = grid.rows.get(row_index)?;
            let value = row.get(column_index).copied().flatten()?;
            let day_label = grid.day_labels.get(column_index)?;
            Some((
                format!("Selected {day_label}"),
                Some(score_band_from_value(value)),
            ))
        })
        .or_else(|| {
            if note.is_empty() {
                None
            } else {
                Some((note.to_owned(), None))
            }
        })
}

fn render_panel_text(
    frame: &mut Frame<'_>,
    area: Rect,
    text: impl AsRef<str>,
    theme: &Theme,
    alignment: Alignment,
) {
    let text = text.as_ref();
    render_panel_lines(
        frame,
        area,
        text.lines()
            .map(|line| Line::from(line.to_owned()))
            .collect::<Vec<_>>(),
        theme,
        alignment,
    );
}

fn render_panel_lines(
    frame: &mut Frame<'_>,
    area: Rect,
    lines: Vec<Line<'static>>,
    theme: &Theme,
    alignment: Alignment,
) {
    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.body())
            .alignment(alignment),
        area,
    );
}

const fn measured_panel_support_lane(area: Rect, chart_metrics: DashboardChartMetrics) -> u16 {
    if area.height >= 5 {
        chart_metrics.support_lane_height
    } else {
        0
    }
}

fn render_support_lane(
    frame: &mut Frame<'_>,
    area: Rect,
    note: &str,
    theme: &Theme,
    alignment: Alignment,
) {
    if area.width == 0 || area.height == 0 || note.is_empty() {
        return;
    }

    render_line_in_area(
        frame,
        area,
        Line::from(Span::styled(
            support_lane_text(note, usize::from(area.width)),
            theme.annotation(),
        )),
        theme,
        alignment,
    );
}

fn centered_line(width: usize, text: impl AsRef<str>) -> String {
    let text = text.as_ref();
    format!("{text:^width$}")
}

fn centered_body_area(area: Rect, line_count: usize) -> Rect {
    let height = u16::try_from(line_count)
        .unwrap_or(u16::MAX)
        .min(area.height.max(1));
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);
    Rect::new(area.x, y, area.width, height)
}

fn compact_heatmap_row(values: &[Option<u8>], selected_cell: Option<(usize, usize)>) -> String {
    let selected_column = selected_cell.map(|(_, column_index)| column_index);
    let cells = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let glyph = match value {
                Some(0) | None => '·',
                Some(1) => '░',
                Some(2) => '▒',
                Some(_) => '▓',
            };
            if selected_column == Some(index) {
                format!("[{glyph}]")
            } else {
                glyph.to_string()
            }
        })
        .collect::<String>();
    format!("S {cells}")
}

fn heatmap_compact_header_line(
    theme: &Theme,
    grid: &crate::app::DashboardHeatmapGrid,
    mode: WeeklyHeatmapMode,
    layout: WeeklyTrendsLayout,
) -> Line<'static> {
    let mut spans = vec![Span::raw(
        " ".repeat(usize::from(layout.label_column_width)),
    )];
    for day in &grid.day_labels {
        let label = fit_day_header(
            &heatmap_day_label(mode, day),
            usize::from(layout.slot_width),
        );
        spans.push(Span::styled(
            format!("{label:^width$}", width = usize::from(layout.slot_width)),
            theme.section_title(Tone::Muted),
        ));
    }
    Line::from(spans)
}

fn heatmap_compact_group_line(
    theme: &Theme,
    label: &str,
    values: &[Option<u8>],
    selected_column: Option<usize>,
    layout: WeeklyTrendsLayout,
) -> Line<'static> {
    let mut spans = vec![Span::styled(
        format!(
            "{:<width$}",
            fit_weekly_group_label(label, usize::from(layout.label_column_width)),
            width = usize::from(layout.label_column_width)
        ),
        theme.section_title(Tone::Muted),
    )];
    for column_index in 0..usize::from(layout.grid_viewport.width / layout.slot_width.max(1)) {
        let value = values.get(column_index).copied().flatten();
        let band = value.map(score_band_from_value);
        let level = value.map_or(0, |score| usize::from(score.min(100)) * 4 / 100 + 1);
        let fill = heatmap_paired_cell_fill(value, usize::from(layout.cell_width));
        if selected_column == Some(column_index) {
            spans.push(Span::styled("[", theme.badge(Tone::Focus)));
            spans.push(Span::styled(
                fill,
                theme.status_marker(score_band_cue_tone(band)),
            ));
            spans.push(Span::styled("]", theme.badge(Tone::Focus)));
        } else {
            spans.push(Span::raw(" ".repeat(usize::from(layout.cell_gap))));
            spans.push(Span::styled(fill, theme.chart_ramp(level, 5)));
            spans.push(Span::raw(" ".repeat(usize::from(layout.cell_gap))));
        }
    }
    Line::from(spans)
}

fn heatmap_compact_legend_line(theme: &Theme, layout: WeeklyTrendsLayout) -> Line<'static> {
    let mut spans = vec![Span::raw(
        " ".repeat(usize::from(layout.label_column_width)),
    )];
    spans.extend(heatmap_legend_line(theme, layout).spans);
    Line::from(spans)
}

fn heatmap_compact_summary_line(
    theme: &Theme,
    grid: &crate::app::DashboardHeatmapGrid,
    _row_labels: &[String],
    note: &str,
    window_summary: Option<&str>,
    layout: WeeklyTrendsLayout,
    line_width: usize,
) -> Line<'static> {
    let (summary, band) =
        selected_heatmap_panel_summary(grid, note).unwrap_or_else(|| (note.to_owned(), None));
    let label_width = usize::from(layout.label_column_width);
    let summary_width = line_width.saturating_sub(label_width);
    let text_budget = summary_width.saturating_sub(if band.is_some() { 10 } else { 0 });
    let mut spans = vec![Span::raw(" ".repeat(label_width))];
    let body =
        window_summary.map_or_else(|| summary.clone(), |window| format!("{window} | {summary}"));
    spans.push(Span::styled(
        support_lane_text(&body, text_budget),
        theme.body(),
    ));
    if band.is_some() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("[{}]", score_band_label(band)),
            theme.badge(score_band_cue_tone(band)),
        ));
    }
    Line::from(spans)
}

fn render_line_in_area(
    frame: &mut Frame<'_>,
    area: Rect,
    line: Line<'static>,
    theme: &Theme,
    alignment: Alignment,
) {
    render_panel_lines(frame, area, vec![line], theme, alignment);
}

fn clamped_instrument_width(
    width: usize,
    metrics: DashboardMetrics,
    minimum: usize,
    maximum: usize,
) -> usize {
    width
        .saturating_sub(usize::from(metrics.major_inset_x))
        .clamp(minimum, maximum)
}

fn fill_percent(values: &[u64]) -> u16 {
    let max = values.iter().copied().max().unwrap_or(0);
    let current = values.last().copied().unwrap_or(0);
    if max == 0 {
        0
    } else {
        u16::try_from((current.saturating_mul(100)) / max).unwrap_or(100)
    }
}

fn histogram_compare_line(title: &str, panel: &DashboardHistogramPanel, width: usize) -> String {
    if title == "Resp Rate" {
        let strip = micro_histogram(&panel.bars, width.clamp(6, 12));
        return fit_single_line_with(
            &format!("{strip} {} | {}", panel.delta_label, panel.range_label),
            width,
            &[
                &format!("{strip} {}", panel.delta_label),
                &panel.delta_label,
                &panel.range_label,
                "range --",
            ],
        )
        .text;
    }

    fit_single_line_with(
        &format!("{} | {}", panel.delta_label, panel.range_label),
        width,
        &[&panel.delta_label, &panel.range_label, "range --"],
    )
    .text
}

fn trend_instrument_lines(title: &str, panel: &DashboardTrendPanel, width: usize) -> Vec<String> {
    match title {
        "HRV Trend" => vec![
            spark_strip(&panel.values, width.max(8)),
            micro_histogram(&panel.values, width.max(8)),
        ],
        "SpO2" => vec![
            centered_line(
                width,
                meter_bar(
                    fill_percent(&panel.values),
                    if width == 0 { 1 } else { width.min(18) },
                ),
            ),
            spark_strip(&panel.values, width.max(8)),
        ],
        _ => vec![spark_strip(&panel.values, width.max(8))],
    }
}

fn compact_temperature_lines(deviation_tenths: Option<i16>, width: usize) -> Vec<String> {
    let marker_index = deviation_tenths.map_or(1, |value| match value {
        value if value > 2 => 0,
        value if value < -2 => 2,
        _ => 1,
    });
    ["│", "┼", "│"]
        .into_iter()
        .enumerate()
        .map(|(index, glyph)| {
            centered_line(
                width,
                if index == marker_index {
                    "█".to_owned()
                } else {
                    glyph.to_owned()
                },
            )
        })
        .collect()
}

const fn score_band_label(score_band: Option<DashboardScoreBand>) -> &'static str {
    match score_band {
        Some(DashboardScoreBand::Optimal) => "optimal",
        Some(DashboardScoreBand::Good) => "good",
        Some(DashboardScoreBand::Fair) => "fair",
        Some(DashboardScoreBand::PayAttention) => "watch",
        None => "score",
    }
}

const fn score_band_from_value(value: u8) -> DashboardScoreBand {
    match value {
        85..=100 => DashboardScoreBand::Optimal,
        70..=84 => DashboardScoreBand::Good,
        60..=69 => DashboardScoreBand::Fair,
        _ => DashboardScoreBand::PayAttention,
    }
}

const fn score_band_cue_tone(score_band: Option<DashboardScoreBand>) -> Tone {
    match score_band {
        Some(DashboardScoreBand::Optimal | DashboardScoreBand::Good) => Tone::JudgedOk,
        Some(DashboardScoreBand::Fair) => Tone::JudgedWarn,
        Some(DashboardScoreBand::PayAttention) => Tone::JudgedAlert,
        None => Tone::Muted,
    }
}

const fn score_band_primary_tone(score_band: Option<DashboardScoreBand>) -> Tone {
    match score_band {
        Some(DashboardScoreBand::Optimal) => Tone::JudgedOk,
        Some(DashboardScoreBand::Fair) => Tone::JudgedWarn,
        Some(DashboardScoreBand::PayAttention) => Tone::JudgedAlert,
        Some(DashboardScoreBand::Good) | None => Tone::Default,
    }
}

const fn delta_state_tone(delta_state: DashboardDeltaState) -> Tone {
    match delta_state {
        DashboardDeltaState::Cool => Tone::DeltaCool,
        DashboardDeltaState::Neutral => Tone::Muted,
        DashboardDeltaState::Warm => Tone::DeltaWarm,
    }
}

const fn judged_badge_tone(judged_state: Option<DashboardJudgedState>) -> Tone {
    match judged_state {
        Some(DashboardJudgedState::Ok) => Tone::JudgedOk,
        Some(DashboardJudgedState::Warn) => Tone::JudgedWarn,
        Some(DashboardJudgedState::Alert) => Tone::JudgedAlert,
        None => Tone::Muted,
    }
}

const fn judged_primary_tone(judged_state: Option<DashboardJudgedState>) -> Tone {
    match judged_state {
        Some(DashboardJudgedState::Warn) => Tone::JudgedWarn,
        Some(DashboardJudgedState::Alert) => Tone::JudgedAlert,
        Some(DashboardJudgedState::Ok) | None => Tone::Default,
    }
}

const fn judged_state_label(judged_state: Option<DashboardJudgedState>) -> &'static str {
    match judged_state {
        Some(DashboardJudgedState::Ok) => "ok",
        Some(DashboardJudgedState::Warn) => "watch",
        Some(DashboardJudgedState::Alert) => "alert",
        None => "",
    }
}

fn breakdown_line(
    rail: &DashboardBreakdownRail,
    label_width: usize,
    theme: &Theme,
    bar_segments: usize,
    delta_width: usize,
) -> Line<'static> {
    let rail_label = fit_breakdown_label(&rail.label, label_width.saturating_sub(2));
    let label = format!("{} {rail_label}", if rail.selected { ">" } else { " " });
    let (marker_label, cue_text, cue_tone) = breakdown_rail_cue(rail);
    let label_style = if rail.selected {
        theme.section_title(Tone::Focus)
    } else {
        theme.body()
    };

    Line::from(vec![
        Span::styled(format!("{label:<label_width$}"), label_style),
        Span::styled(
            format!(
                "[{marker_label:<5}]",
                marker_label = fit_badge_label(&marker_label, 5)
            ),
            theme.badge(cue_tone),
        ),
        Span::raw(" "),
        Span::styled(
            segmented_bar(rail.fill_percent, bar_segments),
            theme.annotation(),
        ),
        Span::raw(" "),
        Span::styled(
            format!(
                "{:>delta_width$}",
                fit_breakdown_delta(&cue_text, delta_width)
            ),
            theme.status_marker(cue_tone),
        ),
    ])
}

fn heatmap_summary_line(
    theme: &Theme,
    grid: &crate::app::DashboardHeatmapGrid,
    row_labels: &[String],
    note: &str,
    window_label: &str,
    window_page_label: &str,
    layout: WeeklyTrendsLayout,
) -> Line<'static> {
    let (summary, band) =
        selected_heatmap_panel_summary(grid, note).unwrap_or_else(|| (note.to_owned(), None));
    let body = heatmap_window_summary(window_label, window_page_label)
        .map_or_else(|| summary.clone(), |window| format!("{window} | {summary}"));
    let compact_summary = compact_heatmap_summary(grid, row_labels, note);
    let fallback_strings = if body == summary {
        compact_summary.into_iter().collect::<Vec<_>>()
    } else {
        compact_summary
            .into_iter()
            .chain(std::iter::once(summary))
            .collect::<Vec<_>>()
    };
    let fallback_refs = fallback_strings
        .iter()
        .map(std::string::String::as_str)
        .collect::<Vec<_>>();
    let mut spans = Vec::new();
    spans.push(Span::styled(
        support_lane_text_with(
            &body,
            usize::from(layout.summary_area.width).saturating_sub(if band.is_some() {
                10
            } else {
                0
            }),
            &fallback_refs,
        ),
        theme.body(),
    ));
    if band.is_some() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("[{}]", score_band_label(band)),
            theme.badge(score_band_cue_tone(band)),
        ));
    }
    Line::from(spans)
}

fn heatmap_window_summary(window_label: &str, window_page_label: &str) -> Option<String> {
    if window_page_label.is_empty() || window_page_label.starts_with("latest") {
        None
    } else if window_label.is_empty() {
        Some(window_page_label.to_owned())
    } else {
        Some(format!("{window_label} | {window_page_label}"))
    }
}

fn compact_heatmap_summary(
    grid: &crate::app::DashboardHeatmapGrid,
    row_labels: &[String],
    note: &str,
) -> Option<String> {
    grid.selected_cell
        .and_then(|(row_index, column_index)| {
            let _ = row_labels.get(row_index)?;
            let row = grid.rows.get(row_index)?;
            row.get(column_index).copied().flatten()?;
            let day_label = grid.day_labels.get(column_index)?;
            Some(format!("Selected {day_label}"))
        })
        .or_else(|| {
            if note.is_empty() {
                None
            } else {
                Some(note.to_owned())
            }
        })
}

fn heatmap_legend_line(theme: &Theme, _layout: WeeklyTrendsLayout) -> Line<'static> {
    Line::from(vec![
        Span::styled("ramp ", theme.section_title(Tone::Muted)),
        Span::styled("░", theme.chart_ramp(1, 5)),
        Span::styled("▒", theme.chart_ramp(2, 5)),
        Span::styled("▓", theme.chart_ramp(3, 5)),
        Span::styled("█", theme.chart_ramp(4, 5)),
        Span::styled(" higher", theme.annotation()),
    ])
}

fn heatmap_cell_fill(value: Option<u8>, cell_width: usize, subrow_index: usize) -> String {
    let level = value.map_or(0, |score| usize::from(score.min(100)) * 4 / 100 + 1);
    let glyph = match (subrow_index, level) {
        (_, 0) => '·',
        (0, 1) => '░',
        (0, 2) => '▒',
        (0, 3) => '▓',
        (_, _) if subrow_index == 0 => '█',
        (1, 1) => '╶',
        (1, 2) => '─',
        (1, 3) => '━',
        (_, _) if subrow_index == 1 => '█',
        _ => '·',
    };
    glyph.to_string().repeat(cell_width)
}

fn heatmap_paired_cell_fill(value: Option<u8>, cell_width: usize) -> String {
    if cell_width <= 1 {
        return heatmap_cell_fill(value, cell_width.max(1), 0);
    }

    let top_width = cell_width.div_ceil(2);
    let bottom_width = cell_width.saturating_sub(top_width);
    let mut fill = heatmap_cell_fill(value, top_width, 0);
    if bottom_width > 0 {
        fill.push_str(&heatmap_cell_fill(value, bottom_width, 1));
    }
    fill
}

fn breakdown_rail_cue(rail: &DashboardBreakdownRail) -> (String, String, Tone) {
    let cue_text = if availability_has_reading(rail.availability) {
        rail.delta_label.clone()
    } else {
        rail.availability.label().to_owned()
    };
    let marker_label = if !availability_has_reading(rail.availability) {
        rail.availability.label().to_ascii_lowercase()
    } else if rail.judged_state.is_some() {
        judged_state_label(rail.judged_state).to_owned()
    } else {
        match rail.delta_state {
            DashboardDeltaState::Cool => "cool".to_owned(),
            DashboardDeltaState::Warm => "warm".to_owned(),
            DashboardDeltaState::Neutral => "steady".to_owned(),
        }
    };
    let cue_tone = if !availability_has_reading(rail.availability) {
        rail.availability.tone()
    } else if rail.judged_state.is_some() {
        judged_badge_tone(rail.judged_state)
    } else {
        delta_state_tone(rail.delta_state)
    };
    (marker_label, cue_text, cue_tone)
}

fn heatmap_header_line(
    theme: &Theme,
    grid: &crate::app::DashboardHeatmapGrid,
    mode: WeeklyHeatmapMode,
    layout: WeeklyTrendsLayout,
) -> Line<'static> {
    let mut spans = Vec::new();
    for day in &grid.day_labels {
        let label = fit_day_header(
            &heatmap_day_label(mode, day),
            usize::from(layout.slot_width),
        );
        spans.push(Span::styled(
            format!("{label:^width$}", width = usize::from(layout.slot_width)),
            theme.section_title(Tone::Muted),
        ));
    }
    Line::from(spans)
}

fn heatmap_subrow_line(
    theme: &Theme,
    values: &[Option<u8>],
    selected_column: Option<usize>,
    layout: WeeklyTrendsLayout,
    subrow_index: usize,
) -> Line<'static> {
    let mut spans = Vec::new();
    for column_index in 0..usize::from(layout.grid_viewport.width / layout.slot_width.max(1)) {
        let value = values.get(column_index).copied().flatten();
        let band = value.map(score_band_from_value);
        let level = value.map_or(0, |score| usize::from(score.min(100)) * 4 / 100 + 1);
        let fill = heatmap_cell_fill(value, usize::from(layout.cell_width), subrow_index);
        if selected_column == Some(column_index) {
            spans.push(Span::styled("[", theme.badge(Tone::Focus)));
            spans.push(Span::styled(
                fill,
                theme.status_marker(score_band_cue_tone(band)),
            ));
            spans.push(Span::styled("]", theme.badge(Tone::Focus)));
        } else {
            spans.push(Span::raw(" ".repeat(usize::from(layout.cell_gap))));
            spans.push(Span::styled(fill, theme.chart_ramp(level, 5)));
            spans.push(Span::raw(" ".repeat(usize::from(layout.cell_gap))));
        }
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::{compact_heatmap_summary, trend_instrument_lines};
    use crate::app::{
        DashboardDeltaState, DashboardHeatmapGrid, DashboardTileFallback, DashboardTileState,
        DashboardTrendPanel,
    };
    use crate::ui::telemetry::MetricPanelState;

    #[test]
    fn compact_heatmap_summary_uses_note_when_selected_cell_has_no_value() {
        let labels = vec!["Readiness".to_owned()];
        let missing_value_grid = DashboardHeatmapGrid {
            day_labels: vec!["04-08".to_owned()],
            rows: vec![vec![None]],
            selected_cell: Some((0, 0)),
        };

        assert_eq!(
            compact_heatmap_summary(&missing_value_grid, &labels, "No data yet"),
            Some("No data yet".to_owned())
        );

        let populated_grid = DashboardHeatmapGrid {
            day_labels: vec!["04-08".to_owned()],
            rows: vec![vec![Some(82)]],
            selected_cell: Some((0, 0)),
        };

        assert_eq!(
            compact_heatmap_summary(&populated_grid, &labels, "No data yet"),
            Some("Selected 04-08".to_owned())
        );
    }

    #[test]
    fn spo2_trend_instrument_lines_respect_small_widths() {
        let panel = DashboardTrendPanel {
            availability: MetricPanelState::Fresh,
            tile_state: DashboardTileState::Fresh,
            primary_label: "97%".to_owned(),
            baseline_label: "30d 97.2%".to_owned(),
            range_label: "96-99%".to_owned(),
            delta_state: DashboardDeltaState::Neutral,
            judged_state: None,
            values: vec![96, 97, 97, 98],
            note: "Stable overnight readings.".to_owned(),
            fallback: DashboardTileFallback::default(),
        };

        let lines = trend_instrument_lines("SpO2", &panel, 4);

        assert_eq!(lines.len(), 2);
        assert!(lines[0].chars().count() <= 4);
    }
}
