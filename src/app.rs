use crate::action::Action;
use crate::config::Config;
use crate::insights::{InsightConfidence, MetricInsight, MetricPoint, build_metric_insight};
use crate::oura::models::{AuthStatus, CapabilityKind, CapabilityReport};
use crate::refresh::SyncFamily;
use crate::store::Store;
use crate::store::queries::{
    DailyOverviewRow, HeartRatePoint, PersonalInfoRecord, RecordCounts, SyncRunStatus,
    SyncStateRecord,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataFamily {
    Personal,
    Daily,
    Heartrate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreshnessKind {
    Fresh,
    Stale,
    NoDataYet,
    NeverSynced,
    MissingScope,
    AuthFailure,
    SourceDelayed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreshnessState {
    pub family: DataFamily,
    pub kind: FreshnessKind,
    pub summary: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveSnapshot {
    pub captured_at: String,
    pub refresh_policy: RefreshPolicySnapshot,
    pub auth_status: AuthStatus,
    pub personal_info: Option<PersonalInfoRecord>,
    pub daily_history: Vec<DailyOverviewRow>,
    pub heartrate_days: Vec<HeartRateDay>,
    pub heartrate_daily_averages: Vec<MetricPoint>,
    pub sync_states: Vec<SyncStateRecord>,
    pub record_counts: RecordCounts,
    pub schema_version: u32,
    pub database_path: String,
    pub config_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshPolicySnapshot {
    pub personal_interval_secs: u64,
    pub daily_interval_secs: u64,
    pub heartrate_interval_secs: u64,
    pub personal_stale_after_secs: u64,
    pub daily_stale_after_secs: u64,
    pub heartrate_stale_after_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartRateDay {
    pub day: String,
    pub points: Vec<HeartRatePoint>,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendWindowKind {
    Days7,
    Days30,
    Days90,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppState {
    pub mode: RunMode,
    pub active_screen: Screen,
    pub model: AppModel,
    pub status_line: String,
    pub tick_count: u64,
    pub should_quit: bool,
    pub refresh_in_flight: bool,
    live_snapshot: Option<LiveSnapshot>,
    timeline_selected_day: usize,
    timeline_selected_point: usize,
    timeline_window_hours: u16,
    trends_window: TrendWindowKind,
}

#[derive(Debug, Clone, PartialEq)]
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
    pub day_labels: Vec<String>,
    pub selected_day_index: usize,
    pub selected_point_index: Option<usize>,
    pub window_hours: u16,
    pub selected_detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrendsModel {
    pub windows: Vec<TrendWindow>,
    pub selected_window_index: usize,
    pub metrics: Vec<TrendMetricView>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsModel {
    pub mode_label: String,
    pub family_statuses: Vec<FamilyStatusView>,
    pub items: Vec<OpsItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreCard {
    pub label: &'static str,
    pub value: String,
    pub badge: String,
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
    pub recorded_at: String,
    pub bpm: u16,
    pub minute_of_day: u16,
    pub gap_before: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrendWindow {
    pub label: &'static str,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrendMetricView {
    pub label: &'static str,
    pub current_value: String,
    pub summary: String,
    pub sparkline: Vec<u64>,
    pub confidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyStatusView {
    pub label: &'static str,
    pub state_label: String,
    pub scope_label: String,
    pub last_sync: String,
    pub detail: String,
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
                        "Demo data is deterministic; refresh keeps the current snapshot.".to_owned()
                    }
                    RunMode::Live => "Manual refresh requested.".to_owned(),
                };
            }
            Action::RefreshStarted { families, manual } => {
                self.refresh_in_flight = true;
                let prefix = if manual {
                    "Manual refresh"
                } else {
                    "Background refresh"
                };
                self.status_line = format!("{prefix} started for {}.", families.join(", "));
                self.rebuild_live_model();
            }
            Action::LiveSnapshotLoaded { snapshot, summary } => {
                self.refresh_in_flight = false;
                self.replace_live_snapshot(*snapshot);
                self.status_line = summary;
            }
            Action::RefreshFailed { message } => {
                self.refresh_in_flight = false;
                self.status_line = message;
                self.rebuild_live_model();
            }
            Action::OlderTimelineDay => {
                if self.timeline_selected_day + 1 < self.timeline_day_count() {
                    self.timeline_selected_day += 1;
                    self.timeline_selected_point = 0;
                    "Showing an older heartrate day.".clone_into(&mut self.status_line);
                    self.rebuild_live_model();
                }
            }
            Action::NewerTimelineDay => {
                if self.timeline_selected_day > 0 {
                    self.timeline_selected_day -= 1;
                    self.timeline_selected_point = 0;
                    "Showing a newer heartrate day.".clone_into(&mut self.status_line);
                    self.rebuild_live_model();
                }
            }
            Action::PreviousTimelinePoint => {
                if self.timeline_selected_point > 0 {
                    self.timeline_selected_point -= 1;
                    "Moved to an earlier heartrate point.".clone_into(&mut self.status_line);
                    self.rebuild_live_model();
                }
            }
            Action::NextTimelinePoint => {
                let max_index = self.visible_timeline_points().saturating_sub(1);
                if self.timeline_selected_point < max_index {
                    self.timeline_selected_point += 1;
                    "Moved to a later heartrate point.".clone_into(&mut self.status_line);
                    self.rebuild_live_model();
                }
            }
            Action::TimelineZoomIn => {
                self.timeline_window_hours = match self.timeline_window_hours {
                    24 => 12,
                    12 => 6,
                    _ => 6,
                };
                self.status_line =
                    format!("Timeline window set to {}h.", self.timeline_window_hours);
                self.rebuild_live_model();
            }
            Action::TimelineZoomOut => {
                self.timeline_window_hours = match self.timeline_window_hours {
                    6 => 12,
                    12 => 24,
                    _ => 24,
                };
                self.status_line =
                    format!("Timeline window set to {}h.", self.timeline_window_hours);
                self.rebuild_live_model();
            }
            Action::PreviousTrendWindow => {
                self.trends_window = self.trends_window.previous();
                self.status_line =
                    format!("Trend window changed to {}.", self.trends_window.label());
                self.rebuild_live_model();
            }
            Action::NextTrendWindow => {
                self.trends_window = self.trends_window.next();
                self.status_line =
                    format!("Trend window changed to {}.", self.trends_window.label());
                self.rebuild_live_model();
            }
        }
    }

    pub fn footer(&self) -> String {
        let spinner = ["·", "o", "O", "o"][(self.tick_count % 4) as usize];
        let screen_hint = match self.active_screen {
            Screen::Timeline => "[ ] day | , . point | -/= zoom",
            Screen::Trends => "[ ] window",
            _ => "tab/shift-tab cycle | 1-4 jump",
        };
        let refresh_hint = if self.refresh_in_flight {
            "refreshing"
        } else {
            "r refresh"
        };

        format!(
            "{spinner} {} | {} | {} | q quit",
            self.status_line, screen_hint, refresh_hint
        )
    }

    pub fn active_tab_index(&self) -> usize {
        self.active_screen.index()
    }

    fn replace_live_snapshot(&mut self, snapshot: LiveSnapshot) {
        let previous_day = self
            .live_snapshot
            .as_ref()
            .and_then(|current| current.heartrate_days.get(self.timeline_selected_day))
            .map(|day| day.day.clone());
        self.live_snapshot = Some(snapshot);

        if let Some(snapshot) = &self.live_snapshot {
            self.timeline_selected_day = previous_day
                .as_deref()
                .and_then(|selected_day| {
                    snapshot
                        .heartrate_days
                        .iter()
                        .position(|day| day.day == selected_day)
                })
                .unwrap_or_else(|| snapshot.heartrate_days.len().saturating_sub(1));
            self.timeline_selected_point = 0;
        }

        self.rebuild_live_model();
    }

    fn rebuild_live_model(&mut self) {
        if let Some(snapshot) = &self.live_snapshot {
            self.model = build_live_model(
                snapshot,
                self.timeline_selected_day,
                self.timeline_selected_point,
                self.timeline_window_hours,
                self.trends_window,
                self.refresh_in_flight,
            );
        }
    }

    fn timeline_day_count(&self) -> usize {
        self.live_snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.heartrate_days.len())
    }

    fn visible_timeline_points(&self) -> usize {
        self.model.timeline.heart_rate.len()
    }
}

impl Screen {
    pub const ALL: [Self; 4] = [Self::Dashboard, Self::Timeline, Self::Trends, Self::Ops];

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

impl TrendWindowKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Days7 => "7d",
            Self::Days30 => "30d",
            Self::Days90 => "90d",
        }
    }

    pub fn days(self) -> usize {
        match self {
            Self::Days7 => 7,
            Self::Days30 => 30,
            Self::Days90 => 90,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Days7 => Self::Days30,
            Self::Days30 => Self::Days90,
            Self::Days90 => Self::Days7,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Days7 => Self::Days90,
            Self::Days30 => Self::Days7,
            Self::Days90 => Self::Days30,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Days7 => 0,
            Self::Days30 => 1,
            Self::Days90 => 2,
        }
    }
}

pub fn build_live_state(
    config: &Config,
    store: &Store,
    auth_status: &AuthStatus,
) -> crate::error::Result<AppState> {
    let snapshot = load_live_snapshot(config, store, auth_status)?;
    let model = build_live_model(&snapshot, 0, 0, 24, TrendWindowKind::Days7, false);

    Ok(AppState {
        mode: RunMode::Live,
        active_screen: Screen::Dashboard,
        model,
        status_line: "Live mode is reading from the local store.".to_owned(),
        tick_count: 0,
        should_quit: false,
        refresh_in_flight: false,
        live_snapshot: Some(snapshot),
        timeline_selected_day: 0,
        timeline_selected_point: 0,
        timeline_window_hours: 24,
        trends_window: TrendWindowKind::Days7,
    })
}

pub fn load_live_snapshot(
    config: &Config,
    store: &Store,
    auth_status: &AuthStatus,
) -> crate::error::Result<LiveSnapshot> {
    let daily_history = store
        .views()
        .daily_history(usize::from(config.refresh.daily_history_days))?;
    let heartrate_days = load_heartrate_days(store, 14)?;
    let heartrate_daily_averages = load_heartrate_daily_averages(store, 90)?;

    Ok(LiveSnapshot {
        captured_at: now_rfc3339(),
        refresh_policy: RefreshPolicySnapshot::from_config(config),
        auth_status: auth_status.clone(),
        personal_info: store.views().latest_personal_info()?,
        daily_history,
        heartrate_days,
        heartrate_daily_averages,
        sync_states: store.sync_state().list()?,
        record_counts: store.views().record_counts()?,
        schema_version: store.metadata().schema_version()?,
        database_path: store.plan().db_path.display().to_string(),
        config_path: config.paths.config_file.display().to_string(),
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
                        badge: "fresh".to_owned(),
                        subtitle: "7d baseline +2.0".to_owned(),
                    },
                    ScoreCard {
                        label: "Readiness",
                        value: "79".to_owned(),
                        badge: "fresh".to_owned(),
                        subtitle: "30d baseline -1.0".to_owned(),
                    },
                    ScoreCard {
                        label: "Activity",
                        value: "72".to_owned(),
                        badge: "source delayed".to_owned(),
                        subtitle: "30d baseline +4.0".to_owned(),
                    },
                ],
                freshness: "Daily fresh | Heartrate fresh | Personal fresh".to_owned(),
                capabilities: capability_views(&capability_report),
                change_summary:
                    "Today vs baseline: sleep is above normal, readiness is close to normal, and activity is waiting on Oura's daily closeout."
                        .to_owned(),
                highlights: vec![
                    "Sleep started 42m later than your 30d norm.".to_owned(),
                    "Average heartrate is 3 bpm above your recent range.".to_owned(),
                    "Demo mode shows how stale and delayed states render without network access."
                        .to_owned(),
                ],
            },
            timeline: TimelineModel {
                summary: "Demo heartrate timeline for 2026-04-08 | 24h window | fresh".to_owned(),
                heart_rate: vec![
                    timeline_point("06:00", "2026-04-08T06:00:00Z", 58, false),
                    timeline_point("06:30", "2026-04-08T06:30:00Z", 57, false),
                    timeline_point("07:00", "2026-04-08T07:00:00Z", 59, false),
                    timeline_point("07:45", "2026-04-08T07:45:00Z", 66, true),
                    timeline_point("08:15", "2026-04-08T08:15:00Z", 71, false),
                    timeline_point("08:45", "2026-04-08T08:45:00Z", 76, false),
                    timeline_point("09:15", "2026-04-08T09:15:00Z", 74, false),
                ],
                overlays: vec![
                    "Days: 2026-04-07 | [2026-04-08]".to_owned(),
                    "Selected point: 2026-04-08T09:15:00Z at 74 bpm.".to_owned(),
                    "Source legend: current MVP stores bpm and timestamps, but not per-sample source labels.".to_owned(),
                ],
                day_labels: vec!["2026-04-07".to_owned(), "2026-04-08".to_owned()],
                selected_day_index: 1,
                selected_point_index: Some(6),
                window_hours: 24,
                selected_detail: "2026-04-08T09:15:00Z · 74 bpm".to_owned(),
            },
            trends: TrendsModel {
                windows: vec![
                    TrendWindow {
                        label: "7d",
                        summary: "Short view emphasizing daily swings and 7d baselines.".to_owned(),
                    },
                    TrendWindow {
                        label: "30d",
                        summary: "Monthly view stabilizing the baseline comparison.".to_owned(),
                    },
                    TrendWindow {
                        label: "90d",
                        summary: "Long view showing seasonality while still comparing against 30d baselines.".to_owned(),
                    },
                ],
                selected_window_index: 0,
                metrics: vec![
                    TrendMetricView {
                        label: "Sleep",
                        current_value: "86".to_owned(),
                        summary: "Above your 7d baseline by 2.0 points.".to_owned(),
                        sparkline: vec![79, 81, 80, 83, 82, 84, 86],
                        confidence: "confidence: medium".to_owned(),
                    },
                    TrendMetricView {
                        label: "Readiness",
                        current_value: "79".to_owned(),
                        summary: "Close to your 7d baseline with a small day-over-day dip.".to_owned(),
                        sparkline: vec![78, 79, 80, 81, 80, 79, 79],
                        confidence: "confidence: medium".to_owned(),
                    },
                    TrendMetricView {
                        label: "Activity",
                        current_value: "72".to_owned(),
                        summary: "Daily closeout is still pending, so today's activity is provisional."
                            .to_owned(),
                        sparkline: vec![65, 68, 70, 72, 74, 73, 72],
                        confidence: "confidence: medium".to_owned(),
                    },
                    TrendMetricView {
                        label: "Heartrate",
                        current_value: "68.7".to_owned(),
                        summary: "Average heartrate is slightly above your recent range.".to_owned(),
                        sparkline: vec![64, 65, 65, 67, 68, 69, 69],
                        confidence: "confidence: thin".to_owned(),
                    },
                ],
                notes: vec![
                    "Demo trends use deterministic values so screen tests can assert real layouts."
                        .to_owned(),
                    "Insight text stays descriptive and avoids causal claims.".to_owned(),
                ],
            },
            ops: OpsModel {
                mode_label: "Demo".to_owned(),
                family_statuses: vec![
                    FamilyStatusView {
                        label: "Personal",
                        state_label: "fresh".to_owned(),
                        scope_label: "scope granted".to_owned(),
                        last_sync: "2026-04-08T03:55:00Z".to_owned(),
                        detail: "Profile data is current in the demo snapshot.".to_owned(),
                    },
                    FamilyStatusView {
                        label: "Daily",
                        state_label: "source delayed".to_owned(),
                        scope_label: "scope granted".to_owned(),
                        last_sync: "2026-04-08T03:58:00Z".to_owned(),
                        detail: "Oura daily closeout can lag behind real time.".to_owned(),
                    },
                    FamilyStatusView {
                        label: "Heartrate",
                        state_label: "fresh".to_owned(),
                        scope_label: "scope granted".to_owned(),
                        last_sync: "2026-04-08T03:59:00Z".to_owned(),
                        detail: "Intraday heartrate is ready for the timeline.".to_owned(),
                    },
                ],
                items: vec![
                    ops_item("Config file", config.paths.config_file.display().to_string()),
                    ops_item("Database", config.paths.database_file.display().to_string()),
                    ops_item("Callback URL", config.oura.callback_url()),
                    ops_item("Refresh policy", "personal=3600s daily=300s heartrate=60s".to_owned()),
                    ops_item("Capabilities", capability_report.available_labels().join(", ")),
                ],
                warnings: vec![
                    "Demo mode bypasses live OAuth and background sync.".to_owned(),
                    "Webhook invalidation remains intentionally deferred.".to_owned(),
                ],
            },
        },
        status_line: "Demo mode ready.".to_owned(),
        tick_count: 0,
        should_quit: false,
        refresh_in_flight: false,
        live_snapshot: None,
        timeline_selected_day: 0,
        timeline_selected_point: 0,
        timeline_window_hours: 24,
        trends_window: TrendWindowKind::Days7,
    }
}

fn build_live_model(
    snapshot: &LiveSnapshot,
    selected_day_index: usize,
    selected_point_index: usize,
    window_hours: u16,
    trends_window: TrendWindowKind,
    refresh_in_flight: bool,
) -> AppModel {
    AppModel {
        title: "ringmaster.rs".to_owned(),
        dashboard: build_dashboard_model(snapshot, refresh_in_flight),
        timeline: build_timeline_model(
            snapshot,
            selected_day_index,
            selected_point_index,
            window_hours,
        ),
        trends: build_trends_model(snapshot, trends_window),
        ops: build_ops_model(snapshot, refresh_in_flight),
    }
}

fn build_dashboard_model(snapshot: &LiveSnapshot, refresh_in_flight: bool) -> DashboardModel {
    let latest_daily = snapshot.daily_history.last();
    let sleep_insight = build_metric_insight(
        "sleep",
        &metric_points_from_daily(&snapshot.daily_history, |row| {
            row.sleep_score.map(f64::from)
        }),
    );
    let readiness_insight = build_metric_insight(
        "readiness",
        &metric_points_from_daily(&snapshot.daily_history, |row| {
            row.readiness_score.map(f64::from)
        }),
    );
    let activity_insight = build_metric_insight(
        "activity",
        &metric_points_from_daily(&snapshot.daily_history, |row| {
            row.activity_score.map(f64::from)
        }),
    );
    let heartrate_insight = build_metric_insight("heartrate", &snapshot.heartrate_daily_averages);

    let daily_freshness = family_freshness(snapshot, DataFamily::Daily);
    let heartrate_freshness = family_freshness(snapshot, DataFamily::Heartrate);
    let personal_freshness = family_freshness(snapshot, DataFamily::Personal);

    let scores = vec![
        score_card(
            "Sleep",
            latest_daily.and_then(|row| row.sleep_score),
            freshness_badge(&daily_freshness),
            metric_subtitle(&sleep_insight),
        ),
        score_card(
            "Readiness",
            latest_daily.and_then(|row| row.readiness_score),
            freshness_badge(&daily_freshness),
            metric_subtitle(&readiness_insight),
        ),
        score_card(
            "Activity",
            latest_daily.and_then(|row| row.activity_score),
            freshness_badge(&daily_freshness),
            metric_subtitle(&activity_insight),
        ),
    ];

    let freshness = [
        format!("Daily {}", freshness_badge(&daily_freshness)),
        format!("Heartrate {}", freshness_badge(&heartrate_freshness)),
        format!("Personal {}", freshness_badge(&personal_freshness)),
    ]
    .join(" | ");

    let today_vs_baseline = [
        short_baseline_phrase("sleep", &sleep_insight),
        short_baseline_phrase("readiness", &readiness_insight),
        short_baseline_phrase("activity", &activity_insight),
    ]
    .join(" ");

    let mut highlights = vec![
        sleep_insight.summary,
        readiness_insight.summary,
        activity_insight.summary,
    ];
    if snapshot.heartrate_daily_averages.is_empty() {
        highlights.push(heartrate_freshness.detail);
    } else {
        highlights.push(heartrate_insight.summary);
    }
    if refresh_in_flight {
        highlights.insert(
            0,
            "Background refresh is running; the screen stays on persisted data until the next snapshot lands."
                .to_owned(),
        );
    }

    DashboardModel {
        scores,
        freshness,
        capabilities: capability_views(&snapshot.auth_status.capability_report),
        change_summary: today_vs_baseline,
        highlights,
    }
}

fn build_timeline_model(
    snapshot: &LiveSnapshot,
    selected_day_index: usize,
    selected_point_index: usize,
    window_hours: u16,
) -> TimelineModel {
    let freshness = family_freshness(snapshot, DataFamily::Heartrate);
    let day_labels = snapshot
        .heartrate_days
        .iter()
        .map(|day| day.day.clone())
        .collect::<Vec<_>>();
    let clamped_day_index = if day_labels.is_empty() {
        0
    } else {
        usize::min(selected_day_index, day_labels.len() - 1)
    };
    let selected_day = snapshot.heartrate_days.get(clamped_day_index);
    let visible_points = selected_day
        .map(|day| visible_timeline_points(day, window_hours))
        .unwrap_or_default();
    let selected_point_index = if visible_points.is_empty() {
        None
    } else {
        Some(usize::min(selected_point_index, visible_points.len() - 1))
    };
    let selected_detail = selected_point_index
        .and_then(|index| visible_points.get(index))
        .map(|point| format!("Selected: {} at {} bpm", point.recorded_at, point.bpm))
        .unwrap_or_else(|| freshness.detail.clone());
    let selected_day_label = selected_day
        .map(|day| day.day.clone())
        .unwrap_or_else(|| "no heartrate day selected".to_owned());
    let summary = format!(
        "Heartrate timeline for {} | {}h window | {}",
        selected_day_label, window_hours, freshness.summary
    );
    let overlays =
        vec![
        format!("Days: {}", format_day_selector(&day_labels, clamped_day_index)),
        selected_detail.clone(),
        "Source legend: current MVP stores bpm and timestamps, but not per-sample source labels."
            .to_owned(),
    ];

    TimelineModel {
        summary,
        heart_rate: visible_points,
        overlays,
        day_labels,
        selected_day_index: clamped_day_index,
        selected_point_index,
        window_hours,
        selected_detail,
    }
}

fn build_trends_model(snapshot: &LiveSnapshot, trends_window: TrendWindowKind) -> TrendsModel {
    let sleep_points = metric_points_from_daily(&snapshot.daily_history, |row| {
        row.sleep_score.map(f64::from)
    });
    let readiness_points = metric_points_from_daily(&snapshot.daily_history, |row| {
        row.readiness_score.map(f64::from)
    });
    let activity_points = metric_points_from_daily(&snapshot.daily_history, |row| {
        row.activity_score.map(f64::from)
    });
    let heartrate_points = snapshot.heartrate_daily_averages.clone();

    let sleep_insight = build_metric_insight("sleep", &sleep_points);
    let readiness_insight = build_metric_insight("readiness", &readiness_points);
    let activity_insight = build_metric_insight("activity", &activity_points);
    let heartrate_insight = build_metric_insight("heartrate", &heartrate_points);

    let windows = vec![
        TrendWindow {
            label: "7d",
            summary: "Short view for day-to-day movement and 7d baselines.".to_owned(),
        },
        TrendWindow {
            label: "30d",
            summary: "Monthly view smoothing daily noise against 30d baselines.".to_owned(),
        },
        TrendWindow {
            label: "90d",
            summary: "Long view showing history while still comparing against recent baselines."
                .to_owned(),
        },
    ];

    TrendsModel {
        windows,
        selected_window_index: trends_window.index(),
        metrics: vec![
            build_trend_metric("Sleep", &sleep_points, &sleep_insight, trends_window),
            build_trend_metric(
                "Readiness",
                &readiness_points,
                &readiness_insight,
                trends_window,
            ),
            build_trend_metric(
                "Activity",
                &activity_points,
                &activity_insight,
                trends_window,
            ),
            build_trend_metric(
                "Heartrate",
                &heartrate_points,
                &heartrate_insight,
                trends_window,
            ),
        ],
        notes: trend_notes(
            trends_window,
            [
                &sleep_insight,
                &readiness_insight,
                &activity_insight,
                &heartrate_insight,
            ],
        ),
    }
}

fn build_ops_model(snapshot: &LiveSnapshot, refresh_in_flight: bool) -> OpsModel {
    let family_statuses = [
        DataFamily::Personal,
        DataFamily::Daily,
        DataFamily::Heartrate,
    ]
    .into_iter()
    .map(|family| {
        let freshness = family_freshness(snapshot, family);
        let sync_state = sync_state_for(&snapshot.sync_states, family);
        let scope_label = snapshot
            .auth_status
            .capability_report
            .status_for(family.capability_kind())
            .map_or_else(
                || "scope unknown".to_owned(),
                |entry| {
                    if entry.granted {
                        "scope granted".to_owned()
                    } else if entry.requested {
                        "scope missing".to_owned()
                    } else {
                        "scope not requested".to_owned()
                    }
                },
            );

        FamilyStatusView {
            label: family.label(),
            state_label: freshness_badge(&freshness),
            scope_label,
            last_sync: sync_state.map_or_else(
                || "never".to_owned(),
                |state| {
                    state
                        .last_completed_at
                        .clone()
                        .unwrap_or_else(|| state.last_attempted_at.clone())
                },
            ),
            detail: freshness.detail,
        }
    })
    .collect::<Vec<_>>();

    let mut warnings = family_statuses
        .iter()
        .filter(|status| {
            matches!(
                status.state_label.as_str(),
                "stale" | "auth failure" | "missing scope" | "never synced" | "no data yet"
            )
        })
        .map(|status| format!("{}: {}", status.label, status.detail))
        .collect::<Vec<_>>();
    if refresh_in_flight {
        warnings.insert(
            0,
            "Background refresh is active; diagnostics update after the next persisted snapshot."
                .to_owned(),
        );
    }

    OpsModel {
        mode_label: if refresh_in_flight {
            "Live (refreshing)".to_owned()
        } else {
            "Live".to_owned()
        },
        family_statuses,
        items: vec![
            ops_item("Config path", snapshot.config_path.clone()),
            ops_item("Database path", snapshot.database_path.clone()),
            ops_item("Schema version", snapshot.schema_version.to_string()),
            ops_item("Auth state", auth_state_label(&snapshot.auth_status)),
            ops_item(
                "Granted scopes",
                if snapshot.auth_status.granted_scopes.is_empty() {
                    "none".to_owned()
                } else {
                    snapshot.auth_status.granted_scopes.join(", ")
                },
            ),
            ops_item(
                "Access token expiry",
                snapshot
                    .auth_status
                    .access_token_expires_at
                    .clone()
                    .unwrap_or_else(|| "unknown".to_owned()),
            ),
            ops_item(
                "Last auth refresh",
                snapshot
                    .auth_status
                    .last_refresh_at
                    .clone()
                    .unwrap_or_else(|| "never".to_owned()),
            ),
            ops_item(
                "Secret backend",
                snapshot.auth_status.secret_backend.clone(),
            ),
            ops_item("Refresh policy", snapshot.refresh_policy.summary()),
            ops_item(
                "Record counts",
                format!(
                    "profile={} daily={} heartrate={} raw={}",
                    snapshot.record_counts.personal_info,
                    snapshot.record_counts.daily_sleep
                        + snapshot.record_counts.daily_readiness
                        + snapshot.record_counts.daily_activity,
                    snapshot.record_counts.heartrate_samples,
                    snapshot.record_counts.raw_payloads,
                ),
            ),
        ],
        warnings,
    }
}

fn load_heartrate_days(store: &Store, limit: usize) -> crate::error::Result<Vec<HeartRateDay>> {
    let days = store.views().available_heartrate_days(limit)?;
    let mut heartrate_days = Vec::new();

    for day in days {
        heartrate_days.push(HeartRateDay {
            points: store.views().heartrate_for_day(&day)?,
            day,
        });
    }

    Ok(heartrate_days)
}

fn load_heartrate_daily_averages(
    store: &Store,
    limit: usize,
) -> crate::error::Result<Vec<MetricPoint>> {
    let days = store.views().available_heartrate_days(limit)?;
    let mut points = Vec::new();

    for day in days {
        let samples = store.views().heartrate_for_day(&day)?;
        if samples.is_empty() {
            continue;
        }

        let mean_bpm = samples
            .iter()
            .map(|point| f64::from(point.bpm))
            .sum::<f64>()
            / samples.len() as f64;
        points.push(MetricPoint {
            day,
            value: mean_bpm,
        });
    }

    Ok(points)
}

fn family_freshness(snapshot: &LiveSnapshot, family: DataFamily) -> FreshnessState {
    let capability_report = &snapshot.auth_status.capability_report;
    if !capability_report.is_granted(family.capability_kind()) {
        return FreshnessState {
            family,
            kind: FreshnessKind::MissingScope,
            summary: "missing scope".to_owned(),
            detail: format!(
                "{} scope is not granted, so the UI keeps this family unavailable.",
                family.capability_kind().scope_name()
            ),
        };
    }

    let has_data = family_has_data(snapshot, family);
    let sync_state = sync_state_for(&snapshot.sync_states, family);
    let now = parse_timestamp(&snapshot.captured_at).unwrap_or_else(OffsetDateTime::now_utc);

    let Some(sync_state) = sync_state else {
        return FreshnessState {
            family,
            kind: FreshnessKind::NeverSynced,
            summary: "never synced".to_owned(),
            detail: format!("{} has not completed a sync yet.", family.label()),
        };
    };

    if sync_state.last_error.as_ref().is_some_and(is_auth_problem) {
        return FreshnessState {
            family,
            kind: FreshnessKind::AuthFailure,
            summary: "auth failure".to_owned(),
            detail: sync_state
                .last_error
                .as_ref()
                .map_or_else(String::new, ToString::to_string),
        };
    }

    if matches!(sync_state.status, SyncRunStatus::Blocked) && !has_data {
        return FreshnessState {
            family,
            kind: FreshnessKind::NeverSynced,
            summary: "sync blocked".to_owned(),
            detail: sync_state.message.clone().unwrap_or_else(|| {
                format!("{} is waiting for auth or configuration.", family.label())
            }),
        };
    }

    let reference = sync_state
        .last_completed_at
        .as_deref()
        .or(Some(sync_state.last_attempted_at.as_str()));
    let is_fresh = reference
        .and_then(parse_timestamp)
        .map(|timestamp| {
            now - timestamp
                <= time::Duration::seconds(
                    snapshot.refresh_policy.stale_after_seconds(family) as i64
                )
        })
        .unwrap_or(false);

    if family == DataFamily::Daily
        && sync_state.status == SyncRunStatus::Success
        && latest_day_is_before_today(snapshot)
        && is_fresh
    {
        return FreshnessState {
            family,
            kind: FreshnessKind::SourceDelayed,
            summary: "source delayed".to_owned(),
            detail: "Daily closeout is still pending from Oura, so today is being compared against the latest fully available day.".to_owned(),
        };
    }

    if !has_data {
        return FreshnessState {
            family,
            kind: FreshnessKind::NoDataYet,
            summary: "no data yet".to_owned(),
            detail: sync_state.message.clone().unwrap_or_else(|| {
                format!(
                    "{} has synced, but there is nothing persisted for this family yet.",
                    family.label()
                )
            }),
        };
    }

    if is_fresh && matches!(sync_state.status, SyncRunStatus::Success) {
        FreshnessState {
            family,
            kind: FreshnessKind::Fresh,
            summary: "fresh".to_owned(),
            detail: format!(
                "{} updated at {}.",
                family.label(),
                sync_state
                    .last_completed_at
                    .clone()
                    .unwrap_or_else(|| sync_state.last_attempted_at.clone())
            ),
        }
    } else {
        FreshnessState {
            family,
            kind: FreshnessKind::Stale,
            summary: "stale".to_owned(),
            detail: sync_state.message.clone().unwrap_or_else(|| {
                format!(
                    "{} is older than its freshness window or the last refresh was partial.",
                    family.label()
                )
            }),
        }
    }
}

fn metric_points_from_daily<F>(history: &[DailyOverviewRow], mut mapper: F) -> Vec<MetricPoint>
where
    F: FnMut(&DailyOverviewRow) -> Option<f64>,
{
    history
        .iter()
        .filter_map(|row| {
            mapper(row).map(|value| MetricPoint {
                day: row.day.clone(),
                value,
            })
        })
        .collect()
}

fn metric_subtitle(insight: &MetricInsight) -> String {
    if let Some(delta) = insight.baseline_7d.delta_from_today {
        format!("7d baseline {:+.1}", delta)
    } else {
        insight
            .confidence_note
            .clone()
            .unwrap_or_else(|| "insufficient history".to_owned())
    }
}

fn short_baseline_phrase(label: &str, insight: &MetricInsight) -> String {
    if let Some(delta) = insight.baseline_7d.delta_from_today {
        let relation = if delta >= 1.0 {
            "above"
        } else if delta <= -1.0 {
            "below"
        } else {
            "close to"
        };
        format!("{label} is {relation} normal.")
    } else {
        format!("{label} is still building a baseline.")
    }
}

fn build_trend_metric(
    label: &'static str,
    history: &[MetricPoint],
    insight: &MetricInsight,
    window: TrendWindowKind,
) -> TrendMetricView {
    let current_value = insight
        .today
        .as_ref()
        .map_or_else(|| "--".to_owned(), |point| format_float(point.value));
    let baseline = match window {
        TrendWindowKind::Days7 => &insight.baseline_7d,
        TrendWindowKind::Days30 | TrendWindowKind::Days90 => &insight.baseline_30d,
    };
    let comparison = if baseline.sample_count >= 4 {
        baseline.delta_from_today.map_or_else(
            || "baseline comparison unavailable".to_owned(),
            |delta| {
                format!(
                    "{} vs {} baseline ({:+.1})",
                    window.label(),
                    if window == TrendWindowKind::Days90 {
                        "30d"
                    } else {
                        window.label()
                    },
                    delta
                )
            },
        )
    } else {
        insight
            .confidence_note
            .clone()
            .unwrap_or_else(|| "insufficient history".to_owned())
    };

    TrendMetricView {
        label,
        current_value,
        summary: comparison,
        sparkline: window_sparkline(history, window.days()),
        confidence: confidence_label(insight.confidence),
    }
}

fn trend_notes(window: TrendWindowKind, insights: [&MetricInsight; 4]) -> Vec<String> {
    let mut notes = vec![format!(
        "{} view compares recent movement against honest rolling baselines.",
        window.label()
    )];

    for insight in insights {
        if let Some(note) = &insight.confidence_note {
            notes.push(format!("{}: {}", insight.label, note));
        }
    }

    notes
}

fn visible_timeline_points(day: &HeartRateDay, window_hours: u16) -> Vec<TimelinePoint> {
    let latest_minute = day
        .points
        .last()
        .map(|point| minutes_from_timestamp(&point.recorded_at))
        .unwrap_or(0);
    let window_start = latest_minute.saturating_sub(window_hours.saturating_mul(60));
    let mut visible = Vec::new();
    let mut previous_minute = None;

    for point in &day.points {
        let minute = minutes_from_timestamp(&point.recorded_at);
        if minute < window_start {
            continue;
        }

        let gap_before = previous_minute
            .map(|previous| minute.saturating_sub(previous) > 30)
            .unwrap_or(false);
        visible.push(TimelinePoint {
            label: trim_timestamp(&point.recorded_at),
            recorded_at: point.recorded_at.clone(),
            bpm: point.bpm,
            minute_of_day: minute,
            gap_before,
        });
        previous_minute = Some(minute);
    }

    visible
}

fn sync_state_for(sync_states: &[SyncStateRecord], family: DataFamily) -> Option<&SyncStateRecord> {
    sync_states
        .iter()
        .find(|state| state.sync_key == family.sync_key())
}

fn family_has_data(snapshot: &LiveSnapshot, family: DataFamily) -> bool {
    match family {
        DataFamily::Personal => snapshot.personal_info.is_some(),
        DataFamily::Daily => !snapshot.daily_history.is_empty(),
        DataFamily::Heartrate => snapshot
            .heartrate_days
            .iter()
            .any(|day| !day.points.is_empty()),
    }
}

fn latest_day_is_before_today(snapshot: &LiveSnapshot) -> bool {
    snapshot
        .daily_history
        .last()
        .is_some_and(|row| row.day < OffsetDateTime::now_utc().date().to_string())
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

fn score_card(
    label: &'static str,
    value: Option<u8>,
    badge: String,
    subtitle: String,
) -> ScoreCard {
    ScoreCard {
        label,
        value: value.map_or_else(|| "--".to_owned(), |score| score.to_string()),
        badge,
        subtitle,
    }
}

fn timeline_point(label: &str, recorded_at: &str, bpm: u16, gap_before: bool) -> TimelinePoint {
    TimelinePoint {
        label: label.to_owned(),
        recorded_at: recorded_at.to_owned(),
        bpm,
        minute_of_day: minutes_from_timestamp(recorded_at),
        gap_before,
    }
}

fn ops_item(label: &'static str, value: String) -> OpsItem {
    OpsItem { label, value }
}

fn auth_state_label(auth_status: &AuthStatus) -> String {
    if auth_status.access_token_stored || auth_status.refresh_token_stored {
        "authenticated".to_owned()
    } else if auth_status.configured {
        "configured_without_session".to_owned()
    } else {
        "unconfigured".to_owned()
    }
}

fn confidence_label(confidence: InsightConfidence) -> String {
    match confidence {
        InsightConfidence::Thin => "confidence: thin".to_owned(),
        InsightConfidence::Medium => "confidence: medium".to_owned(),
        InsightConfidence::Strong => "confidence: strong".to_owned(),
    }
}

fn window_sparkline(history: &[MetricPoint], days: usize) -> Vec<u64> {
    history
        .iter()
        .rev()
        .take(days)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|point| point.value.round().max(0.0) as u64)
        .collect()
}

fn freshness_badge(state: &FreshnessState) -> String {
    state.summary.clone()
}

fn format_day_selector(day_labels: &[String], selected_index: usize) -> String {
    if day_labels.is_empty() {
        return "no heartrate days cached".to_owned();
    }

    day_labels
        .iter()
        .enumerate()
        .map(|(index, day)| {
            if index == selected_index {
                format!("[{day}]")
            } else {
                day.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn format_float(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn trim_timestamp(value: &str) -> String {
    if value.len() >= 16 {
        value.chars().skip(11).take(5).collect()
    } else {
        value.to_owned()
    }
}

fn minutes_from_timestamp(value: &str) -> u16 {
    let hour = value
        .get(11..13)
        .and_then(|segment| segment.parse::<u16>().ok())
        .unwrap_or(0);
    let minute = value
        .get(14..16)
        .and_then(|segment| segment.parse::<u16>().ok())
        .unwrap_or(0);
    hour.saturating_mul(60).saturating_add(minute)
}

fn parse_timestamp(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339).ok()
}

fn is_auth_problem(problem: &crate::error::OuraProblem) -> bool {
    problem.oauth_error.is_some()
        || problem
            .status
            .is_some_and(|status| matches!(status, 401 | 403))
        || {
            let title = problem.title.to_ascii_lowercase();
            title.contains("auth") || title.contains("token")
        }
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| OffsetDateTime::now_utc().date().to_string())
}

impl DataFamily {
    fn label(self) -> &'static str {
        match self {
            Self::Personal => "Personal",
            Self::Daily => "Daily",
            Self::Heartrate => "Heartrate",
        }
    }

    fn sync_key(self) -> &'static str {
        match self {
            Self::Personal => SyncFamily::Personal.sync_key(),
            Self::Daily => SyncFamily::Daily.sync_key(),
            Self::Heartrate => SyncFamily::Heartrate.sync_key(),
        }
    }

    fn capability_kind(self) -> CapabilityKind {
        match self {
            Self::Personal => CapabilityKind::Personal,
            Self::Daily => CapabilityKind::Daily,
            Self::Heartrate => CapabilityKind::Heartrate,
        }
    }
}

impl RefreshPolicySnapshot {
    fn from_config(config: &Config) -> Self {
        Self {
            personal_interval_secs: config.refresh.personal_interval_secs,
            daily_interval_secs: config.refresh.daily_interval_secs,
            heartrate_interval_secs: config.refresh.heartrate_interval_secs,
            personal_stale_after_secs: config.refresh.personal_stale_after_secs,
            daily_stale_after_secs: config.refresh.daily_stale_after_secs,
            heartrate_stale_after_secs: config.refresh.heartrate_stale_after_secs,
        }
    }

    fn stale_after_seconds(&self, family: DataFamily) -> u64 {
        match family {
            DataFamily::Personal => self.personal_stale_after_secs,
            DataFamily::Daily => self.daily_stale_after_secs,
            DataFamily::Heartrate => self.heartrate_stale_after_secs,
        }
    }

    fn summary(&self) -> String {
        format!(
            "personal={}s daily={}s heartrate={}s",
            self.personal_interval_secs, self.daily_interval_secs, self.heartrate_interval_secs
        )
    }
}
