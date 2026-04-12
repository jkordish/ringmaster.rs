use std::collections::BTreeSet;
use std::fs;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use reqwest::{Client, StatusCode};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use time::{Date, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::ai_prompts::{
    COMPARE_PROMPT_VERSION, FOLLOW_UP_PROMPT_VERSION, REVIEW_PROMPT_VERSION, compare_system_prompt,
    compare_task_framing, follow_up_system_prompt, follow_up_task_framing, review_system_prompt,
    review_task_framing,
};
use crate::config::{AiConfig, AiInputTransport, AiRequestMode, Config, PromptCacheMode};
use crate::error::{Result, RingmasterError};
use crate::evidence::policy::{evidence_badges, validate_claim_text};
use crate::evidence::registry::{
    EvidenceTier, InterpretationScope, PopulationProfile, PopulationSupportStatus,
    resolve_evidence_descriptor,
};
use crate::snapshot::{
    ArtifactRecordInput, LoadedSnapshotArtifact, PrivacyProfile, SnapshotBundleV1,
    SnapshotFollowUpTarget, SnapshotReviewSignal, artifact_record, rebuild_follow_up_targets,
};
use crate::store::queries::AiArtifactRecord;

pub const REVIEW_OUTPUT_SCHEMA_VERSION: &str = "ringmaster.ai.review.v3";
pub const COMPARE_OUTPUT_SCHEMA_VERSION: &str = "ringmaster.ai.compare.v3";
pub const FOLLOW_UP_OUTPUT_SCHEMA_VERSION: &str = "ringmaster.ai.follow_up.v3";

type ProviderFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiRunMode {
    Real,
    DryRun,
    Fixture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactStatus {
    Success,
    Insufficient,
    DryRun,
    Fixture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AiRunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GuidedFollowUpKind {
    ExpandEvidence,
    ShowCounterevidence,
    ExplainRanking,
    SuggestLocalDrilldown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SufficiencyLevel {
    Missing,
    Thin,
    Medium,
    Strong,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ArtifactEvidenceRef {
    pub export_ref: String,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ArtifactFinding {
    pub finding_id: String,
    pub title: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_tier: Option<EvidenceTier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interpretation_scope: Option<InterpretationScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_population_profile: Option<PopulationProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub population_support_status: Option<PopulationSupportStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_population_profile: Option<PopulationProfile>,
    #[serde(default)]
    pub caution_labels: Vec<String>,
    pub confidence: ConfidenceLevel,
    pub sufficiency: SufficiencyLevel,
    pub evidence_refs: Vec<ArtifactEvidenceRef>,
    pub counterevidence_refs: Vec<ArtifactEvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ArtifactLimitation {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ArtifactFollowUpTarget {
    pub label: String,
    pub command: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ReviewArtifactV1 {
    pub schema_version: String,
    pub prompt_version: String,
    pub status: ArtifactStatus,
    pub overview: String,
    pub headline_findings: Vec<ArtifactFinding>,
    pub positive_findings: Vec<ArtifactFinding>,
    pub negative_findings: Vec<ArtifactFinding>,
    pub unresolved_questions: Vec<String>,
    pub limitations: Vec<ArtifactLimitation>,
    pub follow_up_targets: Vec<ArtifactFollowUpTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct CompareArtifactV1 {
    pub schema_version: String,
    pub prompt_version: String,
    pub status: ArtifactStatus,
    pub overview: String,
    pub material_differences: Vec<ArtifactFinding>,
    pub supporting_evidence: Vec<ArtifactEvidenceRef>,
    pub uncertainty_warnings: Vec<String>,
    pub investigation_targets: Vec<ArtifactFollowUpTarget>,
    pub only_in_a: Vec<String>,
    pub only_in_b: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct FollowUpArtifactV1 {
    pub schema_version: String,
    pub prompt_version: String,
    pub status: ArtifactStatus,
    pub follow_up_kind: GuidedFollowUpKind,
    pub overview: String,
    pub focal_findings: Vec<ArtifactFinding>,
    pub reasoning_steps: Vec<String>,
    pub unresolved_questions: Vec<String>,
    pub suggested_local_targets: Vec<ArtifactFollowUpTarget>,
}

#[derive(Debug, Clone)]
pub struct ReviewRunOutput {
    pub artifact: ReviewArtifactV1,
    pub payload_json: String,
    pub rendered_briefing: String,
    pub request_preview: AiRequestPreview,
    pub request_fingerprint: String,
    pub record: AiArtifactRecord,
}

#[derive(Debug, Clone)]
pub struct CompareRunOutput {
    pub artifact: CompareArtifactV1,
    pub payload_json: String,
    pub rendered_briefing: String,
    pub request_preview: AiRequestPreview,
    pub request_fingerprint: String,
    pub record: AiArtifactRecord,
}

#[derive(Debug, Clone)]
pub struct FollowUpRunOutput {
    pub artifact: FollowUpArtifactV1,
    pub payload_json: String,
    pub rendered_briefing: String,
    pub request_preview: AiRequestPreview,
    pub request_fingerprint: String,
    pub record: AiArtifactRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredArtifact {
    Review(ReviewArtifactV1),
    Compare(CompareArtifactV1),
    FollowUp(FollowUpArtifactV1),
}

#[derive(Debug, Clone)]
pub struct ProviderMetadata {
    pub provider: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub request_mode: String,
    pub input_transport: String,
    pub run_mode: AiRunMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct AiRequestPreviewSnapshot {
    pub label: String,
    pub snapshot_hash: String,
    pub scope: String,
    pub anchor_day: String,
    pub privacy_profile: PrivacyProfile,
    pub active_population_profile: PopulationProfile,
    pub day_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct AiRequestPreview {
    pub task_family: String,
    pub provider: String,
    pub model: String,
    pub request_mode: String,
    pub input_transport: String,
    pub prompt_cache: String,
    pub prompt_version: String,
    pub output_schema_version: String,
    pub snapshots: Vec<AiRequestPreviewSnapshot>,
    pub snapshot_bytes: usize,
    pub approximate_input_tokens: usize,
    pub stateless: bool,
    pub tools_disabled: bool,
    pub includes_notes_or_free_text: bool,
    pub content_classes: Vec<String>,
    pub prefix_fingerprint: String,
    pub payload_fingerprint: String,
    pub request_fingerprint: String,
}

pub struct ReviewProviderRequest<'a> {
    pub snapshot: &'a SnapshotBundleV1,
    pub snapshot_json: &'a str,
}

pub struct CompareProviderRequest<'a> {
    pub snapshot_a: &'a SnapshotBundleV1,
    pub snapshot_a_json: &'a str,
    pub snapshot_b: &'a SnapshotBundleV1,
    pub snapshot_b_json: &'a str,
}

pub struct FollowUpProviderRequest<'a> {
    pub snapshots: &'a [(&'a SnapshotBundleV1, &'a str)],
    pub source_artifact_json: &'a str,
    pub source_artifact_kind: &'a str,
    pub follow_up_kind: GuidedFollowUpKind,
}

struct StructuredOutputRequestPlan {
    body: Value,
    preview: AiRequestPreview,
    request_fingerprint: String,
}

trait AiProvider {
    fn metadata(&self) -> ProviderMetadata;

    fn review<'a>(
        &'a self,
        request: ReviewProviderRequest<'a>,
    ) -> ProviderFuture<'a, ReviewArtifactV1>;

    fn compare<'a>(
        &'a self,
        request: CompareProviderRequest<'a>,
    ) -> ProviderFuture<'a, CompareArtifactV1>;

    fn follow_up<'a>(
        &'a self,
        request: FollowUpProviderRequest<'a>,
    ) -> ProviderFuture<'a, FollowUpArtifactV1>;
}

struct DryRunProvider {
    config: AiConfig,
}

struct FixtureProvider {
    fixture_path: Box<Path>,
}

struct OpenAiProvider {
    config: AiConfig,
}

/// # Errors
///
/// Returns an error if the snapshot cannot be turned into a valid review request or if the provider run fails.
pub async fn review_snapshot(
    config: &Config,
    snapshot: &LoadedSnapshotArtifact,
    dry_run: bool,
    fixture: Option<&Path>,
) -> Result<ReviewRunOutput> {
    review_snapshot_with_run_identity(config, snapshot, dry_run, fixture, None).await
}

/// # Errors
///
/// Returns an error if the snapshot cannot be converted into a valid review request preview.
pub fn preview_review_request(
    config: &Config,
    snapshot: &LoadedSnapshotArtifact,
) -> Result<AiRequestPreview> {
    build_review_request_plan(&config.ai, &snapshot.bundle, &snapshot.compact_json)
        .map(|plan| plan.preview)
}

pub(crate) async fn review_snapshot_with_run_identity(
    config: &Config,
    snapshot: &LoadedSnapshotArtifact,
    dry_run: bool,
    fixture: Option<&Path>,
    run_identity_override: Option<&str>,
) -> Result<ReviewRunOutput> {
    let mut request_plan =
        build_review_request_plan(&config.ai, &snapshot.bundle, &snapshot.compact_json)?;
    let provider = select_provider(&config.ai, dry_run, fixture)?;
    let metadata = provider.metadata();
    apply_provider_metadata_to_preview(&mut request_plan.preview, &metadata);
    let artifact = provider
        .review(ReviewProviderRequest {
            snapshot: &snapshot.bundle,
            snapshot_json: &snapshot.compact_json,
        })
        .await?;
    let artifact = sanitize_review_artifact(&snapshot.bundle, artifact)?;
    let created_at = resolve_run_created_at(run_identity_override)?;
    let payload_json = serde_json::to_string_pretty(&artifact)?;
    let rendered_briefing = render_review_briefing(&artifact);
    let summary_cache = review_summary_cache(&artifact);
    let record = artifact_record(ArtifactRecordInput {
        artifact_id: artifact_id(
            "review",
            &snapshot.bundle.metadata.snapshot_hash,
            None,
            metadata.run_mode,
            &created_at,
            &payload_json,
        ),
        artifact_kind: "review",
        output_schema_version: REVIEW_OUTPUT_SCHEMA_VERSION,
        prompt_version: REVIEW_PROMPT_VERSION,
        provider: &metadata.provider,
        model: &metadata.model,
        reasoning_effort: metadata.reasoning_effort.as_deref(),
        request_mode: &metadata.request_mode,
        input_transport: &metadata.input_transport,
        run_mode: metadata.run_mode.as_str(),
        created_at,
        snapshot_hash_a: &snapshot.bundle.metadata.snapshot_hash,
        snapshot_hash_b: None,
        privacy_profile: snapshot.bundle.metadata.privacy_profile,
        artifact_status: artifact.status.as_str(),
        overview: &artifact.overview,
        summary_cache: &summary_cache,
        request_fingerprint: Some(&request_plan.request_fingerprint),
        payload_json: payload_json.clone(),
        rendered_briefing: rendered_briefing.clone(),
    })?;

    Ok(ReviewRunOutput {
        artifact,
        payload_json,
        rendered_briefing,
        request_preview: request_plan.preview,
        request_fingerprint: request_plan.request_fingerprint,
        record,
    })
}

/// # Errors
///
/// Returns an error if the compare request cannot be built, executed, sanitized, or serialized.
pub async fn compare_snapshots(
    config: &Config,
    snapshot_a: &LoadedSnapshotArtifact,
    snapshot_b: &LoadedSnapshotArtifact,
    dry_run: bool,
    fixture: Option<&Path>,
) -> Result<CompareRunOutput> {
    compare_snapshots_with_run_identity(config, snapshot_a, snapshot_b, dry_run, fixture, None)
        .await
}

/// # Errors
///
/// Returns an error if the follow-up request cannot be built, executed, sanitized, or serialized.
pub async fn follow_up_from_artifact(
    config: &Config,
    snapshots: &[LoadedSnapshotArtifact],
    source_record: &AiArtifactRecord,
    follow_up_kind: GuidedFollowUpKind,
    dry_run: bool,
    fixture: Option<&Path>,
) -> Result<FollowUpRunOutput> {
    follow_up_from_artifact_with_run_identity(
        config,
        snapshots,
        source_record,
        follow_up_kind,
        dry_run,
        fixture,
        None,
    )
    .await
}

/// # Errors
///
/// Returns an error if the saved artifact or snapshots cannot be converted into a valid follow-up request preview.
pub fn preview_follow_up_request(
    config: &Config,
    snapshots: &[LoadedSnapshotArtifact],
    source_record: &AiArtifactRecord,
    follow_up_kind: GuidedFollowUpKind,
) -> Result<AiRequestPreview> {
    let snapshot_views = snapshots
        .iter()
        .map(|snapshot| (&snapshot.bundle, snapshot.compact_json.as_str()))
        .collect::<Vec<_>>();
    build_follow_up_request_plan(
        &config.ai,
        &snapshot_views,
        &source_record.artifact_kind,
        &source_record.payload_json,
        follow_up_kind,
    )
    .map(|plan| plan.preview)
}

/// # Errors
///
/// Returns an error if the two snapshots cannot be converted into a valid compare request preview.
pub fn preview_compare_request(
    config: &Config,
    snapshot_a: &LoadedSnapshotArtifact,
    snapshot_b: &LoadedSnapshotArtifact,
) -> Result<AiRequestPreview> {
    build_compare_request_plan(
        &config.ai,
        &snapshot_a.bundle,
        &snapshot_a.compact_json,
        &snapshot_b.bundle,
        &snapshot_b.compact_json,
    )
    .map(|plan| plan.preview)
}

pub(crate) async fn compare_snapshots_with_run_identity(
    config: &Config,
    snapshot_a: &LoadedSnapshotArtifact,
    snapshot_b: &LoadedSnapshotArtifact,
    dry_run: bool,
    fixture: Option<&Path>,
    run_identity_override: Option<&str>,
) -> Result<CompareRunOutput> {
    let mut request_plan = build_compare_request_plan(
        &config.ai,
        &snapshot_a.bundle,
        &snapshot_a.compact_json,
        &snapshot_b.bundle,
        &snapshot_b.compact_json,
    )?;
    let provider = select_provider(&config.ai, dry_run, fixture)?;
    let metadata = provider.metadata();
    apply_provider_metadata_to_preview(&mut request_plan.preview, &metadata);
    let artifact = provider
        .compare(CompareProviderRequest {
            snapshot_a: &snapshot_a.bundle,
            snapshot_a_json: &snapshot_a.compact_json,
            snapshot_b: &snapshot_b.bundle,
            snapshot_b_json: &snapshot_b.compact_json,
        })
        .await?;
    let created_at = resolve_run_created_at(run_identity_override)?;
    let payload_json = serde_json::to_string_pretty(&artifact)?;
    let rendered_briefing = render_compare_briefing(&artifact);
    let summary_cache = compare_summary_cache(&artifact);
    let record = artifact_record(ArtifactRecordInput {
        artifact_id: artifact_id(
            "compare",
            &snapshot_a.bundle.metadata.snapshot_hash,
            Some(&snapshot_b.bundle.metadata.snapshot_hash),
            metadata.run_mode,
            &created_at,
            &payload_json,
        ),
        artifact_kind: "compare",
        output_schema_version: COMPARE_OUTPUT_SCHEMA_VERSION,
        prompt_version: COMPARE_PROMPT_VERSION,
        provider: &metadata.provider,
        model: &metadata.model,
        reasoning_effort: metadata.reasoning_effort.as_deref(),
        request_mode: &metadata.request_mode,
        input_transport: &metadata.input_transport,
        run_mode: metadata.run_mode.as_str(),
        created_at,
        snapshot_hash_a: &snapshot_a.bundle.metadata.snapshot_hash,
        snapshot_hash_b: Some(&snapshot_b.bundle.metadata.snapshot_hash),
        privacy_profile: merged_privacy_profile(
            snapshot_a.bundle.metadata.privacy_profile,
            snapshot_b.bundle.metadata.privacy_profile,
        ),
        artifact_status: artifact.status.as_str(),
        overview: &artifact.overview,
        summary_cache: &summary_cache,
        request_fingerprint: Some(&request_plan.request_fingerprint),
        payload_json: payload_json.clone(),
        rendered_briefing: rendered_briefing.clone(),
    })?;

    Ok(CompareRunOutput {
        artifact,
        payload_json,
        rendered_briefing,
        request_preview: request_plan.preview,
        request_fingerprint: request_plan.request_fingerprint,
        record,
    })
}

pub(crate) async fn follow_up_from_artifact_with_run_identity(
    config: &Config,
    snapshots: &[LoadedSnapshotArtifact],
    source_record: &AiArtifactRecord,
    follow_up_kind: GuidedFollowUpKind,
    dry_run: bool,
    fixture: Option<&Path>,
    run_identity_override: Option<&str>,
) -> Result<FollowUpRunOutput> {
    let (snapshot_hash_a, snapshot_hash_b) = follow_up_snapshot_hashes(snapshots)?;
    let snapshot_views = snapshots
        .iter()
        .map(|snapshot| (&snapshot.bundle, snapshot.compact_json.as_str()))
        .collect::<Vec<_>>();
    let mut request_plan = build_follow_up_request_plan(
        &config.ai,
        &snapshot_views,
        &source_record.artifact_kind,
        &source_record.payload_json,
        follow_up_kind,
    )?;
    let provider = select_provider(&config.ai, dry_run, fixture)?;
    let metadata = provider.metadata();
    apply_provider_metadata_to_preview(&mut request_plan.preview, &metadata);
    let artifact = provider
        .follow_up(FollowUpProviderRequest {
            snapshots: &snapshot_views,
            source_artifact_json: &source_record.payload_json,
            source_artifact_kind: &source_record.artifact_kind,
            follow_up_kind,
        })
        .await?;
    let created_at = resolve_run_created_at(run_identity_override)?;
    let payload_json = serde_json::to_string_pretty(&artifact)?;
    let rendered_briefing = render_follow_up_briefing(&artifact);
    let summary_cache = follow_up_summary_cache(&artifact);
    let privacy_profile = merged_snapshot_privacy_profile(snapshots).unwrap_or_else(|| {
        parse_privacy_profile(&source_record.privacy_profile).unwrap_or(PrivacyProfile::Redacted)
    });
    let record = artifact_record(ArtifactRecordInput {
        artifact_id: artifact_id(
            "follow_up",
            snapshot_hash_a,
            snapshot_hash_b,
            metadata.run_mode,
            &created_at,
            &payload_json,
        ),
        artifact_kind: "follow_up",
        output_schema_version: FOLLOW_UP_OUTPUT_SCHEMA_VERSION,
        prompt_version: FOLLOW_UP_PROMPT_VERSION,
        provider: &metadata.provider,
        model: &metadata.model,
        reasoning_effort: metadata.reasoning_effort.as_deref(),
        request_mode: &metadata.request_mode,
        input_transport: &metadata.input_transport,
        run_mode: metadata.run_mode.as_str(),
        created_at,
        snapshot_hash_a,
        snapshot_hash_b,
        privacy_profile,
        artifact_status: artifact.status.as_str(),
        overview: &artifact.overview,
        summary_cache: &summary_cache,
        request_fingerprint: Some(&request_plan.request_fingerprint),
        payload_json: payload_json.clone(),
        rendered_briefing: rendered_briefing.clone(),
    })?;

    Ok(FollowUpRunOutput {
        artifact,
        payload_json,
        rendered_briefing,
        request_preview: request_plan.preview,
        request_fingerprint: request_plan.request_fingerprint,
        record,
    })
}

#[must_use]
pub fn render_review_briefing(artifact: &ReviewArtifactV1) -> String {
    let mut lines = vec![
        "ringmaster ai review".to_owned(),
        String::new(),
        format!("status: {}", artifact.status.as_str()),
        format!("overview: {}", artifact.overview),
    ];
    if !artifact.headline_findings.is_empty() {
        lines.push("headline_findings:".to_owned());
        lines.extend(render_findings(&artifact.headline_findings));
    }
    if !artifact.positive_findings.is_empty() {
        lines.push("positive_findings:".to_owned());
        lines.extend(render_findings(&artifact.positive_findings));
    }
    if !artifact.negative_findings.is_empty() {
        lines.push("negative_findings:".to_owned());
        lines.extend(render_findings(&artifact.negative_findings));
    }
    if !artifact.unresolved_questions.is_empty() {
        lines.push("unresolved_questions:".to_owned());
        lines.extend(
            artifact
                .unresolved_questions
                .iter()
                .map(|question| format!("  - {question}")),
        );
    }
    if !artifact.limitations.is_empty() {
        lines.push("limitations:".to_owned());
        lines.extend(
            artifact
                .limitations
                .iter()
                .map(|limitation| format!("  - {}: {}", limitation.code, limitation.message)),
        );
    }
    if !artifact.follow_up_targets.is_empty() {
        lines.push("follow_up_targets:".to_owned());
        lines.extend(artifact.follow_up_targets.iter().map(|target| {
            format!(
                "  - {} => {} ({})",
                target.label, target.command, target.reason
            )
        }));
    }
    lines.join("\n")
}

#[must_use]
pub fn render_compare_briefing(artifact: &CompareArtifactV1) -> String {
    let mut lines = vec![
        "ringmaster ai compare".to_owned(),
        String::new(),
        format!("status: {}", artifact.status.as_str()),
        format!("overview: {}", artifact.overview),
    ];
    if !artifact.material_differences.is_empty() {
        lines.push("material_differences:".to_owned());
        lines.extend(render_findings(&artifact.material_differences));
    }
    if !artifact.supporting_evidence.is_empty() {
        lines.push("supporting_evidence:".to_owned());
        lines.extend(
            artifact
                .supporting_evidence
                .iter()
                .map(|evidence| format!("  - {} ({})", evidence.export_ref, evidence.note)),
        );
    }
    if !artifact.uncertainty_warnings.is_empty() {
        lines.push("uncertainty_warnings:".to_owned());
        lines.extend(
            artifact
                .uncertainty_warnings
                .iter()
                .map(|warning| format!("  - {warning}")),
        );
    }
    if !artifact.investigation_targets.is_empty() {
        lines.push("investigation_targets:".to_owned());
        lines.extend(artifact.investigation_targets.iter().map(|target| {
            format!(
                "  - {} => {} ({})",
                target.label, target.command, target.reason
            )
        }));
    }
    if !artifact.only_in_a.is_empty() {
        lines.push("only_in_a:".to_owned());
        lines.extend(artifact.only_in_a.iter().map(|item| format!("  - {item}")));
    }
    if !artifact.only_in_b.is_empty() {
        lines.push("only_in_b:".to_owned());
        lines.extend(artifact.only_in_b.iter().map(|item| format!("  - {item}")));
    }
    lines.join("\n")
}

#[must_use]
pub fn render_follow_up_briefing(artifact: &FollowUpArtifactV1) -> String {
    let mut lines = vec![
        "ringmaster ai follow-up".to_owned(),
        String::new(),
        format!("status: {}", artifact.status.as_str()),
        format!("follow_up_kind: {}", artifact.follow_up_kind.as_str()),
        format!("overview: {}", artifact.overview),
    ];
    if !artifact.focal_findings.is_empty() {
        lines.push("focal_findings:".to_owned());
        lines.extend(render_findings(&artifact.focal_findings));
    }
    if !artifact.reasoning_steps.is_empty() {
        lines.push("reasoning_steps:".to_owned());
        lines.extend(
            artifact
                .reasoning_steps
                .iter()
                .map(|step| format!("  - {step}")),
        );
    }
    if !artifact.unresolved_questions.is_empty() {
        lines.push("unresolved_questions:".to_owned());
        lines.extend(
            artifact
                .unresolved_questions
                .iter()
                .map(|question| format!("  - {question}")),
        );
    }
    if !artifact.suggested_local_targets.is_empty() {
        lines.push("suggested_local_targets:".to_owned());
        lines.extend(artifact.suggested_local_targets.iter().map(|target| {
            format!(
                "  - {} => {} ({})",
                target.label, target.command, target.reason
            )
        }));
    }
    lines.join("\n")
}

/// # Errors
///
/// Returns an error if the stored payload does not match a supported artifact kind or fails to deserialize.
pub fn parse_stored_artifact(record: &AiArtifactRecord) -> Result<StoredArtifact> {
    match record.artifact_kind.as_str() {
        "review" => Ok(StoredArtifact::Review(serde_json::from_str(
            &record.payload_json,
        )?)),
        "compare" => Ok(StoredArtifact::Compare(serde_json::from_str(
            &record.payload_json,
        )?)),
        "follow_up" => Ok(StoredArtifact::FollowUp(serde_json::from_str(
            &record.payload_json,
        )?)),
        other => Err(RingmasterError::Ui(format!(
            "unsupported AI artifact kind `{other}`"
        ))),
    }
}

fn sanitize_review_artifact(
    snapshot: &SnapshotBundleV1,
    mut artifact: ReviewArtifactV1,
) -> Result<ReviewArtifactV1> {
    let valid_export_refs = collect_export_refs_from_snapshot(snapshot)?;
    let mut seen_finding_ids = BTreeSet::new();
    let mut seen_finding_themes = BTreeSet::new();

    artifact.headline_findings = sanitize_review_findings(
        artifact.headline_findings,
        &valid_export_refs,
        snapshot.metadata.active_population_profile,
        &mut seen_finding_ids,
        &mut seen_finding_themes,
    );
    artifact.positive_findings = sanitize_review_findings(
        artifact.positive_findings,
        &valid_export_refs,
        snapshot.metadata.active_population_profile,
        &mut seen_finding_ids,
        &mut seen_finding_themes,
    );
    artifact.negative_findings = sanitize_review_findings(
        artifact.negative_findings,
        &valid_export_refs,
        snapshot.metadata.active_population_profile,
        &mut seen_finding_ids,
        &mut seen_finding_themes,
    );
    artifact.follow_up_targets = rebuild_follow_up_targets(snapshot)
        .iter()
        .map(follow_up_target)
        .collect();

    Ok(artifact)
}

fn sanitize_review_findings(
    findings: Vec<ArtifactFinding>,
    valid_export_refs: &BTreeSet<String>,
    active_population: PopulationProfile,
    seen_finding_ids: &mut BTreeSet<String>,
    seen_finding_themes: &mut BTreeSet<String>,
) -> Vec<ArtifactFinding> {
    findings
        .into_iter()
        .filter_map(|finding| {
            sanitize_review_finding(finding, valid_export_refs, active_population)
        })
        .filter(|finding| {
            let finding_id_key = normalize_dedupe_key(&finding.finding_id);
            if !finding_id_key.is_empty() && !seen_finding_ids.insert(finding_id_key) {
                return false;
            }

            let theme_key = format!(
                "{}|{}",
                normalize_dedupe_key(&finding.title),
                normalize_dedupe_key(&finding.summary)
            );
            if theme_key == "|" {
                return true;
            }
            seen_finding_themes.insert(theme_key)
        })
        .collect()
}

fn sanitize_review_finding(
    mut finding: ArtifactFinding,
    valid_export_refs: &BTreeSet<String>,
    active_population: PopulationProfile,
) -> Option<ArtifactFinding> {
    apply_evidence_metadata(&mut finding, active_population);
    if let Some(claim_key) = finding.claim_key.as_deref() {
        let joined = format!("{} {}", finding.title, finding.summary);
        let population = finding
            .active_population_profile
            .unwrap_or(active_population);
        if !validate_claim_text(claim_key, population, &joined).is_empty() {
            return None;
        }
    }

    let evidence_refs = sanitize_evidence_refs(&finding.evidence_refs, valid_export_refs, None);
    let evidence_export_refs = evidence_refs
        .iter()
        .map(|evidence| evidence.export_ref.as_str())
        .collect::<BTreeSet<_>>();
    let counterevidence_refs = sanitize_evidence_refs(
        &finding.counterevidence_refs,
        valid_export_refs,
        Some(&evidence_export_refs),
    );

    finding.evidence_refs = evidence_refs;
    finding.counterevidence_refs = counterevidence_refs;

    (!finding.title.trim().is_empty() || !finding.summary.trim().is_empty()).then_some(finding)
}

fn sanitize_evidence_refs(
    refs: &[ArtifactEvidenceRef],
    valid_export_refs: &BTreeSet<String>,
    excluded_refs: Option<&BTreeSet<&str>>,
) -> Vec<ArtifactEvidenceRef> {
    let mut seen_export_refs = BTreeSet::new();

    refs.iter()
        .filter(|evidence| valid_export_refs.contains(&evidence.export_ref))
        .filter(|evidence| {
            excluded_refs.is_none_or(|excluded| !excluded.contains(evidence.export_ref.as_str()))
        })
        .filter(|evidence| seen_export_refs.insert(evidence.export_ref.clone()))
        .cloned()
        .collect()
}

fn collect_export_refs_from_snapshot(snapshot: &SnapshotBundleV1) -> Result<BTreeSet<String>> {
    fn visit(value: &Value, refs: &mut BTreeSet<String>) {
        match value {
            Value::Object(map) => {
                if let Some(export_ref) = map.get("export_ref").and_then(Value::as_str) {
                    refs.insert(export_ref.to_owned());
                }
                for value in map.values() {
                    visit(value, refs);
                }
            }
            Value::Array(values) => {
                for value in values {
                    visit(value, refs);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }

    let value = serde_json::to_value(snapshot)?;
    let mut refs = BTreeSet::new();
    visit(&value, &mut refs);
    Ok(refs)
}

fn normalize_dedupe_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn render_findings(findings: &[ArtifactFinding]) -> Vec<String> {
    findings
        .iter()
        .enumerate()
        .flat_map(|(index, finding)| {
            let mut lines = vec![
                format!("  {}. {}", index + 1, finding.title),
                format!(
                    "     {} / {}",
                    finding.confidence.as_str(),
                    finding.sufficiency.as_str()
                ),
                finding.evidence_tier.map_or_else(String::new, |tier| {
                    format!("     evidence: {}", tier.chip_label())
                }),
                format!("     {}", finding.summary),
            ];
            if lines[2].is_empty() {
                lines.remove(2);
            }
            if !finding.caution_labels.is_empty() {
                lines.push(format!(
                    "     rails: {}",
                    finding.caution_labels.join(" | ")
                ));
            }
            if !finding.evidence_refs.is_empty() {
                lines.push("     evidence:".to_owned());
                lines.extend(finding.evidence_refs.iter().map(|evidence| {
                    format!("       - {} ({})", evidence.export_ref, evidence.note)
                }));
            }
            if !finding.counterevidence_refs.is_empty() {
                lines.push("     counterevidence:".to_owned());
                lines.extend(finding.counterevidence_refs.iter().map(|evidence| {
                    format!("       - {} ({})", evidence.export_ref, evidence.note)
                }));
            }
            lines
        })
        .collect()
}

fn select_provider(
    config: &AiConfig,
    dry_run: bool,
    fixture: Option<&Path>,
) -> Result<Box<dyn AiProvider + Send + Sync>> {
    if dry_run {
        return Ok(Box::new(DryRunProvider {
            config: config.clone(),
        }));
    }

    if let Some(path) = fixture {
        return Ok(Box::new(FixtureProvider {
            fixture_path: path.into(),
        }));
    }

    if !config.enabled {
        return Err(RingmasterError::Config(
            "AI provider is disabled; enable `[ai].enabled` or use `--dry-run`/`--fixture`"
                .to_owned(),
        ));
    }

    if config.input_transport != AiInputTransport::Inline {
        return Err(RingmasterError::Config(
            "AI input transport `file_upload` is intentionally deferred in this pass; use inline snapshots"
                .to_owned(),
        ));
    }

    Ok(Box::new(OpenAiProvider {
        config: config.clone(),
    }))
}

impl AiProvider for DryRunProvider {
    fn metadata(&self) -> ProviderMetadata {
        ProviderMetadata {
            provider: "dry_run".to_owned(),
            model: "deterministic".to_owned(),
            reasoning_effort: None,
            request_mode: self.config.request_mode.as_str().to_owned(),
            input_transport: self.config.input_transport.as_str().to_owned(),
            run_mode: AiRunMode::DryRun,
        }
    }

    fn review<'a>(
        &'a self,
        request: ReviewProviderRequest<'a>,
    ) -> ProviderFuture<'a, ReviewArtifactV1> {
        Box::pin(async move { Ok(dry_run_review_artifact(request.snapshot)) })
    }

    fn compare<'a>(
        &'a self,
        request: CompareProviderRequest<'a>,
    ) -> ProviderFuture<'a, CompareArtifactV1> {
        Box::pin(async move {
            Ok(dry_run_compare_artifact(
                request.snapshot_a,
                request.snapshot_b,
            ))
        })
    }

    fn follow_up<'a>(
        &'a self,
        request: FollowUpProviderRequest<'a>,
    ) -> ProviderFuture<'a, FollowUpArtifactV1> {
        Box::pin(async move {
            dry_run_follow_up_artifact(
                request.snapshots,
                request.source_artifact_json,
                request.follow_up_kind,
            )
        })
    }
}

impl AiProvider for FixtureProvider {
    fn metadata(&self) -> ProviderMetadata {
        ProviderMetadata {
            provider: "fixture".to_owned(),
            model: "fixture".to_owned(),
            reasoning_effort: None,
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            run_mode: AiRunMode::Fixture,
        }
    }

    fn review<'a>(
        &'a self,
        _request: ReviewProviderRequest<'a>,
    ) -> ProviderFuture<'a, ReviewArtifactV1> {
        Box::pin(async move { read_fixture_artifact::<ReviewArtifactV1>(&self.fixture_path) })
    }

    fn compare<'a>(
        &'a self,
        _request: CompareProviderRequest<'a>,
    ) -> ProviderFuture<'a, CompareArtifactV1> {
        Box::pin(async move { read_fixture_artifact::<CompareArtifactV1>(&self.fixture_path) })
    }

    fn follow_up<'a>(
        &'a self,
        _request: FollowUpProviderRequest<'a>,
    ) -> ProviderFuture<'a, FollowUpArtifactV1> {
        Box::pin(async move { read_fixture_artifact::<FollowUpArtifactV1>(&self.fixture_path) })
    }
}

