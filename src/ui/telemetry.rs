use std::fmt::Write as _;

use ratatui::layout::Rect;

use super::layout::{DashboardChartMetrics, ViewportClass, WeeklyHeatmapMode, WeeklyTrendsLayout};
use crate::ui::theme::Tone;

pub use super::text_fit::concise_detail;
use super::text_fit::fit_weekly_group_label;

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
            Self::Fresh => Tone::Fresh,
            Self::Stale | Self::RateLimited => Tone::Stale,
            Self::NoData | Self::Unsupported | Self::MissingScope => Tone::Unavailable,
            Self::Error => Tone::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricPanelState {
    Fresh,
    Stale,
    NoCurrentSample,
    BaselineOnly,
    HistoricalOnly,
    MissingScope,
    Unavailable,
    Empty,
    Error,
}

impl MetricPanelState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fresh => "FRESH",
            Self::Stale => "STALE",
            Self::NoCurrentSample => "NO CURRENT SAMPLE",
            Self::BaselineOnly => "BASELINE ONLY",
            Self::HistoricalOnly => "HISTORICAL ONLY",
            Self::MissingScope => "MISSING SCOPE",
            Self::Unavailable => "UNAVAILABLE",
            Self::Empty => "EMPTY",
            Self::Error => "ERROR",
        }
    }

    #[must_use]
    pub const fn tone(self) -> Tone {
        match self {
            Self::Fresh => Tone::Fresh,
            Self::Stale | Self::NoCurrentSample => Tone::Stale,
            Self::BaselineOnly | Self::HistoricalOnly => Tone::Info,
            Self::MissingScope | Self::Unavailable | Self::Empty => Tone::Unavailable,
            Self::Error => Tone::Error,
        }
    }

    #[must_use]
    pub const fn has_current_sample(self) -> bool {
        matches!(self, Self::Fresh | Self::Stale)
    }
}

