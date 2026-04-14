use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    widgets::{Paragraph, Tabs, Wrap},
};

use crate::app::{AiArtifactSummaryView, ReviewModel};
use crate::navigation::FocusRegion;
use crate::ui::{
    chrome::{PanelKind, PanelShellSpec, render_panel_shell},
    layout::{DashboardMetrics, UiContext},
    telemetry::TelemetryAvailability,
    theme::{Theme, Tone},
};

#[derive(Debug, Clone, Copy)]
struct PanelState {
    focused: bool,
    expanded: bool,
}

pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &ReviewModel,
    ui: &UiContext,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
) {
    if ui.viewport.is_compact() {
        draw_compact(frame, area, model, theme, focused_region, expanded_region);
    } else {
        draw_wide(frame, area, model, theme, focused_region, expanded_region);
    }
}

fn draw_wide(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &ReviewModel,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
) {
    let metrics =
        DashboardMetrics::for_viewport(crate::ui::layout::ViewportClass::from_width(area.width));
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Min(14),
            Constraint::Length(7),
        ])
        .split(area);

    draw_intro(frame, layout[0], model, theme, false);
    draw_mode_tabs(
        frame,
        layout[1],
        model,
        theme,
        false,
        focused_region == FocusRegion::ContextPrimary,
        expanded_region == Some(FocusRegion::ContextPrimary),
    );
    draw_focus_tabs(
        frame,
        layout[2],
        model,
        theme,
        false,
        focused_region == FocusRegion::ContextSecondary,
        expanded_region == Some(FocusRegion::ContextSecondary),
    );

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(layout[3]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([Constraint::Length(11), Constraint::Min(8)])
        .split(body[1]);

    render_lines_panel(
        frame,
        body[0],
        theme,
        "Ranked Observations",
        model.cards_availability,
        &card_lines(model),
        PanelState {
            focused: focused_region == FocusRegion::Primary,
            expanded: expanded_region == Some(FocusRegion::Primary),
        },
    );
    render_lines_panel(
        frame,
        right[0],
        theme,
        "AI Artifact",
        model.ai_artifact.availability,
        &ai_artifact_lines(&model.ai_artifact, &model.ai_actions),
        PanelState {
            focused: false,
            expanded: false,
        },
    );
    render_lines_panel(
        frame,
        right[1],
        theme,
        "Selected Brief",
        model.detail_availability,
        &detail_lines(model),
        PanelState {
            focused: focused_region == FocusRegion::Secondary,
            expanded: expanded_region == Some(FocusRegion::Secondary),
        },
    );
    render_lines_panel(
        frame,
        layout[4],
        theme,
        "Warnings and Caveats",
        model.warnings_availability,
        &warning_lines(model),
        PanelState {
            focused: false,
            expanded: false,
        },
    );
}

fn draw_compact(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &ReviewModel,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
) {
    let metrics =
        DashboardMetrics::for_viewport(crate::ui::layout::ViewportClass::from_width(area.width));
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(7),
        ])
        .split(area);

    draw_intro(frame, layout[0], model, theme, true);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(layout[1]);
    render_lines_panel(
        frame,
        body[0],
        theme,
        "Ranked Observations",
        model.cards_availability,
        &card_lines(model),
        PanelState {
            focused: focused_region == FocusRegion::Primary,
            expanded: expanded_region == Some(FocusRegion::Primary),
        },
    );
    render_lines_panel(
        frame,
        body[1],
        theme,
        "Selected Brief",
        model.detail_availability,
        &detail_lines(model),
        PanelState {
            focused: focused_region == FocusRegion::Secondary,
            expanded: expanded_region == Some(FocusRegion::Secondary),
        },
    );

    render_lines_panel(
        frame,
        layout[2],
        theme,
        "AI Artifact",
        model.ai_artifact.availability,
        &compact_footer_lines(model),
        PanelState {
            focused: false,
            expanded: false,
        },
    );
}