impl AiProvider for OpenAiProvider {
    fn metadata(&self) -> ProviderMetadata {
        ProviderMetadata {
            provider: "openai".to_owned(),
            model: self.config.model.clone(),
            reasoning_effort: self.config.reasoning_effort.clone(),
            request_mode: self.config.request_mode.as_str().to_owned(),
            input_transport: self.config.input_transport.as_str().to_owned(),
            run_mode: AiRunMode::Real,
        }
    }

    fn review<'a>(
        &'a self,
        request: ReviewProviderRequest<'a>,
    ) -> ProviderFuture<'a, ReviewArtifactV1> {
        Box::pin(async move {
            let plan =
                build_review_request_plan(&self.config, request.snapshot, request.snapshot_json)?;
            self.invoke_structured_output::<ReviewArtifactV1>(plan)
                .await
        })
    }

    fn compare<'a>(
        &'a self,
        request: CompareProviderRequest<'a>,
    ) -> ProviderFuture<'a, CompareArtifactV1> {
        Box::pin(async move {
            let plan = build_compare_request_plan(
                &self.config,
                request.snapshot_a,
                request.snapshot_a_json,
                request.snapshot_b,
                request.snapshot_b_json,
            )?;
            self.invoke_structured_output::<CompareArtifactV1>(plan)
                .await
        })
    }

    fn follow_up<'a>(
        &'a self,
        request: FollowUpProviderRequest<'a>,
    ) -> ProviderFuture<'a, FollowUpArtifactV1> {
        Box::pin(async move {
            let plan = build_follow_up_request_plan(
                &self.config,
                request.snapshots,
                request.source_artifact_kind,
                request.source_artifact_json,
                request.follow_up_kind,
            )?;
            self.invoke_structured_output::<FollowUpArtifactV1>(plan)
                .await
        })
    }
}

