use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    text::Line,
    widgets::{Cell, List, ListItem, Paragraph, Row, Table},
};

use crate::app::{OpsItem, OpsModel};
use crate::navigation::FocusRegion;
use crate::ui::{
    chrome::{self, PanelKind},
    layout::UiContext,
    telemetry::{coverage_rows, panel_block},
    theme::{Theme, Tone},
};

pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    ui: &UiContext,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
) {
    if ui.viewport.is_compact() {
        draw_compact(frame, area, model, theme, focused_region, expanded_region);
    } else {
        draw_wide(frame, area, model, theme, focused_region, expanded_region);
    }
}

fn draw_wide(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(16),
            Constraint::Length(7),
        ])
        .split(area);

    draw_summary(
        frame,
        layout[0],
        model,
        theme,
        false,
        focused_region == FocusRegion::OpsSummary,
        expanded_region == Some(FocusRegion::OpsSummary),
    );

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(26),
            Constraint::Percentage(32),
            Constraint::Percentage(42),
        ])
        .split(layout[1]);

    draw_coverage_panel(
        frame,
        body[0],
        model,
        theme,
        focused_region == FocusRegion::OpsCoverage,
        expanded_region == Some(FocusRegion::OpsCoverage),
    );
    draw_family_table(frame, body[1], model, theme);
    let diagnostics = prioritized_diagnostic_items(model);
    draw_diagnostics_list(
        frame,
        body[2],
        &diagnostics,
        theme,
        None,
        focused_region == FocusRegion::OpsDiagnostics,
        expanded_region == Some(FocusRegion::OpsDiagnostics),
    );
    draw_warnings(
        frame,
        layout[2],
        model,
        theme,
        focused_region == FocusRegion::OpsWarnings,
        expanded_region == Some(FocusRegion::OpsWarnings),
    );
}

fn draw_compact(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(5),
            Constraint::Min(12),
            Constraint::Length(3),
        ])
        .split(area);

    draw_summary(
        frame,
        layout[0],
        model,
        theme,
        true,
        focused_region == FocusRegion::OpsSummary,
        expanded_region == Some(FocusRegion::OpsSummary),
    );
    draw_coverage_panel(
        frame,
        layout[1],
        model,
        theme,
        focused_region == FocusRegion::OpsCoverage,
        expanded_region == Some(FocusRegion::OpsCoverage),
    );

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
        .split(layout[2]);

    let family_items = model
        .family_statuses
        .iter()
        .map(|status| ListItem::new(format!("[{}] {}", status.state_label, status.label)))
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(family_items).block(chrome::panel(
            theme,
            Line::from("Family status"),
            PanelKind::Diagnostic,
        )),
        body[0],
    );

    let diagnostics = prioritized_diagnostic_items(model);
    draw_diagnostics_list(
        frame,
        body[1],
        &diagnostics,
        theme,
        None,
        focused_region == FocusRegion::OpsDiagnostics,
        expanded_region == Some(FocusRegion::OpsDiagnostics),
    );
    draw_warnings(
        frame,
        layout[3],
        model,
        theme,
        focused_region == FocusRegion::OpsWarnings,
        expanded_region == Some(FocusRegion::OpsWarnings),
    );
}

fn draw_summary(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    theme: &Theme,
    compact: bool,
    focused: bool,
    expanded: bool,
) {
    let summary = if compact {
        model
            .summary_lines
            .iter()
            .take(2)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    } else if model.summary_lines.is_empty() {
        format!("Mode: {}", model.mode_label)
    } else {
        model.summary_lines.join("\n")
    };
    frame.render_widget(
        Paragraph::new(summary)
            .style(theme.hero())
            .block(panel_block(
                theme,
                "Status",
                &model.mode_label,
                Tone::Info,
                focused,
                expanded,
            )),
        area,
    );
}

fn draw_coverage_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    theme: &Theme,
    focused: bool,
    expanded: bool,
) {
    let coverage = model
        .coverage
        .iter()
        .map(|cell| (cell.label, cell.availability))
        .collect::<Vec<_>>();
    let body = coverage_rows(&coverage)
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n");
    frame.render_widget(
        Paragraph::new(body).block(panel_block(
            theme,
            "Coverage",
            "MATRIX",
            Tone::Info,
            focused,
            expanded,
        )),
        area,
    );
}

fn draw_family_table(frame: &mut Frame<'_>, area: Rect, model: &OpsModel, theme: &Theme) {
    let family_rows = model.family_statuses.iter().map(|status| {
        Row::new(vec![
            Cell::from(status.label),
            Cell::from(status.state_label.clone()),
            Cell::from(status.scope_label.clone()),
            Cell::from(status.last_sync.clone()),
        ])
    });
    frame.render_widget(
        Table::new(
            family_rows,
            [
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(16),
                Constraint::Min(12),
            ],
        )
        .header(
            Row::new(vec!["Family", "State", "Scope", "Last sync"])
                .style(theme.section_title(Tone::Info)),
        )
        .block(panel_block(
            theme,
            "Family status",
            "SYNC",
            Tone::Info,
            false,
            false,
        )),
        area,
    );
}

fn draw_diagnostics_list(
    frame: &mut Frame<'_>,
    area: Rect,
    items: &[OpsItem],
    theme: &Theme,
    max_items: Option<usize>,
    focused: bool,
    expanded: bool,
) {
    let diagnostics = items
        .iter()
        .take(max_items.unwrap_or(items.len()))
        .map(|item| ListItem::new(format!("{}: {}", item.label, item.value)))
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(diagnostics).block(panel_block(
            theme,
            "Diagnostics",
            "DETAIL",
            Tone::Muted,
            focused,
            expanded,
        )),
        area,
    );
}

fn prioritized_diagnostic_items(model: &OpsModel) -> Vec<OpsItem> {
    const PRIORITY_LABELS: [&str; 11] = [
        "Auth state",
        "Granted scopes",
        "Invalidation queue",
        "Receiver heartbeat",
        "Latest eval",
        "Eval health",
        "Access token expiry",
        "Watch heartbeat",
        "Subscriptions",
        "Last accepted delivery",
        "Last rejected delivery",
    ];

    let mut diagnostics = PRIORITY_LABELS
        .into_iter()
        .filter_map(|label| model.items.iter().find(|item| item.label == label).cloned())
        .collect::<Vec<_>>();

    for item in &model.items {
        if diagnostics
            .iter()
            .all(|selected| selected.label != item.label)
        {
            diagnostics.push(item.clone());
        }
    }

    diagnostics
}

fn draw_warnings(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    theme: &Theme,
    focused: bool,
    expanded: bool,
) {
    let warnings = if model.warnings.is_empty() {
        vec![ListItem::new("[quiet] No warnings.")]
    } else {
        model
            .warnings
            .iter()
            .map(|warning| ListItem::new(format!("[warn] {warning}")))
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(warnings).block(panel_block(
            theme,
            "Warnings",
            if model.warnings.is_empty() {
                "CLEAR"
            } else {
                "ATTN"
            },
            Tone::Warning,
            focused,
            expanded,
        )),
        area,
    );
}
