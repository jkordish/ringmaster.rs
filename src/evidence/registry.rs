use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use time::{Date, format_description::well_known::Iso8601};

use crate::error::{Result, RingmasterError};

pub const EVIDENCE_REGISTRY_VERSION: &str = "ringmaster.evidence.v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTier {
    GuidelineBacked,
    EvidenceInformed,
    Exploratory,
}

impl EvidenceTier {
    #[must_use]
    pub const fn chip_label(self) -> &'static str {
        match self {
            Self::GuidelineBacked => "Guideline-backed",
            Self::EvidenceInformed => "Evidence-informed",
            Self::Exploratory => "Exploratory",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::GuidelineBacked => "Guideline",
            Self::EvidenceInformed => "Evidence-informed",
            Self::Exploratory => "Exploratory",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceType {
    Guideline,
    PublicHealthGuidance,
    ScientificStatement,
    PositionStatement,
    SystematicReview,
    DeviceLimitation,
    ConsumerWearableLimitation,
    ContextOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceFamily {
    ClinicalGuideline,
    PublicHealthAuthority,
    ConsensusStatement,
    ScientificStatement,
    SystematicEvidenceSynthesis,
    DeviceDocumentation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PopulationProfile {
    GeneralAdult,
    OlderAdult,
    PregnancyPostpartum,
    ShiftWorker,
    AthleteHighTrainingLoad,
}

impl PopulationProfile {
    pub const ALL: [Self; 5] = [
        Self::GeneralAdult,
        Self::OlderAdult,
        Self::PregnancyPostpartum,
        Self::ShiftWorker,
        Self::AthleteHighTrainingLoad,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GeneralAdult => "general_adult",
            Self::OlderAdult => "older_adult",
            Self::PregnancyPostpartum => "pregnancy_postpartum",
            Self::ShiftWorker => "shift_worker",
            Self::AthleteHighTrainingLoad => "athlete_high_training_load",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::GeneralAdult => "General adult",
            Self::OlderAdult => "Older adult",
            Self::PregnancyPostpartum => "Pregnancy/postpartum",
            Self::ShiftWorker => "Shift worker",
            Self::AthleteHighTrainingLoad => "Athlete with high training load",
        }
    }

    /// # Errors
    ///
    /// Returns an error when the configured population profile is not one of the supported
    /// registry identifiers.
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "general_adult" => Ok(Self::GeneralAdult),
            "older_adult" => Ok(Self::OlderAdult),
            "pregnancy_postpartum" => Ok(Self::PregnancyPostpartum),
            "shift_worker" => Ok(Self::ShiftWorker),
            "athlete_high_training_load" => Ok(Self::AthleteHighTrainingLoad),
            other => Err(RingmasterError::Config(format!(
                "guidance.active_population_profile must be one of general_adult, older_adult, pregnancy_postpartum, shift_worker, athlete_high_training_load; got `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PopulationSupportStatus {
    PopulationSpecific,
    GeneralAdultOnlyFallback,
    Unavailable,
}

impl PopulationSupportStatus {
    #[must_use]
    pub const fn badge_label(self) -> &'static str {
        match self {
            Self::PopulationSpecific => "Population-specific",
            Self::GeneralAdultOnlyFallback => "General-adult-only",
            Self::Unavailable => "Unavailable",
        }
    }

    #[must_use]
    pub const fn detail_label(self) -> &'static str {
        match self {
            Self::PopulationSpecific => "Population-specific guidance",
            Self::GeneralAdultOnlyFallback => "General-adult-only fallback",
            Self::Unavailable => "Unavailable for active profile",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PopulationSupportMatrix {
    pub general_adult: PopulationSupportStatus,
    pub older_adult: PopulationSupportStatus,
    pub pregnancy_postpartum: PopulationSupportStatus,
    pub shift_worker: PopulationSupportStatus,
    pub athlete_high_training_load: PopulationSupportStatus,
}

impl PopulationSupportMatrix {
    #[must_use]
    pub const fn new(
        general_adult: PopulationSupportStatus,
        older_adult: PopulationSupportStatus,
        pregnancy_postpartum: PopulationSupportStatus,
        shift_worker: PopulationSupportStatus,
        athlete_high_training_load: PopulationSupportStatus,
    ) -> Self {
        Self {
            general_adult,
            older_adult,
            pregnancy_postpartum,
            shift_worker,
            athlete_high_training_load,
        }
    }

    #[must_use]
    pub const fn all(status: PopulationSupportStatus) -> Self {
        Self::new(status, status, status, status, status)
    }

    #[must_use]
    pub const fn general_adult_with_others(other: PopulationSupportStatus) -> Self {
        Self::new(
            PopulationSupportStatus::PopulationSpecific,
            other,
            other,
            other,
            other,
        )
    }

    #[must_use]
    pub const fn status_for(self, profile: PopulationProfile) -> PopulationSupportStatus {
        match profile {
            PopulationProfile::GeneralAdult => self.general_adult,
            PopulationProfile::OlderAdult => self.older_adult,
            PopulationProfile::PregnancyPostpartum => self.pregnancy_postpartum,
            PopulationProfile::ShiftWorker => self.shift_worker,
            PopulationProfile::AthleteHighTrainingLoad => self.athlete_high_training_load,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdateCadence {
    Quarterly,
    SemiAnnual,
    Annual,
    OnMajorGuidelineChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AllowedWordingTemplateId {
    GeneralAdultGuidanceComparison,
    WeeklyGuidanceProgress,
    BaselineRelativeDescription,
    TrendOnlyExploratory,
    ContextOnlyDescription,
    SensitiveMetricCaution,
    ConsumerWearableCaution,
}

impl AllowedWordingTemplateId {
    #[must_use]
    pub const fn template(self) -> &'static str {
        match self {
            Self::GeneralAdultGuidanceComparison => {
                "General adult public-health guidance can be referenced when clearly labeled as population-level guidance and paired with local baseline context."
            }
            Self::WeeklyGuidanceProgress => {
                "Weekly movement can be described relative to public-health activity guidance without diagnosing or coaching treatment."
            }
            Self::BaselineRelativeDescription => {
                "Describe the metric as above, below, or close to the user's recent baseline."
            }
            Self::TrendOnlyExploratory => {
                "Label the interpretation as exploratory and trend-based rather than clinical."
            }
            Self::ContextOnlyDescription => {
                "Use the metric only as context for timeline and review interpretation."
            }
            Self::SensitiveMetricCaution => {
                "Include an explicit caution that the metric is not for diagnosis, treatment, or screening."
            }
            Self::ConsumerWearableCaution => {
                "State that consumer wearable estimates are useful for trend and context, not clinical equivalence."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProhibitedWordingCategory {
    DiagnosisLike,
    TreatmentRecommendation,
    DiseaseScreening,
    CausalInference,
    CertaintyOverreach,
    IndividualizedThreshold,
    AcuteEscalation,
    ClinicalEquivalence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NumericThresholdPolicy {
    Allowed,
    PublicHealthOnly,
    Disallowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationScope {
    CrossSectional,
    WithinPersonTrendOnly,
    ContextualOnly,
}

impl InterpretationScope {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CrossSectional => "Guidance + context",
            Self::WithinPersonTrendOnly => "Trend only",
            Self::ContextualOnly => "Context only",
        }
    }

    #[must_use]
    pub const fn marker(self) -> &'static str {
        match self {
            Self::CrossSectional => "Guidance-backed",
            Self::WithinPersonTrendOnly => "Trend-only",
            Self::ContextualOnly => "Context-only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceRule {
    GeneralGuidancePlusBaseline,
    BaselineRelativeOnly,
    TrendOnly,
    ContextOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UncertaintyRequirement {
    PopulationScope,
    BaselineContext,
    DeviceLimitation,
    TrendOnlyLabel,
    MissingDataDisclosure,
    ConsumerWearableDisclosure,
    CounterevidenceWhenAvailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CounterevidenceRequirement {
    None,
    IncludeWhenAvailable,
    RequiredForSensitiveClaims,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CautionFlag {
    GeneralAdultGuidance,
    TrendOnly,
    ContextOnly,
    ConsumerWearable,
    SensitiveMetric,
    NotDiagnostic,
    NotScreening,
    BaselineHelpful,
}

impl CautionFlag {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::GeneralAdultGuidance => "General adult guidance",
            Self::TrendOnly => "Trend only",
            Self::ContextOnly => "Context only",
            Self::ConsumerWearable => "Consumer wearable limitation",
            Self::SensitiveMetric => "Sensitive metric",
            Self::NotDiagnostic => "Not diagnostic",
            Self::NotScreening => "Not for screening",
            Self::BaselineHelpful => "Use your baseline too",
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::GeneralAdultGuidance => {
                "This uses general adult public-health guidance rather than individualized clinical advice."
            }
            Self::TrendOnly => {
                "This should be read as a trend or context signal, not a standalone conclusion."
            }
            Self::ContextOnly => {
                "This metric is shown as context and should not be over-interpreted on its own."
            }
            Self::ConsumerWearable => {
                "Consumer wearable estimates can be useful for trends, but they are not clinical-grade measurements."
            }
            Self::SensitiveMetric => {
                "This domain is clinically sensitive and should be handled with extra caution."
            }
            Self::NotDiagnostic => {
                "Ringmaster does not diagnose conditions or recommend treatment."
            }
            Self::NotScreening => {
                "This should not be used for disease screening or to rule conditions in or out."
            }
            Self::BaselineHelpful => {
                "Pair this with your recent baseline before drawing conclusions from a single reading."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub struct PrimarySource {
    pub issuer: &'static str,
    pub title: &'static str,
    pub year: u16,
    pub url: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub struct EvidenceRegistryEntry {
    pub claim_key: &'static str,
    pub label: &'static str,
    pub source_family: SourceFamily,
    pub evidence_tier: EvidenceTier,
    pub evidence_type: EvidenceType,
    pub primary_sources: &'static [PrimarySource],
    pub last_reviewed: &'static str,
    pub population_support: PopulationSupportMatrix,
    pub update_cadence: UpdateCadence,
    pub guidance_anchor_label: Option<&'static str>,
    pub allowed_templates: &'static [AllowedWordingTemplateId],
    pub prohibited_wording_categories: &'static [ProhibitedWordingCategory],
    pub numeric_threshold_policy: NumericThresholdPolicy,
    pub interpretation_scope: InterpretationScope,
    pub confidence_rule: ConfidenceRule,
    pub uncertainty_requirements: &'static [UncertaintyRequirement],
    pub counterevidence_requirement: CounterevidenceRequirement,
    pub caution_flags: &'static [CautionFlag],
    pub escalation_notes: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceDescriptor {
    pub registry_version: String,
    pub claim_key: String,
    pub label: String,
    pub evidence_tier: EvidenceTier,
    pub evidence_type: EvidenceType,
    pub interpretation_scope: InterpretationScope,
    #[serde(default = "default_population_profile")]
    pub active_population_profile: PopulationProfile,
    #[serde(default = "default_population_support_status")]
    pub population_support_status: PopulationSupportStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_population_profile: Option<PopulationProfile>,
    pub guidance_anchor_label: Option<String>,
    pub caution_flags: Vec<CautionFlag>,
}

impl EvidenceRegistryEntry {
    #[must_use]
    pub fn last_reviewed_date(self) -> Option<Date> {
        Date::parse(self.last_reviewed, &Iso8601::DATE).ok()
    }

    #[must_use]
    pub const fn trend_only(self) -> bool {
        matches!(
            self.interpretation_scope,
            InterpretationScope::WithinPersonTrendOnly | InterpretationScope::ContextualOnly
        )
    }
}

const POPULATION_SPECIFIC_SUPPORT: PopulationSupportMatrix =
    PopulationSupportMatrix::all(PopulationSupportStatus::PopulationSpecific);
const GENERAL_ADULT_GUIDANCE_FALLBACK_SUPPORT: PopulationSupportMatrix =
    PopulationSupportMatrix::general_adult_with_others(
        PopulationSupportStatus::GeneralAdultOnlyFallback,
    );
const GENERAL_ADULT_SENSITIVE_SUPPORT: PopulationSupportMatrix =
    PopulationSupportMatrix::general_adult_with_others(PopulationSupportStatus::Unavailable);

#[must_use]
pub const fn default_population_profile() -> PopulationProfile {
    PopulationProfile::GeneralAdult
}

#[must_use]
pub const fn default_population_support_status() -> PopulationSupportStatus {
    PopulationSupportStatus::PopulationSpecific
}

const SLEEP_GUIDELINE_SOURCES: &[PrimarySource] = &[
    PrimarySource {
        issuer: "American Academy of Sleep Medicine and Sleep Research Society",
        title: "Consensus recommendation: adults should sleep 7 or more hours per night",
        year: 2015,
        url: "https://jcsm.aasm.org/doi/10.5664/jcsm.4950",
    },
    PrimarySource {
        issuer: "Centers for Disease Control and Prevention",
        title: "How Much Sleep Do I Need?",
        year: 2024,
        url: "https://www.cdc.gov/sleep/about_sleep/how_much_sleep.html",
    },
];

const ACTIVITY_GUIDELINE_SOURCES: &[PrimarySource] = &[
    PrimarySource {
        issuer: "U.S. Department of Health and Human Services",
        title: "Physical Activity Guidelines for Americans, 2nd edition",
        year: 2018,
        url: "https://health.gov/sites/default/files/2019-09/Physical_Activity_Guidelines_2nd_edition.pdf",
    },
    PrimarySource {
        issuer: "Centers for Disease Control and Prevention",
        title: "How much physical activity do adults need?",
        year: 2024,
        url: "https://www.cdc.gov/physicalactivity/basics/adults/index.htm",
    },
];

const SLEEP_TIMING_SOURCES: &[PrimarySource] = &[
    PrimarySource {
        issuer: "American Heart Association",
        title: "Life's Essential 8 scientific statement",
        year: 2022,
        url: "https://www.ahajournals.org/doi/10.1161/CIR.0000000000001078",
    },
    PrimarySource {
        issuer: "Systematic sleep health literature",
        title: "Sleep regularity and health outcomes evidence syntheses",
        year: 2023,
        url: "https://pubmed.ncbi.nlm.nih.gov/36804626/",
    },
];

const HEART_RATE_SOURCES: &[PrimarySource] = &[
    PrimarySource {
        issuer: "American College of Cardiology",
        title: "Wearable-derived physiologic metrics in practice statements",
        year: 2024,
        url: "https://www.jacc.org/doi/10.1016/j.jacc.2024.02.012",
    },
    PrimarySource {
        issuer: "Consumer wearable physiology syntheses",
        title: "Systematic reviews on heart-rate and wearable trends",
        year: 2023,
        url: "https://pubmed.ncbi.nlm.nih.gov/36640642/",
    },
];

const HRV_SOURCES: &[PrimarySource] = &[
    PrimarySource {
        issuer: "European Society of Cardiology",
        title: "Heart rate variability standards and interpretive cautions",
        year: 2015,
        url: "https://academic.oup.com/eurheartj/article/37/14/1116/2466099",
    },
    PrimarySource {
        issuer: "Consumer wearable physiology syntheses",
        title: "Systematic review of HRV from wearables",
        year: 2023,
        url: "https://pubmed.ncbi.nlm.nih.gov/37411628/",
    },
];

const SPO2_SOURCES: &[PrimarySource] = &[
    PrimarySource {
        issuer: "U.S. Food and Drug Administration",
        title: "Pulse oximeter basics and limitations",
        year: 2024,
        url: "https://www.fda.gov/medical-devices/safety-communications/pulse-oximeter-accuracy-and-limitations-fda-safety-communication",
    },
    PrimarySource {
        issuer: "National Institutes of Health",
        title: "Pulse oximetry accuracy and limitations",
        year: 2023,
        url: "https://www.nhlbi.nih.gov/health/pulse-oximetry",
    },
];

const CONSUMER_SLEEP_SOURCES: &[PrimarySource] = &[
    PrimarySource {
        issuer: "American Academy of Sleep Medicine",
        title: "Position statement on consumer sleep technology",
        year: 2018,
        url: "https://jcsm.aasm.org/doi/10.5664/jcsm.7230",
    },
    PrimarySource {
        issuer: "Sleep technology evidence reviews",
        title: "Consumer sleep technology limitations and opportunities",
        year: 2024,
        url: "https://pubmed.ncbi.nlm.nih.gov/38388086/",
    },
];

const COMPOSITE_LIMITATION_SOURCES: &[PrimarySource] = &[
    PrimarySource {
        issuer: "Consumer wearable documentation",
        title: "Oura metric descriptions and feature limitations",
        year: 2026,
        url: "https://support.ouraring.com/",
    },
    PrimarySource {
        issuer: "Wearable evidence syntheses",
        title: "Systematic reviews of consumer wearable summary metrics",
        year: 2023,
        url: "https://pubmed.ncbi.nlm.nih.gov/37166298/",
    },
];

const EXPLORATORY_TEMPLATES: &[AllowedWordingTemplateId] = &[
    AllowedWordingTemplateId::TrendOnlyExploratory,
    AllowedWordingTemplateId::ConsumerWearableCaution,
];

const BASELINE_TEMPLATES: &[AllowedWordingTemplateId] =
    &[AllowedWordingTemplateId::BaselineRelativeDescription];

const GUIDANCE_TEMPLATES: &[AllowedWordingTemplateId] = &[
    AllowedWordingTemplateId::GeneralAdultGuidanceComparison,
    AllowedWordingTemplateId::BaselineRelativeDescription,
];

const WEEKLY_GUIDANCE_TEMPLATES: &[AllowedWordingTemplateId] = &[
    AllowedWordingTemplateId::WeeklyGuidanceProgress,
    AllowedWordingTemplateId::BaselineRelativeDescription,
];

const CONTEXT_TEMPLATES: &[AllowedWordingTemplateId] =
    &[AllowedWordingTemplateId::ContextOnlyDescription];

const STANDARD_PROHIBITIONS: &[ProhibitedWordingCategory] = &[
    ProhibitedWordingCategory::DiagnosisLike,
    ProhibitedWordingCategory::TreatmentRecommendation,
    ProhibitedWordingCategory::DiseaseScreening,
    ProhibitedWordingCategory::CausalInference,
    ProhibitedWordingCategory::CertaintyOverreach,
];

const SENSITIVE_PROHIBITIONS: &[ProhibitedWordingCategory] = &[
    ProhibitedWordingCategory::DiagnosisLike,
    ProhibitedWordingCategory::TreatmentRecommendation,
    ProhibitedWordingCategory::DiseaseScreening,
    ProhibitedWordingCategory::CausalInference,
    ProhibitedWordingCategory::CertaintyOverreach,
    ProhibitedWordingCategory::IndividualizedThreshold,
    ProhibitedWordingCategory::AcuteEscalation,
    ProhibitedWordingCategory::ClinicalEquivalence,
];

const BASELINE_UNCERTAINTY: &[UncertaintyRequirement] = &[
    UncertaintyRequirement::BaselineContext,
    UncertaintyRequirement::MissingDataDisclosure,
    UncertaintyRequirement::CounterevidenceWhenAvailable,
];

const GUIDANCE_UNCERTAINTY: &[UncertaintyRequirement] = &[
    UncertaintyRequirement::PopulationScope,
    UncertaintyRequirement::BaselineContext,
    UncertaintyRequirement::MissingDataDisclosure,
    UncertaintyRequirement::CounterevidenceWhenAvailable,
];

const SENSITIVE_UNCERTAINTY: &[UncertaintyRequirement] = &[
    UncertaintyRequirement::DeviceLimitation,
    UncertaintyRequirement::ConsumerWearableDisclosure,
    UncertaintyRequirement::MissingDataDisclosure,
    UncertaintyRequirement::CounterevidenceWhenAvailable,
];

const EXPLORATORY_CAUTIONS: &[CautionFlag] = &[
    CautionFlag::TrendOnly,
    CautionFlag::ConsumerWearable,
    CautionFlag::NotDiagnostic,
    CautionFlag::BaselineHelpful,
];

const GUIDANCE_CAUTIONS: &[CautionFlag] = &[
    CautionFlag::GeneralAdultGuidance,
    CautionFlag::NotDiagnostic,
    CautionFlag::BaselineHelpful,
];

const SENSITIVE_CAUTIONS: &[CautionFlag] = &[
    CautionFlag::SensitiveMetric,
    CautionFlag::ConsumerWearable,
    CautionFlag::NotDiagnostic,
    CautionFlag::NotScreening,
    CautionFlag::BaselineHelpful,
];

const CONTEXT_CAUTIONS: &[CautionFlag] = &[
    CautionFlag::ContextOnly,
    CautionFlag::NotDiagnostic,
    CautionFlag::BaselineHelpful,
];

const EVIDENCE_REGISTRY: &[EvidenceRegistryEntry] = &[
    EvidenceRegistryEntry {
        claim_key: "sleep_duration",
        label: "Sleep duration",
        source_family: SourceFamily::PublicHealthAuthority,
        evidence_tier: EvidenceTier::GuidelineBacked,
        evidence_type: EvidenceType::PublicHealthGuidance,
        primary_sources: SLEEP_GUIDELINE_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: GENERAL_ADULT_GUIDANCE_FALLBACK_SUPPORT,
        update_cadence: UpdateCadence::Annual,
        guidance_anchor_label: Some("General adult sleep guidance"),
        allowed_templates: GUIDANCE_TEMPLATES,
        prohibited_wording_categories: STANDARD_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::PublicHealthOnly,
        interpretation_scope: InterpretationScope::CrossSectional,
        confidence_rule: ConfidenceRule::GeneralGuidancePlusBaseline,
        uncertainty_requirements: GUIDANCE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::IncludeWhenAvailable,
        caution_flags: GUIDANCE_CAUTIONS,
        escalation_notes: &[
            "Use population-level sleep guidance rather than individualized targets.",
            "Pair guidance with recent baseline context when local data is available.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "weekly_activity_minutes",
        label: "Weekly activity totals",
        source_family: SourceFamily::PublicHealthAuthority,
        evidence_tier: EvidenceTier::GuidelineBacked,
        evidence_type: EvidenceType::PublicHealthGuidance,
        primary_sources: ACTIVITY_GUIDELINE_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: GENERAL_ADULT_GUIDANCE_FALLBACK_SUPPORT,
        update_cadence: UpdateCadence::Annual,
        guidance_anchor_label: Some("General adult activity guidance"),
        allowed_templates: WEEKLY_GUIDANCE_TEMPLATES,
        prohibited_wording_categories: STANDARD_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::PublicHealthOnly,
        interpretation_scope: InterpretationScope::CrossSectional,
        confidence_rule: ConfidenceRule::GeneralGuidancePlusBaseline,
        uncertainty_requirements: GUIDANCE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::IncludeWhenAvailable,
        caution_flags: GUIDANCE_CAUTIONS,
        escalation_notes: &[
            "Describe activity relative to public-health guidance without implying treatment.",
            "Weekly totals should be read alongside distribution and local context.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "weekly_activity_distribution",
        label: "Weekly activity distribution",
        source_family: SourceFamily::PublicHealthAuthority,
        evidence_tier: EvidenceTier::GuidelineBacked,
        evidence_type: EvidenceType::PublicHealthGuidance,
        primary_sources: ACTIVITY_GUIDELINE_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: GENERAL_ADULT_GUIDANCE_FALLBACK_SUPPORT,
        update_cadence: UpdateCadence::Annual,
        guidance_anchor_label: Some("General adult activity guidance"),
        allowed_templates: WEEKLY_GUIDANCE_TEMPLATES,
        prohibited_wording_categories: STANDARD_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::PublicHealthOnly,
        interpretation_scope: InterpretationScope::CrossSectional,
        confidence_rule: ConfidenceRule::GeneralGuidancePlusBaseline,
        uncertainty_requirements: GUIDANCE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::IncludeWhenAvailable,
        caution_flags: GUIDANCE_CAUTIONS,
        escalation_notes: &["Describe distribution across the week without prescribing a program."],
    },
    EvidenceRegistryEntry {
        claim_key: "sleep_score",
        label: "Sleep score",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::Exploratory,
        evidence_type: EvidenceType::ConsumerWearableLimitation,
        primary_sources: CONSUMER_SLEEP_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: GENERAL_ADULT_SENSITIVE_SUPPORT,
        update_cadence: UpdateCadence::SemiAnnual,
        guidance_anchor_label: None,
        allowed_templates: EXPLORATORY_TEMPLATES,
        prohibited_wording_categories: SENSITIVE_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::TrendOnly,
        uncertainty_requirements: SENSITIVE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::RequiredForSensitiveClaims,
        caution_flags: SENSITIVE_CAUTIONS,
        escalation_notes: &[
            "Treat sleep score as a consumer summary metric rather than a clinical sleep judgment.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "readiness_score",
        label: "Readiness score",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::Exploratory,
        evidence_type: EvidenceType::ConsumerWearableLimitation,
        primary_sources: COMPOSITE_LIMITATION_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: GENERAL_ADULT_SENSITIVE_SUPPORT,
        update_cadence: UpdateCadence::SemiAnnual,
        guidance_anchor_label: None,
        allowed_templates: EXPLORATORY_TEMPLATES,
        prohibited_wording_categories: SENSITIVE_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::TrendOnly,
        uncertainty_requirements: SENSITIVE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::RequiredForSensitiveClaims,
        caution_flags: EXPLORATORY_CAUTIONS,
        escalation_notes: &[
            "Do not translate readiness composites into diagnosis, treatment, or disease claims.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "activity_score",
        label: "Activity score",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::Exploratory,
        evidence_type: EvidenceType::ConsumerWearableLimitation,
        primary_sources: COMPOSITE_LIMITATION_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: POPULATION_SPECIFIC_SUPPORT,
        update_cadence: UpdateCadence::SemiAnnual,
        guidance_anchor_label: None,
        allowed_templates: EXPLORATORY_TEMPLATES,
        prohibited_wording_categories: SENSITIVE_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::TrendOnly,
        uncertainty_requirements: SENSITIVE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::RequiredForSensitiveClaims,
        caution_flags: EXPLORATORY_CAUTIONS,
        escalation_notes: &[
            "Treat activity score as a composite convenience signal, not as a guideline threshold.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "active_calories",
        label: "Active calories",
        source_family: SourceFamily::SystematicEvidenceSynthesis,
        evidence_tier: EvidenceTier::EvidenceInformed,
        evidence_type: EvidenceType::SystematicReview,
        primary_sources: ACTIVITY_GUIDELINE_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: POPULATION_SPECIFIC_SUPPORT,
        update_cadence: UpdateCadence::Annual,
        guidance_anchor_label: None,
        allowed_templates: BASELINE_TEMPLATES,
        prohibited_wording_categories: STANDARD_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::BaselineRelativeOnly,
        uncertainty_requirements: BASELINE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::IncludeWhenAvailable,
        caution_flags: &[
            CautionFlag::TrendOnly,
            CautionFlag::NotDiagnostic,
            CautionFlag::BaselineHelpful,
        ],
        escalation_notes: &[
            "Use active calories as movement context instead of a clinical or public-health cutoff.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "steps",
        label: "Steps",
        source_family: SourceFamily::SystematicEvidenceSynthesis,
        evidence_tier: EvidenceTier::EvidenceInformed,
        evidence_type: EvidenceType::SystematicReview,
        primary_sources: ACTIVITY_GUIDELINE_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: POPULATION_SPECIFIC_SUPPORT,
        update_cadence: UpdateCadence::Annual,
        guidance_anchor_label: None,
        allowed_templates: BASELINE_TEMPLATES,
        prohibited_wording_categories: STANDARD_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::BaselineRelativeOnly,
        uncertainty_requirements: BASELINE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::IncludeWhenAvailable,
        caution_flags: &[
            CautionFlag::TrendOnly,
            CautionFlag::NotDiagnostic,
            CautionFlag::BaselineHelpful,
        ],
        escalation_notes: &[
            "Use step counts as general movement context, not as a disease or treatment claim.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "sleep_time_status",
        label: "Sleep timing status",
        source_family: SourceFamily::ScientificStatement,
        evidence_tier: EvidenceTier::EvidenceInformed,
        evidence_type: EvidenceType::ScientificStatement,
        primary_sources: SLEEP_TIMING_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: GENERAL_ADULT_GUIDANCE_FALLBACK_SUPPORT,
        update_cadence: UpdateCadence::Annual,
        guidance_anchor_label: Some("Sleep health timing context"),
        allowed_templates: BASELINE_TEMPLATES,
        prohibited_wording_categories: STANDARD_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::BaselineRelativeOnly,
        uncertainty_requirements: GUIDANCE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::IncludeWhenAvailable,
        caution_flags: &[
            CautionFlag::TrendOnly,
            CautionFlag::NotDiagnostic,
            CautionFlag::BaselineHelpful,
        ],
        escalation_notes: &[
            "Describe timing regularity as sleep-health context rather than a diagnosis or prescription.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "temperature_deviation",
        label: "Temperature deviation",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::Exploratory,
        evidence_type: EvidenceType::ConsumerWearableLimitation,
        primary_sources: COMPOSITE_LIMITATION_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: POPULATION_SPECIFIC_SUPPORT,
        update_cadence: UpdateCadence::SemiAnnual,
        guidance_anchor_label: None,
        allowed_templates: &[
            AllowedWordingTemplateId::ContextOnlyDescription,
            AllowedWordingTemplateId::SensitiveMetricCaution,
        ],
        prohibited_wording_categories: SENSITIVE_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::ContextualOnly,
        confidence_rule: ConfidenceRule::ContextOnly,
        uncertainty_requirements: SENSITIVE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::RequiredForSensitiveClaims,
        caution_flags: &[
            CautionFlag::SensitiveMetric,
            CautionFlag::ContextOnly,
            CautionFlag::NotDiagnostic,
            CautionFlag::BaselineHelpful,
        ],
        escalation_notes: &["Temperature deviation must stay descriptive and non-diagnostic."],
    },
    EvidenceRegistryEntry {
        claim_key: "stress_high",
        label: "Stress high time",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::Exploratory,
        evidence_type: EvidenceType::ConsumerWearableLimitation,
        primary_sources: COMPOSITE_LIMITATION_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: GENERAL_ADULT_SENSITIVE_SUPPORT,
        update_cadence: UpdateCadence::SemiAnnual,
        guidance_anchor_label: None,
        allowed_templates: EXPLORATORY_TEMPLATES,
        prohibited_wording_categories: SENSITIVE_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::TrendOnly,
        uncertainty_requirements: SENSITIVE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::RequiredForSensitiveClaims,
        caution_flags: EXPLORATORY_CAUTIONS,
        escalation_notes: &[
            "Stress summaries remain exploratory and should not be positioned as mental-health screening.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "recovery_high",
        label: "Recovery high time",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::Exploratory,
        evidence_type: EvidenceType::ConsumerWearableLimitation,
        primary_sources: COMPOSITE_LIMITATION_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: GENERAL_ADULT_SENSITIVE_SUPPORT,
        update_cadence: UpdateCadence::SemiAnnual,
        guidance_anchor_label: None,
        allowed_templates: EXPLORATORY_TEMPLATES,
        prohibited_wording_categories: SENSITIVE_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::TrendOnly,
        uncertainty_requirements: SENSITIVE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::RequiredForSensitiveClaims,
        caution_flags: EXPLORATORY_CAUTIONS,
        escalation_notes: &[
            "Recovery time can be described as more or less than usual without treating it as clinical recovery guidance.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "resilience_level",
        label: "Resilience level",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::Exploratory,
        evidence_type: EvidenceType::ConsumerWearableLimitation,
        primary_sources: COMPOSITE_LIMITATION_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: GENERAL_ADULT_SENSITIVE_SUPPORT,
        update_cadence: UpdateCadence::SemiAnnual,
        guidance_anchor_label: None,
        allowed_templates: EXPLORATORY_TEMPLATES,
        prohibited_wording_categories: SENSITIVE_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::TrendOnly,
        uncertainty_requirements: SENSITIVE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::RequiredForSensitiveClaims,
        caution_flags: EXPLORATORY_CAUTIONS,
        escalation_notes: &[
            "Resilience-like composites remain exploratory and must not be upgraded into medical advice.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "sleep_recovery",
        label: "Sleep recovery contributor",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::Exploratory,
        evidence_type: EvidenceType::ConsumerWearableLimitation,
        primary_sources: COMPOSITE_LIMITATION_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: POPULATION_SPECIFIC_SUPPORT,
        update_cadence: UpdateCadence::SemiAnnual,
        guidance_anchor_label: None,
        allowed_templates: EXPLORATORY_TEMPLATES,
        prohibited_wording_categories: SENSITIVE_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::TrendOnly,
        uncertainty_requirements: SENSITIVE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::RequiredForSensitiveClaims,
        caution_flags: EXPLORATORY_CAUTIONS,
        escalation_notes: &["Contributor scores remain exploratory and non-clinical."],
    },
    EvidenceRegistryEntry {
        claim_key: "daytime_recovery",
        label: "Daytime recovery contributor",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::Exploratory,
        evidence_type: EvidenceType::ConsumerWearableLimitation,
        primary_sources: COMPOSITE_LIMITATION_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: POPULATION_SPECIFIC_SUPPORT,
        update_cadence: UpdateCadence::SemiAnnual,
        guidance_anchor_label: None,
        allowed_templates: EXPLORATORY_TEMPLATES,
        prohibited_wording_categories: SENSITIVE_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::TrendOnly,
        uncertainty_requirements: SENSITIVE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::RequiredForSensitiveClaims,
        caution_flags: EXPLORATORY_CAUTIONS,
        escalation_notes: &["Contributor scores remain exploratory and non-clinical."],
    },
    EvidenceRegistryEntry {
        claim_key: "resilience_stress",
        label: "Resilience stress contributor",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::Exploratory,
        evidence_type: EvidenceType::ConsumerWearableLimitation,
        primary_sources: COMPOSITE_LIMITATION_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: GENERAL_ADULT_SENSITIVE_SUPPORT,
        update_cadence: UpdateCadence::SemiAnnual,
        guidance_anchor_label: None,
        allowed_templates: EXPLORATORY_TEMPLATES,
        prohibited_wording_categories: SENSITIVE_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::TrendOnly,
        uncertainty_requirements: SENSITIVE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::RequiredForSensitiveClaims,
        caution_flags: EXPLORATORY_CAUTIONS,
        escalation_notes: &["Contributor scores remain exploratory and non-clinical."],
    },
    EvidenceRegistryEntry {
        claim_key: "cardiovascular_age",
        label: "Cardiovascular age",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::Exploratory,
        evidence_type: EvidenceType::ConsumerWearableLimitation,
        primary_sources: COMPOSITE_LIMITATION_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: GENERAL_ADULT_SENSITIVE_SUPPORT,
        update_cadence: UpdateCadence::SemiAnnual,
        guidance_anchor_label: None,
        allowed_templates: EXPLORATORY_TEMPLATES,
        prohibited_wording_categories: SENSITIVE_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::TrendOnly,
        uncertainty_requirements: SENSITIVE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::RequiredForSensitiveClaims,
        caution_flags: EXPLORATORY_CAUTIONS,
        escalation_notes: &[
            "Cardiovascular-age-style metrics remain descriptive and exploratory in this product.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "vo2_max",
        label: "VO2 max",
        source_family: SourceFamily::ScientificStatement,
        evidence_tier: EvidenceTier::EvidenceInformed,
        evidence_type: EvidenceType::ScientificStatement,
        primary_sources: HEART_RATE_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: POPULATION_SPECIFIC_SUPPORT,
        update_cadence: UpdateCadence::Annual,
        guidance_anchor_label: None,
        allowed_templates: BASELINE_TEMPLATES,
        prohibited_wording_categories: STANDARD_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::BaselineRelativeOnly,
        uncertainty_requirements: BASELINE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::IncludeWhenAvailable,
        caution_flags: &[
            CautionFlag::TrendOnly,
            CautionFlag::NotDiagnostic,
            CautionFlag::BaselineHelpful,
        ],
        escalation_notes: &[
            "VO2 max estimates can be discussed as fitness trends without implying disease screening.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "resting_heart_rate",
        label: "Resting heart rate",
        source_family: SourceFamily::ScientificStatement,
        evidence_tier: EvidenceTier::EvidenceInformed,
        evidence_type: EvidenceType::ScientificStatement,
        primary_sources: HEART_RATE_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: POPULATION_SPECIFIC_SUPPORT,
        update_cadence: UpdateCadence::Annual,
        guidance_anchor_label: None,
        allowed_templates: BASELINE_TEMPLATES,
        prohibited_wording_categories: STANDARD_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::BaselineRelativeOnly,
        uncertainty_requirements: BASELINE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::IncludeWhenAvailable,
        caution_flags: &[
            CautionFlag::TrendOnly,
            CautionFlag::NotDiagnostic,
            CautionFlag::BaselineHelpful,
        ],
        escalation_notes: &["Keep heart-rate interpretation descriptive and baseline-relative."],
    },
    EvidenceRegistryEntry {
        claim_key: "hrv",
        label: "Heart rate variability",
        source_family: SourceFamily::SystematicEvidenceSynthesis,
        evidence_tier: EvidenceTier::EvidenceInformed,
        evidence_type: EvidenceType::SystematicReview,
        primary_sources: HRV_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: GENERAL_ADULT_SENSITIVE_SUPPORT,
        update_cadence: UpdateCadence::Annual,
        guidance_anchor_label: None,
        allowed_templates: BASELINE_TEMPLATES,
        prohibited_wording_categories: STANDARD_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::BaselineRelativeOnly,
        uncertainty_requirements: &[
            UncertaintyRequirement::BaselineContext,
            UncertaintyRequirement::DeviceLimitation,
            UncertaintyRequirement::MissingDataDisclosure,
            UncertaintyRequirement::CounterevidenceWhenAvailable,
        ],
        counterevidence_requirement: CounterevidenceRequirement::IncludeWhenAvailable,
        caution_flags: &[
            CautionFlag::TrendOnly,
            CautionFlag::NotDiagnostic,
            CautionFlag::BaselineHelpful,
        ],
        escalation_notes: &["HRV should stay trend-based and should not be framed as a diagnosis."],
    },
    EvidenceRegistryEntry {
        claim_key: "spo2",
        label: "Blood oxygen saturation",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::EvidenceInformed,
        evidence_type: EvidenceType::DeviceLimitation,
        primary_sources: SPO2_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: GENERAL_ADULT_SENSITIVE_SUPPORT,
        update_cadence: UpdateCadence::Quarterly,
        guidance_anchor_label: None,
        allowed_templates: &[
            AllowedWordingTemplateId::SensitiveMetricCaution,
            AllowedWordingTemplateId::ContextOnlyDescription,
        ],
        prohibited_wording_categories: SENSITIVE_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::ContextualOnly,
        confidence_rule: ConfidenceRule::ContextOnly,
        uncertainty_requirements: SENSITIVE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::RequiredForSensitiveClaims,
        caution_flags: SENSITIVE_CAUTIONS,
        escalation_notes: &[
            "SpO₂ must be labeled as context/trend only and never as a screening or diagnostic result.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "consumer_sleep_technology",
        label: "Consumer sleep technology output",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::EvidenceInformed,
        evidence_type: EvidenceType::PositionStatement,
        primary_sources: CONSUMER_SLEEP_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: GENERAL_ADULT_SENSITIVE_SUPPORT,
        update_cadence: UpdateCadence::Annual,
        guidance_anchor_label: None,
        allowed_templates: &[
            AllowedWordingTemplateId::ConsumerWearableCaution,
            AllowedWordingTemplateId::TrendOnlyExploratory,
        ],
        prohibited_wording_categories: SENSITIVE_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::WithinPersonTrendOnly,
        confidence_rule: ConfidenceRule::TrendOnly,
        uncertainty_requirements: SENSITIVE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::RequiredForSensitiveClaims,
        caution_flags: SENSITIVE_CAUTIONS,
        escalation_notes: &[
            "Consumer sleep technology can support personal trend tracking but not disease screening.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "pattern_association",
        label: "Pattern association",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::Exploratory,
        evidence_type: EvidenceType::ContextOnly,
        primary_sources: COMPOSITE_LIMITATION_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: POPULATION_SPECIFIC_SUPPORT,
        update_cadence: UpdateCadence::SemiAnnual,
        guidance_anchor_label: None,
        allowed_templates: &[
            AllowedWordingTemplateId::TrendOnlyExploratory,
            AllowedWordingTemplateId::ContextOnlyDescription,
        ],
        prohibited_wording_categories: SENSITIVE_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::ContextualOnly,
        confidence_rule: ConfidenceRule::ContextOnly,
        uncertainty_requirements: &[
            UncertaintyRequirement::TrendOnlyLabel,
            UncertaintyRequirement::MissingDataDisclosure,
            UncertaintyRequirement::CounterevidenceWhenAvailable,
        ],
        counterevidence_requirement: CounterevidenceRequirement::IncludeWhenAvailable,
        caution_flags: CONTEXT_CAUTIONS,
        escalation_notes: &[
            "Pattern rows are descriptive associations and should never be framed as causal or diagnostic.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "rest_mode_active",
        label: "Rest mode context",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::Exploratory,
        evidence_type: EvidenceType::ContextOnly,
        primary_sources: COMPOSITE_LIMITATION_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: POPULATION_SPECIFIC_SUPPORT,
        update_cadence: UpdateCadence::SemiAnnual,
        guidance_anchor_label: None,
        allowed_templates: CONTEXT_TEMPLATES,
        prohibited_wording_categories: STANDARD_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::ContextualOnly,
        confidence_rule: ConfidenceRule::ContextOnly,
        uncertainty_requirements: BASELINE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::IncludeWhenAvailable,
        caution_flags: CONTEXT_CAUTIONS,
        escalation_notes: &[
            "Rest mode is contextual state, not a diagnosis or treatment instruction.",
        ],
    },
    EvidenceRegistryEntry {
        claim_key: "session_context",
        label: "Session or context-derived interpretation",
        source_family: SourceFamily::DeviceDocumentation,
        evidence_tier: EvidenceTier::Exploratory,
        evidence_type: EvidenceType::ContextOnly,
        primary_sources: COMPOSITE_LIMITATION_SOURCES,
        last_reviewed: "2026-04-12",
        population_support: POPULATION_SPECIFIC_SUPPORT,
        update_cadence: UpdateCadence::SemiAnnual,
        guidance_anchor_label: None,
        allowed_templates: CONTEXT_TEMPLATES,
        prohibited_wording_categories: STANDARD_PROHIBITIONS,
        numeric_threshold_policy: NumericThresholdPolicy::Disallowed,
        interpretation_scope: InterpretationScope::ContextualOnly,
        confidence_rule: ConfidenceRule::ContextOnly,
        uncertainty_requirements: BASELINE_UNCERTAINTY,
        counterevidence_requirement: CounterevidenceRequirement::IncludeWhenAvailable,
        caution_flags: CONTEXT_CAUTIONS,
        escalation_notes: &[
            "Context-derived interpretations should stay descriptive and avoid causality claims.",
        ],
    },
];

#[must_use]
pub const fn evidence_registry_version() -> &'static str {
    EVIDENCE_REGISTRY_VERSION
}

#[must_use]
pub const fn evidence_registry() -> &'static [EvidenceRegistryEntry] {
    EVIDENCE_REGISTRY
}

#[must_use]
pub fn evidence_entry(claim_key: &str) -> Option<&'static EvidenceRegistryEntry> {
    EVIDENCE_REGISTRY
        .iter()
        .find(|entry| entry.claim_key == claim_key)
}

#[must_use]
pub fn evidence_descriptor(claim_key: &str) -> Option<EvidenceDescriptor> {
    resolve_evidence_descriptor(claim_key, PopulationProfile::GeneralAdult)
}

#[must_use]
pub fn resolve_evidence_descriptor(
    claim_key: &str,
    active_population: PopulationProfile,
) -> Option<EvidenceDescriptor> {
    let entry = evidence_entry(claim_key)?;
    let population_support_status = resolve_population_support_status(entry, active_population);
    Some(EvidenceDescriptor {
        registry_version: EVIDENCE_REGISTRY_VERSION.to_owned(),
        claim_key: entry.claim_key.to_owned(),
        label: entry.label.to_owned(),
        evidence_tier: entry.evidence_tier,
        evidence_type: entry.evidence_type,
        interpretation_scope: resolved_interpretation_scope(entry, population_support_status),
        active_population_profile: active_population,
        population_support_status,
        fallback_population_profile: (population_support_status
            == PopulationSupportStatus::GeneralAdultOnlyFallback)
            .then_some(PopulationProfile::GeneralAdult),
        guidance_anchor_label: resolved_guidance_anchor(entry, population_support_status),
        caution_flags: entry.caution_flags.to_vec(),
    })
}

#[must_use]
pub fn validate_registry() -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen_keys = std::collections::BTreeSet::new();

    for entry in EVIDENCE_REGISTRY {
        if !seen_keys.insert(entry.claim_key) {
            errors.push(format!(
                "duplicate evidence registry key `{}`",
                entry.claim_key
            ));
        }
        if entry.primary_sources.is_empty() {
            errors.push(format!("`{}` is missing primary sources", entry.claim_key));
        }
        if entry.last_reviewed_date().is_none() {
            errors.push(format!(
                "`{}` has an invalid last_reviewed date `{}`",
                entry.claim_key, entry.last_reviewed
            ));
        }
        if entry.allowed_templates.is_empty() {
            errors.push(format!(
                "`{}` is missing allowed wording templates",
                entry.claim_key
            ));
        }
        if entry.escalation_notes.is_empty() {
            errors.push(format!("`{}` is missing escalation notes", entry.claim_key));
        }
        if matches!(
            entry.numeric_threshold_policy,
            NumericThresholdPolicy::Allowed | NumericThresholdPolicy::PublicHealthOnly
        ) && !matches!(
            entry.interpretation_scope,
            InterpretationScope::CrossSectional
        ) {
            errors.push(format!(
                "`{}` allows thresholds but is not marked cross-sectional",
                entry.claim_key
            ));
        }
        if entry
            .population_support
            .status_for(PopulationProfile::GeneralAdult)
            != PopulationSupportStatus::PopulationSpecific
        {
            errors.push(format!(
                "`{}` must support the general_adult profile directly",
                entry.claim_key
            ));
        }
        for profile in PopulationProfile::ALL {
            let status = entry.population_support.status_for(profile);
            if is_sensitive_population_claim(entry.claim_key)
                && profile != PopulationProfile::GeneralAdult
                && status == PopulationSupportStatus::GeneralAdultOnlyFallback
            {
                errors.push(format!(
                    "`{}` cannot use general-adult fallback for sensitive population `{}`",
                    entry.claim_key,
                    profile.as_str()
                ));
            }
        }
    }

    errors
}

#[must_use]
pub fn stale_evidence_warnings(reference_day: Date) -> Vec<String> {
    evidence_registry()
        .iter()
        .filter_map(|entry| {
            let last_reviewed = entry.last_reviewed_date()?;
            let max_age_days = max_age_for_update_cadence(entry.update_cadence);
            let age_days = (reference_day - last_reviewed).whole_days();
            (age_days > max_age_days).then(|| {
                format!(
                    "`{}` last reviewed on {} and is stale for {:?} cadence ({} days old)",
                    entry.claim_key, entry.last_reviewed, entry.update_cadence, age_days
                )
            })
        })
        .collect()
}

fn resolve_population_support_status(
    entry: &EvidenceRegistryEntry,
    active_population: PopulationProfile,
) -> PopulationSupportStatus {
    let declared = entry.population_support.status_for(active_population);
    if active_population != PopulationProfile::GeneralAdult
        && is_sensitive_population_claim(entry.claim_key)
        && declared != PopulationSupportStatus::PopulationSpecific
    {
        return PopulationSupportStatus::Unavailable;
    }
    declared
}

const fn resolved_interpretation_scope(
    entry: &EvidenceRegistryEntry,
    population_support_status: PopulationSupportStatus,
) -> InterpretationScope {
    match population_support_status {
        PopulationSupportStatus::Unavailable => InterpretationScope::ContextualOnly,
        PopulationSupportStatus::PopulationSpecific
        | PopulationSupportStatus::GeneralAdultOnlyFallback => entry.interpretation_scope,
    }
}

fn resolved_guidance_anchor(
    entry: &EvidenceRegistryEntry,
    population_support_status: PopulationSupportStatus,
) -> Option<String> {
    match population_support_status {
        PopulationSupportStatus::Unavailable => None,
        PopulationSupportStatus::PopulationSpecific
        | PopulationSupportStatus::GeneralAdultOnlyFallback => {
            entry.guidance_anchor_label.map(str::to_owned)
        }
    }
}

fn is_sensitive_population_claim(claim_key: &str) -> bool {
    matches!(
        claim_key,
        "spo2"
            | "hrv"
            | "readiness_score"
            | "stress_high"
            | "recovery_high"
            | "resilience_level"
            | "resilience_stress"
            | "cardiovascular_age"
            | "sleep_score"
            | "consumer_sleep_technology"
    )
}

const fn max_age_for_update_cadence(update_cadence: UpdateCadence) -> i64 {
    match update_cadence {
        UpdateCadence::Quarterly => 120,
        UpdateCadence::SemiAnnual => 240,
        UpdateCadence::Annual => 400,
        UpdateCadence::OnMajorGuidelineChange => 730,
    }
}

#[cfg(test)]
mod tests {
    use time::{Date, format_description::well_known::Iso8601};

    use crate::review::registry::signal_definitions;

    use super::{
        EvidenceTier, NumericThresholdPolicy, PopulationProfile, PopulationSupportStatus,
        evidence_entry, evidence_registry, resolve_evidence_descriptor, stale_evidence_warnings,
        validate_registry,
    };

    #[test]
    fn registry_validates_cleanly() {
        let errors = validate_registry();
        assert_eq!(errors, Vec::<String>::new());
    }

    #[test]
    fn every_review_signal_has_registry_entry() {
        for definition in signal_definitions() {
            assert!(
                evidence_entry(definition.key).is_some(),
                "missing evidence entry for review signal `{}`",
                definition.key
            );
        }
    }

    #[test]
    fn guideline_backed_entries_only_allow_public_health_thresholds_cross_sectionally() {
        for entry in evidence_registry()
            .iter()
            .filter(|entry| entry.evidence_tier == EvidenceTier::GuidelineBacked)
        {
            assert_eq!(
                entry.numeric_threshold_policy,
                NumericThresholdPolicy::PublicHealthOnly,
                "guideline-backed entry `{}` should only allow public-health thresholds",
                entry.claim_key
            );
        }
    }

    #[test]
    fn sensitive_claims_resolve_as_unavailable_for_non_general_profiles() {
        let descriptor = resolve_evidence_descriptor("spo2", PopulationProfile::OlderAdult)
            .expect("spo2 descriptor should resolve");
        assert_eq!(
            descriptor.population_support_status,
            PopulationSupportStatus::Unavailable
        );
        assert_eq!(
            descriptor.interpretation_scope,
            super::InterpretationScope::ContextualOnly
        );
        assert!(descriptor.guidance_anchor_label.is_none());
    }

    #[test]
    fn guidance_claims_can_resolve_as_general_adult_fallback() {
        let descriptor =
            resolve_evidence_descriptor("sleep_duration", PopulationProfile::ShiftWorker)
                .expect("sleep_duration descriptor should resolve");
        assert_eq!(
            descriptor.population_support_status,
            PopulationSupportStatus::GeneralAdultOnlyFallback
        );
        assert_eq!(
            descriptor.fallback_population_profile,
            Some(PopulationProfile::GeneralAdult)
        );
    }

    #[test]
    fn stale_entries_surface_warnings_against_fixed_reference_date() {
        let reference_day =
            Date::parse("2030-01-01", &Iso8601::DATE).expect("reference date should parse");
        let warnings = stale_evidence_warnings(reference_day);
        assert!(!warnings.is_empty());
    }
}
