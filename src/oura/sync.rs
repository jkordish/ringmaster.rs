use std::path::PathBuf;

use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::config::Config;
use crate::derive;
use crate::error::{AuthError, OuraApiError, OuraProblem, Result, RingmasterError};
use crate::oura::auth;
use crate::oura::client::{FixtureOuraClient, OuraClient, PageFetch, ReqwestOuraClient};
use crate::oura::models::{
    CapabilityKind, CapabilityReport, DailyActivityDocument, DailyCardiovascularAgeDocument,
    DailyReadinessDocument, DailyResilienceDocument, DailySleepDocument, DailyStressDocument,
    RestModePeriodDocument, SleepTimeDocument, Vo2MaxDocument, WorkoutDocument,
};
use crate::refresh::SyncFamily;
use crate::store::queries::{
    AuthSessionRecord, DailyActivityRecord, DailyCardiovascularAgeRecord, DailyReadinessRecord,
    DailyResilienceRecord, DailySleepRecord, DailyStressRecord, EnhancedTagRecord,
    HeartrateSampleRecord, OURA_PROVIDER, PersonalInfoRecord, RestModePeriodRecord, SessionRecord,
    SleepTimeRecord, SyncRunStatus, SyncStateRecord, Vo2MaxRecord, WorkoutRecord,
};
use crate::store::{Store, StorePlan};

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
    pub trigger_source: Option<String>,
    pub trigger_detail: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum SyncSourcePlan {
    Fixture(PathBuf),
    Live(Box<crate::oura::models::AuthStatus>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedSyncSelection {
    store_plan: StorePlan,
    families: Vec<SyncFamily>,
    options: SyncOptions,
    source: SyncSourcePlan,
}

struct SyncClientSetup {
    client: Box<dyn OuraClient>,
    capability_report: CapabilityReport,
    notes: Vec<String>,
}

struct BlockedSyncReportContext<'a> {
    config: &'a Config,
    store_plan: &'a StorePlan,
    families: &'a [SyncFamily],
    granted_scopes: &'a [String],
    capability_report: CapabilityReport,
    started_at: &'a str,
    database_path: &'a str,
    options: &'a SyncOptions,
}

enum PreparedSyncExecution {
    Ready(SyncClientSetup),
    Report(SyncReport),
}

/// # Errors
///
/// Returns an error if any selected Oura slice cannot be fetched, stored, or summarized.
pub fn sync_once<'a>(
    config: &'a Config,
    store: &'a Store,
    options: SyncOptions,
) -> impl std::future::Future<Output = Result<SyncReport>> + Send + 'a {
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
            trigger_source: options.trigger_source,
            trigger_detail: options.trigger_detail,
        },
    )
}

/// # Errors
///
/// Returns an error if authorization, slice retrieval, store writes, or sync-state updates fail.
pub fn sync_selected<'a>(
    config: &'a Config,
    store: &'a Store,
    options: SyncOptions,
) -> impl std::future::Future<Output = Result<SyncReport>> + Send + 'a {
    let prepared = prepare_sync_selection(config, store, options);
    async move {
        let prepared = prepared?;
        sync_selected_with_plan(config, prepared).await
    }
}

fn prepare_sync_selection(
    config: &Config,
    store: &Store,
    options: SyncOptions,
) -> Result<PreparedSyncSelection> {
    let families = if options.families.is_empty() {
        SyncFamily::ALL.to_vec()
    } else {
        options.families.clone()
    };
    let source = if let Some(fixture_dir) = options.fixture_dir.clone() {
        SyncSourcePlan::Fixture(fixture_dir)
    } else {
        SyncSourcePlan::Live(Box::new(auth::inspect_auth(config, store)?))
    };

    Ok(PreparedSyncSelection {
        store_plan: store.plan().clone(),
        families,
        options,
        source,
    })
}

async fn sync_selected_with_plan(
    config: &Config,
    prepared: PreparedSyncSelection,
) -> Result<SyncReport> {
    let started_at = now_rfc3339()?;
    let PreparedSyncSelection {
        store_plan,
        families,
        options,
        source,
    } = prepared;
    let database_path = store_plan.db_path.display().to_string();
    let setup = prepare_sync_client_setup(
        config,
        &store_plan,
        &families,
        &options,
        &source,
        &started_at,
        &database_path,
    )
    .await?;
    let PreparedSyncExecution::Ready(SyncClientSetup {
        client,
        capability_report,
        mut notes,
    }) = setup
    else {
        let PreparedSyncExecution::Report(report) = setup else {
            unreachable!("ready setup handled above");
        };
        return Ok(report);
    };

    let slice_reports = collect_sync_slice_reports(
        config,
        &store_plan,
        &families,
        client.as_ref(),
        &capability_report,
        &options,
    )
    .await?;
    append_post_sync_notes(config, &store_plan, &slice_reports, &options, &mut notes)?;
    let finished_at = now_rfc3339()?;
    Ok(finalize_sync_report(
        summarize_status(&slice_reports),
        started_at,
        finished_at,
        database_path,
        notes,
        capability_report,
        slice_reports,
    ))
}

async fn prepare_sync_client_setup(
    config: &Config,
    store_plan: &StorePlan,
    families: &[SyncFamily],
    options: &SyncOptions,
    source: &SyncSourcePlan,
    started_at: &str,
    database_path: &str,
) -> Result<PreparedSyncExecution> {
    match source {
        SyncSourcePlan::Fixture(fixture_dir) => {
            let client = FixtureOuraClient::new(config, fixture_dir.clone())?;
            let capability_report = client.capability_report();
            Ok(PreparedSyncExecution::Ready(SyncClientSetup {
                client: Box::new(client),
                capability_report,
                notes: vec![format!(
                    "Running fixture-backed sync from {}.",
                    fixture_dir.display()
                )],
            }))
        }
        SyncSourcePlan::Live(auth_status) => {
            prepare_live_sync_client_setup(
                config,
                store_plan,
                families,
                options,
                auth_status,
                started_at,
                database_path,
            )
            .await
        }
    }
}

