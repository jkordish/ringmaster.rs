use std::path::PathBuf;

use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::config::Config;
use crate::error::{AuthError, OuraProblem, Result};
use crate::oura::auth;
use crate::oura::client::{FixtureOuraClient, OuraClient, ReqwestOuraClient};
use crate::oura::models::{CapabilityKind, CapabilityReport};
use crate::store::Store;
use crate::store::queries::{
    AuthSessionRecord, DailyActivityRecord, DailyReadinessRecord, DailySleepRecord,
    HeartrateSampleRecord, OURA_PROVIDER, PersonalInfoRecord, SyncRunStatus, SyncStateRecord,
};

const PERSONAL_SYNC_KEY: &str = "oura.personal";
const DAILY_SYNC_KEY: &str = "oura.daily";
const HEARTRATE_SYNC_KEY: &str = "oura.heartrate";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncOptions {
    pub dry_run: bool,
    pub fixture_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceReport {
    pub sync_key: String,
    pub status: SyncRunStatus,
    pub imported_rows: usize,
    pub watermark: Option<String>,
    pub message: String,
    pub last_error: Option<OuraProblem>,
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
    let started_at = now_rfc3339()?;
    let mut notes = Vec::new();

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
                store,
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
                store,
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
            .user_agent("ringmaster.rs/phase1")
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let session = auth::ensure_authorized_session(config, store, &http_client).await?;
        let client = ReqwestOuraClient::new(config, session.access_token, &session.granted_scopes)?;
        let capability_report = client.capability_report();
        (Box::new(client) as Box<dyn OuraClient>, capability_report)
    };

    let mut slice_reports = Vec::new();
    slice_reports.push(
        sync_personal_info(config, store, client.as_ref(), &capability_report, &options).await?,
    );
    slice_reports
        .push(sync_daily(config, store, client.as_ref(), &capability_report, &options).await?);
    slice_reports
        .push(sync_heartrate(config, store, client.as_ref(), &capability_report, &options).await?);

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

async fn sync_personal_info(
    _config: &Config,
    store: &Store,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !capability_report.is_granted(CapabilityKind::Personal) {
        return persist_slice_report(
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
            auth_session.account_email = fetched.document.email.clone();
            auth_session.updated_at = imported_at.clone();
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
        store,
        SliceReport {
            sync_key: PERSONAL_SYNC_KEY.to_owned(),
            status: SyncRunStatus::Success,
            imported_rows: 1,
            watermark: Some(imported_at.clone()),
            message: format!(
                "Imported personal info for profile {}.",
                fetched.document.id
            ),
            last_error: None,
        },
        granted_scopes_from_report(capability_report),
        options,
    )
}

async fn sync_daily(
    _config: &Config,
    store: &Store,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !capability_report.is_granted(CapabilityKind::Daily) {
        return persist_slice_report(
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
        overlap_day_window(store, DAILY_SYNC_KEY, 90, 2)?
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
        },
        granted_scopes_from_report(capability_report),
        options,
    )
}

async fn sync_heartrate(
    _config: &Config,
    store: &Store,
    client: &dyn OuraClient,
    capability_report: &CapabilityReport,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !capability_report.is_granted(CapabilityKind::Heartrate) {
        return persist_slice_report(
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
        overlap_heartrate_window(store, 7, 60)?
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
        },
        granted_scopes_from_report(capability_report),
        options,
    )
}

fn persist_blocked_slice_reports(
    store: &Store,
    granted_scopes: &[String],
    message: &str,
    options: &SyncOptions,
) -> Result<Vec<SliceReport>> {
    [
        slice_blocked(PERSONAL_SYNC_KEY, message),
        slice_blocked(DAILY_SYNC_KEY, message),
        slice_blocked(HEARTRATE_SYNC_KEY, message),
    ]
    .into_iter()
    .map(|report| persist_slice_report(store, report, granted_scopes.to_vec(), options))
    .collect()
}

fn persist_slice_report(
    store: &Store,
    report: SliceReport,
    granted_scopes: Vec<String>,
    options: &SyncOptions,
) -> Result<SliceReport> {
    if !options.dry_run {
        let attempted_at = now_rfc3339()?;
        let completed_at = now_rfc3339()?;
        store.sync_state().upsert(&SyncStateRecord {
            sync_key: report.sync_key.clone(),
            status: report.status.clone(),
            cursor: report.watermark.clone(),
            last_attempted_at: attempted_at,
            last_completed_at: Some(completed_at),
            message: Some(report.message.clone()),
            granted_scopes,
            last_error: report.last_error.clone(),
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
        .list()?
        .into_iter()
        .find(|record| record.sync_key == sync_key && record.status == SyncRunStatus::Success)
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
    let Some(sync_state) = store.sync_state().list()?.into_iter().find(|record| {
        record.sync_key == HEARTRATE_SYNC_KEY && record.status == SyncRunStatus::Success
    }) else {
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
    use crate::config::{AppPaths, Config, LoggingConfig, OuraConfig};
    use crate::store::Store;
    use crate::store::queries::SyncRunStatus;

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
                ],
                auth_timeout_secs: 120,
            },
        }
    }

    #[tokio::test]
    async fn fixture_sync_populates_phase1_tables_idempotently() {
        let store = Store::open_in_memory().expect("store should open");
        let config = fixture_config();
        let options = SyncOptions {
            dry_run: false,
            fixture_dir: Some(PathBuf::from(
                "/home/ubuntu/ringmaster.rs/tests/fixtures/phase1",
            )),
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
        assert_eq!(counts.daily_sleep, 3);
        assert_eq!(counts.daily_readiness, 3);
        assert_eq!(counts.daily_activity, 3);
        assert_eq!(counts.heartrate_samples, 5);
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
                fixture_dir: Some(PathBuf::from(
                    "/home/ubuntu/ringmaster.rs/tests/fixtures/phase1",
                )),
            },
        )
        .await
        .expect("dry run should succeed");
        let counts = store.views().record_counts().expect("record counts");

        assert_eq!(report.status, SyncRunStatus::Success);
        assert_eq!(counts.personal_info, 0);
        assert_eq!(counts.daily_sleep, 0);
        assert_eq!(counts.heartrate_samples, 0);
    }
}
