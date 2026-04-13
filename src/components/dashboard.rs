use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::{Alignment, Line, Rect, Span},
    widgets::Paragraph,
};

use crate::app::{
    DashboardBreakdownPanel, DashboardBreakdownRail, DashboardDeltaState, DashboardHistogramPanel,
    DashboardJudgedState, DashboardModel, DashboardScoreBand, DashboardScoreTile,
    DashboardSleepTile, DashboardThermometerPanel, DashboardTrendPanel, DashboardWeeklyHeatmap,
};
use crate::navigation::FocusRegion;
use crate::ui::{
    chrome::{PanelKind, PanelShellSpec, render_panel_shell},
    layout::{DashboardMetrics, UiContext, ViewportClass},
    telemetry::{
        MetricPanelState, TelemetryAvailability, WeeklyHeatmapLayout, WeeklyHeatmapMode,
        concise_detail, concise_text, fit_heatmap_label, heatmap_day_label, meter_bar,
        metric_panel_scaffold, micro_histogram, segmented_bar, spark_strip, stacked_profile_rows,
        weekly_heatmap_layout,
    },
    theme::{Theme, Tone},
};

#[derive(Debug, Clone, Copy)]
struct PanelRenderState {
    focused: bool,
    expanded: bool,
    metrics: DashboardMetrics,
}

#[derive(Debug, Clone, Copy)]
struct DashboardDrawContext<'a> {
    theme: &'a Theme,
    viewport: ViewportClass,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
    metrics: DashboardMetrics,
}

