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
use crate::resolved_demo_fixture_dir;
use crate::snapshot::{self, LoadedSnapshotArtifact, SnapshotBundleV1};
use crate::store::Store;
use crate::store::queries::{AiArtifactRecord, ReportExportRecord, SnapshotProvenanceRefRecord};

const MARKDOWN_TEMPLATE_VERSION: &str = "report_markdown_v1";
const HTML_TEMPLATE_VERSION: &str = "report_html_v1";

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

    let Some(record) = store.analysis().snapshot_export(spec)? else {
        return Err(RingmasterError::Ui(format!(
            "snapshot `{spec}` was not found in the local catalog and is not a readable file path"
        )));
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
    let Some(record) = store.analysis().ai_artifact(run_id)? else {
        return Err(RingmasterError::Ui(format!(
            "AI run `{run_id}` was not found in the local registry"
        )));
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
        .map(|trend| format!("{}: {}", trend.label, trend.summary))
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
    let mut uncertainty_notes = bundle.freshness.warnings.clone();
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
        uncertainty_notes,
        provenance_refs: provenance
            .iter()
            .map(|record| {
                format!(
                    "{} [{}:{}]",
                    record.export_ref, record.local_kind, record.local_locator
                )
            })
            .collect(),
        artifact_refs: vec![format!("snapshot_hash={}", bundle.metadata.snapshot_hash)],
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
    let (scope, privacy_profile, freshness_summary, trust_summary) = snapshot_a.map_or_else(
        || {
            (
                "unknown".to_owned(),
                record.privacy_profile.clone(),
                "snapshot record unavailable".to_owned(),
                "lineage available from persisted AI run metadata".to_owned(),
            )
        },
        |snapshot_a| {
            let summary = snapshot::summarize_snapshot_bundle(snapshot_a, provenance_a);
            (
                snapshot_a.metadata.scope.clone(),
                record.privacy_profile.clone(),
                summary.freshness_summary,
                summary.trust_summary,
            )
        },
    );

    let mut artifact_refs = vec![format!("ai_run={}", record.artifact_id)];
    artifact_refs.push(format!("snapshot_a={}", record.snapshot_hash_a));
    if let Some(snapshot_hash_b) = &record.snapshot_hash_b {
        artifact_refs.push(format!("snapshot_b={snapshot_hash_b}"));
    }

    match artifact {
        StoredArtifact::Review(review) => ReportDocument {
            report_kind: "ai_review_report".to_owned(),
            title: "AI review report".to_owned(),
            generated_at: record.created_at.clone(),
            scope,
            privacy_profile,
            ai_used: true,
            ai_provider: Some(record.provider.clone()),
            ai_model: Some(record.model.clone()),
            prompt_version: Some(record.prompt_version.clone()),
            output_schema_version: Some(record.output_schema_version.clone()),
            freshness_summary,
            trust_summary,
            key_findings: review
                .headline_findings
                .iter()
                .map(|finding| format!("{}: {}", finding.title, finding.summary))
                .collect(),
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
                            .map(|evidence| format!("{}: {}", evidence.export_ref, evidence.note)),
                    )
                    .collect(),
            ),
            uncertainty_notes: review
                .unresolved_questions
                .iter()
                .cloned()
                .chain(
                    review
                        .limitations
                        .iter()
                        .map(|limitation| limitation.message.clone()),
                )
                .collect(),
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
        },
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

            let combined_scope = if let Some(snapshot_b) = snapshot_b {
                format!("{} vs {}", scope, snapshot_b.metadata.scope)
            } else {
                scope
            };

            ReportDocument {
                report_kind: "ai_compare_report".to_owned(),
                title: "AI compare report".to_owned(),
                generated_at: record.created_at.clone(),
                scope: combined_scope,
                privacy_profile,
                ai_used: true,
                ai_provider: Some(record.provider.clone()),
                ai_model: Some(record.model.clone()),
                prompt_version: Some(record.prompt_version.clone()),
                output_schema_version: Some(record.output_schema_version.clone()),
                freshness_summary,
                trust_summary,
                key_findings: compare
                    .material_differences
                    .iter()
                    .map(|finding| format!("{}: {}", finding.title, finding.summary))
                    .collect(),
                supporting_evidence: unique_evidence_refs(
                    compare
                        .supporting_evidence
                        .iter()
                        .map(|evidence| format!("{}: {}", evidence.export_ref, evidence.note))
                        .collect(),
                ),
                uncertainty_notes: compare.uncertainty_warnings.clone(),
                provenance_refs,
                artifact_refs,
                source_snapshot_hash_a: Some(record.snapshot_hash_a.clone()),
                source_snapshot_hash_b: record.snapshot_hash_b.clone(),
                source_ai_artifact_id: Some(record.artifact_id.clone()),
            }
        }
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

fn report_format_label(format: ReportFormatArg) -> &'static str {
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
    use super::{ReportDocument, render_html, render_markdown};

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
}
