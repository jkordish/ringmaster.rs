use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::ai::{self, StoredArtifact};
use crate::cli::{ReportExportArgs, ReportFormatArg};
use crate::config::Config;
use crate::error::{Result, RingmasterError};
use crate::evidence::policy::claim_language_spec;
use crate::evidence::registry::{
    EvidenceDescriptor, evidence_descriptor, resolve_evidence_descriptor,
};
use crate::resolved_demo_fixture_dir;
use crate::snapshot::{self, LoadedSnapshotArtifact, SnapshotBundleV1};
use crate::store::Store;
use crate::store::queries::{AiArtifactRecord, ReportExportRecord, SnapshotProvenanceRefRecord};

const MARKDOWN_TEMPLATE_VERSION: &str = "report_markdown_v2";
const HTML_TEMPLATE_VERSION: &str = "report_html_v2";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReportDocument {
    report_kind: String,
    title: String,
    generated_at: String,
    scope: String,
    privacy_profile: String,
    ai_used: bool,
    ai_provider: Option<String>,
    ai_model: Option<String>,
    prompt_version: Option<String>,
    output_schema_version: Option<String>,
    freshness_summary: String,
    trust_summary: String,
    key_findings: Vec<String>,
    supporting_evidence: Vec<String>,
    evidence_and_rails: Vec<String>,
    uncertainty_notes: Vec<String>,
    provenance_refs: Vec<String>,
    artifact_refs: Vec<String>,
    source_snapshot_hash_a: Option<String>,
    source_snapshot_hash_b: Option<String>,
    source_ai_artifact_id: Option<String>,
}

enum ReportSource {
    Snapshot {
        artifact: Box<LoadedSnapshotArtifact>,
        provenance: Vec<SnapshotProvenanceRefRecord>,
    },
    AiRun {
        record: Box<AiArtifactRecord>,
        artifact: Box<StoredArtifact>,
        snapshot_a: Box<Option<SnapshotBundleV1>>,
        provenance_a: Vec<SnapshotProvenanceRefRecord>,
        snapshot_b: Box<Option<SnapshotBundleV1>>,
        provenance_b: Vec<SnapshotProvenanceRefRecord>,
    },
}

/// # Errors
///
/// Returns an error if the report source cannot be resolved, rendered, or written to disk.
pub async fn export_report(config: &Config, args: ReportExportArgs) -> Result<Option<String>> {
    let Some(source_spec) = selected_source(&args)? else {
        return Err(RingmasterError::Cli(
            "report export requires exactly one of `--from-snapshot` or `--from-ai-run`".to_owned(),
        ));
    };

    let fixture_dir = resolved_demo_fixture_dir(config, args.demo, args.fixture_dir.clone());
    let context = crate::load_library_command_context(
        config,
        args.demo,
        fixture_dir.clone(),
        args.from_ai_run.is_some(),
    )
    .await?;
    let source = match source_spec {
        ReportSourceSpec::Snapshot(spec) => resolve_snapshot_source(&context.store, spec)?,
        ReportSourceSpec::AiRun(run_id) => resolve_ai_run_source(&context.store, run_id)?,
    };
    persist_source_lineage(&context.store, &source)?;

    let document = build_report_document(&source);
    let rendered = match args.format {
        ReportFormatArg::Markdown => render_markdown(&document),
        ReportFormatArg::Html => render_html(&document),
    };
    write_text_file(&args.out, &rendered, "writing report export")?;

    let now = now_rfc3339()?;
    let manifest = ReportExportRecord {
        report_id: report_id(
            &document.report_kind,
            document.source_snapshot_hash_a.as_deref(),
            document.source_snapshot_hash_b.as_deref(),
            document.source_ai_artifact_id.as_deref(),
            rendered.as_bytes(),
        ),
        report_kind: document.report_kind.clone(),
        title: document.title.clone(),
        format: report_format_label(args.format).to_owned(),
        output_path: args.out.display().to_string(),
        content_hash: content_hash(rendered.as_bytes()),
        privacy_profile: document.privacy_profile.clone(),
        created_at: now.clone(),
        source_snapshot_hash_a: document.source_snapshot_hash_a.clone(),
        source_snapshot_hash_b: document.source_snapshot_hash_b.clone(),
        source_ai_artifact_id: document.source_ai_artifact_id.clone(),
        provider: document.ai_provider.clone(),
        model: document.ai_model.clone(),
        prompt_version: document.prompt_version.clone(),
        output_schema_version: document.output_schema_version.clone(),
        export_status: "written".to_owned(),
        last_verified_exists: args.out.exists(),
        last_verified_at: now,
    };
    context.store.analysis().upsert_report_export(&manifest)?;

    Ok(Some(format!(
        "\
ringmaster report export

report_id: {}
report_kind: {}
format: {}
privacy_profile: {}
out: {}
",
        manifest.report_id,
        manifest.report_kind,
        manifest.format,
        manifest.privacy_profile,
        manifest.output_path,
    )))
}

enum ReportSourceSpec<'a> {
    Snapshot(&'a str),
    AiRun(&'a str),
}

fn persist_source_lineage(store: &Store, source: &ReportSource) -> Result<()> {
    match source {
        ReportSource::Snapshot { artifact, .. } => {
            let record = snapshot::catalog_record_from_loaded_artifact(artifact, None);
            store.analysis().upsert_snapshot_export(&record, &[])?;
        }
        ReportSource::AiRun {
            snapshot_a,
            snapshot_b,
            ..
        } => {
            if let Some(snapshot_a) = snapshot_a.as_ref() {
                let artifact = LoadedSnapshotArtifact {
                    bundle: snapshot_a.clone(),
                    compact_json: snapshot::canonicalize_snapshot_bundle(snapshot_a)?,
                };
                let record = snapshot::catalog_record_from_loaded_artifact(&artifact, None);
                store.analysis().upsert_snapshot_export(&record, &[])?;
            }
            if let Some(snapshot_b) = snapshot_b.as_ref() {
                let artifact = LoadedSnapshotArtifact {
                    bundle: snapshot_b.clone(),
                    compact_json: snapshot::canonicalize_snapshot_bundle(snapshot_b)?,
                };
                let record = snapshot::catalog_record_from_loaded_artifact(&artifact, None);
                store.analysis().upsert_snapshot_export(&record, &[])?;
            }
        }
    }
    Ok(())
}

fn selected_source(args: &ReportExportArgs) -> Result<Option<ReportSourceSpec<'_>>> {
    match (args.from_snapshot.as_deref(), args.from_ai_run.as_deref()) {
        (Some(_), Some(_)) => Err(RingmasterError::Cli(
            "report export accepts either `--from-snapshot` or `--from-ai-run`, not both"
                .to_owned(),
        )),
        (Some(snapshot), None) => Ok(Some(ReportSourceSpec::Snapshot(snapshot))),
        (None, Some(run_id)) => Ok(Some(ReportSourceSpec::AiRun(run_id))),
        (None, None) => Ok(None),
    }
}

