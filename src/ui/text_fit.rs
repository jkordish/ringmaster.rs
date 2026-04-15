#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredText {
    pub normalized: String,
    pub width: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextFit {
    pub text: String,
    pub width: usize,
    pub truncated: bool,
}

#[must_use]
pub fn measure_one_line(value: &str) -> MeasuredText {
    let normalized = normalized_text(value);
    let width = normalized.chars().count();
    MeasuredText { normalized, width }
}

#[must_use]
pub fn fit_single_line(value: &str, width: usize) -> TextFit {
    fit_single_line_with(value, width, &[])
}

#[must_use]
pub fn fit_single_line_with(value: &str, width: usize, compact_fallbacks: &[&str]) -> TextFit {
    if width == 0 {
        return TextFit {
            text: String::new(),
            width: 0,
            truncated: !value.trim().is_empty(),
        };
    }

    let measured = measure_one_line(value);
    if measured.width <= width {
        return TextFit {
            text: measured.normalized,
            width: measured.width,
            truncated: false,
        };
    }

    for fallback in compact_fallbacks {
        let measured_fallback = measure_one_line(fallback);
        if measured_fallback.width <= width {
            return TextFit {
                text: measured_fallback.normalized,
                width: measured_fallback.width,
                truncated: true,
            };
        }
    }

    let text = truncate_with_ellipsis(&measured.normalized, width);
    let width = text.chars().count();
    TextFit {
        text,
        width,
        truncated: true,
    }
}

#[must_use]
pub fn truncate_plain(value: &str, width: usize) -> String {
    value.chars().take(width.max(1)).collect()
}

#[must_use]
pub fn truncate_with_ellipsis(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let measured = measure_one_line(value);
    if measured.width <= width {
        return measured.normalized;
    }

    if width <= 3 {
        return truncate_plain(&measured.normalized, width);
    }

    let prefix = truncate_plain(&measured.normalized, width.saturating_sub(3));
    format!("{prefix}...")
}

#[must_use]
pub fn concise_text(value: &str, width: usize) -> String {
    truncate_plain(value, width)
}

#[must_use]
pub fn concise_detail(note: &str, width: usize) -> String {
    let measured = measure_one_line(note.trim_end_matches('.'));
    truncate_plain(&measured.normalized, width)
}

#[must_use]
pub fn support_lane_text(note: &str, width: usize) -> String {
    fit_single_line(note.trim_end_matches('.'), width).text
}

#[must_use]
pub fn support_lane_text_with(note: &str, width: usize, compact_fallbacks: &[&str]) -> String {
    fit_single_line_with(note.trim_end_matches('.'), width, compact_fallbacks).text
}

#[must_use]
pub fn canonical_weekly_group_label(label: &str) -> &str {
    match label {
        "Sleep" => "Sleep",
        "Readiness" => "Ready",
        "Activity" => "Actv",
        _ => label,
    }
}

#[must_use]
pub fn canonical_breakdown_label(label: &str) -> Option<&'static str> {
    match label {
        "HRV Balance" => Some("HRV Bal"),
        "Resting HR" => Some("Rest HR"),
        "Sleep Balance" => Some("Sleep Bal"),
        "Recovery Index" => Some("Recovery"),
        _ => None,
    }
}

#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub fn fit_heatmap_label(label: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    fit_single_line_with(
        label,
        width,
        match label {
            "Readiness" => &["Ready"][..],
            "Activity" => &["Actv"][..],
            _ => &[],
        },
    )
    .text
}

#[must_use]
pub fn fit_weekly_group_label(label: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    fit_single_line(canonical_weekly_group_label(label), width).text
}

#[must_use]
pub fn fit_breakdown_label(label: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    canonical_breakdown_label(label).map_or_else(
        || fit_single_line(label, width).text,
        |compact| fit_single_line_with(label, width, &[compact]).text,
    )
}

#[must_use]
pub fn fit_breakdown_delta(label: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let normalized = normalized_text(label);
    if normalized.chars().count() <= width {
        return normalized;
    }

    if let Some(delta) = normalized.strip_prefix("vs 7d ")
        && delta.chars().count() <= width
    {
        return delta.to_owned();
    }

    if let Some(delta) = normalized.strip_prefix("d/d ")
        && delta.chars().count() <= width
    {
        return delta.to_owned();
    }

    truncate_with_ellipsis(&normalized, width)
}

#[must_use]
pub fn fit_badge_label(label: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let normalized = normalized_text(label);
    let lookup = normalized.to_ascii_lowercase();
    fit_single_line_with(
        &normalized,
        width,
        canonical_badge_abbreviations(label, &lookup),
    )
    .text
}

#[must_use]
pub fn fit_day_header(label: &str, width: usize) -> String {
    truncate_with_ellipsis(label, width)
}

