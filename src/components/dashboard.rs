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
        concise_text, meter_bar, micro_histogram, primary_secondary_line, segmented_bar,
        spark_strip, stacked_profile_rows, weekly_heatmap_rows,
    },
    theme::Theme,
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
    let trend_width = clamped_instrument_width(width, state.metrics, 14, 22);
    let capacity = usize::from(shell.content_area.height).max(1);
    let note_slot = usize::from(state.focused || state.expanded);
    let instrument_rows = if capacity >= 6 { 2 } else { 1 };
    let secondary_budget = capacity
        .saturating_sub(instrument_rows + 2 + note_slot)
        .min(if width >= 24 { 2 } else { 1 });

    let mut lines = Vec::new();
    if instrument_rows == 2 {
        lines.push(centered_line(
            width,
            spark_strip(&score_tile.trend, trend_width),
        ));
    }
    lines.push(centered_line(
        width,
        meter_bar(score_tile.ring_fill_percent, trend_width),
    ));
    lines.push(centered_line(
        width,
        concise_text(&score_tile.primary_value, width),
    ));
    lines.push(centered_line(
        width,
        concise_text(&score_tile.delta_label, width),
    ));
    lines.extend(
        score_tile
            .secondary_lines
            .iter()
            .take(secondary_budget)
            .map(|line| centered_line(width, concise_text(line, width))),
    );
    if state.focused || state.expanded {
        lines.push(centered_line(
            width,
            concise_detail(&score_tile.note, width),
        ));
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
    let band_width = clamped_instrument_width(width, state.metrics, 12, width.max(12));
    let capacity = usize::from(shell.content_area.height).max(4);
    let band_height = capacity
        .saturating_sub(if state.focused || state.expanded {
            3
        } else {
            2
        })
        .clamp(4, 7);
    let mut lines = vec![primary_secondary_line(
        &format!("duration {}", tile.duration_label),
        &tile.score_label,
        width,
    )];
    lines.extend(
        stacked_profile_rows(&tile.trend, band_width, band_height)
            .into_iter()
            .map(|row| centered_line(width, row)),
    );
    lines.push(centered_line(width, spark_strip(&tile.trend, band_width)));
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
    let mut lines = vec![concise_text(&panel.primary_label, width)];
    lines.extend(trend_instrument_lines(title, panel, width));
    lines.push(primary_secondary_line(
        &panel.baseline_label,
        &panel.range_label,
        width,
    ));
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
    let mut lines = vec![centered_line(
        width,
        concise_text(&panel.value_label, width),
    )];
    lines.extend(compact_temperature_lines(panel.deviation_tenths, width));
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
    let instrument_width = clamped_instrument_width(width, state.metrics, 10, width.max(10));
    let mut lines = vec![
        concise_text(&panel.primary_label, width),
        centered_line(
            width,
            meter_bar(fill_percent(&panel.bars), instrument_width),
        ),
        micro_histogram(&panel.bars, width.max(6)),
    ];
    if (state.focused || state.expanded) && shell.content_area.height >= 4 {
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
        .map(|rail| rail.label.len() + 2)
        .max()
        .unwrap_or(14)
        .min(width.saturating_div(3).max(12));
    let delta_width = panel
        .rails
        .iter()
        .map(|rail| rail.delta_label.len())
        .max()
        .unwrap_or(10)
        .clamp(8, 14);
    let bar_segments = width.saturating_sub(label_width + delta_width + 4).max(12);
    let focus_note = panel
        .rails
        .iter()
        .find(|rail| rail.selected)
        .map_or_else(|| panel.note.clone(), |rail| rail.note.clone());
    let support_height = usize::from(shell.content_area.height)
        .saturating_sub(panel.rails.len() + 2)
        .clamp(2, 4);

    let mut lines = vec![format!(
        "{:<label_width$} {:<bar_segments$} {:>delta_width$}",
        "factor", "signal", "delta"
    )];
    lines.extend(panel.rails.iter().map(|rail| {
        let label = format!("{} {}", if rail.selected { ">" } else { " " }, rail.label);
        let status = if availability_has_reading(rail.availability) {
            rail.delta_label.clone()
        } else {
            rail.availability.label().to_owned()
        };
        format!(
            "{label:<label_width$} {} {:>delta_width$}",
            segmented_bar(rail.fill_percent, bar_segments),
            concise_text(&status, delta_width),
        )
    }));
    let support_width = width.saturating_sub(label_width + 1).max(10);
    for (index, row) in stacked_profile_rows(&panel.waveform, support_width, support_height)
        .into_iter()
        .enumerate()
    {
        lines.push(format!(
            "{:<label_width$} {}",
            if index == 0 { "band" } else { "" },
            row
        ));
    }
    lines.push(format!(
        "{:<label_width$} {}",
        "focus",
        concise_text(&focus_note, width.saturating_sub(label_width + 1),)
    ));
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
    let label_width = if use_dense_history { 6 } else { 8 };
    let cell_width = width
        .saturating_sub(label_width)
        .checked_div(label_count)
        .unwrap_or(1)
        .saturating_sub(1)
        .clamp(2, 6);
    let mode = if use_dense_history {
        WeeklyHeatmapMode::DenseHistory
    } else {
        WeeklyHeatmapMode::Standard
    };
    let row_height = usize::from(shell.content_area.height)
        .saturating_sub(3)
        .checked_div(grid.rows.len().max(1))
        .unwrap_or(1)
        .clamp(1, 2);
    let mut lines = weekly_heatmap_rows(
        &grid.day_labels,
        &row_refs,
        &grid.rows,
        grid.selected_cell,
        mode,
        cell_width,
        row_height,
    );
    lines.push(concise_text(
        &selected_heatmap_summary(grid, &panel.row_labels, &panel.note),
        width,
    ));
    lines.push("band ░▒▓ higher".to_owned());
    let rendered_lines = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    if (state.focused || state.expanded) && shell.content_area.height > rendered_lines {
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
            Some(format!("{row_label} {value} | {day_label}"))
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
