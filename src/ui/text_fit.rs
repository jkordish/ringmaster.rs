#[must_use]
pub fn truncate_plain(value: &str, width: usize) -> String {
    value.chars().take(width.max(1)).collect()
}

#[must_use]
pub fn truncate_with_ellipsis(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let normalized = normalized_text(value);
    if normalized.chars().count() <= width {
        return normalized;
    }

    if width <= 3 {
        return truncate_plain(&normalized, width);
    }

    let prefix = truncate_plain(&normalized, width.saturating_sub(3));
    format!("{prefix}...")
}

#[must_use]
pub fn concise_text(value: &str, width: usize) -> String {
    truncate_plain(value, width)
}

#[must_use]
pub fn concise_detail(note: &str, width: usize) -> String {
    truncate_plain(&normalized_text(note.trim_end_matches('.')), width)
}

#[must_use]
pub fn support_lane_text(note: &str, width: usize) -> String {
    truncate_with_ellipsis(&normalized_text(note.trim_end_matches('.')), width)
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

    truncate_with_ellipsis(preferred, width)
}

#[must_use]
pub fn fit_breakdown_label(label: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let preferred = match (label, width) {
        ("HRV Balance", 1..=9) => "HRV Bal",
        ("Resting HR", 1..=9) => "Rest HR",
        ("Sleep Balance", 1..=10) => "Sleep Bal",
        ("Recovery Index", 1..=11) => "Recovery",
        _ => label,
    };

    truncate_with_ellipsis(preferred, width)
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
    let preferred = match (normalized.as_str(), width) {
        ("no data", 1..=5) => "empty",
        ("steady", 1..=5) => "stdy",
        _ => normalized.as_str(),
    };

    truncate_with_ellipsis(preferred, width)
}

#[must_use]
pub fn fit_day_header(label: &str, width: usize) -> String {
    truncate_with_ellipsis(label, width)
}

fn normalized_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::{
        concise_detail, fit_badge_label, fit_breakdown_delta, fit_breakdown_label, fit_day_header,
        fit_heatmap_label, support_lane_text, truncate_plain, truncate_with_ellipsis,
    };

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
    }

    #[test]
    fn heatmap_and_breakdown_labels_abbreviate_before_truncating() {
        assert_eq!(fit_heatmap_label("Readiness", 6), "Ready");
        assert_eq!(fit_heatmap_label("Activity", 4), "Actv");
        assert_eq!(fit_breakdown_label("Resting HR", 7), "Rest HR");
        assert_eq!(fit_breakdown_label("Recovery Index", 8), "Recovery");
        assert_eq!(fit_breakdown_delta("vs 7d -13.7", 5), "-13.7");
        assert_eq!(fit_breakdown_delta("d/d +1.2", 4), "+1.2");
        assert_eq!(fit_badge_label("no data", 5), "empty");
        assert_eq!(fit_badge_label("steady", 4), "stdy");
    }

    #[test]
    fn day_headers_and_plain_truncation_stay_predictable() {
        assert_eq!(fit_day_header("401", 3), "401");
        assert_eq!(fit_day_header("401", 2), "40");
        assert_eq!(truncate_plain("abcdef", 4), "abcd");
    }
}
