pub mod policy;
pub mod registry;

pub use policy::{
    ClaimLanguageSpec, GuidanceComparison, PolicyViolation, append_required_disclaimers,
    claim_language_spec, evidence_badges, guidance_comparison_text, validate_claim_text,
};
pub use registry::{
    AllowedWordingTemplateId, CautionFlag, ConfidenceRule, CounterevidenceRequirement,
    EvidenceDescriptor, EvidenceRegistryEntry, EvidenceTier, EvidenceType, InterpretationScope,
    NumericThresholdPolicy, PopulationProfile, PopulationSupportMatrix, PopulationSupportStatus,
    PrimarySource, ProhibitedWordingCategory, SourceFamily, UncertaintyRequirement, UpdateCadence,
    evidence_entry, evidence_registry, evidence_registry_version, resolve_evidence_descriptor,
    stale_evidence_warnings, validate_registry,
};
