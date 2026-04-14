use crate::review::engine::{ReviewConfidence, ReviewMode};
use crate::review::features::ReviewSufficiency;
use crate::review::registry::{SignalDefinition, SignalDirectionality};

#[must_use]
pub fn headline_for_signal(
    definition: &SignalDefinition,
    mode: ReviewMode,
    delta: Option<f64>,
    z_score: Option<f64>,
) -> String {
    match (definition.directionality, signed_bucket(delta, z_score)) {
        (SignalDirectionality::HigherBetter, Some(1)) => {
            format!("{} is above your baseline.", definition.label)
        }
        (SignalDirectionality::HigherBetter, Some(-1)) => {
            format!("{} is below your baseline.", definition.label)
        }
        (SignalDirectionality::LowerBetter, Some(1)) => {
            format!("{} is higher than usual.", definition.label)
        }
        (SignalDirectionality::LowerBetter, Some(-1)) => {
            format!("{} is lower than usual.", definition.label)
        }
        (SignalDirectionality::Contextual, Some(1)) => {
            format!("{} is above its recent range.", definition.label)
        }
        (SignalDirectionality::Contextual, Some(-1)) => {
            format!("{} is below its recent range.", definition.label)
        }
        _ => match mode {
            ReviewMode::Today => format!("{} is worth a closer look today.", definition.label),
            ReviewMode::Week => format!("{} shifted this week.", definition.label),
        },
    }
}

#[must_use]
pub fn summary_for_signal(
    definition: &SignalDefinition,
    mode: ReviewMode,
    baseline_window_days: usize,
    persistence_days: u32,
) -> String {
    let mode_phrase = match mode {
        ReviewMode::Today => "today",
        ReviewMode::Week => "this week",
    };
    if persistence_days > 1 {
        format!(
            "{} is being shown because it stayed off your {}-day baseline for {} recent days {}.",
            definition.label, baseline_window_days, persistence_days, mode_phrase
        )
    } else {
        format!(
            "{} is being shown because it moved away from your {}-day baseline {}.",
            definition.label, baseline_window_days, mode_phrase
        )
    }
}

#[must_use]
pub fn sufficiency_line(sufficiency: ReviewSufficiency) -> String {
    match sufficiency {
        ReviewSufficiency::Missing => {
            "Evidence is limited because no comparable baseline days are available.".to_owned()
        }
        ReviewSufficiency::Thin => {
            "Evidence is limited because only a thin set of comparable days is available."
                .to_owned()
        }
        ReviewSufficiency::Medium => {
            "Evidence is based on a moderate set of comparable days.".to_owned()
        }
        ReviewSufficiency::Strong => {
            "Evidence is based on a strong set of comparable days.".to_owned()
        }
    }
}

#[must_use]
pub fn confidence_badge(confidence: ReviewConfidence, sufficiency: ReviewSufficiency) -> String {
    format!(
        "{} confidence / {} data",
        confidence.label(),
        sufficiency.label()
    )
}

#[must_use]
pub fn why_this_is_shown(
    baseline_window_days: usize,
    deviation_bucket: i32,
    persistence_bucket: i32,
    corroboration_points: i32,
) -> String {
    format!(
        "Why this is shown: deviation bucket={deviation_bucket}, persistence bucket={persistence_bucket}, corroboration={corroboration_points} against a {baseline_window_days}-day baseline."
    )
}

fn signed_bucket(delta: Option<f64>, z_score: Option<f64>) -> Option<i8> {
    let comparator = z_score.or(delta)?;
    if comparator > 0.0 {
        Some(1)
    } else if comparator < 0.0 {
        Some(-1)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::oura::models::CapabilityKind;
    use crate::review::engine::{ReviewConfidence, ReviewMode};
    use crate::review::features::ReviewSufficiency;
    use crate::review::registry::{
        EvidenceKind, ReviewSurface, SignalDefinition, SignalDirectionality, SignalGranularity,
        WeeklyAggregation,
    };

    use super::{headline_for_signal, sufficiency_line};

    fn sample_definition(directionality: SignalDirectionality) -> SignalDefinition {
        SignalDefinition {
            key: "sample",
            label: "Sample signal",
            family: "sample_family",
            granularity: SignalGranularity::Day,
            baseline_window_days: 30,
            directionality,
            required_capability: CapabilityKind::Daily,
            wording_constraint: "deterministic wording only",
            suitable_surfaces: &[ReviewSurface::Today],
            evidence_kind: EvidenceKind::Direct,
            weekly_aggregation: WeeklyAggregation::Mean,
        }
    }

    #[test]
    fn templates_avoid_ai_and_causal_language() {
        let headline = headline_for_signal(
            &sample_definition(SignalDirectionality::HigherBetter),
            ReviewMode::Today,
            Some(-5.0),
            Some(-1.2),
        );
        assert!(headline.contains("below your baseline"));
        assert!(!headline.contains("AI"));
        assert!(!headline.contains("caused"));
        assert!(!headline.contains("should"));
    }

    #[test]
    fn lower_better_templates_match_delta_direction() {
        let definition = sample_definition(SignalDirectionality::LowerBetter);

        let higher_headline =
            headline_for_signal(&definition, ReviewMode::Today, Some(5.0), Some(1.2));
        let lower_headline =
            headline_for_signal(&definition, ReviewMode::Today, Some(-5.0), Some(-1.2));

        assert!(higher_headline.contains("higher than usual"));
        assert!(lower_headline.contains("lower than usual"));
    }

    #[test]
    fn sufficiency_lines_surface_uncertainty() {
        let line = sufficiency_line(ReviewSufficiency::Thin);
        assert!(line.contains("Evidence is limited"));
        assert!(!line.contains("AI"));
        let badge = super::confidence_badge(ReviewConfidence::Medium, ReviewSufficiency::Medium);
        assert_eq!(badge, "Medium confidence / Medium data");
    }
}
