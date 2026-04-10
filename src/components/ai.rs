use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    text::Line,
    widgets::{List, ListItem, Paragraph, Tabs, Wrap},
};

use crate::app::{AiLaunchPointView, AiPreflightView, AiWorkbenchModel};
use crate::ui::{
    chrome::{self, PanelKind},
    layout::UiContext,
    theme::{Theme, Tone},
};

pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &AiWorkbenchModel,
    ui: &UiContext,
    theme: &Theme,
) {
    if ui.viewport.is_compact() {
        draw_compact(frame, area, model, theme);
    } else {
        draw_wide(frame, area, model, theme);
    }

    if let Some(preflight) = &model.preflight {
        draw_preflight_overlay(frame, area, preflight, theme, ui.viewport.is_compact());
    }
}

fn draw_wide(frame: &mut Frame<'_>, area: Rect, model: &AiWorkbenchModel, theme: &Theme) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Min(16),
            Constraint::Length(7),
        ])
        .split(area);

    draw_intro(frame, layout[0], model, theme, false);
    draw_browser_tabs(frame, layout[1], model, theme);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(layout[2]);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(8)])
        .split(body[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(10)])
        .split(body[1]);

    draw_launch_points(frame, left[0], &model.launch_points, theme, true);
    draw_browser_list(frame, left[1], model, theme);
    draw_trust(frame, right[0], model, theme);
    draw_detail(frame, right[1], model, theme);
    draw_warnings(frame, layout[3], model, theme);
}

fn draw_compact(frame: &mut Frame<'_>, area: Rect, model: &AiWorkbenchModel, theme: &Theme) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Min(8),
            Constraint::Length(5),
        ])
        .split(area);

    draw_intro(frame, layout[0], model, theme, true);
    draw_browser_tabs(frame, layout[1], model, theme);
    draw_launch_points(frame, layout[2], &model.launch_points, theme, false);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(layout[3]);
    draw_browser_list(frame, body[0], model, theme);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(6)])
        .split(body[1]);
    draw_trust(frame, right[0], model, theme);
    draw_detail(frame, right[1], model, theme);
    draw_warnings(frame, layout[4], model, theme);
}

fn draw_intro(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &AiWorkbenchModel,
    theme: &Theme,
    compact: bool,
) {
    let mut lines = vec![model.headline.clone()];
    if compact {
        lines.extend(model.summary_lines.iter().take(1).cloned());
    } else {
        lines.extend(model.summary_lines.iter().cloned());
    }
    let text = lines.join("\n");
    frame.render_widget(
        Paragraph::new(text)
            .style(theme.hero())
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(theme, "AI workbench", "snapshot-first", Tone::Accent),
                PanelKind::Hero,
            )),
        area,
    );
}

fn draw_browser_tabs(frame: &mut Frame<'_>, area: Rect, model: &AiWorkbenchModel, theme: &Theme) {
    let titles = model
        .browser_tabs
        .iter()
        .map(|tab| format!("{} ({})", tab.label, tab.count))
        .collect::<Vec<_>>();
    frame.render_widget(
        Tabs::new(titles)
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(theme, "Browser", "saved artifacts", Tone::Focus),
                PanelKind::Section,
            ))
            .style(theme.annotation())
            .highlight_style(theme.emphasis(Tone::Focus))
            .divider(" ")
            .select(model.selected_tab_index),
        area,
    );
}

