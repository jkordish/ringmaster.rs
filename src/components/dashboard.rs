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
    layout::{
        BreakdownLayout, DashboardChartMetrics, DashboardMetrics, UiContext, ViewportClass,
        WeeklyHeatmapMode, WeeklyTrendsLayout, panel_content_metrics,
    },
    telemetry::{
        MetricPanelState, TelemetryAvailability, heatmap_day_label, meter_bar,
        metric_panel_scaffold, micro_histogram, placeholder_rule, segmented_bar, spark_strip,
        stacked_profile_rows,
    },
    text_fit::{
        concise_detail, concise_text, fit_badge_label, fit_breakdown_delta, fit_breakdown_label,
        fit_day_header, fit_heatmap_label, measure_one_line, support_lane_text,
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
        render_metric_scaffold_with_support(
            frame,
            shell.content_area,
            tile.availability,
            &tile.strip_note,
            theme,
            Alignment::Center,
            state.chart_metrics,
        );
        return;
    }

    let content = panel_content_metrics(
        shell.content_area,
        measured_panel_support_lane(shell.content_area, state.chart_metrics),
    );
    let width = usize::from(content.chart.area.width);
    let cue_tone = score_band_cue_tone(tile.score_band);
    let band_width = clamped_instrument_width(width, state.metrics, 12, width.max(12));
    let capacity = usize::from(content.chart.area.height).max(4);
    let band_height = capacity.saturating_sub(2).clamp(4, 7);
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
        &tile.strip_note,
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
        render_metric_scaffold_with_support(
            frame,
            shell.content_area,
            panel.availability,
            &panel.note,
            theme,
            Alignment::Left,
            state.chart_metrics,
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
        render_metric_scaffold_with_support(
            frame,
            shell.content_area,
            panel.availability,
            &panel.note,
            theme,
            Alignment::Center,
            state.chart_metrics,
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
        render_metric_scaffold_with_support(
            frame,
            shell.content_area,
            panel.availability,
            &panel.note,
            theme,
            Alignment::Center,
            state.chart_metrics,
        );
        return;
    }

    let content = panel_content_metrics(
        shell.content_area,
        measured_panel_support_lane(shell.content_area, state.chart_metrics),
    );
    let width = usize::from(content.chart.area.width);
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
    render_panel_lines(frame, content.chart.area, lines, theme, Alignment::Center);
    render_support_lane(
        frame,
        content.support.area,
        &panel.note,
        theme,
        Alignment::Center,
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
        render_metric_scaffold_with_support(
            frame,
            shell.content_area,
            panel.availability,
            &panel.note,
            theme,
            Alignment::Left,
            state.chart_metrics,
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
                "factor",
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
        Line::from(Span::styled("signal", theme.annotation())),
        theme,
        Alignment::Left,
    );
    render_line_in_area(
        frame,
        layout.delta_cell(layout.header_area),
        Line::from(Span::styled("delta", theme.section_title(Tone::Muted))),
        theme,
        Alignment::Right,
    );

    for (index, rail) in panel.rails.iter().enumerate() {
        let row = layout.row_area(index);
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

    if layout.band_area.height > 0 {
        render_line_in_area(
            frame,
            layout.label_cell(layout.band_area),
            Line::from(Span::styled(
                format!(
                    "{:<width$}",
                    "band",
                    width = usize::from(layout.label_column_width)
                ),
                theme.annotation(),
            )),
            theme,
            Alignment::Left,
        );
        let band_rows = stacked_profile_rows(
            &panel.waveform,
            usize::from(layout.bar_viewport_width),
            usize::from(layout.band_area.height),
        );
        render_panel_lines(
            frame,
            layout.signal_track_cell(layout.band_area),
            band_rows
                .into_iter()
                .map(|row| Line::from(Span::styled(row, theme.chart_ramp(3, 4))))
                .collect(),
            theme,
            Alignment::Left,
        );
    }

    if layout.support_lane.height > 0 {
        render_line_in_area(
            frame,
            layout.support_lane,
            Line::from(Span::styled(
                support_lane_text(&focus_note, usize::from(layout.support_lane.width)),
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
        let row = panel.recent.rows.first().map_or_else(
            || concise_detail(&panel.note, usize::from(shell.content_area.width)),
            |values| compact_heatmap_row(values, panel.recent.selected_cell),
        );
        render_panel_text(frame, shell.content_area, row, theme, Alignment::Left);
        return;
    }

    if !metric_panel_has_reading(panel.availability) {
        render_metric_scaffold_with_support(
            frame,
            shell.content_area,
            panel.availability,
            &panel.note,
            theme,
            Alignment::Left,
            state.chart_metrics,
        );
        return;
    }

    let grid = panel.grid_for_viewport(viewport);
    let mode =
        if viewport.is_wide() && panel.history.day_labels.len() > panel.recent.day_labels.len() {
            WeeklyHeatmapMode::DenseHistory
        } else {
            WeeklyHeatmapMode::Standard
        };
    let layout = WeeklyTrendsLayout::for_panel(
        shell.content_area,
        state.chart_metrics,
        mode,
        grid.day_labels.len(),
        panel.row_labels.len(),
    );
    if layout.grid_viewport.height == 0
        || layout.legend_area.height == 0
        || layout.summary_area.height == 0
    {
        render_panel_text(
            frame,
            shell.content_area,
            concise_detail(&panel.note, usize::from(shell.content_area.width)),
            theme,
            Alignment::Left,
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

    for (row_index, label) in panel.row_labels.iter().enumerate() {
        let label_area = layout.row_label_area(row_index);
        let label_line = Line::from(Span::styled(
            format!(
                "{:<width$}",
                fit_heatmap_label(label, usize::from(layout.label_column_width)),
                width = usize::from(layout.label_column_width)
            ),
            theme.section_title(Tone::Muted),
        ));
        let label_lines = vec![label_line; usize::from(layout.row_height.max(1))];
        render_panel_lines(frame, label_area, label_lines, theme, Alignment::Left);
        let row_lines = grid.rows.get(row_index).map_or_else(
            || vec![Line::from(Span::raw(String::new())); usize::from(layout.row_height.max(1))],
            |values| heatmap_row_lines(theme, values, row_index, grid.selected_cell, layout),
        );
        render_panel_lines(
            frame,
            layout.row_area(row_index),
            row_lines,
            theme,
            Alignment::Left,
        );
    }

    render_line_in_area(
        frame,
        layout.legend_area,
        heatmap_legend_line(theme, layout),
        theme,
        Alignment::Left,
    );
    render_line_in_area(
        frame,
        layout.summary_area,
        heatmap_summary_line(theme, grid, &panel.row_labels, &panel.note, layout),
        theme,
        Alignment::Left,
    );
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

fn render_metric_scaffold_with_support(
    frame: &mut Frame<'_>,
    area: Rect,
    state: MetricPanelState,
    reason: &str,
    theme: &Theme,
    alignment: Alignment,
    chart_metrics: DashboardChartMetrics,
) {
    let layout = panel_content_metrics(area, measured_panel_support_lane(area, chart_metrics));
    if layout.support.area.height == 0 {
        render_panel_text(
            frame,
            area,
            metric_panel_scaffold(state, reason, usize::from(area.width)).join("\n"),
            theme,
            alignment,
        );
        return;
    }

    let width = usize::from(layout.chart.area.width).max(10);
    let body_lines = [
        match alignment {
            Alignment::Left => format!("{:<width$}", state.label()),
            Alignment::Right => format!("{:>width$}", state.label()),
            Alignment::Center => format!("{:^width$}", state.label()),
        },
        placeholder_rule(width),
    ];
    render_panel_text(
        frame,
        centered_body_area(layout.chart.area, body_lines.len()),
        body_lines.join("\n"),
        theme,
        alignment,
    );
    render_support_lane(frame, layout.support.area, reason, theme, alignment);
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
    layout: WeeklyTrendsLayout,
) -> Line<'static> {
    let (summary, band) =
        selected_heatmap_summary(grid, row_labels, note).unwrap_or_else(|| (note.to_owned(), None));
    let mut spans = Vec::new();
    spans.push(Span::styled(
        support_lane_text(
            &summary,
            usize::from(layout.summary_area.width).saturating_sub(if band.is_some() {
                10
            } else {
                0
            }),
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

fn heatmap_row_lines(
    theme: &Theme,
    values: &[Option<u8>],
    row_index: usize,
    selected_cell: Option<(usize, usize)>,
    layout: WeeklyTrendsLayout,
) -> Vec<Line<'static>> {
    let mut spans = Vec::new();
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
        let fill = glyph.to_string().repeat(usize::from(layout.cell_width));
        if selected_cell == Some((row_index, column_index)) {
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
    let line = Line::from(spans);
    vec![line; usize::from(layout.row_height.max(1))]
}
