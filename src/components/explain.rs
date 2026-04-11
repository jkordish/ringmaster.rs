use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    text::Line,
    widgets::{List, ListItem, Paragraph},
};

use crate::app::ExplainModel;
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
            Constraint::Length(if ui.viewport.is_compact() { 5 } else { 7 }),
            Constraint::Min(if ui.viewport.is_compact() { 6 } else { 12 }),
            Constraint::Length(if ui.viewport.is_compact() { 4 } else { 7 }),
        ])
        .split(area);

    let headline = if ui.viewport.is_compact() {
        format!("{}\n{}", model.selected_day_label, model.breadcrumb)
    } else {
        format!(
            "{}\n{}\nSelected day: {}",
            model.headline, model.breadcrumb, model.selected_day_label
        )
    };
    frame.render_widget(
        Paragraph::new(headline)
            .style(theme.hero())
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(theme, "Explain", "evidence view", Tone::Accent),
                PanelKind::Hero,
            )),
        layout[0],
    );

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
        .split(layout[1]);

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
        .split(layout[2]);

    frame.render_widget(
        List::new(model.evidence_lines.iter().cloned().map(ListItem::new)).block(chrome::panel(
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
        .split(layout[3]);

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