fn resolve_snapshot_source(store: &Store, spec: &str) -> Result<ReportSource> {
    let path = PathBuf::from(spec);
    if path.exists() {
        let artifact = snapshot::load_snapshot_artifact(&path)?;
        let provenance = store
            .analysis()
            .snapshot_provenance_refs(&artifact.bundle.metadata.snapshot_hash)
            .unwrap_or_default();
        return Ok(ReportSource::Snapshot {
            artifact: Box::new(artifact),
            provenance,
        });
    }

    let record = if let Some(record) = store.analysis().snapshot_export(spec)? {
        record
    } else {
        let matches = store.analysis().snapshot_exports_with_prefix(spec)?;
        if matches.len() > 1 {
            return Err(RingmasterError::Ui(format!(
                "snapshot `{spec}` matched multiple catalog entries; use a longer prefix"
            )));
        }
        let Some(record) = matches.into_iter().next() else {
            return Err(RingmasterError::Ui(format!(
                "snapshot `{spec}` was not found in the local catalog and is not a readable file path"
            )));
        };
        record
    };
    let provenance = store
        .analysis()
        .snapshot_provenance_refs(&record.snapshot_hash)?;
    let bundle = snapshot::deserialize_snapshot_bundle(&record.snapshot_json)?;
    Ok(ReportSource::Snapshot {
        artifact: Box::new(LoadedSnapshotArtifact {
            compact_json: record.snapshot_json,
            bundle,
        }),
        provenance,
    })
}

fn resolve_ai_run_source(store: &Store, run_id: &str) -> Result<ReportSource> {
    let record = if let Some(record) = store.analysis().ai_artifact(run_id)? {
        record
    } else {
        let matches = store.analysis().ai_artifacts_with_prefix(run_id)?;
        if matches.len() > 1 {
            return Err(RingmasterError::Ui(format!(
                "AI run `{run_id}` matched multiple registry entries; use a longer prefix"
            )));
        }
        let Some(record) = matches.into_iter().next() else {
            return Err(RingmasterError::Ui(format!(
                "AI run `{run_id}` was not found in the local registry"
            )));
        };
        record
    };
    let artifact = ai::parse_stored_artifact(&record)?;

    let (snapshot_a, provenance_a) =
        if let Some(snapshot_record) = store.analysis().snapshot_export(&record.snapshot_hash_a)? {
            (
                Some(snapshot::deserialize_snapshot_bundle(
                    &snapshot_record.snapshot_json,
                )?),
                store
                    .analysis()
                    .snapshot_provenance_refs(&snapshot_record.snapshot_hash)?,
            )
        } else {
            (None, Vec::new())
        };

    let (snapshot_b, provenance_b) = if let Some(snapshot_hash_b) = &record.snapshot_hash_b {
        if let Some(snapshot_record) = store.analysis().snapshot_export(snapshot_hash_b)? {
            (
                Some(snapshot::deserialize_snapshot_bundle(
                    &snapshot_record.snapshot_json,
                )?),
                store
                    .analysis()
                    .snapshot_provenance_refs(&snapshot_record.snapshot_hash)?,
            )
        } else {
            (None, Vec::new())
        }
    } else {
        (None, Vec::new())
    };

    Ok(ReportSource::AiRun {
        record: Box::new(record),
        artifact: Box::new(artifact),
        snapshot_a: Box::new(snapshot_a),
        provenance_a,
        snapshot_b: Box::new(snapshot_b),
        provenance_b,
    })
}

fn build_report_document(source: &ReportSource) -> ReportDocument {
    match source {
        ReportSource::Snapshot {
            artifact,
            provenance,
        } => build_snapshot_report_document(&artifact.bundle, provenance),
        ReportSource::AiRun {
            record,
            artifact,
            snapshot_a,
            provenance_a,
            snapshot_b,
            provenance_b,
        } => build_ai_report_document(
            record,
            artifact,
            snapshot_a.as_ref().as_ref(),
            provenance_a,
            snapshot_b.as_ref().as_ref(),
            provenance_b,
        ),
    }
}

fn build_snapshot_report_document(
    bundle: &SnapshotBundleV1,
    provenance: &[SnapshotProvenanceRefRecord],
) -> ReportDocument {
    let summary = snapshot::summarize_snapshot_bundle(bundle, provenance);
    let mut key_findings = bundle
        .trend_summaries
        .iter()
        .take(3)
        .map(format_snapshot_trend_finding)
        .collect::<Vec<_>>();
    if key_findings.is_empty() {
        key_findings.extend(
            bundle
                .follow_up_targets
                .iter()
                .take(3)
                .map(|target| format!("{}: {}", target.label, target.reason)),
        );
    }
    let supporting_evidence = provenance
        .iter()
        .take(6)
        .map(|record| {
            format!(
                "{} -> {}:{}",
                record.export_ref, record.local_kind, record.local_locator
            )
        })
        .collect::<Vec<_>>();
    let evidence_and_rails = snapshot_evidence_and_rails(bundle);
    let mut uncertainty_notes = bundle.freshness.warnings.clone();
    uncertainty_notes.extend(snapshot_uncertainty_notes(bundle));
    if uncertainty_notes.is_empty() {
        uncertainty_notes.push("No freshness warnings were recorded in this snapshot.".to_owned());
    }
    if !bundle.capabilities.missing_scopes.is_empty() {
        uncertainty_notes.push(format!(
            "Missing scopes: {}",
            bundle.capabilities.missing_scopes.join(", ")
        ));
    }

    ReportDocument {
        report_kind: "snapshot_report".to_owned(),
        title: format!("Snapshot report: {}", bundle.metadata.scope),
        generated_at: bundle.metadata.generated_at.clone(),
        scope: bundle.metadata.scope.clone(),
        privacy_profile: bundle.metadata.privacy_profile.as_str().to_owned(),
        ai_used: false,
        ai_provider: None,
        ai_model: None,
        prompt_version: None,
        output_schema_version: None,
        freshness_summary: summary.freshness_summary,
        trust_summary: summary.trust_summary,
        key_findings,
        supporting_evidence,
        evidence_and_rails,
        uncertainty_notes: unique_evidence_refs(uncertainty_notes),
        provenance_refs: provenance
            .iter()
            .map(|record| {
                format!(
                    "{} [{}:{}]",
                    record.export_ref, record.local_kind, record.local_locator
                )
            })
            .collect(),
        artifact_refs: vec![
            format!("snapshot_hash={}", bundle.metadata.snapshot_hash),
            format!(
                "evidence_registry_version={}",
                bundle.metadata.evidence_registry_version
            ),
        ],
        source_snapshot_hash_a: Some(bundle.metadata.snapshot_hash.clone()),
        source_snapshot_hash_b: None,
        source_ai_artifact_id: None,
    }
}