fn draw_intro(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &ReviewModel,
    theme: &Theme,
    compact: bool,
) {
    let selected_mode = selected_tab_label(&model.mode_tabs, model.selected_mode_index);
    let selected_focus = selected_tab_label(&model.focus_tabs, model.selected_focus_index);
    let intro = if compact {
        format!(
            "{}\nMode [{}] | Focus [{}]",
            model.selected_day_label, selected_mode, selected_focus
        )
    } else {
        format!(
            "{}\n{}\nMode [{}] | Focus [{}]\nPrioritized from persisted local telemetry and evidence rails.",
            model.selected_day_label, model.breadcrumb, selected_mode, selected_focus
        )
    };
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        DashboardMetrics::for_viewport(crate::ui::layout::ViewportClass::from_width(area.width)),
        PanelShellSpec {
            title: "Review / Daily Brief",
            status: "LOCAL",
            status_tone: Tone::Info,
            focused: false,
            expanded: false,
            kind: PanelKind::Hero,
        },
    );
    frame.render_widget(
        Paragraph::new(intro)
            .wrap(Wrap { trim: true })
            .style(theme.hero()),
        shell.content_area,
    );
}

fn draw_mode_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &ReviewModel,
    theme: &Theme,
    compact: bool,
    focused: bool,
    expanded: bool,
) {
    if compact {
        let selected = selected_tab_label(&model.mode_tabs, model.selected_mode_index);
        let line = format!(
            "Mode [{selected}]  {}",
            model
                .mode_tabs
                .iter()
                .map(|tab| tab.label.as_str())
                .collect::<Vec<_>>()
                .join("  ")
        );
        let shell = render_panel_shell(
            frame,
            area,
            theme,
            DashboardMetrics::for_viewport(crate::ui::layout::ViewportClass::from_width(
                area.width,
            )),
            PanelShellSpec {
                title: "Mode",
                status: selected,
                status_tone: Tone::Focus,
                focused,
                expanded,
                kind: PanelKind::Section,
            },
        );
        frame.render_widget(
            Paragraph::new(line).wrap(Wrap { trim: true }),
            shell.content_area,
        );
    } else {
        let shell = render_panel_shell(
            frame,
            area,
            theme,
            DashboardMetrics::for_viewport(crate::ui::layout::ViewportClass::from_width(
                area.width,
            )),
            PanelShellSpec {
                title: "Mode",
                status: "FILTER",
                status_tone: Tone::Focus,
                focused,
                expanded,
                kind: PanelKind::Section,
            },
        );
        frame.render_widget(
            Tabs::new(model.mode_tabs.iter().map(|tab| tab.label.as_str()))
                .style(theme.annotation())
                .highlight_style(theme.emphasis(Tone::Focus))
                .divider(" ")
                .select(model.selected_mode_index),
            shell.content_area,
        );
    }
}

fn draw_focus_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &ReviewModel,
    theme: &Theme,
    compact: bool,
    focused: bool,
    expanded: bool,
) {
    if compact {
        let selected = selected_tab_label(&model.focus_tabs, model.selected_focus_index);
        let line = format!(
            "Focus [{selected}]  {}",
            model
                .focus_tabs
                .iter()
                .map(|tab| tab.label.as_str())
                .collect::<Vec<_>>()
                .join("  ")
        );
        let shell = render_panel_shell(
            frame,
            area,
            theme,
            DashboardMetrics::for_viewport(crate::ui::layout::ViewportClass::from_width(
                area.width,
            )),
            PanelShellSpec {
                title: "Focus",
                status: selected,
                status_tone: Tone::Info,
                focused,
                expanded,
                kind: PanelKind::Section,
            },
        );
        frame.render_widget(
            Paragraph::new(line).wrap(Wrap { trim: true }),
            shell.content_area,
        );
    } else {
        let shell = render_panel_shell(
            frame,
            area,
            theme,
            DashboardMetrics::for_viewport(crate::ui::layout::ViewportClass::from_width(
                area.width,
            )),
            PanelShellSpec {
                title: "Focus",
                status: "FILTER",
                status_tone: Tone::Info,
                focused,
                expanded,
                kind: PanelKind::Section,
            },
        );
        frame.render_widget(
            Tabs::new(model.focus_tabs.iter().map(|tab| tab.label.as_str()))
                .style(theme.annotation())
                .highlight_style(theme.emphasis(Tone::Focus))
                .divider(" ")
                .select(model.selected_focus_index),
            shell.content_area,
        );
    }
}

