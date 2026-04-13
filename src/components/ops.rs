use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    widgets::{Cell, List, ListItem, Paragraph, Row, Table, Wrap},
};

use crate::app::{FamilyStatusView, OpsItem, OpsModel};
use crate::navigation::FocusRegion;
use crate::ui::{
    chrome::{PanelKind, PanelShellSpec, render_panel_shell},
    layout::{DashboardMetrics, UiContext, ViewportClass},
    telemetry::coverage_rows,
    theme::{Theme, Tone},
};

#[derive(Debug, Clone, Copy)]
struct OpsPanelState {
    metrics: DashboardMetrics,
    focused: bool,
    expanded: bool,
}

pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    ui: &UiContext,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
) {
    let metrics = DashboardMetrics::for_viewport(ui.viewport);
    match ui.viewport {
        ViewportClass::Compact => {
            draw_compact(
                frame,
                area,
                model,
                theme,
                focused_region,
                expanded_region,
                metrics,
            );
        }
        ViewportClass::Medium => {
            draw_medium(
                frame,
                area,
                model,
                theme,
                focused_region,
                expanded_region,
                metrics,
            );
        }
        ViewportClass::Wide => {
            draw_wide(
                frame,
                area,
                model,
                theme,
                focused_region,
                expanded_region,
                metrics,
            );
        }
    }
}

fn draw_wide(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
    metrics: DashboardMetrics,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(16),
            Constraint::Length(7),
        ])
        .split(area);

    draw_summary(
        frame,
        layout[0],
        model,
        theme,
        OpsPanelState {
            metrics,
            focused: focused_region == FocusRegion::OpsSummary,
            expanded: expanded_region == Some(FocusRegion::OpsSummary),
        },
    );

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(metrics.panel_gap_x)
        .constraints([
            Constraint::Percentage(26),
            Constraint::Percentage(32),
            Constraint::Percentage(42),
        ])
        .split(layout[1]);

    draw_coverage_panel(
        frame,
        body[0],
        model,
        theme,
        OpsPanelState {
            metrics,
            focused: focused_region == FocusRegion::OpsCoverage,
            expanded: expanded_region == Some(FocusRegion::OpsCoverage),
        },
    );
    draw_family_table(frame, body[1], model, theme, metrics);
    let diagnostics = prioritized_diagnostic_items(model);
    draw_diagnostics_list(
        frame,
        body[2],
        &diagnostics,
        theme,
        None,
        OpsPanelState {
            metrics,
            focused: focused_region == FocusRegion::OpsDiagnostics,
            expanded: expanded_region == Some(FocusRegion::OpsDiagnostics),
        },
    );
    draw_warning_list(
        frame,
        layout[2],
        model,
        theme,
        OpsPanelState {
            metrics,
            focused: focused_region == FocusRegion::OpsWarnings,
            expanded: expanded_region == Some(FocusRegion::OpsWarnings),
        },
    );
}

fn draw_medium(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
    metrics: DashboardMetrics,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(15),
            Constraint::Length(7),
        ])
        .split(area);

    draw_summary(
        frame,
        layout[0],
        model,
        theme,
        OpsPanelState {
            metrics,
            focused: focused_region == FocusRegion::OpsSummary,
            expanded: expanded_region == Some(FocusRegion::OpsSummary),
        },
    );

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(metrics.panel_gap_x)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(30),
            Constraint::Percentage(50),
        ])
        .split(layout[1]);

    draw_coverage_panel(
        frame,
        body[0],
        model,
        theme,
        OpsPanelState {
            metrics,
            focused: focused_region == FocusRegion::OpsCoverage,
            expanded: expanded_region == Some(FocusRegion::OpsCoverage),
        },
    );
    draw_family_summary(
        frame,
        body[1],
        &model.family_statuses,
        theme,
        metrics,
        false,
    );
    let diagnostics = prioritized_diagnostic_items(model);
    draw_diagnostics_readout(
        frame,
        body[2],
        &diagnostics,
        theme,
        None,
        OpsPanelState {
            metrics,
            focused: focused_region == FocusRegion::OpsDiagnostics,
            expanded: expanded_region == Some(FocusRegion::OpsDiagnostics),
        },
    );
    draw_warning_readout(
        frame,
        layout[2],
        model,
        theme,
        OpsPanelState {
            metrics,
            focused: focused_region == FocusRegion::OpsWarnings,
            expanded: expanded_region == Some(FocusRegion::OpsWarnings),
        },
    );
}