impl OpenAiProvider {
    async fn invoke_structured_output<T>(&self, plan: StructuredOutputRequestPlan) -> Result<T>
    where
        T: for<'de> Deserialize<'de> + JsonSchema,
    {
        let api_key = std::env::var(&self.config.api_key_env).map_err(|_| {
            RingmasterError::Config(format!(
                "AI API key is not set in environment variable `{}`",
                self.config.api_key_env
            ))
        })?;
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(self.config.timeout_secs))
            .build()?;

        let mut attempts = 0;
        loop {
            attempts += 1;
            match self.invoke_once::<T>(&client, &api_key, &plan.body).await {
                Ok(artifact) => return Ok(artifact),
                Err(error) if attempts <= self.config.max_retries => {
                    if retryable_error(&error) {
                        continue;
                    }
                    return Err(error);
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn invoke_once<T>(&self, client: &Client, api_key: &str, body: &Value) -> Result<T>
    where
        T: for<'de> Deserialize<'de> + JsonSchema,
    {
        let response = client
            .post(format!(
                "{}/responses",
                self.config.api_base_url.trim_end_matches('/')
            ))
            .bearer_auth(api_key)
            .json(body)
            .send()
            .await
            .map_err(|error| ai_transport_error(&error, self.config.timeout_secs))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(RingmasterError::Ui(format!(
                "OpenAI Responses API request failed with {status}: {body}"
            )));
        }

        let value = response.json::<Value>().await?;
        let response_text = extract_output_text(&value)?;
        serde_json::from_str::<T>(&response_text).map_err(Into::into)
    }
}

fn ai_transport_error(error: &reqwest::Error, timeout_secs: u64) -> RingmasterError {
    if error.is_timeout() {
        return RingmasterError::Ui(ai_timeout_message(timeout_secs));
    }

    let mut detail = error.to_string();
    let mut source = std::error::Error::source(&error);
    while let Some(next) = source {
        if !detail.contains(&next.to_string()) {
            detail.push_str(": ");
            detail.push_str(&next.to_string());
        }
        source = next.source();
    }
    RingmasterError::Ui(format!("OpenAI transport error: {detail}"))
}

fn ai_timeout_message(timeout_secs: u64) -> String {
    format!(
        "OpenAI Responses API request timed out after {timeout_secs}s. Increase `ai.timeout_secs` or set `RINGMASTER_AI_TIMEOUT_SECS` to allow longer snapshot reviews."
    )
}

fn build_review_request_plan(
    config: &AiConfig,
    snapshot: &SnapshotBundleV1,
    snapshot_json: &str,
) -> Result<StructuredOutputRequestPlan> {
    build_structured_output_request::<ReviewArtifactV1>(
        config,
        StructuredOutputRequestSpec {
            task_family: "review",
            snapshots: vec![preview_snapshot("primary", snapshot)],
            content_classes: request_content_classes_from_snapshots([snapshot]),
            includes_notes_or_free_text: request_includes_notes_or_free_text([snapshot]),
            schema_name: "ringmaster_review_artifact",
            prompt_version: REVIEW_PROMPT_VERSION,
            output_schema_version: REVIEW_OUTPUT_SCHEMA_VERSION,
            system_instructions: review_system_prompt(),
            task_framing: review_task_framing(),
            snapshot_payload: format!("snapshot_artifact_json:\n{snapshot_json}\n"),
        },
    )
}

fn build_compare_request_plan(
    config: &AiConfig,
    snapshot_a: &SnapshotBundleV1,
    snapshot_a_json: &str,
    comparison_snapshot: &SnapshotBundleV1,
    comparison_snapshot_json: &str,
) -> Result<StructuredOutputRequestPlan> {
    build_structured_output_request::<CompareArtifactV1>(
        config,
        StructuredOutputRequestSpec {
            task_family: "compare",
            snapshots: vec![
                preview_snapshot("snapshot_a", snapshot_a),
                preview_snapshot("snapshot_b", comparison_snapshot),
            ],
            content_classes: request_content_classes_from_snapshots([
                snapshot_a,
                comparison_snapshot,
            ]),
            includes_notes_or_free_text: request_includes_notes_or_free_text([
                snapshot_a,
                comparison_snapshot,
            ]),
            schema_name: "ringmaster_compare_artifact",
            prompt_version: COMPARE_PROMPT_VERSION,
            output_schema_version: COMPARE_OUTPUT_SCHEMA_VERSION,
            system_instructions: compare_system_prompt(),
            task_framing: compare_task_framing(),
            snapshot_payload: format!(
                "snapshot_a_json:\n{snapshot_a_json}\n\nsnapshot_b_json:\n{comparison_snapshot_json}\n"
            ),
        },
    )
}

fn build_follow_up_request_plan(
    config: &AiConfig,
    snapshots: &[(&SnapshotBundleV1, &str)],
    source_artifact_kind: &str,
    source_artifact_json: &str,
    follow_up_kind: GuidedFollowUpKind,
) -> Result<StructuredOutputRequestPlan> {
    let snapshot_payload = snapshots
        .iter()
        .enumerate()
        .map(|(index, (_, json))| format!("snapshot_{}_json:\n{}\n", index + 1, json))
        .collect::<Vec<_>>()
        .join("\n");

    build_structured_output_request::<FollowUpArtifactV1>(
        config,
        StructuredOutputRequestSpec {
            task_family: "follow_up",
            snapshots: snapshots
                .iter()
                .enumerate()
                .map(|(index, (snapshot, _))| {
                    preview_snapshot(&format!("snapshot_{}", index + 1), snapshot)
                })
                .collect(),
            content_classes: {
                let mut classes = request_content_classes_from_snapshots(
                    snapshots.iter().map(|(snapshot, _)| *snapshot),
                );
                classes.push("stored_artifact_context".to_owned());
                classes
            },
            includes_notes_or_free_text: request_includes_notes_or_free_text(
                snapshots.iter().map(|(snapshot, _)| *snapshot),
            ),
            schema_name: "ringmaster_follow_up_artifact",
            prompt_version: FOLLOW_UP_PROMPT_VERSION,
            output_schema_version: FOLLOW_UP_OUTPUT_SCHEMA_VERSION,
            system_instructions: follow_up_system_prompt(),
            task_framing: follow_up_task_framing(),
            snapshot_payload: format!(
                "follow_up_kind: {}\nsource_artifact_kind: {}\nsource_artifact_json:\n{}\n\n{}",
                follow_up_kind.as_str(),
                source_artifact_kind,
                source_artifact_json,
                snapshot_payload
            ),
        },
    )
}

struct StructuredOutputRequestSpec<'a> {
    task_family: &'a str,
    snapshots: Vec<AiRequestPreviewSnapshot>,
    content_classes: Vec<String>,
    includes_notes_or_free_text: bool,
    schema_name: &'a str,
    prompt_version: &'a str,
    output_schema_version: &'a str,
    system_instructions: &'a str,
    task_framing: &'a str,
    snapshot_payload: String,
}

fn build_structured_output_request<T>(
    config: &AiConfig,
    spec: StructuredOutputRequestSpec<'_>,
) -> Result<StructuredOutputRequestPlan>
where
    T: JsonSchema,
{
    let StructuredOutputRequestSpec {
        task_family,
        snapshots,
        content_classes,
        includes_notes_or_free_text,
        schema_name,
        prompt_version,
        output_schema_version,
        system_instructions,
        task_framing,
        snapshot_payload,
    } = spec;

    let user_prompt = format!("{task_framing}\n\n{snapshot_payload}");
    let prefix_fingerprint = request_fingerprint(
        prompt_version,
        output_schema_version,
        &format!("{system_instructions}\n\n{task_framing}\n\n{schema_name}"),
    );
    let payload_fingerprint = content_fingerprint(snapshot_payload.as_bytes());
    let request_fingerprint = request_fingerprint(
        prompt_version,
        output_schema_version,
        &format!("{task_framing}\n\n{snapshot_payload}"),
    );

    let mut body = json!({
        "model": config.model,
        "store": matches!(config.request_mode, AiRequestMode::Stateful),
        "input": [
            {
                "role": "system",
                "content": [
                    {
                        "type": "input_text",
                        "text": system_instructions
                    }
                ]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "input_text",
                        "text": user_prompt
                    }
                ]
            }
        ],
        "text": {
            "format": {
                "type": "json_schema",
                "name": schema_name,
                "schema": schema_value::<T>()?,
                "strict": true
            }
        }
    });

    if let Some(reasoning_effort) = &config.reasoning_effort {
        body["reasoning"] = json!({ "effort": reasoning_effort });
    }
    if let Some(safety_identifier) = &config.safety_identifier {
        body["safety_identifier"] = json!(safety_identifier);
    }

    Ok(StructuredOutputRequestPlan {
        body,
        preview: AiRequestPreview {
            task_family: task_family.to_owned(),
            provider: "openai".to_owned(),
            model: config.model.clone(),
            request_mode: config.request_mode.as_str().to_owned(),
            input_transport: config.input_transport.as_str().to_owned(),
            prompt_cache: prompt_cache_label(config.prompt_cache).to_owned(),
            prompt_version: prompt_version.to_owned(),
            output_schema_version: output_schema_version.to_owned(),
            snapshots,
            snapshot_bytes: snapshot_payload.len(),
            approximate_input_tokens: snapshot_payload.len().div_ceil(4),
            stateless: matches!(config.request_mode, AiRequestMode::Stateless),
            tools_disabled: true,
            includes_notes_or_free_text,
            content_classes,
            prefix_fingerprint: short_fingerprint(&prefix_fingerprint, 16),
            payload_fingerprint: short_fingerprint(&payload_fingerprint, 16),
            request_fingerprint: short_fingerprint(&request_fingerprint, 16),
        },
        request_fingerprint,
    })
}

