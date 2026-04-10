use ratatui::style::Style;

use super::theme::{Theme, Tone};

pub fn line_style(theme: &Theme) -> Style {
    theme.emphasis(Tone::Accent)
}

pub fn selected_point_style(theme: &Theme) -> Style {
    theme.emphasis(Tone::Focus)
}

pub fn baseline_style(theme: &Theme) -> Style {
    theme.annotation()
}
