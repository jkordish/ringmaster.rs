use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    widgets::Paragraph,
};

use crate::app::{
    CoverageCellView, DashboardBreakdownPanel, DashboardHistogramPanel, DashboardModel,
    DashboardScoreTile, DashboardSleepTile, DashboardThermometerPanel, DashboardTrendPanel,
    DashboardWeeklyHeatmap,
};
use crate::navigation::FocusRegion;
use crate::ui::{
    layout::{UiContext, ViewportClass},
    telemetry::{micro_histogram, panel_block, score_ring_lines, spark_strip, weekly_heatmap_rows},
    theme::Theme,
};

pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &DashboardModel,
    ui: &UiContext,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
) {
    match ui.viewport {
        ViewportClass::Compact => {
            draw_compact(frame, area, model, theme, focused_region, expanded_region);
        }
        ViewportClass::Medium => {
            draw_medium(frame, area, model, theme, focused_region, expanded_region);
        }
        ViewportClass::Wide => {
            draw_wide(frame, area, model, theme, focused_region, expanded_region);
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
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(20),
            Constraint::Min(10),
        ])
        .split(area);
    render_header_strip(frame, layout[0], model, theme);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(29),
            Constraint::Percentage(39),
            Constraint::Percentage(32),
        ])
        .split(layout[1]);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(top[0]);
    let center = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(46), Constraint::Percentage(54)])
        .split(top[1]);
    let center_lower = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(24),
            Constraint::Percentage(43),
            Constraint::Percentage(33),
        ])
        .split(center[1]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(66), Constraint::Percentage(34)])
        .split(top[2]);
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(69), Constraint::Percentage(31)])
        .split(layout[2]);

    render_score_tile(
        frame,
        left[0],
        "Readiness",
        &model.readiness,
        theme,
        focused_region == FocusRegion::DashboardReadiness,
        expanded_region == Some(FocusRegion::DashboardReadiness),
    );
    render_trend_panel(
        frame,
        left[1],
        "HRV Trend",
        &model.hrv,
        theme,
        focused_region == FocusRegion::DashboardHrv,
        expanded_region == Some(FocusRegion::DashboardHrv),
    );
    render_sleep_tile(
        frame,
        center[0],
        &model.sleep,
        theme,
        focused_region == FocusRegion::DashboardSleep,
        expanded_region == Some(FocusRegion::DashboardSleep),
    );
    render_temp_panel(
        frame,
        center_lower[0],
        &model.body_temp,
        theme,
        focused_region == FocusRegion::DashboardTemp,
        expanded_region == Some(FocusRegion::DashboardTemp),
    );
    render_trend_panel(
        frame,
        center_lower[1],
        "Heart Rate",
        &model.heart_rate,
        theme,
        focused_region == FocusRegion::DashboardHeartRate,
        expanded_region == Some(FocusRegion::DashboardHeartRate),
    );
    render_trend_panel(
        frame,
        center_lower[2],
        "SpO2",
        &model.spo2,
        theme,
        focused_region == FocusRegion::DashboardSpo2,
        expanded_region == Some(FocusRegion::DashboardSpo2),
    );
    render_score_tile(
        frame,
        right[0],
        "Activity",
        &model.activity,
        theme,
        focused_region == FocusRegion::DashboardActivity,
        expanded_region == Some(FocusRegion::DashboardActivity),
    );
    render_histogram_panel(
        frame,
        right[1],
        "Resp Rate",
        &model.respiratory_rate,
        theme,
        focused_region == FocusRegion::DashboardRespRate,
        expanded_region == Some(FocusRegion::DashboardRespRate),
    );
    render_breakdown_panel(
        frame,
        bottom[0],
        &model.breakdown,
        theme,
        focused_region == FocusRegion::DashboardBreakdown,
        expanded_region == Some(FocusRegion::DashboardBreakdown),
    );
    render_heatmap_panel(
        frame,
        bottom[1],
        &model.weekly,
        theme,
        focused_region == FocusRegion::DashboardHeatmap,
        expanded_region == Some(FocusRegion::DashboardHeatmap),
    );
}

