use std::env;

use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorCapability {
    TrueColor,
    Ansi256,
    Ansi16,
    Mono,
}

impl ColorCapability {
    #[must_use]
    pub fn detect() -> Self {
        Self::from_env_vars(
            env::var_os("NO_COLOR").as_deref(),
            env::var_os("COLORTERM").as_deref(),
            env::var_os("TERM").as_deref(),
        )
    }

    #[must_use]
    pub fn from_env_vars(
        no_color: Option<&std::ffi::OsStr>,
        colorterm: Option<&std::ffi::OsStr>,
        term: Option<&std::ffi::OsStr>,
    ) -> Self {
        if no_color.is_some() {
            return Self::Mono;
        }

        let colorterm = colorterm
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if matches!(colorterm.as_str(), "truecolor" | "24bit") {
            return Self::TrueColor;
        }

        let term = term
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if term.contains("256color") {
            return Self::Ansi256;
        }

        Self::Ansi16
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Default,
    Accent,
    Warning,
    Info,
    Muted,
    Focus,
    Fresh,
    Stale,
    Error,
    Unavailable,
    JudgedOk,
    JudgedWarn,
    JudgedAlert,
    DeltaCool,
    DeltaWarm,
    AccentNeutral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub capability: ColorCapability,
    pub surface_base: Color,
    pub surface_panel: Color,
    pub surface_panel_alt: Color,
    pub border_subtle: Color,
    pub border_normal: Color,
    pub border_focus: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_tertiary: Color,
    pub text_disabled: Color,
    pub focus_lilac: Color,
    pub fresh_cyan: Color,
    pub ok_mint: Color,
    pub warn_butter: Color,
    pub alert_rose: Color,
    pub delta_cool: Color,
    pub delta_warm: Color,
    pub na_gray: Color,
}

impl Theme {
    #[must_use]
    pub const fn for_capability(capability: ColorCapability) -> Self {
        match capability {
            ColorCapability::TrueColor => Self {
                capability,
                surface_base: Color::Rgb(12, 16, 20),
                surface_panel: Color::Rgb(18, 24, 30),
                surface_panel_alt: Color::Rgb(22, 28, 34),
                border_subtle: Color::Rgb(53, 61, 72),
                border_normal: Color::Rgb(90, 101, 114),
                border_focus: Color::Rgb(145, 133, 188),
                text_primary: Color::Rgb(236, 241, 247),
                text_secondary: Color::Rgb(203, 211, 220),
                text_tertiary: Color::Rgb(147, 157, 168),
                text_disabled: Color::Rgb(111, 120, 131),
                focus_lilac: Color::Rgb(199, 184, 255),
                fresh_cyan: Color::Rgb(141, 210, 255),
                ok_mint: Color::Rgb(159, 227, 178),
                warn_butter: Color::Rgb(243, 219, 146),
                alert_rose: Color::Rgb(245, 166, 165),
                delta_cool: Color::Rgb(141, 210, 255),
                delta_warm: Color::Rgb(247, 192, 162),
                na_gray: Color::Rgb(173, 182, 195),
            },
            ColorCapability::Ansi256 => Self {
                capability,
                surface_base: Color::Indexed(233),
                surface_panel: Color::Indexed(235),
                surface_panel_alt: Color::Indexed(236),
                border_subtle: Color::Indexed(240),
                border_normal: Color::Indexed(245),
                border_focus: Color::Indexed(183),
                text_primary: Color::Indexed(255),
                text_secondary: Color::Indexed(252),
                text_tertiary: Color::Indexed(249),
                text_disabled: Color::Indexed(244),
                focus_lilac: Color::Indexed(183),
                fresh_cyan: Color::Indexed(117),
                ok_mint: Color::Indexed(151),
                warn_butter: Color::Indexed(223),
                alert_rose: Color::Indexed(217),
                delta_cool: Color::Indexed(117),
                delta_warm: Color::Indexed(216),
                na_gray: Color::Indexed(146),
            },
            ColorCapability::Ansi16 => Self {
                capability,
                surface_base: Color::Black,
                surface_panel: Color::Black,
                surface_panel_alt: Color::Black,
                border_subtle: Color::DarkGray,
                border_normal: Color::Gray,
                border_focus: Color::Magenta,
                text_primary: Color::White,
                text_secondary: Color::Gray,
                text_tertiary: Color::DarkGray,
                text_disabled: Color::DarkGray,
                focus_lilac: Color::Magenta,
                fresh_cyan: Color::Cyan,
                ok_mint: Color::Green,
                warn_butter: Color::Yellow,
                alert_rose: Color::Red,
                delta_cool: Color::Cyan,
                delta_warm: Color::Yellow,
                na_gray: Color::DarkGray,
            },
            ColorCapability::Mono => Self {
                capability,
                surface_base: Color::Reset,
                surface_panel: Color::Reset,
                surface_panel_alt: Color::Reset,
                border_subtle: Color::Reset,
                border_normal: Color::Reset,
                border_focus: Color::Reset,
                text_primary: Color::Reset,
                text_secondary: Color::Reset,
                text_tertiary: Color::Reset,
                text_disabled: Color::Reset,
                focus_lilac: Color::Reset,
                fresh_cyan: Color::Reset,
                ok_mint: Color::Reset,
                warn_butter: Color::Reset,
                alert_rose: Color::Reset,
                delta_cool: Color::Reset,
                delta_warm: Color::Reset,
                na_gray: Color::Reset,
            },
        }
    }

