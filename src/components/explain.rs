use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    widgets::{Paragraph, Tabs, Wrap},
};

use crate::app::{ExplainModel, OverlayToggleView};
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

#[derive(Debug, Clone, Copy)]
struct LinesPanelSpec<'a> {
    title: &'a str,
    availability: TelemetryAvailability,
    lines: &'a [String],
    panel_state: PanelState,
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
    let metrics = DashboardMetrics::for_viewport(ui.viewport);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([
            Constraint::Length(if ui.viewport.is_compact() { 4 } else { 5 }),
            Constraint::Length(3),
            Constraint::Length(if ui.viewport.is_compact() { 7 } else { 8 }),
            Constraint::Min(if ui.viewport.is_compact() { 8 } else { 12 }),
            Constraint::Length(if ui.viewport.is_compact() { 6 } else { 8 }),
        ])
        .split(area);

    let hero_shell = render_panel_shell(
        frame,
        layout[0],
        theme,
        metrics,
        PanelShellSpec {
            title: "Explain / Day Story",
            status: "LOCAL",
            status_tone: Tone::Info,
            focused: false,
            expanded: false,
            kind: PanelKind::Hero,
        },
    );
    frame.render_widget(
        Paragraph::new(explain_headline(model, ui))
            .wrap(Wrap { trim: true })
            .style(theme.hero()),
        hero_shell.content_area,
    );

    draw_overlay_tabs(
        frame,
        layout[1],
        &model.overlay_toggles,
        theme,
        metrics,
        focused_region == FocusRegion::ContextPrimary,
        expanded_region == Some(FocusRegion::ContextPrimary),
    );

    draw_summary_section(
        frame,
        layout[2],
        model,
        ui,
        theme,
        metrics,
        PanelState {
            focused: focused_region == FocusRegion::Primary,
            expanded: expanded_region == Some(FocusRegion::Primary),
        },
    );
    draw_evidence_section(frame, layout[3], model, ui, theme, metrics);
    draw_footer_section(frame, layout[4], model, ui, theme, metrics);
}

fn draw_overlay_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    toggles: &[OverlayToggleView],
    theme: &Theme,
    metrics: DashboardMetrics,
    focused: bool,
    expanded: bool,
) {
    let selected_index = toggles
        .iter()
        .position(|toggle| toggle.selected)
        .unwrap_or(0);
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        metrics,
        PanelShellSpec {
            title: "Overlay Filters",
            status: "FILTER",
            status_tone: Tone::Info,
            focused,
            expanded,
            kind: PanelKind::Section,
        },
    );
    frame.render_widget(
        Tabs::new(toggles.iter().map(overlay_tab_label))
            .style(theme.annotation())
            .highlight_style(theme.emphasis(Tone::Focus))
            .divider(" ")
            .select(selected_index),
        shell.content_area,
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
    metrics: DashboardMetrics,
    panel_state: PanelState,
) {
    let summary = Layout::default()
        .direction(if ui.viewport.is_compact() {
            Direction::Vertical
        } else {
            Direction::Horizontal
        })
        .spacing(metrics.panel_gap_x)
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
        metrics,
        LinesPanelSpec {
            title: "Claim",
            availability: model.claim_availability,
            lines: &model.summary_lines,
            panel_state,
        },
    );
    render_lines_panel(
        frame,
        summary[1],
        theme,
        metrics,
        LinesPanelSpec {
            title: "Measured Inputs",
            availability: model.measurements_availability,
            lines: &model.measurement_lines,
            panel_state: PanelState {
                focused: false,
                expanded: false,
            },
        },
    );
}

fn draw_evidence_section(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &ExplainModel,
    ui: &UiContext,
    theme: &Theme,
    metrics: DashboardMetrics,
) {
    let middle = Layout::default()
        .direction(if ui.viewport.is_compact() {
            Direction::Vertical
        } else {
            Direction::Horizontal
        })
        .spacing(metrics.panel_gap_x)
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
        metrics,
        LinesPanelSpec {
            title: "Evidence Rails",
            availability: model.evidence_availability,
            lines: &evidence_lines,
            panel_state: PanelState {
                focused: false,
                expanded: false,
            },
        },
    );
    render_lines_panel(
        frame,
        middle[1],
        theme,
        metrics,
        LinesPanelSpec {
            title: "Context",
            availability: model.context_availability,
            lines: &model.context_lines,
            panel_state: PanelState {
                focused: false,
                expanded: false,
            },
        },
    );
}

fn draw_footer_section(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &ExplainModel,
    ui: &UiContext,
    theme: &Theme,
    metrics: DashboardMetrics,
) {
    let footer = Layout::default()
        .direction(if ui.viewport.is_compact() {
            Direction::Horizontal
        } else {
            Direction::Vertical
        })
        .spacing(metrics.panel_gap_y)
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
        metrics,
        LinesPanelSpec {
            title: "Uncertainty",
            availability: model.uncertainty_availability,
            lines: &std::iter::once("Uncertainty".to_owned())
                .chain(
                    model
                        .caveat_lines
                        .iter()
                        .map(|line| format!("[caveat] {line}")),
                )
                .collect::<Vec<_>>(),
            panel_state: PanelState {
                focused: false,
                expanded: false,
            },
        },
    );
    render_lines_panel(
        frame,
        footer[1],
        theme,
        metrics,
        LinesPanelSpec {
            title: "AI Launch",
            availability: model.ai_availability,
            lines: &model.ai_actions,
            panel_state: PanelState {
                focused: false,
                expanded: false,
            },
        },
    );
}

fn render_lines_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: &Theme,
    metrics: DashboardMetrics,
    spec: LinesPanelSpec<'_>,
) {
    let body = if spec.lines.is_empty() {
        "No local evidence yet.".to_owned()
    } else {
        spec.lines.join("\n")
    };
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        metrics,
        PanelShellSpec {
            title: spec.title,
            status: spec.availability.label(),
            status_tone: spec.availability.tone(),
            focused: spec.panel_state.focused,
            expanded: spec.panel_state.expanded,
            kind: PanelKind::Section,
        },
    );
    frame.render_widget(
        Paragraph::new(body).wrap(Wrap { trim: true }),
        shell.content_area,
    );
}
