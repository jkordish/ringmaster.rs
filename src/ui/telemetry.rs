use ratatui::{
    prelude::{Line, Span},
    widgets::{Block, Borders},
};

use crate::numeric::rounded_clamped_f64_to_u16;
use crate::ui::theme::{Theme, Tone};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryAvailability {
    Fresh,
    Stale,
    NoData,
    MissingScope,
    RateLimited,
    Error,
    Unsupported,
}

impl TelemetryAvailability {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fresh => "FRESH",
            Self::Stale => "STALE",
            Self::NoData => "NO DATA",
            Self::MissingScope => "SCOPE",
            Self::RateLimited => "429",
            Self::Error => "ERROR",
            Self::Unsupported => "N/A",
        }
    }

    #[must_use]
    pub const fn tone(self) -> Tone {
        match self {
            Self::Fresh => Tone::Positive,
            Self::Stale | Self::RateLimited => Tone::Warning,
            Self::NoData | Self::Unsupported => Tone::Muted,
            Self::MissingScope => Tone::Info,
            Self::Error => Tone::Danger,
        }
    }
}

#[must_use]
pub fn panel_block<'a>(
    theme: &Theme,
    title: &str,
    status: &str,
    status_tone: Tone,
    focused: bool,
    expanded: bool,
) -> Block<'a> {
    let border_tone = if focused { Tone::Focus } else { status_tone };
    let marker = if focused { ">" } else { " " };
    let expand = if expanded { " [OPEN]" } else { "" };
    Block::default()
        .title(Line::from(vec![
            Span::styled(
                format!("{marker} {}", title.to_ascii_uppercase()),
                theme.section_title(if focused { Tone::Focus } else { Tone::Default }),
            ),
            Span::raw(" "),
            Span::styled(format!("[{status}]"), theme.badge(status_tone)),
            Span::styled(expand.to_owned(), theme.badge(Tone::Focus)),
        ]))
        .borders(Borders::ALL)
        .border_style(if focused {
            theme.border(Tone::Focus)
        } else if matches!(status_tone, Tone::Muted) {
            theme.muted_border()
        } else {
            theme.border(border_tone)
        })
        .style(theme.body())
}

#[must_use]
pub fn spark_strip(values: &[u64], width: usize) -> String {
    let levels = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    if values.is_empty() || width == 0 {
        return "····".to_owned();
    }
    let max = values.iter().copied().max().unwrap_or(0);
    if max == 0 {
        return "─".repeat(width.min(values.len()));
    }
    resample(values, width)
        .into_iter()
        .map(|value| {
            let index = ((value.saturating_mul((levels.len() - 1) as u64)) + (max / 2)) / max;
            let index = usize::try_from(index).unwrap_or_else(|_| levels.len().saturating_sub(1));
            levels[index]
        })
        .collect()
}

#[must_use]
pub fn micro_histogram(values: &[u64], width: usize) -> String {
    let levels = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    if values.is_empty() || width == 0 {
        return "no bars".to_owned();
    }
    let max = values.iter().copied().max().unwrap_or(0);
    if max == 0 {
        return "·".repeat(width.min(values.len()));
    }
    resample(values, width)
        .into_iter()
        .map(|value| {
            let index = ((value.saturating_mul((levels.len() - 1) as u64)) + (max / 2)) / max;
            let index = usize::try_from(index).unwrap_or_else(|_| levels.len().saturating_sub(1));
            levels[index]
        })
        .collect()
}

#[must_use]
pub fn segmented_bar(fill_percent: u16, segments: usize) -> String {
    if segments == 0 {
        return String::new();
    }
    let filled = usize::from(fill_percent.min(100)).saturating_mul(segments) / 100;
    (0..segments)
        .map(|index| if index < filled { '█' } else { '░' })
        .collect()
}

