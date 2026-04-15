use std::path::PathBuf;

use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::config::Config;
use crate::derive;
use crate::error::{AuthError, OuraApiError, OuraProblem, Result, RingmasterError};
use crate::oura::auth;
use crate::oura::client::{FixtureOuraClient, OuraClient, PageFetch, ReqwestOuraClient};
use crate::oura::models::{
    CapabilityKind, CapabilityReport, DailyActivityDocument, DailyCardiovascularAgeDocument,
    DailyReadinessDocument, DailyResilienceDocument, DailySleepDocument, DailySpO2Document,
    DailyStressDocument, RestModePeriodDocument, SleepDocument, SleepTimeDocument, Vo2MaxDocument,
    WorkoutDocument,
};
use crate::oura::policy::SyncPolicy;
use crate::refresh::SyncFamily;
use crate::store::queries::{
    AuthSessionRecord, DailyActivityRecord, DailyCardiovascularAgeRecord, DailyReadinessRecord,
    DailyResilienceRecord, DailySleepRecord, DailySpO2Record, DailyStressRecord, EnhancedTagRecord,
    HeartrateSampleRecord, OURA_PROVIDER, PersonalInfoRecord, RestModePeriodRecord, SessionRecord,
    SleepPeriodRecord, SleepTimeRecord, SyncRunStatus, SyncStateRecord, Vo2MaxRecord,
    WorkoutRecord,
};
use crate::store::{Store, StorePlan};

