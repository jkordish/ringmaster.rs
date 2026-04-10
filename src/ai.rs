use std::fs;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use reqwest::{Client, StatusCode};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::ai_prompts::{
    COMPARE_PROMPT_VERSION, REVIEW_PROMPT_VERSION, compare_system_prompt, compare_task_framing,
    review_system_prompt, review_task_framing,
};
use crate::config::{AiConfig, AiInputTransport, AiRequestMode, Config, PromptCacheMode};
use crate::error::{Result, RingmasterError};
use crate::snapshot::{
    ArtifactRecordInput, LoadedSnapshotArtifact, PrivacyProfile, SnapshotBundleV1,
    SnapshotFollowUpTarget, SnapshotReviewSignal, artifact_record,
};
use crate::store::queries::AiArtifactRecord;

pub const REVIEW_OUTPUT_SCHEMA_VERSION: &str = "ringmaster.ai.review.v1";
pub const COMPARE_OUTPUT_SCHEMA_VERSION: &str = "ringmaster.ai.compare.v1";

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

#[derive(Debug, Clone)]
pub struct ReviewRunOutput {
    pub artifact: ReviewArtifactV1,
    pub payload_json: String,
    pub rendered_briefing: String,
    pub request_preview: String,
    pub request_fingerprint: String,
    pub record: AiArtifactRecord,
}