const fn panel_state(focused: bool, expanded: bool, metrics: DashboardMetrics) -> PanelRenderState {
    PanelRenderState {
        focused,
        expanded,
        metrics,
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
        .constraints([Constraint::Percentage(66), Constraint::Percentage(34)])
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
        ),
    );
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
            Constraint::Length(11),
            Constraint::Length(7),
            Constraint::Min(11),
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
            status: score_tile.availability.label(),
            status_tone: score_tile.availability.tone(),
            focused: state.focused,
            expanded: state.expanded,
            kind: PanelKind::Hero,
        },
    );
    if shell.content_area.width == 0 || shell.content_area.height == 0 {
        return;
    }

    if area.height <= 3 {
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

    if !metric_panel_has_reading(score_tile.availability) {
        let lines = metric_panel_scaffold(
            score_tile.availability,
            &score_tile.note,
            usize::from(shell.content_area.width),
        );
        render_panel_text(
            frame,
            centered_body_area(shell.content_area, lines.len()),
            lines.join("\n"),
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
            status: tile.availability.label(),
            status_tone: tile.availability.tone(),
            focused: state.focused,
            expanded: state.expanded,
            kind: PanelKind::Hero,
        },
    );
    if shell.content_area.width == 0 || shell.content_area.height == 0 {
        return;
    }

    if area.height <= 3 {
        let cue_tone = score_band_cue_tone(tile.score_band);
        render_panel_lines(
            frame,
            shell.content_area,
            vec![Line::from(vec![
                Span::styled(
                    tile.duration_label.clone(),
                    theme.dominant_metric(score_band_primary_tone(tile.score_band)),
                ),
                Span::raw(" "),
                Span::styled(
                    score_band_label(tile.score_band).to_owned(),
                    theme.badge(cue_tone),
                ),
            ])],
            theme,
            Alignment::Center,
        );
        return;
    }

    if !metric_panel_has_reading(tile.availability) {
        let lines = metric_panel_scaffold(
            tile.availability,
            &tile.strip_note,
            usize::from(shell.content_area.width),
        );
        render_panel_text(
            frame,
            centered_body_area(shell.content_area, lines.len()),
            lines.join("\n"),
            theme,
            Alignment::Center,
        );
        return;
    }

    let width = usize::from(shell.content_area.width);
    let cue_tone = score_band_cue_tone(tile.score_band);
    let band_width = clamped_instrument_width(width, state.metrics, 12, width.max(12));
    let capacity = usize::from(shell.content_area.height).max(4);
    let band_height = capacity
        .saturating_sub(if state.focused || state.expanded {
            3
        } else {
            2
        })
        .clamp(4, 7);
    let mut lines = vec![Line::from(vec![
        Span::styled(
            centered_line(width / 2, format!("duration {}", tile.duration_label)),
            theme.dominant_metric(score_band_primary_tone(tile.score_band)),
        ),
        Span::styled(
            centered_line(
                width.saturating_sub(width / 2),
                format!("{} {}", tile.score_label, score_band_label(tile.score_band)),
            ),
            theme.badge(cue_tone),
        ),
    ])];
    lines.extend(
        stacked_profile_rows(&tile.trend, band_width, band_height)
            .into_iter()
            .map(|row| {
                Line::from(Span::styled(
                    centered_line(width, row),
                    theme.chart_ramp(3, 4),
                ))
            }),
    );
    lines.push(Line::from(Span::styled(
        centered_line(width, spark_strip(&tile.trend, band_width)),
        theme.status_marker(cue_tone),
    )));
    if state.focused || state.expanded {
        lines.push(Line::from(Span::styled(
            concise_detail(&tile.strip_note, width),
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
        if metric_panel_has_reading(panel.availability) {
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
        render_panel_text(
            frame,
            shell.content_area,
            metric_panel_scaffold(
                panel.availability,
                &panel.note,
                usize::from(shell.content_area.width),
            )
            .join("\n"),
            theme,
            Alignment::Left,
        );
        return;
    }

    let width = usize::from(shell.content_area.width);
    let mut lines = vec![Line::from(vec![
        Span::styled(
            concise_text(&panel.primary_label, width.saturating_sub(10)),
            theme.dominant_metric(judged_primary_tone(panel.judged_state)),
        ),
        Span::raw(" "),
        Span::styled(
            judged_state_label(panel.judged_state).to_owned(),
            theme.badge(judged_badge_tone(panel.judged_state)),
        ),
    ])];
    lines.extend(
        trend_instrument_lines(title, panel, width)
            .into_iter()
            .map(|line| {
                Line::from(Span::styled(
                    line,
                    theme.status_marker(delta_state_tone(panel.delta_state)),
                ))
            }),
    );
    lines.push(Line::from(vec![
        Span::styled(
            concise_text(&panel.baseline_label, width / 2),
            theme.status_marker(delta_state_tone(panel.delta_state)),
        ),
        Span::raw(" "),
        Span::styled(
            concise_text(&panel.range_label, width.saturating_sub(width / 2 + 1)),
            theme.annotation(),
        ),
    ]));
    if state.focused || state.expanded {
        lines.push(Line::from(Span::styled(
            concise_detail(&panel.note, width),
            theme.annotation(),
        )));
    }
    render_panel_lines(frame, shell.content_area, lines, theme, Alignment::Left);
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

    if !metric_panel_has_reading(panel.availability) {
        let lines = metric_panel_scaffold(
            panel.availability,
            &panel.note,
            usize::from(shell.content_area.width),
        );
        render_panel_text(
            frame,
            centered_body_area(shell.content_area, lines.len()),
            lines.join("\n"),
            theme,
            Alignment::Center,
        );
        return;
    }

    let width = usize::from(shell.content_area.width);
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
    if state.focused || state.expanded {
        lines.push(Line::from(Span::styled(
            centered_line(width, concise_detail(&panel.note, width)),
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
        if metric_panel_has_reading(panel.availability) {
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
            render_panel_text(
                frame,
                shell.content_area,
                concise_detail(&panel.note, usize::from(shell.content_area.width)),
                theme,
                Alignment::Center,
            );
        }
        return;
    }

    if !metric_panel_has_reading(panel.availability) {
        render_panel_text(
            frame,
            shell.content_area,
            metric_panel_scaffold(
                panel.availability,
                &panel.note,
                usize::from(shell.content_area.width),
            )
            .join("\n"),
            theme,
            Alignment::Center,
        );
        return;
    }

    let width = usize::from(shell.content_area.width);
    let instrument_width = clamped_instrument_width(width, state.metrics, 10, width.max(10));
    let mut lines = vec![Line::from(vec![
        Span::styled(
            concise_text(&panel.primary_label, width.saturating_sub(10)),
            theme.dominant_metric(judged_primary_tone(panel.judged_state)),
        ),
        Span::raw(" "),
        Span::styled(
            judged_state_label(panel.judged_state).to_owned(),
            theme.badge(judged_badge_tone(panel.judged_state)),
        ),
    ])];
    lines.push(Line::from(Span::styled(
        centered_line(
            width,
            meter_bar(fill_percent(&panel.bars), instrument_width),
        ),
        theme.status_marker(delta_state_tone(panel.delta_state)),
    )));
    lines.push(Line::from(Span::styled(
        micro_histogram(&panel.bars, width.max(6)),
        theme.chart_ramp(3, 4),
    )));
    lines.push(Line::from(vec![
        Span::styled(
            concise_text(&panel.delta_label, width / 2),
            theme.status_marker(delta_state_tone(panel.delta_state)),
        ),
        Span::raw(" "),
        Span::styled(
            concise_text(&panel.range_label, width.saturating_sub(width / 2 + 1)),
            theme.annotation(),
        ),
    ]));
    if (state.focused || state.expanded) && shell.content_area.height >= 4 {
        lines.push(Line::from(Span::styled(
            concise_detail(&panel.note, width),
            theme.annotation(),
        )));
    }
    render_panel_lines(frame, shell.content_area, lines, theme, Alignment::Center);
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
        render_panel_text(
            frame,
            shell.content_area,
            metric_panel_scaffold(
                panel.availability,
                &panel.note,
                usize::from(shell.content_area.width),
            )
            .join("\n"),
            theme,
            Alignment::Left,
        );
        return;
    }

    let width = usize::from(shell.content_area.width);
    let label_width = panel
        .rails
        .iter()
        .map(|rail| rail.label.len() + 2)
        .max()
        .unwrap_or(14)
        .min(width.saturating_div(4).clamp(11, 14));
    let delta_width = panel
        .rails
        .iter()
        .map(|rail| rail.delta_label.len())
        .max()
        .unwrap_or(10)
        .clamp(7, 11);
    let bar_segments = width.saturating_sub(label_width + delta_width + 4).max(12);
    let focus_note = panel
        .rails
        .iter()
        .find(|rail| rail.selected)
        .map_or_else(|| panel.note.clone(), |rail| rail.note.clone());
    let support_height = usize::from(shell.content_area.height)
        .saturating_sub(panel.rails.len() + 2)
        .clamp(2, 4);

    let mut lines = vec![Line::from(vec![
        Span::styled(
            format!("{:<label_width$}", "factor"),
            theme.section_title(Tone::Muted),
        ),
        Span::styled(format!(" {:<bar_segments$}", "signal"), theme.annotation()),
        Span::styled(
            format!(" {:>delta_width$}", "delta"),
            theme.section_title(Tone::Muted),
        ),
    ])];
    lines.extend(
        panel
            .rails
            .iter()
            .map(|rail| breakdown_line(rail, label_width, theme, bar_segments, delta_width)),
    );
    let support_width = width.saturating_sub(label_width + 1).max(10);
    for (index, row) in stacked_profile_rows(&panel.waveform, support_width, support_height)
        .into_iter()
        .enumerate()
    {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<label_width$}", if index == 0 { "band" } else { "" }),
                theme.annotation(),
            ),
            Span::raw(" "),
            Span::styled(row, theme.chart_ramp(3, 4)),
        ]));
    }
    lines.push(Line::from(vec![
        Span::styled(
            format!("{:<label_width$}", "focus"),
            theme.section_title(Tone::Muted),
        ),
        Span::raw(" "),
        Span::styled(
            concise_text(&focus_note, width.saturating_sub(label_width + 1)),
            theme.annotation(),
        ),
    ]));
    render_panel_lines(frame, shell.content_area, lines, theme, Alignment::Left);
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
        let row = panel.recent.rows.first().map_or_else(
            || concise_detail(&panel.note, usize::from(shell.content_area.width)),
            |values| compact_heatmap_row(values, panel.recent.selected_cell),
        );
        render_panel_text(frame, shell.content_area, row, theme, Alignment::Left);
        return;
    }

    if !metric_panel_has_reading(panel.availability) {
        let lines = metric_panel_scaffold(
            panel.availability,
            &panel.note,
            usize::from(shell.content_area.width),
        );
        render_panel_text(
            frame,
            centered_body_area(shell.content_area, lines.len()),
            lines.join("\n"),
            theme,
            Alignment::Left,
        );
        return;
    }

    let width = usize::from(shell.content_area.width);
    let use_dense_history = panel.history.day_labels.len() > panel.recent.day_labels.len()
        && matches!(viewport, ViewportClass::Wide)
        && shell.content_area.width >= 40;
    let grid = if use_dense_history {
        &panel.history
    } else {
        &panel.recent
    };
    let row_refs = panel
        .row_labels
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let label_count = grid.day_labels.len().max(1);
    let mode = if use_dense_history {
        WeeklyHeatmapMode::DenseHistory
    } else {
        WeeklyHeatmapMode::Standard
    };
    let layout = weekly_heatmap_layout(
        mode,
        label_count,
        panel.row_labels.len(),
        width,
        usize::from(shell.content_area.height),
    );
    let mut lines = styled_heatmap_lines(HeatmapRenderSpec {
        theme,
        grid,
        row_labels: &row_refs,
        mode,
        layout,
    });
    lines.push(heatmap_summary_line(
        theme,
        grid,
        &panel.row_labels,
        &panel.note,
        layout,
        width,
    ));
    lines.push(heatmap_legend_line(theme, layout));
    let rendered_lines = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    if (state.focused || state.expanded) && shell.content_area.height > rendered_lines {
        lines.push(Line::from(Span::styled(
            concise_detail(&panel.note, width),
            theme.annotation(),
        )));
    }
    render_panel_lines(frame, shell.content_area, lines, theme, Alignment::Left);
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

fn selected_heatmap_summary(
    grid: &crate::app::DashboardHeatmapGrid,
    row_labels: &[String],
    note: &str,
) -> Option<(String, Option<DashboardScoreBand>)> {
    grid.selected_cell
        .and_then(|(row_index, column_index)| {
            let row = grid.rows.get(row_index)?;
            let value = row.get(column_index).copied().flatten()?;
            let row_label = row_labels.get(row_index)?;
            let day_label = grid.day_labels.get(column_index)?;
            let score_band = score_band_from_value(value);
            Some((
                format!("{row_label} {value} | {day_label}"),
                Some(score_band),
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

#[derive(Debug, Clone, Copy)]
struct HeatmapRenderSpec<'a> {
    theme: &'a Theme,
    grid: &'a crate::app::DashboardHeatmapGrid,
    row_labels: &'a [&'a str],
    mode: WeeklyHeatmapMode,
    layout: WeeklyHeatmapLayout,
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
            if selected_cell.is_some_and(|(row, col)| row == 0 && col == index) {
                format!("[{glyph}]")
            } else {
                glyph.to_string()
            }
        })
        .collect::<String>();
    format!("S {cells}")
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

fn trend_instrument_lines(title: &str, panel: &DashboardTrendPanel, width: usize) -> Vec<String> {
    match title {
        "HRV Trend" => vec![
            spark_strip(&panel.values, width.max(8)),
            micro_histogram(&panel.values, width.max(8)),
        ],
        "Heart Rate" => stacked_profile_rows(&panel.values, width.max(8), 2),
        "SpO2" => vec![
            centered_line(
                width,
                meter_bar(fill_percent(&panel.values), width.clamp(10, 18)),
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
    let label = concise_text(
        &format!("{} {}", if rail.selected { ">" } else { " " }, rail.label),
        label_width,
    );
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
    let label_style = if rail.selected {
        theme.section_title(Tone::Focus)
    } else {
        theme.body()
    };
    let cue_tone = if !availability_has_reading(rail.availability) {
        rail.availability.tone()
    } else if rail.judged_state.is_some() {
        judged_badge_tone(rail.judged_state)
    } else {
        delta_state_tone(rail.delta_state)
    };

    Line::from(vec![
        Span::styled(format!("{label:<label_width$}"), label_style),
        Span::styled(format!("[{marker_label:<5}]"), theme.badge(cue_tone)),
        Span::raw(" "),
        Span::styled(
            segmented_bar(rail.fill_percent, bar_segments),
            theme.annotation(),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:>delta_width$}", concise_text(&cue_text, delta_width)),
            theme.status_marker(cue_tone),
        ),
    ])
}

fn styled_heatmap_lines(spec: HeatmapRenderSpec<'_>) -> Vec<Line<'static>> {
    let HeatmapRenderSpec {
        theme,
        grid,
        row_labels,
        mode,
        layout,
    } = spec;
    let cell_width = layout.cell_width.max(1);
    let row_height = layout.row_height.max(1);
    let label_width = layout.label_column_width;
    let slot_width = layout.slot_width;
    let mut output = Vec::new();
    let mut header_spans = vec![Span::raw(" ".repeat(layout.grid_origin_x))];
    for day in &grid.day_labels {
        let label = heatmap_day_label(mode, day);
        header_spans.push(Span::styled(
            format!("{label:^slot_width$}"),
            theme.section_title(Tone::Muted),
        ));
    }
    output.push(Line::from(header_spans));

    for (row_index, label) in row_labels.iter().enumerate() {
        let mut row_spans = vec![Span::styled(
            format!(
                "{label:<label_width$}",
                label = fit_heatmap_label(label, label_width)
            ),
            theme.section_title(Tone::Muted),
        )];
        let mut repeat_spans = vec![Span::raw(" ".repeat(label_width))];
        if let Some(values) = grid.rows.get(row_index) {
            for (column_index, value) in values.iter().enumerate() {
                let band = (*value).map(score_band_from_value);
                let level = value.map_or(0, |score| usize::from(score) * 4 / 100 + 1);
                let glyph = match level {
                    0 => '·',
                    1 => '░',
                    2 => '▒',
                    3 => '▓',
                    _ => '█',
                };
                let fill = glyph.to_string().repeat(cell_width);
                if grid.selected_cell == Some((row_index, column_index)) {
                    let open = Span::styled("[", theme.badge(Tone::Focus));
                    let body = Span::styled(fill, theme.status_marker(score_band_cue_tone(band)));
                    let close = Span::styled("]", theme.badge(Tone::Focus));
                    row_spans.push(open.clone());
                    row_spans.push(body.clone());
                    row_spans.push(close.clone());
                    repeat_spans.push(open);
                    repeat_spans.push(body);
                    repeat_spans.push(close);
                } else {
                    let open = Span::raw(" ");
                    let body = Span::styled(fill, theme.chart_ramp(level, 5));
                    let close = Span::raw(" ");
                    row_spans.push(open.clone());
                    row_spans.push(body.clone());
                    row_spans.push(close.clone());
                    repeat_spans.push(open);
                    repeat_spans.push(body);
                    repeat_spans.push(close);
                }
            }
        }
        output.push(Line::from(row_spans));
        for _ in 1..row_height {
            output.push(Line::from(repeat_spans.clone()));
        }
    }

    output
}

fn heatmap_summary_line(
    theme: &Theme,
    grid: &crate::app::DashboardHeatmapGrid,
    row_labels: &[String],
    note: &str,
    layout: WeeklyHeatmapLayout,
    width: usize,
) -> Line<'static> {
    let (summary, band) =
        selected_heatmap_summary(grid, row_labels, note).unwrap_or_else(|| (note.to_owned(), None));
    let mut spans = vec![Span::raw(" ".repeat(layout.summary_origin_x))];
    spans.push(Span::styled(
        concise_text(&summary, width.saturating_sub(layout.summary_origin_x + 12)),
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

fn heatmap_legend_line(theme: &Theme, layout: WeeklyHeatmapLayout) -> Line<'static> {
    Line::from(vec![
        Span::raw(" ".repeat(layout.legend_origin_x)),
        Span::styled("ramp ", theme.section_title(Tone::Muted)),
        Span::styled("░", theme.chart_ramp(1, 5)),
        Span::styled("▒", theme.chart_ramp(2, 5)),
        Span::styled("▓", theme.chart_ramp(3, 5)),
        Span::styled("█", theme.chart_ramp(4, 5)),
        Span::styled(" higher", theme.annotation()),
    ])
}