fn draw_compact(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    theme: &Theme,
    focused_region: FocusRegion,
    expanded_region: Option<FocusRegion>,
    metrics: DashboardMetrics,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .spacing(metrics.panel_gap_y)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(6),
            Constraint::Min(11),
            Constraint::Length(3),
        ])
        .split(area);

    draw_summary(
        frame,
        layout[0],
        model,
        theme,
        OpsPanelState {
            metrics,
            focused: focused_region == FocusRegion::OpsSummary,
            expanded: expanded_region == Some(FocusRegion::OpsSummary),
        },
    );
    draw_compact_coverage_panel(
        frame,
        layout[1],
        model,
        theme,
        OpsPanelState {
            metrics,
            focused: focused_region == FocusRegion::OpsCoverage,
            expanded: expanded_region == Some(FocusRegion::OpsCoverage),
        },
    );

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(metrics.panel_gap_x)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
        .split(layout[2]);

    draw_family_summary(frame, body[0], &model.family_statuses, theme, metrics, true);

    let diagnostics = prioritized_diagnostic_items(model);
    draw_diagnostics_readout(
        frame,
        body[1],
        &diagnostics,
        theme,
        Some(6),
        OpsPanelState {
            metrics,
            focused: focused_region == FocusRegion::OpsDiagnostics,
            expanded: expanded_region == Some(FocusRegion::OpsDiagnostics),
        },
    );
    draw_warning_readout(
        frame,
        layout[3],
        model,
        theme,
        OpsPanelState {
            metrics,
            focused: focused_region == FocusRegion::OpsWarnings,
            expanded: expanded_region == Some(FocusRegion::OpsWarnings),
        },
    );
}

fn draw_summary(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    theme: &Theme,
    panel_state: OpsPanelState,
) {
    let summary = if model.summary_lines.is_empty() {
        format!("Mode: {}", model.mode_label)
    } else {
        model.summary_lines.join("\n")
    };
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        panel_state.metrics,
        PanelShellSpec {
            title: "Status",
            status: "MODE",
            status_tone: Tone::Info,
            focused: panel_state.focused,
            expanded: panel_state.expanded,
            kind: PanelKind::Hero,
        },
    );
    frame.render_widget(
        Paragraph::new(summary).style(theme.hero()),
        shell.content_area,
    );
}

fn draw_compact_coverage_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    theme: &Theme,
    panel_state: OpsPanelState,
) {
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        panel_state.metrics,
        PanelShellSpec {
            title: "Coverage",
            status: "MATRIX",
            status_tone: Tone::Info,
            focused: panel_state.focused,
            expanded: panel_state.expanded,
            kind: PanelKind::Section,
        },
    );
    let tokens = model
        .coverage
        .iter()
        .map(|cell| format!("{} [{}]", cell.label, cell.availability.label()))
        .collect::<Vec<_>>();
    let body = packed_token_rows(
        &tokens,
        usize::from(shell.content_area.width.max(1)),
        usize::from(shell.content_area.height.max(1)),
    )
    .join("\n");
    frame.render_widget(Paragraph::new(body), shell.content_area);
}

