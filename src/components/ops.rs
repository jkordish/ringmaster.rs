use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    text::Line,
    widgets::{Cell, List, ListItem, Paragraph, Row, Table},
};

use crate::app::{OpsItem, OpsModel};
use crate::ui::{
    chrome::{self, PanelKind},
    layout::UiContext,
    theme::{Theme, Tone},
};

pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &OpsModel, ui: &UiContext, theme: &Theme) {
    if ui.viewport.is_compact() {
        draw_compact(frame, area, model, theme);
    } else {
        draw_wide(frame, area, model, theme);
    }
}

fn draw_wide(frame: &mut Frame<'_>, area: Rect, model: &OpsModel, theme: &Theme) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(16),
            Constraint::Length(7),
        ])
        .split(area);

    draw_summary(frame, layout[0], model, theme, false);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(layout[1]);

    draw_family_table(frame, body[0], model, theme);
    draw_diagnostics_list(frame, body[1], &model.items, theme, None);
    draw_warnings(frame, layout[2], model, theme);
}

fn draw_compact(frame: &mut Frame<'_>, area: Rect, model: &OpsModel, theme: &Theme) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(12),
            Constraint::Length(3),
        ])
        .split(area);

    draw_summary(frame, layout[0], model, theme, true);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
        .split(layout[1]);

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

    let diagnostics = compact_diagnostic_items(model);
    draw_diagnostics_list(frame, body[1], &diagnostics, theme, None);
    draw_warnings(frame, layout[2], model, theme);
}

fn draw_summary(frame: &mut Frame<'_>, area: Rect, model: &OpsModel, theme: &Theme, compact: bool) {
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
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(theme, "Status console", &model.mode_label, Tone::Info),
                PanelKind::Diagnostic,
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
        .block(chrome::panel(
            theme,
            Line::from("Family status"),
            PanelKind::Diagnostic,
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
) {
    let diagnostics = items
        .iter()
        .take(max_items.unwrap_or(items.len()))
        .map(|item| ListItem::new(format!("{}: {}", item.label, item.value)))
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(diagnostics).block(chrome::panel(
            theme,
            chrome::title_with_badge(theme, "Diagnostics", "dense operator read", Tone::Muted),
            PanelKind::Subtle,
        )),
        area,
    );
}

fn compact_diagnostic_items(model: &OpsModel) -> Vec<OpsItem> {
    const PRIORITY_LABELS: [&str; 10] = [
        "Auth state",
        "Granted scopes",
        "Access token expiry",
        "Receiver heartbeat",
        "Watch heartbeat",
        "Subscriptions",
        "Last accepted delivery",
        "Last rejected delivery",
        "Invalidation queue",
        "Last periodic sync",
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

fn draw_warnings(frame: &mut Frame<'_>, area: Rect, model: &OpsModel, theme: &Theme) {
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
        List::new(warnings).block(chrome::panel(
            theme,
            chrome::title_with_badge(theme, "Warnings", "operator attention", Tone::Warning),
            PanelKind::Section,
        )),
        area,
    );
}