fn draw_medium(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &DashboardModel,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(11),
            Constraint::Length(8),
            Constraint::Min(11),
        ])
        .split(area);
    render_header_strip(frame, layout[0], model, theme);

    let row1 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(layout[1]);
    let row2 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(26),
            Constraint::Percentage(18),
            Constraint::Percentage(30),
            Constraint::Percentage(26),
        ])
        .split(layout[2]);
    let row3 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(layout[3]);
    let row3_right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(row3[1]);

    render_score_tile(
        frame,
        row1[0],
        "Readiness",
        &model.readiness,
        theme,
        focused_region == FocusRegion::DashboardReadiness,
        expanded_region == Some(FocusRegion::DashboardReadiness),
    );
    render_sleep_tile(
        frame,
        row1[1],
        &model.sleep,
        theme,
        focused_region == FocusRegion::DashboardSleep,
        expanded_region == Some(FocusRegion::DashboardSleep),
    );
    render_score_tile(
        frame,
        row1[2],
        "Activity",
        &model.activity,
        theme,
        focused_region == FocusRegion::DashboardActivity,
        expanded_region == Some(FocusRegion::DashboardActivity),
    );
    render_trend_panel(
        frame,
        row2[0],
        "HRV Trend",
        &model.hrv,
        theme,
        focused_region == FocusRegion::DashboardHrv,
        expanded_region == Some(FocusRegion::DashboardHrv),
    );
    render_temp_panel(
        frame,
        row2[1],
        &model.body_temp,
        theme,
        focused_region == FocusRegion::DashboardTemp,
        expanded_region == Some(FocusRegion::DashboardTemp),
    );
    render_trend_panel(
        frame,
        row2[2],
        "Heart Rate",
        &model.heart_rate,
        theme,
        focused_region == FocusRegion::DashboardHeartRate,
        expanded_region == Some(FocusRegion::DashboardHeartRate),
    );
    render_trend_panel(
        frame,
        row2[3],
        "SpO2",
        &model.spo2,
        theme,
        focused_region == FocusRegion::DashboardSpo2,
        expanded_region == Some(FocusRegion::DashboardSpo2),
    );
    render_breakdown_panel(
        frame,
        row3[0],
        &model.breakdown,
        theme,
        focused_region == FocusRegion::DashboardBreakdown,
        expanded_region == Some(FocusRegion::DashboardBreakdown),
    );
    render_histogram_panel(
        frame,
        row3_right[0],
        "Resp Rate",
        &model.respiratory_rate,
        theme,
        focused_region == FocusRegion::DashboardRespRate,
        expanded_region == Some(FocusRegion::DashboardRespRate),
    );
    render_heatmap_panel(
        frame,
        row3_right[1],
        &model.weekly,
        theme,
        focused_region == FocusRegion::DashboardHeatmap,
        expanded_region == Some(FocusRegion::DashboardHeatmap),
    );
}

