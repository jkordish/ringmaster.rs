use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    text::Line,
    widgets::{List, ListItem, Paragraph, Tabs},
};

use crate::app::ReviewModel;
use crate::ui::{
    chrome::{self, PanelKind},
    layout::UiContext,
    theme::{Theme, Tone},
};

pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &ReviewModel, ui: &UiContext, theme: &Theme) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if ui.viewport.is_compact() { 4 } else { 5 }),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Min(if ui.viewport.is_compact() { 7 } else { 14 }),
            Constraint::Length(if ui.viewport.is_compact() { 4 } else { 7 }),
        ])
        .split(area);

    let intro = if ui.viewport.is_compact() {
        format!("{}\n{}", model.selected_day_label, model.breadcrumb)
    } else {
        format!(
            "Selected day {}\n{}\nRanked observations first, evidence second.",
            model.selected_day_label, model.breadcrumb
        )
    };
    frame.render_widget(
        Paragraph::new(intro)
            .style(theme.hero())
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    "Review digest",
                    "editorial briefing",
                    Tone::Accent,
                ),
                PanelKind::Hero,
            )),
        layout[0],
    );

    frame.render_widget(
        Tabs::new(model.mode_tabs.iter().map(|tab| tab.label.as_str()))
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    "Mode",
                    model.mode_tabs[model.selected_mode_index].label.as_str(),
                    Tone::Focus,
                ),
                PanelKind::Section,
            ))
            .style(theme.annotation())
            .highlight_style(theme.emphasis(Tone::Focus))
            .divider(" ")
            .select(model.selected_mode_index),
        layout[1],
    );

    frame.render_widget(
        Tabs::new(model.focus_tabs.iter().map(|tab| tab.label.as_str()))
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    "Focus",
                    model.focus_tabs[model.selected_focus_index].label.as_str(),
                    Tone::Info,
                ),
                PanelKind::Subtle,
            ))
            .style(theme.annotation())
            .highlight_style(theme.emphasis(Tone::Focus))
            .divider(" ")
            .select(model.selected_focus_index),
        layout[2],
    );

    let body = Layout::default()
        .direction(if ui.viewport.is_compact() {
            Direction::Vertical
        } else {
            Direction::Horizontal
        })
        .constraints(if ui.viewport.is_compact() {
            vec![Constraint::Length(3), Constraint::Min(4)]
        } else {
            vec![Constraint::Percentage(42), Constraint::Percentage(58)]
        })
        .split(layout[3]);

    let cards = if model.cards.is_empty() {
        vec![ListItem::new(chrome::badge_label(
            "EMPTY",
            &model.empty_message,
        ))]
    } else {
        model
            .cards
            .iter()
            .enumerate()
            .map(|(index, card)| {
                let prefix = chrome::focus_prefix(card.selected);
                let rank = index + 1;
                ListItem::new(format!(
                    "{prefix} #{rank} {} | {} | {}",
                    card.headline, card.section_label, card.confidence_label
                ))
            })
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(cards).block(chrome::panel(
            theme,
            chrome::title_with_badge(theme, "Ranked observations", "scan top-down", Tone::Focus),
            PanelKind::Section,
        )),
        body[0],
    );

    let details = if model.detail_lines.is_empty() {
        vec![ListItem::new(
            "[pending] Review evidence appears after enough local history accumulates.",
        )]
    } else {
        model
            .detail_lines
            .iter()
            .map(|line| {
                let formatted = if line.is_empty() {
                    " ".to_owned()
                } else if line.ends_with(':') {
                    format!("[section] {line}")
                } else {
                    line.clone()
                };
                ListItem::new(formatted)
            })
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(details).block(chrome::panel(
            theme,
            Line::from("Briefing detail"),
            PanelKind::Hero,
        )),
        body[1],
    );

    let warnings = if model.warning_lines.is_empty() {
        vec![ListItem::new("[quiet] No active briefing warnings.")]
    } else {
        model
            .warning_lines
            .iter()
            .map(|line| ListItem::new(format!("[watch] {line}")))
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(warnings).block(chrome::panel(
            theme,
            chrome::title_with_badge(
                theme,
                "Warnings and caveats",
                "read before acting",
                Tone::Warning,
            ),
            PanelKind::Section,
        )),
        layout[4],
    );
}
