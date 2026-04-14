use ratatui::style::Style;

use super::theme::{Theme, Tone};

#[must_use]
pub fn line_style(theme: &Theme) -> Style {
    theme.emphasis(Tone::AccentNeutral)
}

#[must_use]
pub fn selected_point_style(theme: &Theme) -> Style {
    theme.emphasis(Tone::Focus)
}

#[must_use]
pub fn baseline_style(theme: &Theme) -> Style {
    theme.annotation()
}