fn build_ai_report_document(
    record: &AiArtifactRecord,
    artifact: &StoredArtifact,
    snapshot_a: Option<&SnapshotBundleV1>,
    provenance_a: &[SnapshotProvenanceRefRecord],
    snapshot_b: Option<&SnapshotBundleV1>,
    provenance_b: &[SnapshotProvenanceRefRecord],
) -> ReportDocument {
    let mut artifact_refs = vec![format!("ai_run={}", record.artifact_id)];
    artifact_refs.push(format!("snapshot_a={}", record.snapshot_hash_a));
    if let Some(snapshot_hash_b) = &record.snapshot_hash_b {
        artifact_refs.push(format!("snapshot_b={snapshot_hash_b}"));
    }

    match artifact {
        StoredArtifact::Review(review) => {
            let (scope, freshness_summary, trust_summary, mut evidence_and_rails) = snapshot_a
                .map_or_else(
                    || {
                        (
                            "unknown".to_owned(),
                            "snapshot record unavailable".to_owned(),
                            "lineage available from persisted AI run metadata".to_owned(),
                            Vec::new(),
                        )
                    },
                    |snapshot_a| {
                        let summary = snapshot::summarize_snapshot_bundle(snapshot_a, provenance_a);
                        (
                            snapshot_a.metadata.scope.clone(),
                            summary.freshness_summary,
                            summary.trust_summary,
                            vec![format!(
                                "Snapshot evidence registry version: {}",
                                snapshot_a.metadata.evidence_registry_version
                            )],
                        )
                    },
                );
            evidence_and_rails.extend(finding_evidence_and_rails([
                &review.headline_findings,
                &review.positive_findings,
                &review.negative_findings,
            ]));

            ReportDocument {
                report_kind: "ai_review_report".to_owned(),
                title: "AI review report".to_owned(),
                generated_at: record.created_at.clone(),
                scope,
                privacy_profile: record.privacy_profile.clone(),
                ai_used: true,
                ai_provider: Some(record.provider.clone()),
                ai_model: Some(record.model.clone()),
                prompt_version: Some(record.prompt_version.clone()),
                output_schema_version: Some(record.output_schema_version.clone()),
                freshness_summary,
                trust_summary,
                key_findings: report_lines_for_findings(&review.headline_findings),
                supporting_evidence: unique_evidence_refs(
                    review
                        .headline_findings
                        .iter()
                        .flat_map(|finding| finding.evidence_refs.iter())
                        .map(|evidence| format!("{}: {}", evidence.export_ref, evidence.note))
                        .chain(
                            review
                                .positive_findings
                                .iter()
                                .flat_map(|finding| finding.evidence_refs.iter())
                                .map(|evidence| {
                                    format!("{}: {}", evidence.export_ref, evidence.note)
                                }),
                        )
                        .collect(),
                ),
                evidence_and_rails: unique_evidence_refs(evidence_and_rails),
                uncertainty_notes: unique_evidence_refs(
                    review
                        .unresolved_questions
                        .iter()
                        .cloned()
                        .chain(
                            review
                                .limitations
                                .iter()
                                .map(|limitation| limitation.message.clone()),
                        )
                        .chain(finding_uncertainty_notes([
                            &review.headline_findings,
                            &review.positive_findings,
                            &review.negative_findings,
                        ]))
                        .collect(),
                ),
                provenance_refs: provenance_a
                    .iter()
                    .map(|record| {
                        format!(
                            "{} [{}:{}]",
                            record.export_ref, record.local_kind, record.local_locator
                        )
                    })
                    .collect(),
                artifact_refs,
                source_snapshot_hash_a: Some(record.snapshot_hash_a.clone()),
                source_snapshot_hash_b: None,
                source_ai_artifact_id: Some(record.artifact_id.clone()),
            }
        }
        StoredArtifact::Compare(compare) => {
            let mut provenance_refs = provenance_a
                .iter()
                .map(|record| {
                    format!(
                        "{} [{}:{}]",
                        record.export_ref, record.local_kind, record.local_locator
                    )
                })
                .collect::<Vec<_>>();
            provenance_refs.extend(provenance_b.iter().map(|record| {
                format!(
                    "{} [{}:{}]",
                    record.export_ref, record.local_kind, record.local_locator
                )
            }));

            let mut evidence_and_rails = Vec::new();
            if let Some(snapshot_a) = snapshot_a {
                evidence_and_rails.push(format!(
                    "Snapshot A evidence registry version: {}",
                    snapshot_a.metadata.evidence_registry_version
                ));
            }
            if let Some(snapshot_b) = snapshot_b {
                evidence_and_rails.push(format!(
                    "Snapshot B evidence registry version: {}",
                    snapshot_b.metadata.evidence_registry_version
                ));
            }
            evidence_and_rails.extend(finding_evidence_and_rails([&compare.material_differences]));

            ReportDocument {
                report_kind: "ai_compare_report".to_owned(),
                title: "AI compare report".to_owned(),
                generated_at: record.created_at.clone(),
                scope: compare_report_scope(snapshot_a, snapshot_b),
                privacy_profile: record.privacy_profile.clone(),
                ai_used: true,
                ai_provider: Some(record.provider.clone()),
                ai_model: Some(record.model.clone()),
                prompt_version: Some(record.prompt_version.clone()),
                output_schema_version: Some(record.output_schema_version.clone()),
                freshness_summary: compare_report_freshness_summary(
                    snapshot_a,
                    provenance_a,
                    snapshot_b,
                    provenance_b,
                ),
                trust_summary: compare_report_trust_summary(
                    snapshot_a,
                    provenance_a,
                    snapshot_b,
                    provenance_b,
                ),
                key_findings: report_lines_for_findings(&compare.material_differences),
                supporting_evidence: unique_evidence_refs(
                    compare
                        .supporting_evidence
                        .iter()
                        .chain(
                            compare
                                .material_differences
                                .iter()
                                .flat_map(|finding| finding.evidence_refs.iter()),
                        )
                        .chain(
                            compare
                                .material_differences
                                .iter()
                                .flat_map(|finding| finding.counterevidence_refs.iter()),
                        )
                        .map(|evidence| format!("{}: {}", evidence.export_ref, evidence.note))
                        .collect(),
                ),
                evidence_and_rails: unique_evidence_refs(evidence_and_rails),
                uncertainty_notes: unique_evidence_refs(
                    compare
                        .uncertainty_warnings
                        .iter()
                        .cloned()
                        .chain(finding_uncertainty_notes([&compare.material_differences]))
                        .collect(),
                ),
                provenance_refs,
                artifact_refs,
                source_snapshot_hash_a: Some(record.snapshot_hash_a.clone()),
                source_snapshot_hash_b: record.snapshot_hash_b.clone(),
                source_ai_artifact_id: Some(record.artifact_id.clone()),
            }
        }
        StoredArtifact::FollowUp(follow_up) => {
            let (scope, freshness_summary, trust_summary, mut evidence_and_rails) = snapshot_a
                .map_or_else(
                    || {
                        (
                            "unknown".to_owned(),
                            "snapshot record unavailable".to_owned(),
                            "lineage available from persisted AI run metadata".to_owned(),
                            Vec::new(),
                        )
                    },
                    |snapshot_a| {
                        let summary = snapshot::summarize_snapshot_bundle(snapshot_a, provenance_a);
                        (
                            snapshot_a.metadata.scope.clone(),
                            summary.freshness_summary,
                            summary.trust_summary,
                            vec![format!(
                                "Snapshot evidence registry version: {}",
                                snapshot_a.metadata.evidence_registry_version
                            )],
                        )
                    },
                );
            evidence_and_rails.extend(finding_evidence_and_rails([&follow_up.focal_findings]));

            ReportDocument {
                report_kind: "ai_follow_up_report".to_owned(),
                title: format!("AI follow-up report: {}", follow_up.follow_up_kind.as_str()),
                generated_at: record.created_at.clone(),
                scope,
                privacy_profile: record.privacy_profile.clone(),
                ai_used: true,
                ai_provider: Some(record.provider.clone()),
                ai_model: Some(record.model.clone()),
                prompt_version: Some(record.prompt_version.clone()),
                output_schema_version: Some(record.output_schema_version.clone()),
                freshness_summary,
                trust_summary,
                key_findings: report_lines_for_findings(&follow_up.focal_findings),
                supporting_evidence: unique_evidence_refs(
                    follow_up
                        .focal_findings
                        .iter()
                        .flat_map(|finding| finding.evidence_refs.iter())
                        .chain(
                            follow_up
                                .focal_findings
                                .iter()
                                .flat_map(|finding| finding.counterevidence_refs.iter()),
                        )
                        .map(|evidence| format!("{}: {}", evidence.export_ref, evidence.note))
                        .collect(),
                ),
                evidence_and_rails: unique_evidence_refs(evidence_and_rails),
                uncertainty_notes: unique_evidence_refs(
                    follow_up
                        .unresolved_questions
                        .iter()
                        .cloned()
                        .chain(finding_uncertainty_notes([&follow_up.focal_findings]))
                        .collect(),
                ),
                provenance_refs: provenance_a
                    .iter()
                    .map(|record| {
                        format!(
                            "{} [{}:{}]",
                            record.export_ref, record.local_kind, record.local_locator
                        )
                    })
                    .collect(),
                artifact_refs,
                source_snapshot_hash_a: Some(record.snapshot_hash_a.clone()),
                source_snapshot_hash_b: record.snapshot_hash_b.clone(),
                source_ai_artifact_id: Some(record.artifact_id.clone()),
            }
        }
    }
}

