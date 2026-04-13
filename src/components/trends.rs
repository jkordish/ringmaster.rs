use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    widgets::{List, ListItem, Paragraph, Tabs},
};

use crate::app::{TrendMatrixCell, TrendMatrixRow, TrendsModel};
use crate::navigation::FocusRegion;
use crate::ui::{
    chrome::{PanelKind, PanelShellSpec, render_panel_shell},
    layout::{UiContext, ViewportClass},
    telemetry::{segmented_bar, spark_strip},
    theme::{Theme, Tone},
};

pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &TrendsModel,
    ui: &UiContext,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
) {
    let metrics = crate::ui::layout::DashboardMetrics::for_viewport(ui.viewport);
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

fn draw_medium(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &TrendsModel,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
    metrics: crate::ui::layout::DashboardMetrics,
) {
    let sort_focused =
        focused_region == FocusRegion::TrendsMatrix && model.focused_subfocus.is_sort_tabs();
    let matrix_focused =
        focused_region == FocusRegion::TrendsMatrix && model.focused_subfocus.is_rows();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(5),
        ])
        .split(area);

    draw_sort_tabs(frame, layout[0], model, theme, metrics, sort_focused, false);

    let body = model.rows.iter().collect::<Vec<_>>();
    let shell = render_panel_shell(
        frame,
        layout[1],
        theme,
        metrics,
        PanelShellSpec {
            title: "Trend Matrix",
            status: "SORTED",
            status_tone: Tone::Info,
            focused: matrix_focused,
            expanded: expanded_region == Some(FocusRegion::TrendsMatrix),
            kind: PanelKind::Section,
        },
    );
    frame.render_widget(
        Paragraph::new(render_matrix(&body, shell.content_area.width as usize)),
        shell.content_area,
    );

    draw_notes(
        frame,
        layout[2],
        model,
        theme,
        metrics,
        focused_region == FocusRegion::TrendsInspector,
        expanded_region == Some(FocusRegion::TrendsInspector),
    );
}

fn draw_wide(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &TrendsModel,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
    metrics: crate::ui::layout::DashboardMetrics,
) {
    let sort_focused =
        focused_region == FocusRegion::TrendsMatrix && model.focused_subfocus.is_sort_tabs();
    let matrix_focused =
        focused_region == FocusRegion::TrendsMatrix && model.focused_subfocus.is_rows();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(14),
            Constraint::Length(5),
        ])
        .split(area);

    draw_sort_tabs(frame, layout[0], model, theme, metrics, sort_focused, false);

    let shell = render_panel_shell(
        frame,
        layout[1],
        theme,
        metrics,
        PanelShellSpec {
            title: "Trend Matrix",
            status: "SORTED",
            status_tone: Tone::Info,
            focused: matrix_focused,
            expanded: expanded_region == Some(FocusRegion::TrendsMatrix),
            kind: PanelKind::Section,
        },
    );
    frame.render_widget(
        Paragraph::new(render_matrix(
            model.rows.iter().collect::<Vec<_>>().as_slice(),
            shell.content_area.width as usize,
        )),
        shell.content_area,
    );

    draw_notes(
        frame,
        layout[2],
        model,
        theme,
        metrics,
        focused_region == FocusRegion::TrendsInspector,
        expanded_region == Some(FocusRegion::TrendsInspector),
    );
}

fn draw_compact(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &TrendsModel,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
    metrics: crate::ui::layout::DashboardMetrics,
) {
    let sort_focused =
        focused_region == FocusRegion::TrendsMatrix && model.focused_subfocus.is_sort_tabs();
    let matrix_focused =
        focused_region == FocusRegion::TrendsMatrix && model.focused_subfocus.is_rows();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(5),
        ])
        .split(area);

    draw_sort_tabs(frame, layout[0], model, theme, metrics, sort_focused, false);
    let rows = model
        .rows
        .iter()
        .map(|row| {
            let cells = row
                .cells
                .iter()
                .map(|cell| format!("{} {}", cell.label, cell.delta_label))
                .collect::<Vec<_>>()
                .join(" | ");
            ListItem::new(format!(
                "{} {} | {} | {}",
                if row.selected { ">" } else { " " },
                row.label,
                row.current_value,
                cells
            ))
        })
        .collect::<Vec<_>>();
    let shell = render_panel_shell(
        frame,
        layout[1],
        theme,
        metrics,
        PanelShellSpec {
            title: "Trend Matrix",
            status: "COMPACT",
            status_tone: Tone::Info,
            focused: matrix_focused,
            expanded: expanded_region == Some(FocusRegion::TrendsMatrix),
            kind: PanelKind::Section,
        },
    );
    frame.render_widget(List::new(rows), shell.content_area);
    draw_notes(
        frame,
        layout[2],
        model,
        theme,
        metrics,
        focused_region == FocusRegion::TrendsInspector,
        expanded_region == Some(FocusRegion::TrendsInspector),
    );
}