#[must_use]
pub fn spark_strip(values: &[u64], width: usize) -> String {
    let levels = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    if width == 0 {
        return String::new();
    }
    if values.is_empty() {
        return "·".repeat(width);
    }
    let max = values.iter().copied().max().unwrap_or(0);
    if max == 0 {
        return "─".repeat(width);
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
    if width == 0 {
        return String::new();
    }
    if values.is_empty() {
        return "·".repeat(width);
    }
    let max = values.iter().copied().max().unwrap_or(0);
    if max == 0 {
        return "·".repeat(width);
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
pub fn meter_bar(fill_percent: u16, width: usize) -> String {
    if width <= 2 {
        return segmented_bar(fill_percent, width.max(1));
    }
    let interior = segmented_bar(fill_percent, width - 2)
        .chars()
        .map(|glyph| if glyph == '░' { '·' } else { glyph })
        .collect::<String>();
    format!("╺{interior}╸")
}

#[must_use]
pub fn stacked_profile_rows(values: &[u64], width: usize, height: usize) -> Vec<String> {
    if values.is_empty() || width == 0 || height == 0 {
        return vec![placeholder_rule(width.max(4))];
    }
    let sampled = resample(values, width);
    let max = sampled.iter().copied().max().unwrap_or(0);
    if max == 0 {
        return vec!["·".repeat(sampled.len().max(1))];
    }

    let max = usize::try_from(max).unwrap_or(usize::MAX).max(1);
    let scaled = sampled
        .iter()
        .map(|value| {
            let value = usize::try_from(*value).unwrap_or(usize::MAX);
            value.saturating_mul(height).div_ceil(max).min(height)
        })
        .collect::<Vec<_>>();

    let mut rows = vec![String::with_capacity(sampled.len()); height];
    for (row_index, row) in rows.iter_mut().enumerate().take(height) {
        let threshold = height.saturating_sub(row_index);
        for value in &scaled {
            let glyph = if *value >= threshold {
                '█'
            } else if row_index == height.saturating_sub(1) {
                '·'
            } else {
                ' '
            };
            row.push(glyph);
        }
    }
    rows
}

#[must_use]
pub fn heatmap_day_label(mode: WeeklyHeatmapMode, day: &str) -> String {
    match mode {
        WeeklyHeatmapMode::Standard => day.rsplit_once('-').map_or_else(
            || day.chars().next().unwrap_or('?').to_string(),
            |(_, day_of_month)| day_of_month.to_owned(),
        ),
        WeeklyHeatmapMode::DenseHistory => day.rsplit_once('-').map_or_else(
            || {
                day.chars()
                    .rev()
                    .take(2)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect()
            },
            |(_, day_of_month)| day_of_month.to_owned(),
        ),
    }
}

#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub fn weekly_heatmap_rows(
    day_labels: &[String],
    row_labels: &[&str],
    rows: &[Vec<Option<u8>>],
    selected: Option<(usize, usize)>,
    mode: WeeklyHeatmapMode,
    cell_width: usize,
    row_height: usize,
) -> Vec<String> {
    let layout = WeeklyTrendsLayout::for_panel(
        Rect::new(
            0,
            0,
            u16::try_from(layout_width_for_rows(
                mode,
                day_labels.len(),
                cell_width.max(1),
            ))
            .unwrap_or(u16::MAX),
            u16::try_from(
                row_height
                    .max(1)
                    .saturating_mul(row_labels.len())
                    .saturating_add(3),
            )
            .unwrap_or(u16::MAX),
        ),
        DashboardChartMetrics::for_viewport(ViewportClass::Wide),
        mode,
        day_labels.len(),
        row_labels.len(),
    );
    let header_prefix = " ".repeat(usize::from(layout.grid_viewport.x));
    let mut output = Vec::new();
    let mut header_cells = String::new();
    for day in day_labels {
        let label = heatmap_day_label(mode, day);
        let _ = write!(
            header_cells,
            "{label:^slot_width$}",
            slot_width = usize::from(layout.slot_width)
        );
    }
    output.push(format!("{header_prefix}{header_cells}"));
    let selected_column = selected.map(|(_, column_index)| column_index);
    for (group_index, label) in row_labels.iter().enumerate() {
        let top_cells = grouped_heatmap_row(
            values_for_group(rows, group_index),
            selected_column,
            &layout,
            0,
        );
        output.push(format!(
            "{:label_width$}{top_cells}",
            "",
            label_width = usize::from(layout.label_column_width)
        ));
        if layout.subrow_gap > 0 {
            output.push(format!(
                "{label:<label_width$}{:grid_width$}",
                "",
                label = fit_weekly_group_label(label, usize::from(layout.label_column_width)),
                label_width = usize::from(layout.label_column_width),
                grid_width = usize::from(layout.grid_viewport.width),
            ));
        }
        let bottom_cells = grouped_heatmap_row(
            values_for_group(rows, group_index),
            selected_column,
            &layout,
            1,
        );
        output.push(format!(
            "{label:<label_width$}{bottom_cells}",
            label = fit_weekly_group_label(label, usize::from(layout.label_column_width)),
            label_width = usize::from(layout.label_column_width),
        ));
        if group_index + 1 < row_labels.len() && group_index < usize::from(layout.group_gap_count) {
            output.push(String::new());
        }
    }
    output
}

fn values_for_group(rows: &[Vec<Option<u8>>], group_index: usize) -> &[Option<u8>] {
    rows.get(group_index).map_or(&[], Vec::as_slice)
}

fn grouped_heatmap_row(
    values: &[Option<u8>],
    selected_column: Option<usize>,
    layout: &WeeklyTrendsLayout,
    subrow_index: usize,
) -> String {
    let column_count = usize::from(layout.grid_viewport.width / layout.slot_width.max(1));
    (0..column_count)
        .map(|column_index| {
            let value = values.get(column_index).copied().flatten();
            let glyph = match (
                subrow_index,
                value.map_or(0, |score| usize::from(score.min(100)) * 4 / 100 + 1),
            ) {
                (_, 0) => '·',
                (0, 1) => '░',
                (0, 2) => '▒',
                (0, 3) => '▓',
                (_, _) if subrow_index == 0 => '█',
                (1, 1) => '╶',
                (1, 2) => '─',
                (1, 3) => '━',
                (_, _) if subrow_index == 1 => '█',
                _ => '·',
            };
            let fill = glyph.to_string().repeat(usize::from(layout.cell_width));
            if selected_column == Some(column_index) {
                format!("[{fill}]")
            } else {
                format!(" {fill} ")
            }
        })
        .collect()
}

const fn layout_width_for_rows(
    mode: WeeklyHeatmapMode,
    day_count: usize,
    cell_width: usize,
) -> usize {
    let label_column_width = match mode {
        WeeklyHeatmapMode::Standard => 8,
        WeeklyHeatmapMode::DenseHistory => 5,
    };
    label_column_width + day_count.saturating_mul(cell_width.saturating_add(2))
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
    let summary = if delta.is_empty() || delta == "Δ --" {
        exact.to_owned()
    } else {
        format!("{exact} / {delta}")
    };
    [
        Some(label),
        (!summary.is_empty()).then_some(summary.as_str()),
        (!freshness.is_empty()).then_some(freshness),
        (!hint.is_empty()).then_some(hint),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" | ")
}

#[must_use]
pub fn placeholder_rule(width: usize) -> String {
    "·".repeat(width.max(4))
}

#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub fn availability_scaffold(
    availability: TelemetryAvailability,
    reason: &str,
    width: usize,
) -> Vec<String> {
    let width = width.max(8);
    vec![
        format!("{:^width$}", availability.label()),
        placeholder_rule(width),
        concise_detail(reason, width),
    ]
}

#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub fn metric_panel_scaffold(state: MetricPanelState, reason: &str, width: usize) -> Vec<String> {
    let width = width.max(10);
    vec![
        format!("{:^width$}", state.label()),
        placeholder_rule(width),
        concise_detail(reason, width),
    ]
}

