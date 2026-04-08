use std::future::Future;
use std::path::PathBuf;
use std::time::Duration as StdDuration;
use std::{collections::HashMap, fmt::Write as _};

use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::config::{Config, RefreshConfig};
use crate::error::{Result, RingmasterError};
use crate::oura::models::CapabilityKind;
use crate::oura::sync::{SliceReport, SyncOptions, SyncReport, sync_selected};
use crate::store::Store;
use crate::store::queries::SyncStateRecord;
use crate::store::webhook_store::{InvalidationRecord, RuntimeHeartbeatRecord, now_rfc3339};
use crate::webhook::WebhookEventType;
use crate::webhook::sync_family_for_data_type;

const WATCH_COMPONENT: &str = "sync.watch";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SyncFamily {
    Personal,
    Daily,
    Heartrate,
    Workout,
    EnhancedTag,
    Session,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WatchOptions {
    pub dry_run: bool,
    pub demo: bool,
    pub fixture_dir: Option<PathBuf>,
    pub max_iterations: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchReport {
    pub iterations: u32,
    pub dry_run: bool,
    pub demo: bool,
    pub database_path: String,
    pub last_report: Option<SyncReport>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidationRunReport {
    pub claimed_invalidations: usize,
    pub families: Vec<SyncFamily>,
    pub sync_report: Option<SyncReport>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SyncInterruption<T> {
    Completed(T),
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClaimedInvalidation {
    record: InvalidationRecord,
    attempt_id: Option<i64>,
}

impl SyncFamily {
    pub const ALL: [Self; 6] = [
        Self::Personal,
        Self::Daily,
        Self::Heartrate,
        Self::Workout,
        Self::EnhancedTag,
        Self::Session,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Daily => "daily",
            Self::Heartrate => "heartrate",
            Self::Workout => "workout",
            Self::EnhancedTag => "enhanced_tag",
            Self::Session => "session",
        }
    }

    pub fn sync_key(self) -> &'static str {
        match self {
            Self::Personal => "oura.personal",
            Self::Daily => "oura.daily",
            Self::Heartrate => "oura.heartrate",
            Self::Workout => "oura.workouts",
            Self::EnhancedTag => "oura.enhanced_tags",
            Self::Session => "oura.sessions",
        }
    }

    pub fn capability_kind(self) -> CapabilityKind {
        match self {
            Self::Personal => CapabilityKind::Personal,
            Self::Daily => CapabilityKind::Daily,
            Self::Heartrate => CapabilityKind::Heartrate,
            Self::Workout => CapabilityKind::Workout,
            Self::EnhancedTag => CapabilityKind::EnhancedTag,
            Self::Session => CapabilityKind::Session,
        }
    }

    pub fn interval_secs(self, refresh: &RefreshConfig) -> u64 {
        match self {
            Self::Personal => refresh.personal_interval_secs,
            Self::Daily => refresh.daily_interval_secs,
            Self::Heartrate => refresh.heartrate_interval_secs,
            Self::Workout => refresh.workout_interval_secs,
            Self::EnhancedTag => refresh.enhanced_tag_interval_secs,
            Self::Session => refresh.session_interval_secs,
        }
    }

    pub fn stale_after_secs(self, refresh: &RefreshConfig) -> u64 {
        match self {
            Self::Personal => refresh.personal_stale_after_secs,
            Self::Daily => refresh.daily_stale_after_secs,
            Self::Heartrate => refresh.heartrate_stale_after_secs,
            Self::Workout => refresh.workout_stale_after_secs,
            Self::EnhancedTag => refresh.enhanced_tag_stale_after_secs,
            Self::Session => refresh.session_stale_after_secs,
        }
    }
}

pub fn due_families(
    config: &Config,
    sync_states: &[SyncStateRecord],
    now: OffsetDateTime,
    force_all: bool,
) -> Result<Vec<SyncFamily>> {
    let mut families = Vec::new();

    for family in SyncFamily::ALL {
        if force_all || family_is_due(config, sync_states, family, now)? {
            families.push(family);
        }
    }

    Ok(families)
}

pub fn next_wake_duration(
    config: &Config,
    sync_states: &[SyncStateRecord],
    now: OffsetDateTime,
) -> Result<StdDuration> {
    let next_due = SyncFamily::ALL
        .into_iter()
        .map(|family| next_due_at(config, sync_states, family, now))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .min()
        .unwrap_or(now + Duration::seconds(1));

    let delay = (next_due - now).whole_seconds().max(1);
    Ok(StdDuration::from_secs(delay as u64))
}

pub async fn run_watch(config: &Config, options: WatchOptions) -> Result<WatchReport> {
    let store = Store::open(config)?;
    let fixture_dir = resolve_fixture_dir(config, &options);
    let dry_run = options.dry_run || options.demo;
    upsert_watch_heartbeat(
        &store,
        config,
        watch_mode_label(config),
        Some("watch loop starting".to_owned()),
    )?;
    if options.max_iterations == Some(0) {
        return Ok(WatchReport {
            iterations: 0,
            dry_run,
            demo: options.demo,
            database_path: store.plan().db_path.display().to_string(),
            last_report: None,
            notes: vec![
                "watch loop stopped before syncing because max_iterations was set to 0".to_owned(),
            ],
        });
    }
    let mut simulated_sync_states = if dry_run {
        Some(store.sync_state().list()?)
    } else {
        None
    };
    let mut iterations = 0_u32;
    let mut last_report = None;
    let mut notes = Vec::new();

    loop {
        upsert_watch_heartbeat(
            &store,
            config,
            watch_mode_label(config),
            Some("watch loop idle".to_owned()),
        )?;

        let invalidation_report = match await_sync_or_interrupt(
            process_pending_invalidations_once(config, &store, dry_run, fixture_dir.clone()),
            tokio::signal::ctrl_c(),
        )
        .await?
        {
            SyncInterruption::Completed(report) => report,
            SyncInterruption::Interrupted => {
                notes.push("watch loop interrupted by ctrl-c".to_owned());
                break;
            }
        };
        if let Some(report) = invalidation_report.sync_report.clone() {
            if let Some(simulated_sync_states) = simulated_sync_states.as_mut() {
                advance_dry_run_sync_states(
                    config,
                    simulated_sync_states,
                    &report,
                    "webhook",
                    "simulated webhook-triggered reconcile",
                );
            }
            notes.extend(invalidation_report.notes.clone());
            iterations = iterations.saturating_add(1);
            last_report = Some(report);

            if options
                .max_iterations
                .is_some_and(|max_iterations| iterations >= max_iterations)
            {
                notes.push(format!(
                    "watch loop stopped after {} bounded iteration(s)",
                    iterations
                ));
                break;
            }
            continue;
        }
        notes.extend(invalidation_report.notes);

        let sync_states = if let Some(sync_states) = simulated_sync_states.as_ref() {
            sync_states.clone()
        } else {
            store.sync_state().list()?
        };
        let force_all = options.max_iterations.is_some() && iterations == 0;
        let families = due_families(config, &sync_states, OffsetDateTime::now_utc(), force_all)?;

        if families.is_empty() {
            let sleep_for = next_wake_duration(config, &sync_states, OffsetDateTime::now_utc())?;
            tokio::select! {
                _ = tokio::time::sleep(sleep_for) => {}
                signal = tokio::signal::ctrl_c() => {
                    signal.map_err(|error| RingmasterError::Config(format!("failed to listen for ctrl-c: {error}")))?;
                    notes.push("watch loop interrupted by ctrl-c".to_owned());
                    break;
                }
            }
            continue;
        }

        let report = match await_sync_or_interrupt(
            sync_selected(
                config,
                &store,
                SyncOptions {
                    dry_run,
                    fixture_dir: fixture_dir.clone(),
                    families,
                    trigger_source: Some("periodic_reconcile".to_owned()),
                    trigger_detail: Some("sync watch scheduler".to_owned()),
                },
            ),
            tokio::signal::ctrl_c(),
        )
        .await?
        {
            SyncInterruption::Completed(report) => report,
            SyncInterruption::Interrupted => {
                notes.push("watch loop interrupted by ctrl-c".to_owned());
                break;
            }
        };
        if let Some(simulated_sync_states) = simulated_sync_states.as_mut() {
            advance_dry_run_sync_states(
                config,
                simulated_sync_states,
                &report,
                "periodic_reconcile",
                "sync watch scheduler",
            );
        }
        iterations = iterations.saturating_add(1);
        last_report = Some(report);

        if options
            .max_iterations
            .is_some_and(|max_iterations| iterations >= max_iterations)
        {
            notes.push(format!(
                "watch loop stopped after {} bounded iteration(s)",
                iterations
            ));
            break;
        }
    }

    Ok(WatchReport {
        iterations,
        dry_run,
        demo: options.demo,
        database_path: store.plan().db_path.display().to_string(),
        last_report,
        notes,
    })
}

pub async fn process_pending_invalidations_once(
    config: &Config,
    store: &Store,
    dry_run: bool,
    fixture_dir: Option<PathBuf>,
) -> Result<InvalidationRunReport> {
    let now = now_rfc3339()?;
    let lease_until = (OffsetDateTime::now_utc() + Duration::seconds(30))
        .format(&Rfc3339)
        .map_err(|error| {
            RingmasterError::Config(format!(
                "failed to format webhook invalidation lease timestamp: {error}"
            ))
        })?;
    let claimed = if dry_run {
        preview_due_invalidations(store, &now)?
            .into_iter()
            .map(|record| ClaimedInvalidation {
                record,
                attempt_id: None,
            })
            .collect::<Vec<_>>()
    } else {
        store
            .webhook()
            .claim_available_invalidations(
                &format!("watch-{}", std::process::id()),
                &now,
                &lease_until,
                128,
            )?
            .into_iter()
            .map(|record| ClaimedInvalidation {
                record,
                attempt_id: None,
            })
            .collect::<Vec<_>>()
    };
    if claimed.is_empty() {
        return Ok(InvalidationRunReport {
            claimed_invalidations: 0,
            families: Vec::new(),
            sync_report: None,
            notes: Vec::new(),
        });
    }

    let families = claimed
        .iter()
        .filter_map(|claimed| sync_family_for_data_type(&claimed.record.data_type))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if families.is_empty() {
        if !dry_run {
            for claimed in &claimed {
                let started_at = now_rfc3339()?;
                let attempt = store
                    .webhook()
                    .start_processing_attempt(claimed.record.invalidation_id, &started_at)?;
                store.webhook().complete_processing_attempt_success(
                    claimed.record.invalidation_id,
                    attempt.attempt_id,
                    &now_rfc3339()?,
                    Some("unsupported invalidation family"),
                )?;
            }
        }
        return Ok(InvalidationRunReport {
            claimed_invalidations: claimed.len(),
            families,
            sync_report: None,
            notes: vec![
                "Webhook invalidations were present but none mapped to supported sync families."
                    .to_owned(),
            ],
        });
    }

    let claimed = if dry_run {
        claimed
    } else {
        let mut started = Vec::with_capacity(claimed.len());
        for claimed_record in claimed {
            let attempt = store
                .webhook()
                .start_processing_attempt(claimed_record.record.invalidation_id, &now_rfc3339()?)?;
            started.push(ClaimedInvalidation {
                record: claimed_record.record,
                attempt_id: Some(attempt.attempt_id),
            });
        }
        started
    };

    let trigger_detail = build_invalidation_trigger_detail(
        &claimed
            .iter()
            .map(|claimed| claimed.record.clone())
            .collect::<Vec<_>>(),
    );
    let report = sync_selected(
        config,
        store,
        SyncOptions {
            dry_run,
            fixture_dir,
            families: families.clone(),
            trigger_source: Some("webhook".to_owned()),
            trigger_detail: Some(trigger_detail),
        },
    )
    .await;
    let report = match report {
        Ok(report) => report,
        Err(error) => {
            if !dry_run {
                record_global_invalidation_failure(config, store, &claimed, &error.to_string())?;
            }
            return Err(error);
        }
    };
    let notes = if dry_run {
        vec![format!(
            "Previewed {} pending invalidation(s) across {} family/families.",
            claimed.len(),
            families.len()
        )]
    } else {
        settle_processed_invalidations(config, store, &claimed, &report)?
    };

    Ok(InvalidationRunReport {
        claimed_invalidations: claimed.len(),
        families,
        sync_report: Some(report),
        notes,
    })
}

async fn await_sync_or_interrupt<T, SyncFuture, InterruptFuture>(
    sync_future: SyncFuture,
    interrupt_future: InterruptFuture,
) -> Result<SyncInterruption<T>>
where
    SyncFuture: Future<Output = Result<T>>,
    InterruptFuture: Future<Output = std::result::Result<(), std::io::Error>>,
{
    tokio::select! {
        sync_result = sync_future => sync_result.map(SyncInterruption::Completed),
        signal = interrupt_future => {
            signal.map_err(|error| RingmasterError::Config(format!("failed to listen for ctrl-c: {error}")))?;
            Ok(SyncInterruption::Interrupted)
        }
    }
}

fn resolve_fixture_dir(config: &Config, options: &WatchOptions) -> Option<PathBuf> {
    options.fixture_dir.clone().or_else(|| {
        options.demo.then(|| {
            config
                .refresh
                .demo_fixture_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from("tests/fixtures/phase3"))
        })
    })
}

fn preview_due_invalidations(store: &Store, now: &str) -> Result<Vec<InvalidationRecord>> {
    Ok(store
        .webhook()
        .list_pending_invalidations()?
        .into_iter()
        .filter(|record| record.available_at.as_str() <= now)
        .collect::<Vec<_>>())
}

fn settle_processed_invalidations(
    config: &Config,
    store: &Store,
    invalidations: &[ClaimedInvalidation],
    report: &SyncReport,
) -> Result<Vec<String>> {
    let mut notes = Vec::new();
    let mut deleted_rows = 0_u32;

    for invalidation in invalidations {
        let Some(family) = sync_family_for_data_type(&invalidation.record.data_type) else {
            store.webhook().complete_processing_attempt_success(
                invalidation.record.invalidation_id,
                invalidation.attempt_id.ok_or_else(|| {
                    RingmasterError::Config(
                        "missing webhook processing attempt for claimed invalidation".to_owned(),
                    )
                })?,
                &now_rfc3339()?,
                Some("unsupported invalidation family"),
            )?;
            continue;
        };
        let slice = report
            .slice_reports
            .iter()
            .find(|slice| slice.sync_key == family.sync_key());
        let Some(slice) = slice else {
            let next_available_at =
                compute_invalidation_retry_at(config, family, &invalidation.record)?;
            let failed = store.webhook().complete_processing_attempt_failure(
                invalidation.record.invalidation_id,
                invalidation.attempt_id.ok_or_else(|| {
                    RingmasterError::Config(
                        "missing webhook processing attempt for claimed invalidation".to_owned(),
                    )
                })?,
                &now_rfc3339()?,
                &next_available_at,
                "no sync slice was produced for the claimed invalidation family",
            )?;
            notes.push(format!(
                "{} invalidation retried after missing sync slice (attempt_count={}).",
                family.label(),
                failed.attempt_count
            ));
            continue;
        };

        if slice.status == crate::store::queries::SyncRunStatus::Success {
            deleted_rows += u32::from(apply_delete_side_effect(store, &invalidation.record)?);
            store.webhook().complete_processing_attempt_success(
                invalidation.record.invalidation_id,
                invalidation.attempt_id.ok_or_else(|| {
                    RingmasterError::Config(
                        "missing webhook processing attempt for claimed invalidation".to_owned(),
                    )
                })?,
                &now_rfc3339()?,
                Some(&slice.message),
            )?;
        } else {
            let next_available_at =
                compute_invalidation_retry_at(config, family, &invalidation.record)?;
            let failed = store.webhook().complete_processing_attempt_failure(
                invalidation.record.invalidation_id,
                invalidation.attempt_id.ok_or_else(|| {
                    RingmasterError::Config(
                        "missing webhook processing attempt for claimed invalidation".to_owned(),
                    )
                })?,
                &now_rfc3339()?,
                &next_available_at,
                &slice.message,
            )?;
            notes.push(format!(
                "{} invalidation queued for retry at {} (attempt_count={}).",
                family.label(),
                failed.available_at,
                failed.attempt_count
            ));
        }
    }

    if deleted_rows > 0 {
        let derive_report = crate::derive::rebuild_recent_store(store, config)?;
        notes.push(format!(
            "Applied {} webhook delete side-effect(s) and rebuilt derived state (events={}, patterns={}).",
            deleted_rows, derive_report.context_event_count, derive_report.pattern_summary_count
        ));
    } else {
        notes.push(format!(
            "Processed {} webhook invalidation(s).",
            invalidations.len()
        ));
    }

    Ok(notes)
}

fn apply_delete_side_effect(store: &Store, invalidation: &InvalidationRecord) -> Result<bool> {
    if invalidation.event_type != WebhookEventType::Delete {
        return Ok(false);
    }
    let Some(object_id) = invalidation.object_id.as_deref() else {
        return Ok(false);
    };

    match invalidation.data_type.as_str() {
        "workout" => {
            store.imports().delete_workout(object_id)?;
            Ok(true)
        }
        "enhanced_tag" => {
            store.imports().delete_enhanced_tag(object_id)?;
            Ok(true)
        }
        "session" => {
            store.imports().delete_session(object_id)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn compute_invalidation_retry_at(
    config: &Config,
    family: SyncFamily,
    invalidation: &InvalidationRecord,
) -> Result<String> {
    let failure_count = invalidation.attempt_count.saturating_add(1);
    let base_interval_secs = family.interval_secs(&config.refresh);
    let capped_shift = failure_count.saturating_sub(1).min(6);
    let multiplier = 1_u64 << capped_shift;
    let backoff_secs = base_interval_secs
        .saturating_mul(multiplier)
        .min(config.refresh.max_backoff_secs);

    (OffsetDateTime::now_utc() + Duration::seconds(backoff_secs as i64))
        .format(&Rfc3339)
        .map_err(|error| {
            RingmasterError::Config(format!(
                "failed to format webhook invalidation retry timestamp: {error}"
            ))
        })
}

fn build_invalidation_trigger_detail(invalidations: &[InvalidationRecord]) -> String {
    invalidations
        .iter()
        .take(6)
        .map(|record| {
            format!(
                "{}:{}:{}",
                record.data_type,
                record.event_type.as_str(),
                record.object_id.as_deref().unwrap_or("*")
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn record_global_invalidation_failure(
    config: &Config,
    store: &Store,
    invalidations: &[ClaimedInvalidation],
    detail: &str,
) -> Result<()> {
    let mut failures_by_family = HashMap::<SyncFamily, String>::new();
    for invalidation in invalidations {
        if let Some(family) = sync_family_for_data_type(&invalidation.record.data_type) {
            let entry = failures_by_family.entry(family).or_insert_with(|| {
                let mut rendered = String::new();
                let _ = write!(&mut rendered, "{detail}");
                rendered
            });
            if entry != detail {
                detail.clone_into(entry);
            }
        }
    }

    for invalidation in invalidations {
        let Some(family) = sync_family_for_data_type(&invalidation.record.data_type) else {
            continue;
        };
        let next_available_at =
            compute_invalidation_retry_at(config, family, &invalidation.record)?;
        store.webhook().complete_processing_attempt_failure(
            invalidation.record.invalidation_id,
            invalidation.attempt_id.ok_or_else(|| {
                RingmasterError::Config(
                    "missing webhook processing attempt for claimed invalidation".to_owned(),
                )
            })?,
            &now_rfc3339()?,
            &next_available_at,
            failures_by_family
                .get(&family)
                .map(String::as_str)
                .unwrap_or(detail),
        )?;
    }

    Ok(())
}

fn watch_mode_label(config: &Config) -> &'static str {
    if config.webhook.receiver_configured() {
        "hybrid"
    } else {
        "scheduler_only"
    }
}

fn upsert_watch_heartbeat(
    store: &Store,
    config: &Config,
    mode: &str,
    detail: Option<String>,
) -> Result<()> {
    store
        .webhook()
        .upsert_runtime_heartbeat(&RuntimeHeartbeatRecord {
            component: WATCH_COMPONENT.to_owned(),
            mode: mode.to_owned(),
            bind_address: None,
            public_base_url: config.webhook.public_base_url.clone(),
            detail,
            last_seen_at: now_rfc3339()?,
        })
}

fn family_is_due(
    config: &Config,
    sync_states: &[SyncStateRecord],
    family: SyncFamily,
    now: OffsetDateTime,
) -> Result<bool> {
    let next_due = next_due_at(config, sync_states, family, now)?;
    Ok(next_due <= now)
}

fn next_due_at(
    config: &Config,
    sync_states: &[SyncStateRecord],
    family: SyncFamily,
    now: OffsetDateTime,
) -> Result<OffsetDateTime> {
    let Some(state) = sync_states
        .iter()
        .find(|state| state.sync_key == family.sync_key())
    else {
        return Ok(now);
    };

    if let Some(next_attempt_after) = state.next_attempt_after.as_deref() {
        let next_attempt_after = parse_timestamp(next_attempt_after)?;
        if next_attempt_after > now {
            return Ok(next_attempt_after);
        }
    }

    let reference = state
        .last_completed_at
        .as_deref()
        .or(Some(state.last_attempted_at.as_str()))
        .ok_or_else(|| {
            RingmasterError::Config(format!(
                "sync state for {} is missing timing information",
                family.sync_key()
            ))
        })?;
    let reference = parse_timestamp(reference)?;

    Ok(reference + Duration::seconds(family.interval_secs(&config.refresh) as i64))
}

fn parse_timestamp(value: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|error| {
        RingmasterError::Config(format!("invalid persisted timestamp `{value}`: {error}"))
    })
}

fn advance_dry_run_sync_states(
    config: &Config,
    sync_states: &mut Vec<SyncStateRecord>,
    report: &SyncReport,
    trigger_source: &str,
    trigger_detail: &str,
) {
    let granted_scopes = report
        .capability_report
        .entries
        .iter()
        .filter(|entry| entry.granted)
        .map(|entry| entry.kind.scope_name().to_owned())
        .collect::<Vec<_>>();

    for slice in &report.slice_reports {
        let next_state = build_simulated_sync_state(
            config,
            sync_states,
            slice,
            &report.finished_at,
            &granted_scopes,
            trigger_source,
            trigger_detail,
        );
        upsert_simulated_sync_state(sync_states, next_state);
    }
}

fn build_simulated_sync_state(
    config: &Config,
    sync_states: &[SyncStateRecord],
    slice: &SliceReport,
    completed_at: &str,
    granted_scopes: &[String],
    trigger_source: &str,
    trigger_detail: &str,
) -> SyncStateRecord {
    let previous = sync_states
        .iter()
        .find(|state| state.sync_key == slice.sync_key);
    let failure_count = match slice.status {
        crate::store::queries::SyncRunStatus::Failed => previous
            .map(|state| state.failure_count.saturating_add(1))
            .unwrap_or(1),
        _ => 0,
    };
    let next_attempt_after = if slice.status == crate::store::queries::SyncRunStatus::Failed {
        slice.next_attempt_after.clone().or_else(|| {
            simulated_next_attempt_after(
                config,
                sync_family_from_key(&slice.sync_key),
                failure_count,
            )
        })
    } else {
        None
    };

    SyncStateRecord {
        sync_key: slice.sync_key.clone(),
        status: slice.status.clone(),
        cursor: slice.watermark.clone(),
        last_attempted_at: completed_at.to_owned(),
        last_completed_at: Some(completed_at.to_owned()),
        message: Some(slice.message.clone()),
        granted_scopes: granted_scopes.to_vec(),
        last_error: slice.last_error.clone(),
        failure_count,
        next_attempt_after,
        last_trigger_source: Some(trigger_source.to_owned()),
        last_trigger_detail: Some(trigger_detail.to_owned()),
    }
}

fn sync_family_from_key(sync_key: &str) -> Option<SyncFamily> {
    SyncFamily::ALL
        .into_iter()
        .find(|family| family.sync_key() == sync_key)
}

fn simulated_next_attempt_after(
    config: &Config,
    family: Option<SyncFamily>,
    failure_count: u32,
) -> Option<String> {
    let family = family?;
    let base_interval_secs = family.interval_secs(&config.refresh);
    let capped_shift = failure_count.saturating_sub(1).min(6);
    let multiplier = 1_u64 << capped_shift;
    let backoff_secs = base_interval_secs
        .saturating_mul(multiplier)
        .min(config.refresh.max_backoff_secs);

    (OffsetDateTime::now_utc() + Duration::seconds(backoff_secs as i64))
        .format(&Rfc3339)
        .ok()
}

fn upsert_simulated_sync_state(
    sync_states: &mut Vec<SyncStateRecord>,
    next_state: SyncStateRecord,
) {
    if let Some(existing) = sync_states
        .iter_mut()
        .find(|state| state.sync_key == next_state.sync_key)
    {
        *existing = next_state;
    } else {
        sync_states.push(next_state);
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::{SyncFamily, WatchOptions, due_families, next_wake_duration};
    use crate::config::{
        AppPaths, Config, LoggingConfig, OuraConfig, RefreshConfig, WebhookConfig,
    };
    use crate::oura::models::CapabilityReport;
    use crate::oura::sync::{SliceReport, SyncReport};
    use crate::refresh::run_watch;
    use crate::store::Store;
    use crate::store::queries::{SyncRunStatus, SyncStateRecord};
    use crate::webhook::default_desired_subscriptions;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use time::format_description::well_known::Rfc3339;
    use time::{Duration, OffsetDateTime};

    fn test_config() -> Config {
        let unique_root = unique_test_root("refresh");
        Config {
            app_name: "ringmaster",
            paths: AppPaths::from_roots(
                PathBuf::from("/home/tester"),
                unique_root.join("config"),
                unique_root.join("state"),
                unique_root.join("cache"),
            )
            .unwrap_or_else(|error| panic!("paths should resolve: {error}")),
            logging: LoggingConfig {
                filter: "ringmaster=debug".to_owned(),
            },
            oura: OuraConfig {
                client_id: None,
                client_secret: None,
                authorize_url: "https://cloud.oura.com/oauth/authorize".to_owned(),
                token_url: "https://api.oura.com/oauth/token".to_owned(),
                api_base_url: "https://api.oura.com".to_owned(),
                callback_bind: "127.0.0.1:8788"
                    .parse()
                    .unwrap_or_else(|error| panic!("socket should parse: {error}")),
                callback_path: "/callback".to_owned(),
                requested_scopes: vec![
                    "personal".to_owned(),
                    "daily".to_owned(),
                    "heartrate".to_owned(),
                    "workout".to_owned(),
                    "enhanced_tag".to_owned(),
                    "session".to_owned(),
                ],
                auth_timeout_secs: 120,
            },
            refresh: RefreshConfig {
                personal_interval_secs: 3_600,
                daily_interval_secs: 300,
                heartrate_interval_secs: 60,
                workout_interval_secs: 600,
                enhanced_tag_interval_secs: 300,
                session_interval_secs: 300,
                personal_stale_after_secs: 72 * 60 * 60,
                daily_stale_after_secs: 12 * 60 * 60,
                heartrate_stale_after_secs: 15 * 60,
                workout_stale_after_secs: 24 * 60 * 60,
                enhanced_tag_stale_after_secs: 12 * 60 * 60,
                session_stale_after_secs: 12 * 60 * 60,
                daily_history_days: 90,
                daily_overlap_days: 2,
                heartrate_history_days: 7,
                heartrate_overlap_minutes: 60,
                workout_history_days: 90,
                workout_overlap_days: 2,
                enhanced_tag_history_days: 90,
                enhanced_tag_overlap_days: 2,
                session_history_days: 90,
                session_overlap_days: 2,
                max_backoff_secs: 60 * 60,
                demo_fixture_dir: None,
            },
            webhook: WebhookConfig {
                bind: "127.0.0.1:8799".parse().unwrap(),
                path: "/webhooks/oura".to_owned(),
                public_base_url: Some("https://example.test".to_owned()),
                verification_token: Some("verify-me".to_owned()),
                signature_tolerance_secs: 300,
                heartbeat_secs: 15,
                renewal_lead_secs: 7 * 24 * 60 * 60,
                subscriptions: default_desired_subscriptions(),
            },
        }
    }

    fn unique_test_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        PathBuf::from("/tmp").join(format!("ringmaster-{label}-{}-{nanos}", std::process::id()))
    }

    fn phase3_fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/phase3")
    }

    fn seed_delivery(
        store: &Store,
        data_type: &str,
        event_type: crate::webhook::WebhookEventType,
        object_id: &str,
    ) -> i64 {
        let received_at = crate::store::webhook_store::now_rfc3339()
            .unwrap_or_else(|error| panic!("timestamp should render: {error}"));
        let delivery = store
            .webhook()
            .insert_accepted_delivery(&crate::store::webhook_store::AcceptedWebhookDeliveryInput {
                delivery_fingerprint: format!("{data_type}:{}:{object_id}", event_type.as_str()),
                received_at: received_at.clone(),
                signature_timestamp: Some(received_at),
                data_type: Some(data_type.to_owned()),
                event_type: Some(event_type),
                object_id: Some(object_id.to_owned()),
                payload_json: "{}".to_owned(),
                headers_json: "{}".to_owned(),
                query_json: "{}".to_owned(),
            })
            .unwrap_or_else(|error| panic!("accepted delivery should persist: {error}"));

        match delivery {
            crate::store::webhook_store::AcceptedWebhookDeliveryResult::Inserted(record)
            | crate::store::webhook_store::AcceptedWebhookDeliveryResult::Duplicate(record) => {
                record.delivery_id
            }
        }
    }

    #[test]
    fn due_families_respects_next_attempt_after() {
        let config = test_config();
        let now = OffsetDateTime::now_utc();
        let families = due_families(
            &config,
            &[SyncStateRecord {
                sync_key: SyncFamily::Heartrate.sync_key().to_owned(),
                status: SyncRunStatus::Failed,
                cursor: None,
                last_attempted_at: now.format(&Rfc3339).unwrap_or_default(),
                last_completed_at: None,
                message: Some("backing off".to_owned()),
                granted_scopes: vec!["heartrate".to_owned()],
                last_error: None,
                failure_count: 2,
                next_attempt_after: Some(
                    (now + Duration::minutes(5))
                        .format(&Rfc3339)
                        .unwrap_or_default(),
                ),
                last_trigger_source: None,
                last_trigger_detail: None,
            }],
            now,
            false,
        )
        .unwrap_or_else(|error| panic!("due families should compute: {error}"));

        assert!(!families.contains(&SyncFamily::Heartrate));
    }

    #[test]
    fn next_wake_duration_is_positive() {
        let config = test_config();
        let duration = next_wake_duration(&config, &[], OffsetDateTime::now_utc())
            .unwrap_or_else(|error| panic!("next wake should compute: {error}"));
        assert!(duration.as_secs() >= 1);
    }

    #[test]
    fn dry_run_reports_advance_local_schedule_state() {
        let config = test_config();
        let base = OffsetDateTime::parse("2026-04-08T06:00:00Z", &Rfc3339)
            .unwrap_or_else(|error| panic!("timestamp should parse: {error}"));
        let mut sync_states = Vec::new();
        let scopes = vec![
            "personal".to_owned(),
            "daily".to_owned(),
            "heartrate".to_owned(),
            "workout".to_owned(),
            "enhanced_tag".to_owned(),
            "session".to_owned(),
        ];
        let report = SyncReport {
            status: SyncRunStatus::Success,
            started_at: base.format(&Rfc3339).unwrap_or_default(),
            finished_at: base.format(&Rfc3339).unwrap_or_default(),
            database_path: ":memory:".to_owned(),
            notes: Vec::new(),
            capability_report: CapabilityReport::from_scopes(&scopes, &scopes),
            slice_reports: vec![
                SliceReport {
                    sync_key: SyncFamily::Personal.sync_key().to_owned(),
                    status: SyncRunStatus::Success,
                    imported_rows: 1,
                    watermark: Some(base.format(&Rfc3339).unwrap_or_default()),
                    message: "personal synced".to_owned(),
                    last_error: None,
                    next_attempt_after: None,
                },
                SliceReport {
                    sync_key: SyncFamily::Daily.sync_key().to_owned(),
                    status: SyncRunStatus::Success,
                    imported_rows: 3,
                    watermark: Some("2026-04-08".to_owned()),
                    message: "daily synced".to_owned(),
                    last_error: None,
                    next_attempt_after: None,
                },
                SliceReport {
                    sync_key: SyncFamily::Heartrate.sync_key().to_owned(),
                    status: SyncRunStatus::Success,
                    imported_rows: 5,
                    watermark: Some(base.format(&Rfc3339).unwrap_or_default()),
                    message: "heartrate synced".to_owned(),
                    last_error: None,
                    next_attempt_after: None,
                },
                SliceReport {
                    sync_key: SyncFamily::Workout.sync_key().to_owned(),
                    status: SyncRunStatus::Success,
                    imported_rows: 2,
                    watermark: Some("2026-04-08".to_owned()),
                    message: "workouts synced".to_owned(),
                    last_error: None,
                    next_attempt_after: None,
                },
                SliceReport {
                    sync_key: SyncFamily::EnhancedTag.sync_key().to_owned(),
                    status: SyncRunStatus::Success,
                    imported_rows: 2,
                    watermark: Some("2026-04-08".to_owned()),
                    message: "enhanced tags synced".to_owned(),
                    last_error: None,
                    next_attempt_after: None,
                },
                SliceReport {
                    sync_key: SyncFamily::Session.sync_key().to_owned(),
                    status: SyncRunStatus::Success,
                    imported_rows: 2,
                    watermark: Some("2026-04-08".to_owned()),
                    message: "sessions synced".to_owned(),
                    last_error: None,
                    next_attempt_after: None,
                },
            ],
        };

        super::advance_dry_run_sync_states(
            &config,
            &mut sync_states,
            &report,
            "periodic_reconcile",
            "simulated watch iteration",
        );

        let due = due_families(&config, &sync_states, base + Duration::seconds(1), false)
            .unwrap_or_else(|error| panic!("due families should compute: {error}"));
        assert!(due.is_empty());

        let next_wake = next_wake_duration(&config, &sync_states, base)
            .unwrap_or_else(|error| panic!("next wake should compute: {error}"));
        assert_eq!(next_wake.as_secs(), config.refresh.heartrate_interval_secs);
    }

    #[tokio::test]
    async fn bounded_demo_watch_runs_one_iteration() {
        let config = test_config();
        let report = run_watch(
            &config,
            WatchOptions {
                dry_run: false,
                demo: true,
                fixture_dir: Some(phase3_fixture_dir()),
                max_iterations: Some(1),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("watch should succeed: {error}"));

        assert_eq!(report.iterations, 1);
        assert!(report.dry_run);
        assert!(report.last_report.is_some());
    }

    #[tokio::test]
    async fn zero_bounded_iterations_skip_sync_entirely() {
        let config = test_config();
        let report = run_watch(
            &config,
            WatchOptions {
                dry_run: false,
                demo: true,
                fixture_dir: Some(phase3_fixture_dir()),
                max_iterations: Some(0),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("zero-iteration watch should succeed: {error}"));

        assert_eq!(report.iterations, 0);
        assert!(report.dry_run);
        assert!(report.last_report.is_none());
        assert_eq!(
            report.notes,
            vec![
                "watch loop stopped before syncing because max_iterations was set to 0".to_owned()
            ]
        );
    }

    #[tokio::test]
    async fn interrupt_preempts_inflight_sync_future() {
        let interrupted = super::await_sync_or_interrupt(
            async {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                Ok::<_, crate::error::RingmasterError>(())
            },
            async {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                Ok::<_, std::io::Error>(())
            },
        )
        .await
        .unwrap_or_else(|error| panic!("interrupt should be handled: {error}"));

        assert_eq!(interrupted, super::SyncInterruption::Interrupted);
    }

    #[tokio::test]
    async fn invalidation_processing_updates_trigger_provenance_and_clears_queue() {
        let config = test_config();
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let queued_at = crate::store::webhook_store::now_rfc3339()
            .unwrap_or_else(|error| panic!("timestamp should render: {error}"));
        let delivery_id = seed_delivery(
            &store,
            "daily_sleep",
            crate::webhook::WebhookEventType::Create,
            "sleep_2026-04-08",
        );
        store
            .webhook()
            .enqueue_invalidation(&crate::store::webhook_store::InvalidationInput {
                queue_key: "daily_sleep:create:sleep_2026-04-08".to_owned(),
                data_type: "daily_sleep".to_owned(),
                event_type: crate::webhook::WebhookEventType::Create,
                object_id: Some("sleep_2026-04-08".to_owned()),
                delivery_id,
                queued_at: queued_at.clone(),
                available_at: queued_at,
            })
            .unwrap_or_else(|error| panic!("invalidation should queue: {error}"));

        let report = super::process_pending_invalidations_once(
            &config,
            &store,
            false,
            Some(phase3_fixture_dir()),
        )
        .await
        .unwrap_or_else(|error| panic!("invalidation processing should succeed: {error}"));

        assert_eq!(report.claimed_invalidations, 1);
        assert_eq!(report.families, vec![SyncFamily::Daily]);
        assert!(report.sync_report.is_some());
        assert!(
            store
                .webhook()
                .list_pending_invalidations()
                .unwrap_or_else(|error| panic!("queue should read: {error}"))
                .is_empty()
        );
        let attempts = store
            .webhook()
            .list_recent_processing_attempts(8)
            .unwrap_or_else(|error| panic!("attempts should read: {error}"));
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].outcome, "success");
        let daily_state = store
            .sync_state()
            .get(SyncFamily::Daily.sync_key())
            .unwrap_or_else(|error| panic!("daily sync state should read: {error}"))
            .unwrap_or_else(|| panic!("daily sync state should exist"));
        assert_eq!(daily_state.last_trigger_source.as_deref(), Some("webhook"));
        let trigger_detail = daily_state.last_trigger_detail.unwrap_or_default();
        assert!(trigger_detail.contains("daily_sleep:create:sleep_2026-04-08"));
    }

    #[tokio::test]
    async fn invalidation_delete_side_effect_removes_deleted_workout() {
        let config = test_config();
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let seed_report = crate::oura::sync::sync_selected(
            &config,
            &store,
            crate::oura::sync::SyncOptions {
                dry_run: false,
                fixture_dir: Some(phase3_fixture_dir()),
                families: vec![SyncFamily::Workout],
                trigger_source: Some("periodic_reconcile".to_owned()),
                trigger_detail: Some("seed workouts".to_owned()),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("seed sync should succeed: {error}"));
        assert_eq!(seed_report.status, SyncRunStatus::Success);
        assert_eq!(
            store
                .views()
                .record_counts()
                .unwrap_or_else(|error| panic!("counts should read: {error}"))
                .workouts,
            3
        );

        let queued_at = crate::store::webhook_store::now_rfc3339()
            .unwrap_or_else(|error| panic!("timestamp should render: {error}"));
        let delivery_id = seed_delivery(
            &store,
            "workout",
            crate::webhook::WebhookEventType::Delete,
            "workout_2026-04-05_strength",
        );
        store
            .webhook()
            .enqueue_invalidation(&crate::store::webhook_store::InvalidationInput {
                queue_key: "workout:delete:workout_2026-04-05_strength".to_owned(),
                data_type: "workout".to_owned(),
                event_type: crate::webhook::WebhookEventType::Delete,
                object_id: Some("workout_2026-04-05_strength".to_owned()),
                delivery_id,
                queued_at: queued_at.clone(),
                available_at: queued_at,
            })
            .unwrap_or_else(|error| panic!("delete invalidation should queue: {error}"));

        let report = super::process_pending_invalidations_once(
            &config,
            &store,
            false,
            Some(phase3_fixture_dir()),
        )
        .await
        .unwrap_or_else(|error| panic!("delete invalidation processing should succeed: {error}"));

        assert_eq!(report.claimed_invalidations, 1);
        assert_eq!(report.families, vec![SyncFamily::Workout]);
        assert_eq!(
            store
                .views()
                .record_counts()
                .unwrap_or_else(|error| panic!("counts should read: {error}"))
                .workouts,
            2
        );
        assert!(
            store
                .webhook()
                .list_pending_invalidations()
                .unwrap_or_else(|error| panic!("queue should read: {error}"))
                .is_empty()
        );
    }
}