#[must_use]
pub fn render_request_preview(preview: &AiRequestPreview) -> String {
    let mut lines = vec![
        "ringmaster ai request preview".to_owned(),
        String::new(),
        format!("task_family: {}", preview.task_family),
        format!("provider: {}", preview.provider),
        format!("model: {}", preview.model),
        format!("request_mode: {}", preview.request_mode),
        format!("input_transport: {}", preview.input_transport),
        format!("prompt_cache: {}", preview.prompt_cache),
        format!("prompt_version: {}", preview.prompt_version),
        format!("output_schema_version: {}", preview.output_schema_version),
        format!("stateless: {}", yes_no(preview.stateless)),
        format!("tools_disabled: {}", yes_no(preview.tools_disabled)),
        format!(
            "includes_notes_or_free_text: {}",
            yes_no(preview.includes_notes_or_free_text)
        ),
        format!("snapshot_bytes: {}", preview.snapshot_bytes),
        format!(
            "approximate_input_tokens: {}",
            preview.approximate_input_tokens
        ),
        format!("prefix_fingerprint: {}", preview.prefix_fingerprint),
        format!("payload_fingerprint: {}", preview.payload_fingerprint),
        format!("request_fingerprint: {}", preview.request_fingerprint),
    ];
    if !preview.content_classes.is_empty() {
        lines.push("content_classes:".to_owned());
        lines.extend(
            preview
                .content_classes
                .iter()
                .map(|content_class| format!("  - {content_class}")),
        );
    }
    if !preview.snapshots.is_empty() {
        lines.push("snapshots:".to_owned());
        lines.extend(preview.snapshots.iter().map(|snapshot| {
            format!(
                "  - {} | hash={} | scope={} | anchor_day={} | privacy={} | population={} | days={}",
                snapshot.label,
                snapshot.snapshot_hash,
                snapshot.scope,
                snapshot.anchor_day,
                snapshot.privacy_profile.as_str(),
                snapshot.active_population_profile.as_str(),
                snapshot.day_count
            )
        }));
    }
    lines.join("\n")
}

fn preview_snapshot(label: &str, snapshot: &SnapshotBundleV1) -> AiRequestPreviewSnapshot {
    AiRequestPreviewSnapshot {
        label: label.to_owned(),
        snapshot_hash: snapshot.metadata.snapshot_hash.clone(),
        scope: snapshot.metadata.scope.clone(),
        anchor_day: snapshot.metadata.anchor_day.clone(),
        privacy_profile: snapshot.metadata.privacy_profile,
        active_population_profile: snapshot.metadata.active_population_profile,
        day_count: day_span_count(&snapshot.metadata.start_day, &snapshot.metadata.end_day),
    }
}

fn apply_provider_metadata_to_preview(preview: &mut AiRequestPreview, metadata: &ProviderMetadata) {
    preview.provider.clone_from(&metadata.provider);
    preview.model.clone_from(&metadata.model);
    preview.request_mode.clone_from(&metadata.request_mode);
    preview
        .input_transport
        .clone_from(&metadata.input_transport);
    preview.stateless = metadata.request_mode == "stateless";
}

fn request_includes_notes_or_free_text<'a>(
    snapshots: impl IntoIterator<Item = &'a SnapshotBundleV1>,
) -> bool {
    snapshots.into_iter().any(|bundle| {
        bundle.context_events.iter().any(|event| {
            event
                .summary
                .as_ref()
                .is_some_and(|summary| !summary.trim().is_empty())
        })
    })
}

