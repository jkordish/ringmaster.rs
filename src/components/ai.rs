use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    text::Line,
    widgets::{Clear, List, ListItem, Paragraph, Tabs, Wrap},
};

use crate::app::{
    AiArtifactActionView, AiLaunchPointView, AiPreflightControlView, AiPreflightView,
    AiWorkbenchModel,
};
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
    if ui.viewport.is_wide() {
        draw_wide(frame, area, model, theme);
    } else {
        draw_narrow(frame, area, model, theme, ui.viewport.is_compact());
    }

    if let Some(preflight) = &model.preflight {
        draw_preflight_overlay(frame, area, preflight, theme, !ui.viewport.is_wide());
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
    draw_browser_list(frame, left[1], model, theme, false);
    draw_trust(frame, right[0], model, theme);
    draw_artifact_pane(frame, right[1], model, theme, false);
    draw_warnings(frame, layout[3], model, theme);
}

fn draw_narrow(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &AiWorkbenchModel,
    theme: &Theme,
    compact: bool,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if compact { 4 } else { 5 }),
            Constraint::Length(3),
            Constraint::Min(if compact { 9 } else { 12 }),
            Constraint::Length(if compact { 5 } else { 6 }),
        ])
        .split(area);

    draw_intro(frame, layout[0], model, theme, compact);
    draw_browser_tabs(frame, layout[1], model, theme);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(if compact {
            [Constraint::Percentage(44), Constraint::Percentage(56)]
        } else {
            [Constraint::Percentage(40), Constraint::Percentage(60)]
        })
        .split(layout[2]);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if compact { 5 } else { 6 }),
            Constraint::Min(4),
        ])
        .split(body[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if compact { 5 } else { 6 }),
            Constraint::Min(4),
        ])
        .split(body[1]);
    draw_launch_points(frame, left[0], &model.launch_points, theme, true);
    draw_browser_list(frame, left[1], model, theme, true);
    draw_trust(frame, right[0], model, theme);
    draw_artifact_pane(frame, right[1], model, theme, compact);
    draw_warnings(frame, layout[3], model, theme);
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
            let prefix = chrome::focus_prefix(item.selected);
            let detail = if trim_to_label {
                format!("{prefix} {}", item.label)
            } else {
                format!("{prefix} {}\n{}", item.label, item.detail)
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

fn draw_browser_list(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &AiWorkbenchModel,
    theme: &Theme,
    condensed: bool,
) {
    let items = if model.browser_items.is_empty() {
        vec![ListItem::new(
            "[empty] Nothing is saved in this browser slice yet.",
        )]
    } else {
        model
            .browser_items
            .iter()
            .map(|item| {
                let focus = chrome::focus_prefix(item.selected);
                let headline = if condensed {
                    truncate_text(&item.headline, area.width.saturating_sub(8) as usize)
                } else {
                    format!("{} | {}", item.headline, item.status_badge)
                };
                let detail = truncate_text(&item.detail, area.width.saturating_sub(4) as usize);
                ListItem::new(format!("{focus} {headline}\n{detail}"))
            })
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(items).block(chrome::panel(theme, Line::from("List"), PanelKind::Section)),
        area,
    );
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_owned();
    }
    if max_chars <= 3 {
        return text.chars().take(max_chars).collect();
    }
    let prefix = text.chars().take(max_chars - 3).collect::<String>();
    format!("{prefix}...")
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

fn draw_artifact_pane(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &AiWorkbenchModel,
    theme: &Theme,
    compact: bool,
) {
    let action_height = if model.artifact_actions.is_empty() {
        4
    } else if compact {
        6
    } else {
        8
    };
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(action_height), Constraint::Min(4)])
        .split(area);
    draw_artifact_actions(frame, sections[0], &model.artifact_actions, theme);

    let text = if model.detail_lines.is_empty() {
        "Select a saved snapshot, run, or report to inspect the persisted local detail.".to_owned()
    } else {
        format!("{}\n{}", model.detail_title, model.detail_lines.join("\n"))
    };
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .block(chrome::panel(
                theme,
                chrome::title_with_badge(
                    theme,
                    "Artifact detail",
                    "read-only context",
                    Tone::Muted,
                ),
                PanelKind::Diagnostic,
            )),
        sections[1],
    );
}

fn draw_artifact_actions(
    frame: &mut Frame<'_>,
    area: Rect,
    actions: &[AiArtifactActionView],
    theme: &Theme,
) {
    let items = if actions.is_empty() {
        vec![ListItem::new(
            "[empty] No direct actions are available for this saved artifact.",
        )]
    } else {
        actions
            .iter()
            .map(|action| {
                let focus = chrome::focus_prefix(action.selected);
                ListItem::new(format!("{focus} {}\n{}", action.label, action.detail))
            })
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        List::new(items).block(chrome::panel(
            theme,
            chrome::title_with_badge(theme, "Artifact actions", "canonical controls", Tone::Focus),
            PanelKind::Section,
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
    frame.render_widget(Clear, popup);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(3),
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

    draw_preflight_controls(frame, sections[1], &preflight.controls, theme);

    frame.render_widget(
        List::new(warning_items).block(chrome::panel(
            theme,
            chrome::title_with_badge(theme, "Confirmation", "trust before upload", Tone::Warning),
            PanelKind::Diagnostic,
        )),
        sections[2],
    );
}

fn draw_preflight_controls(
    frame: &mut Frame<'_>,
    area: Rect,
    controls: &[AiPreflightControlView],
    theme: &Theme,
) {
    let selected_index = controls
        .iter()
        .position(|control| control.selected)
        .unwrap_or(0);
    frame.render_widget(
        Tabs::new(
            controls
                .iter()
                .map(|control| format!("{}: {}", control.label, control.detail)),
        )
        .block(chrome::panel(
            theme,
            chrome::title_with_badge(theme, "Controls", "confirm / privacy / cancel", Tone::Focus),
            PanelKind::Section,
        ))
        .style(theme.annotation())
        .highlight_style(theme.emphasis(Tone::Focus))
        .divider(" ")
        .select(selected_index),
        area,
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
