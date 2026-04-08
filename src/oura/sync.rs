use std::path::PathBuf;

use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::config::Config;
use crate::derive;
use crate::error::{AuthError, OuraApiError, OuraProblem, Result, RingmasterError};
use crate::oura::auth;
use crate::oura::client::{FixtureOuraClient, OuraClient, ReqwestOuraClient};
use crate::oura::models::{CapabilityKind, CapabilityReport, WorkoutDocument};
use crate::refresh::SyncFamily;
use crate::store::Store;
use crate::store::queries::{
    AuthSessionRecord, DailyActivityRecord, DailyReadinessRecord, DailySleepRecord,
    EnhancedTagRecord, HeartrateSampleRecord, OURA_PROVIDER, PersonalInfoRecord, SessionRecord,
    SyncRunStatus, SyncStateRecord, WorkoutRecord,
};

const PERSONAL_SYNC_KEY: &str = "oura.personal";
const DAILY_SYNC_KEY: &str = "oura.daily";
const HEARTRATE_SYNC_KEY: &str = "oura.heartrate";
const WORKOUT_SYNC_KEY: &str = "oura.workouts";
const ENHANCED_TAG_SYNC_KEY: &str = "oura.enhanced_tags";
const SESSION_SYNC_KEY: &str = "oura.sessions";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncOptions {
    pub dry_run: bool,
    pub fixture_dir: Option<PathBuf>,
    pub families: Vec<SyncFamily>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceReport {
    pub sync_key: String,
    pub status: SyncRunStatus,
    pub imported_rows: usize,
    pub watermark: Option<String>,
    pub message: String,
    pub last_error: Option<OuraProblem>,
    pub next_attempt_after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncReport {
    pub status: SyncRunStatus,
    pub started_at: String,
    pub finished_at: String,
    pub database_path: String,
    pub notes: Vec<String>,
    pub capability_report: CapabilityReport,
    pub slice_reports: Vec<SliceReport>,
}

pub async fn sync_once(config: &Config, store: &Store, options: SyncOptions) -> Result<SyncReport> {
    sync_selected(
        config,
        store,
        SyncOptions {
            dry_run: options.dry_run,
            fixture_dir: options.fixture_dir,
            families: if options.families.is_empty() {
                SyncFamily::ALL.to_vec()
            } else {
                options.families
            },
        },
    )
    .await
}

pub async fn sync_selected(
    config: &Config,
    store: &Store,
    options: SyncOptions,
) -> Result<SyncReport> {
    let started_at = now_rfc3339()?;
    let mut notes = Vec::new();
    let families = if options.families.is_empty() {
        SyncFamily::ALL.to_vec()
    } else {
        options.families.clone()
    };

    let (client, capability_report) = if let Some(fixture_dir) = &options.fixture_dir {
        notes.push(format!(
            "Running fixture-backed sync from {}.",
            fixture_dir.display()
        ));
        let client = FixtureOuraClient::new(config, fixture_dir.clone())?;
        let capability_report = client.capability_report();
        (Box::new(client) as Box<dyn OuraClient>, capability_report)
    } else {
        let auth_status = auth::inspect_auth(config, store)?;
        if !auth_status.configured {
            let slice_reports = persist_blocked_slice_reports(
                config,
                store,
                &families,
                &auth_status.granted_scopes,
                "Oura client credentials are not configured; live sync is blocked.",
                &options,
            )?;
            let finished_at = now_rfc3339()?;
            return Ok(SyncReport {
                status: SyncRunStatus::Blocked,
                started_at,
                finished_at,
                database_path: store.plan().db_path.display().to_string(),
                notes: vec![
                    "Add `RINGMASTER_OURA_CLIENT_ID` and `RINGMASTER_OURA_CLIENT_SECRET` to run live auth."
                        .to_owned(),
                ],
                capability_report: auth_status.capability_report,
                slice_reports,
            });
        }

        if !auth_status.access_token_stored && !auth_status.refresh_token_stored {
            let slice_reports = persist_blocked_slice_reports(
                config,
                store,
                &families,
                &auth_status.granted_scopes,
                "No persisted auth session is available yet; run `ringmaster auth login` first.",
                &options,
            )?;
            let finished_at = now_rfc3339()?;
            return Ok(SyncReport {
                status: SyncRunStatus::Blocked,
                started_at,
                finished_at,
                database_path: store.plan().db_path.display().to_string(),
                notes: vec![
                    "Live sync only reads persisted auth state; the TUI will never refresh tokens on the render path."
                        .to_owned(),
                ],
                capability_report: auth_status.capability_report,
                slice_reports,
            });
        }

        let http_client = reqwest::Client::builder()
            .user_agent("ringmaster.rs/phase3")
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let session = match auth::ensure_authorized_session(config, store, &http_client).await {
            Ok(session) => session,
            Err(error) => {
                let slice_reports = persist_failed_slice_reports(
                    config,
                    store,
                    &families,
                    auth_status.granted_scopes.clone(),
                    &options,
                    error_problem(&error),
                )?;
                let finished_at = now_rfc3339()?;
                return Ok(SyncReport {
                    status: SyncRunStatus::Failed,
                    started_at,
                    finished_at,
                    database_path: store.plan().db_path.display().to_string(),
                    notes: vec![
                        "The scheduler keeps auth handling out of the render path and persists the failure state for each requested family.".to_owned(),
                    ],
                    capability_report: auth_status.capability_report,
                    slice_reports,
                });
            }
        };
        let client = ReqwestOuraClient::new(config, session.access_token, &session.granted_scopes)?;
        let capability_report = client.capability_report();
        (Box::new(client) as Box<dyn OuraClient>, capability_report)
    };

    let mut slice_reports = Vec::new();
    for family in &families {
        let report = match family {
            SyncFamily::Personal => {
                sync_personal_info(config, store, client.as_ref(), &capability_report, &options)
                    .await
            }
            SyncFamily::Daily => {
                sync_daily(config, store, client.as_ref(), &capability_report, &options).await
            }
            SyncFamily::Heartrate => {
                sync_heartrate(config, store, client.as_ref(), &capability_report, &options).await
            }
            SyncFamily::Workout => {
                sync_workouts(config, store, client.as_ref(), &capability_report, &options).await
            }
            SyncFamily::EnhancedTag => {
                sync_enhanced_tags(config, store, client.as_ref(), &capability_report, &options)
                    .await
            }
            SyncFamily::Session => {
                sync_sessions(config, store, client.as_ref(), &capability_report, &options).await
            }
        };

        match report {
            Ok(report) => slice_reports.push(report),
            Err(error) => slice_reports.push(persist_slice_report(
                config,
                store,
                failed_slice_report(family.sync_key(), error_problem(&error)),
                granted_scopes_from_report(&capability_report),
                &options,
            )?),
        }
    }

    if !options.dry_run && should_rebuild_derived_state(&slice_reports) {
        let derive_report = derive::rebuild_recent_store(store, config)?;
        notes.push(format!(
            "Derived context events and pattern summaries were rebuilt after sync (events={}, patterns={}).",
            derive_report.context_event_count, derive_report.pattern_summary_count
        ));
        notes.extend(derive_report.notes);
    }

    let status = summarize_status(&slice_reports);
    if options.dry_run {
        notes.push("Dry-run mode fetched and normalized data without mutating SQLite.".to_owned());
    } else {
        notes.push("Raw payloads and normalized rows were persisted to SQLite.".to_owned());
    }

    let finished_at = now_rfc3339()?;
    Ok(SyncReport {
        status,
        started_at,
        finished_at,
        database_path: store.plan().db_path.display().to_string(),
        notes,
        capability_report,
        slice_reports,
    })
}

fn should_rebuild_derived_state(slice_reports: &[SliceReport]) -> bool {
    slice_reports.iter().any(|report| {
        report.status == SyncRunStatus::Success
            && matches!(
                report.sync_key.as_str(),
                DAILY_SYNC_KEY | WORKOUT_SYNC_KEY | ENHANCED_TAG_SYNC_KEY | SESSION_SYNC_KEY
            )
    })
}

async fn sync_personal_info(
    config: &Config,
    store: &Store,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !capability_report.is_granted(CapabilityKind::Personal) {
        return persist_slice_report(
            config,
            store,
            slice_blocked(
                PERSONAL_SYNC_KEY,
                "Missing `personal` scope; profile data remains unavailable.",
            ),
            granted_scopes_from_report(capability_report),
            options,
        );
    }

    let fetched = client.fetch_personal_info().await?;
    let imported_at = now_rfc3339()?;

    if !options.dry_run {
        store.imports().upsert_raw_payload(&fetched.raw_payload)?;
        store.imports().upsert_personal_info(&PersonalInfoRecord {
            profile_id: fetched.document.id.clone(),
            age: fetched.document.age,
            weight: fetched.document.weight,
            height: fetched.document.height,
            biological_sex: fetched.document.biological_sex.clone(),
            email: fetched.document.email.clone(),
            raw_cache_key: Some(fetched.raw_payload.cache_key.clone()),
            updated_at: imported_at.clone(),
        })?;

        if let Some(mut auth_session) = store.auth().get(OURA_PROVIDER)? {
            auth_session.account_id = Some(fetched.document.id.clone());
            auth_session
                .account_email
                .clone_from(&fetched.document.email);
            auth_session.updated_at.clone_from(&imported_at);
            store.auth().upsert(&auth_session)?;
        } else {
            store.auth().upsert(&AuthSessionRecord {
                provider: OURA_PROVIDER.to_owned(),
                account_id: Some(fetched.document.id.clone()),
                account_email: fetched.document.email.clone(),
                token_type: "Bearer".to_owned(),
                granted_scopes: granted_scopes_from_report(capability_report),
                access_token_expires_at: None,
                last_authenticated_at: None,
                last_refresh_at: None,
                last_error: None,
                updated_at: imported_at.clone(),
            })?;
        }
    }

    persist_slice_report(
        config,
        store,
        SliceReport {
            sync_key: PERSONAL_SYNC_KEY.to_owned(),
            status: SyncRunStatus::Success,
            imported_rows: 1,
            watermark: Some(imported_at),
            message: format!(
                "Imported personal info for profile {}.",
                fetched.document.id
            ),
            last_error: None,
            next_attempt_after: None,
        },
        granted_scopes_from_report(capability_report),
        options,
    )
}

async fn sync_daily(
    config: &Config,
    store: &Store,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !capability_report.is_granted(CapabilityKind::Daily) {
        return persist_slice_report(
            config,
            store,
            slice_blocked(
                DAILY_SYNC_KEY,
                "Missing `daily` scope; dashboard summary rows remain unavailable.",
            ),
            granted_scopes_from_report(capability_report),
            options,
        );
    }

    let end_date = utc_date_string(OffsetDateTime::now_utc());
    let start_date = if options.fixture_dir.is_some() {
        "1970-01-01".to_owned()
    } else {
        overlap_day_window(
            store,
            DAILY_SYNC_KEY,
            i64::from(config.refresh.daily_history_days),
            i64::from(config.refresh.daily_overlap_days),
        )?
    };
    let (sleep_pages, readiness_pages, activity_pages) = tokio::try_join!(
        client.fetch_daily_sleep(start_date.clone(), end_date.clone()),
        client.fetch_daily_readiness(start_date.clone(), end_date.clone()),
        client.fetch_daily_activity(start_date.clone(), end_date.clone()),
    )?;
    let imported_at = now_rfc3339()?;

    if !options.dry_run {
        for page in &sleep_pages {
            store.imports().upsert_raw_payload(&page.raw_payload)?;
            for document in &page.documents {
                store.imports().upsert_daily_sleep(&DailySleepRecord {
                    day: document.day.clone(),
                    sleep_score: document.score,
                    raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                    updated_at: imported_at.clone(),
                })?;
            }
        }
        for page in &readiness_pages {
            store.imports().upsert_raw_payload(&page.raw_payload)?;
            for document in &page.documents {
                store
                    .imports()
                    .upsert_daily_readiness(&DailyReadinessRecord {
                        day: document.day.clone(),
                        readiness_score: document.score,
                        temperature_deviation: document.temperature_deviation,
                        temperature_trend_deviation: document.temperature_trend_deviation,
                        raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                        updated_at: imported_at.clone(),
                    })?;
            }
        }
        for page in &activity_pages {
            store.imports().upsert_raw_payload(&page.raw_payload)?;
            for document in &page.documents {
                store
                    .imports()
                    .upsert_daily_activity(&DailyActivityRecord {
                        day: document.day.clone(),
                        activity_score: document.score,
                        active_calories: document.active_calories,
                        steps: document.steps,
                        total_calories: document.total_calories,
                        raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                        updated_at: imported_at.clone(),
                    })?;
            }
        }
    }

    let imported_rows = count_documents(&sleep_pages)
        + count_documents(&readiness_pages)
        + count_documents(&activity_pages);
    persist_slice_report(
        config,
        store,
        SliceReport {
            sync_key: DAILY_SYNC_KEY.to_owned(),
            status: SyncRunStatus::Success,
            imported_rows,
            watermark: Some(end_date.clone()),
            message: format!(
                "Imported {imported_rows} daily summary rows from {start_date} through {end_date}."
            ),
            last_error: None,
            next_attempt_after: None,
        },
        granted_scopes_from_report(capability_report),
        options,
    )
}

async fn sync_heartrate(
    config: &Config,
    store: &Store,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !capability_report.is_granted(CapabilityKind::Heartrate) {
        return persist_slice_report(
            config,
            store,
            slice_blocked(
                HEARTRATE_SYNC_KEY,
                "Missing `heartrate` scope; timeline and trends remain stale.",
            ),
            granted_scopes_from_report(capability_report),
            options,
        );
    }

    let end_datetime = now_rfc3339()?;
    let start_datetime = if options.fixture_dir.is_some() {
        "1970-01-01T00:00:00Z".to_owned()
    } else {
        overlap_heartrate_window(
            store,
            i64::from(config.refresh.heartrate_history_days),
            i64::from(config.refresh.heartrate_overlap_minutes),
        )?
    };
    let heartrate_pages = client
        .fetch_heartrate(start_datetime.clone(), end_datetime.clone())
        .await?;
    let imported_at = now_rfc3339()?;

    if !options.dry_run {
        for page in &heartrate_pages {
            store.imports().upsert_raw_payload(&page.raw_payload)?;
            for document in &page.documents {
                store
                    .imports()
                    .upsert_heartrate_sample(&HeartrateSampleRecord {
                        recorded_at: document.timestamp.clone(),
                        bpm: document.bpm,
                        source_day: Some(document.timestamp.chars().take(10).collect()),
                        raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                        updated_at: imported_at.clone(),
                    })?;
            }
        }
    }

    let imported_rows = count_documents(&heartrate_pages);
    persist_slice_report(
        config,
        store,
        SliceReport {
            sync_key: HEARTRATE_SYNC_KEY.to_owned(),
            status: SyncRunStatus::Success,
            imported_rows,
            watermark: Some(end_datetime.clone()),
            message: format!(
                "Imported {imported_rows} heartrate samples from {start_datetime} through {end_datetime}."
            ),
            last_error: None,
            next_attempt_after: None,
        },
        granted_scopes_from_report(capability_report),
        options,
    )
}

async fn sync_workouts(
    config: &Config,
    store: &Store,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !capability_report.is_granted(CapabilityKind::Workout) {
        return persist_slice_report(
            config,
            store,
            slice_blocked(
                WORKOUT_SYNC_KEY,
                "Missing `workout` scope; workout overlays and context evidence remain unavailable.",
            ),
            granted_scopes_from_report(capability_report),
            options,
        );
    }

    let end_date = utc_date_string(OffsetDateTime::now_utc());
    let start_date = if options.fixture_dir.is_some() {
        "1970-01-01".to_owned()
    } else {
        overlap_day_window(
            store,
            WORKOUT_SYNC_KEY,
            i64::from(config.refresh.workout_history_days),
            i64::from(config.refresh.workout_overlap_days),
        )?
    };
    let workout_pages = client
        .fetch_workouts(start_date.clone(), end_date.clone())
        .await?;
    let imported_at = now_rfc3339()?;

    if !options.dry_run {
        for page in &workout_pages {
            store.imports().upsert_raw_payload(&page.raw_payload)?;
            for document in &page.documents {
                store.imports().upsert_workout(&WorkoutRecord {
                    workout_id: document.id.clone(),
                    day: document.anchor_day(),
                    started_at: workout_start_at(document),
                    ended_at: document.end_datetime.clone(),
                    timezone: document.timezone.clone(),
                    sport: document.sport.clone(),
                    activity: document.activity.clone(),
                    intensity: document.intensity.clone(),
                    title: document.title(),
                    notes: json_string_field(&document.extra, "note")
                        .or_else(|| json_string_field(&document.extra, "notes")),
                    source: document.source.clone(),
                    raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                    updated_at: imported_at.clone(),
                })?;
            }
        }
    }

    let imported_rows = count_documents(&workout_pages);
    persist_slice_report(
        config,
        store,
        SliceReport {
            sync_key: WORKOUT_SYNC_KEY.to_owned(),
            status: SyncRunStatus::Success,
            imported_rows,
            watermark: Some(end_date.clone()),
            message: format!(
                "Imported {imported_rows} workouts from {start_date} through {end_date}."
            ),
            last_error: None,
            next_attempt_after: None,
        },
        granted_scopes_from_report(capability_report),
        options,
    )
}

async fn sync_enhanced_tags(
    config: &Config,
    store: &Store,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !capability_report.is_granted(CapabilityKind::EnhancedTag) {
        return persist_slice_report(
            config,
            store,
            slice_blocked(
                ENHANCED_TAG_SYNC_KEY,
                "Missing `enhanced_tag` scope; tag overlays and explainability evidence remain unavailable.",
            ),
            granted_scopes_from_report(capability_report),
            options,
        );
    }

    let end_date = utc_date_string(OffsetDateTime::now_utc());
    let start_date = if options.fixture_dir.is_some() {
        "1970-01-01".to_owned()
    } else {
        overlap_day_window(
            store,
            ENHANCED_TAG_SYNC_KEY,
            i64::from(config.refresh.enhanced_tag_history_days),
            i64::from(config.refresh.enhanced_tag_overlap_days),
        )?
    };
    let enhanced_tag_pages = client
        .fetch_enhanced_tags(start_date.clone(), end_date.clone())
        .await?;
    let imported_at = now_rfc3339()?;

    if !options.dry_run {
        for page in &enhanced_tag_pages {
            store.imports().upsert_raw_payload(&page.raw_payload)?;
            for document in &page.documents {
                store.imports().upsert_enhanced_tag(&EnhancedTagRecord {
                    enhanced_tag_id: document.id.clone(),
                    day: document.day.clone(),
                    label: document.title(),
                    started_at: document.start_time.clone(),
                    ended_at: document.end_time.clone(),
                    subtype: document.subtype(),
                    comment: document.comment.clone(),
                    intensity: document.intensity.clone(),
                    raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                    updated_at: imported_at.clone(),
                })?;
            }
        }
    }

    let imported_rows = count_documents(&enhanced_tag_pages);
    persist_slice_report(
        config,
        store,
        SliceReport {
            sync_key: ENHANCED_TAG_SYNC_KEY.to_owned(),
            status: SyncRunStatus::Success,
            imported_rows,
            watermark: Some(end_date.clone()),
            message: format!(
                "Imported {imported_rows} enhanced tags from {start_date} through {end_date}."
            ),
            last_error: None,
            next_attempt_after: None,
        },
        granted_scopes_from_report(capability_report),
        options,
    )
}

async fn sync_sessions(
    config: &Config,
    store: &Store,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !capability_report.is_granted(CapabilityKind::Session) {
        return persist_slice_report(
            config,
            store,
            slice_blocked(
                SESSION_SYNC_KEY,
                "Missing `session` scope; session overlays and explainability evidence remain unavailable.",
            ),
            granted_scopes_from_report(capability_report),
            options,
        );
    }

    let end_date = utc_date_string(OffsetDateTime::now_utc());
    let start_date = if options.fixture_dir.is_some() {
        "1970-01-01".to_owned()
    } else {
        overlap_day_window(
            store,
            SESSION_SYNC_KEY,
            i64::from(config.refresh.session_history_days),
            i64::from(config.refresh.session_overlap_days),
        )?
    };
    let session_pages = client
        .fetch_sessions(start_date.clone(), end_date.clone())
        .await?;
    let imported_at = now_rfc3339()?;

    if !options.dry_run {
        for page in &session_pages {
            store.imports().upsert_raw_payload(&page.raw_payload)?;
            for document in &page.documents {
                store.imports().upsert_session(&SessionRecord {
                    session_id: document.id.clone(),
                    day: document.day.clone(),
                    started_at: document.start_at(),
                    ended_at: document.end_datetime.clone(),
                    kind: document.kind.clone(),
                    state: document.state.clone(),
                    score: document.score,
                    title: document.title(),
                    raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                    updated_at: imported_at.clone(),
                })?;
            }
        }
    }

    let imported_rows = count_documents(&session_pages);
    persist_slice_report(
        config,
        store,
        SliceReport {
            sync_key: SESSION_SYNC_KEY.to_owned(),
            status: SyncRunStatus::Success,
            imported_rows,
            watermark: Some(end_date.clone()),
            message: format!(
                "Imported {imported_rows} sessions from {start_date} through {end_date}."
            ),
            last_error: None,
            next_attempt_after: None,
        },
        granted_scopes_from_report(capability_report),
        options,
    )
}

fn persist_blocked_slice_reports(
    config: &Config,
    store: &Store,
    families: &[SyncFamily],
    granted_scopes: &[String],
    message: &str,
    options: &SyncOptions,
) -> Result<Vec<SliceReport>> {
    families
        .iter()
        .copied()
        .map(|family| {
            persist_slice_report(
                config,
                store,
                slice_blocked(family.sync_key(), message),
                granted_scopes.to_vec(),
                options,
            )
        })
        .collect()
}

fn persist_failed_slice_reports(
    config: &Config,
    store: &Store,
    families: &[SyncFamily],
    granted_scopes: Vec<String>,
    options: &SyncOptions,
    problem: OuraProblem,
) -> Result<Vec<SliceReport>> {
    families
        .iter()
        .copied()
        .map(|family| {
            persist_slice_report(
                config,
                store,
                failed_slice_report(family.sync_key(), problem.clone()),
                granted_scopes.clone(),
                options,
            )
        })
        .collect()
}

fn persist_slice_report(
    config: &Config,
    store: &Store,
    report: SliceReport,
    granted_scopes: Vec<String>,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !options.dry_run {
        let previous = store.sync_state().get(&report.sync_key)?;
        let failure_count = match report.status {
            SyncRunStatus::Failed => previous
                .as_ref()
                .map(|state| state.failure_count.saturating_add(1))
                .unwrap_or(1),
            _ => 0,
        };
        let attempted_at = now_rfc3339()?;
        let completed_at = now_rfc3339()?;
        let next_attempt_after = if report.status == SyncRunStatus::Failed {
            report
                .next_attempt_after
                .clone()
                .or_else(|| compute_next_attempt_after(config, &report.sync_key, failure_count))
        } else {
            None
        };
        store.sync_state().upsert(&SyncStateRecord {
            sync_key: report.sync_key.clone(),
            status: report.status.clone(),
            cursor: report.watermark.clone(),
            last_attempted_at: attempted_at,
            last_completed_at: Some(completed_at),
            message: Some(report.message.clone()),
            granted_scopes,
            last_error: report.last_error.clone(),
            failure_count,
            next_attempt_after,
        })?;
    }

    Ok(report)
}

fn slice_blocked(sync_key: &str, message: &str) -> SliceReport {
    SliceReport {
        sync_key: sync_key.to_owned(),
        status: SyncRunStatus::Blocked,
        imported_rows: 0,
        watermark: None,
        message: message.to_owned(),
        last_error: None,
        next_attempt_after: None,
    }
}

fn failed_slice_report(sync_key: &str, problem: OuraProblem) -> SliceReport {
    let message = format!("{}: {}", sync_key, problem);
    SliceReport {
        sync_key: sync_key.to_owned(),
        status: SyncRunStatus::Failed,
        imported_rows: 0,
        watermark: None,
        message,
        last_error: Some(problem),
        next_attempt_after: None,
    }
}

fn overlap_day_window(
    store: &Store,
    sync_key: &str,
    initial_days: i64,
    overlap_days: i64,
) -> Result<String> {
    let fallback = OffsetDateTime::now_utc().date() - Duration::days(initial_days - 1);
    let Some(sync_state) = store
        .sync_state()
        .get(sync_key)?
        .filter(|record| record.status == SyncRunStatus::Success)
    else {
        return Ok(fallback.to_string());
    };
    let Some(cursor) = sync_state.cursor.as_deref() else {
        return Ok(fallback.to_string());
    };
    let date = time::Date::parse(
        cursor,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .map_err(|error| {
        AuthError::OAuthFlow(format!("invalid stored day watermark `{cursor}`: {error}"))
    })?;
    Ok((date - Duration::days(overlap_days)).to_string())
}

fn overlap_heartrate_window(
    store: &Store,
    initial_days: i64,
    overlap_minutes: i64,
) -> Result<String> {
    let fallback = OffsetDateTime::now_utc() - Duration::days(initial_days);
    let Some(sync_state) = store
        .sync_state()
        .get(HEARTRATE_SYNC_KEY)?
        .filter(|record| record.status == SyncRunStatus::Success)
    else {
        return fallback.format(&Rfc3339).map_err(|error| {
            AuthError::OAuthFlow(format!(
                "failed to format heartrate fallback watermark: {error}"
            ))
            .into()
        });
    };
    let Some(cursor) = sync_state.cursor.as_deref() else {
        return fallback.format(&Rfc3339).map_err(|error| {
            AuthError::OAuthFlow(format!(
                "failed to format heartrate fallback watermark: {error}"
            ))
            .into()
        });
    };
    let parsed = OffsetDateTime::parse(cursor, &Rfc3339).map_err(|error| {
        AuthError::OAuthFlow(format!(
            "invalid stored heartrate watermark `{cursor}`: {error}"
        ))
    })?;
    (parsed - Duration::minutes(overlap_minutes))
        .format(&Rfc3339)
        .map_err(|error| {
            AuthError::OAuthFlow(format!("failed to format heartrate watermark: {error}")).into()
        })
}

fn count_documents<T>(pages: &[crate::oura::client::PageFetch<T>]) -> usize {
    pages.iter().map(|page| page.documents.len()).sum()
}

fn summarize_status(slice_reports: &[SliceReport]) -> SyncRunStatus {
    if slice_reports
        .iter()
        .all(|report| report.status == SyncRunStatus::Success)
    {
        SyncRunStatus::Success
    } else if slice_reports
        .iter()
        .all(|report| report.status == SyncRunStatus::Blocked)
    {
        SyncRunStatus::Blocked
    } else if slice_reports
        .iter()
        .any(|report| report.status == SyncRunStatus::Failed)
    {
        if slice_reports
            .iter()
            .any(|report| report.status == SyncRunStatus::Success)
        {
            SyncRunStatus::Partial
        } else {
            SyncRunStatus::Failed
        }
    } else {
        SyncRunStatus::Partial
    }
}

fn compute_next_attempt_after(
    config: &Config,
    sync_key: &str,
    failure_count: u32,
) -> Option<String> {
    let family = family_from_sync_key(sync_key)?;
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

fn family_from_sync_key(sync_key: &str) -> Option<SyncFamily> {
    match sync_key {
        PERSONAL_SYNC_KEY => Some(SyncFamily::Personal),
        DAILY_SYNC_KEY => Some(SyncFamily::Daily),
        HEARTRATE_SYNC_KEY => Some(SyncFamily::Heartrate),
        WORKOUT_SYNC_KEY => Some(SyncFamily::Workout),
        ENHANCED_TAG_SYNC_KEY => Some(SyncFamily::EnhancedTag),
        SESSION_SYNC_KEY => Some(SyncFamily::Session),
        _ => None,
    }
}

fn error_problem(error: &RingmasterError) -> OuraProblem {
    match error {
        RingmasterError::Auth(AuthError::Problem(problem))
        | RingmasterError::OuraApi(OuraApiError::Problem(problem)) => problem.clone(),
        RingmasterError::Auth(auth_error) => OuraProblem::new(
            None,
            "auth failure during sync",
            Some(auth_error.to_string()),
        ),
        RingmasterError::Transport(transport_error) => OuraProblem::new(
            None,
            "transport failure during sync",
            Some(transport_error.to_string()),
        ),
        other => OuraProblem::new(None, "sync failure", Some(other.to_string())),
    }
}

fn granted_scopes_from_report(report: &CapabilityReport) -> Vec<String> {
    report
        .entries
        .iter()
        .filter(|entry| entry.granted)
        .map(|entry| entry.kind.scope_name().to_owned())
        .collect()
}

fn utc_date_string(timestamp: OffsetDateTime) -> String {
    timestamp.date().to_string()
}

fn workout_start_at(document: &WorkoutDocument) -> String {
    document
        .start_datetime
        .clone()
        .unwrap_or_else(|| format!("{}T00:00:00Z", document.anchor_day()))
}

fn json_string_field(
    map: &std::collections::BTreeMap<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    map.get(key)
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc().format(&Rfc3339).map_err(|error| {
        AuthError::OAuthFlow(format!("failed to format sync timestamp: {error}")).into()
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;

    use super::{SyncOptions, sync_once};
    use crate::config::{AppPaths, Config, LoggingConfig, OuraConfig, RefreshConfig};
    use crate::refresh::SyncFamily;
    use crate::store::Store;
    use crate::store::queries::SyncRunStatus;

    fn phase3_fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/phase3")
    }

    fn fixture_config() -> Config {
        Config {
            app_name: "ringmaster",
            paths: AppPaths::from_roots(
                PathBuf::from("/home/tester"),
                PathBuf::from("/tmp/config"),
                PathBuf::from("/tmp/state"),
                PathBuf::from("/tmp/cache"),
            )
            .unwrap(),
            logging: LoggingConfig {
                filter: "ringmaster=debug".to_owned(),
            },
            oura: OuraConfig {
                client_id: None,
                client_secret: None,
                authorize_url: "https://cloud.oura.com/oauth/authorize".to_owned(),
                token_url: "https://api.oura.com/oauth/token".to_owned(),
                api_base_url: "https://api.oura.com".to_owned(),
                callback_bind: "127.0.0.1:8788".parse().unwrap(),
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
        }
    }

    #[tokio::test]
    async fn fixture_sync_populates_phase3_tables_idempotently() {
        let store = Store::open_in_memory().expect("store should open");
        let config = fixture_config();
        let options = SyncOptions {
            dry_run: false,
            fixture_dir: Some(phase3_fixture_dir()),
            families: SyncFamily::ALL.to_vec(),
        };

        let first = sync_once(&config, &store, options.clone())
            .await
            .expect("first fixture sync should succeed");
        let second = sync_once(&config, &store, options)
            .await
            .expect("second fixture sync should stay idempotent");
        let counts = store.views().record_counts().expect("record counts");

        assert_eq!(first.status, SyncRunStatus::Success);
        assert_eq!(second.status, SyncRunStatus::Success);
        assert_eq!(counts.personal_info, 1);
        assert_eq!(counts.daily_sleep, 7);
        assert_eq!(counts.daily_readiness, 7);
        assert_eq!(counts.daily_activity, 7);
        assert_eq!(counts.heartrate_samples, 12);
        assert_eq!(counts.workouts, 3);
        assert_eq!(counts.enhanced_tags, 4);
        assert_eq!(counts.sessions, 3);
        assert_eq!(counts.derived_context_events, 10);
    }

    #[tokio::test]
    async fn dry_run_does_not_write_any_rows() {
        let store = Store::open_in_memory().expect("store should open");
        let config = fixture_config();
        let report = sync_once(
            &config,
            &store,
            SyncOptions {
                dry_run: true,
                fixture_dir: Some(phase3_fixture_dir()),
                families: SyncFamily::ALL.to_vec(),
            },
        )
        .await
        .expect("dry run should succeed");
        let counts = store.views().record_counts().expect("record counts");

        assert_eq!(report.status, SyncRunStatus::Success);
        assert_eq!(counts.personal_info, 0);
        assert_eq!(counts.daily_sleep, 0);
        assert_eq!(counts.heartrate_samples, 0);
        assert_eq!(counts.workouts, 0);
        assert_eq!(counts.enhanced_tags, 0);
        assert_eq!(counts.sessions, 0);
    }
}
