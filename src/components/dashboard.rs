use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::{Alignment, Rect},
    widgets::Paragraph,
};

use crate::app::{
    DashboardBreakdownPanel, DashboardHistogramPanel, DashboardModel, DashboardScoreTile,
    DashboardSleepTile, DashboardThermometerPanel, DashboardTrendPanel, DashboardWeeklyHeatmap,
};
use crate::navigation::FocusRegion;
use crate::ui::{
    chrome::{PanelKind, PanelShellSpec, render_panel_shell},
    layout::{DashboardMetrics, UiContext, ViewportClass},
    telemetry::{
        TelemetryAvailability, WeeklyHeatmapMode, availability_scaffold, concise_detail,
        concise_text, micro_histogram, primary_secondary_line, score_ring_lines, segmented_bar,
        spark_strip, thermometer_lines, weekly_heatmap_rows,
    },
    theme::Theme,
};

#[derive(Debug, Clone, Copy)]
struct PanelRenderState {
    focused: bool,
    expanded: bool,
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
    match ui.viewport {
        ViewportClass::Compact => {
            draw_compact(
                frame,
                area,
                model,
                theme,
                focused_region,
                expanded_region,
                metrics,
            );
        }
        ViewportClass::Medium => {
            draw_medium(
                frame,
                area,
                model,
                theme,
                focused_region,
                expanded_region,
                metrics,
            );
        }
        ViewportClass::Wide => {
            draw_wide(
                frame,
                area,
                model,
                theme,
                focused_region,
                expanded_region,
                metrics,
            );
        }
    }
}