#[must_use]
pub fn score_ring_lines(
    value: &str,
    fill_percent: u16,
    delta: Option<&str>,
    trend: &str,
    subtitle: Option<&str>,
) -> Vec<String> {
    let ring = segmented_bar(fill_percent, 10);
    let glyphs = ring.chars().collect::<Vec<_>>();
    let left = glyphs.iter().take(5).collect::<String>();
    let right = glyphs.iter().skip(5).collect::<String>();
    let delta_text = delta.unwrap_or("Δ --");
    vec![
        format!("    ╭{}╮", left),
        format!("  ╭{}  {}╮", left, right),
        format!(" │   {:^8} │", value),
        format!(" │ {:^12} │", delta_text),
        format!("  ╰{}  {}╯", right, left),
        format!("    ╰{}╯", right),
        subtitle.map_or_else(
            || trend.to_owned(),
            |subtitle| format!("{subtitle} | {trend}"),
        ),
    ]
}

#[must_use]
pub fn thermometer_lines(value: Option<f64>, label: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let normalized = value.unwrap_or(0.0).clamp(-1.0, 1.0);
    let fill = usize::from(rounded_clamped_f64_to_u16(
        normalized.midpoint(1.0) * 6.0,
        0.0,
        6.0,
    ));
    lines.push("   ╭─╮".to_owned());
    for index in (0..6).rev() {
        let glyph = if index < fill { "█" } else { " " };
        lines.push(format!("   │{glyph}│"));
    }
    lines.push("   ╰█╯".to_owned());
    lines.push(format!("{label:>7}"));
    lines
}

#[must_use]
pub fn weekly_heatmap_rows(
    day_labels: &[String],
    row_labels: &[&str],
    rows: &[Vec<Option<u8>>],
    selected: Option<(usize, usize)>,
) -> Vec<String> {
    let levels = ['·', '░', '▒', '▓', '█'];
    let mut output = Vec::new();
    output.push(format!(
        "      {}",
        day_labels
            .iter()
            .map(|day| day.chars().next().unwrap_or('?').to_string())
            .collect::<Vec<_>>()
            .join(" ")
    ));
    for (row_index, label) in row_labels.iter().enumerate() {
        let cells = rows
            .get(row_index)
            .map(|values| {
                values
                    .iter()
                    .enumerate()
                    .map(|(column_index, value)| {
                        let glyph = value.map_or('·', |score| {
                            let band = usize::from(score.min(100)) * (levels.len() - 1) / 100;
                            levels[band]
                        });
                        if selected == Some((row_index, column_index)) {
                            format!("[{glyph}]")
                        } else {
                            format!(" {glyph} ")
                        }
                    })
                    .collect::<String>()
            })
            .unwrap_or_default();
        output.push(format!("{label:<5} {cells}"));
    }
    output
}

#[must_use]
pub fn coverage_rows(items: &[(&str, TelemetryAvailability)]) -> Vec<String> {
    items
        .iter()
        .map(|(label, state)| format!("{label:<10} [{}]", state.label()))
        .collect()
}

#[must_use]
pub fn footer_inspector(
    label: &str,
    exact: &str,
    delta: &str,
    freshness: &str,
    hint: &str,
) -> String {
    format!("{label}: {exact} | {delta} | {freshness} | {hint}")
}

fn resample(values: &[u64], width: usize) -> Vec<u64> {
    if values.len() <= width {
        return values.to_vec();
    }
    (0..width)
        .map(|index| {
            let start = index.saturating_mul(values.len()) / width;
            let mut end = (index + 1)
                .saturating_mul(values.len())
                .saturating_add(width.saturating_sub(1))
                / width;
            end = end.max(start + 1).min(values.len());
            let slice = &values[start..end];
            slice.iter().copied().sum::<u64>() / u64::try_from(slice.len()).unwrap_or(1)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{TelemetryAvailability, micro_histogram, segmented_bar, spark_strip};

    #[test]
    fn segmented_bar_keeps_density_when_partially_filled() {
        assert_eq!(segmented_bar(50, 8), "████░░░░");
    }

    #[test]
    fn spark_strip_uses_terminal_safe_blocks() {
        let strip = spark_strip(&[1, 2, 3, 4, 5, 6, 7, 8], 8);
        assert_eq!(strip.chars().count(), 8);
    }

    #[test]
    fn histogram_returns_placeholder_for_empty_data() {
        assert_eq!(micro_histogram(&[], 6), "no bars");
    }

    #[test]
    fn availability_labels_are_explicit() {
        assert_eq!(TelemetryAvailability::MissingScope.label(), "SCOPE");
        assert_eq!(TelemetryAvailability::RateLimited.label(), "429");
    }
}