async fn prepare_live_sync_client_setup(
    config: &Config,
    store_plan: &StorePlan,
    families: &[SyncFamily],
    options: &SyncOptions,
    auth_status: &crate::oura::models::AuthStatus,
    started_at: &str,
    database_path: &str,
) -> Result<PreparedSyncExecution> {
    if !auth_status.configured {
        return build_blocked_sync_report(
            BlockedSyncReportContext {
                config,
                store_plan,
                families,
                granted_scopes: &auth_status.granted_scopes,
                capability_report: auth_status.capability_report.clone(),
                started_at,
                database_path,
                options,
            },
            "Oura client credentials are not configured; live sync is blocked.",
            vec![
                "Add `RINGMASTER_OURA_CLIENT_ID` and `RINGMASTER_OURA_CLIENT_SECRET` to run live auth."
                    .to_owned(),
            ],
        );
    }

    if !auth_status.access_token_stored && !auth_status.refresh_token_stored {
        return build_blocked_sync_report(
            BlockedSyncReportContext {
                config,
                store_plan,
                families,
                granted_scopes: &auth_status.granted_scopes,
                capability_report: auth_status.capability_report.clone(),
                started_at,
                database_path,
                options,
            },
            "No persisted auth session is available yet; run `ringmaster auth login` first.",
            vec![
                "Live sync only reads persisted auth state; the TUI will never refresh tokens on the render path."
                    .to_owned(),
            ],
        );
    }

    let auth_store = reopen_store(config, store_plan)?;
    let session = match auth::ensure_authorized_session(config, &auth_store).await {
        Ok(session) => session,
        Err(error) => {
            let persist_store = reopen_store(config, store_plan)?;
            let slice_reports = persist_failed_slice_reports(
                config,
                &persist_store,
                families,
                &auth_status.granted_scopes,
                options,
                &error_problem(&error),
            )?;
            return Ok(PreparedSyncExecution::Report(finalize_sync_report(
                SyncRunStatus::Failed,
                started_at.to_owned(),
                now_rfc3339()?,
                database_path.to_owned(),
                vec![
                    "The scheduler keeps auth handling out of the render path and persists the failure state for each requested family.".to_owned(),
                ],
                auth_status.capability_report.clone(),
                slice_reports,
            )));
        }
    };

    let client = ReqwestOuraClient::new(config, session.access_token, &session.granted_scopes)?;
    let capability_report = client.capability_report();
    Ok(PreparedSyncExecution::Ready(SyncClientSetup {
        client: Box::new(client),
        capability_report,
        notes: Vec::new(),
    }))
}

fn build_blocked_sync_report(
    context: BlockedSyncReportContext<'_>,
    blocked_message: &str,
    notes: Vec<String>,
) -> Result<PreparedSyncExecution> {
    let persist_store = reopen_store(context.config, context.store_plan)?;
    let slice_reports = persist_blocked_slice_reports(
        context.config,
        &persist_store,
        context.families,
        context.granted_scopes,
        blocked_message,
        context.options,
    )?;
    Ok(PreparedSyncExecution::Report(finalize_sync_report(
        SyncRunStatus::Blocked,
        context.started_at.to_owned(),
        now_rfc3339()?,
        context.database_path.to_owned(),
        notes,
        context.capability_report,
        slice_reports,
    )))
}