#[must_use]
#[allow(dead_code)]
pub fn primary_secondary_line(primary: &str, secondary: &str, width: usize) -> String {
    let width = width.max(primary.len() + secondary.len() + 1);
    let gap = width.saturating_sub(primary.len() + secondary.len());
    format!("{primary}{}{secondary}", " ".repeat(gap))
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
    use super::{
        MetricPanelState, TelemetryAvailability, WeeklyHeatmapMode, availability_scaffold,
        footer_inspector, meter_bar, metric_panel_scaffold, micro_histogram, segmented_bar,
        spark_strip, stacked_profile_rows, weekly_heatmap_rows,
    };
    use crate::ui::layout::{DashboardChartMetrics, ViewportClass, WeeklyTrendsLayout};
    use crate::ui::text_fit::fit_heatmap_label;
    use ratatui::layout::Rect;

    #[test]
    fn segmented_bar_keeps_density_when_partially_filled() {
        assert_eq!(segmented_bar(50, 8), "████░░░░");
    }

    #[test]
    fn meter_bar_uses_endcaps_and_quiet_empty_cells() {
        assert_eq!(meter_bar(50, 8), "╺███···╸");
    }

    #[test]
    fn spark_strip_uses_terminal_safe_blocks() {
        let strip = spark_strip(&[1, 2, 3, 4, 5, 6, 7, 8], 8);
        assert_eq!(strip.chars().count(), 8);
    }

    #[test]
    fn spark_strip_preserves_requested_width_for_empty_and_flat_inputs() {
        assert_eq!(spark_strip(&[], 6), "······");
        assert_eq!(spark_strip(&[0, 0, 0], 6), "──────");
    }

    #[test]
    fn histogram_returns_placeholder_for_empty_data() {
        assert_eq!(micro_histogram(&[], 6), "······");
        assert_eq!(micro_histogram(&[0, 0, 0], 6), "······");
        assert_eq!(micro_histogram(&[1, 2, 3], 0), "");
    }

    #[test]
    fn stacked_profiles_gain_vertical_mass_without_extra_labels() {
        let rows = stacked_profile_rows(&[1, 2, 3, 4], 4, 3);
        assert_eq!(rows.len(), 3);
        assert!(rows[2].contains('·') || rows[2].contains('█'));
    }

    #[test]
    fn availability_labels_are_explicit() {
        assert_eq!(TelemetryAvailability::MissingScope.label(), "SCOPE");
        assert_eq!(TelemetryAvailability::RateLimited.label(), "429");
    }

    #[test]
    fn footer_inspector_front_loads_summary_and_tucks_hints_last() {
        let footer = footer_inspector(
            "Readiness tile",
            "score 74",
            "vs 7d -5.7",
            "FRESH | FRESH",
            "`Tab` next | `?` help",
        );

        assert!(footer.starts_with("Readiness tile | score 74 / vs 7d -5.7"));
        assert!(footer.ends_with("`Tab` next | `?` help"));
    }

    #[test]
    fn weekly_heatmap_rows_scale_cells_without_losing_alignment() {
        let rows = weekly_heatmap_rows(
            &["Mon".to_owned(), "Tue".to_owned()],
            &["Sleep"],
            &[vec![Some(30), Some(80)]],
            Some((0, 1)),
            WeeklyHeatmapMode::Standard,
            2,
            1,
        );

        assert!(rows[0].contains('M'));
        assert!(rows[2].contains("Sleep"));
        assert!(rows[1].contains('[') || rows[2].contains('['));
        assert!(rows[1].contains(']') || rows[2].contains(']'));
    }

    #[test]
    fn weekly_heatmap_rows_use_day_of_month_headers_for_date_labels() {
        let rows = weekly_heatmap_rows(
            &["04-05".to_owned(), "04-06".to_owned(), "04-07".to_owned()],
            &["Sleep"],
            &[vec![Some(30), Some(80), Some(55)]],
            Some((0, 1)),
            WeeklyHeatmapMode::Standard,
            2,
            1,
        );

        assert!(rows[0].contains("05"));
        assert!(rows[0].contains("06"));
        assert!(rows[0].contains("07"));
        assert!(!rows[0].contains("0   0   0"));
    }

    #[test]
    fn weekly_heatmap_dense_history_keeps_headers_compact() {
        let rows = weekly_heatmap_rows(
            &["04-01".to_owned(), "04-02".to_owned(), "04-03".to_owned()],
            &["Sleep"],
            &[vec![Some(30), Some(80), Some(55)]],
            Some((0, 2)),
            WeeklyHeatmapMode::DenseHistory,
            1,
            2,
        );

        assert!(rows[0].contains("01"));
        assert!(rows[0].contains("02"));
        assert!(rows[0].contains("03"));
        assert!(rows[1].contains("[▒]") || rows[1].contains("[▓]"));
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn availability_scaffolds_keep_missing_states_structured() {
        let lines = availability_scaffold(
            TelemetryAvailability::MissingScope,
            "daily scope missing",
            12,
        );

        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("SCOPE"));
    }

    #[test]
    fn metric_panel_states_keep_semantics_distinct() {
        assert_eq!(MetricPanelState::BaselineOnly.label(), "BASELINE ONLY");
        assert_eq!(
            MetricPanelState::NoCurrentSample.label(),
            "NO CURRENT SAMPLE"
        );
        assert_eq!(MetricPanelState::HistoricalOnly.label(), "HISTORICAL ONLY");
        assert!(MetricPanelState::Fresh.has_current_sample());
        assert!(!MetricPanelState::BaselineOnly.has_current_sample());
    }

    #[test]
    fn metric_panel_scaffold_uses_precise_state_labels() {
        let lines = metric_panel_scaffold(
            MetricPanelState::BaselineOnly,
            "Baseline remains available while the current-day sample is missing.",
            36,
        );

        assert_eq!(lines[0].trim(), "BASELINE ONLY");
        assert!(lines[2].contains("Baseline remains available"));
    }

    #[test]
    fn weekly_heatmap_layout_aligns_grid_and_legend_from_one_origin() {
        let layout = WeeklyTrendsLayout::for_panel(
            Rect::new(0, 0, 40, 10),
            DashboardChartMetrics::for_viewport(ViewportClass::Wide),
            WeeklyHeatmapMode::Standard,
            7,
            3,
        );

        assert_eq!(layout.grid_viewport.x, layout.label_column_width);
        assert_eq!(layout.summary_area.x, layout.grid_viewport.x);
        assert_eq!(layout.legend_area.x, layout.grid_viewport.x);
        assert_eq!(
            layout.selected_bracket_origin(6),
            layout.grid_viewport.x + (layout.slot_width * 6)
        );
    }

    #[test]
    fn heatmap_labels_abbreviate_before_stealing_grid_width() {
        assert_eq!(fit_heatmap_label("Readiness", 6), "Ready");
        assert_eq!(fit_heatmap_label("Activity", 4), "Actv");
        assert_eq!(fit_heatmap_label("Sleep", 6), "Sleep");
    }
}