#[derive(Debug, Clone)]
pub struct CompareRunOutput {
    pub artifact: CompareArtifactV1,
    pub payload_json: String,
    pub rendered_briefing: String,
    pub request_preview: String,
    pub request_fingerprint: String,
    pub record: AiArtifactRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredArtifact {
    Review(ReviewArtifactV1),
    Compare(CompareArtifactV1),
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

struct StructuredOutputRequestPlan {
    body: Value,
    preview: String,
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

pub async fn review_snapshot(
    config: &Config,
    snapshot: &LoadedSnapshotArtifact,
    dry_run: bool,
    fixture: Option<&Path>,
) -> Result<ReviewRunOutput> {
    review_snapshot_with_run_identity(config, snapshot, dry_run, fixture, None).await
}

pub(crate) async fn review_snapshot_with_run_identity(
    config: &Config,
    snapshot: &LoadedSnapshotArtifact,
    dry_run: bool,
    fixture: Option<&Path>,
    run_identity_override: Option<&str>,
) -> Result<ReviewRunOutput> {
    let request_plan = build_review_request_plan(&config.ai, &snapshot.compact_json)?;
    let provider = select_provider(&config.ai, dry_run, fixture)?;
    let metadata = provider.metadata();
    let artifact = provider
        .review(ReviewProviderRequest {
            snapshot: &snapshot.bundle,
            snapshot_json: &snapshot.compact_json,
        })
        .await?;
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

pub(crate) async fn compare_snapshots_with_run_identity(
    config: &Config,
    snapshot_a: &LoadedSnapshotArtifact,
    snapshot_b: &LoadedSnapshotArtifact,
    dry_run: bool,
    fixture: Option<&Path>,
    run_identity_override: Option<&str>,
) -> Result<CompareRunOutput> {
    let request_plan = build_compare_request_plan(
        &config.ai,
        &snapshot_a.compact_json,
        &snapshot_b.compact_json,
    )?;
    let provider = select_provider(&config.ai, dry_run, fixture)?;
    let metadata = provider.metadata();
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

pub fn parse_stored_artifact(record: &AiArtifactRecord) -> Result<StoredArtifact> {
    match record.artifact_kind.as_str() {
        "review" => Ok(StoredArtifact::Review(serde_json::from_str(
            &record.payload_json,
        )?)),
        "compare" => Ok(StoredArtifact::Compare(serde_json::from_str(
            &record.payload_json,
        )?)),
        other => Err(RingmasterError::Ui(format!(
            "unsupported AI artifact kind `{other}`"
        ))),
    }
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
                format!("     {}", finding.summary),
            ];
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
            let plan = build_review_request_plan(&self.config, request.snapshot_json)?;
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
                request.snapshot_a_json,
                request.snapshot_b_json,
            )?;
            self.invoke_structured_output::<CompareArtifactV1>(plan)
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
            .await?;
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

fn build_review_request_plan(
    config: &AiConfig,
    snapshot_json: &str,
) -> Result<StructuredOutputRequestPlan> {
    build_structured_output_request::<ReviewArtifactV1>(
        config,
        StructuredOutputRequestSpec {
            task_family: "review",
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
    snapshot_a_json: &str,
    comparison_snapshot_json: &str,
) -> Result<StructuredOutputRequestPlan> {
    build_structured_output_request::<CompareArtifactV1>(
        config,
        StructuredOutputRequestSpec {
            task_family: "compare",
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

struct StructuredOutputRequestSpec<'a> {
    task_family: &'a str,
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
        preview: format!(
            "\
ringmaster ai request preview

task_family: {task_family}
provider: openai
request_mode: {}
input_transport: {}
prompt_cache: {}
prompt_version: {prompt_version}
output_schema_version: {output_schema_version}
prefix_fingerprint: {}
payload_fingerprint: {}
request_fingerprint: {}
sections:
  - system_instructions
  - task_framing
  - output_schema
  - snapshot_payload
snapshot_bytes: {}
",
            config.request_mode.as_str(),
            config.input_transport.as_str(),
            prompt_cache_label(config.prompt_cache),
            short_fingerprint(&prefix_fingerprint, 16),
            short_fingerprint(&payload_fingerprint, 16),
            short_fingerprint(&request_fingerprint, 16),
            snapshot_payload.len(),
        ),
        request_fingerprint,
    })
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

fn review_findings_from_snapshot(snapshot: &SnapshotBundleV1) -> Vec<ArtifactFinding> {
    let mut findings = snapshot
        .trend_summaries
        .iter()
        .take(3)
        .map(|summary| ArtifactFinding {
            finding_id: finding_id(&summary.metric_key, &summary.label),
            title: summary.label.clone(),
            summary: trend_summary_text(summary.label.as_str(), summary.direction.as_str()),
            confidence: if summary.current_average.is_some() && summary.previous_average.is_some() {
                ConfidenceLevel::Medium
            } else {
                ConfidenceLevel::Low
            },
            sufficiency: if summary.current_average.is_some() && summary.previous_average.is_some()
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
        })
        .collect::<Vec<_>>();

    if findings.is_empty() {
        findings.push(ArtifactFinding {
            finding_id: finding_id("insufficient", &snapshot.metadata.scope),
            title: "Insufficient direct trend evidence".to_owned(),
            summary: "The snapshot did not contain enough trend data to derive a stronger dry-run finding.".to_owned(),
            confidence: ConfidenceLevel::Low,
            sufficiency: SufficiencyLevel::Missing,
            evidence_refs: Vec::new(),
            counterevidence_refs: Vec::new(),
        });
    }

    findings
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
                "The average combined daily score shifted from {:.1} to {:.1}.",
                left, right
            ),
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
            "Review signal coverage changed from {} signal rows to {} signal rows.",
            count_a, count_b
        ),
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

fn retryable_status_codes() -> &'static [StatusCode] {
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

fn prompt_cache_label(mode: PromptCacheMode) -> &'static str {
    match mode {
        PromptCacheMode::Off => "off",
        PromptCacheMode::Auto => "auto",
    }
}

fn review_summary_cache(artifact: &ReviewArtifactV1) -> String {
    artifact
        .headline_findings
        .first()
        .map(|finding| format!("{}: {}", finding.title, finding.summary))
        .unwrap_or_else(|| artifact.overview.clone())
}

fn compare_summary_cache(artifact: &CompareArtifactV1) -> String {
    artifact
        .material_differences
        .first()
        .map(|finding| format!("{}: {}", finding.title, finding.summary))
        .unwrap_or_else(|| artifact.overview.clone())
}

fn merged_privacy_profile(left: PrivacyProfile, right: PrivacyProfile) -> PrivacyProfile {
    use PrivacyProfile::{Balanced, Full, Redacted};
    match (left, right) {
        (Full, _) | (_, Full) => Full,
        (Balanced, _) | (_, Balanced) => Balanced,
        (Redacted, Redacted) => Redacted,
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
                Some(parts.iter().sum::<f64>() / parts.len() as f64)
            }
        })
        .collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
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
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Real => "real",
            Self::DryRun => "dry_run",
            Self::Fixture => "fixture",
        }
    }
}

impl ArtifactStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Insufficient => "insufficient",
            Self::DryRun => "dry_run",
            Self::Fixture => "fixture",
        }
    }
}