fn draw_sort_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &TrendsModel,
    theme: &Theme,
    metrics: crate::ui::layout::DashboardMetrics,
    focused: bool,
    expanded: bool,
) {
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        metrics,
        PanelShellSpec {
            title: "Trend Sort",
            status: model
                .sort_tabs
                .get(model.selected_sort_index)
                .map_or("SORT", |tab| tab.label),
            status_tone: Tone::Accent,
            focused,
            expanded,
            kind: PanelKind::Section,
        },
    );
    frame.render_widget(
        Tabs::new(model.sort_tabs.iter().map(|tab| tab.label))
            .style(theme.annotation())
            .highlight_style(theme.emphasis(Tone::Focus))
            .divider(" ")
            .select(model.selected_sort_index),
        shell.content_area,
    );
}

fn draw_notes(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &TrendsModel,
    theme: &Theme,
    metrics: crate::ui::layout::DashboardMetrics,
    focused: bool,
    expanded: bool,
) {
    let notes = inspector_lines(model)
        .into_iter()
        .map(ListItem::new)
        .collect::<Vec<_>>();
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        metrics,
        PanelShellSpec {
            title: "Inspector",
            status: "DETAIL",
            status_tone: Tone::Muted,
            focused,
            expanded,
            kind: PanelKind::Diagnostic,
        },
    );
    frame.render_widget(List::new(notes), shell.content_area);
}

fn render_matrix(rows: &[&TrendMatrixRow], width: usize) -> String {
    let widths = matrix_widths(width);
    let mut lines = vec![format!(
        "{} {} {} {} {} {} {} {}",
        pad(" ", widths.marker, false),
        pad("metric", widths.metric, false),
        pad("current", widths.current, true),
        pad("7d", widths.window, false),
        pad("30d", widths.window, false),
        pad("90d", widths.window, false),
        pad("cue", widths.cue, false),
        pad("signature", widths.spark, false),
    )];

    for row in rows {
        lines.push(render_matrix_row(row, widths));
    }

    lines.join("\n")
}

fn render_matrix_row(row: &TrendMatrixRow, widths: TrendMatrixWidths) -> String {
    let marker = if row.selected { ">" } else { " " };
    let mut cells = row
        .cells
        .iter()
        .take(3)
        .map(|cell| render_matrix_cell(cell, widths.window))
        .collect::<Vec<_>>();
    while cells.len() < 3 {
        cells.push(String::new());
    }

    format!(
        "{} {} {} {} {} {} {} {}",
        pad(marker, widths.marker, false),
        pad(row.label, widths.metric, false),
        pad(&row.current_value, widths.current, true),
        pad(&cells[0], widths.window, false),
        pad(&cells[1], widths.window, false),
        pad(&cells[2], widths.window, false),
        pad(&row.concern_label, widths.cue, false),
        spark_strip(&row.sparkline, widths.spark),
    )
}

fn render_matrix_cell(cell: &TrendMatrixCell, width: usize) -> String {
    let bar_width = width.saturating_sub(6).clamp(3, 6);
    format!(
        "{} {}",
        pad(
            &cell.delta_label,
            width.saturating_sub(bar_width + 1),
            false
        ),
        segmented_bar(cell.fill_percent, bar_width)
    )
}

fn inspector_lines(model: &TrendsModel) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(selected_row) = model.rows.iter().find(|row| row.selected) {
        lines.push(format!(
            "{}: {} | cue {}",
            selected_row.label, selected_row.detail, selected_row.concern_label
        ));
    }
    lines.extend(model.notes.iter().cloned());
    lines
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrendMatrixWidths {
    marker: usize,
    metric: usize,
    current: usize,
    window: usize,
    cue: usize,
    spark: usize,
}

const fn matrix_widths(total_width: usize) -> TrendMatrixWidths {
    let marker = 2;
    let current = 8;
    let window = 10;
    let cue = 12;
    let spacing = 7;
    let spark_min = 8;
    let metric_min = 14;

    let fixed_without_metric = marker + current + (window * 3) + cue + spark_min + spacing;
    let extra = total_width.saturating_sub(fixed_without_metric + metric_min);

    TrendMatrixWidths {
        marker,
        metric: metric_min + extra / 2,
        current,
        window,
        cue,
        spark: spark_min + (extra - (extra / 2)),
    }
}

fn pad(value: &str, width: usize, right_align: bool) -> String {
    if right_align {
        format!("{value:>width$.width$}")
    } else {
        format!("{value:<width$.width$}")
    }
}
