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
    pub background: Color,
    pub surface_1: Color,
    pub surface_2: Color,
    pub surface_3: Color,
    pub text_strong: Color,
    pub text: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub positive: Color,
    pub warning: Color,
    pub danger: Color,
    pub info: Color,
    pub focus: Color,
}

impl Theme {
    pub const fn new() -> Self {
        Self {
            background: Color::Black,
            surface_1: Color::Rgb(24, 28, 34),
            surface_2: Color::Rgb(31, 36, 43),
            surface_3: Color::Rgb(39, 45, 53),
            text_strong: Color::Rgb(235, 239, 244),
            text: Color::Rgb(205, 212, 219),
            text_muted: Color::Rgb(128, 138, 150),
            accent: Color::Rgb(94, 168, 212),
            positive: Color::Rgb(115, 186, 123),
            warning: Color::Rgb(214, 177, 94),
            danger: Color::Rgb(210, 103, 89),
            info: Color::Rgb(120, 161, 206),
            focus: Color::Rgb(188, 218, 95),
        }
    }

    pub fn tone(self, tone: Tone) -> Color {
        match tone {
            Tone::Default => self.text,
            Tone::Accent => self.accent,
            Tone::Positive => self.positive,
            Tone::Warning => self.warning,
            Tone::Danger => self.danger,
            Tone::Info => self.info,
            Tone::Muted => self.text_muted,
            Tone::Focus => self.focus,
        }
    }

    pub fn screen(self) -> Style {
        Style::default().bg(self.background).fg(self.text)
    }

    pub fn hero(self) -> Style {
        Style::default()
            .fg(self.text_strong)
            .add_modifier(Modifier::BOLD)
    }

    pub fn section_title(self, tone: Tone) -> Style {
        Style::default()
            .fg(self.tone(tone))
            .add_modifier(Modifier::BOLD)
    }

    pub fn body(self) -> Style {
        Style::default().fg(self.text)
    }

    pub fn annotation(self) -> Style {
        Style::default().fg(self.text_muted)
    }

    pub fn badge(self, tone: Tone) -> Style {
        Style::default()
            .fg(self.tone(tone))
            .add_modifier(Modifier::BOLD)
    }

    pub fn emphasis(self, tone: Tone) -> Style {
        Style::default()
            .fg(self.tone(tone))
            .add_modifier(Modifier::BOLD)
    }

    pub fn border(self, tone: Tone) -> Style {
        Style::default().fg(self.tone(tone))
    }

    pub fn muted_border(self) -> Style {
        Style::default().fg(self.surface_3)
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
    fn palette_roles_are_distinct_enough_for_state_language() {
        let theme = Theme::default();
        assert_ne!(theme.tone(Tone::Accent), theme.tone(Tone::Warning));
        assert_ne!(theme.tone(Tone::Warning), theme.tone(Tone::Danger));
        assert_ne!(theme.tone(Tone::Positive), theme.tone(Tone::Danger));
    }
}