fn normalized_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn canonical_badge_abbreviations(label: &str, lookup: &str) -> &'static [&'static str] {
    match (uses_uppercase_badge(label), lookup) {
        (true, "baseline" | "baseline only") => &["BASE"],
        (false, "baseline" | "baseline only") => &["base"],
        (true, "no current sample") => &["SAMPLE"],
        (false, "no current sample") => &["sample"],
        (true, "historical only") => &["HISTORY"],
        (false, "historical only") => &["history"],
        (true, "missing scope") => &["SCOPE"],
        (false, "missing scope") => &["scope"],
        (true, "unavailable") => &["N/A"],
        (false, "unavailable") => &["n/a"],
        (true, "no data") => &["EMPTY"],
        (false, "no data") => &["empty"],
        (true, "steady") => &["STDY"],
        (false, "steady") => &["stdy"],
        _ => &[],
    }
}

fn uses_uppercase_badge(label: &str) -> bool {
    let mut saw_alpha = false;
    for ch in label.chars() {
        if ch.is_ascii_alphabetic() {
            saw_alpha = true;
            if !ch.is_ascii_uppercase() {
                return false;
            }
        }
    }
    saw_alpha
}

#[cfg(test)]
mod tests {
    use super::{
        MeasuredText, TextFit, canonical_breakdown_label, canonical_weekly_group_label,
        concise_detail, fit_badge_label, fit_breakdown_delta, fit_breakdown_label, fit_day_header,
        fit_heatmap_label, fit_single_line, fit_single_line_with, fit_weekly_group_label,
        measure_one_line, support_lane_text, support_lane_text_with, truncate_plain,
        truncate_with_ellipsis,
    };

    #[test]
    fn measured_text_normalizes_whitespace_and_counts_width() {
        assert_eq!(
            measure_one_line("Weekly\n Trends  "),
            MeasuredText {
                normalized: "Weekly Trends".to_owned(),
                width: 13,
            }
        );
    }

    #[test]
    fn single_line_fit_reports_when_compact_fallbacks_or_ellipsis_were_used() {
        assert_eq!(
            fit_single_line_with("Readiness", 5, &["Ready"]),
            TextFit {
                text: "Ready".to_owned(),
                width: 5,
                truncated: true,
            }
        );
        assert_eq!(
            fit_single_line("Readiness Breakdown", 10),
            TextFit {
                text: "Readine...".to_owned(),
                width: 10,
                truncated: true,
            }
        );
    }

    #[test]
    fn ellipsis_is_deterministic_for_long_copy() {
        assert_eq!(
            truncate_with_ellipsis("Readiness Breakdown", 10),
            "Readine..."
        );
        assert_eq!(truncate_with_ellipsis("abc", 10), "abc");
        assert_eq!(truncate_with_ellipsis("abcdef", 3), "abc");
    }

    #[test]
    fn support_lane_text_flattens_whitespace_before_truncating() {
        assert_eq!(
            support_lane_text("Focus note\nwith extra   spacing.", 16),
            "Focus note wi..."
        );
        assert_eq!(concise_detail("Ends with period.", 16), "Ends with period");
        assert_eq!(
            support_lane_text_with(
                "Apr 05-Apr 08 | Sleep 76 | 04-08",
                18,
                &["Sleep 76 | 04-08"]
            ),
            "Sleep 76 | 04-08"
        );
    }

    #[test]
    fn heatmap_and_breakdown_labels_abbreviate_before_truncating() {
        assert_eq!(fit_heatmap_label("Readiness", 6), "Ready");
        assert_eq!(fit_heatmap_label("Activity", 4), "Actv");
        assert_eq!(fit_weekly_group_label("Readiness", 8), "Ready");
        assert_eq!(fit_weekly_group_label("Activity", 8), "Actv");
        assert_eq!(fit_breakdown_label("Resting HR", 7), "Rest HR");
        assert_eq!(fit_breakdown_label("Recovery Index", 8), "Recovery");
        assert_eq!(fit_breakdown_delta("vs 7d -13.7", 5), "-13.7");
        assert_eq!(fit_breakdown_delta("d/d +1.2", 4), "+1.2");
        assert_eq!(fit_badge_label("no data", 5), "empty");
        assert_eq!(fit_badge_label("steady", 4), "stdy");
        assert_eq!(fit_badge_label("baseline", 4), "base");
        assert_eq!(fit_badge_label("unavailable", 3), "n/a");
        assert_eq!(fit_badge_label("BASELINE", 4), "BASE");
        assert_eq!(fit_badge_label("MISSING SCOPE", 5), "SCOPE");
    }

    #[test]
    fn canonical_abbreviation_maps_are_deterministic() {
        assert_eq!(canonical_weekly_group_label("Sleep"), "Sleep");
        assert_eq!(canonical_weekly_group_label("Readiness"), "Ready");
        assert_eq!(canonical_weekly_group_label("Activity"), "Actv");
        assert_eq!(canonical_breakdown_label("HRV Balance"), Some("HRV Bal"));
        assert_eq!(
            canonical_breakdown_label("Sleep Balance"),
            Some("Sleep Bal")
        );
        assert_eq!(canonical_breakdown_label("Unknown"), None);
    }

    #[test]
    fn day_headers_and_plain_truncation_stay_predictable() {
        assert_eq!(fit_day_header("401", 3), "401");
        assert_eq!(fit_day_header("401", 2), "40");
        assert_eq!(truncate_plain("abcdef", 4), "abcd");
    }
}
