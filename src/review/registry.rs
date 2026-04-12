use serde::Serialize;

use crate::oura::models::CapabilityKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ReviewSurface {
    Today,
    Week,
    Investigate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SignalGranularity {
    Day,
    Timeseries,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SignalDirectionality {
    HigherBetter,
    LowerBetter,
    Neutral,
    Contextual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EvidenceKind {
    Direct,
    Contextual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum WeeklyAggregation {
    Mean,
    Sum,
    Latest,
    Count,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ReviewFocus {
    Readiness,
    Sleep,
    Recovery,
    Stress,
    Activity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalDefinition {
    pub key: &'static str,
    pub label: &'static str,
    pub family: &'static str,
    pub granularity: SignalGranularity,
    pub baseline_window_days: usize,
    pub directionality: SignalDirectionality,
    pub required_capability: CapabilityKind,
    pub wording_constraint: &'static str,
    pub suitable_surfaces: &'static [ReviewSurface],
    pub evidence_kind: EvidenceKind,
    pub weekly_aggregation: WeeklyAggregation,
}

const SURFACES_ALL: &[ReviewSurface] = &[
    ReviewSurface::Today,
    ReviewSurface::Week,
    ReviewSurface::Investigate,
];
const SURFACES_DAILY: &[ReviewSurface] = &[ReviewSurface::Today, ReviewSurface::Investigate];
const SURFACES_WEEKLY: &[ReviewSurface] = &[ReviewSurface::Week, ReviewSurface::Investigate];
const SURFACES_CONTEXT: &[ReviewSurface] = &[
    ReviewSurface::Today,
    ReviewSurface::Week,
    ReviewSurface::Investigate,
];

const SIGNALS: [SignalDefinition; 19] = [
    SignalDefinition {
        key: "sleep_duration",
        label: "Sleep duration",
        family: "daily_sleep",
        granularity: SignalGranularity::Day,
        baseline_window_days: 30,
        directionality: SignalDirectionality::HigherBetter,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Use public-health sleep guidance plus baseline context without diagnosis language.",
        suitable_surfaces: SURFACES_ALL,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Mean,
    },
    SignalDefinition {
        key: "sleep_score",
        label: "Sleep score",
        family: "daily_sleep",
        granularity: SignalGranularity::Day,
        baseline_window_days: 30,
        directionality: SignalDirectionality::HigherBetter,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Describe sleep as above or below baseline without prescribing action.",
        suitable_surfaces: SURFACES_ALL,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Mean,
    },
    SignalDefinition {
        key: "readiness_score",
        label: "Readiness score",
        family: "daily_readiness",
        granularity: SignalGranularity::Day,
        baseline_window_days: 30,
        directionality: SignalDirectionality::HigherBetter,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Describe readiness as baseline-relative and non-diagnostic.",
        suitable_surfaces: SURFACES_ALL,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Mean,
    },
    SignalDefinition {
        key: "activity_score",
        label: "Activity score",
        family: "daily_activity",
        granularity: SignalGranularity::Day,
        baseline_window_days: 30,
        directionality: SignalDirectionality::HigherBetter,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Describe activity as higher or lower than baseline without coaching.",
        suitable_surfaces: SURFACES_ALL,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Mean,
    },
    SignalDefinition {
        key: "active_calories",
        label: "Active calories",
        family: "daily_activity",
        granularity: SignalGranularity::Day,
        baseline_window_days: 21,
        directionality: SignalDirectionality::HigherBetter,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Use as supporting movement evidence rather than value judgment.",
        suitable_surfaces: SURFACES_ALL,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Sum,
    },
    SignalDefinition {
        key: "steps",
        label: "Steps",
        family: "daily_activity",
        granularity: SignalGranularity::Day,
        baseline_window_days: 21,
        directionality: SignalDirectionality::HigherBetter,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Use steps as movement context and avoid prescriptive wording.",
        suitable_surfaces: SURFACES_ALL,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Sum,
    },
    SignalDefinition {
        key: "weekly_activity_minutes",
        label: "Weekly activity totals",
        family: "workout",
        granularity: SignalGranularity::Day,
        baseline_window_days: 28,
        directionality: SignalDirectionality::HigherBetter,
        required_capability: CapabilityKind::Workout,
        wording_constraint: "Describe workout minutes against general adult guidance and recent baseline without treatment language.",
        suitable_surfaces: SURFACES_WEEKLY,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Sum,
    },
    SignalDefinition {
        key: "weekly_activity_distribution",
        label: "Weekly activity distribution",
        family: "workout",
        granularity: SignalGranularity::Day,
        baseline_window_days: 28,
        directionality: SignalDirectionality::HigherBetter,
        required_capability: CapabilityKind::Workout,
        wording_constraint: "Describe how activity is spread across the week without prescribing a plan.",
        suitable_surfaces: SURFACES_WEEKLY,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Count,
    },
    SignalDefinition {
        key: "temperature_deviation",
        label: "Temperature deviation",
        family: "daily_readiness",
        granularity: SignalGranularity::Day,
        baseline_window_days: 30,
        directionality: SignalDirectionality::Contextual,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Describe deviation magnitude only; avoid medical interpretation.",
        suitable_surfaces: SURFACES_DAILY,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Mean,
    },
    SignalDefinition {
        key: "stress_high",
        label: "Stress high time",
        family: "daily_stress",
        granularity: SignalGranularity::Day,
        baseline_window_days: 21,
        directionality: SignalDirectionality::LowerBetter,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Frame stress as elevated or reduced time, never as diagnosis.",
        suitable_surfaces: SURFACES_ALL,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Mean,
    },
    SignalDefinition {
        key: "recovery_high",
        label: "Recovery high time",
        family: "daily_stress",
        granularity: SignalGranularity::Day,
        baseline_window_days: 21,
        directionality: SignalDirectionality::HigherBetter,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Describe recovery as more or less time than usual.",
        suitable_surfaces: SURFACES_ALL,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Mean,
    },
    SignalDefinition {
        key: "resilience_level",
        label: "Resilience level",
        family: "daily_resilience",
        granularity: SignalGranularity::Day,
        baseline_window_days: 30,
        directionality: SignalDirectionality::HigherBetter,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Describe resilience level as trend context, not a health judgment.",
        suitable_surfaces: SURFACES_ALL,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Mean,
    },
    SignalDefinition {
        key: "sleep_recovery",
        label: "Sleep recovery contributor",
        family: "daily_resilience",
        granularity: SignalGranularity::Day,
        baseline_window_days: 30,
        directionality: SignalDirectionality::HigherBetter,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Use contributor movement as supporting evidence only.",
        suitable_surfaces: SURFACES_WEEKLY,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Mean,
    },
    SignalDefinition {
        key: "daytime_recovery",
        label: "Daytime recovery contributor",
        family: "daily_resilience",
        granularity: SignalGranularity::Day,
        baseline_window_days: 30,
        directionality: SignalDirectionality::HigherBetter,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Use contributor movement as supporting evidence only.",
        suitable_surfaces: SURFACES_WEEKLY,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Mean,
    },
    SignalDefinition {
        key: "resilience_stress",
        label: "Resilience stress contributor",
        family: "daily_resilience",
        granularity: SignalGranularity::Day,
        baseline_window_days: 30,
        directionality: SignalDirectionality::LowerBetter,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Describe as stress load movement, not cause or diagnosis.",
        suitable_surfaces: SURFACES_WEEKLY,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Mean,
    },
    SignalDefinition {
        key: "cardiovascular_age",
        label: "Cardiovascular age",
        family: "daily_cardiovascular_age",
        granularity: SignalGranularity::Day,
        baseline_window_days: 30,
        directionality: SignalDirectionality::LowerBetter,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Describe as relative movement only; never interpret clinically.",
        suitable_surfaces: SURFACES_WEEKLY,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Latest,
    },
    SignalDefinition {
        key: "vo2_max",
        label: "VO2 max",
        family: "vo2_max",
        granularity: SignalGranularity::Timeseries,
        baseline_window_days: 30,
        directionality: SignalDirectionality::HigherBetter,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Describe as estimated fitness movement only.",
        suitable_surfaces: SURFACES_WEEKLY,
        evidence_kind: EvidenceKind::Direct,
        weekly_aggregation: WeeklyAggregation::Latest,
    },
    SignalDefinition {
        key: "sleep_time_status",
        label: "Sleep timing status",
        family: "sleep_time",
        granularity: SignalGranularity::Day,
        baseline_window_days: 14,
        directionality: SignalDirectionality::Contextual,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Use only as context about timing guidance.",
        suitable_surfaces: SURFACES_CONTEXT,
        evidence_kind: EvidenceKind::Contextual,
        weekly_aggregation: WeeklyAggregation::Count,
    },
    SignalDefinition {
        key: "rest_mode_active",
        label: "Rest mode",
        family: "rest_mode_period",
        granularity: SignalGranularity::Day,
        baseline_window_days: 14,
        directionality: SignalDirectionality::Contextual,
        required_capability: CapabilityKind::Daily,
        wording_constraint: "Use only as contextual support and uncertainty.",
        suitable_surfaces: SURFACES_CONTEXT,
        evidence_kind: EvidenceKind::Contextual,
        weekly_aggregation: WeeklyAggregation::Count,
    },
];

impl ReviewFocus {
    pub const ALL: [Self; 5] = [
        Self::Readiness,
        Self::Sleep,
        Self::Recovery,
        Self::Stress,
        Self::Activity,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Readiness => "readiness",
            Self::Sleep => "sleep",
            Self::Recovery => "recovery",
            Self::Stress => "stress",
            Self::Activity => "activity",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Readiness => "Readiness",
            Self::Sleep => "Sleep",
            Self::Recovery => "Recovery",
            Self::Stress => "Stress",
            Self::Activity => "Activity",
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Readiness => Self::Sleep,
            Self::Sleep => Self::Recovery,
            Self::Recovery => Self::Stress,
            Self::Stress => Self::Activity,
            Self::Activity => Self::Readiness,
        }
    }

    #[must_use]
    pub const fn primary_signal_keys(self) -> &'static [&'static str] {
        match self {
            Self::Readiness => &[
                "readiness_score",
                "sleep_score",
                "temperature_deviation",
                "stress_high",
                "rest_mode_active",
            ],
            Self::Sleep => &[
                "sleep_duration",
                "sleep_score",
                "sleep_time_status",
                "readiness_score",
                "stress_high",
            ],
            Self::Recovery => &[
                "recovery_high",
                "daytime_recovery",
                "sleep_recovery",
                "resilience_level",
                "rest_mode_active",
            ],
            Self::Stress => &[
                "stress_high",
                "recovery_high",
                "resilience_stress",
                "rest_mode_active",
            ],
            Self::Activity => &[
                "weekly_activity_minutes",
                "weekly_activity_distribution",
                "activity_score",
                "steps",
                "active_calories",
                "readiness_score",
                "vo2_max",
            ],
        }
    }
}

#[must_use]
pub const fn signal_definitions() -> &'static [SignalDefinition] {
    &SIGNALS
}

#[must_use]
pub fn signal_definition(key: &str) -> Option<&'static SignalDefinition> {
    SIGNALS.iter().find(|definition| definition.key == key)
}

#[cfg(test)]
mod tests {
    use super::{EvidenceKind, ReviewFocus, ReviewSurface, signal_definition, signal_definitions};

    #[test]
    fn registry_contains_expected_high_signal_keys() {
        let keys = signal_definitions()
            .iter()
            .map(|definition| definition.key)
            .collect::<Vec<_>>();

        for key in [
            "sleep_duration",
            "sleep_score",
            "readiness_score",
            "stress_high",
            "resilience_level",
            "cardiovascular_age",
            "vo2_max",
            "weekly_activity_minutes",
            "weekly_activity_distribution",
            "sleep_time_status",
            "rest_mode_active",
        ] {
            assert!(keys.contains(&key), "missing review signal {key}");
        }
    }

    #[test]
    fn focus_primary_signals_stay_bounded_and_deterministic() {
        assert_eq!(
            ReviewFocus::Readiness.primary_signal_keys(),
            &[
                "readiness_score",
                "sleep_score",
                "temperature_deviation",
                "stress_high",
                "rest_mode_active",
            ]
        );
        assert!(
            ReviewFocus::Sleep
                .primary_signal_keys()
                .contains(&"sleep_duration")
        );
        assert!(
            ReviewFocus::Activity
                .primary_signal_keys()
                .contains(&"weekly_activity_minutes")
        );
    }

    #[test]
    fn contextual_signals_do_not_claim_direct_evidence() {
        let definition = signal_definition("sleep_time_status")
            .unwrap_or_else(|| panic!("sleep_time_status must be registered"));
        assert_eq!(definition.evidence_kind, EvidenceKind::Contextual);
        assert!(definition.suitable_surfaces.contains(&ReviewSurface::Today));
    }
}
