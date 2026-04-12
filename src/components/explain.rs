use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    text::Line,
    widgets::{List, ListItem, Paragraph, Tabs},
};

use crate::app::{ExplainModel, OverlayToggleView};
use crate::ui::{
    chrome::{self, PanelKind},
    layout::UiContext,
    theme::{Theme, Tone},
};

pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &ExplainModel,
    ui: &UiContext,
    theme: &Theme,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if ui.viewport.is_compact() { 4 } else { 5 }),
            Constraint::Length(3),
            Constraint::Length(if ui.viewport.is_compact() { 5 } else { 7 }),
            Constraint::Min(if ui.viewport.is_compact() { 6 } else { 12 }),
            Constraint::Length(if ui.viewport.is_compact() { 4 } else { 7 }),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(explain_headline(model, ui))
            .style(theme.hero())
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(theme, "Explain", "evidence view", Tone::Accent),
                PanelKind::Hero,
            )),
        layout[0],
    );

    draw_overlay_tabs(frame, layout[1], &model.overlay_toggles, theme);

    draw_summary_section(frame, layout[2], model, ui, theme);
    draw_evidence_section(frame, layout[3], model, ui, theme);
    draw_footer_section(frame, layout[4], model, ui, theme);
}

fn draw_overlay_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    toggles: &[OverlayToggleView],
    theme: &Theme,
) {
    let selected_index = toggles
        .iter()
        .position(|toggle| toggle.selected)
        .unwrap_or(0);
    frame.render_widget(
        Tabs::new(toggles.iter().map(overlay_tab_label))
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    "Overlay filters",
                    toggles
                        .get(selected_index)
                        .map_or("Workouts", |toggle| toggle.label),
                    Tone::Info,
                ),
                PanelKind::Section,
            ))
            .style(theme.annotation())
            .highlight_style(theme.emphasis(Tone::Focus))
            .divider(" ")
            .select(selected_index),
        area,
    );
}

fn overlay_tab_label(toggle: &OverlayToggleView) -> String {
    format!(
        "{} {}",
        toggle.label,
        if toggle.enabled { "on" } else { "off" }
    )
}

fn explain_headline(model: &ExplainModel, ui: &UiContext) -> String {
    if ui.viewport.is_compact() {
        format!("{}\n{}", model.selected_day_label, model.breadcrumb)
    } else {
        format!(
            "{}\n{}\nSelected day: {}",
            model.headline, model.breadcrumb, model.selected_day_label
        )
    }
}

fn draw_summary_section(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &ExplainModel,
    ui: &UiContext,
    theme: &Theme,
) {
    let summary = Layout::default()
        .direction(if ui.viewport.is_compact() {
            Direction::Vertical
        } else {
            Direction::Horizontal
        })
        .constraints(if ui.viewport.is_compact() {
            vec![Constraint::Length(3), Constraint::Length(2)]
        } else {
            vec![Constraint::Percentage(58), Constraint::Percentage(42)]
        })
        .split(area);

    frame.render_widget(
        List::new(model.summary_lines.iter().cloned().map(ListItem::new)).block(chrome::panel(
            theme,
            Line::from("Claim"),
            PanelKind::Section,
        )),
        summary[0],
    );

    frame.render_widget(
        List::new(model.measurement_lines.iter().cloned().map(ListItem::new)).block(chrome::panel(
            theme,
            Line::from("Measured inputs"),
            PanelKind::Subtle,
        )),
        summary[1],
    );
}

fn draw_evidence_section(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &ExplainModel,
    ui: &UiContext,
    theme: &Theme,
) {
    let evidence_items = model
        .evidence_badges
        .iter()
        .map(|badge| ListItem::new(format!("[evidence] {badge}")))
        .chain(model.evidence_lines.iter().cloned().map(ListItem::new))
        .collect::<Vec<_>>();
    let middle = Layout::default()
        .direction(if ui.viewport.is_compact() {
            Direction::Vertical
        } else {
            Direction::Horizontal
        })
        .constraints(if ui.viewport.is_compact() {
            vec![Constraint::Length(3), Constraint::Min(2)]
        } else {
            vec![Constraint::Percentage(54), Constraint::Percentage(46)]
        })
        .split(area);

    frame.render_widget(
        List::new(evidence_items).block(chrome::panel(
            theme,
            chrome::title_with_badge(theme, "Supporting evidence", "why", Tone::Positive),
            PanelKind::Section,
        )),
        middle[0],
    );

    frame.render_widget(
        List::new(model.context_lines.iter().cloned().map(ListItem::new)).block(chrome::panel(
            theme,
            chrome::title_with_badge(theme, "Context and continuity", "events", Tone::Info),
            PanelKind::Subtle,
        )),
        middle[1],
    );
}

fn draw_footer_section(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &ExplainModel,
    ui: &UiContext,
    theme: &Theme,
) {
    let footer = Layout::default()
        .direction(if ui.viewport.is_compact() {
            Direction::Horizontal
        } else {
            Direction::Vertical
        })
        .constraints(if ui.viewport.is_compact() {
            vec![Constraint::Percentage(60), Constraint::Percentage(40)]
        } else {
            vec![Constraint::Percentage(58), Constraint::Percentage(42)]
        })
        .split(area);

    frame.render_widget(
        List::new(
            model
                .caveat_lines
                .iter()
                .map(|line| ListItem::new(format!("[caveat] {line}"))),
        )
        .block(chrome::panel(
            theme,
            chrome::title_with_badge(theme, "Uncertainty", "read before acting", Tone::Warning),
            PanelKind::Section,
        )),
        footer[0],
    );

    frame.render_widget(
        List::new(model.ai_actions.iter().cloned().map(ListItem::new)).block(chrome::panel(
            theme,
            chrome::title_with_badge(theme, "AI launch", "guided only", Tone::Info),
            PanelKind::Subtle,
        )),
        footer[1],
    );
}
