use ratatui::{
    prelude::{Line, Span, Style},
    widgets::{Block, Borders, Paragraph},
};

use super::theme::{Theme, Tone};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    Hero,
    Section,
    Subtle,
    Diagnostic,
}

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

pub fn title_with_badge<'a>(theme: &Theme, title: &str, badge: &str, badge_tone: Tone) -> Line<'a> {
    Line::from(vec![
        Span::styled(title.to_owned(), theme.section_title(Tone::Default)),
        Span::raw(" "),
        Span::styled(format!("[{badge}]"), theme.badge(badge_tone)),
    ])
}

pub fn hero_paragraph<'a>(
    theme: &Theme,
    title: impl Into<Line<'a>>,
    body: impl Into<String>,
    kind: PanelKind,
) -> Paragraph<'a> {
    Paragraph::new(body.into())
        .style(theme.hero())
        .block(panel(theme, title, kind))
}

pub fn badge_label(prefix: &str, text: &str) -> String {
    format!("[{prefix}] {text}")
}

pub fn tone_for_text(text: &str) -> Tone {
    let lower = text.to_ascii_lowercase();
    if lower.contains("error") || lower.contains("failed") || lower.contains("missing heartbeat") {
        Tone::Danger
    } else if lower.contains("stale")
        || lower.contains("warning")
        || lower.contains("thin")
        || lower.contains("waiting")
    {
        Tone::Warning
    } else if lower.contains("focus") || lower.contains("selected") {
        Tone::Focus
    } else if lower.contains("fresh") || lower.contains("ready") || lower.contains("success") {
        Tone::Positive
    } else if lower.contains("mode") || lower.contains("receiver") || lower.contains("queue") {
        Tone::Info
    } else {
        Tone::Muted
    }
}

pub fn focus_prefix(selected: bool) -> &'static str {
    if selected { ">" } else { " " }
}

pub fn emphasis_style(theme: &Theme, selected: bool, tone: Tone) -> Style {
    let base = if selected {
        theme.emphasis(Tone::Focus)
    } else {
        theme.body()
    };

    if selected {
        base
    } else {
        base.fg(theme.tone(tone))
    }
}

#[cfg(test)]
mod tests {
    use super::badge_label;

    #[test]
    fn badge_labels_do_not_rely_on_color() {
        assert_eq!(
            badge_label("STALE", "daily sync pending"),
            "[STALE] daily sync pending"
        );
    }
}