fn draw_coverage_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    theme: &Theme,
    panel_state: OpsPanelState,
) {
    let coverage = model
        .coverage
        .iter()
        .map(|cell| (cell.label, cell.availability))
        .collect::<Vec<_>>();
    let body = coverage_rows(&coverage).join("\n");
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        panel_state.metrics,
        PanelShellSpec {
            title: "Coverage",
            status: "MATRIX",
            status_tone: Tone::Info,
            focused: panel_state.focused,
            expanded: panel_state.expanded,
            kind: PanelKind::Section,
        },
    );
    frame.render_widget(Paragraph::new(body), shell.content_area);
}

fn draw_family_summary(
    frame: &mut Frame<'_>,
    area: Rect,
    family_statuses: &[FamilyStatusView],
    theme: &Theme,
    metrics: DashboardMetrics,
    compact: bool,
) {
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        metrics,
        PanelShellSpec {
            title: "Family status",
            status: "SYNC",
            status_tone: Tone::Info,
            focused: false,
            expanded: false,
            kind: PanelKind::Diagnostic,
        },
    );
    let body = family_summary_text(
        family_statuses,
        usize::from(shell.content_area.width.max(1)),
        usize::from(shell.content_area.height.max(1)),
        compact,
    );
    frame.render_widget(
        Paragraph::new(body).wrap(Wrap { trim: true }),
        shell.content_area,
    );
}

fn draw_family_table(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    theme: &Theme,
    metrics: DashboardMetrics,
) {
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        metrics,
        PanelShellSpec {
            title: "Family status",
            status: "SYNC",
            status_tone: Tone::Info,
            focused: false,
            expanded: false,
            kind: PanelKind::Diagnostic,
        },
    );
    let family_rows = model.family_statuses.iter().map(|status| {
        Row::new(vec![
            Cell::from(status.label),
            Cell::from(status.state_label.clone()),
            Cell::from(status.scope_label.clone()),
            Cell::from(status.last_sync.clone()),
        ])
    });
    frame.render_widget(
        Table::new(
            family_rows,
            [
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(16),
                Constraint::Min(12),
            ],
        )
        .header(
            Row::new(vec!["Family", "State", "Scope", "Last sync"])
                .style(theme.section_title(Tone::Info)),
        ),
        shell.content_area,
    );
}

fn draw_diagnostics_list(
    frame: &mut Frame<'_>,
    area: Rect,
    items: &[OpsItem],
    theme: &Theme,
    max_items: Option<usize>,
    panel_state: OpsPanelState,
) {
    let diagnostics = items
        .iter()
        .take(max_items.unwrap_or(items.len()))
        .map(|item| ListItem::new(format!("{}: {}", item.label, item.value)))
        .collect::<Vec<_>>();
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        panel_state.metrics,
        PanelShellSpec {
            title: "Diagnostics",
            status: "DETAIL",
            status_tone: Tone::Muted,
            focused: panel_state.focused,
            expanded: panel_state.expanded,
            kind: PanelKind::Diagnostic,
        },
    );
    frame.render_widget(List::new(diagnostics), shell.content_area);
}

fn draw_diagnostics_readout(
    frame: &mut Frame<'_>,
    area: Rect,
    items: &[OpsItem],
    theme: &Theme,
    max_items: Option<usize>,
    panel_state: OpsPanelState,
) {
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        panel_state.metrics,
        PanelShellSpec {
            title: "Diagnostics",
            status: "DETAIL",
            status_tone: Tone::Muted,
            focused: panel_state.focused,
            expanded: panel_state.expanded,
            kind: PanelKind::Diagnostic,
        },
    );
    let truncated_items = items
        .iter()
        .take(max_items.unwrap_or(items.len()))
        .cloned()
        .collect::<Vec<_>>();
    let body = key_value_text(
        &truncated_items,
        usize::from(shell.content_area.width.max(1)),
        usize::from(shell.content_area.height.max(1)),
    );
    frame.render_widget(
        Paragraph::new(body).wrap(Wrap { trim: true }),
        shell.content_area,
    );
}

