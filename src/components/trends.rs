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
        ViewportClass::Medium | ViewportClass::Wide => {
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

    let body = model
        .rows
        .iter()
        .map(render_matrix_row)
        .collect::<Vec<_>>()
        .join("\n");
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
        Paragraph::new(format!(
            "metric          current   concern     7d               30d              90d              spark\n{body}"
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

fn render_matrix_row(row: &TrendMatrixRow) -> String {
    let cells = row.cells.iter().map(render_matrix_cell).collect::<Vec<_>>();
    format!(
        "{} {:<14} {:>7} {:<11} {:<16} {:<16} {:<16} {}",
        if row.selected { ">" } else { " " },
        row.label,
        row.current_value,
        row.concern_label,
        cells.first().cloned().unwrap_or_default(),
        cells.get(1).cloned().unwrap_or_default(),
        cells.get(2).cloned().unwrap_or_default(),
        spark_strip(&row.sparkline, 10),
    )
}

fn render_matrix_cell(cell: &TrendMatrixCell) -> String {
    format!(
        "{} {} {}",
        cell.label,
        cell.delta_label,
        segmented_bar(cell.fill_percent, 5)
    )
}
