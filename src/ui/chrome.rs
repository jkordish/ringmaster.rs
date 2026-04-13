use ratatui::{
    Frame,
    layout::Rect,
    prelude::{Line, Span},
    symbols::border,
    widgets::{Block, Borders, Paragraph},
};

use super::layout::{DashboardMetrics, inset};
use super::theme::{Theme, Tone};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    Hero,
    Section,
    Subtle,
    Diagnostic,
}

#[derive(Debug, Clone, Copy)]
pub struct PanelShellSpec<'a> {
    pub title: &'a str,
    pub status: &'a str,
    pub status_tone: Tone,
    pub focused: bool,
    pub expanded: bool,
    pub kind: PanelKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelShell {
    pub inner: Rect,
    pub title_area: Rect,
    pub body_area: Rect,
    pub content_area: Rect,
}

#[must_use]
pub fn panel<'a>(theme: &Theme, title: impl Into<Line<'a>>, kind: PanelKind) -> Block<'a> {
    let tone = match kind {
        PanelKind::Hero => Tone::Accent,
        PanelKind::Section => Tone::Default,
        PanelKind::Subtle => Tone::Muted,
        PanelKind::Diagnostic => Tone::Info,
    };

    Block::default()
        .title(title)
        .title_style(theme.section_title(tone))
        .borders(Borders::ALL)
        .border_style(if matches!(kind, PanelKind::Subtle) {
            theme.muted_border()
        } else {
            theme.border(tone)
        })
        .style(theme.body())
}

#[must_use]
pub fn app_frame(theme: &Theme) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_set(border::DOUBLE)
        .border_style(theme.border(Tone::Accent))
        .style(theme.body())
}

#[must_use]
pub fn title_with_badge<'a>(theme: &Theme, title: &str, badge: &str, badge_tone: Tone) -> Line<'a> {
    Line::from(vec![
        Span::styled(title.to_owned(), theme.section_title(Tone::Default)),
        Span::raw(" "),
        Span::styled(format!("[{badge}]"), theme.badge(badge_tone)),
    ])
}

pub fn render_panel_shell(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: &Theme,
    metrics: DashboardMetrics,
    spec: PanelShellSpec<'_>,
) -> PanelShell {
    let base_block = Block::default()
        .borders(Borders::ALL)
        .border_set(if spec.focused {
            border::THICK
        } else {
            border::PLAIN
        })
        .border_style(match (spec.focused, spec.kind) {
            (true, _) => theme.border(Tone::Focus),
            (false, PanelKind::Hero) => theme.border(Tone::Default),
            (false, _) => theme.muted_border(),
        })
        .style(theme.body());
    if area.height <= 3 {
        let (left_text, open_text, status_text) =
            panel_title_row_segments(metrics, spec, area.width.saturating_sub(2));
        let title = Line::from(vec![
            Span::styled(
                left_text,
                theme.section_title(if spec.focused {
                    Tone::Focus
                } else {
                    Tone::Default
                }),
            ),
            Span::styled(open_text, theme.badge(Tone::Focus)),
            Span::styled(status_text, theme.badge(spec.status_tone)),
        ]);
        let block = base_block.title(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let content_area = inset(inner, metrics.panel_pad_x, metrics.panel_pad_y);
        return PanelShell {
            inner,
            title_area: Rect::new(
                area.x.saturating_add(1),
                area.y,
                area.width.saturating_sub(2),
                1,
            ),
            body_area: inner,
            content_area,
        };
    }

    let block = base_block;
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return PanelShell {
            inner,
            title_area: Rect::new(inner.x, inner.y, 0, 0),
            body_area: Rect::new(inner.x, inner.y, 0, 0),
            content_area: Rect::new(inner.x, inner.y, 0, 0),
        };
    }

    let title_height = metrics.title_row_height.min(inner.height);
    let title_area = Rect::new(inner.x, inner.y, inner.width, title_height);
    let (left_text, open_text, status_text) = panel_title_row_segments(metrics, spec, inner.width);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                left_text,
                theme.section_title(if spec.focused {
                    Tone::Focus
                } else {
                    Tone::Default
                }),
            ),
            Span::styled(open_text, theme.badge(Tone::Focus)),
            Span::styled(status_text, theme.badge(spec.status_tone)),
        ]))
        .style(theme.body()),
        title_area,
    );

    let body_top = if inner.height > metrics.content_top_inset {
        inner.y.saturating_add(metrics.content_top_inset)
    } else {
        inner.y.saturating_add(title_height)
    };
    if metrics.title_separator_gap > 0 && inner.height > metrics.content_top_inset {
        let separator_y = inner.y.saturating_add(title_height);
        let separator_area = Rect::new(inner.x, separator_y, inner.width, 1);
        frame.render_widget(
            Paragraph::new(subtle_rule(inner.width as usize)).style(theme.muted_border()),
            separator_area,
        );
    }

    let body_height = inner
        .height
        .saturating_sub(body_top.saturating_sub(inner.y))
        .max(1);
    let body_area = Rect::new(inner.x, body_top, inner.width, body_height);
    let content_area = inset(body_area, metrics.panel_pad_x, metrics.panel_pad_y);

    PanelShell {
        inner,
        title_area,
        body_area,
        content_area,
    }
}