    #[must_use]
    pub const fn tone(self, tone: Tone) -> Color {
        match tone {
            Tone::Default => self.text_secondary,
            Tone::Accent | Tone::Info | Tone::Fresh | Tone::AccentNeutral => self.fresh_cyan,
            Tone::JudgedOk => self.ok_mint,
            Tone::Warning | Tone::Stale | Tone::JudgedWarn => self.warn_butter,
            Tone::Error | Tone::JudgedAlert => self.alert_rose,
            Tone::Muted => self.text_tertiary,
            Tone::Focus => self.focus_lilac,
            Tone::Unavailable => self.na_gray,
            Tone::DeltaCool => self.delta_cool,
            Tone::DeltaWarm => self.delta_warm,
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
            Tone::Unavailable => self.na_gray,
            other => self.tone(other),
        };

        let style = Style::default().fg(foreground).add_modifier(Modifier::BOLD);
        self.with_semantic_modifiers(style, tone)
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
    pub fn badge(self, tone: Tone) -> Style {
        let style = Style::default()
            .fg(self.tone(tone))
            .add_modifier(Modifier::BOLD);
        self.with_semantic_modifiers(style, tone)
    }

    #[must_use]
    pub fn emphasis(self, tone: Tone) -> Style {
        let style = Style::default()
            .fg(self.tone(tone))
            .add_modifier(Modifier::BOLD);
        self.with_semantic_modifiers(style, tone)
    }

    #[must_use]
    pub fn border(self, tone: Tone) -> Style {
        let line = match tone {
            Tone::Default => self.border_normal,
            Tone::Muted => self.border_subtle,
            Tone::Focus => self.border_focus,
            Tone::Unavailable => self.na_gray,
            Tone::Accent | Tone::Info | Tone::Fresh | Tone::AccentNeutral => self.fresh_cyan,
            Tone::JudgedOk => self.ok_mint,
            Tone::Warning | Tone::Stale | Tone::JudgedWarn => self.warn_butter,
            Tone::Error | Tone::JudgedAlert => self.alert_rose,
            Tone::DeltaCool => self.delta_cool,
            Tone::DeltaWarm => self.delta_warm,
        };
        self.with_semantic_modifiers(Style::default().fg(line), tone)
    }

    #[must_use]
    pub fn strong_border(self, tone: Tone) -> Style {
        self.border(tone).add_modifier(Modifier::BOLD)
    }

    #[must_use]
    pub fn muted_border(self) -> Style {
        Style::default().fg(self.border_subtle)
    }

    #[must_use]
    pub fn subtle_fill(self) -> Style {
        Style::default().fg(self.border_subtle)
    }

    #[must_use]
    pub fn dominant_metric(self, tone: Tone) -> Style {
        let style = Style::default()
            .fg(match tone {
                Tone::JudgedWarn if self.capability != ColorCapability::Mono => self.text_primary,
                Tone::JudgedOk | Tone::JudgedAlert | Tone::DeltaCool | Tone::DeltaWarm => {
                    self.tone(tone)
                }
                Tone::Focus => self.tone(Tone::Focus),
                _ => self.text_primary,
            })
            .add_modifier(Modifier::BOLD);
        self.with_semantic_modifiers(style, tone)
    }

    #[must_use]
    pub fn status_marker(self, tone: Tone) -> Style {
        let style = Style::default().fg(self.tone(tone));
        self.with_semantic_modifiers(style, tone)
    }

    #[must_use]
    pub fn chart_ramp(self, level: usize, total: usize) -> Style {
        let max_index = total.saturating_sub(1).max(1);
        let bucket = level.min(max_index).saturating_mul(4) / max_index;
        match self.capability {
            ColorCapability::Mono => {
                let modifier = match bucket {
                    0 => Modifier::DIM,
                    1 => Modifier::empty(),
                    _ => Modifier::BOLD,
                };
                Style::default()
                    .fg(self.text_secondary)
                    .add_modifier(modifier)
            }
            ColorCapability::TrueColor | ColorCapability::Ansi256 | ColorCapability::Ansi16 => {
                let fg = match self.capability {
                    ColorCapability::TrueColor => match bucket {
                        0 => self.text_disabled,
                        1 => self.text_tertiary,
                        2 => Color::Rgb(137, 177, 197),
                        3 => Color::Rgb(150, 204, 221),
                        _ => self.ok_mint,
                    },
                    ColorCapability::Ansi256 => match bucket {
                        0 => Color::Indexed(244),
                        1 => Color::Indexed(146),
                        2 => Color::Indexed(109),
                        3 => Color::Indexed(116),
                        _ => Color::Indexed(151),
                    },
                    ColorCapability::Ansi16 => match bucket {
                        0 => Color::DarkGray,
                        1 => Color::Gray,
                        2 | 3 => Color::Cyan,
                        _ => Color::Green,
                    },
                    ColorCapability::Mono => unreachable!("mono handled above"),
                };
                let mut style = Style::default().fg(fg);
                if matches!(self.capability, ColorCapability::Ansi16) && bucket >= 3 {
                    style = style.add_modifier(Modifier::BOLD);
                }
                style
            }
        }
    }

    fn with_semantic_modifiers(self, style: Style, tone: Tone) -> Style {
        if self.capability != ColorCapability::Mono {
            return style;
        }

        match tone {
            Tone::Focus => style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            Tone::Warning
            | Tone::Stale
            | Tone::JudgedWarn
            | Tone::DeltaWarm
            | Tone::DeltaCool
            | Tone::JudgedOk
            | Tone::Fresh
            | Tone::Info => style.add_modifier(Modifier::BOLD),
            Tone::Error | Tone::JudgedAlert => {
                style.add_modifier(Modifier::BOLD | Modifier::REVERSED)
            }
            Tone::Unavailable | Tone::Muted => style.add_modifier(Modifier::DIM),
            Tone::Default | Tone::Accent | Tone::AccentNeutral => style,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::for_capability(ColorCapability::TrueColor)
    }
}

#[cfg(test)]
mod tests {
    use super::{ColorCapability, Theme, Tone};

    #[test]
    fn detects_color_capability_with_expected_precedence() {
        assert_eq!(
            ColorCapability::from_env_vars(Some("1".as_ref()), Some("truecolor".as_ref()), None),
            ColorCapability::Mono
        );
        assert_eq!(
            ColorCapability::from_env_vars(
                None,
                Some("24bit".as_ref()),
                Some("xterm-256color".as_ref())
            ),
            ColorCapability::TrueColor
        );
        assert_eq!(
            ColorCapability::from_env_vars(None, None, Some("screen-256color".as_ref())),
            ColorCapability::Ansi256
        );
        assert_eq!(
            ColorCapability::from_env_vars(None, None, Some("screen".as_ref())),
            ColorCapability::Ansi16
        );
    }

    #[test]
    fn focus_freshness_and_health_stay_distinct_in_truecolor() {
        let theme = Theme::default();
        assert_ne!(theme.tone(Tone::Focus), theme.tone(Tone::Fresh));
        assert_ne!(theme.tone(Tone::Focus), theme.tone(Tone::JudgedOk));
        assert_ne!(theme.tone(Tone::Fresh), theme.tone(Tone::JudgedOk));
        assert_ne!(theme.tone(Tone::JudgedWarn), theme.tone(Tone::JudgedAlert));
    }

    #[test]
    fn ansi16_fallback_preserves_role_separation() {
        let theme = Theme::for_capability(ColorCapability::Ansi16);
        assert_ne!(theme.tone(Tone::Focus), theme.tone(Tone::Fresh));
        assert_ne!(theme.tone(Tone::Focus), theme.tone(Tone::JudgedOk));
        assert_ne!(theme.tone(Tone::JudgedWarn), theme.tone(Tone::JudgedAlert));
    }

    #[test]
    fn neutral_shell_hierarchy_stays_ordered() {
        let theme = Theme::default();
        assert_ne!(theme.border_subtle, theme.border_normal);
        assert_ne!(theme.border_normal, theme.border_focus);
        assert_ne!(theme.text_secondary, theme.text_tertiary);
    }
}