fn prioritized_diagnostic_items(model: &OpsModel) -> Vec<OpsItem> {
    const PRIORITY_LABELS: [&str; 13] = [
        "Auth state",
        "Granted scopes",
        "Invalidation queue",
        "Receiver heartbeat",
        "Latest eval",
        "Eval health",
        "Subscriptions",
        "Config path",
        "Database path",
        "Access token expiry",
        "Watch heartbeat",
        "Last accepted delivery",
        "Last rejected delivery",
    ];

    let mut diagnostics = PRIORITY_LABELS
        .into_iter()
        .filter_map(|label| model.items.iter().find(|item| item.label == label).cloned())
        .collect::<Vec<_>>();

    for item in &model.items {
        if diagnostics
            .iter()
            .all(|selected| selected.label != item.label)
        {
            diagnostics.push(item.clone());
        }
    }

    diagnostics
}

fn draw_warning_list(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    theme: &Theme,
    panel_state: OpsPanelState,
) {
    let warnings = if model.warnings.is_empty() {
        vec![ListItem::new("[quiet] No warnings.")]
    } else {
        model
            .warnings
            .iter()
            .map(|warning| ListItem::new(format!("[warn] {warning}")))
            .collect::<Vec<_>>()
    };
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        panel_state.metrics,
        PanelShellSpec {
            title: "Warnings",
            status: if model.warnings.is_empty() {
                "CLEAR"
            } else {
                "ATTN"
            },
            status_tone: Tone::Warning,
            focused: panel_state.focused,
            expanded: panel_state.expanded,
            kind: PanelKind::Section,
        },
    );
    frame.render_widget(List::new(warnings), shell.content_area);
}

fn draw_warning_readout(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &OpsModel,
    theme: &Theme,
    panel_state: OpsPanelState,
) {
    let shell = render_panel_shell(
        frame,
        area,
        theme,
        panel_state.metrics,
        PanelShellSpec {
            title: "Warnings",
            status: if model.warnings.is_empty() {
                "CLEAR"
            } else {
                "ATTN"
            },
            status_tone: Tone::Warning,
            focused: panel_state.focused,
            expanded: panel_state.expanded,
            kind: PanelKind::Section,
        },
    );
    let warning_lines = if model.warnings.is_empty() {
        vec!["[quiet] No warnings.".to_owned()]
    } else {
        model
            .warnings
            .iter()
            .map(|warning| format!("[warn] {warning}"))
            .collect::<Vec<_>>()
    };
    let body = limited_rows_text(
        &warning_lines,
        usize::from(shell.content_area.width.max(1)),
        usize::from(shell.content_area.height.max(1)),
    );
    frame.render_widget(
        Paragraph::new(body).wrap(Wrap { trim: true }),
        shell.content_area,
    );
}

fn family_summary_text(
    family_statuses: &[FamilyStatusView],
    width: usize,
    max_lines: usize,
    compact: bool,
) -> String {
    let blocks = family_statuses
        .iter()
        .map(|status| family_status_lines(status, width, compact))
        .collect::<Vec<_>>();
    limited_block_text(&blocks, width, max_lines)
}

fn family_status_lines(status: &FamilyStatusView, width: usize, compact: bool) -> Vec<String> {
    let sync = compact_timestamp(&status.last_sync);
    let scope = status
        .scope_label
        .strip_prefix("scope ")
        .unwrap_or(&status.scope_label);
    if compact {
        vec![
            format!(
                "{} | {}",
                status.label,
                compact_state_label(&status.state_label)
            ),
            format!("{scope} | {sync}"),
        ]
    } else {
        let inline = format!(
            "{} | {} | {} | {}",
            status.label, status.state_label, scope, sync
        );
        if inline.chars().count() <= width {
            vec![inline]
        } else {
            vec![
                format!("{} | {}", status.label, status.state_label),
                format!("{scope} | {sync}"),
            ]
        }
    }
}

fn key_value_text(items: &[OpsItem], width: usize, max_lines: usize) -> String {
    let blocks = items
        .iter()
        .map(|item| key_value_lines(&item.label, &item.value, width))
        .collect::<Vec<_>>();
    limited_block_text(&blocks, width, max_lines)
}