const PERSONAL_SYNC_KEY: &str = "oura.personal";
const DAILY_SYNC_KEY: &str = "oura.daily";
const SPO2_SYNC_KEY: &str = "oura.spo2";
const HEARTRATE_SYNC_KEY: &str = "oura.heartrate";
const WORKOUT_SYNC_KEY: &str = "oura.workouts";
const ENHANCED_TAG_SYNC_KEY: &str = "oura.enhanced_tags";
const SESSION_SYNC_KEY: &str = "oura.sessions";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SyncMode {
    #[default]
    Standard,
    Reconcile {
        days: u16,
    },
    Backfill {
        days: u16,
        chunk_days: Option<u16>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncOptions {
    pub dry_run: bool,
    pub fixture_dir: Option<PathBuf>,
    pub families: Vec<SyncFamily>,
    pub mode: SyncMode,
    pub trigger_source: Option<String>,
    pub trigger_detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceReport {
    pub sync_key: String,
    pub family: SyncFamily,
    pub status: SyncRunStatus,
    pub imported_rows: usize,
    pub watermark: Option<String>,
    pub last_successful_sync_end: Option<String>,
    pub last_reconcile_end: Option<String>,
    pub oldest_recently_reconciled_at: Option<String>,
    pub message: String,
    pub last_error: Option<OuraProblem>,
    pub next_attempt_after: Option<String>,
}

impl Default for SliceReport {
    fn default() -> Self {
        Self {
            sync_key: DAILY_SYNC_KEY.to_owned(),
            family: SyncFamily::Daily,
            status: SyncRunStatus::Ready,
            imported_rows: 0,
            watermark: None,
            last_successful_sync_end: None,
            last_reconcile_end: None,
            oldest_recently_reconciled_at: None,
            message: String::new(),
            last_error: None,
            next_attempt_after: None,
        }
    }
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
            mode: options.mode,
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
            let client = FixtureOuraClient::new(config, fixture_dir.clone());
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
        SyncFamily::Spo2 => sync_spo2(config, store_plan, client, capability_report, options).await,
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
    if let Some(report) = guard_family_scope(
        config,
        &store_plan,
        capability_report,
        FamilyScopeGuard {
            capability: CapabilityKind::Personal,
            family: SyncFamily::Personal,
            not_requested_message: "`personal` scope is not requested; skipping profile sync.",
            missing_scope_message: "Missing `personal` scope; profile data remains unavailable.",
        },
        options,
    )? {
        return Ok(report);
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
        new_slice_report(
            SyncFamily::Personal,
            SyncRunStatus::Success,
            1,
            Some(imported_at),
            format!(
                "Imported personal info for profile {}.",
                fetched.document.id
            ),
            None,
        ),
        granted_scopes_from_report(capability_report),
        options,
    )
}

fn reopen_store(config: &Config, store_plan: &StorePlan) -> Result<Store> {
    Store::open_with_plan(store_plan.clone(), config.app_name)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DailySyncWindow {
    start_date: String,
    end_date: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncWindowPurpose {
    Tail,
    Reconcile,
    Backfill,
}

impl SyncWindowPurpose {
    const fn label(self) -> &'static str {
        match self {
            Self::Tail => "tail",
            Self::Reconcile => "reconcile",
            Self::Backfill => "backfill",
        }
    }

    const fn records_reconcile_coverage(self) -> bool {
        matches!(self, Self::Reconcile | Self::Backfill)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlannedDailyWindow {
    purpose: SyncWindowPurpose,
    window: DailySyncWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlannedHeartrateWindow {
    purpose: SyncWindowPurpose,
    start: OffsetDateTime,
    end: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FamilySyncSummary {
    family: SyncFamily,
    imported_rows: usize,
    last_successful_sync_end: Option<String>,
    last_reconcile_end: Option<String>,
    oldest_recently_reconciled_at: Option<String>,
    messages: Vec<String>,
    last_error: Option<OuraProblem>,
    statuses: Vec<SyncRunStatus>,
}

impl FamilySyncSummary {
    const fn new(family: SyncFamily) -> Self {
        Self {
            family,
            imported_rows: 0,
            last_successful_sync_end: None,
            last_reconcile_end: None,
            oldest_recently_reconciled_at: None,
            messages: Vec::new(),
            last_error: None,
            statuses: Vec::new(),
        }
    }

    fn observe(&mut self, report: SliceReport) {
        self.imported_rows = self.imported_rows.saturating_add(report.imported_rows);
        self.messages.push(report.message);
        self.statuses.push(report.status.clone());

        if matches!(
            report.status,
            SyncRunStatus::Success | SyncRunStatus::Partial
        ) {
            self.last_successful_sync_end = prefer_later_marker(
                report.last_successful_sync_end,
                self.last_successful_sync_end.clone(),
            );
            self.last_reconcile_end =
                prefer_later_marker(report.last_reconcile_end, self.last_reconcile_end.clone());
            self.oldest_recently_reconciled_at = prefer_earlier_marker(
                report.oldest_recently_reconciled_at,
                self.oldest_recently_reconciled_at.clone(),
            );
        }

        if report.last_error.is_some() {
            self.last_error = report.last_error;
        }
    }

    fn finish(self) -> SliceReport {
        let status = summarize_family_status(&self.statuses);
        let message = self.messages.join(" ");
        SliceReport {
            sync_key: self.family.sync_key().to_owned(),
            family: self.family,
            status,
            imported_rows: self.imported_rows,
            watermark: self.last_successful_sync_end.clone(),
            last_successful_sync_end: self.last_successful_sync_end,
            last_reconcile_end: self.last_reconcile_end,
            oldest_recently_reconciled_at: self.oldest_recently_reconciled_at,
            message,
            last_error: self.last_error,
            next_attempt_after: None,
        }
    }
}

struct DailyPageFetches {
    daily_sleep_pages: Vec<PageFetch<DailySleepDocument>>,
    sleep_period_pages: Vec<PageFetch<SleepDocument>>,
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

fn summarize_family_status(statuses: &[SyncRunStatus]) -> SyncRunStatus {
    if statuses.is_empty() {
        return SyncRunStatus::Blocked;
    }
    if statuses
        .iter()
        .all(|status| *status == SyncRunStatus::Success)
    {
        return SyncRunStatus::Success;
    }
    if statuses
        .iter()
        .all(|status| *status == SyncRunStatus::Blocked)
    {
        return SyncRunStatus::Blocked;
    }
    if statuses.contains(&SyncRunStatus::Failed) {
        if statuses
            .iter()
            .any(|status| matches!(status, SyncRunStatus::Success | SyncRunStatus::Partial))
        {
            return SyncRunStatus::Partial;
        }
        return SyncRunStatus::Failed;
    }
    SyncRunStatus::Partial
}

fn recent_day_start(days: i64, now: OffsetDateTime) -> time::Date {
    now.date() - Duration::days(days.max(1).saturating_sub(1))
}

fn configured_daily_history_start(
    config: &Config,
    family: SyncFamily,
    now: OffsetDateTime,
) -> time::Date {
    let history_days = match family {
        SyncFamily::Daily | SyncFamily::Spo2 => config.refresh.daily_history_days,
        SyncFamily::Workout => config.refresh.workout_history_days,
        SyncFamily::EnhancedTag => config.refresh.enhanced_tag_history_days,
        SyncFamily::Session => config.refresh.session_history_days,
        SyncFamily::Personal | SyncFamily::Heartrate => {
            unreachable!("daily history planning only applies to day-bounded sync families")
        }
    };
    recent_day_start(i64::from(history_days), now)
}

fn configured_heartrate_history_start(config: &Config, now: OffsetDateTime) -> OffsetDateTime {
    now - Duration::days(i64::from(config.refresh.heartrate_history_days))
}

fn parse_date_marker(marker: Option<&str>) -> Result<Option<time::Date>> {
    marker
        .map(|value| {
            time::Date::parse(
                value,
                &time::macros::format_description!("[year]-[month]-[day]"),
            )
            .map_err(|error| {
                AuthError::OAuthFlow(format!("invalid stored day watermark `{value}`: {error}"))
                    .into()
            })
        })
        .transpose()
}

fn parse_timestamp_marker(marker: Option<&str>) -> Result<Option<OffsetDateTime>> {
    marker.map(parse_timestamp_value).transpose()
}

fn parse_timestamp_value(value: &str) -> Result<OffsetDateTime> {
    if let Ok(timestamp) = OffsetDateTime::parse(value, &Rfc3339) {
        return Ok(timestamp);
    }

    if let Ok(date) = time::Date::parse(
        value,
        &time::macros::format_description!("[year]-[month]-[day]"),
    ) {
        return Ok(date.midnight().assume_utc());
    }

    Err(AuthError::OAuthFlow(format!(
        "invalid stored sync marker `{value}`: expected RFC3339 or YYYY-MM-DD"
    ))
    .into())
}

fn format_timestamp_marker(timestamp: OffsetDateTime) -> Result<String> {
    timestamp.format(&Rfc3339).map_err(|error| {
        AuthError::OAuthFlow(format!("failed to format sync timestamp: {error}")).into()
    })
}

fn chunked_daily_windows(
    start: time::Date,
    end: time::Date,
    chunk_days: i64,
    purpose: SyncWindowPurpose,
) -> Vec<PlannedDailyWindow> {
    let mut windows = Vec::new();
    let mut chunk_start = start;
    let chunk_days = chunk_days.max(1);

    while chunk_start <= end {
        let chunk_end = std::cmp::min(chunk_start + Duration::days(chunk_days - 1), end);
        windows.push(PlannedDailyWindow {
            purpose,
            window: DailySyncWindow {
                start_date: chunk_start.to_string(),
                end_date: chunk_end.to_string(),
            },
        });
        chunk_start = chunk_end + Duration::days(1);
    }

    windows
}

fn chunked_heartrate_windows(
    start: OffsetDateTime,
    end: OffsetDateTime,
    chunk_days: i64,
    purpose: SyncWindowPurpose,
) -> Vec<PlannedHeartrateWindow> {
    let mut windows = Vec::new();
    let mut chunk_start = start;
    let chunk_duration = Duration::days(chunk_days.max(1));

    while chunk_start < end {
        let chunk_end = std::cmp::min(chunk_start + chunk_duration, end);
        windows.push(PlannedHeartrateWindow {
            purpose,
            start: chunk_start,
            end: chunk_end,
        });
        chunk_start = chunk_end;
    }

    if windows.is_empty() {
        windows.push(PlannedHeartrateWindow {
            purpose,
            start,
            end,
        });
    }

    windows
}

fn observe_failed_chunk(
    summary: &mut FamilySyncSummary,
    family: SyncFamily,
    purpose: SyncWindowPurpose,
    error: &RingmasterError,
) {
    summary.observe(SliceReport {
        sync_key: family.sync_key().to_owned(),
        family,
        status: SyncRunStatus::Failed,
        imported_rows: 0,
        watermark: None,
        last_successful_sync_end: None,
        last_reconcile_end: None,
        oldest_recently_reconciled_at: None,
        message: format!("{} window: {error}", purpose.label()),
        last_error: Some(error_problem(error)),
        next_attempt_after: None,
    });
}

fn should_run_reconcile(
    policy: &SyncPolicy,
    sync_state: Option<&SyncStateRecord>,
    recent_start: &str,
    now_marker: &str,
) -> Result<bool> {
    let Some(sync_state) = sync_state else {
        return Ok(true);
    };
    let Some(last_reconcile_end) =
        parse_timestamp_marker(sync_state.last_reconcile_end.as_deref())?
    else {
        return Ok(true);
    };
    let Some(now_timestamp) = parse_timestamp_marker(Some(now_marker))? else {
        return Ok(true);
    };
    if now_timestamp - last_reconcile_end >= policy.overlap {
        return Ok(true);
    }
    let oldest_recent = sync_state
        .oldest_recently_reconciled_at
        .as_deref()
        .unwrap_or_default();
    Ok(oldest_recent.is_empty() || oldest_recent > recent_start)
}

fn plan_daily_windows(
    config: &Config,
    store_plan: &StorePlan,
    family: SyncFamily,
    options: &SyncOptions,
    policy: &SyncPolicy,
) -> Result<Vec<PlannedDailyWindow>> {
    let now = OffsetDateTime::now_utc();
    if options.fixture_dir.is_some() {
        return Ok(vec![PlannedDailyWindow {
            purpose: SyncWindowPurpose::Tail,
            window: DailySyncWindow {
                start_date: "1970-01-01".to_owned(),
                end_date: now.date().to_string(),
            },
        }]);
    }

    let store = reopen_store(config, store_plan)?;
    let sync_state = store.sync_state().get(family.sync_key())?;
    let today = now.date();
    let history_start = configured_daily_history_start(config, family, now);
    let startup_start = recent_day_start(policy.startup_catchup_days(), now);

    match options.mode {
        SyncMode::Standard => {
            let success_marker = sync_state
                .as_ref()
                .and_then(|state| state.last_successful_sync_end.as_deref())
                .or_else(|| {
                    sync_state
                        .as_ref()
                        .and_then(|state| state.cursor.as_deref())
                });
            let tail_start = parse_date_marker(success_marker)?.map_or(history_start, |cursor| {
                (cursor - policy.overlap).max(startup_start)
            });
            let mut windows = chunked_daily_windows(
                tail_start,
                today,
                policy.backfill_chunk_days(),
                SyncWindowPurpose::Tail,
            );
            let reconcile_start = recent_day_start(policy.reconcile_days(), now);
            let tail_covers_reconcile = tail_start <= reconcile_start;
            let reconcile_start_marker = reconcile_start.to_string();
            let now_marker = format_timestamp_marker(now)?;
            if !tail_covers_reconcile
                && should_run_reconcile(
                    policy,
                    sync_state.as_ref(),
                    &reconcile_start_marker,
                    &now_marker,
                )?
            {
                windows.extend(chunked_daily_windows(
                    reconcile_start,
                    today,
                    policy.backfill_chunk_days(),
                    SyncWindowPurpose::Reconcile,
                ));
            }
            Ok(windows)
        }
        SyncMode::Reconcile { days } => Ok(chunked_daily_windows(
            recent_day_start(i64::from(days), now),
            today,
            policy.backfill_chunk_days(),
            SyncWindowPurpose::Reconcile,
        )),
        SyncMode::Backfill { days, chunk_days } => Ok(chunked_daily_windows(
            recent_day_start(i64::from(days), now),
            today,
            chunk_days.map_or_else(|| policy.backfill_chunk_days(), i64::from),
            SyncWindowPurpose::Backfill,
        )),
    }
}

fn plan_heartrate_windows(
    config: &Config,
    store_plan: &StorePlan,
    options: &SyncOptions,
    policy: &SyncPolicy,
) -> Result<Vec<PlannedHeartrateWindow>> {
    let now = OffsetDateTime::now_utc();
    if options.fixture_dir.is_some() {
        return Ok(vec![PlannedHeartrateWindow {
            purpose: SyncWindowPurpose::Tail,
            start: OffsetDateTime::UNIX_EPOCH,
            end: now,
        }]);
    }

    let store = reopen_store(config, store_plan)?;
    let sync_state = store.sync_state().get(HEARTRATE_SYNC_KEY)?;
    let history_start = configured_heartrate_history_start(config, now);
    let startup_start = now - Duration::days(policy.startup_catchup_days());

    match options.mode {
        SyncMode::Standard => {
            let success_marker = sync_state
                .as_ref()
                .and_then(|state| state.last_successful_sync_end.as_deref())
                .or_else(|| {
                    sync_state
                        .as_ref()
                        .and_then(|state| state.cursor.as_deref())
                });
            let tail_start = parse_timestamp_marker(success_marker)?
                .map_or(history_start, |cursor| {
                    (cursor - policy.overlap).max(startup_start)
                });
            let mut windows = chunked_heartrate_windows(
                tail_start,
                now,
                policy.backfill_chunk_days(),
                SyncWindowPurpose::Tail,
            );
            let reconcile_start = now - Duration::days(policy.reconcile_days());
            let tail_covers_reconcile = tail_start <= reconcile_start;
            let reconcile_start_marker = reconcile_start.date().to_string();
            let now_marker = format_timestamp_marker(now)?;
            if !tail_covers_reconcile
                && should_run_reconcile(
                    policy,
                    sync_state.as_ref(),
                    &reconcile_start_marker,
                    &now_marker,
                )?
            {
                windows.extend(chunked_heartrate_windows(
                    reconcile_start,
                    now,
                    policy.backfill_chunk_days(),
                    SyncWindowPurpose::Reconcile,
                ));
            }
            Ok(windows)
        }
        SyncMode::Reconcile { days } => Ok(chunked_heartrate_windows(
            now - Duration::days(i64::from(days)),
            now,
            policy.backfill_chunk_days(),
            SyncWindowPurpose::Reconcile,
        )),
        SyncMode::Backfill { days, chunk_days } => Ok(chunked_heartrate_windows(
            now - Duration::days(i64::from(days)),
            now,
            chunk_days.map_or_else(|| policy.backfill_chunk_days(), i64::from),
            SyncWindowPurpose::Backfill,
        )),
    }
}

const fn retryable_problem(problem: &OuraProblem) -> bool {
    matches!(problem.status, Some(429 | 500..=599))
        || (problem.status.is_none() && problem.oauth_error.is_none())
}

async fn fetch_with_retry<T, F, Fut>(config: &Config, mut fetch: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut attempt = 0_u32;
    let max_attempts = config.refresh.sync_retry_max_attempts.max(1);

    loop {
        attempt = attempt.saturating_add(1);
        match fetch().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                let problem = error_problem(&error);
                if attempt >= max_attempts || !retryable_problem(&problem) {
                    return Err(error);
                }
                let backoff_multiplier = 1_u64 << attempt.saturating_sub(1).min(6);
                let backoff_secs = config
                    .refresh
                    .sync_retry_base_backoff_secs
                    .max(1)
                    .saturating_mul(backoff_multiplier)
                    .min(config.refresh.max_backoff_secs.max(1));
                tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
            }
        }
    }
}

async fn sync_daily(
    config: &Config,
    store_plan: StorePlan,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if let Some(report) = guard_family_scope(
        config,
        &store_plan,
        capability_report,
        FamilyScopeGuard {
            capability: CapabilityKind::Daily,
            family: SyncFamily::Daily,
            not_requested_message: "`daily` scope is not requested; skipping daily summary sync.",
            missing_scope_message: "Missing `daily` scope; dashboard summary rows remain unavailable.",
        },
        options,
    )? {
        return Ok(report);
    }
    let policy = SyncPolicy::for_family(&config.refresh, SyncFamily::Daily);
    let windows = plan_daily_windows(config, &store_plan, SyncFamily::Daily, options, &policy)?;
    let mut summary = FamilySyncSummary::new(SyncFamily::Daily);

    for planned in windows {
        match execute_daily_window(
            config,
            &store_plan,
            client,
            capability_report,
            &planned.window,
            options,
        )
        .await
        {
            Ok((status, message, last_error, imported_rows)) => summary.observe(SliceReport {
                sync_key: SyncFamily::Daily.sync_key().to_owned(),
                family: SyncFamily::Daily,
                status,
                imported_rows,
                watermark: Some(planned.window.end_date.clone()),
                last_successful_sync_end: Some(planned.window.end_date.clone()),
                last_reconcile_end: planned
                    .purpose
                    .records_reconcile_coverage()
                    .then(|| planned.window.end_date.clone()),
                oldest_recently_reconciled_at: planned
                    .purpose
                    .records_reconcile_coverage()
                    .then(|| planned.window.start_date.clone()),
                message: format!("{} window: {message}", planned.purpose.label()),
                last_error,
                next_attempt_after: None,
            }),
            Err(error) => {
                observe_failed_chunk(&mut summary, SyncFamily::Daily, planned.purpose, &error);
                break;
            }
        }
    }

    let persist_store = reopen_store(config, &store_plan)?;
    persist_slice_report(
        config,
        &persist_store,
        summary.finish(),
        granted_scopes_from_report(capability_report),
        options,
    )
}

async fn execute_daily_window(
    config: &Config,
    store_plan: &StorePlan,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    window: &DailySyncWindow,
    options: &SyncOptions,
) -> Result<(SyncRunStatus, String, Option<OuraProblem>, usize)> {
    let pages = fetch_with_retry(config, || {
        fetch_daily_pages(client, capability_report, window)
    })
    .await?;
    let imported_at = now_rfc3339()?;
    let persist_store = reopen_store(config, store_plan)?;
    persist_daily_pages(&persist_store, window, &pages, &imported_at, options)?;
    Ok(summarize_daily_sync(window, &pages))
}

async fn sync_spo2(
    config: &Config,
    store_plan: StorePlan,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if let Some(report) = guard_family_scope(
        config,
        &store_plan,
        capability_report,
        FamilyScopeGuard {
            capability: CapabilityKind::Daily,
            family: SyncFamily::Spo2,
            not_requested_message: "`daily` scope is not requested; skipping blood oxygen sync because SpO2 depends on daily coverage.",
            missing_scope_message: "Missing `daily` scope; blood oxygen sync requires daily coverage.",
        },
        options,
    )? {
        return Ok(report);
    }
    if let Some(report) = guard_family_scope(
        config,
        &store_plan,
        capability_report,
        FamilyScopeGuard {
            capability: CapabilityKind::Spo2,
            family: SyncFamily::Spo2,
            not_requested_message: "`spo2` scope is not requested; skipping blood oxygen sync.",
            missing_scope_message: "Missing `spo2` scope; blood oxygen coverage remains unavailable.",
        },
        options,
    )? {
        return Ok(report);
    }
    let policy = SyncPolicy::for_family(&config.refresh, SyncFamily::Spo2);
    let windows = plan_daily_windows(config, &store_plan, SyncFamily::Spo2, options, &policy)?;
    let mut summary = FamilySyncSummary::new(SyncFamily::Spo2);

    for planned in windows {
        match execute_spo2_window(config, &store_plan, client, &planned.window, options).await {
            Ok((message, imported_rows)) => summary.observe(SliceReport {
                sync_key: SyncFamily::Spo2.sync_key().to_owned(),
                family: SyncFamily::Spo2,
                status: SyncRunStatus::Success,
                imported_rows,
                watermark: Some(planned.window.end_date.clone()),
                last_successful_sync_end: Some(planned.window.end_date.clone()),
                last_reconcile_end: planned
                    .purpose
                    .records_reconcile_coverage()
                    .then(|| planned.window.end_date.clone()),
                oldest_recently_reconciled_at: planned
                    .purpose
                    .records_reconcile_coverage()
                    .then(|| planned.window.start_date.clone()),
                message: format!("{} window: {message}", planned.purpose.label()),
                last_error: None,
                next_attempt_after: None,
            }),
            Err(error) => {
                observe_failed_chunk(&mut summary, SyncFamily::Spo2, planned.purpose, &error);
                break;
            }
        }
    }

    let persist_store = reopen_store(config, &store_plan)?;
    persist_slice_report(
        config,
        &persist_store,
        summary.finish(),
        granted_scopes_from_report(capability_report),
        options,
    )
}

async fn execute_spo2_window(
    config: &Config,
    store_plan: &StorePlan,
    client: &dyn OuraClient,
    window: &DailySyncWindow,
    options: &SyncOptions,
) -> Result<(String, usize)> {
    let pages = fetch_with_retry(config, || {
        client.fetch_daily_spo2(window.start_date.clone(), window.end_date.clone())
    })
    .await?;
    let imported_at = now_rfc3339()?;
    let persist_store = reopen_store(config, store_plan)?;
    if !options.dry_run {
        persist_daily_spo2_pages(&persist_store, &pages, &imported_at)?;
    }
    let imported_rows = count_documents(&pages);
    Ok((
        format!(
            "Imported {imported_rows} SpO2 rows from {} through {}.",
            window.start_date, window.end_date
        ),
        imported_rows,
    ))
}

async fn fetch_daily_pages(
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    window: &DailySyncWindow,
) -> Result<DailyPageFetches> {
    let (
        daily_sleep_pages_result,
        sleep_period_pages_result,
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
        client.fetch_sleep(window.start_date.clone(), window.end_date.clone()),
        client.fetch_daily_readiness(window.start_date.clone(), window.end_date.clone()),
        client.fetch_daily_activity(window.start_date.clone(), window.end_date.clone()),
        async {
            if capability_report.is_granted(CapabilityKind::Stress) {
                client
                    .fetch_sleep_time(window.start_date.clone(), window.end_date.clone())
                    .await
            } else {
                Ok(Vec::new())
            }
        },
        async {
            if capability_report.is_granted(CapabilityKind::Stress) {
                client
                    .fetch_rest_mode_periods(window.start_date.clone(), window.end_date.clone())
                    .await
            } else {
                Ok(Vec::new())
            }
        },
        async {
            if capability_report.is_granted(CapabilityKind::Stress) {
                client
                    .fetch_daily_stress(window.start_date.clone(), window.end_date.clone())
                    .await
            } else {
                Ok(Vec::new())
            }
        },
        async {
            if capability_report.is_granted(CapabilityKind::HeartHealth) {
                client
                    .fetch_daily_resilience(window.start_date.clone(), window.end_date.clone())
                    .await
            } else {
                Ok(Vec::new())
            }
        },
        async {
            if capability_report.is_granted(CapabilityKind::HeartHealth) {
                client
                    .fetch_daily_cardiovascular_age(
                        window.start_date.clone(),
                        window.end_date.clone(),
                    )
                    .await
            } else {
                Ok(Vec::new())
            }
        },
        async {
            if capability_report.is_granted(CapabilityKind::HeartHealth) {
                client
                    .fetch_vo2_max(window.start_date.clone(), window.end_date.clone())
                    .await
            } else {
                Ok(Vec::new())
            }
        },
    );
    let mut optional_failures = Vec::new();

    Ok(DailyPageFetches {
        daily_sleep_pages: daily_sleep_pages_result?,
        sleep_period_pages: sleep_period_pages_result?,
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
    window: &DailySyncWindow,
    pages: &DailyPageFetches,
    imported_at: &str,
    options: &SyncOptions,
) -> Result<()> {
    if options.dry_run {
        return Ok(());
    }

    persist_daily_sleep_pages(persist_store, &pages.daily_sleep_pages, imported_at)?;
    persist_sleep_period_pages(
        persist_store,
        window,
        &pages.sleep_period_pages,
        imported_at,
    )?;
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

fn persist_sleep_period_pages(
    persist_store: &Store,
    window: &DailySyncWindow,
    pages: &[PageFetch<SleepDocument>],
    imported_at: &str,
) -> Result<()> {
    persist_store
        .imports()
        .delete_sleep_periods_between_days(&window.start_date, &window.end_date)?;

    for page in pages {
        persist_store
            .imports()
            .upsert_raw_payload(&page.raw_payload)?;
        for document in &page.documents {
            persist_store
                .imports()
                .upsert_sleep_period(&SleepPeriodRecord {
                    oura_id: document.id.clone(),
                    day: document.day.clone(),
                    bedtime_start: document.bedtime_start.clone(),
                    bedtime_end: document.bedtime_end.clone(),
                    sleep_type: document.sleep_type.clone(),
                    average_heart_rate: document.average_heart_rate,
                    average_hrv: document.average_hrv,
                    average_breath: document.average_breath,
                    total_sleep_duration: document.total_sleep_duration,
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

fn persist_daily_spo2_pages(
    persist_store: &Store,
    pages: &[PageFetch<DailySpO2Document>],
    imported_at: &str,
) -> Result<()> {
    for page in pages {
        persist_store
            .imports()
            .upsert_raw_payload(&page.raw_payload)?;
        for document in &page.documents {
            persist_store
                .imports()
                .upsert_daily_spo2(&DailySpO2Record {
                    oura_id: Some(document.id.clone()),
                    day: document.day.clone(),
                    average_spo2: document
                        .spo2_percentage
                        .as_ref()
                        .and_then(|value| value.average),
                    breathing_disturbance_index: document.breathing_disturbance_index,
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
    let imported_rows = count_documents(&pages.daily_sleep_pages)
        + count_documents(&pages.sleep_period_pages)
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
                "Imported {imported_rows} daily summary, physiology, and review-support rows from {} through {}.",
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
                "Imported {imported_rows} core daily and physiology rows from {} through {}; optional review-support endpoints degraded independently: {failure_summary}.",
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
    if let Some(report) = guard_family_scope(
        config,
        &store_plan,
        capability_report,
        FamilyScopeGuard {
            capability: CapabilityKind::Heartrate,
            family: SyncFamily::Heartrate,
            not_requested_message: "`heartrate` scope is not requested; skipping heartrate sync.",
            missing_scope_message: "Missing `heartrate` scope; timeline and trends remain stale.",
        },
        options,
    )? {
        return Ok(report);
    }
    let policy = SyncPolicy::for_family(&config.refresh, SyncFamily::Heartrate);
    let windows = plan_heartrate_windows(config, &store_plan, options, &policy)?;
    let mut summary = FamilySyncSummary::new(SyncFamily::Heartrate);

    for planned in windows {
        match execute_heartrate_window(config, &store_plan, client, &planned, options).await {
            Ok((message, imported_rows)) => {
                let end_marker = format_timestamp_marker(planned.end)?;
                summary.observe(SliceReport {
                    sync_key: SyncFamily::Heartrate.sync_key().to_owned(),
                    family: SyncFamily::Heartrate,
                    status: SyncRunStatus::Success,
                    imported_rows,
                    watermark: Some(end_marker.clone()),
                    last_successful_sync_end: Some(end_marker.clone()),
                    last_reconcile_end: planned
                        .purpose
                        .records_reconcile_coverage()
                        .then_some(end_marker),
                    oldest_recently_reconciled_at: planned
                        .purpose
                        .records_reconcile_coverage()
                        .then(|| planned.start.date().to_string()),
                    message: format!("{} window: {message}", planned.purpose.label()),
                    last_error: None,
                    next_attempt_after: None,
                });
            }
            Err(error) => {
                observe_failed_chunk(&mut summary, SyncFamily::Heartrate, planned.purpose, &error);
                break;
            }
        }
    }

    let persist_store = reopen_store(config, &store_plan)?;
    persist_slice_report(
        config,
        &persist_store,
        summary.finish(),
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
    if let Some(report) = guard_family_scope(
        config,
        &store_plan,
        capability_report,
        FamilyScopeGuard {
            capability: CapabilityKind::Workout,
            family: SyncFamily::Workout,
            not_requested_message: "`workout` scope is not requested; skipping workout sync.",
            missing_scope_message: "Missing `workout` scope; workout overlays and context evidence remain unavailable.",
        },
        options,
    )? {
        return Ok(report);
    }
    let policy = SyncPolicy::for_family(&config.refresh, SyncFamily::Workout);
    let windows = plan_daily_windows(config, &store_plan, SyncFamily::Workout, options, &policy)?;
    let mut summary = FamilySyncSummary::new(SyncFamily::Workout);

    for planned in windows {
        match execute_workout_window(config, &store_plan, client, &planned.window, options).await {
            Ok((message, imported_rows)) => summary.observe(SliceReport {
                sync_key: SyncFamily::Workout.sync_key().to_owned(),
                family: SyncFamily::Workout,
                status: SyncRunStatus::Success,
                imported_rows,
                watermark: Some(planned.window.end_date.clone()),
                last_successful_sync_end: Some(planned.window.end_date.clone()),
                last_reconcile_end: planned
                    .purpose
                    .records_reconcile_coverage()
                    .then(|| planned.window.end_date.clone()),
                oldest_recently_reconciled_at: planned
                    .purpose
                    .records_reconcile_coverage()
                    .then(|| planned.window.start_date.clone()),
                message: format!("{} window: {message}", planned.purpose.label()),
                last_error: None,
                next_attempt_after: None,
            }),
            Err(error) => {
                observe_failed_chunk(&mut summary, SyncFamily::Workout, planned.purpose, &error);
                break;
            }
        }
    }

    let persist_store = reopen_store(config, &store_plan)?;
    persist_slice_report(
        config,
        &persist_store,
        summary.finish(),
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
    if let Some(report) = guard_family_scope(
        config,
        &store_plan,
        capability_report,
        FamilyScopeGuard {
            capability: CapabilityKind::EnhancedTag,
            family: SyncFamily::EnhancedTag,
            not_requested_message: "`tag` scope is not requested; skipping tag sync.",
            missing_scope_message: "Missing `tag` scope; tag overlays and explainability evidence remain unavailable.",
        },
        options,
    )? {
        return Ok(report);
    }
    let policy = SyncPolicy::for_family(&config.refresh, SyncFamily::EnhancedTag);
    let windows = plan_daily_windows(
        config,
        &store_plan,
        SyncFamily::EnhancedTag,
        options,
        &policy,
    )?;
    let mut summary = FamilySyncSummary::new(SyncFamily::EnhancedTag);

    for planned in windows {
        match execute_enhanced_tag_window(config, &store_plan, client, &planned.window, options)
            .await
        {
            Ok((message, imported_rows)) => summary.observe(SliceReport {
                sync_key: SyncFamily::EnhancedTag.sync_key().to_owned(),
                family: SyncFamily::EnhancedTag,
                status: SyncRunStatus::Success,
                imported_rows,
                watermark: Some(planned.window.end_date.clone()),
                last_successful_sync_end: Some(planned.window.end_date.clone()),
                last_reconcile_end: planned
                    .purpose
                    .records_reconcile_coverage()
                    .then(|| planned.window.end_date.clone()),
                oldest_recently_reconciled_at: planned
                    .purpose
                    .records_reconcile_coverage()
                    .then(|| planned.window.start_date.clone()),
                message: format!("{} window: {message}", planned.purpose.label()),
                last_error: None,
                next_attempt_after: None,
            }),
            Err(error) => {
                observe_failed_chunk(
                    &mut summary,
                    SyncFamily::EnhancedTag,
                    planned.purpose,
                    &error,
                );
                break;
            }
        }
    }

    let persist_store = reopen_store(config, &store_plan)?;
    persist_slice_report(
        config,
        &persist_store,
        summary.finish(),
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
    if let Some(report) = guard_family_scope(
        config,
        &store_plan,
        capability_report,
        FamilyScopeGuard {
            capability: CapabilityKind::Session,
            family: SyncFamily::Session,
            not_requested_message: "`session` scope is not requested; skipping session sync.",
            missing_scope_message: "Missing `session` scope; session overlays and explainability evidence remain unavailable.",
        },
        options,
    )? {
        return Ok(report);
    }
    let policy = SyncPolicy::for_family(&config.refresh, SyncFamily::Session);
    let windows = plan_daily_windows(config, &store_plan, SyncFamily::Session, options, &policy)?;
    let mut summary = FamilySyncSummary::new(SyncFamily::Session);

    for planned in windows {
        match execute_session_window(config, &store_plan, client, &planned.window, options).await {
            Ok((message, imported_rows)) => summary.observe(SliceReport {
                sync_key: SyncFamily::Session.sync_key().to_owned(),
                family: SyncFamily::Session,
                status: SyncRunStatus::Success,
                imported_rows,
                watermark: Some(planned.window.end_date.clone()),
                last_successful_sync_end: Some(planned.window.end_date.clone()),
                last_reconcile_end: planned
                    .purpose
                    .records_reconcile_coverage()
                    .then(|| planned.window.end_date.clone()),
                oldest_recently_reconciled_at: planned
                    .purpose
                    .records_reconcile_coverage()
                    .then(|| planned.window.start_date.clone()),
                message: format!("{} window: {message}", planned.purpose.label()),
                last_error: None,
                next_attempt_after: None,
            }),
            Err(error) => {
                observe_failed_chunk(&mut summary, SyncFamily::Session, planned.purpose, &error);
                break;
            }
        }
    }

    let persist_store = reopen_store(config, &store_plan)?;
    persist_slice_report(
        config,
        &persist_store,
        summary.finish(),
        granted_scopes_from_report(capability_report),
        options,
    )
}

async fn execute_heartrate_window(
    config: &Config,
    store_plan: &StorePlan,
    client: &dyn OuraClient,
    planned: &PlannedHeartrateWindow,
    options: &SyncOptions,
) -> Result<(String, usize)> {
    let start_datetime = format_timestamp_marker(planned.start)?;
    let end_datetime = format_timestamp_marker(planned.end)?;
    let heartrate_pages = fetch_with_retry(config, || {
        client.fetch_heartrate(start_datetime.clone(), end_datetime.clone())
    })
    .await?;
    let imported_at = now_rfc3339()?;
    let persist_store = reopen_store(config, store_plan)?;

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
    Ok((
        format!(
            "Imported {imported_rows} heartrate samples from {start_datetime} through {end_datetime}."
        ),
        imported_rows,
    ))
}

async fn execute_workout_window(
    config: &Config,
    store_plan: &StorePlan,
    client: &dyn OuraClient,
    window: &DailySyncWindow,
    options: &SyncOptions,
) -> Result<(String, usize)> {
    let workout_pages = fetch_with_retry(config, || {
        client.fetch_workouts(window.start_date.clone(), window.end_date.clone())
    })
    .await?;
    let imported_at = now_rfc3339()?;
    let persist_store = reopen_store(config, store_plan)?;

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
    Ok((
        format!(
            "Imported {imported_rows} workouts from {} through {}.",
            window.start_date, window.end_date
        ),
        imported_rows,
    ))
}

async fn execute_enhanced_tag_window(
    config: &Config,
    store_plan: &StorePlan,
    client: &dyn OuraClient,
    window: &DailySyncWindow,
    options: &SyncOptions,
) -> Result<(String, usize)> {
    let enhanced_tag_pages = fetch_with_retry(config, || {
        client.fetch_enhanced_tags(window.start_date.clone(), window.end_date.clone())
    })
    .await?;
    let imported_at = now_rfc3339()?;
    let persist_store = reopen_store(config, store_plan)?;

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
    Ok((
        format!(
            "Imported {imported_rows} enhanced tags from {} through {}.",
            window.start_date, window.end_date
        ),
        imported_rows,
    ))
}

async fn execute_session_window(
    config: &Config,
    store_plan: &StorePlan,
    client: &dyn OuraClient,
    window: &DailySyncWindow,
    options: &SyncOptions,
) -> Result<(String, usize)> {
    let session_pages = fetch_with_retry(config, || {
        client.fetch_sessions(window.start_date.clone(), window.end_date.clone())
    })
    .await?;
    let imported_at = now_rfc3339()?;
    let persist_store = reopen_store(config, store_plan)?;

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
    Ok((
        format!(
            "Imported {imported_rows} sessions from {} through {}.",
            window.start_date, window.end_date
        ),
        imported_rows,
    ))
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
        let previous_cursor = previous.as_ref().and_then(|state| state.cursor.clone());
        let previous_success = previous
            .as_ref()
            .and_then(|state| state.last_successful_sync_end.clone())
            .or_else(|| previous_cursor.clone());
        let previous_reconcile_end = previous
            .as_ref()
            .and_then(|state| state.last_reconcile_end.clone());
        let previous_reconcile_start = previous
            .as_ref()
            .and_then(|state| state.oldest_recently_reconciled_at.clone());
        let failure_count = match report.status {
            SyncRunStatus::Failed => previous
                .as_ref()
                .map_or(1, |state| state.failure_count.saturating_add(1)),
            _ => 0,
        };
        let attempted_at = now_rfc3339()?;
        let completed_at = now_rfc3339()?;
        let cursor = if matches!(
            report.status,
            SyncRunStatus::Success | SyncRunStatus::Partial
        ) {
            prefer_later_marker(report.watermark.clone(), previous_cursor)
        } else {
            previous_cursor
        };
        let last_successful_sync_end = if matches!(
            report.status,
            SyncRunStatus::Success | SyncRunStatus::Partial
        ) {
            prefer_later_marker(report.last_successful_sync_end.clone(), previous_success)
        } else {
            previous_success
        };
        let last_reconcile_end =
            prefer_later_marker(report.last_reconcile_end.clone(), previous_reconcile_end);
        let oldest_recently_reconciled_at = prefer_earlier_marker(
            report.oldest_recently_reconciled_at.clone(),
            previous_reconcile_start,
        );
        let next_attempt_after = if report.status == SyncRunStatus::Failed {
            report
                .next_attempt_after
                .clone()
                .or_else(|| compute_next_attempt_after(config, &report.sync_key, failure_count))
        } else {
            None
        };
        let last_error_at =
            if report.last_error.is_some() || report.status == SyncRunStatus::Blocked {
                Some(completed_at.clone())
            } else {
                None
            };
        store.sync_state().upsert(&SyncStateRecord {
            sync_key: report.sync_key.clone(),
            family: report.family.label().to_owned(),
            status: report.status.clone(),
            cursor,
            last_successful_sync_end,
            last_attempted_at: attempted_at,
            last_completed_at: Some(completed_at),
            last_reconcile_end,
            oldest_recently_reconciled_at,
            message: Some(report.message.clone()),
            granted_scopes,
            last_error: report.last_error.clone(),
            last_error_at,
            last_error_kind: report
                .last_error
                .as_ref()
                .map(classify_problem_kind)
                .map(str::to_owned)
                .or_else(|| {
                    (report.status == SyncRunStatus::Blocked).then(|| "blocked".to_owned())
                }),
            last_error_detail: if report.last_error.is_some()
                || report.status == SyncRunStatus::Blocked
            {
                Some(bound_sync_detail(&report.message))
            } else {
                None
            },
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
            updated_at: now_rfc3339()?,
        })?;
    }

    Ok(report)
}

fn prefer_later_marker(next: Option<String>, previous: Option<String>) -> Option<String> {
    match (next, previous) {
        (Some(next), Some(previous)) => Some(if next >= previous { next } else { previous }),
        (Some(next), None) => Some(next),
        (None, Some(previous)) => Some(previous),
        (None, None) => None,
    }
}

fn prefer_earlier_marker(next: Option<String>, previous: Option<String>) -> Option<String> {
    match (next, previous) {
        (Some(next), Some(previous)) => Some(if next <= previous { next } else { previous }),
        (Some(next), None) => Some(next),
        (None, Some(previous)) => Some(previous),
        (None, None) => None,
    }
}

fn new_slice_report(
    family: SyncFamily,
    status: SyncRunStatus,
    imported_rows: usize,
    watermark: Option<String>,
    message: String,
    last_error: Option<OuraProblem>,
) -> SliceReport {
    let last_successful_sync_end =
        if matches!(status, SyncRunStatus::Success | SyncRunStatus::Partial) {
            watermark.clone()
        } else {
            None
        };
    SliceReport {
        sync_key: family.sync_key().to_owned(),
        family,
        status,
        imported_rows,
        watermark,
        last_successful_sync_end,
        last_reconcile_end: None,
        oldest_recently_reconciled_at: None,
        message,
        last_error,
        next_attempt_after: None,
    }
}

fn slice_blocked_family(family: SyncFamily, message: &str) -> SliceReport {
    new_slice_report(
        family,
        SyncRunStatus::Blocked,
        0,
        None,
        message.to_owned(),
        None,
    )
}

fn slice_success_family(family: SyncFamily, message: &str) -> SliceReport {
    new_slice_report(
        family,
        SyncRunStatus::Success,
        0,
        None,
        message.to_owned(),
        None,
    )
}

fn failed_slice_report_family(family: SyncFamily, problem: OuraProblem) -> SliceReport {
    let message = format!("{}: {problem}", family.sync_key());
    new_slice_report(
        family,
        SyncRunStatus::Failed,
        0,
        None,
        message,
        Some(problem),
    )
}

#[derive(Debug, Clone, Copy)]
struct FamilyScopeGuard<'a> {
    capability: CapabilityKind,
    family: SyncFamily,
    not_requested_message: &'a str,
    missing_scope_message: &'a str,
}

fn guard_family_scope(
    config: &Config,
    store_plan: &StorePlan,
    capability_report: &CapabilityReport,
    guard: FamilyScopeGuard<'_>,
    options: &SyncOptions,
) -> Result<Option<SliceReport>> {
    let Some(entry) = capability_report.status_for(guard.capability) else {
        return Ok(None);
    };
    if entry.granted {
        return Ok(None);
    }

    let persist_store = reopen_store(config, store_plan)?;
    let report = if entry.requested {
        slice_blocked_family(guard.family, guard.missing_scope_message)
    } else {
        slice_success_family(guard.family, guard.not_requested_message)
    };
    let persisted = persist_slice_report(
        config,
        &persist_store,
        report,
        granted_scopes_from_report(capability_report),
        options,
    )?;
    Ok(Some(persisted))
}

const fn classify_problem_kind(problem: &OuraProblem) -> &'static str {
    match problem.status {
        Some(401 | 403) => "auth",
        Some(429) => "rate_limit",
        Some(500..=599) => "transient_api",
        Some(_) => "api_error",
        None if problem.oauth_error.is_some() => "auth",
        None => "transport",
    }
}

fn bound_sync_detail(detail: &str) -> String {
    const MAX_DETAIL_CHARS: usize = 512;
    let mut bounded = detail.chars().take(MAX_DETAIL_CHARS).collect::<String>();
    if detail.chars().count() > MAX_DETAIL_CHARS {
        bounded.push_str("...");
    }
    bounded
}

fn sync_family_from_key(sync_key: &str) -> Option<SyncFamily> {
    SyncFamily::ALL
        .into_iter()
        .find(|family| family.sync_key() == sync_key)
}

fn slice_blocked_by_key(sync_key: &str, message: &str) -> SliceReport {
    let family = sync_family_from_key(sync_key).unwrap_or(SyncFamily::Daily);
    slice_blocked_family(family, message)
}

fn failed_slice_report_by_key(sync_key: &str, problem: OuraProblem) -> SliceReport {
    let family = sync_family_from_key(sync_key).unwrap_or(SyncFamily::Daily);
    failed_slice_report_family(family, problem)
}

fn slice_blocked(sync_key: &str, message: &str) -> SliceReport {
    slice_blocked_by_key(sync_key, message)
}

fn failed_slice_report(sync_key: &str, problem: OuraProblem) -> SliceReport {
    failed_slice_report_by_key(sync_key, problem)
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
        SPO2_SYNC_KEY => Some(SyncFamily::Spo2),
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

    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    use super::{CapabilityReport, FixtureOuraClient, SyncOptions, sync_once};
    use crate::config::{
        AppPaths, Config, DEFAULT_OURA_API_BASE_URL, DEFAULT_OURA_AUTHORIZE_URL,
        DEFAULT_OURA_TOKEN_URL, LoggingConfig, OuraConfig, RefreshConfig, WebhookConfig,
    };
    use crate::error::{OuraApiError, RingmasterError};
    use crate::oura::client::PageFetch;
    use crate::oura::models::{DailySpO2Document, SleepDocument};
    use crate::oura::policy::SyncPolicy;
    use crate::refresh::SyncFamily;
    use crate::store::Store;
    use crate::store::queries::{
        RawPayloadRecord, SleepPeriodRecord, SyncRunStatus, SyncStateRecord,
    };
    use crate::test_support::{ok, some};
    use crate::webhook::default_desired_subscriptions;
    use serde_json::json;

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
                demo_fixture_dir: None,
                ..RefreshConfig::default()
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
            mode: super::SyncMode::Standard,
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
                mode: super::SyncMode::Standard,
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
                mode: super::SyncMode::Standard,
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
                mode: super::SyncMode::Standard,
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
                mode: super::SyncMode::Standard,
                trigger_source: Some("periodic_reconcile".to_owned()),
                trigger_detail: Some("test degraded optional daily sync".to_owned()),
            },
        )
        .await;
        let report = ok(report, "daily sync should degrade instead of failing");
        let counts = ok(store.views().record_counts(), "record counts should load");
        let daily_state = some(
            ok(
                store.sync_state().get(super::DAILY_SYNC_KEY),
                "daily state should load",
            ),
            "daily state should persist",
        );
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
        let expected_success_end = OffsetDateTime::now_utc().date().to_string();
        assert_eq!(
            daily_state.last_successful_sync_end.as_deref(),
            Some(expected_success_end.as_str()),
            "partial daily syncs should advance the committed daily window even when an optional endpoint degrades",
        );
        assert_eq!(
            daily_state.cursor.as_deref(),
            Some(expected_success_end.as_str()),
            "partial daily syncs should keep the retry cursor aligned with the committed daily window",
        );
    }

    #[tokio::test]
    async fn daily_sync_fails_when_sleep_endpoint_fixture_is_malformed() {
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        let tempdir = ok(tempfile::tempdir(), "tempdir should build");
        let fixture_dir = tempdir.path().join("review-malformed-sleep");
        copy_fixture_dir(&review_fixture_dir(), &fixture_dir);
        ok(
            fs::write(fixture_dir.join("sleep.json"), "{ not valid json"),
            "core sleep fixture should be rewritable",
        );

        let report = sync_once(
            &config,
            &store,
            SyncOptions {
                dry_run: false,
                fixture_dir: Some(fixture_dir),
                families: vec![SyncFamily::Daily],
                mode: super::SyncMode::Standard,
                trigger_source: Some("periodic_reconcile".to_owned()),
                trigger_detail: Some("test failed daily sync".to_owned()),
            },
        )
        .await;
        let report = ok(report, "daily sync should report a persisted failure");
        let counts = ok(store.views().record_counts(), "record counts should load");
        let daily_slice = some(
            report
                .slice_reports
                .iter()
                .find(|slice| slice.sync_key == "oura.daily"),
            "daily slice should exist",
        );

        assert_eq!(report.status, SyncRunStatus::Failed);
        assert_eq!(daily_slice.status, SyncRunStatus::Failed);
        assert!(daily_slice.message.contains("sleep"));
        assert!(daily_slice.last_error.is_some());
        assert_eq!(counts.daily_sleep, 0);
        assert_eq!(counts.daily_readiness, 0);
        assert_eq!(counts.daily_activity, 0);
    }

    #[test]
    fn persist_sleep_period_pages_writes_metric_samples() {
        let store = ok(Store::open_test_store(), "store should open");
        let window = super::DailySyncWindow {
            start_date: "2026-04-08".to_owned(),
            end_date: "2026-04-08".to_owned(),
        };
        let document: SleepDocument = ok(
            serde_json::from_value(json!({
                "id": "sleep_2026-04-08_primary",
                "day": "2026-04-08",
                "bedtime_start": "2026-04-08T22:45:00Z",
                "bedtime_end": "2026-04-09T06:35:00Z",
                "average_heart_rate": 55.2,
                "average_hrv": 41.8,
                "average_breath": 13.4,
                "total_sleep_duration": 28200,
                "type": "long_sleep"
            })),
            "sleep fixture document should deserialize",
        );
        let pages = vec![PageFetch {
            raw_payload: RawPayloadRecord {
                cache_key: "sleep-page-2026-04-08".to_owned(),
                endpoint: "sleep".to_owned(),
                requested_at: "2026-04-09T06:40:00Z".to_owned(),
                scope: Some("daily".to_owned()),
                etag: None,
                payload: "{\"data\":[]}".to_owned(),
            },
            documents: vec![document],
        }];

        ok(
            super::persist_sleep_period_pages(&store, &window, &pages, "2026-04-09T06:45:00Z"),
            "sleep pages should persist",
        );

        let counts = ok(store.views().record_counts(), "record counts should load");
        let records = ok(
            store
                .views()
                .sleep_periods_between_days("2026-04-08", "2026-04-08"),
            "sleep period records should load",
        );

        assert_eq!(counts.sleep_periods, 1);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].oura_id, "sleep_2026-04-08_primary");
        assert_eq!(records[0].average_hrv, Some(41.8));
        assert_eq!(records[0].average_breath, Some(13.4));
        assert_eq!(records[0].sleep_type.as_deref(), Some("long_sleep"));
    }

    #[test]
    fn persist_sleep_period_pages_reconciles_removed_rows_within_the_sync_window() {
        let store = ok(Store::open_test_store(), "store should open");
        let imports = store.imports();

        ok(
            imports.upsert_sleep_period(&SleepPeriodRecord {
                oura_id: "sleep_2026-04-07_outside".to_owned(),
                day: "2026-04-07".to_owned(),
                bedtime_start: Some("2026-04-07T22:30:00Z".to_owned()),
                bedtime_end: Some("2026-04-08T06:15:00Z".to_owned()),
                sleep_type: Some("long_sleep".to_owned()),
                average_heart_rate: Some(54.0),
                average_hrv: Some(42.0),
                average_breath: Some(13.0),
                total_sleep_duration: Some(27_900),
                raw_cache_key: Some("seed-outside".to_owned()),
                updated_at: "2026-04-09T05:00:00Z".to_owned(),
            }),
            "out-of-window seed row should persist",
        );
        ok(
            imports.upsert_sleep_period(&SleepPeriodRecord {
                oura_id: "sleep_2026-04-08_removed".to_owned(),
                day: "2026-04-08".to_owned(),
                bedtime_start: Some("2026-04-08T21:45:00Z".to_owned()),
                bedtime_end: Some("2026-04-09T05:45:00Z".to_owned()),
                sleep_type: Some("long_sleep".to_owned()),
                average_heart_rate: Some(57.0),
                average_hrv: Some(30.0),
                average_breath: Some(14.1),
                total_sleep_duration: Some(25_200),
                raw_cache_key: Some("seed-removed".to_owned()),
                updated_at: "2026-04-09T05:00:00Z".to_owned(),
            }),
            "removed in-window row should persist",
        );
        ok(
            imports.upsert_sleep_period(&SleepPeriodRecord {
                oura_id: "sleep_2026-04-09_removed".to_owned(),
                day: "2026-04-09".to_owned(),
                bedtime_start: Some("2026-04-09T22:00:00Z".to_owned()),
                bedtime_end: Some("2026-04-10T06:00:00Z".to_owned()),
                sleep_type: Some("long_sleep".to_owned()),
                average_heart_rate: Some(58.0),
                average_hrv: Some(29.5),
                average_breath: Some(14.4),
                total_sleep_duration: Some(24_600),
                raw_cache_key: Some("seed-removed-2".to_owned()),
                updated_at: "2026-04-10T05:00:00Z".to_owned(),
            }),
            "second removed in-window row should persist",
        );

        let window = super::DailySyncWindow {
            start_date: "2026-04-08".to_owned(),
            end_date: "2026-04-09".to_owned(),
        };
        let replacement: SleepDocument = ok(
            serde_json::from_value(json!({
                "id": "sleep_2026-04-08_survives",
                "day": "2026-04-08",
                "bedtime_start": "2026-04-08T22:45:00Z",
                "bedtime_end": "2026-04-09T06:35:00Z",
                "average_heart_rate": 55.2,
                "average_hrv": 41.8,
                "average_breath": 13.4,
                "total_sleep_duration": 28200,
                "type": "long_sleep"
            })),
            "replacement sleep fixture document should deserialize",
        );
        let pages = vec![PageFetch {
            raw_payload: RawPayloadRecord {
                cache_key: "sleep-page-2026-04-window".to_owned(),
                endpoint: "sleep".to_owned(),
                requested_at: "2026-04-10T06:40:00Z".to_owned(),
                scope: Some("daily".to_owned()),
                etag: None,
                payload: "{\"data\":[]}".to_owned(),
            },
            documents: vec![replacement],
        }];

        ok(
            super::persist_sleep_period_pages(&store, &window, &pages, "2026-04-10T06:45:00Z"),
            "sleep pages should reconcile the in-window rows",
        );

        let counts = ok(store.views().record_counts(), "record counts should load");
        let records = ok(
            store
                .views()
                .sleep_periods_between_days("2026-04-07", "2026-04-09"),
            "sleep period records should load after reconciliation",
        );

        assert_eq!(counts.sleep_periods, 2);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].oura_id, "sleep_2026-04-07_outside");
        assert_eq!(records[1].oura_id, "sleep_2026-04-08_survives");
        assert_eq!(records[1].average_hrv, Some(41.8));
        assert!(
            records
                .iter()
                .all(|record| record.oura_id != "sleep_2026-04-08_removed"
                    && record.oura_id != "sleep_2026-04-09_removed")
        );
    }

    #[test]
    fn persist_daily_spo2_pages_writes_average_spo2() {
        let store = ok(Store::open_test_store(), "store should open");
        let document: DailySpO2Document = ok(
            serde_json::from_value(json!({
                "id": "spo2_2026-04-08",
                "day": "2026-04-08",
                "spo2_percentage": {
                    "average": 97.4
                },
                "breathing_disturbance_index": 0.6
            })),
            "daily_spo2 fixture document should deserialize",
        );
        let pages = vec![PageFetch {
            raw_payload: RawPayloadRecord {
                cache_key: "daily-spo2-page-2026-04-08".to_owned(),
                endpoint: "daily_spo2".to_owned(),
                requested_at: "2026-04-09T06:40:00Z".to_owned(),
                scope: Some("spo2".to_owned()),
                etag: None,
                payload: "{\"data\":[]}".to_owned(),
            },
            documents: vec![document],
        }];

        ok(
            super::persist_daily_spo2_pages(&store, &pages, "2026-04-09T06:45:00Z"),
            "daily_spo2 pages should persist",
        );

        let counts = ok(store.views().record_counts(), "record counts should load");
        let records = ok(
            store
                .views()
                .daily_spo2_between_days("2026-04-08", "2026-04-08"),
            "daily_spo2 records should load",
        );

        assert_eq!(counts.daily_spo2, 1);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].oura_id.as_deref(), Some("spo2_2026-04-08"));
        assert_eq!(records[0].average_spo2, Some(97.4));
        assert_eq!(records[0].breathing_disturbance_index, Some(0.6));
    }

    #[tokio::test]
    async fn spo2_sync_is_blocked_when_daily_scope_is_missing() {
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        let tempdir = ok(tempfile::tempdir(), "tempdir should build");
        let fixture_dir = tempdir.path().join("spo2-missing-daily");
        copy_fixture_dir(&review_fixture_dir(), &fixture_dir);

        let client = FixtureOuraClient::new(&config, &fixture_dir);
        let capability_report = CapabilityReport::from_scopes(
            &["daily".to_owned(), "spo2".to_owned()],
            &["spo2".to_owned()],
        );
        let report = super::sync_spo2(
            &config,
            store.plan().clone(),
            &client,
            &capability_report,
            &SyncOptions {
                dry_run: false,
                fixture_dir: Some(fixture_dir),
                families: vec![SyncFamily::Spo2],
                mode: super::SyncMode::Standard,
                trigger_source: Some("periodic_reconcile".to_owned()),
                trigger_detail: Some("test spo2 dependency guard".to_owned()),
            },
        )
        .await;
        let spo2_slice = ok(report, "spo2 sync should short-circuit cleanly");

        assert_eq!(spo2_slice.status, SyncRunStatus::Blocked);
        assert!(spo2_slice.message.contains("daily coverage"));
        let sync_state = some(
            ok(
                store.sync_state().get(super::SPO2_SYNC_KEY),
                "spo2 sync state should persist",
            ),
            "spo2 sync state should exist",
        );
        assert_eq!(sync_state.status, SyncRunStatus::Blocked);
        assert_eq!(sync_state.family, "spo2");
        assert!(
            sync_state
                .message
                .unwrap_or_default()
                .contains("daily coverage")
        );
    }

    #[test]
    fn partial_daily_slice_still_triggers_derive_rebuild() {
        assert!(super::should_rebuild_derived_state(&[super::SliceReport {
            sync_key: "oura.daily".to_owned(),
            family: SyncFamily::Daily,
            status: SyncRunStatus::Partial,
            imported_rows: 3,
            watermark: Some("2026-04-08".to_owned()),
            last_successful_sync_end: Some("2026-04-08".to_owned()),
            last_reconcile_end: None,
            oldest_recently_reconciled_at: None,
            message: "partial daily sync".to_owned(),
            last_error: None,
            next_attempt_after: None,
        }]));
    }

    #[test]
    fn fresh_daily_sync_uses_configured_history_window() {
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        let policy = SyncPolicy::for_family(&config.refresh, SyncFamily::Daily);
        let windows = ok(
            super::plan_daily_windows(
                &config,
                store.plan(),
                SyncFamily::Daily,
                &SyncOptions {
                    dry_run: false,
                    fixture_dir: None,
                    families: vec![SyncFamily::Daily],
                    mode: super::SyncMode::Standard,
                    trigger_source: Some("startup".to_owned()),
                    trigger_detail: Some("planner test".to_owned()),
                },
                &policy,
            ),
            "daily startup plan should build",
        );

        let expected_start = super::recent_day_start(
            i64::from(config.refresh.daily_history_days),
            OffsetDateTime::now_utc(),
        )
        .to_string();

        assert!(!windows.is_empty());
        assert!(
            windows
                .iter()
                .all(|window| window.purpose == super::SyncWindowPurpose::Tail)
        );
        assert_eq!(
            windows
                .first()
                .map(|window| window.window.start_date.as_str()),
            Some(expected_start.as_str())
        );
    }

    #[test]
    fn seeded_daily_sync_still_limits_tail_to_recent_window() {
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        let policy = SyncPolicy::for_family(&config.refresh, SyncFamily::Daily);
        ok(
            store.sync_state().upsert(&SyncStateRecord {
                sync_key: "oura.daily".to_owned(),
                family: "daily".to_owned(),
                status: SyncRunStatus::Success,
                cursor: Some("2026-01-01".to_owned()),
                last_successful_sync_end: Some("2026-01-01".to_owned()),
                last_attempted_at: "2026-01-01T06:00:00Z".to_owned(),
                last_completed_at: Some("2026-01-01T06:00:05Z".to_owned()),
                last_reconcile_end: Some("2026-01-01".to_owned()),
                oldest_recently_reconciled_at: Some("2025-12-01".to_owned()),
                message: Some("seeded daily sync".to_owned()),
                granted_scopes: vec!["daily".to_owned()],
                last_error: None,
                last_error_at: None,
                last_error_kind: None,
                last_error_detail: None,
                failure_count: 0,
                next_attempt_after: None,
                last_trigger_source: Some("periodic_reconcile".to_owned()),
                last_trigger_detail: Some("seed daily sync".to_owned()),
                updated_at: "2026-01-01T06:00:05Z".to_owned(),
            }),
            "seeded daily sync state should persist",
        );
        let windows = ok(
            super::plan_daily_windows(
                &config,
                store.plan(),
                SyncFamily::Daily,
                &SyncOptions {
                    dry_run: false,
                    fixture_dir: None,
                    families: vec![SyncFamily::Daily],
                    mode: super::SyncMode::Standard,
                    trigger_source: Some("startup".to_owned()),
                    trigger_detail: Some("seeded planner test".to_owned()),
                },
                &policy,
            ),
            "seeded daily startup plan should build",
        );

        let expected_start =
            super::recent_day_start(policy.startup_catchup_days(), OffsetDateTime::now_utc())
                .to_string();

        assert!(!windows.is_empty());
        assert!(
            windows
                .iter()
                .all(|window| window.purpose == super::SyncWindowPurpose::Tail)
        );
        assert_eq!(
            windows
                .first()
                .map(|window| window.window.start_date.as_str()),
            Some(expected_start.as_str())
        );
    }

    #[test]
    fn steady_state_daily_sync_accepts_date_markers_from_backfill() {
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        let policy = SyncPolicy::for_family(&config.refresh, SyncFamily::Daily);
        let now = OffsetDateTime::now_utc();
        let success_end = now.date().to_string();
        let reconcile_start = super::recent_day_start(policy.reconcile_days(), now).to_string();
        ok(
            store.sync_state().upsert(&SyncStateRecord {
                sync_key: "oura.daily".to_owned(),
                family: "daily".to_owned(),
                status: SyncRunStatus::Success,
                cursor: Some(success_end.clone()),
                last_successful_sync_end: Some(success_end.clone()),
                last_attempted_at: "2026-04-08T06:00:00Z".to_owned(),
                last_completed_at: Some("2026-04-08T06:00:05Z".to_owned()),
                last_reconcile_end: Some(success_end),
                oldest_recently_reconciled_at: Some(reconcile_start),
                message: Some("recent daily backfill".to_owned()),
                granted_scopes: vec!["daily".to_owned()],
                last_error: None,
                last_error_at: None,
                last_error_kind: None,
                last_error_detail: None,
                failure_count: 0,
                next_attempt_after: None,
                last_trigger_source: Some("manual_backfill".to_owned()),
                last_trigger_detail: Some("seed daily backfill".to_owned()),
                updated_at: "2026-04-08T06:00:05Z".to_owned(),
            }),
            "daily sync state should persist",
        );

        let windows = ok(
            super::plan_daily_windows(
                &config,
                store.plan(),
                SyncFamily::Daily,
                &SyncOptions {
                    dry_run: false,
                    fixture_dir: None,
                    families: vec![SyncFamily::Daily],
                    mode: super::SyncMode::Standard,
                    trigger_source: Some("periodic_reconcile".to_owned()),
                    trigger_detail: Some("daily planner test".to_owned()),
                },
                &policy,
            ),
            "daily sync plan should build",
        );

        assert!(!windows.is_empty());
        assert!(
            windows
                .iter()
                .all(|window| window.purpose == super::SyncWindowPurpose::Tail)
        );
    }

    #[test]
    fn recent_day_start_clamps_zero_days_to_today() {
        let now = OffsetDateTime::parse("2026-04-14T12:00:00Z", &Rfc3339)
            .unwrap_or_else(|error| panic!("timestamp should parse: {error}"));

        assert_eq!(super::recent_day_start(0, now), now.date());
    }

    #[test]
    fn steady_state_heartrate_sync_skips_recent_reconcile() {
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        let policy = SyncPolicy::for_family(&config.refresh, SyncFamily::Heartrate);
        let now = OffsetDateTime::now_utc();
        let success_end = ok(
            super::format_timestamp_marker(now - time::Duration::minutes(30)),
            "success watermark should format",
        );
        let reconcile_end = success_end.clone();
        let reconcile_start = (now - policy.reconcile_window).date().to_string();
        ok(
            store.sync_state().upsert(&SyncStateRecord {
                sync_key: "oura.heartrate".to_owned(),
                family: "heartrate".to_owned(),
                status: SyncRunStatus::Success,
                cursor: Some(success_end.clone()),
                last_successful_sync_end: Some(success_end),
                last_attempted_at: "2026-04-08T06:00:00Z".to_owned(),
                last_completed_at: Some("2026-04-08T06:00:05Z".to_owned()),
                last_reconcile_end: Some(reconcile_end),
                oldest_recently_reconciled_at: Some(reconcile_start),
                message: Some("recent heartrate sync".to_owned()),
                granted_scopes: vec!["heartrate".to_owned()],
                last_error: None,
                last_error_at: None,
                last_error_kind: None,
                last_error_detail: None,
                failure_count: 0,
                next_attempt_after: None,
                last_trigger_source: Some("periodic_reconcile".to_owned()),
                last_trigger_detail: Some("recent heartrate reconcile".to_owned()),
                updated_at: "2026-04-08T06:00:05Z".to_owned(),
            }),
            "heartrate sync state should persist",
        );

        let windows = ok(
            super::plan_heartrate_windows(
                &config,
                store.plan(),
                &SyncOptions {
                    dry_run: false,
                    fixture_dir: None,
                    families: vec![SyncFamily::Heartrate],
                    mode: super::SyncMode::Standard,
                    trigger_source: Some("periodic_reconcile".to_owned()),
                    trigger_detail: Some("heartrate planner test".to_owned()),
                },
                &policy,
            ),
            "heartrate sync plan should build",
        );

        assert!(!windows.is_empty());
        assert!(
            windows
                .iter()
                .all(|window| window.purpose == super::SyncWindowPurpose::Tail)
        );
        assert!(
            windows
                .iter()
                .all(|window| window.end - window.start <= policy.backfill_chunk),
            "steady-state heartrate sync should stay chunk-bounded"
        );
    }

    #[test]
    fn fresh_heartrate_sync_uses_configured_history_window() {
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        let policy = SyncPolicy::for_family(&config.refresh, SyncFamily::Heartrate);
        let now = OffsetDateTime::now_utc();
        let windows = ok(
            super::plan_heartrate_windows(
                &config,
                store.plan(),
                &SyncOptions {
                    dry_run: false,
                    fixture_dir: None,
                    families: vec![SyncFamily::Heartrate],
                    mode: super::SyncMode::Standard,
                    trigger_source: Some("periodic_reconcile".to_owned()),
                    trigger_detail: Some("fresh heartrate planner test".to_owned()),
                },
                &policy,
            ),
            "fresh heartrate sync plan should build",
        );

        let expected_start =
            now - time::Duration::days(i64::from(config.refresh.heartrate_history_days));

        assert!(!windows.is_empty());
        assert_eq!(
            windows.first().map(|window| window.purpose),
            Some(super::SyncWindowPurpose::Tail)
        );
        assert_eq!(
            windows.first().map(|window| window.start.unix_timestamp()),
            Some(expected_start.unix_timestamp())
        );
    }

    #[test]
    fn manual_heartrate_backfill_uses_chunked_windows() {
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        let policy = SyncPolicy::for_family(&config.refresh, SyncFamily::Heartrate);
        let windows = ok(
            super::plan_heartrate_windows(
                &config,
                store.plan(),
                &SyncOptions {
                    dry_run: false,
                    fixture_dir: None,
                    families: vec![SyncFamily::Heartrate],
                    mode: super::SyncMode::Backfill {
                        days: 30,
                        chunk_days: None,
                    },
                    trigger_source: Some("manual_backfill".to_owned()),
                    trigger_detail: Some("heartrate backfill planner test".to_owned()),
                },
                &policy,
            ),
            "manual heartrate backfill should plan",
        );

        assert!(windows.len() > 1, "30-day heartrate backfill should chunk");
        assert!(
            windows
                .iter()
                .all(|window| window.purpose == super::SyncWindowPurpose::Backfill)
        );
        assert!(
            windows
                .iter()
                .all(|window| window.end - window.start <= policy.backfill_chunk),
            "heartrate backfill windows should honor the chunk policy"
        );
    }

    #[test]
    fn failed_slice_preserves_last_successful_sync_end() {
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        ok(
            store.sync_state().upsert(&SyncStateRecord {
                sync_key: "oura.daily".to_owned(),
                family: "daily".to_owned(),
                status: SyncRunStatus::Success,
                cursor: Some("2026-04-08".to_owned()),
                last_successful_sync_end: Some("2026-04-08".to_owned()),
                last_attempted_at: "2026-04-08T06:00:00Z".to_owned(),
                last_completed_at: Some("2026-04-08T06:00:05Z".to_owned()),
                last_reconcile_end: Some("2026-04-08".to_owned()),
                oldest_recently_reconciled_at: Some("2026-03-10".to_owned()),
                message: Some("previous success".to_owned()),
                granted_scopes: vec!["daily".to_owned()],
                last_error: None,
                last_error_at: None,
                last_error_kind: None,
                last_error_detail: None,
                failure_count: 0,
                next_attempt_after: None,
                last_trigger_source: Some("periodic_reconcile".to_owned()),
                last_trigger_detail: Some("seed success".to_owned()),
                updated_at: "2026-04-08T06:00:05Z".to_owned(),
            }),
            "seed sync state should persist",
        );

        let persisted = ok(
            super::persist_slice_report(
                &config,
                &store,
                super::SliceReport {
                    sync_key: "oura.daily".to_owned(),
                    family: SyncFamily::Daily,
                    status: SyncRunStatus::Failed,
                    imported_rows: 0,
                    watermark: None,
                    last_successful_sync_end: None,
                    last_reconcile_end: None,
                    oldest_recently_reconciled_at: None,
                    message: "daily upstream failure".to_owned(),
                    last_error: Some(crate::error::OuraProblem::new(
                        Some(500),
                        "Internal Server Error",
                        Some("temporary upstream failure".to_owned()),
                    )),
                    next_attempt_after: None,
                },
                vec!["daily".to_owned()],
                &SyncOptions {
                    dry_run: false,
                    fixture_dir: None,
                    families: vec![SyncFamily::Daily],
                    mode: super::SyncMode::Standard,
                    trigger_source: Some("periodic_reconcile".to_owned()),
                    trigger_detail: Some("failed slice preservation test".to_owned()),
                },
            ),
            "failed slice should persist without losing coverage",
        );

        let record = some(
            ok(
                store.sync_state().get("oura.daily"),
                "daily sync state should load",
            ),
            "daily sync state should exist",
        );

        assert_eq!(persisted.status, SyncRunStatus::Failed);
        assert_eq!(
            record.last_successful_sync_end.as_deref(),
            Some("2026-04-08")
        );
        assert_eq!(record.cursor.as_deref(), Some("2026-04-08"));
        assert_eq!(record.failure_count, 1);
        assert_eq!(record.last_error_kind.as_deref(), Some("transient_api"));
        assert!(record.next_attempt_after.is_some());
    }

    #[test]
    fn failed_later_chunk_preserves_successful_chunk_progress() {
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        let mut summary = super::FamilySyncSummary::new(SyncFamily::Daily);
        summary.observe(super::SliceReport {
            sync_key: "oura.daily".to_owned(),
            family: SyncFamily::Daily,
            status: SyncRunStatus::Success,
            imported_rows: 12,
            watermark: Some("2026-04-10".to_owned()),
            last_successful_sync_end: Some("2026-04-10".to_owned()),
            last_reconcile_end: Some("2026-04-10".to_owned()),
            oldest_recently_reconciled_at: Some("2026-04-01".to_owned()),
            message: "backfill window: imported 12 daily rows".to_owned(),
            last_error: None,
            next_attempt_after: None,
        });
        let chunk_error =
            RingmasterError::from(OuraApiError::from(crate::error::OuraProblem::new(
                Some(500),
                "Internal Server Error",
                Some("temporary upstream failure".to_owned()),
            )));
        super::observe_failed_chunk(
            &mut summary,
            SyncFamily::Daily,
            super::SyncWindowPurpose::Backfill,
            &chunk_error,
        );

        let persisted = ok(
            super::persist_slice_report(
                &config,
                &store,
                summary.finish(),
                vec!["daily".to_owned()],
                &SyncOptions {
                    dry_run: false,
                    fixture_dir: None,
                    families: vec![SyncFamily::Daily],
                    mode: super::SyncMode::Backfill {
                        days: 30,
                        chunk_days: None,
                    },
                    trigger_source: Some("manual_backfill".to_owned()),
                    trigger_detail: Some("partial chunk preservation test".to_owned()),
                },
            ),
            "partial chunk summary should persist",
        );

        let record = some(
            ok(
                store.sync_state().get("oura.daily"),
                "daily sync state should load",
            ),
            "daily sync state should exist",
        );

        assert_eq!(persisted.status, SyncRunStatus::Partial);
        assert_eq!(record.status, SyncRunStatus::Partial);
        assert_eq!(
            record.last_successful_sync_end.as_deref(),
            Some("2026-04-10")
        );
        assert_eq!(record.cursor.as_deref(), Some("2026-04-10"));
        assert_eq!(record.last_reconcile_end.as_deref(), Some("2026-04-10"));
        assert_eq!(
            record.oldest_recently_reconciled_at.as_deref(),
            Some("2026-04-01")
        );
        assert_eq!(record.failure_count, 0);
        assert_eq!(record.last_error_kind.as_deref(), Some("transient_api"));
        assert!(record.last_error_detail.is_some());
    }

    #[tokio::test]
    async fn manual_backfill_remains_idempotent() {
        let store = ok(Store::open_test_store(), "store should open");
        let config = fixture_config();
        let options = SyncOptions {
            dry_run: false,
            fixture_dir: Some(baseline_fixture_dir()),
            families: vec![SyncFamily::Heartrate],
            mode: super::SyncMode::Backfill {
                days: 30,
                chunk_days: Some(1),
            },
            trigger_source: Some("manual_backfill".to_owned()),
            trigger_detail: Some("fixture heartrate backfill".to_owned()),
        };

        let first = ok(
            sync_once(&config, &store, options.clone()).await,
            "first heartrate backfill should succeed",
        );
        let second = ok(
            sync_once(&config, &store, options).await,
            "second heartrate backfill should stay idempotent",
        );
        let counts = ok(store.views().record_counts(), "record counts should load");

        assert_eq!(first.status, SyncRunStatus::Success);
        assert_eq!(second.status, SyncRunStatus::Success);
        assert_eq!(counts.heartrate_samples, 12);
    }
}