fn draw_compact(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &DashboardModel,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
        ])
        .split(area);
    render_score_tile(
        frame,
        layout[0],
        "Readiness",
        &model.readiness,
        theme,
        focused_region == FocusRegion::DashboardReadiness,
        expanded_region == Some(FocusRegion::DashboardReadiness),
    );
    render_sleep_tile(
        frame,
        layout[1],
        &model.sleep,
        theme,
        focused_region == FocusRegion::DashboardSleep,
        expanded_region == Some(FocusRegion::DashboardSleep),
    );
    render_score_tile(
        frame,
        layout[2],
        "Activity",
        &model.activity,
        theme,
        focused_region == FocusRegion::DashboardActivity,
        expanded_region == Some(FocusRegion::DashboardActivity),
    );

    let phys1 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(layout[3]);
    let phys2 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[4]);
    render_trend_panel(
        frame,
        phys1[0],
        "HRV Trend",
        &model.hrv,
        theme,
        focused_region == FocusRegion::DashboardHrv,
        expanded_region == Some(FocusRegion::DashboardHrv),
    );
    render_temp_panel(
        frame,
        phys1[1],
        &model.body_temp,
        theme,
        focused_region == FocusRegion::DashboardTemp,
        expanded_region == Some(FocusRegion::DashboardTemp),
    );
    render_trend_panel(
        frame,
        phys1[2],
        "SpO2",
        &model.spo2,
        theme,
        focused_region == FocusRegion::DashboardSpo2,
        expanded_region == Some(FocusRegion::DashboardSpo2),
    );
    render_trend_panel(
        frame,
        phys2[0],
        "Heart Rate",
        &model.heart_rate,
        theme,
        focused_region == FocusRegion::DashboardHeartRate,
        expanded_region == Some(FocusRegion::DashboardHeartRate),
    );
    render_histogram_panel(
        frame,
        phys2[1],
        "Resp Rate",
        &model.respiratory_rate,
        theme,
        focused_region == FocusRegion::DashboardRespRate,
        expanded_region == Some(FocusRegion::DashboardRespRate),
    );

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(layout[5]);
    render_breakdown_panel(
        frame,
        bottom[0],
        &model.breakdown,
        theme,
        focused_region == FocusRegion::DashboardBreakdown,
        expanded_region == Some(FocusRegion::DashboardBreakdown),
    );
    render_heatmap_panel(
        frame,
        bottom[1],
        &model.weekly,
        theme,
        focused_region == FocusRegion::DashboardHeatmap,
        expanded_region == Some(FocusRegion::DashboardHeatmap),
    );
}

fn render_header_strip(frame: &mut Frame<'_>, area: Rect, model: &DashboardModel, theme: &Theme) {
    let coverage_summary =
        coverage_summary(&model.header.coverage, usize::from(area.width / 14).max(2));
    let body = [
        format!(
            "{} | {} | {} | {}",
            model.header.app_title,
            model.header.selected_period,
            model.header.freshness_badge,
            model.header.sync_status
        ),
        format!(
            "{} || {}",
            model.header.capability_summary.join("  "),
            coverage_summary
        ),
    ]
    .join("\n");
    frame.render_widget(
        Paragraph::new(body).block(panel_block(
            theme,
            "Header / Status",
            "LIVE",
            crate::ui::theme::Tone::Info,
            false,
            false,
        )),
        area,
    );
}

