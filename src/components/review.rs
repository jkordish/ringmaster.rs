use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    text::Line,
    widgets::{List, ListItem, Paragraph, Tabs, Wrap},
};

use crate::app::{AiArtifactSummaryView, ReviewModel};
use crate::ui::{
    chrome::{self, PanelKind},
    layout::UiContext,
    theme::{Theme, Tone},
};

pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &ReviewModel, ui: &UiContext, theme: &Theme) {
    if ui.viewport.is_compact() {
        draw_compact(frame, area, model, theme);
    } else {
        draw_wide(frame, area, model, theme);
    }
}

fn draw_wide(frame: &mut Frame<'_>, area: Rect, model: &ReviewModel, theme: &Theme) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(14),
            Constraint::Length(7),
        ])
        .split(area);

    draw_intro(frame, layout[0], model, theme, false);
    draw_mode_tabs(frame, layout[1], model, theme);
    draw_focus_tabs(frame, layout[2], model, theme);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(layout[3]);
    let detail = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(11), Constraint::Min(8)])
        .split(body[1]);

    draw_cards(frame, body[0], model, theme);
    draw_ai_artifact(
        frame,
        detail[0],
        &model.ai_artifact,
        &model.ai_actions,
        theme,
    );
    draw_details(frame, detail[1], model, theme);
    draw_warnings(frame, layout[4], model, theme);
}

fn draw_compact(frame: &mut Frame<'_>, area: Rect, model: &ReviewModel, theme: &Theme) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(4),
        ])
        .split(area);

    draw_intro(frame, layout[0], model, theme, true);

    let controls = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[1]);
    draw_mode_tabs(frame, controls[0], model, theme);
    draw_focus_tabs(frame, controls[1], model, theme);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(layout[2]);
    let detail = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(5)])
        .split(body[1]);
    draw_cards(frame, body[0], model, theme);
    draw_details(frame, detail[0], model, theme);
    draw_ai_artifact(
        frame,
        detail[1],
        &model.ai_artifact,
        &model.ai_actions,
        theme,
    );
    draw_warnings(frame, layout[3], model, theme);
}

fn draw_intro(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &ReviewModel,
    theme: &Theme,
    compact: bool,
) {
    let intro = if compact {
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
        area,
    );
}

fn draw_mode_tabs(frame: &mut Frame<'_>, area: Rect, model: &ReviewModel, theme: &Theme) {
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
        area,
    );
}

fn draw_focus_tabs(frame: &mut Frame<'_>, area: Rect, model: &ReviewModel, theme: &Theme) {
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
        area,
    );
}

fn draw_cards(frame: &mut Frame<'_>, area: Rect, model: &ReviewModel, theme: &Theme) {
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
        area,
    );
}

fn draw_details(frame: &mut Frame<'_>, area: Rect, model: &ReviewModel, theme: &Theme) {
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
        area,
    );
}

fn draw_ai_artifact(
    frame: &mut Frame<'_>,
    area: Rect,
    artifact: &AiArtifactSummaryView,
    ai_actions: &[String],
    theme: &Theme,
) {
    let badge_tone = if artifact.status_label == "available" {
        Tone::Positive
    } else {
        Tone::Muted
    };
    let mut lines = vec![format!("AI artifact: {}", artifact.status_label)];
    lines.extend(artifact.metadata_lines.iter().cloned());

    if !artifact.summary_text.is_empty() {
        lines.push(String::new());
        lines.push("Saved summary:".to_owned());
        lines.push(artifact.summary_text.clone());
    }

    if !artifact.lineage_lines.is_empty() {
        lines.push(String::new());
        lines.extend(artifact.lineage_lines.iter().cloned());
    }

    if !ai_actions.is_empty() {
        lines.push(String::new());
        lines.extend(ai_actions.iter().cloned());
    }

    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .wrap(Wrap { trim: true })
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(theme, "AI artifact", &artifact.status_label, badge_tone),
                PanelKind::Diagnostic,
            )),
        area,
    );
}

fn draw_warnings(frame: &mut Frame<'_>, area: Rect, model: &ReviewModel, theme: &Theme) {
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
        area,
    );
}
