#[derive(Debug, Clone, PartialEq)]
pub struct MetricPoint {
    pub day: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BaselineStats {
    pub window_days: usize,
    pub sample_count: usize,
    pub mean: Option<f64>,
    pub standard_deviation: Option<f64>,
    pub delta_from_today: Option<f64>,
    pub z_score: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsightConfidence {
    Thin,
    Medium,
    Strong,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetricInsight {
    pub label: &'static str,
    pub today: Option<MetricPoint>,
    pub previous_day: Option<MetricPoint>,
    pub day_over_day_delta: Option<f64>,
    pub baseline_7d: BaselineStats,
    pub baseline_30d: BaselineStats,
    pub summary: String,
    pub confidence: InsightConfidence,
    pub confidence_note: Option<String>,
}

#[must_use]
pub fn build_metric_insight(label: &'static str, history: &[MetricPoint]) -> MetricInsight {
    let today = history.last().cloned();
    let previous_day = history.iter().rev().nth(1).cloned();
    let day_over_day_delta = today
        .as_ref()
        .zip(previous_day.as_ref())
        .map(|(current, previous)| current.value - previous.value);
    let baseline_7d = baseline_for_window(history, 7);
    let baseline_30d = baseline_for_window(history, 30);
    let confidence = classify_confidence(baseline_30d.sample_count.max(baseline_7d.sample_count));
    let confidence_note = confidence_note(
        confidence,
        baseline_7d.sample_count,
        baseline_30d.sample_count,
    );
    let summary = build_summary(label, today.as_ref(), &baseline_7d, &baseline_30d);

    MetricInsight {
        label,
        today,
        previous_day,
        day_over_day_delta,
        baseline_7d,
        baseline_30d,
        summary,
        confidence,
        confidence_note,
    }
}

fn baseline_for_window(history: &[MetricPoint], window_days: usize) -> BaselineStats {
    let Some(today) = history.last() else {
        return BaselineStats {
            window_days,
            sample_count: 0,
            mean: None,
            standard_deviation: None,
            delta_from_today: None,
            z_score: None,
        };
    };

    let baseline_slice = history
        .iter()
        .rev()
        .skip(1)
        .take(window_days)
        .map(|point| point.value)
        .collect::<Vec<_>>();
    let sample_count = baseline_slice.len();
    let mean = mean(&baseline_slice);
    let standard_deviation = standard_deviation(&baseline_slice, mean);
    let delta_from_today = mean.map(|baseline| today.value - baseline);
    let z_score = standard_deviation
        .zip(delta_from_today)
        .and_then(|(stdev, delta)| {
            if stdev > f64::EPSILON && sample_count >= 4 {
                Some(delta / stdev)
            } else {
                None
            }
        });

    BaselineStats {
        window_days,
        sample_count,
        mean,
        standard_deviation,
        delta_from_today,
        z_score,
    }
}

fn build_summary(
    label: &str,
    today: Option<&MetricPoint>,
    baseline_7d: &BaselineStats,
    baseline_30d: &BaselineStats,
) -> String {
    let Some(today) = today else {
        return format!("No {label} data is available yet.");
    };

    let preferred = if baseline_7d.sample_count >= 4 {
        baseline_7d
    } else {
        baseline_30d
    };

    let value = format_value(today.value);

    if preferred.sample_count < 4 {
        return format!(
            "Today's {label} is {value}; there is not enough history to compare it to your normal yet."
        );
    }

    let baseline = preferred.mean.unwrap_or(today.value);
    let baseline_value = format_value(baseline);
    let direction = match preferred.z_score {
        Some(z_score) if z_score >= 1.0 => "above",
        Some(z_score) if z_score <= -1.0 => "below",
        _ => "close to",
    };

    format!(
        "Today's {label} is {direction} your {window}d baseline ({value} vs {baseline_value}).",
        window = preferred.window_days
    )
}

const fn classify_confidence(sample_count: usize) -> InsightConfidence {
    if sample_count >= 21 {
        InsightConfidence::Strong
    } else if sample_count >= 7 {
        InsightConfidence::Medium
    } else {
        InsightConfidence::Thin
    }
}

fn confidence_note(
    confidence: InsightConfidence,
    sample_count_7d: usize,
    sample_count_30d: usize,
) -> Option<String> {
    match confidence {
        InsightConfidence::Strong => None,
        InsightConfidence::Medium => Some(format!(
            "Confidence is moderate because only {sample_count_30d} prior daily points are available."
        )),
        InsightConfidence::Thin => Some(format!(
            "Confidence is thin because only {sample_count_7d} to {sample_count_30d} prior daily points are available."
        )),
    }
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }

    Some(values.iter().sum::<f64>() / crate::numeric::usize_to_f64(values.len()))
}

fn standard_deviation(values: &[f64], mean: Option<f64>) -> Option<f64> {
    let mean = mean?;
    if values.is_empty() {
        return None;
    }

    let variance = values
        .iter()
        .map(|value| {
            let delta = value - mean;
            delta * delta
        })
        .sum::<f64>()
        / crate::numeric::usize_to_f64(values.len());

    Some(variance.sqrt())
}

fn format_value(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::{InsightConfidence, MetricPoint, build_metric_insight};

    #[test]
    fn computes_baselines_and_deltas() {
        let history = vec![
            MetricPoint {
                day: "2026-04-01".to_owned(),
                value: 80.0,
            },
            MetricPoint {
                day: "2026-04-02".to_owned(),
                value: 81.0,
            },
            MetricPoint {
                day: "2026-04-03".to_owned(),
                value: 82.0,
            },
            MetricPoint {
                day: "2026-04-04".to_owned(),
                value: 83.0,
            },
            MetricPoint {
                day: "2026-04-05".to_owned(),
                value: 84.0,
            },
            MetricPoint {
                day: "2026-04-06".to_owned(),
                value: 85.0,
            },
            MetricPoint {
                day: "2026-04-07".to_owned(),
                value: 86.0,
            },
            MetricPoint {
                day: "2026-04-08".to_owned(),
                value: 90.0,
            },
        ];

        let insight = build_metric_insight("sleep", &history);

        assert_eq!(insight.day_over_day_delta, Some(4.0));
        assert_eq!(insight.baseline_7d.sample_count, 7);
        assert!(insight.baseline_7d.mean.is_some());
        assert!(insight.baseline_7d.z_score.is_some());
        assert_eq!(insight.confidence, InsightConfidence::Medium);
    }

    #[test]
    fn reports_thin_history_honestly() {
        let history = vec![
            MetricPoint {
                day: "2026-04-07".to_owned(),
                value: 83.0,
            },
            MetricPoint {
                day: "2026-04-08".to_owned(),
                value: 84.0,
            },
        ];

        let insight = build_metric_insight("readiness", &history);

        assert_eq!(insight.baseline_7d.sample_count, 1);
        assert!(insight.baseline_7d.z_score.is_none());
        assert_eq!(insight.confidence, InsightConfidence::Thin);
        assert!(insight.summary.contains("not enough history"));
        assert!(insight.confidence_note.is_some());
    }
}