fn draw_wide(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &DashboardModel,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
    metrics: DashboardMetrics,
) {
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
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
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
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
    metrics: DashboardMetrics,
) {
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
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
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
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
    metrics: DashboardMetrics,
) {
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
        let compact = format!("{} {}", score_tile.primary_value, score_tile.delta_label);
        render_panel_text(frame, shell.content_area, compact, theme, Alignment::Center);
        return;
    }

    if !availability_has_reading(score_tile.availability) {
        let lines = availability_scaffold(
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
    let trend = spark_strip(&score_tile.trend, width.saturating_sub(2).max(8));
    let subtitle = score_tile
        .secondary_lines
        .first()
        .map(|line| concise_text(line, width.min(22)));
    let mut lines = score_ring_lines(
        &score_tile.primary_value,
        score_tile.ring_fill_percent,
        Some(score_tile.delta_label.as_str()),
        &trend,
        subtitle.as_deref(),
    );
    if let Some(secondary) = score_tile.secondary_lines.get(1) {
        lines.push(concise_text(secondary, width));
    }
    if state.focused || state.expanded {
        lines.push(concise_detail(&score_tile.note, width));
    }
    render_panel_text(
        frame,
        centered_body_area(shell.content_area, lines.len()),
        lines.join("\n"),
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
        render_panel_text(
            frame,
            shell.content_area,
            format!("{} {}", tile.duration_label, tile.score_label),
            theme,
            Alignment::Center,
        );
        return;
    }

    if !availability_has_reading(tile.availability) {
        let lines = availability_scaffold(
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
    let strip = micro_histogram(&tile.trend, width.max(8));
    let spark = spark_strip(&tile.trend, width.max(8));
    let mut lines = vec![
        primary_secondary_line(
            &format!("duration {}", tile.duration_label),
            &tile.score_label,
            width,
        ),
        strip,
        spark,
    ];
    if state.focused || state.expanded {
        lines.push(concise_detail(&tile.strip_note, width));
    }
    render_panel_text(
        frame,
        centered_body_area(shell.content_area, lines.len()),
        lines.join("\n"),
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
        let compact = if availability_has_reading(panel.availability) {
            concise_text(&panel.primary_label, usize::from(shell.content_area.width))
        } else {
            concise_detail(&panel.note, usize::from(shell.content_area.width))
        };
        render_panel_text(frame, shell.content_area, compact, theme, Alignment::Left);
        return;
    }

    if !availability_has_reading(panel.availability) {
        render_panel_text(
            frame,
            shell.content_area,
            availability_scaffold(
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
    let mut lines = vec![
        concise_text(&panel.primary_label, width),
        spark_strip(&panel.values, width.max(6)),
        primary_secondary_line(&panel.baseline_label, &panel.range_label, width),
    ];
    if state.focused || state.expanded {
        lines.push(concise_detail(&panel.note, width));
    }
    render_panel_text(
        frame,
        shell.content_area,
        lines.join("\n"),
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
        render_panel_text(
            frame,
            shell.content_area,
            concise_text(&panel.value_label, usize::from(shell.content_area.width)),
            theme,
            Alignment::Center,
        );
        return;
    }

    if !availability_has_reading(panel.availability) {
        let lines = availability_scaffold(
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
    let value = panel.deviation_tenths.map(|raw| f64::from(raw) / 10.0);
    let mut lines = thermometer_lines(value, &panel.value_label);
    if state.focused || state.expanded {
        lines.push(concise_detail(&panel.note, width));
    }
    render_panel_text(
        frame,
        centered_body_area(shell.content_area, lines.len()),
        lines.join("\n"),
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
        let compact = if availability_has_reading(panel.availability) {
            concise_text(&panel.primary_label, usize::from(shell.content_area.width))
        } else {
            concise_detail(&panel.note, usize::from(shell.content_area.width))
        };
        render_panel_text(frame, shell.content_area, compact, theme, Alignment::Center);
        return;
    }

    if !availability_has_reading(panel.availability) {
        render_panel_text(
            frame,
            shell.content_area,
            availability_scaffold(
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
    let mut lines = vec![
        concise_text(&panel.primary_label, width),
        micro_histogram(&panel.bars, width.max(6)),
    ];
    if state.focused || state.expanded {
        lines.push(concise_detail(&panel.note, width));
    }
    render_panel_text(
        frame,
        shell.content_area,
        lines.join("\n"),
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
        let compact = panel.rails.iter().find(|rail| rail.selected).map_or_else(
            || concise_detail(&panel.note, usize::from(shell.content_area.width)),
            |rail| format!("{} {}", rail.label, rail.delta_label),
        );
        render_panel_text(frame, shell.content_area, compact, theme, Alignment::Left);
        return;
    }

    if !availability_has_reading(panel.availability) {
        render_panel_text(
            frame,
            shell.content_area,
            availability_scaffold(
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
        .map(|rail| rail.label.len() + 1)
        .max()
        .unwrap_or(12)
        .min(width.saturating_div(3).max(10));
    let delta_width = panel
        .rails
        .iter()
        .map(|rail| rail.delta_label.len())
        .max()
        .unwrap_or(5)
        .min(10);
    let bar_segments = width.saturating_sub(label_width + delta_width + 4).max(8);

    let mut lines = panel
        .rails
        .iter()
        .map(|rail| {
            let label = format!("{}{}", if rail.selected { ">" } else { " " }, rail.label);
            format!(
                "{label:<label_width$} {} {:>delta_width$}",
                segmented_bar(rail.fill_percent, bar_segments),
                concise_text(&rail.delta_label, delta_width),
            )
        })
        .collect::<Vec<_>>();
    lines.push(format!(
        "{:<label_width$} {}",
        " signal",
        spark_strip(
            &panel.waveform,
            width.saturating_sub(label_width + 1).max(8)
        )
    ));
    lines.push(format!(
        "{:<label_width$} {}",
        " drift",
        micro_histogram(
            &panel.waveform,
            width.saturating_sub(label_width + 1).max(8)
        )
    ));
    if state.focused || state.expanded {
        let detail = panel
            .rails
            .iter()
            .find(|rail| rail.selected)
            .map_or_else(|| panel.note.clone(), |rail| rail.note.clone());
        lines.push(concise_detail(&detail, width));
    }
    render_panel_text(
        frame,
        shell.content_area,
        lines.join("\n"),
        theme,
        Alignment::Left,
    );
}

fn render_heatmap_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    panel: &DashboardWeeklyHeatmap,
    theme: &Theme,
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

    if !availability_has_reading(panel.availability) {
        let lines = availability_scaffold(
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
    let cell_width = width
        .saturating_sub(if use_dense_history { 6 } else { 8 })
        .checked_div(label_count)
        .unwrap_or(1)
        .saturating_sub(2)
        .clamp(1, 5);
    let mode = if use_dense_history {
        WeeklyHeatmapMode::DenseHistory
    } else {
        WeeklyHeatmapMode::Standard
    };
    let mut lines = weekly_heatmap_rows(
        &grid.day_labels,
        &row_refs,
        &grid.rows,
        grid.selected_cell,
        mode,
        cell_width,
    );
    lines.push("lower ░▒▓ higher".to_owned());
    if state.focused || state.expanded {
        lines.push(concise_detail(
            &selected_heatmap_summary(grid, &panel.row_labels, &panel.note),
            width,
        ));
    }
    render_panel_text(
        frame,
        centered_body_area(shell.content_area, lines.len()),
        lines.join("\n"),
        theme,
        Alignment::Left,
    );
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
) -> String {
    grid.selected_cell
        .and_then(|(row_index, column_index)| {
            let row = grid.rows.get(row_index)?;
            let value = row.get(column_index).copied().flatten()?;
            let row_label = row_labels.get(row_index)?;
            let day_label = grid.day_labels.get(column_index)?;
            Some(format!("{row_label} {value} on {day_label}"))
        })
        .unwrap_or_else(|| note.to_owned())
}

fn render_panel_text(
    frame: &mut Frame<'_>,
    area: Rect,
    text: String,
    theme: &Theme,
    alignment: Alignment,
) {
    frame.render_widget(
        Paragraph::new(text)
            .style(theme.body())
            .alignment(alignment),
        area,
    );
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