fn draw_launch_points(
    frame: &mut Frame<'_>,
    area: Rect,
    items: &[AiLaunchPointView],
    theme: &Theme,
    trim_to_label: bool,
) {
    let rows = items
        .iter()
        .map(|item| {
            let detail = if trim_to_label {
                format!("{} [{}]", item.label, item.key_hint)
            } else {
                format!("{} [{}]\n{}", item.label, item.key_hint, item.detail)
            };
            ListItem::new(detail)
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(rows).block(chrome::panel(
            theme,
            chrome::title_with_badge(theme, "Launch points", "guided only", Tone::Info),
            PanelKind::Subtle,
        )),
        area,
    );
}

fn draw_browser_list(frame: &mut Frame<'_>, area: Rect, model: &AiWorkbenchModel, theme: &Theme) {
    let items = if model.browser_items.is_empty() {
        vec![ListItem::new(
            "[empty] Nothing is saved in this browser slice yet.",
        )]
    } else {
        model
            .browser_items
            .iter()
            .map(|item| {
                ListItem::new(format!(
                    "{} {} | {}\n{}",
                    chrome::focus_prefix(item.selected),
                    item.headline,
                    item.status_badge,
                    item.detail
                ))
            })
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(items).block(chrome::panel(theme, Line::from("List"), PanelKind::Section)),
        area,
    );
}

fn draw_trust(frame: &mut Frame<'_>, area: Rect, model: &AiWorkbenchModel, theme: &Theme) {
    frame.render_widget(
        Paragraph::new(model.trust_lines.join("\n"))
            .wrap(Wrap { trim: true })
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(theme, "Preflight defaults", "trust surface", Tone::Muted),
                PanelKind::Diagnostic,
            )),
        area,
    );
}

fn draw_detail(frame: &mut Frame<'_>, area: Rect, model: &AiWorkbenchModel, theme: &Theme) {
    let text = if model.detail_lines.is_empty() {
        "Select a saved snapshot, run, or report to inspect the persisted local detail.".to_owned()
    } else {
        model.detail_lines.join("\n")
    };
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(theme, &model.detail_title, "detail", Tone::Focus),
                PanelKind::Hero,
            )),
        area,
    );
}

fn draw_warnings(frame: &mut Frame<'_>, area: Rect, model: &AiWorkbenchModel, theme: &Theme) {
    let items = if model.warning_lines.is_empty() {
        vec![ListItem::new(
            "[quiet] Privacy, provider, and artifact state look healthy.",
        )]
    } else {
        model
            .warning_lines
            .iter()
            .map(|line| ListItem::new(format!("[warn] {line}")))
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(items).block(chrome::panel(
            theme,
            chrome::title_with_badge(theme, "Warnings", "operator attention", Tone::Warning),
            PanelKind::Section,
        )),
        area,
    );
}

fn draw_preflight_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    preflight: &AiPreflightView,
    theme: &Theme,
    compact: bool,
) {
    let popup = centered_rect(
        area,
        if compact { 92 } else { 74 },
        if compact { 72 } else { 68 },
    );
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(if compact { 5 } else { 7 }),
        ])
        .split(popup);

    frame.render_widget(
        Paragraph::new(preflight.body_lines.join("\n"))
            .wrap(Wrap { trim: true })
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    &preflight.title,
                    "explicit send gate",
                    Tone::Accent,
                ),
                PanelKind::Hero,
            )),
        sections[0],
    );

    let warning_items = if preflight.warning_lines.is_empty() {
        vec![ListItem::new(if preflight.confirm_enabled {
            "[ready] This run stays snapshot-bounded and stateless by default."
        } else {
            "[blocked] This preflight cannot be confirmed until readiness issues are resolved."
        })]
    } else {
        preflight
            .warning_lines
            .iter()
            .map(|line| ListItem::new(format!("[warn] {line}")))
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(warning_items).block(chrome::panel(
            theme,
            chrome::title_with_badge(theme, "Confirmation", "trust before upload", Tone::Warning),
            PanelKind::Diagnostic,
        )),
        sections[1],
    );
}

fn centered_rect(area: Rect, width_pct: u16, height_pct: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_pct) / 2),
            Constraint::Percentage(height_pct),
            Constraint::Percentage((100 - height_pct) / 2),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_pct) / 2),
            Constraint::Percentage(width_pct),
            Constraint::Percentage((100 - width_pct) / 2),
        ])
        .split(vertical[1]);
    horizontal[1]
}