fn format_snapshot_trend_finding(trend: &snapshot::SnapshotTrendSummary) -> String {
    format_summary_with_descriptor(&trend.label, &trend.summary, trend.evidence.as_ref())
}

fn report_lines_for_findings(findings: &[ai::ArtifactFinding]) -> Vec<String> {
    findings.iter().map(format_artifact_finding).collect()
}

fn format_artifact_finding(finding: &ai::ArtifactFinding) -> String {
    let descriptor = finding.claim_key.as_deref().and_then(|claim_key| {
        finding
            .active_population_profile
            .and_then(|population| resolve_evidence_descriptor(claim_key, population))
            .or_else(|| evidence_descriptor(claim_key))
    });
    format_summary_with_descriptor(&finding.title, &finding.summary, descriptor.as_ref())
}

fn format_summary_with_descriptor(
    label: &str,
    summary: &str,
    descriptor: Option<&EvidenceDescriptor>,
) -> String {
    let badge_block = descriptor
        .map(descriptor_badge_text)
        .filter(|badges| !badges.is_empty())
        .map(|badges| format!(" [{badges}]"));
    badge_block.map_or_else(
        || format!("{label}: {summary}"),
        |badge_block| format!("{label}{badge_block}: {summary}"),
    )
}

fn descriptor_badge_text(descriptor: &EvidenceDescriptor) -> String {
    let mut badges = vec![
        descriptor.evidence_tier.chip_label().to_owned(),
        descriptor.interpretation_scope.label().to_owned(),
        descriptor
            .population_support_status
            .badge_label()
            .to_owned(),
        format!("profile: {}", descriptor.active_population_profile.label()),
    ];
    if let Some(fallback) = descriptor.fallback_population_profile {
        badges.push(format!("fallback: {}", fallback.label()));
    }
    if let Some(anchor) = &descriptor.guidance_anchor_label {
        badges.push(anchor.clone());
    }
    for label in descriptor
        .caution_flags
        .iter()
        .map(|flag| flag.label().to_owned())
    {
        if !badges.contains(&label) {
            badges.push(label);
        }
    }
    badges.join(" | ")
}

