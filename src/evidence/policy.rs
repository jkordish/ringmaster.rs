use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::evidence::registry::{
    CautionFlag, EvidenceTier, PopulationProfile, PopulationSupportStatus,
    ProhibitedWordingCategory, evidence_entry, resolve_evidence_descriptor,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClaimLanguageSpec {
    pub claim_key: String,
    pub tier_label: String,
    pub guidance_label: Option<String>,
    pub interpretation_label: String,
    pub active_population_profile: PopulationProfile,
    pub population_support_status: PopulationSupportStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_population_profile: Option<PopulationProfile>,
    pub caution_labels: Vec<String>,
    pub disclaimer_lines: Vec<String>,
    pub allowed_template_lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PolicyViolation {
    pub claim_key: String,
    pub category: ProhibitedWordingCategory,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GuidanceComparison {
    pub summary: String,
    pub anchor_label: Option<String>,
    pub interpretation_label: String,
    pub population_support_status: PopulationSupportStatus,
}

#[must_use]
pub fn claim_language_spec(
    claim_key: &str,
    active_population: PopulationProfile,
) -> Option<ClaimLanguageSpec> {
    let entry = evidence_entry(claim_key)?;
    let descriptor = resolve_evidence_descriptor(claim_key, active_population)?;
    let mut caution_labels = Vec::new();
    caution_labels.push(
        descriptor
            .population_support_status
            .badge_label()
            .to_owned(),
    );
    caution_labels.extend(
        entry
            .caution_flags
            .iter()
            .map(|flag| flag.label().to_owned()),
    );

    let mut disclaimer_lines = entry
        .caution_flags
        .iter()
        .map(|flag| flag.message().to_owned())
        .collect::<Vec<_>>();
    if let Some(population_line) = population_support_disclaimer(
        descriptor.active_population_profile,
        descriptor.population_support_status,
        descriptor.fallback_population_profile,
    ) {
        disclaimer_lines.insert(0, population_line);
    }

    let allowed_template_lines = entry
        .allowed_templates
        .iter()
        .map(|template| template.template().to_owned())
        .collect::<Vec<_>>();

    Some(ClaimLanguageSpec {
        claim_key: entry.claim_key.to_owned(),
        tier_label: entry.evidence_tier.chip_label().to_owned(),
        guidance_label: descriptor.guidance_anchor_label.clone(),
        interpretation_label: descriptor.interpretation_scope.label().to_owned(),
        active_population_profile: descriptor.active_population_profile,
        population_support_status: descriptor.population_support_status,
        fallback_population_profile: descriptor.fallback_population_profile,
        caution_labels: dedupe_preserving_order(caution_labels),
        disclaimer_lines: dedupe_preserving_order(disclaimer_lines),
        allowed_template_lines,
    })
}

pub fn append_required_disclaimers(
    claim_key: &str,
    active_population: PopulationProfile,
    lines: &mut Vec<String>,
) {
    if let Some(spec) = claim_language_spec(claim_key, active_population) {
        let mut seen = lines
            .iter()
            .map(|line| normalize(line))
            .collect::<BTreeSet<_>>();
        for disclaimer in spec.disclaimer_lines {
            if seen.insert(normalize(&disclaimer)) {
                lines.push(disclaimer);
            }
        }
    }
}

#[must_use]
pub fn guidance_comparison_text(
    claim_key: &str,
    active_population: PopulationProfile,
    observed_value: Option<f64>,
) -> Option<GuidanceComparison> {
    let descriptor = resolve_evidence_descriptor(claim_key, active_population)?;
    let value = observed_value?;
    let summary = match claim_key {
        "sleep_duration" => sleep_duration_summary(value, &descriptor),
        "weekly_activity_minutes" => weekly_activity_summary(value, &descriptor),
        "weekly_activity_distribution" => weekly_distribution_summary(value, &descriptor),
        _ => return None,
    };

    Some(GuidanceComparison {
        summary,
        anchor_label: descriptor.guidance_anchor_label.clone(),
        interpretation_label: descriptor.interpretation_scope.label().to_owned(),
        population_support_status: descriptor.population_support_status,
    })
}

#[must_use]
pub fn evidence_badges(claim_key: &str, active_population: PopulationProfile) -> Vec<String> {
    let Some(entry) = evidence_entry(claim_key) else {
        return Vec::new();
    };
    let Some(descriptor) = resolve_evidence_descriptor(claim_key, active_population) else {
        return Vec::new();
    };

    let mut badges = vec![
        entry.evidence_tier.short_label().to_owned(),
        descriptor.interpretation_scope.marker().to_owned(),
        descriptor
            .population_support_status
            .badge_label()
            .to_owned(),
    ];
    if let Some(label) = descriptor.guidance_anchor_label {
        badges.push(label);
    }
    badges.extend(
        entry
            .caution_flags
            .iter()
            .map(|flag| flag.label().to_owned()),
    );
    dedupe_preserving_order(badges)
}

#[must_use]
pub fn validate_claim_text(
    claim_key: &str,
    active_population: PopulationProfile,
    text: &str,
) -> Vec<PolicyViolation> {
    let Some(entry) = evidence_entry(claim_key) else {
        return Vec::new();
    };
    let Some(descriptor) = resolve_evidence_descriptor(claim_key, active_population) else {
        return Vec::new();
    };

    let normalized = normalize(text);
    let mut violations = Vec::new();

    for category in entry.prohibited_wording_categories {
        if prohibited_terms(*category)
            .iter()
            .any(|term| normalized.contains(term))
        {
            violations.push(PolicyViolation {
                claim_key: claim_key.to_owned(),
                category: *category,
                message: format!(
                    "claim text for `{claim_key}` uses prohibited {category:?} wording"
                ),
            });
        }
    }

    if matches!(entry.evidence_tier, EvidenceTier::Exploratory) {
        let trend_markers = [
            "exploratory",
            "trend",
            "trend-based",
            "within-person",
            "context",
        ];
        if !trend_markers
            .iter()
            .any(|marker| normalized.contains(marker))
        {
            violations.push(PolicyViolation {
                claim_key: claim_key.to_owned(),
                category: ProhibitedWordingCategory::CertaintyOverreach,
                message: format!(
                    "exploratory claim `{claim_key}` must be marked as trend-based or exploratory"
                ),
            });
        }
    }

    if entry
        .caution_flags
        .iter()
        .any(|flag| matches!(flag, CautionFlag::SensitiveMetric))
    {
        let sensitive_markers = ["not diagnostic", "not for screening", "trend", "context"];
        if !sensitive_markers
            .iter()
            .any(|marker| normalized.contains(marker))
        {
            violations.push(PolicyViolation {
                claim_key: claim_key.to_owned(),
                category: ProhibitedWordingCategory::ClinicalEquivalence,
                message: format!(
                    "sensitive claim `{claim_key}` is missing a caution about diagnosis or screening limits"
                ),
            });
        }
    }

    match descriptor.population_support_status {
        PopulationSupportStatus::PopulationSpecific => {}
        PopulationSupportStatus::GeneralAdultOnlyFallback => {
            let required_marker_groups = [
                &["general adult"][..],
                &["fallback", "falls back"][..],
                &["population-specific", "profile-specific"][..],
            ];
            if !required_marker_groups
                .iter()
                .all(|markers| contains_any_marker(&normalized, markers))
            {
                violations.push(PolicyViolation {
                    claim_key: claim_key.to_owned(),
                    category: ProhibitedWordingCategory::CertaintyOverreach,
                    message: format!(
                        "fallback claim `{claim_key}` must say it relies on general-adult guidance rather than profile-specific guidance"
                    ),
                });
            }
        }
        PopulationSupportStatus::Unavailable => {
            let required_marker_groups = [
                &["context", "context-only", "context only"][..],
                &["unavailable", "not supported"][..],
            ];
            if !required_marker_groups
                .iter()
                .all(|markers| contains_any_marker(&normalized, markers))
            {
                violations.push(PolicyViolation {
                    claim_key: claim_key.to_owned(),
                    category: ProhibitedWordingCategory::CertaintyOverreach,
                    message: format!(
                        "unsupported claim `{claim_key}` must stay context-only for the active population"
                    ),
                });
            }
        }
    }

    violations
}

fn sleep_duration_summary(
    value: f64,
    descriptor: &crate::evidence::registry::EvidenceDescriptor,
) -> String {
    let comparison = format!("Observed sleep duration is {value:.1} hours.");
    match descriptor.population_support_status {
        PopulationSupportStatus::PopulationSpecific => {
            if value >= 7.0 {
                format!(
                    "{comparison} That falls within general adult sleep guidance of 7 or more hours per night."
                )
            } else {
                format!(
                    "{comparison} That is below general adult sleep guidance of 7 or more hours per night."
                )
            }
        }
        PopulationSupportStatus::GeneralAdultOnlyFallback => format!(
            "{comparison} Profile-specific sleep guidance for {} is unavailable here, so Ringmaster falls back to general adult guidance as context rather than a matched target.",
            descriptor.active_population_profile.label()
        ),
        PopulationSupportStatus::Unavailable => format!(
            "{comparison} Profile-specific guidance for {} is unavailable here, so Ringmaster keeps this as context only.",
            descriptor.active_population_profile.label()
        ),
    }
}

fn weekly_activity_summary(
    value: f64,
    descriptor: &crate::evidence::registry::EvidenceDescriptor,
) -> String {
    let comparison =
        format!("Recorded moderate-or-higher workout time is {value:.0} minutes this week.");
    match descriptor.population_support_status {
        PopulationSupportStatus::PopulationSpecific => {
            if value >= 150.0 {
                format!(
                    "{comparison} That meets general adult activity guidance of at least 150 moderate-intensity minutes per week."
                )
            } else {
                format!(
                    "{comparison} That is below general adult activity guidance of at least 150 moderate-intensity minutes per week."
                )
            }
        }
        PopulationSupportStatus::GeneralAdultOnlyFallback => format!(
            "{comparison} Profile-specific activity guidance for {} is unavailable here, so Ringmaster falls back to general adult guidance as context rather than a matched target.",
            descriptor.active_population_profile.label()
        ),
        PopulationSupportStatus::Unavailable => format!(
            "{comparison} Profile-specific activity guidance for {} is unavailable here, so Ringmaster keeps this as context only.",
            descriptor.active_population_profile.label()
        ),
    }
}

fn weekly_distribution_summary(
    value: f64,
    descriptor: &crate::evidence::registry::EvidenceDescriptor,
) -> String {
    let comparison = format!(
        "Recorded moderate-or-higher workouts were spread across {:.0} day(s) this week.",
        value.round()
    );
    match descriptor.population_support_status {
        PopulationSupportStatus::PopulationSpecific => format!(
            "{comparison} Public-health guidance favors spreading activity across the week rather than relying on a single day."
        ),
        PopulationSupportStatus::GeneralAdultOnlyFallback => format!(
            "{comparison} Profile-specific distribution guidance for {} is unavailable here, so Ringmaster falls back to general adult guidance as context rather than a matched target.",
            descriptor.active_population_profile.label()
        ),
        PopulationSupportStatus::Unavailable => format!(
            "{comparison} Profile-specific distribution guidance for {} is unavailable here, so Ringmaster keeps this as context only.",
            descriptor.active_population_profile.label()
        ),
    }
}

fn population_support_disclaimer(
    active_population: PopulationProfile,
    population_support_status: PopulationSupportStatus,
    fallback_population_profile: Option<PopulationProfile>,
) -> Option<String> {
    match population_support_status {
        PopulationSupportStatus::PopulationSpecific => None,
        PopulationSupportStatus::GeneralAdultOnlyFallback => Some(format!(
            "Population-specific guidance for {} is unavailable here; Ringmaster falls back to {} guidance and labels it as a weaker comparison.",
            active_population.label(),
            fallback_population_profile
                .unwrap_or(PopulationProfile::GeneralAdult)
                .label()
        )),
        PopulationSupportStatus::Unavailable => Some(format!(
            "Population-specific guidance for {} is unavailable here, so Ringmaster keeps this metric in context-only mode.",
            active_population.label()
        )),
    }
}

const fn prohibited_terms(category: ProhibitedWordingCategory) -> &'static [&'static str] {
    match category {
        ProhibitedWordingCategory::DiagnosisLike => {
            &["diagnosis", "diagnose", "disorder", "disease", "condition"]
        }
        ProhibitedWordingCategory::TreatmentRecommendation => &[
            "treatment",
            "medication",
            "medications",
            "therapy",
            "should start",
            "should stop",
            "prescribe",
        ],
        ProhibitedWordingCategory::DiseaseScreening => &[
            "screen",
            "screening",
            "rule out",
            "rule in",
            "detect",
            "screening positive",
        ],
        ProhibitedWordingCategory::CausalInference => {
            &["caused", "because of", "due to", "led to", "explains why"]
        }
        ProhibitedWordingCategory::CertaintyOverreach => &[
            "definitely",
            "certainly",
            "proves",
            "confirmed",
            "clear evidence of",
        ],
        ProhibitedWordingCategory::IndividualizedThreshold => {
            &["your ideal", "your target number", "your normal range is"]
        }
        ProhibitedWordingCategory::AcuteEscalation => {
            &["seek urgent care", "emergency", "go to the er", "call 911"]
        }
        ProhibitedWordingCategory::ClinicalEquivalence => {
            &["clinical-grade", "medically equivalent", "medical-grade"]
        }
    }
}

fn dedupe_preserving_order(lines: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for line in lines {
        if seen.insert(line.clone()) {
            deduped.push(line);
        }
    }
    deduped
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn contains_any_marker(normalized: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| normalized.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::{
        PopulationProfile, PopulationSupportStatus, append_required_disclaimers,
        claim_language_spec, validate_claim_text,
    };

    #[test]
    fn fallback_profiles_surface_population_disclaimer() {
        let spec = claim_language_spec("sleep_duration", PopulationProfile::ShiftWorker)
            .expect("sleep duration spec should exist");
        assert_eq!(
            spec.population_support_status,
            PopulationSupportStatus::GeneralAdultOnlyFallback
        );
        assert!(
            spec.disclaimer_lines
                .iter()
                .any(|line| line.contains("falls back to General adult guidance"))
        );
    }

    #[test]
    fn unsupported_sensitive_population_requires_context_only_language() {
        let violations = validate_claim_text(
            "spo2",
            PopulationProfile::OlderAdult,
            "SpO2 looked fine today.",
        );
        assert!(!violations.is_empty());
    }

    #[test]
    fn fallback_claim_requires_general_adult_and_fallback_language() {
        let violations = validate_claim_text(
            "sleep_duration",
            PopulationProfile::ShiftWorker,
            "Population-specific guidance is unavailable here, but this still matches the target.",
        );
        assert!(!violations.is_empty());
    }

    #[test]
    fn unavailable_claim_requires_explicit_unavailable_language() {
        let violations = validate_claim_text(
            "spo2",
            PopulationProfile::OlderAdult,
            "This metric should stay in context only for today.",
        );
        assert!(!violations.is_empty());
    }

    #[test]
    fn disclaimer_append_dedupes_population_lines() {
        let mut lines = vec![
            "Population-specific guidance for Older adult is unavailable here, so Ringmaster keeps this metric in context-only mode.".to_owned(),
        ];
        append_required_disclaimers("spo2", PopulationProfile::OlderAdult, &mut lines);
        assert_eq!(lines.len(), 6);
    }
}