impl ConfidenceLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl SufficiencyLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Thin => "thin",
            Self::Medium => "medium",
            Self::Strong => "strong",
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use reqwest::StatusCode;
    use sha2::Digest;

    use super::{
        ArtifactStatus, COMPARE_OUTPUT_SCHEMA_VERSION, COMPARE_PROMPT_VERSION, CompareArtifactV1,
        REVIEW_OUTPUT_SCHEMA_VERSION, REVIEW_PROMPT_VERSION, ReviewArtifactV1, SufficiencyLevel,
        artifact_id, dry_run_compare_artifact, dry_run_review_artifact, render_compare_briefing,
        render_review_briefing, retryable_error, review_snapshot, schema_value,
    };
    use crate::config::Config;
    use crate::error::RingmasterError;
    use crate::snapshot::{
        PrivacyProfile, SnapshotBundleV1, SnapshotCapabilities, SnapshotCapabilityEntry,
        SnapshotContextEvent, SnapshotFreshness, SnapshotMetadata, SnapshotMetrics,
        SnapshotRecordCounts, SnapshotReviewSignal, SnapshotSourceMode, SnapshotSyncState,
        deserialize_snapshot_bundle,
    };

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
                source_mode: SnapshotSourceMode::Demo,
                schema_version: 13,
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
            }],
            follow_up_targets: vec![crate::snapshot::SnapshotFollowUpTarget {
                label: "Review today".to_owned(),
                command: "review today --day 2026-04-10".to_owned(),
                reason: "Inspect the local brief.".to_owned(),
            }],
        };
        let canonical_without_hash = serde_json::to_string(&bundle)
            .unwrap_or_else(|error| panic!("bundle should encode: {error}"));
        bundle.metadata.snapshot_hash =
            hex::encode(sha2::Sha256::digest(canonical_without_hash.as_bytes()));
        bundle
    }

    fn loaded_snapshot(scope: &str) -> crate::snapshot::LoadedSnapshotArtifact {
        let bundle = snapshot_bundle(scope);
        let compact_json = serde_json::to_string(&bundle)
            .unwrap_or_else(|error| panic!("bundle should encode: {error}"));
        crate::snapshot::LoadedSnapshotArtifact {
            bundle,
            compact_json,
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
        let temp_dir =
            tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir should exist: {error}"));
        let fixture_path = temp_dir.path().join("review.json");
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
            .unwrap_or_else(|error| panic!("fixture should encode: {error}")),
        )
        .unwrap_or_else(|error| panic!("fixture should write: {error}"));

        let output = review_snapshot(
            &Config::load().unwrap_or_else(|error| panic!("config should load: {error}")),
            &loaded_snapshot("today"),
            false,
            Some(&fixture_path),
        )
        .await
        .unwrap_or_else(|error| panic!("fixture review should succeed: {error}"));
        assert_eq!(output.artifact.status, ArtifactStatus::Fixture);
        assert!(output.payload_json.contains("fixture review"));
    }

    #[test]
    fn review_schema_generation_includes_expected_title() {
        let schema = schema_value::<ReviewArtifactV1>()
            .unwrap_or_else(|error| panic!("schema generation should succeed: {error}"));
        assert!(schema.to_string().contains("headline_findings"));
    }

    #[test]
    fn compare_schema_generation_includes_expected_title() {
        let schema = schema_value::<CompareArtifactV1>()
            .unwrap_or_else(|error| panic!("schema generation should succeed: {error}"));
        assert!(schema.to_string().contains("material_differences"));
    }

    #[test]
    fn snapshot_bundle_still_validates_after_round_trip() {
        let loaded = loaded_snapshot("today");
        let reparsed = deserialize_snapshot_bundle(&loaded.compact_json)
            .unwrap_or_else(|error| panic!("snapshot should parse: {error}"));
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
}
