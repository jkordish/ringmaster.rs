use std::fmt::Write as _;

use crate::ui::theme::Tone;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeeklyHeatmapMode {
    Standard,
    DenseHistory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeeklyHeatmapLayout {
    pub label_column_width: usize,
    pub header_height: usize,
    pub grid_origin_x: usize,
    pub cell_width: usize,
    pub row_height: usize,
    pub slot_width: usize,
    pub summary_origin_x: usize,
    pub legend_origin_x: usize,
}

impl WeeklyHeatmapLayout {
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub const fn selected_bracket_origin(self, column_index: usize) -> usize {
        self.grid_origin_x + column_index.saturating_mul(self.slot_width)
    }
}

#[must_use]
pub fn heatmap_day_label(mode: WeeklyHeatmapMode, day: &str) -> String {
    match mode {
        WeeklyHeatmapMode::Standard => day.rsplit_once('-').map_or_else(
            || day.chars().next().unwrap_or('?').to_string(),
            |(_, day_of_month)| day_of_month.to_owned(),
        ),
        WeeklyHeatmapMode::DenseHistory => day.rsplit_once('-').map_or_else(
            || day.chars().take(2).collect::<String>(),
            |(prefix, day_of_month)| {
                let month = prefix.rsplit_once('-').map_or(prefix, |(_, month)| month);
                let month_digit = month
                    .trim_start_matches('0')
                    .chars()
                    .next()
                    .unwrap_or_else(|| month.chars().last().unwrap_or('?'));
                format!("{month_digit}{day_of_month}")
            },
        ),
    }
}

#[must_use]
pub fn fit_heatmap_label(label: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let preferred = match (label, width) {
        ("Readiness", 1..=6) => "Ready",
        ("Activity", 1..=6) => "Actv",
        _ => label,
    };

    concise_text(preferred, width)
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
    let levels = ['·', '░', '▒', '▓', '█'];
    let layout = weekly_heatmap_layout(
        mode,
        day_labels.len(),
        row_labels.len(),
        layout_width_for_rows(mode, day_labels.len(), cell_width.max(1)),
        row_height
            .max(1)
            .saturating_mul(row_labels.len())
            .saturating_add(1),
    );
    let header_prefix = " ".repeat(layout.grid_origin_x);
    let mut output = Vec::new();
    let mut header_cells = String::new();
    for day in day_labels {
        let label = heatmap_day_label(mode, day);
        let _ = write!(
            header_cells,
            "{label:^slot_width$}",
            slot_width = layout.slot_width
        );
    }
    output.push(format!("{header_prefix}{header_cells}"));
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
                        let fill = glyph.to_string().repeat(layout.cell_width);
                        if selected == Some((row_index, column_index)) {
                            format!("[{fill}]")
                        } else {
                            format!(" {fill} ")
                        }
                    })
                    .collect::<String>()
            })
            .unwrap_or_default();
        output.push(format!(
            "{label:<label_width$}{cells}",
            label = fit_heatmap_label(label, layout.label_column_width),
            label_width = layout.label_column_width,
        ));
        for _ in 1..layout.row_height {
            output.push(format!(
                "{:label_width$}{cells}",
                "",
                label_width = layout.label_column_width
            ));
        }
    }
    output
}

#[must_use]
pub fn weekly_heatmap_layout(
    mode: WeeklyHeatmapMode,
    day_count: usize,
    row_count: usize,
    available_width: usize,
    available_height: usize,
) -> WeeklyHeatmapLayout {
    let label_column_width = match mode {
        WeeklyHeatmapMode::Standard => 6,
        WeeklyHeatmapMode::DenseHistory => 4,
    };
    let grid_origin_x = label_column_width;
    let usable_width = available_width.saturating_sub(label_column_width).max(3);
    let column_count = day_count.max(1);
    let cell_width = match mode {
        WeeklyHeatmapMode::DenseHistory => 1,
        WeeklyHeatmapMode::Standard => usable_width
            .checked_div(column_count)
            .unwrap_or(3)
            .saturating_sub(2)
            .clamp(1, 6),
    };
    let slot_width = cell_width.saturating_add(2);
    let header_height = 1;
    let body_rows = available_height.saturating_sub(header_height + 2);
    let row_height = body_rows
        .checked_div(row_count.max(1))
        .unwrap_or(1)
        .clamp(1, 2);

    WeeklyHeatmapLayout {
        label_column_width,
        header_height,
        grid_origin_x,
        cell_width,
        row_height,
        slot_width,
        summary_origin_x: grid_origin_x,
        legend_origin_x: grid_origin_x,
    }
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
    format!("{label} | {summary} | {freshness} | {hint}")
}

#[must_use]
pub fn concise_text(value: &str, width: usize) -> String {
    value.chars().take(width.max(1)).collect()
}

#[must_use]
pub fn concise_detail(note: &str, width: usize) -> String {
    concise_text(note.trim_end_matches('.'), width)
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
        fit_heatmap_label, footer_inspector, meter_bar, metric_panel_scaffold, micro_histogram,
        segmented_bar, spark_strip, stacked_profile_rows, weekly_heatmap_layout,
        weekly_heatmap_rows,
    };

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
        assert!(rows[1].contains("Sleep"));
        assert!(rows[1].contains('['));
        assert!(rows[1].contains(']'));
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

        assert!(rows[0].contains("401"));
        assert!(rows[0].contains("402"));
        assert!(rows[1].contains("[▒]") || rows[1].contains("[▓]"));
        assert_eq!(rows.len(), 2);
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
        let layout = weekly_heatmap_layout(WeeklyHeatmapMode::Standard, 7, 4, 40, 10);

        assert_eq!(layout.grid_origin_x, layout.label_column_width);
        assert_eq!(layout.summary_origin_x, layout.grid_origin_x);
        assert_eq!(layout.legend_origin_x, layout.grid_origin_x);
        assert_eq!(
            layout.selected_bracket_origin(6),
            layout.grid_origin_x + (layout.slot_width * 6)
        );
    }

    #[test]
    fn heatmap_labels_abbreviate_before_stealing_grid_width() {
        assert_eq!(fit_heatmap_label("Readiness", 6), "Ready");
        assert_eq!(fit_heatmap_label("Activity", 4), "Actv");
        assert_eq!(fit_heatmap_label("Sleep", 6), "Sleep");
    }
}