fn coverage_summary(cells: &[CoverageCellView], max_items: usize) -> String {
    if cells.is_empty() {
        return "coverage unavailable".to_owned();
    }
    cells
        .iter()
        .take(max_items)
        .map(|cell| format!("{}:{}", cell.label, cell.availability.label()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_score_tile(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    score_tile: &DashboardScoreTile,
    theme: &Theme,
    focused: bool,
    expanded: bool,
) {
    if area.height <= 3 {
        let compact = score_tile.secondary_lines.first().map_or_else(
            || format!("{} | {}", score_tile.primary_value, score_tile.delta_label),
            |secondary| format!("{} | {}", score_tile.primary_value, secondary),
        );
        frame.render_widget(
            Paragraph::new(compact).block(panel_block(
                theme,
                title,
                score_tile.availability.label(),
                score_tile.availability.tone(),
                focused,
                expanded,
            )),
            area,
        );
        return;
    }

    let trend_width = usize::from(area.width.saturating_sub(8).max(8));
    let trend = spark_strip(&score_tile.trend, trend_width);
    let subtitle = score_tile.secondary_lines.first().map(String::as_str);
    let mut lines = score_ring_lines(
        &score_tile.primary_value,
        score_tile.ring_fill_percent,
        Some(score_tile.delta_label.as_str()),
        &trend,
        subtitle,
    );
    if let Some(secondary) = score_tile.secondary_lines.get(1) {
        lines.push(secondary.clone());
    }
    lines.push(score_tile.note.clone());
    frame.render_widget(
        Paragraph::new(lines.join("\n")).block(panel_block(
            theme,
            title,
            score_tile.availability.label(),
            score_tile.availability.tone(),
            focused,
            expanded,
        )),
        area,
    );
}

fn render_sleep_tile(
    frame: &mut Frame<'_>,
    area: Rect,
    tile: &DashboardSleepTile,
    theme: &Theme,
    focused: bool,
    expanded: bool,
) {
    if area.height <= 3 {
        let compact = format!("{} | {}", tile.duration_label, tile.score_label);
        frame.render_widget(
            Paragraph::new(compact).block(panel_block(
                theme,
                "Sleep",
                tile.availability.label(),
                tile.availability.tone(),
                focused,
                expanded,
            )),
            area,
        );
        return;
    }

    let strip = micro_histogram(
        &tile.trend,
        usize::from(area.width.saturating_sub(8).max(8)),
    );
    let body = [
        format!("duration {}", tile.duration_label),
        format!("{} | {}", tile.score_label, strip),
        tile.strip_note.clone(),
    ]
    .join("\n");
    frame.render_widget(
        Paragraph::new(body).block(panel_block(
            theme,
            "Sleep",
            tile.availability.label(),
            tile.availability.tone(),
            focused,
            expanded,
        )),
        area,
    );
}

fn render_trend_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    panel: &DashboardTrendPanel,
    theme: &Theme,
    focused: bool,
    expanded: bool,
) {
    if area.height <= 3 {
        let compact = if matches!(
            panel.availability,
            crate::ui::telemetry::TelemetryAvailability::NoData
                | crate::ui::telemetry::TelemetryAvailability::MissingScope
                | crate::ui::telemetry::TelemetryAvailability::RateLimited
                | crate::ui::telemetry::TelemetryAvailability::Error
                | crate::ui::telemetry::TelemetryAvailability::Unsupported
        ) {
            panel.note.clone()
        } else {
            format!("{} | {}", panel.primary_label, panel.baseline_label)
        };
        frame.render_widget(
            Paragraph::new(compact).block(panel_block(
                theme,
                title,
                panel.availability.label(),
                panel.availability.tone(),
                focused,
                expanded,
            )),
            area,
        );
        return;
    }

    let body = [
        panel.primary_label.clone(),
        spark_strip(
            &panel.values,
            usize::from(area.width.saturating_sub(6).max(6)),
        ),
        panel.baseline_label.clone(),
        panel.range_label.clone(),
        panel.note.clone(),
    ]
    .join("\n");
    frame.render_widget(
        Paragraph::new(body).block(panel_block(
            theme,
            title,
            panel.availability.label(),
            panel.availability.tone(),
            focused,
            expanded,
        )),
        area,
    );
}

fn render_temp_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    panel: &DashboardThermometerPanel,
    theme: &Theme,
    focused: bool,
    expanded: bool,
) {
    if area.height <= 3 {
        frame.render_widget(
            Paragraph::new(panel.value_label.clone()).block(panel_block(
                theme,
                "Body Temp",
                panel.availability.label(),
                panel.availability.tone(),
                focused,
                expanded,
            )),
            area,
        );
        return;
    }

    let value = panel.deviation_tenths.map(|value| f64::from(value) / 10.0);
    let mut lines = crate::ui::telemetry::thermometer_lines(value, &panel.value_label);
    lines.push(panel.note.clone());
    frame.render_widget(
        Paragraph::new(lines.join("\n")).block(panel_block(
            theme,
            "Body Temp",
            panel.availability.label(),
            panel.availability.tone(),
            focused,
            expanded,
        )),
        area,
    );
}

fn render_histogram_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    panel: &DashboardHistogramPanel,
    theme: &Theme,
    focused: bool,
    expanded: bool,
) {
    if area.height <= 3 {
        let compact = if matches!(
            panel.availability,
            crate::ui::telemetry::TelemetryAvailability::NoData
                | crate::ui::telemetry::TelemetryAvailability::MissingScope
                | crate::ui::telemetry::TelemetryAvailability::RateLimited
                | crate::ui::telemetry::TelemetryAvailability::Error
                | crate::ui::telemetry::TelemetryAvailability::Unsupported
        ) {
            panel.note.clone()
        } else {
            panel.primary_label.clone()
        };
        frame.render_widget(
            Paragraph::new(compact).block(panel_block(
                theme,
                title,
                panel.availability.label(),
                panel.availability.tone(),
                focused,
                expanded,
            )),
            area,
        );
        return;
    }

    let body = [
        panel.primary_label.clone(),
        micro_histogram(
            &panel.bars,
            usize::from(area.width.saturating_sub(6).max(6)),
        ),
        panel.note.clone(),
    ]
    .join("\n");
    frame.render_widget(
        Paragraph::new(body).block(panel_block(
            theme,
            title,
            panel.availability.label(),
            panel.availability.tone(),
            focused,
            expanded,
        )),
        area,
    );
}