fn request_content_classes_from_snapshots<'a>(
    snapshots: impl IntoIterator<Item = &'a SnapshotBundleV1>,
) -> Vec<String> {
    let mut classes = BTreeSet::new();
    for bundle in snapshots {
        if !bundle.metrics.daily_scores.is_empty() {
            classes.insert("daily_scores");
        }
        if !bundle.metrics.activity.is_empty() {
            classes.insert("activity");
        }
        if !bundle.metrics.heartrate_daily_averages.is_empty() {
            classes.insert("heartrate_daily_averages");
        }
        if !bundle.metrics.sleep_windows.is_empty() {
            classes.insert("sleep_windows");
        }
        if !bundle.metrics.stress.is_empty() {
            classes.insert("stress");
        }
        if !bundle.metrics.resilience.is_empty() {
            classes.insert("resilience");
        }
        if !bundle.metrics.cardiovascular_age.is_empty() {
            classes.insert("cardiovascular_age");
        }
        if !bundle.metrics.vo2_max.is_empty() {
            classes.insert("vo2_max");
        }
        if !bundle.metrics.rest_mode_periods.is_empty() {
            classes.insert("rest_mode_periods");
        }
        if !bundle.baselines.is_empty() {
            classes.insert("baselines");
        }
        if !bundle.trend_summaries.is_empty() {
            classes.insert("trend_summaries");
        }
        if !bundle.context_events.is_empty() {
            classes.insert("context_events");
        }
        if !bundle.pattern_summaries.is_empty() {
            classes.insert("pattern_summaries");
        }
        if !bundle.review_signals.is_empty() {
            classes.insert("review_signals");
        }
        if !bundle.follow_up_targets.is_empty() {
            classes.insert("follow_up_targets");
        }
    }
    classes.into_iter().map(str::to_owned).collect()
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn day_span_count(start_day: &str, end_day: &str) -> usize {
    let parse = |value: &str| {
        Date::parse(
            value,
            &time::macros::format_description!("[year]-[month]-[day]"),
        )
    };
    match (parse(start_day), parse(end_day)) {
        (Ok(start), Ok(end)) if end >= start => {
            crate::numeric::i64_to_usize((end - start).whole_days() + 1)
        }
        _ => 0,
    }
}

fn dry_run_review_artifact(snapshot: &SnapshotBundleV1) -> ReviewArtifactV1 {
    let findings = review_findings_from_snapshot(snapshot);
    let positive_findings = findings
        .iter()
        .filter(|finding| {
            finding.summary.contains("improved") || finding.summary.contains("higher")
        })
        .cloned()
        .collect::<Vec<_>>();
    let negative_findings = findings
        .iter()
        .filter(|finding| finding.summary.contains("declined") || finding.summary.contains("lower"))
        .cloned()
        .collect::<Vec<_>>();
    let unresolved_questions = if snapshot.freshness.warnings.is_empty() {
        vec![
            "Which local review target should be checked first to validate the strongest finding?"
                .to_owned(),
        ]
    } else {
        snapshot.freshness.warnings.clone()
    };
    let limitations = vec![ArtifactLimitation {
        code: "dry_run".to_owned(),
        message: "This artifact was rendered locally without contacting an external AI provider."
            .to_owned(),
    }];

    ReviewArtifactV1 {
        schema_version: REVIEW_OUTPUT_SCHEMA_VERSION.to_owned(),
        prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
        status: ArtifactStatus::DryRun,
        overview: format!(
            "Dry-run review for {} covering {} through {} with {} daily score rows.",
            snapshot.metadata.scope,
            snapshot.metadata.start_day,
            snapshot.metadata.end_day,
            snapshot.metrics.daily_scores.len()
        ),
        headline_findings: findings,
        positive_findings,
        negative_findings,
        unresolved_questions,
        limitations,
        follow_up_targets: snapshot
            .follow_up_targets
            .iter()
            .take(4)
            .map(follow_up_target)
            .collect(),
    }
}

fn dry_run_compare_artifact(
    snapshot_a: &SnapshotBundleV1,
    snapshot_b: &SnapshotBundleV1,
) -> CompareArtifactV1 {
    let mut material_differences = Vec::new();
    if let Some(finding) = compare_daily_score_change(snapshot_a, snapshot_b) {
        material_differences.push(finding);
    }
    if let Some(finding) = compare_signal_count_change(snapshot_a, snapshot_b) {
        material_differences.push(finding);
    }
    let only_in_a = unique_context_families(snapshot_a, snapshot_b);
    let only_in_b = unique_context_families(snapshot_b, snapshot_a);
    let supporting_evidence = snapshot_b
        .metrics
        .daily_scores
        .iter()
        .take(2)
        .map(|row| ArtifactEvidenceRef {
            export_ref: row.export_ref.clone(),
            note: format!("Daily score row on {}", row.day),
        })
        .collect::<Vec<_>>();
    let mut uncertainty_warnings = snapshot_a.freshness.warnings.clone();
    uncertainty_warnings.extend(snapshot_b.freshness.warnings.clone());
    uncertainty_warnings.sort();
    uncertainty_warnings.dedup();

    CompareArtifactV1 {
        schema_version: COMPARE_OUTPUT_SCHEMA_VERSION.to_owned(),
        prompt_version: COMPARE_PROMPT_VERSION.to_owned(),
        status: ArtifactStatus::DryRun,
        overview: format!(
            "Dry-run comparison between {} and {}.",
            snapshot_a.metadata.scope, snapshot_b.metadata.scope
        ),
        material_differences,
        supporting_evidence,
        uncertainty_warnings,
        investigation_targets: snapshot_b
            .follow_up_targets
            .iter()
            .take(4)
            .map(follow_up_target)
            .collect(),
        only_in_a,
        only_in_b,
    }
}

fn dry_run_follow_up_artifact(
    snapshots: &[(&SnapshotBundleV1, &str)],
    source_artifact_json: &str,
    follow_up_kind: GuidedFollowUpKind,
) -> Result<FollowUpArtifactV1> {
    let source_snapshot = snapshots
        .first()
        .map(|(snapshot, _)| *snapshot)
        .ok_or_else(|| {
            RingmasterError::Ui("follow-up requests require at least one snapshot".to_owned())
        })?;
    let stored_artifact = match serde_json::from_str::<ReviewArtifactV1>(source_artifact_json) {
        Ok(review) => StoredArtifact::Review(review),
        Err(_) => match serde_json::from_str::<CompareArtifactV1>(source_artifact_json) {
            Ok(compare) => StoredArtifact::Compare(compare),
            Err(_) => StoredArtifact::FollowUp(serde_json::from_str::<FollowUpArtifactV1>(
                source_artifact_json,
            )?),
        },
    };

    let (overview, focal_findings, reasoning_steps, unresolved_questions) = match (&stored_artifact, follow_up_kind) {
        (StoredArtifact::Review(review), GuidedFollowUpKind::ExpandEvidence) => (
            format!("Expanded evidence for the saved review over {}.", source_snapshot.metadata.scope),
            review
                .headline_findings
                .iter()
                .chain(review.positive_findings.iter())
                .chain(review.negative_findings.iter())
                .take(3)
                .cloned()
                .collect(),
            vec![
                "Replayed the saved review findings against the exported snapshot payload.".to_owned(),
                "Kept the strongest evidence refs visible so the next local drill-down stays explicit.".to_owned(),
            ],
            review.unresolved_questions.clone(),
        ),
        (StoredArtifact::Review(review), GuidedFollowUpKind::ShowCounterevidence) => (
            "Surfaced the strongest counterevidence already present in the saved review artifact.".to_owned(),
            review
                .headline_findings
                .iter()
                .chain(review.positive_findings.iter())
                .chain(review.negative_findings.iter())
                .filter(|finding| !finding.counterevidence_refs.is_empty())
                .take(3)
                .cloned()
                .collect(),
            vec![
                "Prioritized findings that already contain explicit counterevidence refs.".to_owned(),
                "If a claim had no stored counterevidence, it was left out rather than padded.".to_owned(),
            ],
            review.unresolved_questions.clone(),
        ),
        (StoredArtifact::Review(review), GuidedFollowUpKind::ExplainRanking) => (
            "Explained why the saved review findings ranked the way they did.".to_owned(),
            review
                .headline_findings
                .iter()
                .take(3)
                .cloned()
                .collect(),
            vec![
                "Headline findings stay first because they combine stronger sufficiency with explicit evidence refs.".to_owned(),
                "Positive and negative findings remain secondary when their evidence is thinner or less recent.".to_owned(),
            ],
            review.unresolved_questions.clone(),
        ),
        (StoredArtifact::Review(review), GuidedFollowUpKind::SuggestLocalDrilldown) => (
            "Suggested the next local investigation targets from the saved review.".to_owned(),
            review.headline_findings.iter().take(2).cloned().collect(),
            vec![
                "Recommended local drill-downs stay inside Review, Explain, Patterns, and Timeline.".to_owned(),
            ],
            review.unresolved_questions.clone(),
        ),
        (StoredArtifact::Compare(compare), GuidedFollowUpKind::ExpandEvidence) => (
            "Expanded evidence for the saved comparison artifact.".to_owned(),
            compare.material_differences.iter().take(3).cloned().collect(),
            vec![
                "Material differences remain the anchor for compare follow-ups.".to_owned(),
                "Supporting evidence from both compared windows stays attached to each finding.".to_owned(),
            ],
            compare.uncertainty_warnings.clone(),
        ),
        (StoredArtifact::Compare(compare), GuidedFollowUpKind::ShowCounterevidence) => (
            "Surfaced the strongest counterevidence captured in the saved comparison artifact.".to_owned(),
            compare
                .material_differences
                .iter()
                .filter(|finding| !finding.counterevidence_refs.is_empty())
                .take(3)
                .cloned()
                .collect(),
            vec![
                "Counterevidence stays tied to the original comparison findings, not ad hoc prose.".to_owned(),
            ],
            compare.uncertainty_warnings.clone(),
        ),
        (StoredArtifact::Compare(compare), GuidedFollowUpKind::ExplainRanking) => (
            "Explained the ranking of material differences in the saved comparison.".to_owned(),
            compare.material_differences.iter().take(3).cloned().collect(),
            vec![
                "Findings with stronger evidence and clearer cross-window deltas rank first.".to_owned(),
                "Thin or stale comparisons remain lower-ranked and carry their uncertainty forward.".to_owned(),
            ],
            compare.uncertainty_warnings.clone(),
        ),
        (StoredArtifact::Compare(compare), GuidedFollowUpKind::SuggestLocalDrilldown) => (
            "Suggested the next local investigation targets from the saved comparison.".to_owned(),
            compare.material_differences.iter().take(2).cloned().collect(),
            vec![
                "Comparison follow-ups stay local by routing into the underlying day or week views.".to_owned(),
            ],
            compare.uncertainty_warnings.clone(),
        ),
        (StoredArtifact::FollowUp(follow_up), _) => (
            format!("Extended the saved {} follow-up with another bounded pass.", follow_up.follow_up_kind.as_str()),
            follow_up.focal_findings.iter().take(3).cloned().collect(),
            follow_up.reasoning_steps.clone(),
            follow_up.unresolved_questions.clone(),
        ),
    };

    let suggested_local_targets = match &stored_artifact {
        StoredArtifact::Review(review) => review.follow_up_targets.clone(),
        StoredArtifact::Compare(compare) => compare.investigation_targets.clone(),
        StoredArtifact::FollowUp(follow_up) => follow_up.suggested_local_targets.clone(),
    };

    Ok(FollowUpArtifactV1 {
        schema_version: FOLLOW_UP_OUTPUT_SCHEMA_VERSION.to_owned(),
        prompt_version: FOLLOW_UP_PROMPT_VERSION.to_owned(),
        status: ArtifactStatus::DryRun,
        follow_up_kind,
        overview,
        focal_findings,
        reasoning_steps,
        unresolved_questions,
        suggested_local_targets,
    })
}

fn review_findings_from_snapshot(snapshot: &SnapshotBundleV1) -> Vec<ArtifactFinding> {
    let mut findings = snapshot
        .trend_summaries
        .iter()
        .take(3)
        .map(|summary| {
            let descriptor = summary.evidence.clone().or_else(|| {
                resolve_evidence_descriptor(
                    &summary.metric_key,
                    snapshot.metadata.active_population_profile,
                )
            });
            ArtifactFinding {
                finding_id: finding_id(&summary.metric_key, &summary.label),
                title: summary.label.clone(),
                summary: trend_summary_text(summary.label.as_str(), summary.direction.as_str()),
                claim_key: Some(summary.metric_key.clone()),
                evidence_tier: descriptor.as_ref().map(|value| value.evidence_tier),
                interpretation_scope: descriptor.as_ref().map(|value| value.interpretation_scope),
                active_population_profile: descriptor
                    .as_ref()
                    .map(|value| value.active_population_profile),
                population_support_status: descriptor
                    .as_ref()
                    .map(|value| value.population_support_status),
                fallback_population_profile: descriptor
                    .as_ref()
                    .and_then(|value| value.fallback_population_profile),
                caution_labels: descriptor
                    .as_ref()
                    .map(|value| {
                        value
                            .caution_flags
                            .iter()
                            .map(|flag| flag.label().to_owned())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
                confidence: if summary.current_average.is_some()
                    && summary.previous_average.is_some()
                {
                    ConfidenceLevel::Medium
                } else {
                    ConfidenceLevel::Low
                },
                sufficiency: if summary.current_average.is_some()
                    && summary.previous_average.is_some()
                {
                    SufficiencyLevel::Medium
                } else {
                    SufficiencyLevel::Thin
                },
                evidence_refs: snapshot
                    .metrics
                    .daily_scores
                    .iter()
                    .take(2)
                    .map(|row| ArtifactEvidenceRef {
                        export_ref: row.export_ref.clone(),
                        note: format!("Daily score row on {}", row.day),
                    })
                    .collect(),
                counterevidence_refs: Vec::new(),
            }
        })
        .collect::<Vec<_>>();

    if findings.is_empty() {
        findings.push(ArtifactFinding {
            finding_id: finding_id("insufficient", &snapshot.metadata.scope),
            title: "Insufficient direct trend evidence".to_owned(),
            summary: "The snapshot did not contain enough trend data to derive a stronger dry-run finding.".to_owned(),
            claim_key: None,
            evidence_tier: None,
            interpretation_scope: None,
            active_population_profile: None,
            population_support_status: None,
            fallback_population_profile: None,
            caution_labels: Vec::new(),
            confidence: ConfidenceLevel::Low,
            sufficiency: SufficiencyLevel::Missing,
            evidence_refs: Vec::new(),
            counterevidence_refs: Vec::new(),
        });
    }

    findings
}

fn apply_evidence_metadata(finding: &mut ArtifactFinding, active_population: PopulationProfile) {
    let claim_key = finding
        .claim_key
        .as_deref()
        .filter(|claim_key| resolve_evidence_descriptor(claim_key, active_population).is_some())
        .map(str::to_owned)
        .or_else(|| {
            let inferred = finding.finding_id.split(':').next().unwrap_or_default();
            resolve_evidence_descriptor(inferred, active_population).map(|_| inferred.to_owned())
        });

    let Some(claim_key) = claim_key else {
        return;
    };
    let Some(descriptor) = resolve_evidence_descriptor(&claim_key, active_population) else {
        return;
    };

    finding.claim_key = Some(claim_key.clone());
    finding.evidence_tier = Some(descriptor.evidence_tier);
    finding.interpretation_scope = Some(descriptor.interpretation_scope);
    finding.active_population_profile = Some(descriptor.active_population_profile);
    finding.population_support_status = Some(descriptor.population_support_status);
    finding.fallback_population_profile = descriptor.fallback_population_profile;
    if finding.caution_labels.is_empty() {
        finding.caution_labels = evidence_badges(&claim_key, active_population)
            .into_iter()
            .skip(2)
            .collect();
    }
}

fn compare_daily_score_change(
    snapshot_a: &SnapshotBundleV1,
    snapshot_b: &SnapshotBundleV1,
) -> Option<ArtifactFinding> {
    let average_a = average_scores(&snapshot_a.metrics.daily_scores);
    let average_b = average_scores(&snapshot_b.metrics.daily_scores);
    average_a
        .zip(average_b)
        .map(|(left, right)| ArtifactFinding {
            finding_id: finding_id("daily_scores", "average"),
            title: "Average daily score change".to_owned(),
            summary: format!(
                "The average combined daily score shifted from {left:.1} to {right:.1}."
            ),
            claim_key: None,
            evidence_tier: None,
            interpretation_scope: None,
            active_population_profile: None,
            population_support_status: None,
            fallback_population_profile: None,
            caution_labels: Vec::new(),
            confidence: ConfidenceLevel::Medium,
            sufficiency: SufficiencyLevel::Medium,
            evidence_refs: snapshot_b
                .metrics
                .daily_scores
                .iter()
                .take(2)
                .map(|row| ArtifactEvidenceRef {
                    export_ref: row.export_ref.clone(),
                    note: format!("Comparison window row on {}", row.day),
                })
                .collect(),
            counterevidence_refs: snapshot_a
                .metrics
                .daily_scores
                .iter()
                .take(2)
                .map(|row| ArtifactEvidenceRef {
                    export_ref: row.export_ref.clone(),
                    note: format!("Base window row on {}", row.day),
                })
                .collect(),
        })
}

fn compare_signal_count_change(
    snapshot_a: &SnapshotBundleV1,
    snapshot_b: &SnapshotBundleV1,
) -> Option<ArtifactFinding> {
    let count_a = snapshot_a.review_signals.len();
    let count_b = snapshot_b.review_signals.len();
    if count_a == count_b {
        return None;
    }

    Some(ArtifactFinding {
        finding_id: finding_id("review_signal_count", "count"),
        title: "Structured signal count changed".to_owned(),
        summary: format!(
            "Review signal coverage changed from {count_a} signal rows to {count_b} signal rows."
        ),
        claim_key: None,
        evidence_tier: None,
        interpretation_scope: None,
        active_population_profile: None,
        population_support_status: None,
        fallback_population_profile: None,
        caution_labels: Vec::new(),
        confidence: ConfidenceLevel::Low,
        sufficiency: SufficiencyLevel::Thin,
        evidence_refs: snapshot_b
            .review_signals
            .iter()
            .take(2)
            .map(signal_evidence)
            .collect(),
        counterevidence_refs: snapshot_a
            .review_signals
            .iter()
            .take(2)
            .map(signal_evidence)
            .collect(),
    })
}

fn unique_context_families(
    primary: &SnapshotBundleV1,
    secondary: &SnapshotBundleV1,
) -> Vec<String> {
    let secondary_families = secondary
        .context_events
        .iter()
        .map(|event| event.family.clone())
        .collect::<std::collections::BTreeSet<_>>();
    primary
        .context_events
        .iter()
        .map(|event| event.family.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .difference(&secondary_families)
        .cloned()
        .collect()
}

fn signal_evidence(signal: &SnapshotReviewSignal) -> ArtifactEvidenceRef {
    ArtifactEvidenceRef {
        export_ref: signal.export_ref.clone(),
        note: format!("Review signal `{}` on {}", signal.signal_key, signal.day),
    }
}

fn read_fixture_artifact<T>(path: &Path) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let raw_json = fs::read_to_string(path)
        .map_err(|error| RingmasterError::io("reading AI fixture", error))?;
    serde_json::from_str(&raw_json).map_err(Into::into)
}

fn schema_value<T>() -> Result<Value>
where
    T: JsonSchema,
{
    serde_json::to_value(schema_for!(T)).map_err(Into::into)
}

fn extract_output_text(value: &Value) -> Result<String> {
    let output_items = value
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RingmasterError::Ui("OpenAI response did not include `output`".to_owned())
        })?;
    for item in output_items {
        if item.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        let Some(content_items) = item.get("content").and_then(Value::as_array) else {
            continue;
        };
        for content in content_items {
            if content.get("type").and_then(Value::as_str) == Some("output_text")
                && let Some(text) = content.get("text").and_then(Value::as_str)
            {
                return Ok(text.to_owned());
            }
        }
    }

    Err(RingmasterError::Ui(
        "OpenAI response did not contain structured output text".to_owned(),
    ))
}

fn retryable_error(error: &RingmasterError) -> bool {
    match error {
        RingmasterError::Transport(source) => {
            source.is_timeout()
                || source.is_connect()
                || source.status().is_some_and(retryable_status_code)
        }
        RingmasterError::Ui(message) => retryable_status_codes()
            .iter()
            .any(|status| message.contains(status.as_str())),
        RingmasterError::Config(_)
        | RingmasterError::Cli(_)
        | RingmasterError::Io { .. }
        | RingmasterError::ConfigParse(_)
        | RingmasterError::Json(_)
        | RingmasterError::Storage(_)
        | RingmasterError::Auth(_)
        | RingmasterError::OuraApi(_) => false,
    }
}

fn retryable_status_code(status: StatusCode) -> bool {
    retryable_status_codes().contains(&status)
}

const fn retryable_status_codes() -> &'static [StatusCode] {
    &[
        StatusCode::REQUEST_TIMEOUT,
        StatusCode::TOO_MANY_REQUESTS,
        StatusCode::INTERNAL_SERVER_ERROR,
        StatusCode::BAD_GATEWAY,
        StatusCode::SERVICE_UNAVAILABLE,
        StatusCode::GATEWAY_TIMEOUT,
    ]
}

