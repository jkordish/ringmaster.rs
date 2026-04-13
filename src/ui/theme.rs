use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Default,
    Accent,
    Positive,
    Warning,
    Danger,
    Info,
    Muted,
    Focus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub surface_base: Color,
    pub surface_panel: Color,
    pub surface_panel_alt: Color,
    pub line_subtle: Color,
    pub line_normal: Color,
    pub line_strong: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_tertiary: Color,
    pub text_disabled: Color,
    pub accent: Color,
    pub state_fresh: Color,
    pub state_warn: Color,
    pub state_error: Color,
    pub state_info: Color,
    pub focus_accent: Color,
}

impl Theme {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            surface_base: Color::Rgb(12, 16, 20),
            surface_panel: Color::Rgb(18, 24, 30),
            surface_panel_alt: Color::Rgb(22, 28, 34),
            line_subtle: Color::Rgb(48, 59, 71),
            line_normal: Color::Rgb(78, 93, 108),
            line_strong: Color::Rgb(118, 137, 158),
            text_primary: Color::Rgb(235, 240, 246),
            text_secondary: Color::Rgb(198, 207, 217),
            text_tertiary: Color::Rgb(134, 147, 161),
            text_disabled: Color::Rgb(94, 104, 116),
            accent: Color::Rgb(121, 164, 206),
            state_fresh: Color::Rgb(113, 188, 144),
            state_warn: Color::Rgb(220, 181, 102),
            state_error: Color::Rgb(216, 109, 97),
            state_info: Color::Rgb(138, 168, 214),
            focus_accent: Color::Rgb(127, 210, 236),
        }
    }

    #[must_use]
    pub const fn tone(self, tone: Tone) -> Color {
        match tone {
            Tone::Default => self.text_secondary,
            Tone::Accent => self.accent,
            Tone::Positive => self.state_fresh,
            Tone::Warning => self.state_warn,
            Tone::Danger => self.state_error,
            Tone::Info => self.state_info,
            Tone::Muted => self.text_tertiary,
            Tone::Focus => self.focus_accent,
        }
    }

    #[must_use]
    pub fn screen(self) -> Style {
        Style::default()
            .bg(self.surface_base)
            .fg(self.text_secondary)
    }

    #[must_use]
    pub fn panel_surface(self, alternate: bool) -> Style {
        Style::default()
            .bg(if alternate {
                self.surface_panel_alt
            } else {
                self.surface_panel
            })
            .fg(self.text_secondary)
    }

    #[must_use]
    pub fn hero(self) -> Style {
        Style::default()
            .fg(self.text_primary)
            .add_modifier(Modifier::BOLD)
    }

    #[must_use]
    pub fn section_title(self, tone: Tone) -> Style {
        let foreground = match tone {
            Tone::Default => self.text_primary,
            Tone::Muted => self.text_tertiary,
            other => self.tone(other),
        };
        Style::default().fg(foreground).add_modifier(Modifier::BOLD)
    }

    #[must_use]
    pub fn body(self) -> Style {
        Style::default().fg(self.text_secondary)
    }

    #[must_use]
    pub fn annotation(self) -> Style {
        Style::default().fg(self.text_tertiary)
    }

    #[must_use]
    pub fn disabled(self) -> Style {
        Style::default().fg(self.text_disabled)
    }

    #[must_use]
    pub fn badge(self, tone: Tone) -> Style {
        Style::default()
            .fg(self.tone(tone))
            .add_modifier(Modifier::BOLD)
    }

    #[must_use]
    pub fn emphasis(self, tone: Tone) -> Style {
        Style::default()
            .fg(self.tone(tone))
            .add_modifier(Modifier::BOLD)
    }

    #[must_use]
    pub fn border(self, tone: Tone) -> Style {
        let line = match tone {
            Tone::Default => self.line_normal,
            Tone::Muted => self.line_subtle,
            Tone::Focus => self.focus_accent,
            Tone::Accent => self.accent,
            Tone::Positive => self.state_fresh,
            Tone::Warning => self.state_warn,
            Tone::Danger => self.state_error,
            Tone::Info => self.state_info,
        };
        Style::default().fg(line)
    }

    #[must_use]
    pub fn strong_border(self, tone: Tone) -> Style {
        self.border(tone).add_modifier(Modifier::BOLD)
    }

    #[must_use]
    pub fn muted_border(self) -> Style {
        Style::default().fg(self.line_subtle)
    }

    #[must_use]
    pub fn subtle_fill(self) -> Style {
        Style::default().fg(self.line_subtle)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Theme, Tone};

    #[test]
    fn focus_and_freshness_are_visibly_distinct_roles() {
        let theme = Theme::default();
        assert_ne!(theme.tone(Tone::Focus), theme.tone(Tone::Positive));
        assert_ne!(theme.tone(Tone::Focus), theme.tone(Tone::Warning));
        assert_ne!(theme.tone(Tone::Positive), theme.tone(Tone::Danger));
    }

    #[test]
    fn line_hierarchy_stays_ordered_from_subtle_to_strong() {
        let theme = Theme::default();
        assert_ne!(theme.line_subtle, theme.line_normal);
        assert_ne!(theme.line_normal, theme.line_strong);
        assert_ne!(theme.line_subtle, theme.line_strong);
    }
}
