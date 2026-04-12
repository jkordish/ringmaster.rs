use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    widgets::{Paragraph, Tabs, Wrap},
};

use crate::app::{ExplainModel, OverlayToggleView};
use crate::navigation::FocusRegion;
use crate::ui::{
    layout::UiContext,
    telemetry::panel_block,
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
    model: &ExplainModel,
    ui: &UiContext,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if ui.viewport.is_compact() { 4 } else { 5 }),
            Constraint::Length(3),
            Constraint::Length(if ui.viewport.is_compact() { 7 } else { 8 }),
            Constraint::Min(if ui.viewport.is_compact() { 8 } else { 12 }),
            Constraint::Length(if ui.viewport.is_compact() { 6 } else { 8 }),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(explain_headline(model, ui))
            .wrap(Wrap { trim: true })
            .style(theme.hero())
            .block(panel_block(
                theme,
                "Explain / Day Story",
                "LOCAL",
                Tone::Info,
                false,
                false,
            )),
        layout[0],
    );

    draw_overlay_tabs(
        frame,
        layout[1],
        &model.overlay_toggles,
        theme,
        focused_region == FocusRegion::ContextPrimary,
        expanded_region == Some(FocusRegion::ContextPrimary),
    );

    draw_summary_section(
        frame,
        layout[2],
        model,
        ui,
        theme,
        focused_region == FocusRegion::Primary,
        expanded_region == Some(FocusRegion::Primary),
    );
    draw_evidence_section(frame, layout[3], model, ui, theme);
    draw_footer_section(frame, layout[4], model, ui, theme);
}

fn draw_overlay_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    toggles: &[OverlayToggleView],
    theme: &Theme,
    focused: bool,
    expanded: bool,
) {
    let selected_index = toggles
        .iter()
        .position(|toggle| toggle.selected)
        .unwrap_or(0);
    frame.render_widget(
        Tabs::new(toggles.iter().map(overlay_tab_label))
            .block(panel_block(
                theme,
                "Overlay Filters",
                "FILTER",
                Tone::Info,
                focused,
                expanded,
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
    focused: bool,
    expanded: bool,
) {
    let summary = Layout::default()
        .direction(if ui.viewport.is_compact() {
            Direction::Vertical
        } else {
            Direction::Horizontal
        })
        .constraints(if ui.viewport.is_compact() {
            vec![Constraint::Percentage(55), Constraint::Percentage(45)]
        } else {
            vec![Constraint::Percentage(58), Constraint::Percentage(42)]
        })
        .split(area);

    render_lines_panel(
        frame,
        summary[0],
        theme,
        "Claim",
        model.claim_availability,
        &model.summary_lines,
        PanelState { focused, expanded },
    );
    render_lines_panel(
        frame,
        summary[1],
        theme,
        "Measured Inputs",
        model.measurements_availability,
        &model.measurement_lines,
        PanelState {
            focused: false,
            expanded: false,
        },
    );
}

fn draw_evidence_section(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &ExplainModel,
    ui: &UiContext,
    theme: &Theme,
) {
    let middle = Layout::default()
        .direction(if ui.viewport.is_compact() {
            Direction::Vertical
        } else {
            Direction::Horizontal
        })
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(area);

    let evidence_lines = model
        .evidence_badges
        .iter()
        .map(|badge| format!("[evidence] {badge}"))
        .chain(model.evidence_lines.iter().cloned())
        .collect::<Vec<_>>();
    let evidence_lines = std::iter::once("Supporting evidence".to_owned())
        .chain(evidence_lines)
        .collect::<Vec<_>>();

    render_lines_panel(
        frame,
        middle[0],
        theme,
        "Evidence Rails",
        model.evidence_availability,
        &evidence_lines,
        PanelState {
            focused: false,
            expanded: false,
        },
    );
    render_lines_panel(
        frame,
        middle[1],
        theme,
        "Context",
        model.context_availability,
        &model.context_lines,
        PanelState {
            focused: false,
            expanded: false,
        },
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

    render_lines_panel(
        frame,
        footer[0],
        theme,
        "Uncertainty",
        model.uncertainty_availability,
        &std::iter::once("Uncertainty".to_owned())
            .chain(
                model
                    .caveat_lines
                    .iter()
                    .map(|line| format!("[caveat] {line}")),
            )
            .collect::<Vec<_>>(),
        PanelState {
            focused: false,
            expanded: false,
        },
    );
    render_lines_panel(
        frame,
        footer[1],
        theme,
        "AI Launch",
        model.ai_availability,
        &model.ai_actions,
        PanelState {
            focused: false,
            expanded: false,
        },
    );
}

fn render_lines_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: &Theme,
    title: &str,
    availability: crate::ui::telemetry::TelemetryAvailability,
    lines: &[String],
    panel_state: PanelState,
) {
    let body = if lines.is_empty() {
        "No local evidence yet.".to_owned()
    } else {
        lines.join("\n")
    };
    frame.render_widget(
        Paragraph::new(body)
            .wrap(Wrap { trim: true })
            .block(panel_block(
                theme,
                title,
                availability.label(),
                availability.tone(),
                panel_state.focused,
                panel_state.expanded,
            )),
        area,
    );
}