fn key_value_lines(label: &str, value: &str, width: usize) -> Vec<String> {
    let inline = if value.is_empty() {
        label.to_owned()
    } else {
        format!("{label}: {value}")
    };
    if inline.chars().count() <= width.max(1) || value.is_empty() {
        vec![inline]
    } else {
        vec![format!("{label}:"), value.to_owned()]
    }
}

fn limited_rows_text(rows: &[String], width: usize, max_lines: usize) -> String {
    let blocks = rows
        .iter()
        .cloned()
        .map(|row| vec![row])
        .collect::<Vec<_>>();
    limited_block_text(&blocks, width, max_lines)
}

fn limited_block_text(blocks: &[Vec<String>], width: usize, max_lines: usize) -> String {
    if max_lines == 0 || width == 0 {
        return String::new();
    }

    let mut rendered = Vec::new();
    let mut hidden = 0usize;

    for (index, block) in blocks.iter().enumerate() {
        let block_lines = estimated_block_lines(block, width);
        let reserve_for_hidden = usize::from(index + 1 < blocks.len());
        if rendered.len() + block_lines + reserve_for_hidden > max_lines {
            hidden = blocks.len().saturating_sub(index);
            break;
        }
        rendered.extend(block.iter().cloned());
    }

    if rendered.is_empty() && hidden > 0 {
        if let Some(first_line) = blocks
            .first()
            .and_then(|block| block.first())
            .map(|line| ellipsize_value(line, width))
        {
            rendered.push(first_line);
        }
        return rendered.join("\n");
    }

    if hidden > 0 {
        if rendered.len() == max_lines {
            rendered.pop();
        }
        rendered.push(format!("+{hidden} more"));
    }

    rendered.join("\n")
}

fn estimated_block_lines(block: &[String], width: usize) -> usize {
    block
        .iter()
        .map(|line| estimated_wrap_lines(line, width))
        .sum()
}

fn estimated_wrap_lines(text: &str, width: usize) -> usize {
    let width = width.max(1);
    let lines = text.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return 1;
    }
    lines
        .iter()
        .map(|line| line.chars().count().max(1).div_ceil(width))
        .sum()
}

fn packed_token_rows(tokens: &[String], width: usize, max_lines: usize) -> Vec<String> {
    if tokens.is_empty() || width == 0 || max_lines == 0 {
        return vec!["[quiet] No coverage".to_owned()];
    }

    let mut rows = Vec::new();
    let mut current = String::new();

    for token in tokens {
        let candidate = if current.is_empty() {
            token.clone()
        } else {
            format!("{current} · {token}")
        };

        if candidate.chars().count() <= width {
            current = candidate;
        } else {
            if !current.is_empty() {
                rows.push(std::mem::take(&mut current));
                if rows.len() == max_lines {
                    return rows;
                }
            }
            current.clone_from(token);
        }
    }

    if !current.is_empty() && rows.len() < max_lines {
        rows.push(current);
    }

    rows
}

fn compact_state_label(state_label: &str) -> &str {
    state_label.split([' ', ':']).next().unwrap_or(state_label)
}

fn compact_timestamp(value: &str) -> String {
    let Some((date, time)) = value.split_once('T') else {
        return value.to_owned();
    };
    let mut parts = time.trim_end_matches('Z').split(':');
    let hour = parts.next().unwrap_or_default();
    let minute = parts.next().unwrap_or_default();
    if hour.is_empty() || minute.is_empty() {
        value.to_owned()
    } else {
        format!("{date} {hour}:{minute}Z")
    }
}

fn ellipsize_value(value: &str, width: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= width {
        return value.to_owned();
    }
    if width <= 3 {
        return value.chars().take(width).collect();
    }
    let mut truncated = value.chars().take(width - 3).collect::<String>();
    truncated.push_str("...");
    truncated
}