fn artifact_id(
    kind: &str,
    snapshot_hash_a: &str,
    snapshot_hash_b: Option<&str>,
    run_mode: AiRunMode,
    run_identity: &str,
    payload_json: &str,
) -> String {
    let mut digest = Sha256::new();
    digest.update(kind.as_bytes());
    digest.update(snapshot_hash_a.as_bytes());
    if let Some(value) = snapshot_hash_b {
        digest.update(value.as_bytes());
    }
    digest.update(run_mode.as_str().as_bytes());
    digest.update(run_identity.as_bytes());
    digest.update(payload_json.as_bytes());
    hex::encode(digest.finalize())
}

fn resolve_run_created_at(run_identity_override: Option<&str>) -> Result<String> {
    if let Some(run_identity) = run_identity_override {
        return Ok(run_identity.to_owned());
    }
    OffsetDateTime::now_utc().format(&Rfc3339).map_err(|error| {
        RingmasterError::Config(format!("failed to format AI run timestamp: {error}"))
    })
}

fn request_fingerprint(prompt_version: &str, schema_version: &str, payload: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(prompt_version.as_bytes());
    digest.update(schema_version.as_bytes());
    digest.update(payload.as_bytes());
    hex::encode(digest.finalize())
}

fn content_fingerprint(contents: &[u8]) -> String {
    hex::encode(Sha256::digest(contents))
}

fn short_fingerprint(value: &str, len: usize) -> String {
    value.chars().take(len).collect()
}

const fn prompt_cache_label(mode: PromptCacheMode) -> &'static str {
    match mode {
        PromptCacheMode::Off => "off",
        PromptCacheMode::Auto => "auto",
    }
}

fn review_summary_cache(artifact: &ReviewArtifactV1) -> String {
    artifact.headline_findings.first().map_or_else(
        || artifact.overview.clone(),
        |finding| format!("{}: {}", finding.title, finding.summary),
    )
}

fn compare_summary_cache(artifact: &CompareArtifactV1) -> String {
    artifact.material_differences.first().map_or_else(
        || artifact.overview.clone(),
        |finding| format!("{}: {}", finding.title, finding.summary),
    )
}

fn follow_up_summary_cache(artifact: &FollowUpArtifactV1) -> String {
    artifact.focal_findings.first().map_or_else(
        || artifact.overview.clone(),
        |finding| format!("{}: {}", finding.title, finding.summary),
    )
}

const fn merged_privacy_profile(left: PrivacyProfile, right: PrivacyProfile) -> PrivacyProfile {
    use PrivacyProfile::{Balanced, Full, Redacted};
    match (left, right) {
        (Full, _) | (_, Full) => Full,
        (Balanced, _) | (_, Balanced) => Balanced,
        (Redacted, Redacted) => Redacted,
    }
}

fn merged_snapshot_privacy_profile(snapshots: &[LoadedSnapshotArtifact]) -> Option<PrivacyProfile> {
    snapshots
        .iter()
        .map(|snapshot| snapshot.bundle.metadata.privacy_profile)
        .reduce(merged_privacy_profile)
}

fn follow_up_snapshot_hashes(snapshots: &[LoadedSnapshotArtifact]) -> Result<(&str, Option<&str>)> {
    let snapshot_hash_a = snapshots
        .first()
        .map(|snapshot| snapshot.bundle.metadata.snapshot_hash.as_str())
        .ok_or_else(|| {
            RingmasterError::Ui(
                "follow-up AI runs require at least one preflight snapshot input".to_owned(),
            )
        })?;
    let snapshot_hash_b = snapshots
        .get(1)
        .map(|snapshot| snapshot.bundle.metadata.snapshot_hash.as_str());
    Ok((snapshot_hash_a, snapshot_hash_b))
}

fn parse_privacy_profile(value: &str) -> Result<PrivacyProfile> {
    match value {
        "redacted" => Ok(PrivacyProfile::Redacted),
        "balanced" => Ok(PrivacyProfile::Balanced),
        "full" => Ok(PrivacyProfile::Full),
        other => Err(RingmasterError::Ui(format!(
            "unsupported privacy profile `{other}` in saved AI artifact metadata"
        ))),
    }
}

fn follow_up_target(target: &SnapshotFollowUpTarget) -> ArtifactFollowUpTarget {
    ArtifactFollowUpTarget {
        label: target.label.clone(),
        command: target.command.clone(),
        reason: target.reason.clone(),
    }
}

fn average_scores(rows: &[crate::snapshot::SnapshotDailyScore]) -> Option<f64> {
    let values = rows
        .iter()
        .filter_map(|row| {
            let mut parts = Vec::new();
            if let Some(value) = row.sleep_score {
                parts.push(f64::from(value));
            }
            if let Some(value) = row.readiness_score {
                parts.push(f64::from(value));
            }
            if let Some(value) = row.activity_score {
                parts.push(f64::from(value));
            }
            if parts.is_empty() {
                None
            } else {
                Some(parts.iter().sum::<f64>() / crate::numeric::usize_to_f64(parts.len()))
            }
        })
        .collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / crate::numeric::usize_to_f64(values.len()))
    }
}

fn trend_summary_text(label: &str, direction: &str) -> String {
    match direction {
        "higher" => {
            format!("{label} improved in the in-scope window relative to the comparison window.")
        }
        "lower" => {
            format!("{label} declined in the in-scope window relative to the comparison window.")
        }
        "flat" => format!("{label} stayed roughly flat across the compared windows."),
        _ => format!(
            "{label} did not have enough evidence for a stronger deterministic dry-run statement."
        ),
    }
}

fn finding_id(metric_key: &str, label: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(metric_key.as_bytes());
    digest.update(label.as_bytes());
    hex::encode(digest.finalize())[..16].to_owned()
}

impl AiRunMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Real => "real",
            Self::DryRun => "dry_run",
            Self::Fixture => "fixture",
        }
    }
}

impl ArtifactStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Insufficient => "insufficient",
            Self::DryRun => "dry_run",
            Self::Fixture => "fixture",
        }
    }
}

impl AiRunStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

impl GuidedFollowUpKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExpandEvidence => "expand_evidence",
            Self::ShowCounterevidence => "show_counterevidence",
            Self::ExplainRanking => "explain_ranking",
            Self::SuggestLocalDrilldown => "suggest_local_drilldown",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ExpandEvidence => "Expand evidence",
            Self::ShowCounterevidence => "Show strongest counterevidence",
            Self::ExplainRanking => "Explain ranking",
            Self::SuggestLocalDrilldown => "Suggest next local drill-down",
        }
    }
}

