use std::path::PathBuf;
use std::time::Duration as StdDuration;

use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::config::{Config, RefreshConfig};
use crate::error::{Result, RingmasterError};
use crate::oura::models::CapabilityKind;
use crate::oura::sync::{SliceReport, SyncOptions, SyncReport, sync_selected};
use crate::store::Store;
use crate::store::queries::SyncStateRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SyncFamily {
    Personal,
    Daily,
    Heartrate,
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

impl SyncFamily {
    pub const ALL: [Self; 3] = [Self::Personal, Self::Daily, Self::Heartrate];

    pub fn label(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Daily => "daily",
            Self::Heartrate => "heartrate",
        }
    }

    pub fn sync_key(self) -> &'static str {
        match self {
            Self::Personal => "oura.personal",
            Self::Daily => "oura.daily",
            Self::Heartrate => "oura.heartrate",
        }
    }

    pub fn capability_kind(self) -> CapabilityKind {
        match self {
            Self::Personal => CapabilityKind::Personal,
            Self::Daily => CapabilityKind::Daily,
            Self::Heartrate => CapabilityKind::Heartrate,
        }
    }

    pub fn interval_secs(self, refresh: &RefreshConfig) -> u64 {
        match self {
            Self::Personal => refresh.personal_interval_secs,
            Self::Daily => refresh.daily_interval_secs,
            Self::Heartrate => refresh.heartrate_interval_secs,
        }
    }

    pub fn stale_after_secs(self, refresh: &RefreshConfig) -> u64 {
        match self {
            Self::Personal => refresh.personal_stale_after_secs,
            Self::Daily => refresh.daily_stale_after_secs,
            Self::Heartrate => refresh.heartrate_stale_after_secs,
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

        let report = sync_selected(
            config,
            &store,
            SyncOptions {
                dry_run,
                fixture_dir: fixture_dir.clone(),
                families,
            },
        )
        .await?;
        if let Some(simulated_sync_states) = simulated_sync_states.as_mut() {
            advance_dry_run_sync_states(config, simulated_sync_states, &report);
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

fn resolve_fixture_dir(config: &Config, options: &WatchOptions) -> Option<PathBuf> {
    options.fixture_dir.clone().or_else(|| {
        options.demo.then(|| {
            config
                .refresh
                .demo_fixture_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from("tests/fixtures/phase1"))
        })
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
    use crate::config::{AppPaths, Config, LoggingConfig, OuraConfig, RefreshConfig};
    use crate::oura::models::CapabilityReport;
    use crate::oura::sync::{SliceReport, SyncReport};
    use crate::refresh::run_watch;
    use crate::store::queries::{SyncRunStatus, SyncStateRecord};
    use std::path::PathBuf;
    use time::format_description::well_known::Rfc3339;
    use time::{Duration, OffsetDateTime};

    fn test_config() -> Config {
        Config {
            app_name: "ringmaster",
            paths: AppPaths::from_roots(
                PathBuf::from("/home/tester"),
                PathBuf::from("/tmp/config"),
                PathBuf::from("/tmp/state"),
                PathBuf::from("/tmp/cache"),
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
                ],
                auth_timeout_secs: 120,
            },
            refresh: RefreshConfig {
                personal_interval_secs: 3_600,
                daily_interval_secs: 300,
                heartrate_interval_secs: 60,
                personal_stale_after_secs: 72 * 60 * 60,
                daily_stale_after_secs: 12 * 60 * 60,
                heartrate_stale_after_secs: 15 * 60,
                daily_history_days: 90,
                daily_overlap_days: 2,
                heartrate_history_days: 7,
                heartrate_overlap_minutes: 60,
                max_backoff_secs: 60 * 60,
                demo_fixture_dir: None,
            },
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
            ],
        };

        super::advance_dry_run_sync_states(&config, &mut sync_states, &report);

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
                fixture_dir: Some(PathBuf::from(
                    "/home/ubuntu/ringmaster.rs/tests/fixtures/phase1",
                )),
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
                fixture_dir: Some(PathBuf::from(
                    "/home/ubuntu/ringmaster.rs/tests/fixtures/phase1",
                )),
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
}