#[must_use]
pub fn subtle_rule(width: usize) -> String {
    "─".repeat(width)
}

#[cfg(test)]
#[must_use]
pub fn badge_label(prefix: &str, text: &str) -> String {
    format!("[{prefix}] {text}")
}

#[must_use]
pub const fn focus_prefix(selected: bool) -> &'static str {
    if selected { ">" } else { " " }
}

#[cfg(test)]
fn panel_title_row_text(metrics: DashboardMetrics, spec: PanelShellSpec<'_>, width: u16) -> String {
    let (left, open, status) = panel_title_row_segments(metrics, spec, width);
    format!("{left}{open}{status}")
}

fn panel_title_row_segments(
    metrics: DashboardMetrics,
    spec: PanelShellSpec<'_>,
    width: u16,
) -> (String, String, String) {
    let available = usize::from(width);
    if available == 0 {
        return (String::new(), String::new(), String::new());
    }

    let title = spec.title.to_ascii_uppercase();
    let focus_marker = if spec.focused { ">" } else { " " };
    let left = format!(
        "{focus_marker:<gutter$}{}",
        title,
        gutter = metrics.focus_gutter_width
    );
    let status = format!("[{:^badge$}]", spec.status, badge = metrics.badge_width);
    let open = if spec.expanded {
        "[OPEN] ".to_owned()
    } else {
        String::new()
    };
    let right_width = open.len() + status.len();

    if right_width >= available {
        return (
            String::new(),
            String::new(),
            truncate_ascii(&format!("{open}{status}"), available),
        );
    }

    let left_width = available.saturating_sub(right_width);
    let left = truncate_ascii(&left, left_width);
    (format!("{left:<left_width$}"), open, status)
}

fn truncate_ascii(value: &str, width: usize) -> String {
    value.chars().take(width).collect()
}

#[cfg(test)]
mod tests {
    use super::{DashboardMetrics, PanelKind, PanelShellSpec, badge_label, panel_title_row_text};
    use crate::ui::layout::ViewportClass;
    use crate::ui::theme::Tone;

    #[test]
    fn badge_labels_do_not_rely_on_color() {
        assert_eq!(
            badge_label("STALE", "daily sync pending"),
            "[STALE] daily sync pending"
        );
    }

    #[test]
    fn panel_title_rows_normalize_badge_width_without_shifting_titles() {
        let metrics = DashboardMetrics::for_viewport(ViewportClass::Wide);
        let fresh = panel_title_row_text(
            metrics,
            PanelShellSpec {
                title: "Readiness",
                status: "FRESH",
                status_tone: Tone::Positive,
                focused: false,
                expanded: false,
                kind: PanelKind::Section,
            },
            40,
        );
        let na = panel_title_row_text(
            metrics,
            PanelShellSpec {
                title: "Readiness",
                status: "N/A",
                status_tone: Tone::Muted,
                focused: false,
                expanded: false,
                kind: PanelKind::Section,
            },
            40,
        );

        assert_eq!(fresh.find("READINESS"), na.find("READINESS"));
        assert_eq!(fresh.len(), na.len());
    }
}
