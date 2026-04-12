use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    text::Line,
    widgets::{List, ListItem, Paragraph, Tabs},
};

use crate::app::{OverlayToggleView, PatternsModel};
use crate::ui::{
    chrome::{self, PanelKind},
    layout::UiContext,
    theme::{Theme, Tone},
};

pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &PatternsModel,
    ui: &UiContext,
    theme: &Theme,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if ui.viewport.is_compact() { 3 } else { 4 }),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(if ui.viewport.is_compact() { 10 } else { 14 }),
            Constraint::Length(if ui.viewport.is_compact() { 4 } else { 6 }),
        ])
        .split(area);

    let headline = if ui.viewport.is_compact() {
        format!("{} | {}", model.header, model.filter_summary)
    } else {
        format!("{}\n{}", model.header, model.filter_summary)
    };
    frame.render_widget(
        Paragraph::new(headline)
            .style(theme.hero())
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    "Patterns browser",
                    "cross-day associations",
                    Tone::Accent,
                ),
                PanelKind::Hero,
            )),
        layout[0],
    );

    draw_metric_tabs(frame, layout[1], model, theme);
    draw_overlay_tabs(frame, layout[2], &model.overlay_toggles, theme);

    let body = Layout::default()
        .direction(if ui.viewport.is_compact() {
            Direction::Vertical
        } else {
            Direction::Horizontal
        })
        .constraints(if ui.viewport.is_compact() {
            vec![Constraint::Min(6), Constraint::Length(4)]
        } else {
            vec![Constraint::Percentage(64), Constraint::Percentage(36)]
        })
        .split(layout[3]);

    let associations = if model.rows.is_empty() {
        vec![ListItem::new(chrome::badge_label(
            "WAIT",
            &model.empty_message,
        ))]
    } else {
        model
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let prefix = if index == 0 { "[lead]" } else { "[scan]" };
                let badge_line = if row.badges.is_empty() {
                    String::new()
                } else {
                    format!("\n      {}", row.badges.join(" / "))
                };
                ListItem::new(format!(
                    "{prefix} {}\n      {}{}",
                    row.headline, row.detail, badge_line
                ))
            })
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(associations).block(chrome::panel(
            theme,
            chrome::title_with_badge(theme, "Grouped findings", "compare across days", Tone::Info),
            PanelKind::Section,
        )),
        body[0],
    );

    let notes = model
        .notes
        .iter()
        .map(|note| ListItem::new(format!("[note] {note}")))
        .chain(std::iter::once(ListItem::new(
            "[guide] Use the filter strip to narrow families, then switch views to validate a finding.",
        )))
        .chain(model.ai_actions.iter().cloned().map(ListItem::new))
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(notes).block(chrome::panel(
            theme,
            Line::from("Reading guide"),
            PanelKind::Subtle,
        )),
        body[1],
    );

    frame.render_widget(
        Paragraph::new(
            "Patterns stay descriptive on purpose. Use this screen to scan recurring pairings, then \
             pivot into Explain or Timeline when a row suggests a story worth validating.",
        )
        .style(theme.annotation())
        .block(chrome::panel(
            theme,
            Line::from("Interpretation"),
            PanelKind::Subtle,
        )),
        layout[4],
    );
}

fn draw_metric_tabs(frame: &mut Frame<'_>, area: Rect, model: &PatternsModel, theme: &Theme) {
    frame.render_widget(
        Tabs::new(model.metric_filters.iter().map(|tab| tab.label))
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    "Metric filter",
                    model
                        .metric_filters
                        .get(model.selected_filter_index)
                        .map_or("All", |tab| tab.label),
                    Tone::Focus,
                ),
                PanelKind::Section,
            ))
            .style(theme.annotation())
            .highlight_style(theme.emphasis(Tone::Focus))
            .divider(" ")
            .select(model.selected_filter_index),
        area,
    );
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
                    "Family filter",
                    toggles
                        .get(selected_index)
                        .map_or("Workouts", |toggle| toggle.label),
                    Tone::Info,
                ),
                PanelKind::Subtle,
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
