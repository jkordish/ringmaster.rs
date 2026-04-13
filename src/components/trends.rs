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
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(5),
        ])
        .split(area);

    draw_sort_tabs(
        frame,
        layout[0],
        model,
        theme,
        metrics,
        focused_region == FocusRegion::TrendsMatrix,
        expanded_region == Some(FocusRegion::TrendsMatrix),
    );

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
            focused: focused_region == FocusRegion::TrendsMatrix,
            expanded: expanded_region == Some(FocusRegion::TrendsMatrix),
            kind: PanelKind::Section,
        },
    );
    frame.render_widget(
        Paragraph::new(render_medium_matrix(&body)),
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
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(14),
            Constraint::Length(5),
        ])
        .split(area);

    draw_sort_tabs(
        frame,
        layout[0],
        model,
        theme,
        metrics,
        focused_region == FocusRegion::TrendsMatrix,
        expanded_region == Some(FocusRegion::TrendsMatrix),
    );

    let shell = render_panel_shell(
        frame,
        layout[1],
        theme,
        metrics,
        PanelShellSpec {
            title: "Trend Matrix",
            status: "SORTED",
            status_tone: Tone::Info,
            focused: focused_region == FocusRegion::TrendsMatrix,
            expanded: expanded_region == Some(FocusRegion::TrendsMatrix),
            kind: PanelKind::Section,
        },
    );
    frame.render_widget(
        Paragraph::new(render_wide_matrix(model)),
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
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(5),
        ])
        .split(area);

    draw_sort_tabs(
        frame,
        layout[0],
        model,
        theme,
        metrics,
        focused_region == FocusRegion::TrendsMatrix,
        expanded_region == Some(FocusRegion::TrendsMatrix),
    );
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
            focused: focused_region == FocusRegion::TrendsMatrix,
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
    let notes = model
        .notes
        .iter()
        .map(|note| ListItem::new(note.clone()))
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

fn render_wide_matrix(model: &TrendsModel) -> String {
    let header = format!(
        "{} {} {} {} {}",
        pad(" ", 2, false),
        pad("metric", 16, false),
        pad("current", 8, true),
        pad("concern", 12, false),
        pad("spark", 14, false),
    );
    let subheader = format!(
        "{} {} {} {}",
        pad(" ", 2, false),
        pad("7d", 18, false),
        pad("30d", 18, false),
        pad("90d", 18, false),
    );

    let mut lines = vec![header, subheader];
    for row in &model.rows {
        let (primary, detail) = render_wide_matrix_row(row);
        lines.push(primary);
        lines.push(detail);
    }
    lines.join("\n")
}

fn render_medium_matrix(rows: &[&TrendMatrixRow]) -> String {
    let header = format!(
        "{} {} {} {} {}",
        pad(" ", 2, false),
        pad("metric", 14, false),
        pad("current", 8, true),
        pad("concern", 11, false),
        pad("spark", 10, false),
    );
    let subheader = format!(
        "{} {} {} {}",
        pad(" ", 2, false),
        pad("7d", 16, false),
        pad("30d", 16, false),
        pad("90d", 16, false),
    );

    let mut lines = vec![header, subheader];
    for row in rows {
        let (primary, detail) = render_medium_matrix_row(row);
        lines.push(primary);
        lines.push(detail);
    }
    lines.join("\n")
}

fn render_medium_matrix_row(row: &TrendMatrixRow) -> (String, String) {
    let marker = if row.selected { ">" } else { " " };
    let spark = spark_strip(&row.sparkline, 10);
    let primary = format!(
        "{} {} {} {} {}",
        pad(marker, 2, false),
        pad(row.label, 14, false),
        pad(&row.current_value, 8, true),
        pad(&row.concern_label, 11, false),
        spark,
    );
    let cells = row
        .cells
        .iter()
        .take(3)
        .map(render_medium_matrix_cell)
        .collect::<Vec<_>>();
    let detail = format!(
        "{} {} {} {}",
        pad(" ", 2, false),
        pad(cells.first().map_or("", String::as_str), 16, false),
        pad(cells.get(1).map_or("", String::as_str), 16, false),
        pad(cells.get(2).map_or("", String::as_str), 16, false),
    );
    (primary, detail)
}

fn render_medium_matrix_cell(cell: &TrendMatrixCell) -> String {
    format!(
        "{} {}",
        pad(&format!("{} {}", cell.label, cell.delta_label), 9, false),
        segmented_bar(cell.fill_percent, 4)
    )
}

fn render_wide_matrix_row(row: &TrendMatrixRow) -> (String, String) {
    let marker = if row.selected { ">" } else { " " };
    let spark = spark_strip(&row.sparkline, 14);
    let primary = format!(
        "{} {} {} {} {}",
        pad(marker, 2, false),
        pad(row.label, 16, false),
        pad(&row.current_value, 8, true),
        pad(&row.concern_label, 12, false),
        spark,
    );
    let cells = row
        .cells
        .iter()
        .take(3)
        .map(render_wide_matrix_cell)
        .collect::<Vec<_>>();
    let detail = format!(
        "{} {} {} {}",
        pad(" ", 2, false),
        pad(cells.first().map_or("", String::as_str), 18, false),
        pad(cells.get(1).map_or("", String::as_str), 18, false),
        pad(cells.get(2).map_or("", String::as_str), 18, false),
    );
    (primary, detail)
}

fn render_wide_matrix_cell(cell: &TrendMatrixCell) -> String {
    format!(
        "{} {}",
        pad(&cell.delta_label, 5, false),
        segmented_bar(cell.fill_percent, 6)
    )
}

fn pad(value: &str, width: usize, right_align: bool) -> String {
    if right_align {
        format!("{value:>width$.width$}")
    } else {
        format!("{value:<width$.width$}")
    }
}