fn snapshot_evidence_and_rails(bundle: &SnapshotBundleV1) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Evidence registry version: {}",
            bundle.metadata.evidence_registry_version
        ),
        format!(
            "Active population profile: {}",
            bundle.metadata.active_population_profile.label()
        ),
    ];
    let mut seen_claim_keys = BTreeSet::new();
    for descriptor in bundle
        .trend_summaries
        .iter()
        .filter_map(|trend| trend.evidence.as_ref())
        .chain(
            bundle
                .pattern_summaries
                .iter()
                .filter_map(|pattern| pattern.evidence.as_ref()),
        )
        .chain(
            bundle
                .review_signals
                .iter()
                .filter_map(|signal| signal.evidence.as_ref()),
        )
    {
        if seen_claim_keys.insert(descriptor.claim_key.clone()) {
            lines.push(descriptor_summary_line(descriptor));
        }
    }
    lines
}

fn finding_evidence_and_rails<I, G>(groups: I) -> Vec<String>
where
    I: IntoIterator<Item = G>,
    G: AsRef<[ai::ArtifactFinding]>,
{
    let mut lines = Vec::new();
    let mut seen_claim_keys = BTreeSet::new();
    let mut seen_fallbacks = BTreeSet::new();
    for group in groups {
        for finding in group.as_ref() {
            if let Some(claim_key) = finding.claim_key.as_deref() {
                if seen_claim_keys.insert(claim_key.to_owned()) {
                    if let Some(descriptor) = finding
                        .active_population_profile
                        .and_then(|population| resolve_evidence_descriptor(claim_key, population))
                        .or_else(|| evidence_descriptor(claim_key))
                    {
                        lines.push(descriptor_summary_line(&descriptor));
                        continue;
                    }
                } else {
                    continue;
                }
            }
            let fallback = fallback_finding_rails(finding);
            if !fallback.is_empty() && seen_fallbacks.insert(fallback.clone()) {
                lines.push(fallback);
            }
        }
    }
    if lines.is_empty() {
        lines.push("No registry-backed claim metadata was attached to this artifact.".to_owned());
    }
    lines
}