fn render_breakdown_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    panel: &DashboardBreakdownPanel,
    theme: &Theme,
    focused: bool,
    expanded: bool,
) {
    if area.height <= 3 {
        let detail = panel.rails.iter().find(|rail| rail.selected).map_or_else(
            || panel.note.clone(),
            |rail| format!("{} {}", rail.label, rail.delta_label),
        );
        frame.render_widget(
            Paragraph::new(detail).block(panel_block(
                theme,
                "Readiness Breakdown",
                panel.availability.label(),
                panel.availability.tone(),
                focused,
                expanded,
            )),
            area,
        );
        return;
    }

    let bar_width = usize::from(area.width.saturating_sub(22).max(8));
    let mut lines = panel
        .rails
        .iter()
        .map(|rail| {
            format!(
                "{} {:<13} {} {}",
                if rail.selected { ">" } else { " " },
                rail.label,
                crate::ui::telemetry::segmented_bar(rail.fill_percent, bar_width.min(16)),
                rail.delta_label
            )
        })
        .collect::<Vec<_>>();
    lines.push(format!(
        "wave {}",
        spark_strip(
            &panel.waveform,
            usize::from(area.width.saturating_sub(8).max(8))
        )
    ));
    let detail = panel
        .rails
        .iter()
        .find(|rail| rail.selected)
        .map_or_else(|| panel.note.clone(), |rail| rail.note.clone());
    lines.push(detail);
    frame.render_widget(
        Paragraph::new(lines.join("\n")).block(panel_block(
            theme,
            "Readiness Breakdown",
            panel.availability.label(),
            panel.availability.tone(),
            focused,
            expanded,
        )),
        area,
    );
}

fn render_heatmap_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    panel: &DashboardWeeklyHeatmap,
    theme: &Theme,
    focused: bool,
    expanded: bool,
) {
    if area.height <= 3 {
        let row = panel.rows.first().map_or_else(
            || panel.note.clone(),
            |values| compact_heatmap_row(values, panel.selected_cell),
        );
        frame.render_widget(
            Paragraph::new(row).block(panel_block(
                theme,
                "Weekly Trends",
                panel.availability.label(),
                panel.availability.tone(),
                focused,
                expanded,
            )),
            area,
        );
        return;
    }

    let row_refs = panel
        .row_labels
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut lines = weekly_heatmap_rows(
        &panel.day_labels,
        &row_refs,
        &panel.rows,
        panel.selected_cell,
    );
    lines.push(panel.note.clone());
    frame.render_widget(
        Paragraph::new(lines.join("\n")).block(panel_block(
            theme,
            "Weekly Trends",
            panel.availability.label(),
            panel.availability.tone(),
            focused,
            expanded,
        )),
        area,
    );
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