async fn collect_sync_slice_reports(
    config: &Config,
    store_plan: &StorePlan,
    families: &[SyncFamily],
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<Vec<SliceReport>> {
    let mut slice_reports = Vec::new();
    for family in families {
        slice_reports.push(
            run_sync_family_or_persist_failure(
                config,
                store_plan,
                *family,
                client,
                capability_report,
                options,
            )
            .await?,
        );
    }
    Ok(slice_reports)
}

async fn run_sync_family_or_persist_failure(
    config: &Config,
    store_plan: &StorePlan,
    family: SyncFamily,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    let report = run_sync_family(
        config,
        store_plan.clone(),
        family,
        client,
        capability_report,
        options,
    )
    .await;
    match report {
        Ok(report) => Ok(report),
        Err(error) => {
            let persist_store = reopen_store(config, store_plan)?;
            persist_slice_report(
                config,
                &persist_store,
                failed_slice_report(family.sync_key(), error_problem(&error)),
                granted_scopes_from_report(capability_report),
                options,
            )
        }
    }
}

async fn run_sync_family(
    config: &Config,
    store_plan: StorePlan,
    family: SyncFamily,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    match family {
        SyncFamily::Personal => {
            sync_personal_info(config, store_plan, client, capability_report, options).await
        }
        SyncFamily::Daily => {
            sync_daily(config, store_plan, client, capability_report, options).await
        }
        SyncFamily::Heartrate => {
            sync_heartrate(config, store_plan, client, capability_report, options).await
        }
        SyncFamily::Workout => {
            sync_workouts(config, store_plan, client, capability_report, options).await
        }
        SyncFamily::EnhancedTag => {
            sync_enhanced_tags(config, store_plan, client, capability_report, options).await
        }
        SyncFamily::Session => {
            sync_sessions(config, store_plan, client, capability_report, options).await
        }
    }
}

fn append_post_sync_notes(
    config: &Config,
    store_plan: &StorePlan,
    slice_reports: &[SliceReport],
    options: &SyncOptions,
    notes: &mut Vec<String>,
) -> Result<()> {
    if !options.dry_run && should_rebuild_derived_state(slice_reports) {
        let derive_store = reopen_store(config, store_plan)?;
        let derive_report = derive::rebuild_recent_store(&derive_store, config)?;
        notes.push(format!(
            "Derived context events and pattern summaries were rebuilt after sync (events={}, patterns={}).",
            derive_report.context_event_count, derive_report.pattern_summary_count
        ));
        notes.extend(derive_report.notes);
    }

    if options.dry_run {
        notes.push("Dry-run mode fetched and normalized data without mutating SQLite.".to_owned());
    } else {
        notes.push("Raw payloads and normalized rows were persisted to SQLite.".to_owned());
    }
    Ok(())
}

const fn finalize_sync_report(
    status: SyncRunStatus,
    started_at: String,
    finished_at: String,
    database_path: String,
    notes: Vec<String>,
    capability_report: CapabilityReport,
    slice_reports: Vec<SliceReport>,
) -> SyncReport {
    SyncReport {
        status,
        started_at,
        finished_at,
        database_path,
        notes,
        capability_report,
        slice_reports,
    }
}

fn should_rebuild_derived_state(slice_reports: &[SliceReport]) -> bool {
    slice_reports.iter().any(|report| {
        matches!(
            report.status,
            SyncRunStatus::Success | SyncRunStatus::Partial
        ) && matches!(
            report.sync_key.as_str(),
            DAILY_SYNC_KEY | WORKOUT_SYNC_KEY | ENHANCED_TAG_SYNC_KEY | SESSION_SYNC_KEY
        )
    })
}

async fn sync_personal_info(
    config: &Config,
    store_plan: StorePlan,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !capability_report.is_granted(CapabilityKind::Personal) {
        let persist_store = reopen_store(config, &store_plan)?;
        return persist_slice_report(
            config,
            &persist_store,
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
    let persist_store = reopen_store(config, &store_plan)?;

    if !options.dry_run {
        persist_store
            .imports()
            .upsert_raw_payload(&fetched.raw_payload)?;
        persist_store
            .imports()
            .upsert_personal_info(&PersonalInfoRecord {
                profile_id: fetched.document.id.clone(),
                age: fetched.document.age,
                weight: fetched.document.weight,
                height: fetched.document.height,
                biological_sex: fetched.document.biological_sex.clone(),
                email: fetched.document.email.clone(),
                raw_cache_key: Some(fetched.raw_payload.cache_key.clone()),
                updated_at: imported_at.clone(),
            })?;

        if let Some(mut auth_session) = persist_store.auth().get(OURA_PROVIDER)? {
            auth_session.account_id = Some(fetched.document.id.clone());
            auth_session
                .account_email
                .clone_from(&fetched.document.email);
            auth_session.updated_at.clone_from(&imported_at);
            persist_store.auth().upsert(&auth_session)?;
        } else {
            persist_store.auth().upsert(&AuthSessionRecord {
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
        &persist_store,
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

fn reopen_store(config: &Config, store_plan: &StorePlan) -> Result<Store> {
    Store::open_with_plan(store_plan.clone(), config.app_name)
}

struct DailySyncWindow {
    start_date: String,
    end_date: String,
}

struct DailyPageFetches {
    sleep_pages: Vec<PageFetch<DailySleepDocument>>,
    readiness_pages: Vec<PageFetch<DailyReadinessDocument>>,
    activity_pages: Vec<PageFetch<DailyActivityDocument>>,
    sleep_time_pages: Vec<PageFetch<SleepTimeDocument>>,
    rest_mode_period_pages: Vec<PageFetch<RestModePeriodDocument>>,
    daily_stress_pages: Vec<PageFetch<DailyStressDocument>>,
    daily_resilience_pages: Vec<PageFetch<DailyResilienceDocument>>,
    cardiovascular_age_pages: Vec<PageFetch<DailyCardiovascularAgeDocument>>,
    vo2_max_pages: Vec<PageFetch<Vo2MaxDocument>>,
    optional_failures: Vec<(&'static str, OuraProblem)>,
}

async fn sync_daily(
    config: &Config,
    store_plan: StorePlan,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !capability_report.is_granted(CapabilityKind::Daily) {
        let persist_store = reopen_store(config, &store_plan)?;
        return persist_slice_report(
            config,
            &persist_store,
            slice_blocked(
                DAILY_SYNC_KEY,
                "Missing `daily` scope; dashboard summary rows remain unavailable.",
            ),
            granted_scopes_from_report(capability_report),
            options,
        );
    }

    let window = resolve_daily_sync_window(config, &store_plan, options)?;
    let pages = fetch_daily_pages(client, &window).await?;
    let imported_at = now_rfc3339()?;
    let persist_store = reopen_store(config, &store_plan)?;
    persist_daily_pages(&persist_store, &pages, &imported_at, options)?;
    let (status, message, last_error, imported_rows) = summarize_daily_sync(&window, &pages);
    persist_slice_report(
        config,
        &persist_store,
        SliceReport {
            sync_key: DAILY_SYNC_KEY.to_owned(),
            status,
            imported_rows,
            watermark: Some(window.end_date.clone()),
            message,
            last_error,
            next_attempt_after: None,
        },
        granted_scopes_from_report(capability_report),
        options,
    )
}

fn resolve_daily_sync_window(
    config: &Config,
    store_plan: &StorePlan,
    options: &SyncOptions,
) -> Result<DailySyncWindow> {
    let end_date = utc_date_string(OffsetDateTime::now_utc());
    let start_date = if options.fixture_dir.is_some() {
        "1970-01-01".to_owned()
    } else {
        let overlap_store = reopen_store(config, store_plan)?;
        overlap_day_window(
            &overlap_store,
            DAILY_SYNC_KEY,
            i64::from(config.refresh.daily_history_days),
            i64::from(config.refresh.daily_overlap_days),
        )?
    };
    Ok(DailySyncWindow {
        start_date,
        end_date,
    })
}

async fn fetch_daily_pages(
    client: &dyn OuraClient,
    window: &DailySyncWindow,
) -> Result<DailyPageFetches> {
    let (
        sleep_pages_result,
        readiness_pages_result,
        activity_pages_result,
        sleep_time_pages_result,
        rest_mode_period_pages_result,
        daily_stress_pages_result,
        daily_resilience_pages_result,
        cardiovascular_age_pages_result,
        vo2_max_pages_result,
    ) = tokio::join!(
        client.fetch_daily_sleep(window.start_date.clone(), window.end_date.clone()),
        client.fetch_daily_readiness(window.start_date.clone(), window.end_date.clone()),
        client.fetch_daily_activity(window.start_date.clone(), window.end_date.clone()),
        client.fetch_sleep_time(window.start_date.clone(), window.end_date.clone()),
        client.fetch_rest_mode_periods(window.start_date.clone(), window.end_date.clone()),
        client.fetch_daily_stress(window.start_date.clone(), window.end_date.clone()),
        client.fetch_daily_resilience(window.start_date.clone(), window.end_date.clone()),
        client.fetch_daily_cardiovascular_age(window.start_date.clone(), window.end_date.clone()),
        client.fetch_vo2_max(window.start_date.clone(), window.end_date.clone()),
    );
    let mut optional_failures = Vec::new();

    Ok(DailyPageFetches {
        sleep_pages: sleep_pages_result?,
        readiness_pages: readiness_pages_result?,
        activity_pages: activity_pages_result?,
        sleep_time_pages: collect_optional_daily_pages(
            "sleep_time",
            sleep_time_pages_result,
            &mut optional_failures,
        ),
        rest_mode_period_pages: collect_optional_daily_pages(
            "rest_mode_period",
            rest_mode_period_pages_result,
            &mut optional_failures,
        ),
        daily_stress_pages: collect_optional_daily_pages(
            "daily_stress",
            daily_stress_pages_result,
            &mut optional_failures,
        ),
        daily_resilience_pages: collect_optional_daily_pages(
            "daily_resilience",
            daily_resilience_pages_result,
            &mut optional_failures,
        ),
        cardiovascular_age_pages: collect_optional_daily_pages(
            "daily_cardiovascular_age",
            cardiovascular_age_pages_result,
            &mut optional_failures,
        ),
        vo2_max_pages: collect_optional_daily_pages(
            "vo2_max",
            vo2_max_pages_result,
            &mut optional_failures,
        ),
        optional_failures,
    })
}

fn persist_daily_pages(
    persist_store: &Store,
    pages: &DailyPageFetches,
    imported_at: &str,
    options: &SyncOptions,
) -> Result<()> {
    if options.dry_run {
        return Ok(());
    }

    persist_daily_sleep_pages(persist_store, &pages.sleep_pages, imported_at)?;
    persist_daily_readiness_pages(persist_store, &pages.readiness_pages, imported_at)?;
    persist_daily_activity_pages(persist_store, &pages.activity_pages, imported_at)?;
    persist_sleep_time_pages(persist_store, &pages.sleep_time_pages, imported_at)?;
    persist_rest_mode_period_pages(persist_store, &pages.rest_mode_period_pages, imported_at)?;
    persist_daily_stress_pages(persist_store, &pages.daily_stress_pages, imported_at)?;
    persist_daily_resilience_pages(persist_store, &pages.daily_resilience_pages, imported_at)?;
    persist_cardiovascular_age_pages(persist_store, &pages.cardiovascular_age_pages, imported_at)?;
    persist_vo2_max_pages(persist_store, &pages.vo2_max_pages, imported_at)?;

    Ok(())
}

fn persist_daily_sleep_pages(
    persist_store: &Store,
    pages: &[PageFetch<DailySleepDocument>],
    imported_at: &str,
) -> Result<()> {
    for page in pages {
        persist_store
            .imports()
            .upsert_raw_payload(&page.raw_payload)?;
        for document in &page.documents {
            persist_store
                .imports()
                .upsert_daily_sleep(&DailySleepRecord {
                    oura_id: Some(document.id.clone()),
                    day: document.day.clone(),
                    sleep_score: document.score,
                    sleep_duration_seconds: document.sleep_duration_seconds,
                    raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                    updated_at: imported_at.to_owned(),
                })?;
        }
    }
    Ok(())
}

fn persist_daily_readiness_pages(
    persist_store: &Store,
    pages: &[PageFetch<DailyReadinessDocument>],
    imported_at: &str,
) -> Result<()> {
    for page in pages {
        persist_store
            .imports()
            .upsert_raw_payload(&page.raw_payload)?;
        for document in &page.documents {
            persist_store
                .imports()
                .upsert_daily_readiness(&DailyReadinessRecord {
                    oura_id: Some(document.id.clone()),
                    day: document.day.clone(),
                    readiness_score: document.score,
                    temperature_deviation: document.temperature_deviation,
                    temperature_trend_deviation: document.temperature_trend_deviation,
                    raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                    updated_at: imported_at.to_owned(),
                })?;
        }
    }
    Ok(())
}

fn persist_daily_activity_pages(
    persist_store: &Store,
    pages: &[PageFetch<DailyActivityDocument>],
    imported_at: &str,
) -> Result<()> {
    for page in pages {
        persist_store
            .imports()
            .upsert_raw_payload(&page.raw_payload)?;
        for document in &page.documents {
            persist_store
                .imports()
                .upsert_daily_activity(&DailyActivityRecord {
                    oura_id: Some(document.id.clone()),
                    day: document.day.clone(),
                    activity_score: document.score,
                    active_calories: document.active_calories,
                    steps: document.steps,
                    total_calories: document.total_calories,
                    raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                    updated_at: imported_at.to_owned(),
                })?;
        }
    }
    Ok(())
}

fn persist_sleep_time_pages(
    persist_store: &Store,
    pages: &[PageFetch<SleepTimeDocument>],
    imported_at: &str,
) -> Result<()> {
    for page in pages {
        persist_store
            .imports()
            .upsert_raw_payload(&page.raw_payload)?;
        for document in &page.documents {
            let optimal_bedtime = document.optimal_bedtime.as_ref();
            persist_store
                .imports()
                .upsert_sleep_time(&SleepTimeRecord {
                    oura_id: Some(document.id.clone()),
                    day: document.day.clone(),
                    status: document
                        .status
                        .as_ref()
                        .map(|value| value.as_str().to_owned()),
                    recommendation: document
                        .recommendation
                        .as_ref()
                        .map(|value| value.as_str().to_owned()),
                    optimal_bedtime_start_offset: optimal_bedtime
                        .map(|window| i64::from(window.start_offset)),
                    optimal_bedtime_end_offset: optimal_bedtime
                        .map(|window| i64::from(window.end_offset)),
                    optimal_bedtime_day_tz: optimal_bedtime.map(|window| i64::from(window.day_tz)),
                    raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                    updated_at: imported_at.to_owned(),
                })?;
        }
    }
    Ok(())
}

fn persist_rest_mode_period_pages(
    persist_store: &Store,
    pages: &[PageFetch<RestModePeriodDocument>],
    imported_at: &str,
) -> Result<()> {
    for page in pages {
        persist_store
            .imports()
            .upsert_raw_payload(&page.raw_payload)?;
        for document in &page.documents {
            persist_store
                .imports()
                .upsert_rest_mode_period(&RestModePeriodRecord {
                    period_id: document.id.clone(),
                    start_day: document.start_day.clone(),
                    start_time: document.start_time.clone(),
                    end_day: document.end_day.clone(),
                    end_time: document.end_time.clone(),
                    episode_count: u32::try_from(document.episodes.len()).map_err(|_| {
                        RingmasterError::Config(
                            "rest mode episode count exceeded u32 range".to_owned(),
                        )
                    })?,
                    tags_json: document.tags_json()?,
                    raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                    updated_at: imported_at.to_owned(),
                })?;
        }
    }
    Ok(())
}

fn persist_daily_stress_pages(
    persist_store: &Store,
    pages: &[PageFetch<DailyStressDocument>],
    imported_at: &str,
) -> Result<()> {
    for page in pages {
        persist_store
            .imports()
            .upsert_raw_payload(&page.raw_payload)?;
        for document in &page.documents {
            persist_store
                .imports()
                .upsert_daily_stress(&DailyStressRecord {
                    oura_id: Some(document.id.clone()),
                    day: document.day.clone(),
                    stress_high: document.stress_high,
                    recovery_high: document.recovery_high,
                    day_summary: document.day_summary.clone(),
                    raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                    updated_at: imported_at.to_owned(),
                })?;
        }
    }
    Ok(())
}

fn persist_daily_resilience_pages(
    persist_store: &Store,
    pages: &[PageFetch<DailyResilienceDocument>],
    imported_at: &str,
) -> Result<()> {
    for page in pages {
        persist_store
            .imports()
            .upsert_raw_payload(&page.raw_payload)?;
        for document in &page.documents {
            persist_store
                .imports()
                .upsert_daily_resilience(&DailyResilienceRecord {
                    oura_id: Some(document.id.clone()),
                    day: document.day.clone(),
                    level: document.level.as_str().to_owned(),
                    sleep_recovery: document.contributors.sleep_recovery,
                    daytime_recovery: document.contributors.daytime_recovery,
                    stress: document.contributors.stress,
                    raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                    updated_at: imported_at.to_owned(),
                })?;
        }
    }
    Ok(())
}

fn persist_cardiovascular_age_pages(
    persist_store: &Store,
    pages: &[PageFetch<DailyCardiovascularAgeDocument>],
    imported_at: &str,
) -> Result<()> {
    for page in pages {
        persist_store
            .imports()
            .upsert_raw_payload(&page.raw_payload)?;
        for document in &page.documents {
            persist_store.imports().upsert_daily_cardiovascular_age(
                &DailyCardiovascularAgeRecord {
                    day: document.day.clone(),
                    vascular_age: document.vascular_age,
                    raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                    updated_at: imported_at.to_owned(),
                },
            )?;
        }
    }
    Ok(())
}

fn persist_vo2_max_pages(
    persist_store: &Store,
    pages: &[PageFetch<Vo2MaxDocument>],
    imported_at: &str,
) -> Result<()> {
    for page in pages {
        persist_store
            .imports()
            .upsert_raw_payload(&page.raw_payload)?;
        for document in &page.documents {
            persist_store.imports().upsert_vo2_max(&Vo2MaxRecord {
                oura_id: Some(document.id.clone()),
                day: document.day.clone(),
                recorded_at: document.timestamp.clone(),
                vo2_max: document.vo2_max,
                raw_cache_key: Some(page.raw_payload.cache_key.clone()),
                updated_at: imported_at.to_owned(),
            })?;
        }
    }
    Ok(())
}

fn summarize_daily_sync(
    window: &DailySyncWindow,
    pages: &DailyPageFetches,
) -> (SyncRunStatus, String, Option<OuraProblem>, usize) {
    let imported_rows = count_documents(&pages.sleep_pages)
        + count_documents(&pages.readiness_pages)
        + count_documents(&pages.activity_pages)
        + count_documents(&pages.sleep_time_pages)
        + count_documents(&pages.rest_mode_period_pages)
        + count_documents(&pages.daily_stress_pages)
        + count_documents(&pages.daily_resilience_pages)
        + count_documents(&pages.cardiovascular_age_pages)
        + count_documents(&pages.vo2_max_pages);

    if pages.optional_failures.is_empty() {
        (
            SyncRunStatus::Success,
            format!(
                "Imported {imported_rows} daily summary and review-support rows from {} through {}.",
                window.start_date, window.end_date
            ),
            None,
            imported_rows,
        )
    } else {
        let failure_summary = pages
            .optional_failures
            .iter()
            .map(|(endpoint, problem)| format!("{endpoint} ({problem})"))
            .collect::<Vec<_>>()
            .join("; ");
        (
            SyncRunStatus::Partial,
            format!(
                "Imported {imported_rows} core daily rows from {} through {}; optional review-support endpoints degraded independently: {failure_summary}.",
                window.start_date, window.end_date
            ),
            pages
                .optional_failures
                .first()
                .map(|(_, problem)| problem.clone()),
            imported_rows,
        )
    }
}

async fn sync_heartrate(
    config: &Config,
    store_plan: StorePlan,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !capability_report.is_granted(CapabilityKind::Heartrate) {
        let persist_store = reopen_store(config, &store_plan)?;
        return persist_slice_report(
            config,
            &persist_store,
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
        let overlap_store = reopen_store(config, &store_plan)?;
        overlap_heartrate_window(
            &overlap_store,
            i64::from(config.refresh.heartrate_history_days),
            i64::from(config.refresh.heartrate_overlap_minutes),
        )?
    };
    let heartrate_pages = client
        .fetch_heartrate(start_datetime.clone(), end_datetime.clone())
        .await?;
    let imported_at = now_rfc3339()?;
    let persist_store = reopen_store(config, &store_plan)?;

    if !options.dry_run {
        for page in &heartrate_pages {
            persist_store
                .imports()
                .upsert_raw_payload(&page.raw_payload)?;
            for document in &page.documents {
                persist_store
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
        &persist_store,
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
    store_plan: StorePlan,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !capability_report.is_granted(CapabilityKind::Workout) {
        let persist_store = reopen_store(config, &store_plan)?;
        return persist_slice_report(
            config,
            &persist_store,
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
        let overlap_store = reopen_store(config, &store_plan)?;
        overlap_day_window(
            &overlap_store,
            WORKOUT_SYNC_KEY,
            i64::from(config.refresh.workout_history_days),
            i64::from(config.refresh.workout_overlap_days),
        )?
    };
    let workout_pages = client
        .fetch_workouts(start_date.clone(), end_date.clone())
        .await?;
    let imported_at = now_rfc3339()?;
    let persist_store = reopen_store(config, &store_plan)?;

    if !options.dry_run {
        for page in &workout_pages {
            persist_store
                .imports()
                .upsert_raw_payload(&page.raw_payload)?;
            for document in &page.documents {
                persist_store.imports().upsert_workout(&WorkoutRecord {
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
        &persist_store,
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
    store_plan: StorePlan,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !capability_report.is_granted(CapabilityKind::EnhancedTag) {
        let persist_store = reopen_store(config, &store_plan)?;
        return persist_slice_report(
            config,
            &persist_store,
            slice_blocked(
                ENHANCED_TAG_SYNC_KEY,
                "Missing `tag` scope; tag overlays and explainability evidence remain unavailable.",
            ),
            granted_scopes_from_report(capability_report),
            options,
        );
    }

    let end_date = utc_date_string(OffsetDateTime::now_utc());
    let start_date = if options.fixture_dir.is_some() {
        "1970-01-01".to_owned()
    } else {
        let overlap_store = reopen_store(config, &store_plan)?;
        overlap_day_window(
            &overlap_store,
            ENHANCED_TAG_SYNC_KEY,
            i64::from(config.refresh.enhanced_tag_history_days),
            i64::from(config.refresh.enhanced_tag_overlap_days),
        )?
    };
    let enhanced_tag_pages = client
        .fetch_enhanced_tags(start_date.clone(), end_date.clone())
        .await?;
    let imported_at = now_rfc3339()?;
    let persist_store = reopen_store(config, &store_plan)?;

    if !options.dry_run {
        for page in &enhanced_tag_pages {
            persist_store
                .imports()
                .upsert_raw_payload(&page.raw_payload)?;
            for document in &page.documents {
                persist_store
                    .imports()
                    .upsert_enhanced_tag(&EnhancedTagRecord {
                        enhanced_tag_id: document.id.clone(),
                        day: document.anchor_day().to_owned(),
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
        &persist_store,
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
    store_plan: StorePlan,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !capability_report.is_granted(CapabilityKind::Session) {
        let persist_store = reopen_store(config, &store_plan)?;
        return persist_slice_report(
            config,
            &persist_store,
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
        let overlap_store = reopen_store(config, &store_plan)?;
        overlap_day_window(
            &overlap_store,
            SESSION_SYNC_KEY,
            i64::from(config.refresh.session_history_days),
            i64::from(config.refresh.session_overlap_days),
        )?
    };
    let session_pages = client
        .fetch_sessions(start_date.clone(), end_date.clone())
        .await?;
    let imported_at = now_rfc3339()?;
    let persist_store = reopen_store(config, &store_plan)?;

    if !options.dry_run {
        for page in &session_pages {
            persist_store
                .imports()
                .upsert_raw_payload(&page.raw_payload)?;
            for document in &page.documents {
                persist_store.imports().upsert_session(&SessionRecord {
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
        &persist_store,
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
    granted_scopes: &[String],
    options: &SyncOptions,
    problem: &OuraProblem,
) -> Result<Vec<SliceReport>> {
    families
        .iter()
        .copied()
        .map(|family| {
            persist_slice_report(
                config,
                store,
                failed_slice_report(family.sync_key(), problem.clone()),
                granted_scopes.to_vec(),
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
                .map_or(1, |state| state.failure_count.saturating_add(1)),
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
            last_trigger_source: Some(
                options
                    .trigger_source
                    .clone()
                    .unwrap_or_else(|| "periodic_reconcile".to_owned()),
            ),
            last_trigger_detail: Some(
                options
                    .trigger_detail
                    .clone()
                    .unwrap_or_else(|| "sync_selected".to_owned()),
            ),
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
    let message = format!("{sync_key}: {problem}");
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
    let Some(sync_state) = store.sync_state().get(sync_key)?.filter(|record| {
        matches!(
            record.status,
            SyncRunStatus::Success | SyncRunStatus::Partial
        )
    }) else {
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

fn collect_optional_daily_pages<T>(
    endpoint: &'static str,
    result: Result<Vec<crate::oura::client::PageFetch<T>>>,
    failures: &mut Vec<(&'static str, OuraProblem)>,
) -> Vec<crate::oura::client::PageFetch<T>> {
    match result {
        Ok(pages) => pages,
        Err(error) => {
            failures.push((endpoint, error_problem(&error)));
            Vec::new()
        }
    }
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

    (OffsetDateTime::now_utc() + Duration::seconds(backoff_secs.cast_signed()))
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
        .granted_scope_names()
        .into_iter()
        .map(str::to_owned)
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
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{SyncOptions, sync_once};
    use crate::config::{
        AppPaths, Config, DEFAULT_OURA_API_BASE_URL, DEFAULT_OURA_AUTHORIZE_URL,
        DEFAULT_OURA_TOKEN_URL, LoggingConfig, OuraConfig, RefreshConfig, WebhookConfig,
    };
    use crate::refresh::SyncFamily;
    use crate::store::Store;
    use crate::store::queries::{SyncRunStatus, SyncStateRecord};
    use crate::test_support::{ok, some};
    use crate::webhook::default_desired_subscriptions;

    fn baseline_fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/phase3")
    }

    fn review_fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/phase5")
    }

    fn copy_fixture_dir(source: &Path, destination: &Path) {
        fs::create_dir_all(destination)
            .unwrap_or_else(|error| unreachable!("fixture destination should exist: {error}"));
        for entry in fs::read_dir(source).unwrap_or_else(|error| {
            unreachable!(
                "fixture directory {} should read: {error}",
                source.display()
            )
        }) {
            let entry =
                entry.unwrap_or_else(|error| unreachable!("fixture entry should load: {error}"));
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            if source_path.is_dir() {
                copy_fixture_dir(&source_path, &destination_path);
            } else {
                fs::copy(&source_path, &destination_path).unwrap_or_else(|error| {
                    unreachable!(
                        "fixture file {} should copy to {}: {error}",
                        source_path.display(),
                        destination_path.display()
                    )
                });
            }
        }
    }

    fn fixture_config() -> Config {
        let paths = ok(
            AppPaths::from_roots(
                PathBuf::from("/home/tester"),
                PathBuf::from("/tmp/config"),
                PathBuf::from("/tmp/state"),
                PathBuf::from("/tmp/cache"),
            ),
            "paths should resolve",
        );
        Config {
            app_name: "ringmaster",
            paths,
            logging: LoggingConfig {
                filter: "ringmaster=debug".to_owned(),
            },
            oura: OuraConfig {
                client_id: None,
                client_secret: None,
                authorize_url: DEFAULT_OURA_AUTHORIZE_URL.to_owned(),
                token_url: DEFAULT_OURA_TOKEN_URL.to_owned(),
                api_base_url: DEFAULT_OURA_API_BASE_URL.to_owned(),
                secret_backend: crate::config::OuraSecretBackend::Keyring,
                secret_file: PathBuf::from("/tmp/state/ringmaster/secrets/oura-tokens.json"),
                callback_bind: ok("127.0.0.1:8788".parse(), "callback bind should parse"),
                callback_path: "/callback".to_owned(),
                requested_scopes: vec![
                    "personal".to_owned(),
                    "daily".to_owned(),
                    "heartrate".to_owned(),
                    "workout".to_owned(),
                    "tag".to_owned(),
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
                bind: ok("127.0.0.1:8799".parse(), "webhook bind should parse"),
                path: "/webhooks/oura".to_owned(),
                public_base_url: Some("https://example.test".to_owned()),
                verification_token: Some("verify-me".to_owned()),
                signature_tolerance_secs: 300,
                heartbeat_secs: 15,
                renewal_lead_secs: 7 * 24 * 60 * 60,
                subscriptions: default_desired_subscriptions(),
            },
            guidance: crate::config::GuidanceConfig::default(),
            ai: crate::config::AiConfig::default(),
        }
    }

    #[tokio::test]
    async fn fixture_sync_populates_baseline_tables_idempotently() {
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        let options = SyncOptions {
            dry_run: false,
            fixture_dir: Some(baseline_fixture_dir()),
            families: SyncFamily::ALL.to_vec(),
            trigger_source: Some("periodic_reconcile".to_owned()),
            trigger_detail: Some("test fixture sync".to_owned()),
        };

        let first = ok(
            sync_once(&config, &store, options.clone()).await,
            "first fixture sync should succeed",
        );
        let second = ok(
            sync_once(&config, &store, options).await,
            "second fixture sync should stay idempotent",
        );
        let counts = ok(store.views().record_counts(), "record counts should load");

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
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        let report = sync_once(
            &config,
            &store,
            SyncOptions {
                dry_run: true,
                fixture_dir: Some(baseline_fixture_dir()),
                families: SyncFamily::ALL.to_vec(),
                trigger_source: Some("periodic_reconcile".to_owned()),
                trigger_detail: Some("test dry-run sync".to_owned()),
            },
        )
        .await;
        let report = ok(report, "dry run should succeed");
        let counts = ok(store.views().record_counts(), "record counts should load");

        assert_eq!(report.status, SyncRunStatus::Success);
        assert_eq!(counts.personal_info, 0);
        assert_eq!(counts.daily_sleep, 0);
        assert_eq!(counts.heartrate_samples, 0);
        assert_eq!(counts.workouts, 0);
        assert_eq!(counts.enhanced_tags, 0);
        assert_eq!(counts.sessions, 0);
    }

    #[tokio::test]
    async fn fixture_sync_populates_review_family_tables() {
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        let report = sync_once(
            &config,
            &store,
            SyncOptions {
                dry_run: false,
                fixture_dir: Some(review_fixture_dir()),
                families: SyncFamily::ALL.to_vec(),
                trigger_source: Some("periodic_reconcile".to_owned()),
                trigger_detail: Some("test review fixture sync".to_owned()),
            },
        )
        .await;
        let report = ok(report, "review fixture sync should succeed");
        let counts = ok(store.views().record_counts(), "record counts should load");
        let latest_source_day = ok(
            store.views().latest_source_day(),
            "latest source day should load",
        );

        assert_eq!(report.status, SyncRunStatus::Success);
        assert_eq!(counts.sleep_time, 7);
        assert_eq!(counts.daily_stress, 7);
        assert_eq!(counts.daily_resilience, 7);
        assert_eq!(counts.daily_cardiovascular_age, 7);
        assert_eq!(counts.vo2_max, 7);
        assert_eq!(counts.rest_mode_periods, 2);
        assert_eq!(latest_source_day.as_deref(), Some("2026-04-08"));
    }

    #[tokio::test]
    async fn fixture_sync_accepts_official_enhanced_tag_start_day_payloads() {
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        let tempdir = ok(tempfile::tempdir(), "tempdir should build");
        let fixture_dir = tempdir.path().join("review-official-enhanced-tags");
        copy_fixture_dir(&review_fixture_dir(), &fixture_dir);
        ok(
            fs::write(
                fixture_dir.join("enhanced_tags.json"),
                serde_json::to_string_pretty(&serde_json::json!({
                    "data": [
                        {
                            "id": "etag_2026-04-04_caffeine",
                            "start_day": "2026-04-04",
                            "end_day": "2026-04-04",
                            "start_time": "2026-04-04T20:15:00Z",
                            "end_time": "2026-04-04T20:15:00Z",
                            "tag_type_code": "caffeine",
                            "tags": ["Late coffee"],
                            "comment": "Espresso after dinner.",
                            "intensity": "medium"
                        },
                        {
                            "id": "etag_2026-04-05_stress",
                            "start_day": "2026-04-05",
                            "end_day": "2026-04-05",
                            "start_time": "2026-04-05T09:00:00Z",
                            "end_time": "2026-04-05T11:30:00Z",
                            "tag_type_code": "stress",
                            "tags": ["Travel day"],
                            "comment": "Packed morning with back-to-back errands.",
                            "intensity": "high"
                        }
                    ],
                    "next_token": null
                }))
                .unwrap_or_else(|error| {
                    unreachable!("official enhanced tag fixture should encode: {error}")
                }),
            ),
            "official enhanced tag fixture should write",
        );

        let report = sync_once(
            &config,
            &store,
            SyncOptions {
                dry_run: false,
                fixture_dir: Some(fixture_dir),
                families: SyncFamily::ALL.to_vec(),
                trigger_source: Some("periodic_reconcile".to_owned()),
                trigger_detail: Some("test official enhanced tag fixture sync".to_owned()),
            },
        )
        .await;
        let report = ok(
            report,
            "review fixture sync should succeed with official enhanced tag shape",
        );
        let counts = ok(store.views().record_counts(), "record counts should load");

        assert_eq!(report.status, SyncRunStatus::Success);
        assert!(counts.enhanced_tags > 0);
    }

    #[tokio::test]
    async fn daily_sync_degrades_when_optional_review_endpoint_fixture_is_malformed() {
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        let tempdir = ok(tempfile::tempdir(), "tempdir should build");
        let fixture_dir = tempdir.path().join("review-malformed-sleep-time");
        copy_fixture_dir(&review_fixture_dir(), &fixture_dir);
        ok(
            fs::write(fixture_dir.join("sleep_time.json"), "{ not valid json"),
            "optional fixture should be rewritable",
        );

        let report = sync_once(
            &config,
            &store,
            SyncOptions {
                dry_run: false,
                fixture_dir: Some(fixture_dir),
                families: vec![SyncFamily::Daily],
                trigger_source: Some("periodic_reconcile".to_owned()),
                trigger_detail: Some("test degraded optional daily sync".to_owned()),
            },
        )
        .await;
        let report = ok(report, "daily sync should degrade instead of failing");
        let counts = ok(store.views().record_counts(), "record counts should load");
        let daily_slice = some(
            report
                .slice_reports
                .iter()
                .find(|slice| slice.sync_key == "oura.daily"),
            "daily slice should exist",
        );

        assert_eq!(report.status, SyncRunStatus::Partial);
        assert_eq!(daily_slice.status, SyncRunStatus::Partial);
        assert!(daily_slice.message.contains("sleep_time"));
        assert_eq!(counts.daily_sleep, 7);
        assert_eq!(counts.daily_readiness, 7);
        assert_eq!(counts.daily_activity, 7);
        assert_eq!(counts.sleep_time, 0);
        assert!(daily_slice.last_error.is_some());
    }

    #[test]
    fn partial_daily_slice_still_triggers_derive_rebuild() {
        assert!(super::should_rebuild_derived_state(&[super::SliceReport {
            sync_key: "oura.daily".to_owned(),
            status: SyncRunStatus::Partial,
            imported_rows: 3,
            watermark: Some("2026-04-08".to_owned()),
            message: "partial daily sync".to_owned(),
            last_error: None,
            next_attempt_after: None,
        }]));
    }

    #[test]
    fn overlap_day_window_reuses_partial_daily_cursor() {
        let store = ok(Store::open_test_store(), "store should open");
        ok(
            store.sync_state().upsert(&SyncStateRecord {
                sync_key: "oura.daily".to_owned(),
                status: SyncRunStatus::Partial,
                cursor: Some("2026-04-08".to_owned()),
                last_attempted_at: "2026-04-08T06:00:00Z".to_owned(),
                last_completed_at: Some("2026-04-08T06:00:05Z".to_owned()),
                message: Some("optional endpoint degraded".to_owned()),
                granted_scopes: vec!["daily".to_owned()],
                last_error: None,
                failure_count: 0,
                next_attempt_after: None,
                last_trigger_source: Some("periodic_reconcile".to_owned()),
                last_trigger_detail: Some("test overlap reuse".to_owned()),
            }),
            "partial sync state should persist",
        );

        let start_day = ok(
            super::overlap_day_window(&store, "oura.daily", 30, 2),
            "window should build",
        );

        assert_eq!(start_day, "2026-04-06");
    }
}