fn descriptor_summary_line(descriptor: &EvidenceDescriptor) -> String {
    let mut parts = vec![
        descriptor.evidence_tier.chip_label().to_owned(),
        descriptor.interpretation_scope.label().to_owned(),
        format!(
            "population: {} ({})",
            descriptor.population_support_status.detail_label(),
            descriptor.active_population_profile.label()
        ),
    ];
    if let Some(fallback) = descriptor.fallback_population_profile {
        parts.push(format!("fallback anchor: {}", fallback.label()));
    }
    if let Some(anchor) = &descriptor.guidance_anchor_label {
        parts.push(anchor.clone());
    }
    if !descriptor.caution_flags.is_empty() {
        parts.push(format!(
            "rails: {}",
            descriptor
                .caution_flags
                .iter()
                .map(|flag| flag.label())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    format!("{} -> {}", descriptor.label, parts.join("; "))
}

fn fallback_finding_rails(finding: &ai::ArtifactFinding) -> String {
    let mut parts = Vec::new();
    if let Some(tier) = finding.evidence_tier {
        parts.push(tier.chip_label().to_owned());
    }
    if let Some(scope) = finding.interpretation_scope {
        parts.push(scope.label().to_owned());
    }
    let mut seen = BTreeSet::new();
    let caution_labels = finding
        .caution_labels
        .iter()
        .filter(|label| seen.insert((*label).clone()))
        .cloned()
        .collect::<Vec<_>>();
    if !caution_labels.is_empty() {
        parts.push(format!("rails: {}", caution_labels.join(", ")));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("{} -> {}", finding.title, parts.join("; "))
    }
}

fn snapshot_uncertainty_notes(bundle: &SnapshotBundleV1) -> Vec<String> {
    descriptor_uncertainty_notes(
        bundle
            .trend_summaries
            .iter()
            .filter_map(|trend| trend.evidence.as_ref())
            .chain(
                bundle
                    .pattern_summaries
                    .iter()
                    .filter_map(|pattern| pattern.evidence.as_ref()),
            )
            .chain(
                bundle
                    .review_signals
                    .iter()
                    .filter_map(|signal| signal.evidence.as_ref()),
            ),
    )
}

fn finding_uncertainty_notes<I, G>(groups: I) -> impl Iterator<Item = String>
where
    I: IntoIterator<Item = G>,
    G: AsRef<[ai::ArtifactFinding]>,
{
    let mut descriptors = Vec::new();
    for group in groups {
        for finding in group.as_ref() {
            if let Some(claim_key) = finding.claim_key.as_deref()
                && let Some(descriptor) = finding
                    .active_population_profile
                    .and_then(|population| resolve_evidence_descriptor(claim_key, population))
                    .or_else(|| evidence_descriptor(claim_key))
            {
                descriptors.push(descriptor);
            }
        }
    }
    descriptor_uncertainty_notes(descriptors.iter()).into_iter()
}

fn descriptor_uncertainty_notes<'a>(
    descriptors: impl IntoIterator<Item = &'a EvidenceDescriptor>,
) -> Vec<String> {
    let mut notes = Vec::new();
    let mut seen_claim_keys = BTreeSet::new();
    for descriptor in descriptors {
        if !seen_claim_keys.insert(descriptor.claim_key.clone()) {
            continue;
        }
        if let Some(spec) =
            claim_language_spec(&descriptor.claim_key, descriptor.active_population_profile)
        {
            notes.extend(spec.disclaimer_lines);
        }
    }
    unique_evidence_refs(notes)
}

fn compare_report_scope(
    snapshot_a: Option<&SnapshotBundleV1>,
    snapshot_b: Option<&SnapshotBundleV1>,
) -> String {
    match (snapshot_a, snapshot_b) {
        (Some(snapshot_a), Some(snapshot_b)) => {
            format!(
                "{} vs {}",
                snapshot_a.metadata.scope, snapshot_b.metadata.scope
            )
        }
        (Some(snapshot_a), None) => snapshot_a.metadata.scope.clone(),
        (None, Some(snapshot_b)) => snapshot_b.metadata.scope.clone(),
        (None, None) => "unknown".to_owned(),
    }
}

fn compare_report_freshness_summary(
    snapshot_a: Option<&SnapshotBundleV1>,
    provenance_a: &[SnapshotProvenanceRefRecord],
    snapshot_b: Option<&SnapshotBundleV1>,
    provenance_b: &[SnapshotProvenanceRefRecord],
) -> String {
    labeled_snapshot_summary(
        snapshot_a,
        provenance_a,
        snapshot_b,
        provenance_b,
        |summary| summary.freshness_summary,
        "snapshot record unavailable",
    )
}

fn compare_report_trust_summary(
    snapshot_a: Option<&SnapshotBundleV1>,
    provenance_a: &[SnapshotProvenanceRefRecord],
    snapshot_b: Option<&SnapshotBundleV1>,
    provenance_b: &[SnapshotProvenanceRefRecord],
) -> String {
    labeled_snapshot_summary(
        snapshot_a,
        provenance_a,
        snapshot_b,
        provenance_b,
        |summary| summary.trust_summary,
        "lineage available from persisted AI run metadata",
    )
}

fn labeled_snapshot_summary(
    snapshot_a: Option<&SnapshotBundleV1>,
    provenance_a: &[SnapshotProvenanceRefRecord],
    snapshot_b: Option<&SnapshotBundleV1>,
    provenance_b: &[SnapshotProvenanceRefRecord],
    map_summary: impl Fn(snapshot::SnapshotCatalogSummary) -> String,
    missing_label: &str,
) -> String {
    match (snapshot_a, snapshot_b) {
        (Some(snapshot_a), Some(snapshot_b)) => format!(
            "snapshot_a: {}; snapshot_b: {}",
            map_summary(snapshot::summarize_snapshot_bundle(
                snapshot_a,
                provenance_a
            )),
            map_summary(snapshot::summarize_snapshot_bundle(
                snapshot_b,
                provenance_b
            )),
        ),
        (Some(snapshot_a), None) => format!(
            "snapshot_a: {}; snapshot_b: {}",
            map_summary(snapshot::summarize_snapshot_bundle(
                snapshot_a,
                provenance_a
            )),
            missing_label,
        ),
        (None, Some(snapshot_b)) => format!(
            "snapshot_a: {}; snapshot_b: {}",
            missing_label,
            map_summary(snapshot::summarize_snapshot_bundle(
                snapshot_b,
                provenance_b
            )),
        ),
        (None, None) => missing_label.to_owned(),
    }
}

fn unique_evidence_refs(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn render_markdown(document: &ReportDocument) -> String {
    let template = include_str!("report_templates/markdown_v1.md");
    template
        .replace("{{title}}", &document.title)
        .replace("{{generated_at}}", &document.generated_at)
        .replace("{{scope}}", &document.scope)
        .replace("{{privacy_profile}}", &document.privacy_profile)
        .replace("{{ai_used}}", if document.ai_used { "yes" } else { "no" })
        .replace(
            "{{ai_provider}}",
            document.ai_provider.as_deref().unwrap_or("n/a"),
        )
        .replace(
            "{{ai_model}}",
            document.ai_model.as_deref().unwrap_or("n/a"),
        )
        .replace(
            "{{prompt_version}}",
            document.prompt_version.as_deref().unwrap_or("n/a"),
        )
        .replace(
            "{{output_schema_version}}",
            document.output_schema_version.as_deref().unwrap_or("n/a"),
        )
        .replace("{{freshness_summary}}", &document.freshness_summary)
        .replace("{{trust_summary}}", &document.trust_summary)
        .replace(
            "{{key_findings}}",
            &render_markdown_list(&document.key_findings),
        )
        .replace(
            "{{supporting_evidence}}",
            &render_markdown_list(&document.supporting_evidence),
        )
        .replace(
            "{{evidence_and_rails}}",
            &render_markdown_list(&document.evidence_and_rails),
        )
        .replace(
            "{{uncertainty_notes}}",
            &render_markdown_list(&document.uncertainty_notes),
        )
        .replace(
            "{{provenance_refs}}",
            &render_markdown_list(&document.provenance_refs),
        )
        .replace(
            "{{artifact_refs}}",
            &render_markdown_list(&document.artifact_refs),
        )
        .replace("{{template_version}}", MARKDOWN_TEMPLATE_VERSION)
}

fn render_html(document: &ReportDocument) -> String {
    let template = include_str!("report_templates/html_v1.html");
    template
        .replace("{{title}}", &escape_html(&document.title))
        .replace("{{generated_at}}", &escape_html(&document.generated_at))
        .replace("{{scope}}", &escape_html(&document.scope))
        .replace(
            "{{privacy_profile}}",
            &escape_html(&document.privacy_profile),
        )
        .replace("{{ai_used}}", if document.ai_used { "yes" } else { "no" })
        .replace(
            "{{ai_provider}}",
            &escape_html(document.ai_provider.as_deref().unwrap_or("n/a")),
        )
        .replace(
            "{{ai_model}}",
            &escape_html(document.ai_model.as_deref().unwrap_or("n/a")),
        )
        .replace(
            "{{prompt_version}}",
            &escape_html(document.prompt_version.as_deref().unwrap_or("n/a")),
        )
        .replace(
            "{{output_schema_version}}",
            &escape_html(document.output_schema_version.as_deref().unwrap_or("n/a")),
        )
        .replace(
            "{{freshness_summary}}",
            &escape_html(&document.freshness_summary),
        )
        .replace("{{trust_summary}}", &escape_html(&document.trust_summary))
        .replace(
            "{{key_findings}}",
            &render_html_list(&document.key_findings),
        )
        .replace(
            "{{supporting_evidence}}",
            &render_html_list(&document.supporting_evidence),
        )
        .replace(
            "{{evidence_and_rails}}",
            &render_html_list(&document.evidence_and_rails),
        )
        .replace(
            "{{uncertainty_notes}}",
            &render_html_list(&document.uncertainty_notes),
        )
        .replace(
            "{{provenance_refs}}",
            &render_html_list(&document.provenance_refs),
        )
        .replace(
            "{{artifact_refs}}",
            &render_html_list(&document.artifact_refs),
        )
        .replace("{{template_version}}", HTML_TEMPLATE_VERSION)
}

fn render_markdown_list(values: &[String]) -> String {
    if values.is_empty() {
        "- none".to_owned()
    } else {
        values
            .iter()
            .map(|value| format!("- {value}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn render_html_list(values: &[String]) -> String {
    if values.is_empty() {
        "<li>none</li>".to_owned()
    } else {
        values.iter().fold(String::new(), |mut rendered, value| {
            rendered.push_str("<li>");
            rendered.push_str(&escape_html(value));
            rendered.push_str("</li>");
            rendered
        })
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn report_id(
    report_kind: &str,
    snapshot_hash_a: Option<&str>,
    snapshot_hash_b: Option<&str>,
    ai_artifact_id: Option<&str>,
    contents: &[u8],
) -> String {
    let mut digest = Sha256::new();
    digest.update(report_kind.as_bytes());
    if let Some(snapshot_hash_a) = snapshot_hash_a {
        digest.update(snapshot_hash_a.as_bytes());
    }
    if let Some(snapshot_hash_b) = snapshot_hash_b {
        digest.update(snapshot_hash_b.as_bytes());
    }
    if let Some(ai_artifact_id) = ai_artifact_id {
        digest.update(ai_artifact_id.as_bytes());
    }
    digest.update(contents);
    hex::encode(digest.finalize())
}

fn content_hash(contents: &[u8]) -> String {
    hex::encode(Sha256::digest(contents))
}

const fn report_format_label(format: ReportFormatArg) -> &'static str {
    match format {
        ReportFormatArg::Markdown => "markdown",
        ReportFormatArg::Html => "html",
    }
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| RingmasterError::Ui(format!("failed to format report timestamp: {error}")))
}

fn write_text_file(path: &Path, contents: &str, context: &'static str) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| RingmasterError::io(context, error))?;
    }
    fs::write(path, contents).map_err(|error| RingmasterError::io(context, error))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{ReportDocument, build_ai_report_document, render_html, render_markdown};
    use crate::ai::{
        ArtifactEvidenceRef, ArtifactFinding, ArtifactStatus, CompareArtifactV1, ConfidenceLevel,
        StoredArtifact, SufficiencyLevel,
    };
    use crate::snapshot::{
        PrivacyProfile, SnapshotBundleV1, SnapshotCapabilities, SnapshotCapabilityEntry,
        SnapshotFollowUpTarget, SnapshotFreshness, SnapshotMetadata, SnapshotMetrics,
        SnapshotRecordCounts, SnapshotReviewSignal, SnapshotSourceMode, SnapshotSyncState,
    };
    use crate::store::queries::{AiArtifactRecord, SnapshotProvenanceRefRecord};

    fn sample_document() -> ReportDocument {
        ReportDocument {
            report_kind: "snapshot_report".to_owned(),
            title: "Snapshot report: today".to_owned(),
            generated_at: "2026-04-10T00:00:00Z".to_owned(),
            scope: "today".to_owned(),
            privacy_profile: "redacted".to_owned(),
            ai_used: false,
            ai_provider: None,
            ai_model: None,
            prompt_version: None,
            output_schema_version: None,
            freshness_summary:
                "latest_source_day=2026-04-10 latest_review_day=2026-04-10 warnings=0".to_owned(),
            trust_summary: "review_signals=2 strong=1 stale=0 follow_up_targets=1".to_owned(),
            key_findings: vec!["Readiness improved.".to_owned()],
            supporting_evidence: vec!["daily:2026-04-10 -> daily_overview:2026-04-10".to_owned()],
            evidence_and_rails: vec![
                "Evidence registry version: ringmaster.evidence.v1".to_owned(),
            ],
            uncertainty_notes: vec![
                "No freshness warnings were recorded in this snapshot.".to_owned(),
            ],
            provenance_refs: vec!["daily:2026-04-10 [daily_overview:2026-04-10]".to_owned()],
            artifact_refs: vec!["snapshot_hash=hash-123".to_owned()],
            source_snapshot_hash_a: Some("hash-123".to_owned()),
            source_snapshot_hash_b: None,
            source_ai_artifact_id: None,
        }
    }

    fn sample_snapshot(
        scope: &str,
        latest_source_day: &str,
        latest_review_day: &str,
        warnings: &[&str],
        review_signal_count: usize,
        strong_signal_count: usize,
        follow_up_target_count: usize,
    ) -> SnapshotBundleV1 {
        let signals = (0..review_signal_count)
            .map(|index| SnapshotReviewSignal {
                export_ref: format!("signal:{scope}:{index}"),
                day: "2026-04-10".to_owned(),
                signal_key: format!("signal_{index}"),
                numeric_value: Some(crate::numeric::usize_to_f64(index)),
                text_value: None,
                delta: Some(crate::numeric::usize_to_f64(index)),
                z_score: Some(crate::numeric::usize_to_f64(index)),
                persistence_days: 1,
                sufficiency: if index < strong_signal_count {
                    "strong".to_owned()
                } else {
                    "medium".to_owned()
                },
                stale_days: u32::from(index + 1 == review_signal_count),
                evidence: None,
            })
            .collect::<Vec<_>>();
        let follow_up_targets = (0..follow_up_target_count)
            .map(|index| SnapshotFollowUpTarget {
                label: format!("target-{index}"),
                command: "review investigate --focus readiness --anchor-day 2026-04-10".to_owned(),
                reason: "Inspect local review output.".to_owned(),
            })
            .collect::<Vec<_>>();

        SnapshotBundleV1 {
            schema_version: crate::snapshot::SNAPSHOT_SCHEMA_VERSION.to_owned(),
            metadata: SnapshotMetadata {
                app_version: "0.1.0".to_owned(),
                generated_at: "2026-04-10T00:00:00Z".to_owned(),
                snapshot_hash: format!("hash-{scope}"),
                scope: scope.to_owned(),
                start_day: "2026-04-10".to_owned(),
                end_day: "2026-04-10".to_owned(),
                anchor_day: "2026-04-10".to_owned(),
                privacy_profile: PrivacyProfile::Redacted,
                active_population_profile: crate::evidence::PopulationProfile::GeneralAdult,
                source_mode: SnapshotSourceMode::Demo,
                schema_version: 13,
                evidence_registry_version: crate::evidence::registry::evidence_registry_version()
                    .to_owned(),
            },
            freshness: SnapshotFreshness {
                latest_source_day: Some(latest_source_day.to_owned()),
                latest_review_day: Some(latest_review_day.to_owned()),
                warnings: warnings
                    .iter()
                    .map(|warning| (*warning).to_owned())
                    .collect(),
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
                context_events: 0,
                pattern_summaries: 0,
                review_signals: signals.len(),
                raw_tables: BTreeMap::default(),
            },
            metrics: SnapshotMetrics {
                daily_scores: Vec::new(),
                activity: Vec::new(),
                heartrate_daily_averages: Vec::new(),
                sleep_windows: Vec::new(),
                stress: Vec::new(),
                resilience: Vec::new(),
                cardiovascular_age: Vec::new(),
                vo2_max: Vec::new(),
                rest_mode_periods: Vec::new(),
            },
            baselines: Vec::new(),
            trend_summaries: Vec::new(),
            context_events: Vec::new(),
            pattern_summaries: Vec::new(),
            review_signals: signals,
            follow_up_targets,
        }
    }

    fn sample_compare_record() -> AiArtifactRecord {
        AiArtifactRecord {
            artifact_id: "artifact-compare".to_owned(),
            artifact_kind: "compare".to_owned(),
            output_schema_version: "ringmaster.ai.compare.v2".to_owned(),
            prompt_version: "compare_prompt_v2".to_owned(),
            provider: "dry_run".to_owned(),
            model: "deterministic".to_owned(),
            reasoning_effort: None,
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            run_mode: "dry_run".to_owned(),
            created_at: "2026-04-10T00:05:00Z".to_owned(),
            snapshot_hash_a: "hash-today".to_owned(),
            snapshot_hash_b: Some("hash-week".to_owned()),
            privacy_profile: "redacted".to_owned(),
            artifact_status: "dry_run".to_owned(),
            overview: "compare overview".to_owned(),
            summary_cache: "compare summary".to_owned(),
            request_fingerprint: Some("fingerprint-compare".to_owned()),
            payload_json: "{\"status\":\"dry_run\"}".to_owned(),
            rendered_briefing: "ringmaster ai compare".to_owned(),
        }
    }

    #[test]
    fn markdown_renderer_includes_sections() {
        let rendered = render_markdown(&sample_document());
        assert!(rendered.contains("# Snapshot report: today"));
        assert!(rendered.contains("## Key Findings"));
        assert!(rendered.contains("## Provenance References"));
    }

    #[test]
    fn html_renderer_includes_sections() {
        let rendered = render_html(&sample_document());
        assert!(rendered.contains("<h1>Snapshot report: today</h1>"));
        assert!(rendered.contains("Key Findings"));
        assert!(rendered.contains("Provenance References"));
    }

    #[test]
    fn compare_reports_surface_freshness_and_trust_for_both_snapshots() {
        let snapshot_a = sample_snapshot("today", "2026-04-10", "2026-04-10", &[], 1, 1, 1);
        let snapshot_b = sample_snapshot(
            "week",
            "2026-04-07",
            "2026-04-08",
            &["stale daily_stress import"],
            3,
            1,
            2,
        );
        let document = build_ai_report_document(
            &sample_compare_record(),
            &StoredArtifact::Compare(CompareArtifactV1 {
                schema_version: "ringmaster.ai.compare.v2".to_owned(),
                prompt_version: "compare_prompt_v2".to_owned(),
                status: ArtifactStatus::DryRun,
                overview: "compare overview".to_owned(),
                material_differences: Vec::new(),
                supporting_evidence: Vec::new(),
                uncertainty_warnings: Vec::new(),
                investigation_targets: Vec::new(),
                only_in_a: Vec::new(),
                only_in_b: Vec::new(),
            }),
            Some(&snapshot_a),
            &[SnapshotProvenanceRefRecord {
                snapshot_hash: "hash-today".to_owned(),
                export_ref: "daily:2026-04-10".to_owned(),
                local_kind: "daily_overview".to_owned(),
                local_locator: "2026-04-10".to_owned(),
                created_at: "2026-04-10T00:00:00Z".to_owned(),
            }],
            Some(&snapshot_b),
            &[SnapshotProvenanceRefRecord {
                snapshot_hash: "hash-week".to_owned(),
                export_ref: "daily:2026-04-08".to_owned(),
                local_kind: "daily_overview".to_owned(),
                local_locator: "2026-04-08".to_owned(),
                created_at: "2026-04-08T00:00:00Z".to_owned(),
            }],
        );

        assert_eq!(document.scope, "today vs week");
        assert!(document.freshness_summary.contains("snapshot_a:"));
        assert!(document.freshness_summary.contains("snapshot_b:"));
        assert!(
            document
                .freshness_summary
                .contains("latest_source_day=2026-04-07")
        );
        assert!(document.trust_summary.contains("snapshot_a:"));
        assert!(document.trust_summary.contains("snapshot_b:"));
        assert!(document.trust_summary.contains("review_signals=3"));
        assert!(document.trust_summary.contains("follow_up_targets=2"));
    }

    #[test]
    fn compare_reports_include_per_difference_evidence_when_top_level_evidence_is_empty() {
        let document = build_ai_report_document(
            &sample_compare_record(),
            &StoredArtifact::Compare(CompareArtifactV1 {
                schema_version: "ringmaster.ai.compare.v2".to_owned(),
                prompt_version: "compare_prompt_v2".to_owned(),
                status: ArtifactStatus::DryRun,
                overview: "compare overview".to_owned(),
                material_differences: vec![ArtifactFinding {
                    finding_id: "diff-1".to_owned(),
                    title: "Training load changed".to_owned(),
                    summary: "Activity load differs between snapshots.".to_owned(),
                    claim_key: Some("weekly_activity_minutes".to_owned()),
                    evidence_tier: Some(crate::evidence::registry::EvidenceTier::GuidelineBacked),
                    interpretation_scope: Some(
                        crate::evidence::registry::InterpretationScope::CrossSectional,
                    ),
                    active_population_profile: Some(
                        crate::evidence::PopulationProfile::GeneralAdult,
                    ),
                    population_support_status: Some(
                        crate::evidence::registry::PopulationSupportStatus::PopulationSpecific,
                    ),
                    fallback_population_profile: None,
                    caution_labels: Vec::new(),
                    confidence: ConfidenceLevel::Medium,
                    sufficiency: SufficiencyLevel::Medium,
                    evidence_refs: vec![ArtifactEvidenceRef {
                        export_ref: "daily:2026-04-10".to_owned(),
                        note: "Snapshot A activity score".to_owned(),
                    }],
                    counterevidence_refs: vec![ArtifactEvidenceRef {
                        export_ref: "daily:2026-04-08".to_owned(),
                        note: "Snapshot B recovery trend".to_owned(),
                    }],
                }],
                supporting_evidence: Vec::new(),
                uncertainty_warnings: Vec::new(),
                investigation_targets: Vec::new(),
                only_in_a: Vec::new(),
                only_in_b: Vec::new(),
            }),
            None,
            &[],
            None,
            &[],
        );

        assert_eq!(document.supporting_evidence.len(), 2);
        assert!(
            document
                .supporting_evidence
                .contains(&"daily:2026-04-10: Snapshot A activity score".to_owned())
        );
        assert!(
            document
                .supporting_evidence
                .contains(&"daily:2026-04-08: Snapshot B recovery trend".to_owned())
        );
    }
}