fn card_lines(model: &ReviewModel) -> Vec<String> {
    if model.cards.is_empty() {
        return vec![
            "Ranked observations".to_owned(),
            format!("[empty] {}", model.empty_message),
        ];
    }

    std::iter::once("Ranked observations".to_owned())
        .chain(model.cards.iter().enumerate().map(|(index, card)| {
            let prefix = if card.selected { ">" } else { " " };
            let rank = index + 1;
            let badge_suffix = if card.badges.is_empty() {
                String::new()
            } else {
                format!(" | {}", card.badges.join(" / "))
            };
            format!(
                "{prefix} #{rank} {} | {} | {}{}",
                card.headline, card.section_label, card.confidence_label, badge_suffix
            )
        }))
        .collect()
}

fn detail_lines(model: &ReviewModel) -> Vec<String> {
    if model.detail_lines.is_empty() {
        return vec![
            "Briefing detail".to_owned(),
            "[pending] Review evidence appears after enough local history accumulates.".to_owned(),
        ];
    }

    std::iter::once("Briefing detail".to_owned())
        .chain(model.detail_lines.iter().map(|line| {
            if line.is_empty() {
                " ".to_owned()
            } else if line.ends_with(':') {
                format!("[section] {line}")
            } else {
                line.clone()
            }
        }))
        .collect()
}

fn ai_artifact_lines(artifact: &AiArtifactSummaryView, ai_actions: &[String]) -> Vec<String> {
    let mut lines = vec![format!("AI artifact: {}", artifact.status_label)];
    lines.extend(artifact.metadata_lines.iter().cloned());

    if !artifact.summary_text.is_empty() {
        if artifact.status_label == "available" && !lines.is_empty() {
            lines.push(String::new());
        }
        if artifact.status_label == "available" {
            lines.push("Saved summary:".to_owned());
        }
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

    lines
}

fn compact_footer_lines(model: &ReviewModel) -> Vec<String> {
    let mut lines = ai_artifact_lines(&model.ai_artifact, &model.ai_actions);
    if let Some(first_warning) = model.warning_lines.first() {
        lines.push(String::new());
        lines.push(format!("[watch] {first_warning}"));
    }
    lines
}

fn warning_lines(model: &ReviewModel) -> Vec<String> {
    if model.warning_lines.is_empty() {
        return vec![
            "Warnings and caveats".to_owned(),
            "[quiet] No active briefing warnings.".to_owned(),
        ];
    }

    std::iter::once("Warnings and caveats".to_owned())
        .chain(
            model
                .warning_lines
                .iter()
                .map(|line| format!("[watch] {line}")),
        )
        .collect()
}

fn selected_tab_label(tabs: &[crate::app::ReviewTab], selected_index: usize) -> &str {
    tabs.get(selected_index)
        .map_or("--", |tab| tab.label.as_str())
}

fn render_lines_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: &Theme,
    title: &str,
    availability: TelemetryAvailability,
    lines: &[String],
    panel_state: PanelState,
) {
    let body = if lines.is_empty() {
        "No local entries yet.".to_owned()
    } else {
        lines.join("\n")
    };
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        DashboardMetrics::for_viewport(crate::ui::layout::ViewportClass::from_width(area.width)),
        PanelShellSpec {
            title,
            status: availability.label(),
            status_tone: availability.tone(),
            focused: panel_state.focused,
            expanded: panel_state.expanded,
            kind: PanelKind::Section,
        },
    );
    frame.render_widget(
        Paragraph::new(body).wrap(Wrap { trim: true }),
        shell.content_area,
    );
}
