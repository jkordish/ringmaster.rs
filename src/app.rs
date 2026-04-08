use crate::action::Action;
use crate::config::Config;
use crate::oura::models::{AuthStatus, CapabilityReport};
use crate::store::Store;
use crate::store::queries::{
    DailyOverviewRow, HeartRatePoint, PersonalInfoRecord, RecordCounts, SyncStateRecord,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Live,
    Demo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    Timeline,
    Trends,
    Ops,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub mode: RunMode,
    pub active_screen: Screen,
    pub model: AppModel,
    pub status_line: String,
    pub tick_count: u64,
    pub should_quit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppModel {
    pub title: String,
    pub dashboard: DashboardModel,
    pub timeline: TimelineModel,
    pub trends: TrendsModel,
    pub ops: OpsModel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardModel {
    pub scores: Vec<ScoreCard>,
    pub freshness: String,
    pub capabilities: Vec<CapabilityView>,
    pub change_summary: String,
    pub highlights: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineModel {
    pub summary: String,
    pub heart_rate: Vec<TimelinePoint>,
    pub overlays: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrendsModel {
    pub windows: Vec<TrendWindow>,
    pub sparkline: Vec<u64>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsModel {
    pub mode_label: String,
    pub items: Vec<OpsItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreCard {
    pub label: &'static str,
    pub value: String,
    pub subtitle: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityView {
    pub label: &'static str,
    pub available: bool,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelinePoint {
    pub label: String,
    pub bpm: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrendWindow {
    pub label: &'static str,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsItem {
    pub label: &'static str,
    pub value: String,
}

impl AppState {
    pub fn handle(&mut self, action: Action) {
        match action {
            Action::Tick => {
                self.tick_count = self.tick_count.saturating_add(1);
            }
            Action::Quit => {
                self.should_quit = true;
            }
            Action::NextScreen => {
                self.active_screen = self.active_screen.next();
                self.status_line = format!("Switched to {}", self.active_screen.title());
            }
            Action::PreviousScreen => {
                self.active_screen = self.active_screen.previous();
                self.status_line = format!("Switched to {}", self.active_screen.title());
            }
            Action::ShowScreen(screen) => {
                self.active_screen = screen;
                self.status_line = format!("Switched to {}", self.active_screen.title());
            }
            Action::RefreshRequested => {
                self.status_line = match self.mode {
                    RunMode::Demo => {
                        "Demo data is deterministic; refresh leaves the snapshot unchanged."
                            .to_owned()
                    }
                    RunMode::Live => {
                        "Local data stays read-only in the UI; run `ringmaster sync once` to refresh."
                            .to_owned()
                    }
                };
            }
        }
    }

    pub fn footer(&self) -> String {
        let spinner = match self.tick_count % 4 {
            0 => "·",
            1 => "o",
            2 => "O",
            _ => "o",
        };

        format!(
            "{spinner} {} | q quit | tab/shift-tab cycle | 1-4 jump | r refresh",
            self.status_line
        )
    }

    pub fn active_tab_index(&self) -> usize {
        self.active_screen.index()
    }
}

impl Screen {
    pub const ALL: [Screen; 4] = [
        Screen::Dashboard,
        Screen::Timeline,
        Screen::Trends,
        Screen::Ops,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Timeline => "Timeline",
            Self::Trends => "Trends",
            Self::Ops => "Ops",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Dashboard => 0,
            Self::Timeline => 1,
            Self::Trends => 2,
            Self::Ops => 3,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Dashboard => Self::Timeline,
            Self::Timeline => Self::Trends,
            Self::Trends => Self::Ops,
            Self::Ops => Self::Dashboard,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Dashboard => Self::Ops,
            Self::Timeline => Self::Dashboard,
            Self::Trends => Self::Timeline,
            Self::Ops => Self::Trends,
        }
    }
}

pub fn build_live_state(
    config: &Config,
    store: &Store,
    auth_status: &AuthStatus,
) -> crate::error::Result<AppState> {
    let latest_daily = store.views().latest_daily_overview()?;
    let personal_info = store.views().latest_personal_info()?;
    let heart_rate = store.views().recent_heartrate(32)?;
    let record_counts = store.views().record_counts()?;
    let sync_states = store.sync_state().list()?;
    let schema_version = store.metadata().schema_version()?;
    let capability_report = auth_status.capability_report.clone();

    Ok(AppState {
        mode: RunMode::Live,
        active_screen: Screen::Dashboard,
        model: AppModel {
            title: "ringmaster.rs".to_owned(),
            dashboard: build_live_dashboard(
                latest_daily.as_ref(),
                sync_states.as_slice(),
                &capability_report,
            ),
            timeline: build_live_timeline(heart_rate, sync_states.as_slice(), &capability_report),
            trends: build_live_trends(
                latest_daily.as_ref(),
                &record_counts,
                sync_states.as_slice(),
                &capability_report,
            ),
            ops: build_live_ops(
                config,
                store,
                auth_status,
                personal_info.as_ref(),
                sync_states.as_slice(),
                schema_version,
                record_counts,
            ),
        },
        status_line: "Live mode is reading from the local store.".to_owned(),
        tick_count: 0,
        should_quit: false,
    })
}

pub fn build_demo_state(config: &Config) -> AppState {
    let capability_report = CapabilityReport::demo();

    AppState {
        mode: RunMode::Demo,
        active_screen: Screen::Dashboard,
        model: AppModel {
            title: "ringmaster.rs demo".to_owned(),
            dashboard: DashboardModel {
                scores: vec![
                    ScoreCard {
                        label: "Sleep",
                        value: "86".to_owned(),
                        subtitle: "7d baseline +2".to_owned(),
                    },
                    ScoreCard {
                        label: "Readiness",
                        value: "79".to_owned(),
                        subtitle: "30d baseline -1".to_owned(),
                    },
                    ScoreCard {
                        label: "Activity",
                        value: "72".to_owned(),
                        subtitle: "90d baseline +4".to_owned(),
                    },
                ],
                freshness: "Demo snapshot: synced 4m ago".to_owned(),
                capabilities: capability_views(&capability_report),
                change_summary: "HRV is softer than your usual range; a late workout and short sleep are the most likely contributors."
                    .to_owned(),
                highlights: vec![
                    "Sleep timing slipped 42m later than your 30d norm.".to_owned(),
                    "Resting heart rate rebounded after yesterday's travel day.".to_owned(),
                    "No network or credentials are required in demo mode.".to_owned(),
                ],
            },
            timeline: TimelineModel {
                summary: "24 deterministic heartrate samples with overlay markers".to_owned(),
                heart_rate: vec![
                    point("06:00", 58),
                    point("06:30", 57),
                    point("07:00", 59),
                    point("07:30", 62),
                    point("08:00", 66),
                    point("08:30", 71),
                    point("09:00", 76),
                    point("09:30", 74),
                    point("10:00", 70),
                    point("10:30", 68),
                    point("11:00", 72),
                    point("11:30", 78),
                ],
                overlays: vec![
                    "07:30 workout: steady run".to_owned(),
                    "09:45 tag: espresso".to_owned(),
                    "22:15 session: wind-down".to_owned(),
                ],
            },
            trends: TrendsModel {
                windows: vec![
                    TrendWindow {
                        label: "7d",
                        summary: "Sleep efficiency up 3%, readiness slightly below trend".to_owned(),
                    },
                    TrendWindow {
                        label: "30d",
                        summary: "Resting heart rate stable; activity load trending upward".to_owned(),
                    },
                    TrendWindow {
                        label: "90d",
                        summary: "Readiness resilient except around travel-heavy weeks".to_owned(),
                    },
                ],
                sparkline: vec![68, 70, 72, 75, 73, 79, 81, 78, 82, 84, 80, 83],
                notes: vec![
                    "Demo trends intentionally include mixed directionality so screen states are easy to validate."
                        .to_owned(),
                    "Use the same layouts for screenshots and UI smoke tests.".to_owned(),
                ],
            },
            ops: OpsModel {
                mode_label: "Demo".to_owned(),
                items: vec![
                    ops_item("Config file", config.paths.config_file.display().to_string()),
                    ops_item("Database", config.paths.database_file.display().to_string()),
                    ops_item("Callback URL", config.oura.callback_url()),
                    ops_item("Capabilities", capability_report.available_labels().join(", ")),
                    ops_item("Log filter", config.logging.filter.clone()),
                ],
                warnings: vec![
                    "Demo mode bypasses live OAuth and store writes.".to_owned(),
                    "Webhook delivery remains intentionally deferred.".to_owned(),
                ],
            },
        },
        status_line: "Demo mode ready.".to_owned(),
        tick_count: 0,
        should_quit: false,
    }
}

fn build_live_dashboard(
    latest_daily: Option<&DailyOverviewRow>,
    sync_states: &[SyncStateRecord],
    capability_report: &CapabilityReport,
) -> DashboardModel {
    let daily_sync = sync_states
        .iter()
        .find(|record| record.sync_key == "oura.daily");
    let scores = match latest_daily {
        Some(row) => vec![
            score_card("Sleep", row.sleep_score, "Latest local daily_sleep row"),
            score_card(
                "Readiness",
                row.readiness_score,
                "Latest local daily_readiness row",
            ),
            score_card(
                "Activity",
                row.activity_score,
                "Latest local daily_activity row",
            ),
        ],
        None => vec![
            empty_score("Sleep"),
            empty_score("Readiness"),
            empty_score("Activity"),
        ],
    };

    let freshness = daily_sync
        .map(|sync| {
            format!(
                "Daily sync status: {} at {}",
                sync.status, sync.last_attempted_at
            )
        })
        .unwrap_or_else(|| "No daily sync has been recorded yet.".to_owned());

    let change_summary = match (latest_daily, daily_sync) {
        (Some(row), _) => format!(
            "Latest daily snapshot is {}. The dashboard is rendering persisted daily summaries from SQLite.",
            row.day
        ),
        (None, Some(sync)) if sync.status == crate::store::queries::SyncRunStatus::Blocked => {
            sync.message.clone().unwrap_or_else(|| {
                "Daily sync is blocked, so the dashboard is staying honest about missing data."
                    .to_owned()
            })
        }
        _ => "No Oura daily summaries are cached yet. Run `ringmaster auth login` and `ringmaster sync once` after configuration."
            .to_owned(),
    };

    let mut highlights = Vec::new();
    if !capability_report.missing_scope_names().is_empty() {
        highlights.push(format!(
            "Missing scopes keep some features unavailable: {}.",
            capability_report.missing_scope_names().join(", ")
        ));
    }
    if latest_daily.is_none() {
        highlights.push("The UI is reading from the local store only, so empty tables render as honest empty states.".to_owned());
    }
    if let Some(sync) = daily_sync
        && let Some(problem) = &sync.last_error
    {
        highlights.push(format!("Latest daily sync error: {problem}"));
    }

    DashboardModel {
        scores,
        freshness,
        capabilities: capability_views(capability_report),
        change_summary,
        highlights,
    }
}

fn build_live_timeline(
    points: Vec<HeartRatePoint>,
    sync_states: &[SyncStateRecord],
    capability_report: &CapabilityReport,
) -> TimelineModel {
    let heartrate_sync = sync_states
        .iter()
        .find(|record| record.sync_key == "oura.heartrate");
    let summary = if !capability_report.is_granted(crate::oura::models::CapabilityKind::Heartrate) {
        "Heartrate scope is missing, so the timeline is showing an explicit missing-capability state."
            .to_owned()
    } else if points.is_empty() {
        heartrate_sync
            .and_then(|sync| sync.message.clone())
            .unwrap_or_else(|| {
                "No heartrate samples are cached yet. Timeline overlays will light up after sync."
                    .to_owned()
            })
    } else {
        format!("{} heartrate samples loaded from SQLite.", points.len())
    };

    let mut overlays = vec![
        "Workout overlays remain a pure presentation layer over persisted state.".to_owned(),
        "Tag and session markers stay in the UI layer; they never call the network directly."
            .to_owned(),
    ];
    if let Some(sync) = heartrate_sync
        && let Some(problem) = &sync.last_error
    {
        overlays.insert(0, format!("Last heartrate sync error: {problem}"));
    }

    TimelineModel {
        summary,
        heart_rate: points
            .into_iter()
            .map(|point| TimelinePoint {
                label: trim_timestamp(&point.recorded_at),
                bpm: point.bpm,
            })
            .collect(),
        overlays,
    }
}

fn build_live_trends(
    latest_daily: Option<&DailyOverviewRow>,
    record_counts: &RecordCounts,
    sync_states: &[SyncStateRecord],
    capability_report: &CapabilityReport,
) -> TrendsModel {
    let heartrate_sync = sync_states
        .iter()
        .find(|record| record.sync_key == "oura.heartrate");
    let windows = if !capability_report.is_granted(crate::oura::models::CapabilityKind::Heartrate) {
        vec![
            TrendWindow {
                label: "7d",
                summary: "Heartrate scope is missing, so trend windows stay empty instead of inventing data.".to_owned(),
            },
            TrendWindow {
                label: "30d",
                summary: "Grant heartrate access to unlock the first real trend slice.".to_owned(),
            },
            TrendWindow {
                label: "90d",
                summary: "Long-range trends remain intentionally deferred until capability coverage exists.".to_owned(),
            },
        ]
    } else if let Some(row) = latest_daily {
        vec![
            TrendWindow {
                label: "7d",
                summary: format!("Latest cached day is {}. Rolling windows will populate once daily history grows.", row.day),
            },
            TrendWindow {
                label: "30d",
                summary: "Trend aggregation is intentionally simple at bootstrap time to keep storage and UI seams honest."
                    .to_owned(),
            },
            TrendWindow {
                label: "90d",
                summary: "Long-range deltas are a follow-up once sync populates more days.".to_owned(),
            },
        ]
    } else {
        vec![
            TrendWindow {
                label: "7d",
                summary: "Need cached daily rows before 7d baselines can be computed.".to_owned(),
            },
            TrendWindow {
                label: "30d",
                summary:
                    "Trend windows are ready, but the store does not have enough daily data yet."
                        .to_owned(),
            },
            TrendWindow {
                label: "90d",
                summary:
                    "Bootstrap keeps the trend view honest instead of inventing synthetic history."
                        .to_owned(),
            },
        ]
    };

    TrendsModel {
        windows,
        sparkline: vec![
            record_counts.personal_info,
            record_counts.daily_sleep,
            record_counts.daily_readiness,
            record_counts.daily_activity,
            record_counts.heartrate_samples,
        ],
        notes: vec![
            "Sparkline currently reflects cached record counts by family to prove the view pipeline."
                .to_owned(),
            "Real 7/30/90-day calculations are the next milestone once sync imports actual history."
                .to_owned(),
            heartrate_sync
                .and_then(|sync| sync.message.clone())
                .unwrap_or_else(|| "Heartrate freshness is derived from persisted sync state.".to_owned()),
        ],
    }
}

fn build_live_ops(
    config: &Config,
    store: &Store,
    auth_status: &AuthStatus,
    personal_info: Option<&PersonalInfoRecord>,
    sync_states: &[SyncStateRecord],
    schema_version: u32,
    record_counts: RecordCounts,
) -> OpsModel {
    let sync_summary = if sync_states.is_empty() {
        "never".to_owned()
    } else {
        sync_states
            .iter()
            .map(|sync| {
                format!(
                    "{}={} @ {}",
                    sync.sync_key, sync.status, sync.last_attempted_at
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    };
    let capability_summary = if auth_status.granted_scopes.is_empty() {
        "none granted yet".to_owned()
    } else {
        auth_status.granted_scopes.join(", ")
    };

    let mut warnings = Vec::new();
    if !auth_status.configured {
        warnings.push(
            "OAuth client credentials are missing, so live auth and sync remain blocked."
                .to_owned(),
        );
    }
    if auth_status.granted_scopes.is_empty() && auth_status.configured {
        warnings.push(
            "Granted scopes are empty, so capability coverage is partial by design.".to_owned(),
        );
    }
    if let Some(problem) = &auth_status.last_error {
        warnings.push(format!("Auth/session warning: {problem}"));
    }
    warnings.extend(sync_states.iter().filter_map(|sync| {
        sync.last_error
            .as_ref()
            .map(|problem| format!("{} error: {problem}", sync.sync_key))
    }));

    OpsModel {
        mode_label: "Live".to_owned(),
        items: vec![
            ops_item(
                "Config file",
                path_with_presence(
                    &config.paths.config_file,
                    config.paths.config_file_present(),
                ),
            ),
            ops_item("State dir", config.paths.state_dir.display().to_string()),
            ops_item(
                "Config path",
                config.paths.config_file.display().to_string(),
            ),
            ops_item(
                "Database",
                path_with_presence(&config.paths.database_file, config.paths.database_present()),
            ),
            ops_item("Schema version", schema_version.to_string()),
            ops_item(
                "Auth state",
                if auth_status.access_token_stored || auth_status.refresh_token_stored {
                    "authenticated".to_owned()
                } else if auth_status.configured {
                    "configured_without_session".to_owned()
                } else {
                    "unconfigured".to_owned()
                },
            ),
            ops_item("Secret backend", auth_status.secret_backend.clone()),
            ops_item("Last sync", sync_summary),
            ops_item("Granted scopes", capability_summary),
            ops_item(
                "Requested scopes",
                if auth_status.requested_scopes.is_empty() {
                    "none".to_owned()
                } else {
                    auth_status.requested_scopes.join(", ")
                },
            ),
            ops_item(
                "Access token expiry",
                auth_status
                    .access_token_expires_at
                    .clone()
                    .unwrap_or_else(|| "unknown".to_owned()),
            ),
            ops_item(
                "Last auth refresh",
                auth_status
                    .last_refresh_at
                    .clone()
                    .unwrap_or_else(|| "never".to_owned()),
            ),
            ops_item(
                "Account",
                auth_status
                    .account_email
                    .clone()
                    .or_else(|| personal_info.and_then(|profile| profile.email.clone()))
                    .or_else(|| auth_status.account_id.clone())
                    .unwrap_or_else(|| "unknown".to_owned()),
            ),
            ops_item(
                "Record counts",
                format!(
                    "profile={} daily={} hr={} workouts={} tags={} enhanced_tags={} sessions={} raw={}",
                    record_counts.personal_info,
                    record_counts.daily_sleep
                        + record_counts.daily_readiness
                        + record_counts.daily_activity,
                    record_counts.heartrate_samples,
                    record_counts.workouts,
                    record_counts.tags,
                    record_counts.enhanced_tags,
                    record_counts.sessions,
                    record_counts.raw_payloads,
                ),
            ),
            ops_item("Callback URL", auth_status.callback_url.clone()),
            ops_item("Database file", store.plan().db_path.display().to_string()),
        ],
        warnings,
    }
}

fn capability_views(report: &CapabilityReport) -> Vec<CapabilityView> {
    report
        .entries
        .iter()
        .map(|entry| CapabilityView {
            label: entry.kind.label(),
            available: entry.granted,
            note: entry.note.clone(),
        })
        .collect()
}

fn score_card(label: &'static str, value: Option<u8>, subtitle: &'static str) -> ScoreCard {
    ScoreCard {
        label,
        value: value
            .map(|score| score.to_string())
            .unwrap_or_else(|| "--".to_owned()),
        subtitle: subtitle.to_owned(),
    }
}

fn empty_score(label: &'static str) -> ScoreCard {
    ScoreCard {
        label,
        value: "--".to_owned(),
        subtitle: "awaiting local sync".to_owned(),
    }
}

fn point(label: &str, bpm: u16) -> TimelinePoint {
    TimelinePoint {
        label: label.to_owned(),
        bpm,
    }
}

fn ops_item(label: &'static str, value: String) -> OpsItem {
    OpsItem { label, value }
}

fn path_with_presence(path: &std::path::Path, present: bool) -> String {
    format!(
        "{} ({})",
        path.display(),
        if present { "present" } else { "missing" }
    )
}

fn trim_timestamp(value: &str) -> String {
    if value.len() >= 16 {
        value.chars().skip(11).take(5).collect()
    } else {
        value.to_owned()
    }
}
