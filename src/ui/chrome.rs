use ratatui::{
    prelude::{Line, Span},
    widgets::{Block, Borders},
};

use super::theme::{Theme, Tone};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    Hero,
    Section,
    Subtle,
    Diagnostic,
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
pub fn title_with_badge<'a>(theme: &Theme, title: &str, badge: &str, badge_tone: Tone) -> Line<'a> {
    Line::from(vec![
        Span::styled(title.to_owned(), theme.section_title(Tone::Default)),
        Span::raw(" "),
        Span::styled(format!("[{badge}]"), theme.badge(badge_tone)),
    ])
}

#[must_use]
pub fn badge_label(prefix: &str, text: &str) -> String {
    format!("[{prefix}] {text}")
}

#[must_use]
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

#[must_use]
pub const fn focus_prefix(selected: bool) -> &'static str {
    if selected { ">" } else { " " }
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
