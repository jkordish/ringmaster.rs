use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    text::Line,
    widgets::{List, ListItem, Paragraph, Sparkline, Tabs},
};

use crate::app::TrendsModel;
use crate::ui::{
    chrome::{self, PanelKind},
    layout::UiContext,
    theme::{Theme, Tone},
};

pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &TrendsModel, ui: &UiContext, theme: &Theme) {
    if ui.viewport.is_compact() {
        draw_compact(frame, area, model, theme);
    } else {
        draw_wide(frame, area, model, theme);
    }
}

fn draw_wide(frame: &mut Frame<'_>, area: Rect, model: &TrendsModel, theme: &Theme) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Min(18),
            Constraint::Length(6),
        ])
        .split(area);

    draw_window_tabs(frame, layout[0], model, theme);
    frame.render_widget(
        Paragraph::new(model.windows[model.selected_window_index].summary.clone())
            .style(theme.annotation()),
        layout[1],
    );

    let metric_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(4); model.metrics.len()])
        .split(layout[2]);

    for (index, metric) in model.metrics.iter().enumerate() {
        let row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(18),
                Constraint::Length(18),
                Constraint::Min(20),
                Constraint::Length(20),
            ])
            .split(metric_areas[index]);

        frame.render_widget(
            Paragraph::new(format!(
                "{}\ncurrent {}",
                metric.label, metric.current_value
            ))
            .style(theme.hero())
            .block(chrome::panel(
                theme,
                Line::from(metric.label),
                PanelKind::Subtle,
            )),
            row[0],
        );
        frame.render_widget(
            Paragraph::new(metric.confidence.clone())
                .style(theme.badge(chrome::tone_for_text(&metric.confidence)))
                .block(chrome::panel(
                    theme,
                    Line::from("Signal"),
                    PanelKind::Subtle,
                )),
            row[1],
        );
        frame.render_widget(
            Sparkline::default()
                .block(chrome::panel(
                    theme,
                    Line::from("Direction"),
                    PanelKind::Section,
                ))
                .data(&metric.sparkline),
            row[2],
        );
        frame.render_widget(
            Paragraph::new(metric.summary.clone())
                .style(theme.body())
                .block(chrome::panel(
                    theme,
                    Line::from("Baseline read"),
                    PanelKind::Subtle,
                )),
            row[3],
        );
    }

    draw_notes(frame, layout[3], model, theme);
}

fn draw_compact(frame: &mut Frame<'_>, area: Rect, model: &TrendsModel, theme: &Theme) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(4),
        ])
        .split(area);

    draw_window_tabs(frame, layout[0], model, theme);
    frame.render_widget(
        Paragraph::new(model.windows[model.selected_window_index].summary.clone())
            .style(theme.annotation()),
        layout[1],
    );

    let metrics = model
        .metrics
        .iter()
        .map(|metric| {
            ListItem::new(format!(
                "[metric] {} | {} | {} | {}",
                metric.label, metric.current_value, metric.confidence, metric.summary
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(metrics).block(chrome::panel(
            theme,
            Line::from("Comparison scan"),
            PanelKind::Section,
        )),
        layout[2],
    );

    let notes = model
        .notes
        .iter()
        .take(2)
        .map(|note| ListItem::new(format!("[note] {note}")))
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(notes).block(chrome::panel(
            theme,
            Line::from("Analyst notes"),
            PanelKind::Subtle,
        )),
        layout[3],
    );
}

fn draw_window_tabs(frame: &mut Frame<'_>, area: Rect, model: &TrendsModel, theme: &Theme) {
    frame.render_widget(
        Tabs::new(model.windows.iter().map(|window| window.label))
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    "Trend windows",
                    model.windows[model.selected_window_index].label,
                    Tone::Accent,
                ),
                PanelKind::Hero,
            ))
            .style(theme.annotation())
            .highlight_style(theme.emphasis(Tone::Focus))
            .divider(" ")
            .select(model.selected_window_index),
        area,
    );
}

fn draw_notes(frame: &mut Frame<'_>, area: Rect, model: &TrendsModel, theme: &Theme) {
    let notes = model
        .notes
        .iter()
        .map(|note| ListItem::new(format!("[note] {note}")))
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(notes).block(chrome::panel(
            theme,
            Line::from("Analyst notes"),
            PanelKind::Section,
        )),
        area,
    );
}
