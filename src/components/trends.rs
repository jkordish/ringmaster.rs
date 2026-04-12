use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    widgets::{List, ListItem, Paragraph, Tabs},
};

use crate::app::{TrendMatrixCell, TrendMatrixRow, TrendsModel};
use crate::navigation::FocusRegion;
use crate::ui::{
    layout::{UiContext, ViewportClass},
    telemetry::{panel_block, segmented_bar, spark_strip},
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
    match ui.viewport {
        ViewportClass::Compact => {
            draw_compact(frame, area, model, theme, focused_region, expanded_region);
        }
        ViewportClass::Medium | ViewportClass::Wide => {
            draw_wide(frame, area, model, theme, focused_region, expanded_region);
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
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
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
        focused_region == FocusRegion::TrendsMatrix,
        expanded_region == Some(FocusRegion::TrendsMatrix),
    );

    let body = model
        .rows
        .iter()
        .map(render_matrix_row)
        .collect::<Vec<_>>()
        .join("\n");
    frame.render_widget(
        Paragraph::new(format!(
            "metric          current   concern     7d               30d              90d              spark\n{body}"
        ))
        .block(panel_block(
            theme,
            "Trend Matrix",
            "SORTED",
            Tone::Info,
            focused_region == FocusRegion::TrendsMatrix,
            expanded_region == Some(FocusRegion::TrendsMatrix),
        )),
        layout[1],
    );

    draw_notes(
        frame,
        layout[2],
        model,
        theme,
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
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
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
    frame.render_widget(
        List::new(rows).block(panel_block(
            theme,
            "Trend Matrix",
            "COMPACT",
            Tone::Info,
            focused_region == FocusRegion::TrendsMatrix,
            expanded_region == Some(FocusRegion::TrendsMatrix),
        )),
        layout[1],
    );
    draw_notes(
        frame,
        layout[2],
        model,
        theme,
        focused_region == FocusRegion::TrendsInspector,
        expanded_region == Some(FocusRegion::TrendsInspector),
    );
}

fn draw_sort_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &TrendsModel,
    theme: &Theme,
    focused: bool,
    expanded: bool,
) {
    frame.render_widget(
        Tabs::new(model.sort_tabs.iter().map(|tab| tab.label))
            .block(panel_block(
                theme,
                "Trend Sort",
                model
                    .sort_tabs
                    .get(model.selected_sort_index)
                    .map_or("Concern", |tab| tab.label),
                Tone::Accent,
                focused,
                expanded,
            ))
            .style(theme.annotation())
            .highlight_style(theme.emphasis(Tone::Focus))
            .divider(" ")
            .select(model.selected_sort_index),
        area,
    );
}

fn draw_notes(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &TrendsModel,
    theme: &Theme,
    focused: bool,
    expanded: bool,
) {
    let notes = model
        .notes
        .iter()
        .map(|note| ListItem::new(note.clone()))
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(notes).block(panel_block(
            theme,
            "Inspector",
            "DETAIL",
            Tone::Muted,
            focused,
            expanded,
        )),
        area,
    );
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