impl ConfidenceLevel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl SufficiencyLevel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Thin => "thin",
            Self::Medium => "medium",
            Self::Strong => "strong",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use reqwest::StatusCode;
    use sha2::Digest;

    use super::{
        ArtifactEvidenceRef, ArtifactFinding, ArtifactStatus, COMPARE_OUTPUT_SCHEMA_VERSION,
        COMPARE_PROMPT_VERSION, CompareArtifactV1, FOLLOW_UP_OUTPUT_SCHEMA_VERSION,
        FOLLOW_UP_PROMPT_VERSION, GuidedFollowUpKind, REVIEW_OUTPUT_SCHEMA_VERSION,
        REVIEW_PROMPT_VERSION, ReviewArtifactV1, SufficiencyLevel, artifact_id,
        build_review_request_plan, dry_run_compare_artifact, dry_run_review_artifact,
        follow_up_from_artifact_with_run_identity, render_compare_briefing, render_request_preview,
        render_review_briefing, retryable_error, review_snapshot, sanitize_review_artifact,
        schema_value,
    };
    use crate::config::{
        AiConfig, AiInputTransport, AiProviderKind, AiRequestMode, AppPaths, Config, LoggingConfig,
        OuraConfig, OuraSecretBackend, RefreshConfig, WebhookConfig,
    };
    use crate::error::RingmasterError;
    use crate::evidence::{PopulationProfile, PopulationSupportStatus};
    use crate::snapshot::{
        PrivacyProfile, SnapshotBundleV1, SnapshotCapabilities, SnapshotCapabilityEntry,
        SnapshotContextEvent, SnapshotFreshness, SnapshotMetadata, SnapshotMetrics,
        SnapshotRecordCounts, SnapshotReviewSignal, SnapshotSourceMode, SnapshotSyncState,
        deserialize_snapshot_bundle, rebuild_follow_up_targets,
    };
    use crate::store::queries::AiArtifactRecord;

    fn snapshot_bundle(scope: &str) -> SnapshotBundleV1 {
        let mut bundle = SnapshotBundleV1 {
            schema_version: crate::snapshot::SNAPSHOT_SCHEMA_VERSION.to_owned(),
            metadata: SnapshotMetadata {
                app_version: "0.1.0".to_owned(),
                generated_at: "2026-04-10T00:00:00Z".to_owned(),
                snapshot_hash: String::new(),
                scope: scope.to_owned(),
                start_day: "2026-04-08".to_owned(),
                end_day: "2026-04-10".to_owned(),
                anchor_day: "2026-04-10".to_owned(),
                privacy_profile: PrivacyProfile::Redacted,
                active_population_profile: PopulationProfile::GeneralAdult,
                source_mode: SnapshotSourceMode::Demo,
                schema_version: 13,
                evidence_registry_version: crate::evidence::registry::evidence_registry_version()
                    .to_owned(),
            },
            freshness: SnapshotFreshness {
                latest_source_day: Some("2026-04-10".to_owned()),
                latest_review_day: Some("2026-04-10".to_owned()),
                warnings: Vec::new(),
                sync_states: vec![SnapshotSyncState {
                    sync_key: "daily".to_owned(),
                    status: "success".to_owned(),
                    last_attempted_at: "2026-04-10T00:00:00Z".to_owned(),
                    last_completed_at: Some("2026-04-10T00:00:00Z".to_owned()),
                    failure_count: 0,
                    next_attempt_after: None,
                    message: None,
                }],
            },
            capabilities: SnapshotCapabilities {
                requested_scopes: vec!["daily".to_owned()],
                granted_scopes: vec!["daily".to_owned()],
                missing_scopes: Vec::new(),
                entries: vec![SnapshotCapabilityEntry {
                    key: "daily".to_owned(),
                    label: "Daily".to_owned(),
                    requested: true,
                    granted: true,
                    note: "available".to_owned(),
                }],
            },
            record_counts: SnapshotRecordCounts {
                daily_history_days: 1,
                heartrate_days: 0,
                context_events: 1,
                pattern_summaries: 0,
                review_signals: 1,
                raw_tables: BTreeMap::default(),
            },
            metrics: SnapshotMetrics {
                daily_scores: vec![crate::snapshot::SnapshotDailyScore {
                    export_ref: "daily:2026-04-10".to_owned(),
                    day: "2026-04-10".to_owned(),
                    sleep_duration_seconds: Some(28_800),
                    sleep_score: Some(84),
                    readiness_score: Some(80),
                    activity_score: Some(78),
                }],
                activity: Vec::new(),
                heartrate_daily_averages: vec![crate::snapshot::SnapshotMetricPoint {
                    export_ref: "heartrate:2026-04-10".to_owned(),
                    day: "2026-04-10".to_owned(),
                    value: Some(58.0),
                }],
                sleep_windows: Vec::new(),
                stress: Vec::new(),
                resilience: Vec::new(),
                cardiovascular_age: Vec::new(),
                vo2_max: Vec::new(),
                rest_mode_periods: Vec::new(),
            },
            baselines: Vec::new(),
            trend_summaries: vec![crate::snapshot::SnapshotTrendSummary {
                metric_key: "sleep_score".to_owned(),
                label: "Sleep score".to_owned(),
                direction: "higher".to_owned(),
                summary: "Sleep score improved.".to_owned(),
                current_average: Some(84.0),
                previous_average: Some(79.0),
                evidence: crate::evidence::registry::evidence_descriptor("sleep_score"),
            }],
            context_events: vec![SnapshotContextEvent {
                export_ref: "context:1".to_owned(),
                anchor_day: "2026-04-10".to_owned(),
                family: "Workout".to_owned(),
                label: "Workout".to_owned(),
                subtype: Some("run".to_owned()),
                intensity: Some("medium".to_owned()),
                summary: None,
            }],
            pattern_summaries: Vec::new(),
            review_signals: vec![SnapshotReviewSignal {
                export_ref: "signal:sleep:2026-04-10".to_owned(),
                day: "2026-04-10".to_owned(),
                signal_key: "sleep_consistency".to_owned(),
                numeric_value: Some(1.0),
                text_value: None,
                delta: Some(3.0),
                z_score: Some(1.4),
                persistence_days: 2,
                sufficiency: "medium".to_owned(),
                stale_days: 0,
                evidence: crate::evidence::registry::evidence_descriptor("sleep_time_status"),
            }],
            follow_up_targets: vec![crate::snapshot::SnapshotFollowUpTarget {
                label: "Review today".to_owned(),
                command: "review today --day 2026-04-10".to_owned(),
                reason: "Inspect the local brief.".to_owned(),
            }],
        };
        let canonical_without_hash = serde_json::to_string(&bundle)
            .unwrap_or_else(|error| unreachable!("bundle should encode: {error}"));
        let canonical_without_hash = serde_json::to_string(
            &serde_json::from_str::<serde_json::Value>(&canonical_without_hash)
                .unwrap_or_else(|error| unreachable!("bundle json should normalize: {error}")),
        )
        .unwrap_or_else(|error| unreachable!("bundle json should re-encode: {error}"));
        bundle.metadata.snapshot_hash =
            hex::encode(sha2::Sha256::digest(canonical_without_hash.as_bytes()));
        bundle
    }

    fn loaded_snapshot(scope: &str) -> crate::snapshot::LoadedSnapshotArtifact {
        let bundle = snapshot_bundle(scope);
        let compact_json = serde_json::to_string(&bundle)
            .unwrap_or_else(|error| unreachable!("bundle should encode: {error}"));
        crate::snapshot::LoadedSnapshotArtifact {
            bundle,
            compact_json,
        }
    }

    fn test_config() -> Config {
        Config {
            app_name: "ringmaster",
            paths: AppPaths::from_roots(
                PathBuf::from("/home/tester"),
                PathBuf::from("/tmp/config"),
                PathBuf::from("/tmp/state"),
                PathBuf::from("/tmp/cache"),
            )
            .unwrap_or_else(|error| unreachable!("paths should resolve: {error}")),
            logging: LoggingConfig {
                filter: "ringmaster=info".to_owned(),
            },
            oura: OuraConfig {
                client_id: Some("test-client".to_owned()),
                client_secret: None,
                authorize_url: "https://example.invalid/auth".to_owned(),
                token_url: "https://example.invalid/token".to_owned(),
                api_base_url: "https://example.invalid/api".to_owned(),
                secret_backend: OuraSecretBackend::Keyring,
                secret_file: PathBuf::from("/tmp/state/ringmaster/secrets/oura-tokens.json"),
                callback_bind: "127.0.0.1:8788"
                    .parse()
                    .unwrap_or_else(|error| unreachable!("socket address should parse: {error}")),
                callback_path: "/callback".to_owned(),
                requested_scopes: vec!["daily".to_owned()],
                auth_timeout_secs: 120,
            },
            refresh: RefreshConfig {
                personal_interval_secs: 3_600,
                daily_interval_secs: 300,
                heartrate_interval_secs: 60,
                workout_interval_secs: 600,
                enhanced_tag_interval_secs: 300,
                session_interval_secs: 300,
                personal_stale_after_secs: 259_200,
                daily_stale_after_secs: 43_200,
                heartrate_stale_after_secs: 900,
                workout_stale_after_secs: 86_400,
                enhanced_tag_stale_after_secs: 43_200,
                session_stale_after_secs: 43_200,
                daily_history_days: 30,
                daily_overlap_days: 7,
                heartrate_history_days: 14,
                heartrate_overlap_minutes: 180,
                workout_history_days: 30,
                workout_overlap_days: 7,
                enhanced_tag_history_days: 30,
                enhanced_tag_overlap_days: 7,
                session_history_days: 30,
                session_overlap_days: 7,
                max_backoff_secs: 3_600,
                demo_fixture_dir: None,
            },
            webhook: WebhookConfig {
                bind: "127.0.0.1:8799"
                    .parse()
                    .unwrap_or_else(|error| unreachable!("socket address should parse: {error}")),
                path: "/webhooks/oura".to_owned(),
                public_base_url: None,
                verification_token: None,
                signature_tolerance_secs: 300,
                heartbeat_secs: 30,
                renewal_lead_secs: 86_400,
                subscriptions: Vec::new(),
            },
            guidance: crate::config::GuidanceConfig::default(),
            ai: AiConfig {
                enabled: false,
                provider: AiProviderKind::OpenAi,
                api_base_url: "https://api.openai.com/v1".to_owned(),
                api_key_env: "OPENAI_API_KEY".to_owned(),
                model: "gpt-5-mini".to_owned(),
                reasoning_effort: None,
                timeout_secs: 120,
                max_retries: 1,
                request_mode: AiRequestMode::Stateless,
                input_transport: AiInputTransport::Inline,
                prompt_cache: crate::config::PromptCacheMode::Off,
                safety_identifier: None,
            },
        }
    }

    #[test]
    fn dry_run_review_is_versioned_and_renderable() {
        let artifact = dry_run_review_artifact(&snapshot_bundle("today"));
        assert_eq!(artifact.schema_version, REVIEW_OUTPUT_SCHEMA_VERSION);
        assert_eq!(artifact.prompt_version, REVIEW_PROMPT_VERSION);
        assert_eq!(artifact.status, ArtifactStatus::DryRun);
        let rendered = render_review_briefing(&artifact);
        assert!(rendered.contains("ringmaster ai review"));
    }

    #[test]
    fn dry_run_review_preserves_claim_backed_findings_after_sanitization() {
        let artifact = dry_run_review_artifact(&snapshot_bundle("today"));

        assert!(!artifact.headline_findings.is_empty());
        assert!(
            artifact
                .headline_findings
                .iter()
                .any(|finding| finding.claim_key.as_deref() == Some("sleep_score"))
        );
    }

    #[test]
    fn dry_run_compare_is_versioned_and_renderable() {
        let artifact = dry_run_compare_artifact(
            &snapshot_bundle("week"),
            &snapshot_bundle("range:2026-04-08..2026-04-10"),
        );
        assert_eq!(artifact.schema_version, COMPARE_OUTPUT_SCHEMA_VERSION);
        assert_eq!(artifact.prompt_version, COMPARE_PROMPT_VERSION);
        let rendered = render_compare_briefing(&artifact);
        assert!(rendered.contains("ringmaster ai compare"));
    }

    #[test]
    fn dry_run_review_marks_missing_trends_as_insufficient() {
        let mut snapshot = snapshot_bundle("today");
        snapshot.trend_summaries.clear();
        snapshot.metrics.daily_scores.clear();

        let artifact = dry_run_review_artifact(&snapshot);

        assert_eq!(artifact.headline_findings.len(), 1);
        assert_eq!(
            artifact.headline_findings[0].sufficiency,
            SufficiencyLevel::Missing
        );
        assert!(
            artifact.headline_findings[0]
                .title
                .contains("Insufficient direct trend evidence")
        );
    }

    #[test]
    fn dry_run_compare_dedupes_uncertainty_warnings() {
        let mut snapshot_a = snapshot_bundle("today");
        snapshot_a.freshness.warnings = vec!["stale data".to_owned()];
        let mut snapshot_b = snapshot_bundle("week");
        snapshot_b.freshness.warnings =
            vec!["missing capability".to_owned(), "stale data".to_owned()];

        let artifact = dry_run_compare_artifact(&snapshot_a, &snapshot_b);

        assert_eq!(
            artifact.uncertainty_warnings,
            vec!["missing capability".to_owned(), "stale data".to_owned()]
        );
    }

    #[tokio::test]
    async fn fixture_backed_review_uses_fixture_payload() {
        let temp_dir = tempfile::tempdir()
            .unwrap_or_else(|error| unreachable!("temp dir should exist: {error}"));
        let fixture_path = temp_dir.path().join("review.json");
        let loaded = loaded_snapshot("today");
        let expected_follow_ups = rebuild_follow_up_targets(&loaded.bundle);
        fs::write(
            &fixture_path,
            serde_json::to_string_pretty(&ReviewArtifactV1 {
                schema_version: REVIEW_OUTPUT_SCHEMA_VERSION.to_owned(),
                prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
                status: ArtifactStatus::Fixture,
                overview: "fixture review".to_owned(),
                headline_findings: Vec::new(),
                positive_findings: Vec::new(),
                negative_findings: Vec::new(),
                unresolved_questions: Vec::new(),
                limitations: Vec::new(),
                follow_up_targets: Vec::new(),
            })
            .unwrap_or_else(|error| unreachable!("fixture should encode: {error}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture should write: {error}"));

        let output = review_snapshot(
            &Config::load().unwrap_or_else(|error| unreachable!("config should load: {error}")),
            &loaded,
            false,
            Some(&fixture_path),
        )
        .await
        .unwrap_or_else(|error| unreachable!("fixture review should succeed: {error}"));
        assert_eq!(output.artifact.status, ArtifactStatus::Fixture);
        assert!(output.payload_json.contains("fixture review"));
        assert_eq!(
            output
                .artifact
                .follow_up_targets
                .iter()
                .map(|target| target.command.as_str())
                .collect::<Vec<_>>(),
            expected_follow_ups
                .iter()
                .map(|target| target.command.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn sanitize_review_artifact_dedupes_findings_and_overwrites_follow_ups() {
        let snapshot = snapshot_bundle("today");
        let expected_follow_ups = rebuild_follow_up_targets(&snapshot);
        let artifact = ReviewArtifactV1 {
            schema_version: REVIEW_OUTPUT_SCHEMA_VERSION.to_owned(),
            prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
            status: ArtifactStatus::Success,
            overview: "fixture review".to_owned(),
            headline_findings: vec![ArtifactFinding {
                finding_id: "sleep-dup".to_owned(),
                title: "Elevated sleep score".to_owned(),
                summary: "Sleep score is above baseline in this exploratory trend view.".to_owned(),
                claim_key: Some("sleep_score".to_owned()),
                evidence_tier: Some(crate::evidence::registry::EvidenceTier::Exploratory),
                interpretation_scope: Some(
                    crate::evidence::registry::InterpretationScope::WithinPersonTrendOnly,
                ),
                active_population_profile: Some(PopulationProfile::GeneralAdult),
                population_support_status: Some(PopulationSupportStatus::PopulationSpecific),
                fallback_population_profile: None,
                caution_labels: Vec::new(),
                confidence: super::ConfidenceLevel::Medium,
                sufficiency: SufficiencyLevel::Medium,
                evidence_refs: vec![
                    ArtifactEvidenceRef {
                        export_ref: "daily:2026-04-10".to_owned(),
                        note: "Daily row".to_owned(),
                    },
                    ArtifactEvidenceRef {
                        export_ref: "daily:2026-04-10".to_owned(),
                        note: "Duplicate daily row".to_owned(),
                    },
                    ArtifactEvidenceRef {
                        export_ref: "bogus:missing".to_owned(),
                        note: "Invalid".to_owned(),
                    },
                ],
                counterevidence_refs: vec![ArtifactEvidenceRef {
                    export_ref: "daily:2026-04-10".to_owned(),
                    note: "Should be removed because it duplicates evidence".to_owned(),
                }],
            }],
            positive_findings: vec![ArtifactFinding {
                finding_id: "sleep-dup".to_owned(),
                title: "Elevated sleep score".to_owned(),
                summary: "Sleep score is above baseline in this exploratory trend view.".to_owned(),
                claim_key: Some("sleep_score".to_owned()),
                evidence_tier: Some(crate::evidence::registry::EvidenceTier::Exploratory),
                interpretation_scope: Some(
                    crate::evidence::registry::InterpretationScope::WithinPersonTrendOnly,
                ),
                active_population_profile: Some(PopulationProfile::GeneralAdult),
                population_support_status: Some(PopulationSupportStatus::PopulationSpecific),
                fallback_population_profile: None,
                caution_labels: Vec::new(),
                confidence: super::ConfidenceLevel::Medium,
                sufficiency: SufficiencyLevel::Medium,
                evidence_refs: vec![ArtifactEvidenceRef {
                    export_ref: "context:1".to_owned(),
                    note: "Duplicate theme should be removed before this matters".to_owned(),
                }],
                counterevidence_refs: Vec::new(),
            }],
            negative_findings: vec![ArtifactFinding {
                finding_id: "workout-context".to_owned(),
                title: "Workout context present".to_owned(),
                summary: "A workout context event exists on the anchor day.".to_owned(),
                claim_key: Some("session_context".to_owned()),
                evidence_tier: Some(crate::evidence::registry::EvidenceTier::Exploratory),
                interpretation_scope: Some(
                    crate::evidence::registry::InterpretationScope::ContextualOnly,
                ),
                active_population_profile: Some(PopulationProfile::GeneralAdult),
                population_support_status: Some(PopulationSupportStatus::PopulationSpecific),
                fallback_population_profile: None,
                caution_labels: Vec::new(),
                confidence: super::ConfidenceLevel::High,
                sufficiency: SufficiencyLevel::Medium,
                evidence_refs: vec![ArtifactEvidenceRef {
                    export_ref: "context:1".to_owned(),
                    note: "Context event".to_owned(),
                }],
                counterevidence_refs: Vec::new(),
            }],
            unresolved_questions: Vec::new(),
            limitations: Vec::new(),
            follow_up_targets: vec![super::ArtifactFollowUpTarget {
                label: "Bad target".to_owned(),
                command: "review investigate --focus bogus --anchor-day 2026-04-10".to_owned(),
                reason: "Should be replaced".to_owned(),
            }],
        };

        let sanitized = sanitize_review_artifact(&snapshot, artifact)
            .unwrap_or_else(|error| unreachable!("review artifact should sanitize: {error}"));

        assert_eq!(sanitized.headline_findings.len(), 1);
        assert!(sanitized.positive_findings.is_empty());
        assert_eq!(sanitized.negative_findings.len(), 1);
        assert_eq!(sanitized.headline_findings[0].evidence_refs.len(), 1);
        assert!(
            sanitized.headline_findings[0]
                .counterevidence_refs
                .is_empty()
        );
        assert_eq!(
            sanitized
                .follow_up_targets
                .iter()
                .map(|target| target.command.as_str())
                .collect::<Vec<_>>(),
            expected_follow_ups
                .iter()
                .map(|target| target.command.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn follow_up_artifact_uses_current_snapshot_privacy_profile() {
        let mut snapshot = loaded_snapshot("today");
        snapshot.bundle.metadata.privacy_profile = PrivacyProfile::Balanced;
        snapshot.compact_json = serde_json::to_string(&snapshot.bundle)
            .unwrap_or_else(|error| unreachable!("bundle should encode: {error}"));

        let source_record = AiArtifactRecord {
            artifact_id: "artifact-source".to_owned(),
            artifact_kind: "review".to_owned(),
            output_schema_version: FOLLOW_UP_OUTPUT_SCHEMA_VERSION.to_owned(),
            prompt_version: FOLLOW_UP_PROMPT_VERSION.to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-5-mini".to_owned(),
            reasoning_effort: None,
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            run_mode: "dry_run".to_owned(),
            created_at: "2026-04-10T00:00:00Z".to_owned(),
            snapshot_hash_a: snapshot.bundle.metadata.snapshot_hash.clone(),
            snapshot_hash_b: None,
            privacy_profile: "redacted".to_owned(),
            artifact_status: "success".to_owned(),
            overview: "source overview".to_owned(),
            summary_cache: "summary".to_owned(),
            request_fingerprint: Some("fingerprint".to_owned()),
            payload_json: serde_json::to_string(&dry_run_review_artifact(&snapshot.bundle))
                .unwrap_or_else(|error| unreachable!("artifact should encode: {error}")),
            rendered_briefing: "briefing".to_owned(),
        };

        let output = follow_up_from_artifact_with_run_identity(
            &test_config(),
            &[snapshot],
            &source_record,
            GuidedFollowUpKind::ExpandEvidence,
            true,
            None,
            Some("2026-04-10T00:00:01Z"),
        )
        .await
        .unwrap_or_else(|error| unreachable!("follow-up should render: {error}"));

        assert_eq!(output.record.privacy_profile, "balanced");
    }

    #[tokio::test]
    async fn follow_up_artifact_uses_snapshot_hashes_from_current_inputs() {
        let mut snapshot_a = loaded_snapshot("week");
        snapshot_a.bundle.metadata.privacy_profile = PrivacyProfile::Balanced;
        snapshot_a.compact_json = serde_json::to_string(&snapshot_a.bundle)
            .unwrap_or_else(|error| unreachable!("bundle should encode: {error}"));
        let mut snapshot_b = loaded_snapshot("range:2026-04-08..2026-04-10");
        snapshot_b.bundle.metadata.privacy_profile = PrivacyProfile::Full;
        snapshot_b.compact_json = serde_json::to_string(&snapshot_b.bundle)
            .unwrap_or_else(|error| unreachable!("bundle should encode: {error}"));

        let source_record = AiArtifactRecord {
            artifact_id: "artifact-source".to_owned(),
            artifact_kind: "compare".to_owned(),
            output_schema_version: FOLLOW_UP_OUTPUT_SCHEMA_VERSION.to_owned(),
            prompt_version: FOLLOW_UP_PROMPT_VERSION.to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-5-mini".to_owned(),
            reasoning_effort: None,
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            run_mode: "dry_run".to_owned(),
            created_at: "2026-04-10T00:00:00Z".to_owned(),
            snapshot_hash_a: "stale-snapshot-a".to_owned(),
            snapshot_hash_b: Some("stale-snapshot-b".to_owned()),
            privacy_profile: "redacted".to_owned(),
            artifact_status: "success".to_owned(),
            overview: "source overview".to_owned(),
            summary_cache: "summary".to_owned(),
            request_fingerprint: Some("fingerprint".to_owned()),
            payload_json: serde_json::to_string(&dry_run_compare_artifact(
                &snapshot_a.bundle,
                &snapshot_b.bundle,
            ))
            .unwrap_or_else(|error| unreachable!("artifact should encode: {error}")),
            rendered_briefing: "briefing".to_owned(),
        };

        let output = follow_up_from_artifact_with_run_identity(
            &test_config(),
            &[snapshot_a.clone(), snapshot_b.clone()],
            &source_record,
            GuidedFollowUpKind::ExpandEvidence,
            true,
            None,
            Some("2026-04-10T00:00:01Z"),
        )
        .await
        .unwrap_or_else(|error| unreachable!("follow-up should render: {error}"));

        assert_eq!(
            output.record.snapshot_hash_a,
            snapshot_a.bundle.metadata.snapshot_hash
        );
        assert_eq!(
            output.record.snapshot_hash_b.as_deref(),
            Some(snapshot_b.bundle.metadata.snapshot_hash.as_str())
        );
        assert_eq!(output.record.privacy_profile, "full");
    }

    #[test]
    fn review_schema_generation_includes_expected_title() {
        let schema = schema_value::<ReviewArtifactV1>()
            .unwrap_or_else(|error| unreachable!("schema generation should succeed: {error}"));
        assert!(schema.to_string().contains("headline_findings"));
    }

    #[test]
    fn compare_schema_generation_includes_expected_title() {
        let schema = schema_value::<CompareArtifactV1>()
            .unwrap_or_else(|error| unreachable!("schema generation should succeed: {error}"));
        assert!(schema.to_string().contains("material_differences"));
    }

    #[test]
    fn request_preview_summarizes_snapshot_scope_and_content_classes() {
        let loaded = loaded_snapshot("today");
        let plan = build_review_request_plan(
            &Config::load()
                .unwrap_or_else(|error| unreachable!("config should load: {error}"))
                .ai,
            &loaded.bundle,
            &loaded.compact_json,
        )
        .unwrap_or_else(|error| unreachable!("request preview plan should build: {error}"));

        assert_eq!(plan.preview.task_family, "review");
        assert_eq!(plan.preview.snapshots.len(), 1);
        assert!(
            plan.preview
                .content_classes
                .iter()
                .any(|class| class == "daily_scores")
        );
        let rendered = render_request_preview(&plan.preview);
        assert!(rendered.contains("ringmaster ai request preview"));
        assert!(rendered.contains("scope=today"));
    }

    #[tokio::test]
    async fn dry_run_review_preview_uses_selected_provider_metadata() {
        let loaded = loaded_snapshot("today");
        let output = review_snapshot(
            &Config::load().unwrap_or_else(|error| unreachable!("config should load: {error}")),
            &loaded,
            true,
            None,
        )
        .await
        .unwrap_or_else(|error| unreachable!("dry-run review should succeed: {error}"));

        assert_eq!(output.request_preview.provider, "dry_run");
        assert_eq!(output.request_preview.model, "deterministic");
        assert!(output.request_preview.stateless);
    }

    #[test]
    fn snapshot_bundle_still_validates_after_round_trip() {
        let loaded = loaded_snapshot("today");
        let reparsed = deserialize_snapshot_bundle(&loaded.compact_json)
            .unwrap_or_else(|error| unreachable!("snapshot should parse: {error}"));
        assert_eq!(
            reparsed.metadata.snapshot_hash,
            loaded.bundle.metadata.snapshot_hash
        );
    }

    #[test]
    fn artifact_ids_include_run_identity_even_for_identical_payloads() {
        let payload = "{\"status\":\"dry_run\"}";

        let first = artifact_id(
            "review",
            "snapshot-hash",
            None,
            super::AiRunMode::DryRun,
            "2026-04-10T00:05:00Z",
            payload,
        );
        let second = artifact_id(
            "review",
            "snapshot-hash",
            None,
            super::AiRunMode::DryRun,
            "2026-04-10T00:05:01Z",
            payload,
        );

        assert_ne!(first, second);
    }

    #[test]
    fn retryable_error_includes_transient_status_codes() {
        for status in [
            StatusCode::REQUEST_TIMEOUT,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
            StatusCode::SERVICE_UNAVAILABLE,
            StatusCode::GATEWAY_TIMEOUT,
        ] {
            let error =
                RingmasterError::Ui(format!("OpenAI Responses API request failed with {status}"));
            assert!(
                retryable_error(&error),
                "status {status} should be treated as retryable"
            );
        }

        let error = RingmasterError::Ui(format!(
            "OpenAI Responses API request failed with {}",
            StatusCode::BAD_REQUEST
        ));
        assert!(
            !retryable_error(&error),
            "permanent client errors should not be retried"
        );
    }

    #[test]
    fn ai_transport_timeout_error_points_to_config_override() {
        let rendered = super::ai_timeout_message(120);
        assert!(rendered.contains("timed out after 120s"));
        assert!(rendered.contains("RINGMASTER_AI_TIMEOUT_SECS"));
    }
}
