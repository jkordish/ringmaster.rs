use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::{Alignment, Rect},
    text::Line,
    widgets::{List, ListItem, Paragraph},
};

use crate::app::DashboardModel;
use crate::ui::{
    chrome::{self, PanelKind},
    layout::{UiContext, equal_columns},
    theme::{Theme, Tone},
};

pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &DashboardModel,
    ui: &UiContext,
    theme: &Theme,
) {
    if ui.viewport.is_compact() {
        draw_compact(frame, area, model, theme);
    } else {
        draw_wide(frame, area, model, theme);
    }
}

fn draw_wide(frame: &mut Frame<'_>, area: Rect, model: &DashboardModel, theme: &Theme) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),
            Constraint::Length(7),
            Constraint::Min(8),
        ])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(layout[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(5)])
        .split(top[1]);

    let lead_body = format!(
        "{}\n{}",
        model.change_summary,
        model
            .highlights
            .first()
            .cloned()
            .unwrap_or_else(|| "No secondary highlight is available yet.".to_owned())
    );
    frame.render_widget(
        Paragraph::new(lead_body)
            .style(theme.hero())
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    &format!("What matters now | {}", model.selected_day_label),
                    "dashboard",
                    Tone::Accent,
                ),
                PanelKind::Hero,
            )),
        top[0],
    );

    frame.render_widget(
        Paragraph::new(model.freshness.clone())
            .style(theme.body())
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    "Freshness and coverage",
                    "live state",
                    chrome::tone_for_text(&model.freshness),
                ),
                PanelKind::Section,
            )),
        right[0],
    );

    let capability_lines = model
        .capabilities
        .iter()
        .map(|capability| {
            let prefix = if capability.available {
                "[ready]"
            } else {
                "[wait]"
            };
            ListItem::new(format!(
                "{prefix} {} | {}",
                capability.label, capability.note
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(capability_lines).block(chrome::panel(
            theme,
            Line::from("Capabilities"),
            PanelKind::Subtle,
        )),
        right[1],
    );

    let metric_columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(equal_columns(model.scores.len()))
        .split(layout[1]);

    for (index, card) in model.scores.iter().enumerate() {
        let tone = chrome::tone_for_text(&card.badge);
        let text = format!("{}\n{}\n{}", card.value, card.subtitle, card.badge);
        frame.render_widget(
            Paragraph::new(text)
                .style(theme.hero())
                .alignment(Alignment::Center)
                .block(chrome::panel(
                    theme,
                    chrome::title_with_badge(theme, card.label, "metric", tone),
                    PanelKind::Section,
                )),
            metric_columns[index],
        );
    }

    let highlights = model
        .highlights
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let prefix = if index == 0 { "[lead]" } else { "[note]" };
            ListItem::new(format!("{prefix} {line}"))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(highlights).block(chrome::panel(
            theme,
            Line::from("Drill-down cues"),
            PanelKind::Section,
        )),
        layout[2],
    );
}

fn draw_compact(frame: &mut Frame<'_>, area: Rect, model: &DashboardModel, theme: &Theme) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(4),
            Constraint::Min(7),
        ])
        .split(area);

    let metric_columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(equal_columns(model.scores.len()))
        .split(layout[1]);

    frame.render_widget(
        Paragraph::new(format!("{}\n{}", model.change_summary, model.freshness))
            .style(theme.hero())
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    &format!("Now | {}", model.selected_day_label),
                    "dashboard",
                    Tone::Accent,
                ),
                PanelKind::Hero,
            )),
        layout[0],
    );

    for (index, card) in model.scores.iter().enumerate() {
        frame.render_widget(
            Paragraph::new(format!("{}\n{}", card.value, card.badge))
                .style(theme.hero())
                .alignment(Alignment::Center)
                .block(chrome::panel(
                    theme,
                    chrome::title_with_badge(
                        theme,
                        card.label,
                        &card.badge,
                        chrome::tone_for_text(&card.badge),
                    ),
                    PanelKind::Subtle,
                )),
            metric_columns[index],
        );
    }

    let detail_items = model
        .capabilities
        .iter()
        .map(|capability| {
            let prefix = if capability.available { "[ok]" } else { "[--]" };
            ListItem::new(format!(
                "{prefix} {} | {}",
                capability.label, capability.note
            ))
        })
        .chain(model.highlights.iter().cloned().map(ListItem::new))
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(detail_items).block(chrome::panel(
            theme,
            Line::from("Secondary detail"),
            PanelKind::Section,
        )),
        layout[2],
    );
}
