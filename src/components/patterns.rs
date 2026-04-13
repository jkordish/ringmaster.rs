use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    widgets::{Paragraph, Tabs, Wrap},
};

use crate::app::{OverlayToggleView, PatternsModel};
use crate::navigation::FocusRegion;
use crate::ui::{
    chrome::{PanelKind, PanelShellSpec, render_panel_shell},
    layout::{DashboardMetrics, UiContext},
    telemetry::{TelemetryAvailability, TelemetryAvailability::Fresh},
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
    model: &PatternsModel,
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
            Constraint::Length(if ui.viewport.is_compact() { 3 } else { 4 }),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(if ui.viewport.is_compact() { 10 } else { 14 }),
            Constraint::Length(if ui.viewport.is_compact() { 5 } else { 6 }),
        ])
        .split(area);

    let headline = if ui.viewport.is_compact() {
        format!("{} | {}", model.header, model.filter_summary)
    } else {
        format!("{}\n{}", model.header, model.filter_summary)
    };
    let hero_shell = render_panel_shell(
        frame,
        layout[0],
        theme,
        metrics,
        PanelShellSpec {
            title: "Patterns / Browser",
            status: "LOCAL",
            status_tone: Tone::Info,
            focused: false,
            expanded: false,
            kind: PanelKind::Hero,
        },
    );
    frame.render_widget(
        Paragraph::new(headline)
            .wrap(Wrap { trim: true })
            .style(theme.hero()),
        hero_shell.content_area,
    );

    draw_metric_tabs(
        frame,
        layout[1],
        model,
        theme,
        metrics,
        focused_region == FocusRegion::ContextPrimary,
        expanded_region == Some(FocusRegion::ContextPrimary),
    );
    draw_overlay_tabs(
        frame,
        layout[2],
        &model.overlay_toggles,
        theme,
        metrics,
        focused_region == FocusRegion::ContextSecondary,
        expanded_region == Some(FocusRegion::ContextSecondary),
    );

    let body = Layout::default()
        .direction(if ui.viewport.is_compact() {
            Direction::Vertical
        } else {
            Direction::Horizontal
        })
        .spacing(metrics.panel_gap_x)
        .constraints(if ui.viewport.is_compact() {
            vec![Constraint::Percentage(62), Constraint::Percentage(38)]
        } else {
            vec![Constraint::Percentage(64), Constraint::Percentage(36)]
        })
        .split(layout[3]);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(body[1]);

    render_lines_panel(
        frame,
        body[0],
        theme,
        metrics,
        LinesPanelSpec {
            title: "Grouped Findings",
            availability: model.findings_availability,
            lines: &pattern_lines(model),
            panel_state: PanelState {
                focused: focused_region == FocusRegion::Primary,
                expanded: expanded_region == Some(FocusRegion::Primary),
            },
        },
    );
    render_lines_panel(
        frame,
        right[0],
        theme,
        metrics,
        LinesPanelSpec {
            title: "Reading Guide",
            availability: model.guide_availability,
            lines: &guide_lines(model),
            panel_state: PanelState {
                focused: false,
                expanded: false,
            },
        },
    );
    render_lines_panel(
        frame,
        right[1],
        theme,
        metrics,
        LinesPanelSpec {
            title: "Interpretation",
            availability: model.interpretation_availability,
            lines: &[String::from(
                "Patterns stay descriptive on purpose. Use Explain or Timeline to validate any row that looks important.",
            )],
            panel_state: PanelState {
                focused: false,
                expanded: false,
            },
        },
    );

    render_lines_panel(
        frame,
        layout[4],
        theme,
        metrics,
        LinesPanelSpec {
            title: "Next Step",
            availability: ai_panel_availability(model),
            lines: &model.ai_actions,
            panel_state: PanelState {
                focused: false,
                expanded: false,
            },
        },
    );
}

fn draw_metric_tabs(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &PatternsModel,
    theme: &Theme,
    metrics: DashboardMetrics,
    focused: bool,
    expanded: bool,
) {
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        metrics,
        PanelShellSpec {
            title: "Metric Filter",
            status: "FILTER",
            status_tone: Tone::Focus,
            focused,
            expanded,
            kind: PanelKind::Section,
        },
    );
    frame.render_widget(
        Tabs::new(model.metric_filters.iter().map(|tab| tab.label))
            .style(theme.annotation())
            .highlight_style(theme.emphasis(Tone::Focus))
            .divider(" ")
            .select(model.selected_filter_index),
        shell.content_area,
    );
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
            title: "Family Filter",
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

fn pattern_lines(model: &PatternsModel) -> Vec<String> {
    if model.rows.is_empty() {
        vec!["Patterns browser".to_owned(), model.empty_message.clone()]
    } else {
        std::iter::once("Patterns browser".to_owned())
            .chain(model.rows.iter().enumerate().map(|(index, row)| {
                let rank = index + 1;
                let badge_suffix = if row.badges.is_empty() {
                    String::new()
                } else {
                    format!(" | {}", row.badges.join(" / "))
                };
                format!("#{rank} {} | {}{}", row.headline, row.detail, badge_suffix)
            }))
            .collect()
    }
}

fn guide_lines(model: &PatternsModel) -> Vec<String> {
    let mut lines = model
        .notes
        .iter()
        .map(|note| format!("[note] {note}"))
        .collect::<Vec<_>>();
    lines.push(
        "[guide] Use the filter strip to narrow families, then switch views to validate a finding."
            .to_owned(),
    );
    lines
}

const fn ai_panel_availability(model: &PatternsModel) -> TelemetryAvailability {
    if model.ai_actions.is_empty() {
        TelemetryAvailability::NoData
    } else {
        Fresh
    }
}

fn render_lines_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: &Theme,
    metrics: DashboardMetrics,
    spec: LinesPanelSpec<'_>,
) {
    let body = if spec.lines.is_empty() {
        "No local entries yet.".to_owned()
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
