use std::collections::BTreeSet;

use crate::action::Action;
use crate::config::Config;
use crate::insights::{InsightConfidence, MetricInsight, MetricPoint, build_metric_insight};
use crate::oura::models::{AuthStatus, CapabilityKind, CapabilityReport};
use crate::refresh::SyncFamily;
use crate::review::{
    InvestigationReport, ReviewCard, ReviewDeck, ReviewFocus, ReviewInputs, ReviewMode,
    build_investigation_report, build_review_deck, ranked_cards,
};
use crate::store::Store;
use crate::store::queries::{
    ContextEventFamily, ContextEventRecord, DailyOverviewRow, EffectDirection, HeartRatePoint,
    PatternMetric, PatternRelationWindow, PatternSummaryRecord, PersonalInfoRecord, RecordCounts,
    RestModePeriodRecord, ReviewSignalDayRecord, SleepTimeRecord, SyncRunStatus, SyncStateRecord,
    TimeSemantics,
};
use crate::store::webhook_store::{
    AcceptedWebhookDeliveryRecord, DesiredWebhookSubscriptionRecord, InvalidationRecord,
    ProcessingAttemptRecord, RejectedWebhookDeliveryRecord, RemoteWebhookSubscriptionRecord,
    RuntimeHeartbeatRecord,
};
use time::{Date, OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataFamily {
    Personal,
    Daily,
    Heartrate,
    Workout,
    EnhancedTag,
    Session,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreshnessKind {
    FreshWebhook,
    FreshPeriodic,
    StaleNoRecentDelivery,
    StaleSyncFailed,
    StaleUnsupportedWebhook,
    StaleReceiverDown,
    StaleSubscriptionMissing,
    StaleCapabilityMissing,
    StaleUpstreamPending,
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
    pub webhook: WebhookOpsSnapshot,
    pub personal_info: Option<PersonalInfoRecord>,
    pub daily_history: Vec<DailyOverviewRow>,
    pub heartrate_days: Vec<HeartRateDay>,
    pub heartrate_daily_averages: Vec<MetricPoint>,
    pub context_events: Vec<ContextEventRecord>,
    pub pattern_summaries: Vec<PatternSummaryRecord>,
    pub review_signal_days: Vec<ReviewSignalDayRecord>,
    pub sleep_time: Vec<SleepTimeRecord>,
    pub rest_mode_periods: Vec<RestModePeriodRecord>,
    pub sync_states: Vec<SyncStateRecord>,
    pub record_counts: RecordCounts,
    pub schema_version: u32,
    pub database_path: String,
    pub config_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WebhookOpsSnapshot {
    pub bind_address: String,
    pub path: String,
    pub callback_url: Option<String>,
    pub verification_token_configured: bool,
    pub signature_tolerance_secs: u64,
    pub heartbeat_secs: u64,
    pub renewal_lead_secs: u64,
    pub desired_subscriptions: Vec<DesiredWebhookSubscriptionRecord>,
    pub remote_subscriptions: Vec<RemoteWebhookSubscriptionRecord>,
    pub recent_deliveries: Vec<AcceptedWebhookDeliveryRecord>,
    pub latest_rejected_delivery: Option<RejectedWebhookDeliveryRecord>,
    pub pending_invalidations: Vec<InvalidationRecord>,
    pub recent_processing_attempts: Vec<ProcessingAttemptRecord>,
    pub runtime_heartbeats: Vec<RuntimeHeartbeatRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshPolicySnapshot {
    pub personal_interval_secs: u64,
    pub daily_interval_secs: u64,
    pub heartrate_interval_secs: u64,
    pub workout_interval_secs: u64,
    pub enhanced_tag_interval_secs: u64,
    pub session_interval_secs: u64,
    pub personal_stale_after_secs: u64,
    pub daily_stale_after_secs: u64,
    pub heartrate_stale_after_secs: u64,
    pub workout_stale_after_secs: u64,
    pub enhanced_tag_stale_after_secs: u64,
    pub session_stale_after_secs: u64,
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
    Explain,
    Patterns,
    Ops,
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendWindowKind {
    Days7,
    Days30,
    Days90,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternMetricFilter {
    All,
    Activity,
    Readiness,
    Sleep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewScreenMode {
    Today,
    Week,
    Investigate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayFilterState {
    pub workouts: bool,
    pub tags: bool,
    pub sessions: bool,
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
    selected_day_index: usize,
    selected_timeline_point: usize,
    timeline_window_hours: u16,
    trends_window: TrendWindowKind,
    selected_event_id: Option<String>,
    selected_review_card_index: usize,
    overlay_filters: OverlayFilterState,
    pattern_metric_filter: PatternMetricFilter,
    review_mode: ReviewScreenMode,
    review_focus: ReviewFocus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppModel {
    pub title: String,
    pub dashboard: DashboardModel,
    pub timeline: TimelineModel,
    pub trends: TrendsModel,
    pub explain: ExplainModel,
    pub patterns: PatternsModel,
    pub ops: OpsModel,
    pub review: ReviewModel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardModel {
    pub selected_day_label: String,
    pub scores: Vec<ScoreCard>,
    pub freshness: String,
    pub capabilities: Vec<CapabilityView>,
    pub change_summary: String,
    pub highlights: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineModel {
    pub summary: String,
    pub day_selector: String,
    pub selected_day_label: String,
    pub selected_day_index: usize,
    pub heart_rate: Vec<TimelinePoint>,
    pub selected_point_index: Option<usize>,
    pub window_hours: u16,
    pub window_start_minute: u16,
    pub window_end_minute: u16,
    pub overlay_toggles: Vec<OverlayToggleView>,
    pub overlay_groups: Vec<OverlayFamilyGroup>,
    pub events: Vec<EventListItem>,
    pub selected_event_index: Option<usize>,
    pub selected_detail: String,
    pub event_detail_lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrendsModel {
    pub windows: Vec<TrendWindow>,
    pub selected_window_index: usize,
    pub metrics: Vec<TrendMetricView>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplainModel {
    pub selected_day_label: String,
    pub headline: String,
    pub summary_lines: Vec<String>,
    pub measurement_lines: Vec<String>,
    pub evidence_lines: Vec<String>,
    pub caveat_lines: Vec<String>,
    pub context_lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternsModel {
    pub header: String,
    pub filter_summary: String,
    pub rows: Vec<PatternRowView>,
    pub notes: Vec<String>,
    pub empty_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsModel {
    pub mode_label: String,
    pub summary_lines: Vec<String>,
    pub family_statuses: Vec<FamilyStatusView>,
    pub items: Vec<OpsItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewModel {
    pub selected_day_label: String,
    pub mode_tabs: Vec<ReviewTab>,
    pub selected_mode_index: usize,
    pub focus_tabs: Vec<ReviewTab>,
    pub selected_focus_index: usize,
    pub cards: Vec<ReviewCardView>,
    pub selected_card_index: Option<usize>,
    pub detail_lines: Vec<String>,
    pub warning_lines: Vec<String>,
    pub empty_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewTab {
    pub label: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewCardView {
    pub headline: String,
    pub confidence_label: String,
    pub section_label: String,
    pub selected: bool,
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
pub struct OverlayToggleView {
    pub label: &'static str,
    pub key_hint: &'static str,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayFamilyGroup {
    pub family_label: &'static str,
    pub glyph: char,
    pub item_count: usize,
    pub blocks: Vec<OverlayBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayBlock {
    pub id: String,
    pub start_minute: u16,
    pub end_minute: u16,
    pub title: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventListItem {
    pub id: String,
    pub family_label: &'static str,
    pub glyph: char,
    pub headline: String,
    pub detail: String,
    pub selected: bool,
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
pub struct PatternRowView {
    pub headline: String,
    pub detail: String,
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
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VisibleTimeline {
    points: Vec<TimelinePoint>,
    window_start_minute: u16,
    window_end_minute: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveModelOptions {
    selected_day_index: usize,
    selected_point_index: usize,
    selected_event_id: Option<String>,
    overlay_filters: OverlayFilterState,
    window_hours: u16,
    trends_window: TrendWindowKind,
    pattern_metric_filter: PatternMetricFilter,
    refresh_in_flight: bool,
    review_mode: ReviewScreenMode,
    review_focus: ReviewFocus,
    selected_review_card_index: usize,
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
                        "Demo mode is deterministic; refresh keeps the current snapshot.".to_owned()
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
            Action::PreviousDay => {
                if self.selected_day_index > 0 {
                    self.selected_day_index -= 1;
                    self.selected_timeline_point = 0;
                    self.selected_review_card_index = 0;
                    self.select_default_event_for_selected_day();
                    self.align_point_to_selected_event();
                    self.status_line = format!(
                        "Showing {}.",
                        self.selected_day_label()
                            .unwrap_or_else(|| "an earlier day".to_owned())
                    );
                    self.rebuild_live_model();
                }
            }
            Action::NextDay => {
                if self.selected_day_index + 1 < self.available_day_count() {
                    self.selected_day_index += 1;
                    self.selected_timeline_point = 0;
                    self.selected_review_card_index = 0;
                    self.select_default_event_for_selected_day();
                    self.align_point_to_selected_event();
                    self.status_line = format!(
                        "Showing {}.",
                        self.selected_day_label()
                            .unwrap_or_else(|| "a later day".to_owned())
                    );
                    self.rebuild_live_model();
                }
            }
            Action::PreviousTimelinePoint => {
                if self.selected_timeline_point > 0 {
                    self.selected_timeline_point -= 1;
                    self.select_nearest_event_for_current_point();
                    "Moved to an earlier heartrate point.".clone_into(&mut self.status_line);
                    self.rebuild_live_model();
                }
            }
            Action::NextTimelinePoint => {
                let max_index = self.visible_timeline_point_count().saturating_sub(1);
                if self.selected_timeline_point < max_index {
                    self.selected_timeline_point += 1;
                    self.select_nearest_event_for_current_point();
                    "Moved to a later heartrate point.".clone_into(&mut self.status_line);
                    self.rebuild_live_model();
                }
            }
            Action::PreviousEvent => {
                if self.select_relative_event(-1) {
                    self.align_point_to_selected_event();
                    "Moved to an earlier context event.".clone_into(&mut self.status_line);
                    self.rebuild_live_model();
                }
            }
            Action::NextEvent => {
                if self.select_relative_event(1) {
                    self.align_point_to_selected_event();
                    "Moved to a later context event.".clone_into(&mut self.status_line);
                    self.rebuild_live_model();
                }
            }
            Action::TimelineZoomIn => {
                self.timeline_window_hours = match self.timeline_window_hours {
                    24 => 12,
                    12 => 6,
                    _ => 6,
                };
                self.align_point_to_selected_event();
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
                self.align_point_to_selected_event();
                self.status_line =
                    format!("Timeline window set to {}h.", self.timeline_window_hours);
                self.rebuild_live_model();
            }
            Action::ToggleWorkoutFilter => {
                self.overlay_filters.workouts = !self.overlay_filters.workouts;
                self.normalize_event_selection();
                self.rebuild_live_model();
                self.status_line = format!(
                    "Workout overlays {}.",
                    if self.overlay_filters.workouts {
                        "enabled"
                    } else {
                        "hidden"
                    }
                );
            }
            Action::ToggleTagFilter => {
                self.overlay_filters.tags = !self.overlay_filters.tags;
                self.normalize_event_selection();
                self.rebuild_live_model();
                self.status_line = format!(
                    "Tag overlays {}.",
                    if self.overlay_filters.tags {
                        "enabled"
                    } else {
                        "hidden"
                    }
                );
            }
            Action::ToggleSessionFilter => {
                self.overlay_filters.sessions = !self.overlay_filters.sessions;
                self.normalize_event_selection();
                self.rebuild_live_model();
                self.status_line = format!(
                    "Session overlays {}.",
                    if self.overlay_filters.sessions {
                        "enabled"
                    } else {
                        "hidden"
                    }
                );
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
            Action::CyclePatternMetric => {
                self.pattern_metric_filter = self.pattern_metric_filter.next();
                self.status_line = format!(
                    "Pattern metric filter: {}.",
                    self.pattern_metric_filter.label()
                );
                self.rebuild_live_model();
            }
            Action::CycleReviewMode => {
                self.review_mode = self.review_mode.next();
                self.selected_review_card_index = 0;
                self.status_line = format!("Review mode changed to {}.", self.review_mode.label());
                self.rebuild_live_model();
            }
            Action::CycleReviewFocus => {
                self.review_focus = self.review_focus.next();
                self.selected_review_card_index = 0;
                self.status_line = format!(
                    "Investigation focus changed to {}.",
                    self.review_focus.label()
                );
                self.rebuild_live_model();
            }
            Action::PreviousReviewCard => {
                if self.selected_review_card_index > 0 {
                    self.selected_review_card_index -= 1;
                    "Moved to an earlier review card.".clone_into(&mut self.status_line);
                    self.rebuild_live_model();
                }
            }
            Action::NextReviewCard => {
                let max_index = self.current_review_card_count().saturating_sub(1);
                if self.selected_review_card_index < max_index {
                    self.selected_review_card_index += 1;
                    "Moved to a later review card.".clone_into(&mut self.status_line);
                    self.rebuild_live_model();
                }
            }
        }
    }

    pub fn footer(&self) -> String {
        let spinner = ["·", "o", "O", "o"][(self.tick_count % 4) as usize];
        let screen_hint = match self.active_screen {
            Screen::Dashboard => "[ ] day | 1-7 jump",
            Screen::Timeline => "[ ] day | , . hr | j k event | -/= zoom | w/t/s filters",
            Screen::Trends => "[ ] window",
            Screen::Explain => "[ ] day | j k event | w/t/s filters",
            Screen::Patterns => "w/t/s family | m metric",
            Screen::Ops => "1-7 jump",
            Screen::Review => "[ ] day | v mode | f focus | j k cards",
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
        let previous_day = self.selected_day_label();
        let previous_event = self.selected_event_id.clone();
        self.live_snapshot = Some(snapshot);

        if let Some(snapshot) = &self.live_snapshot {
            let day_labels = available_days(snapshot);
            self.selected_day_index = previous_day
                .as_deref()
                .and_then(|selected_day| day_labels.iter().position(|day| day == selected_day))
                .unwrap_or_else(|| newest_day_index(snapshot));
            self.selected_event_id = previous_event.filter(|event_id| {
                snapshot
                    .context_events
                    .iter()
                    .any(|event| &event.context_event_id == event_id)
            });
            self.selected_timeline_point = 0;
            self.normalize_event_selection();
            self.align_point_to_selected_event();
        }

        self.rebuild_live_model();
    }

    fn rebuild_live_model(&mut self) {
        if let Some(snapshot) = &self.live_snapshot {
            self.model = build_live_model(
                snapshot,
                &LiveModelOptions {
                    selected_day_index: self.selected_day_index,
                    selected_point_index: self.selected_timeline_point,
                    selected_event_id: self.selected_event_id.clone(),
                    overlay_filters: self.overlay_filters.clone(),
                    window_hours: self.timeline_window_hours,
                    trends_window: self.trends_window,
                    pattern_metric_filter: self.pattern_metric_filter,
                    refresh_in_flight: self.refresh_in_flight,
                    review_mode: self.review_mode,
                    review_focus: self.review_focus,
                    selected_review_card_index: self.selected_review_card_index,
                },
            );
        }
    }

    fn available_day_count(&self) -> usize {
        self.live_snapshot
            .as_ref()
            .map_or(0, |snapshot| available_days(snapshot).len())
    }

    fn selected_day_label(&self) -> Option<String> {
        self.live_snapshot.as_ref().and_then(|snapshot| {
            available_days(snapshot)
                .get(self.selected_day_index)
                .cloned()
        })
    }

    fn visible_timeline_point_count(&self) -> usize {
        self.live_snapshot
            .as_ref()
            .and_then(|snapshot| {
                self.selected_day_label().and_then(|day| {
                    selected_heartrate_day(snapshot, &day).map(|heartrate_day| {
                        visible_timeline(heartrate_day, self.timeline_window_hours)
                            .points
                            .len()
                    })
                })
            })
            .unwrap_or(0)
    }

    fn current_review_card_count(&self) -> usize {
        self.model.review.cards.len()
    }

    fn normalize_event_selection(&mut self) {
        let Some(snapshot) = &self.live_snapshot else {
            self.selected_event_id = None;
            return;
        };
        let Some(day) = self.selected_day_label() else {
            self.selected_event_id = None;
            return;
        };
        let events = filtered_events_for_day(snapshot, &day, &self.overlay_filters);
        if events.is_empty() {
            self.selected_event_id = None;
            return;
        }

        if self.selected_event_id.as_ref().is_some_and(|event_id| {
            events
                .iter()
                .any(|event| event.context_event_id == *event_id)
        }) {
            return;
        }

        if let Some(nearest_event) = nearest_event_for_point(
            snapshot,
            &day,
            self.timeline_window_hours,
            self.selected_timeline_point,
            &events,
        ) {
            self.selected_event_id = Some(nearest_event.context_event_id.clone());
            return;
        }

        self.selected_event_id = events.first().map(|event| event.context_event_id.clone());
    }

    fn select_default_event_for_selected_day(&mut self) {
        self.selected_event_id = None;
        self.normalize_event_selection();
    }

    fn select_nearest_event_for_current_point(&mut self) {
        let Some(snapshot) = &self.live_snapshot else {
            self.selected_event_id = None;
            return;
        };
        let Some(day) = self.selected_day_label() else {
            self.selected_event_id = None;
            return;
        };
        let events = filtered_events_for_day(snapshot, &day, &self.overlay_filters);
        self.selected_event_id = nearest_event_for_point(
            snapshot,
            &day,
            self.timeline_window_hours,
            self.selected_timeline_point,
            &events,
        )
        .map(|event| event.context_event_id.clone());
    }

    fn align_point_to_selected_event(&mut self) {
        let Some(snapshot) = &self.live_snapshot else {
            return;
        };
        let Some(day) = self.selected_day_label() else {
            return;
        };
        let Some(event_id) = self.selected_event_id.as_deref() else {
            return;
        };
        let Some(event) = filtered_events_for_day(snapshot, &day, &self.overlay_filters)
            .into_iter()
            .find(|event| event.context_event_id == event_id)
        else {
            return;
        };
        let Some(heartrate_day) = selected_heartrate_day(snapshot, &day) else {
            return;
        };
        let visible = visible_timeline(heartrate_day, self.timeline_window_hours);
        if visible.points.is_empty() {
            self.selected_timeline_point = 0;
            return;
        }
        self.selected_timeline_point =
            nearest_point_index_to_event(&visible.points, event).unwrap_or_default();
    }

    fn select_relative_event(&mut self, delta: isize) -> bool {
        let Some(snapshot) = &self.live_snapshot else {
            return false;
        };
        let Some(day) = self.selected_day_label() else {
            return false;
        };
        let events = filtered_events_for_day(snapshot, &day, &self.overlay_filters);
        if events.is_empty() {
            self.selected_event_id = None;
            return false;
        }

        let current_index = self
            .selected_event_id
            .as_ref()
            .and_then(|event_id| {
                events
                    .iter()
                    .position(|event| event.context_event_id == *event_id)
            })
            .unwrap_or_default();

        let new_index = if delta.is_negative() {
            current_index.saturating_sub(delta.unsigned_abs())
        } else {
            usize::min(
                current_index.saturating_add(delta as usize),
                events.len().saturating_sub(1),
            )
        };

        self.selected_event_id = Some(events[new_index].context_event_id.clone());
        true
    }
}

impl Screen {
    pub const ALL: [Self; 7] = [
        Self::Dashboard,
        Self::Timeline,
        Self::Trends,
        Self::Explain,
        Self::Patterns,
        Self::Ops,
        Self::Review,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Timeline => "Timeline",
            Self::Trends => "Trends",
            Self::Explain => "Explain",
            Self::Patterns => "Patterns",
            Self::Ops => "Ops",
            Self::Review => "Review",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Dashboard => 0,
            Self::Timeline => 1,
            Self::Trends => 2,
            Self::Explain => 3,
            Self::Patterns => 4,
            Self::Ops => 5,
            Self::Review => 6,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Dashboard => Self::Timeline,
            Self::Timeline => Self::Trends,
            Self::Trends => Self::Explain,
            Self::Explain => Self::Patterns,
            Self::Patterns => Self::Ops,
            Self::Ops => Self::Review,
            Self::Review => Self::Dashboard,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Dashboard => Self::Review,
            Self::Timeline => Self::Dashboard,
            Self::Trends => Self::Timeline,
            Self::Explain => Self::Trends,
            Self::Patterns => Self::Explain,
            Self::Ops => Self::Patterns,
            Self::Review => Self::Ops,
        }
    }
}

impl ReviewScreenMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Week => "Week",
            Self::Investigate => "Investigate",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Today => Self::Week,
            Self::Week => Self::Investigate,
            Self::Investigate => Self::Today,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Today => 0,
            Self::Week => 1,
            Self::Investigate => 2,
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

impl PatternMetricFilter {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "all metrics",
            Self::Activity => "activity",
            Self::Readiness => "next-day readiness",
            Self::Sleep => "same-night sleep",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::All => Self::Activity,
            Self::Activity => Self::Readiness,
            Self::Readiness => Self::Sleep,
            Self::Sleep => Self::All,
        }
    }

    fn metric(self) -> Option<PatternMetric> {
        match self {
            Self::All => None,
            Self::Activity => Some(PatternMetric::ActivityScore),
            Self::Readiness => Some(PatternMetric::ReadinessScore),
            Self::Sleep => Some(PatternMetric::SleepScore),
        }
    }
}

impl OverlayFilterState {
    pub const fn all() -> Self {
        Self {
            workouts: true,
            tags: true,
            sessions: true,
        }
    }

    fn summary(&self) -> String {
        format!(
            "W:{} T:{} S:{}",
            toggle_state(self.workouts),
            toggle_state(self.tags),
            toggle_state(self.sessions)
        )
    }
}

pub fn build_live_state(
    config: &Config,
    store: &Store,
    auth_status: &AuthStatus,
) -> crate::error::Result<AppState> {
    let snapshot = load_live_snapshot(config, store, auth_status)?;
    let selected_day_index = newest_day_index(&snapshot);
    let mut app = AppState {
        mode: RunMode::Live,
        active_screen: Screen::Dashboard,
        model: AppModel::empty(),
        status_line: "Live mode is reading from the local store.".to_owned(),
        tick_count: 0,
        should_quit: false,
        refresh_in_flight: false,
        live_snapshot: Some(snapshot),
        selected_day_index,
        selected_timeline_point: 0,
        timeline_window_hours: 24,
        trends_window: TrendWindowKind::Days7,
        selected_event_id: None,
        selected_review_card_index: 0,
        overlay_filters: OverlayFilterState::all(),
        pattern_metric_filter: PatternMetricFilter::All,
        review_mode: ReviewScreenMode::Today,
        review_focus: ReviewFocus::Readiness,
    };
    app.select_default_event_for_selected_day();
    app.align_point_to_selected_event();
    app.rebuild_live_model();
    Ok(app)
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
    let context_events = store
        .views()
        .context_events_between_days("0000-01-01", "9999-12-31")?;
    let pattern_summaries = store.views().pattern_summaries(None, None)?;

    Ok(LiveSnapshot {
        captured_at: now_rfc3339(),
        refresh_policy: RefreshPolicySnapshot::from_config(config),
        auth_status: auth_status.clone(),
        webhook: WebhookOpsSnapshot {
            bind_address: config.webhook.bind.to_string(),
            path: config.webhook.path.clone(),
            callback_url: config.webhook.callback_url(),
            verification_token_configured: config.webhook.verification_token.is_some(),
            signature_tolerance_secs: config.webhook.signature_tolerance_secs,
            heartbeat_secs: config.webhook.heartbeat_secs,
            renewal_lead_secs: config.webhook.renewal_lead_secs,
            desired_subscriptions: store.webhook().list_desired_subscriptions()?,
            remote_subscriptions: store.webhook().list_remote_subscriptions()?,
            recent_deliveries: store.webhook().list_recent_deliveries(32)?,
            latest_rejected_delivery: store.webhook().latest_rejected_delivery()?,
            pending_invalidations: store.webhook().list_pending_invalidations()?,
            recent_processing_attempts: store.webhook().list_recent_processing_attempts(32)?,
            runtime_heartbeats: store.webhook().list_runtime_heartbeats()?,
        },
        personal_info: store.views().latest_personal_info()?,
        daily_history,
        heartrate_days,
        heartrate_daily_averages,
        context_events,
        pattern_summaries,
        review_signal_days: store
            .views()
            .review_signal_days_between_days("0000-01-01", "9999-12-31")?,
        sleep_time: store
            .views()
            .sleep_time_between_days("0000-01-01", "9999-12-31")?,
        rest_mode_periods: store
            .views()
            .rest_mode_periods_between_days("0000-01-01", "9999-12-31")?,
        sync_states: store.sync_state().list()?,
        record_counts: store.views().record_counts()?,
        schema_version: store.metadata().schema_version()?,
        database_path: store.plan().db_path.display().to_string(),
        config_path: config.paths.config_file.display().to_string(),
    })
}

pub fn build_demo_state(config: &Config) -> AppState {
    let snapshot = demo_snapshot(config);
    let selected_day_index = newest_day_index(&snapshot);
    let mut app = AppState {
        mode: RunMode::Demo,
        active_screen: Screen::Dashboard,
        model: AppModel::empty(),
        status_line: "Demo mode ready.".to_owned(),
        tick_count: 0,
        should_quit: false,
        refresh_in_flight: false,
        live_snapshot: Some(snapshot),
        selected_day_index,
        selected_timeline_point: 0,
        timeline_window_hours: 24,
        trends_window: TrendWindowKind::Days7,
        selected_event_id: None,
        selected_review_card_index: 0,
        overlay_filters: OverlayFilterState::all(),
        pattern_metric_filter: PatternMetricFilter::All,
        review_mode: ReviewScreenMode::Today,
        review_focus: ReviewFocus::Readiness,
    };
    app.select_default_event_for_selected_day();
    app.align_point_to_selected_event();
    app.rebuild_live_model();
    app
}

fn build_live_model(snapshot: &LiveSnapshot, options: &LiveModelOptions) -> AppModel {
    let selected_day = selected_day_label(snapshot, options.selected_day_index)
        .unwrap_or_else(|| latest_review_anchor_day(snapshot));
    let review_inputs = ReviewInputs {
        auth_status: &snapshot.auth_status,
        signal_days: &snapshot.review_signal_days,
        context_events: &snapshot.context_events,
        pattern_summaries: &snapshot.pattern_summaries,
        sleep_time: &snapshot.sleep_time,
        rest_mode_periods: &snapshot.rest_mode_periods,
    };
    let today_review = build_review_deck(ReviewMode::Today, &selected_day, &review_inputs)
        .unwrap_or_else(|error| empty_review_deck(ReviewMode::Today, &selected_day, error));
    let week_review = build_review_deck(ReviewMode::Week, &selected_day, &review_inputs)
        .unwrap_or_else(|error| empty_review_deck(ReviewMode::Week, &selected_day, error));
    let investigation =
        build_investigation_report(options.review_focus, &selected_day, &review_inputs)
            .unwrap_or_else(|error| {
                empty_investigation_report(options.review_focus, &selected_day, error)
            });

    AppModel {
        title: "ringmaster".to_owned(),
        dashboard: build_dashboard_model(
            snapshot,
            options.selected_day_index,
            options.refresh_in_flight,
            &today_review,
        ),
        timeline: build_timeline_model(
            snapshot,
            options.selected_day_index,
            options.selected_point_index,
            options.selected_event_id.as_deref(),
            &options.overlay_filters,
            options.window_hours,
        ),
        trends: build_trends_model(snapshot, options.trends_window, &week_review),
        explain: build_explain_model(
            snapshot,
            options.selected_day_index,
            options.selected_event_id.as_deref(),
            &options.overlay_filters,
            &today_review,
        ),
        patterns: build_patterns_model(
            snapshot,
            &options.overlay_filters,
            options.pattern_metric_filter,
        ),
        ops: build_ops_model(snapshot, options.refresh_in_flight),
        review: build_review_model(
            &selected_day,
            &today_review,
            &week_review,
            &investigation,
            options.review_mode,
            options.review_focus,
            options.selected_review_card_index,
        ),
    }
}

fn build_dashboard_model(
    snapshot: &LiveSnapshot,
    selected_day_index: usize,
    refresh_in_flight: bool,
    today_review: &ReviewDeck,
) -> DashboardModel {
    let selected_day = selected_day_label(snapshot, selected_day_index)
        .unwrap_or_else(|| "no selected day".to_owned());
    let selected_daily = selected_daily_row(snapshot, &selected_day);
    let sleep_insight = build_day_metric_insight(snapshot, &selected_day, "sleep", |row| {
        row.sleep_score.map(f64::from)
    });
    let readiness_insight = build_day_metric_insight(snapshot, &selected_day, "readiness", |row| {
        row.readiness_score.map(f64::from)
    });
    let activity_insight = build_day_metric_insight(snapshot, &selected_day, "activity", |row| {
        row.activity_score.map(f64::from)
    });
    let heartrate_insight = build_metric_insight("heartrate", &snapshot.heartrate_daily_averages);

    let freshness = [
        format!(
            "Daily {}",
            freshness_badge(&family_freshness(snapshot, DataFamily::Daily))
        ),
        format!(
            "Heartrate {}",
            freshness_badge(&family_freshness(snapshot, DataFamily::Heartrate))
        ),
        format!(
            "Workouts {}",
            freshness_badge(&family_freshness(snapshot, DataFamily::Workout))
        ),
        format!(
            "Tags {}",
            freshness_badge(&family_freshness(snapshot, DataFamily::EnhancedTag))
        ),
        format!(
            "Sessions {}",
            freshness_badge(&family_freshness(snapshot, DataFamily::Session))
        ),
    ]
    .join(" | ");

    let change_summary = today_review.observations.first().map_or_else(
        || {
            [
                short_baseline_phrase("sleep", &sleep_insight),
                short_baseline_phrase("readiness", &readiness_insight),
                short_baseline_phrase("activity", &activity_insight),
            ]
            .join(" ")
        },
        |card| format!("{} {}", card.headline, card.confidence_label),
    );

    let mut highlights = vec![
        selected_day_baseline_sentence("Sleep", &selected_day, &sleep_insight),
        selected_day_baseline_sentence("Readiness", &selected_day, &readiness_insight),
        selected_day_baseline_sentence("Activity", &selected_day, &activity_insight),
    ];
    highlights.extend(
        top_context_events_for_day(snapshot, &selected_day)
            .into_iter()
            .take(2)
            .map(|event| format!("{} {}.", event.family_label, event.headline)),
    );
    highlights.extend(
        today_review
            .observations
            .iter()
            .take(2)
            .map(|card| format!("Review: {}", card.headline)),
    );
    if snapshot.heartrate_daily_averages.is_empty() {
        highlights.push(family_freshness(snapshot, DataFamily::Heartrate).detail);
    } else {
        highlights.push(heartrate_insight.summary);
    }
    if refresh_in_flight {
        highlights.insert(
            0,
            "Background refresh is running; the dashboard stays on persisted data until the next snapshot lands."
                .to_owned(),
        );
    }

    DashboardModel {
        selected_day_label: selected_day,
        scores: vec![
            score_card(
                "Sleep",
                selected_daily.and_then(|row| row.sleep_score),
                freshness_badge(&family_freshness(snapshot, DataFamily::Daily)),
                metric_subtitle(&sleep_insight),
            ),
            score_card(
                "Readiness",
                selected_daily.and_then(|row| row.readiness_score),
                freshness_badge(&family_freshness(snapshot, DataFamily::Daily)),
                metric_subtitle(&readiness_insight),
            ),
            score_card(
                "Activity",
                selected_daily.and_then(|row| row.activity_score),
                freshness_badge(&family_freshness(snapshot, DataFamily::Daily)),
                metric_subtitle(&activity_insight),
            ),
        ],
        freshness,
        capabilities: capability_views(&snapshot.auth_status.capability_report),
        change_summary,
        highlights,
    }
}

fn build_timeline_model(
    snapshot: &LiveSnapshot,
    selected_day_index: usize,
    selected_point_index: usize,
    selected_event_id: Option<&str>,
    overlay_filters: &OverlayFilterState,
    window_hours: u16,
) -> TimelineModel {
    let freshness = family_freshness(snapshot, DataFamily::Heartrate);
    let day_labels = available_days(snapshot);
    let clamped_day_index = if day_labels.is_empty() {
        0
    } else {
        usize::min(selected_day_index, day_labels.len().saturating_sub(1))
    };
    let selected_day = day_labels
        .get(clamped_day_index)
        .cloned()
        .unwrap_or_else(|| "no day selected".to_owned());
    let visible = selected_heartrate_day(snapshot, &selected_day)
        .map(|day| visible_timeline(day, window_hours))
        .unwrap_or_else(|| VisibleTimeline {
            points: Vec::new(),
            window_start_minute: 0,
            window_end_minute: 24 * 60 - 1,
        });
    let selected_point_index = if visible.points.is_empty() {
        None
    } else {
        Some(usize::min(
            selected_point_index,
            visible.points.len().saturating_sub(1),
        ))
    };
    let events = filtered_events_for_day(snapshot, &selected_day, overlay_filters);
    let selected_event_index = selected_event_id.and_then(|event_id| {
        events
            .iter()
            .position(|event| event.context_event_id == event_id)
    });
    let selected_point_detail = selected_point_index
        .and_then(|index| visible.points.get(index))
        .map(|point| {
            format!(
                "Heartrate cursor: {} at {} bpm.",
                point.recorded_at, point.bpm
            )
        })
        .unwrap_or_else(|| freshness.detail.clone());

    let event_detail_lines = selected_event_index
        .and_then(|index| events.get(index))
        .map_or_else(
            || {
                vec![
                    "No context event is selected for this day.".to_owned(),
                    "Use j/k or move the heartrate cursor to inspect nearby events.".to_owned(),
                ]
            },
            |event| explain_event_detail_lines(&selected_day, event),
        );

    TimelineModel {
        summary: format!(
            "Timeline for {} | heartrate {}",
            selected_day,
            freshness_badge(&freshness)
        ),
        day_selector: format!(
            "{} | window={}h | filters {}",
            format_day_selector(&day_labels, clamped_day_index),
            window_hours,
            overlay_filters.summary()
        ),
        selected_day_label: selected_day.clone(),
        selected_day_index: clamped_day_index,
        heart_rate: visible.points,
        selected_point_index,
        window_hours,
        window_start_minute: visible.window_start_minute,
        window_end_minute: visible.window_end_minute,
        overlay_toggles: overlay_toggle_views(overlay_filters),
        overlay_groups: build_overlay_groups(
            &selected_day,
            events.as_slice(),
            selected_event_id,
            visible.window_start_minute,
            visible.window_end_minute,
        ),
        events: events
            .iter()
            .map(|event| event_list_item(&selected_day, event, selected_event_id))
            .collect(),
        selected_event_index,
        selected_detail: selected_point_detail,
        event_detail_lines,
    }
}

fn build_trends_model(
    snapshot: &LiveSnapshot,
    trends_window: TrendWindowKind,
    week_review: &ReviewDeck,
) -> TrendsModel {
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

    TrendsModel {
        windows: vec![
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
                summary:
                    "Long view showing history while still comparing against recent baselines."
                        .to_owned(),
            },
        ],
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
        notes: {
            let mut notes = trend_notes(
                trends_window,
                [
                    &sleep_insight,
                    &readiness_insight,
                    &activity_insight,
                    &heartrate_insight,
                ],
            );
            if let Some(card) = week_review.negative_drifts.first() {
                notes.insert(0, format!("Weekly review: {}", card.headline));
            }
            notes
        },
    }
}

fn build_explain_model(
    snapshot: &LiveSnapshot,
    selected_day_index: usize,
    selected_event_id: Option<&str>,
    overlay_filters: &OverlayFilterState,
    today_review: &ReviewDeck,
) -> ExplainModel {
    let selected_day = selected_day_label(snapshot, selected_day_index)
        .unwrap_or_else(|| "no selected day".to_owned());
    let sleep_insight = build_day_metric_insight(snapshot, &selected_day, "sleep", |row| {
        row.sleep_score.map(f64::from)
    });
    let readiness_insight = build_day_metric_insight(snapshot, &selected_day, "readiness", |row| {
        row.readiness_score.map(f64::from)
    });
    let activity_insight = build_day_metric_insight(snapshot, &selected_day, "activity", |row| {
        row.activity_score.map(f64::from)
    });
    let selected_daily = selected_daily_row(snapshot, &selected_day);
    let heartrate = selected_heartrate_day(snapshot, &selected_day);
    let supporting_events =
        supporting_events_for_explain(snapshot, &selected_day, overlay_filters, selected_event_id);

    let mut caveat_lines = missing_scope_messages(&snapshot.auth_status.capability_report);
    if insight_is_thin(&sleep_insight)
        || insight_is_thin(&readiness_insight)
        || insight_is_thin(&activity_insight)
    {
        caveat_lines.push("This day story is based on limited history.".to_owned());
    }
    if selected_daily.is_none() {
        caveat_lines.push(format!(
            "No daily closeout is available for {} yet.",
            selected_day
        ));
    }
    if heartrate.is_none() {
        caveat_lines.push(format!(
            "No heartrate samples were cached for {}.",
            selected_day
        ));
    }
    if supporting_events.is_empty() {
        caveat_lines.push(
            "No persisted workouts, enhanced tags, or sessions were recorded around this day."
                .to_owned(),
        );
    }
    if let Some(card) = today_review
        .observations
        .iter()
        .find(|card| card.anchor_day == selected_day)
    {
        caveat_lines.push(format!("Review hint: {}", card.headline));
    }

    ExplainModel {
        selected_day_label: selected_day.clone(),
        headline: format!("Day story for {}", selected_day),
        summary_lines: vec![
            selected_day_baseline_sentence("Sleep", &selected_day, &sleep_insight),
            selected_day_baseline_sentence("Readiness", &selected_day, &readiness_insight),
            selected_day_baseline_sentence("Activity", &selected_day, &activity_insight),
        ],
        measurement_lines: measurement_lines_for_day(selected_daily, heartrate),
        evidence_lines: if supporting_events.is_empty() {
            vec!["Evidence is limited for this day.".to_owned()]
        } else {
            supporting_events
                .iter()
                .map(|event| format!("{} {}.", event.family_label, event.headline))
                .collect()
        },
        caveat_lines,
        context_lines: if supporting_events.is_empty() {
            vec!["Open Timeline after a sync to inspect raw context entries.".to_owned()]
        } else {
            let mut lines = supporting_events
                .iter()
                .map(|event| {
                    if event.selected {
                        format!("> {} ({})", event.headline, event.detail)
                    } else {
                        format!("  {} ({})", event.headline, event.detail)
                    }
                })
                .collect::<Vec<_>>();
            lines.push("Press 2 to open Timeline with the same selected event.".to_owned());
            lines
        },
    }
}

fn build_patterns_model(
    snapshot: &LiveSnapshot,
    overlay_filters: &OverlayFilterState,
    metric_filter: PatternMetricFilter,
) -> PatternsModel {
    let rows = snapshot
        .pattern_summaries
        .iter()
        .filter(|summary| overlay_filter_matches(overlay_filters, summary.family))
        .filter(|summary| {
            metric_filter
                .metric()
                .is_none_or(|metric| summary.metric == metric)
        })
        .map(pattern_row_view)
        .collect::<Vec<_>>();

    PatternsModel {
        header: "Patterns".to_owned(),
        filter_summary: format!(
            "Families {} | metric {}",
            overlay_filters.summary(),
            metric_filter.label()
        ),
        rows,
        notes: vec![
            "Patterns are descriptive associations, not causal claims.".to_owned(),
            "Rows appear after at least 3 comparable days; same-night sleep refers to the following closeout day.".to_owned(),
        ],
        empty_message:
            "Not enough data yet. Patterns appear after at least 3 comparable days.".to_owned(),
    }
}

fn build_ops_model(snapshot: &LiveSnapshot, refresh_in_flight: bool) -> OpsModel {
    let family_statuses = [
        DataFamily::Personal,
        DataFamily::Daily,
        DataFamily::Heartrate,
        DataFamily::Workout,
        DataFamily::EnhancedTag,
        DataFamily::Session,
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

    let last_accepted_delivery = snapshot
        .webhook
        .recent_deliveries
        .first()
        .map_or_else(|| "none".to_owned(), format_delivery_record);
    let last_rejected_delivery = snapshot
        .webhook
        .latest_rejected_delivery
        .as_ref()
        .map_or_else(|| "none".to_owned(), format_rejection_record);
    let queue_depth = snapshot.webhook.pending_invalidations.len();
    let queue_oldest = snapshot
        .webhook
        .pending_invalidations
        .iter()
        .map(|record| record.first_queued_at.as_str())
        .min()
        .unwrap_or("n/a")
        .to_owned();
    let recent_failures = snapshot
        .webhook
        .recent_processing_attempts
        .iter()
        .filter(|attempt| attempt.outcome == "failed")
        .count();
    let summary_lines = vec![
        format!("Mode: {}", ops_runtime_mode(snapshot)),
        format!("Receiver: {}", receiver_status_line(snapshot)),
        format!(
            "Queue: pending={} oldest={} failed_attempts={}",
            queue_depth, queue_oldest, recent_failures
        ),
    ];

    let mut warnings = family_statuses
        .iter()
        .filter(|status| status.state_label.starts_with("stale"))
        .map(|status| format!("{}: {}", status.label, status.detail))
        .collect::<Vec<_>>();
    if refresh_in_flight {
        warnings.insert(
            0,
            "Background refresh is active; diagnostics update after the next persisted snapshot."
                .to_owned(),
        );
    }
    if snapshot.record_counts.derived_pattern_summaries == 0 {
        warnings.push(
            "Patterns are currently empty; run `cargo run -- derive rebuild` or sync more history."
                .to_owned(),
        );
    }
    warnings.extend(recent_health_incidents(snapshot));

    OpsModel {
        mode_label: ops_runtime_mode(snapshot),
        summary_lines,
        family_statuses,
        items: vec![
            ops_item("Config path", snapshot.config_path.clone()),
            ops_item("Database path", snapshot.database_path.clone()),
            ops_item("Schema version", snapshot.schema_version.to_string()),
            ops_item(
                "Webhook bind",
                format!("{}{}", snapshot.webhook.bind_address, snapshot.webhook.path),
            ),
            ops_item(
                "Webhook callback",
                snapshot
                    .webhook
                    .callback_url
                    .clone()
                    .unwrap_or_else(|| "unconfigured".to_owned()),
            ),
            ops_item(
                "Webhook config",
                if receiver_config_complete(snapshot) {
                    format!(
                        "complete | tolerance={}s | renewal_lead={}s",
                        snapshot.webhook.signature_tolerance_secs,
                        snapshot.webhook.renewal_lead_secs
                    )
                } else {
                    "incomplete".to_owned()
                },
            ),
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
            ops_item(
                "Receiver heartbeat",
                heartbeat_for(snapshot, "webhook.receiver").map_or_else(
                    || "missing".to_owned(),
                    |record| format_heartbeat_status(snapshot, record),
                ),
            ),
            ops_item(
                "Watch heartbeat",
                heartbeat_for(snapshot, "sync.watch").map_or_else(
                    || "missing".to_owned(),
                    |record| format_heartbeat_status(snapshot, record),
                ),
            ),
            ops_item("Subscriptions", overall_subscription_summary(snapshot)),
            ops_item(
                "Subscription horizons",
                subscription_horizon_summary(snapshot),
            ),
            ops_item("Last delivery by family", family_delivery_summary(snapshot)),
            ops_item("Last accepted delivery", last_accepted_delivery),
            ops_item("Last rejected delivery", last_rejected_delivery),
            ops_item(
                "Invalidation queue",
                format!(
                    "pending={} oldest={} recent_attempts={} failures={}",
                    queue_depth,
                    queue_oldest,
                    snapshot.webhook.recent_processing_attempts.len(),
                    recent_failures
                ),
            ),
            ops_item("Freshness sources", freshness_source_summary(snapshot)),
            ops_item(
                "Last webhook sync",
                last_trigger_summary(snapshot, "webhook"),
            ),
            ops_item(
                "Last periodic sync",
                last_trigger_summary(snapshot, "periodic_reconcile"),
            ),
            ops_item("Refresh policy", snapshot.refresh_policy.summary()),
            ops_item(
                "Record counts",
                format!(
                    "profile={} daily={} heartrate={} workouts={} tags={} sessions={} derived={} patterns={} raw={}",
                    snapshot.record_counts.personal_info,
                    snapshot.record_counts.daily_sleep
                        + snapshot.record_counts.daily_readiness
                        + snapshot.record_counts.daily_activity,
                    snapshot.record_counts.heartrate_samples,
                    snapshot.record_counts.workouts,
                    snapshot.record_counts.tags + snapshot.record_counts.enhanced_tags,
                    snapshot.record_counts.sessions,
                    snapshot.record_counts.derived_context_events,
                    snapshot.record_counts.derived_pattern_summaries,
                    snapshot.record_counts.raw_payloads,
                ),
            ),
        ],
        warnings,
    }
}

fn build_review_model(
    selected_day: &str,
    today_review: &ReviewDeck,
    week_review: &ReviewDeck,
    investigation: &InvestigationReport,
    review_mode: ReviewScreenMode,
    review_focus: ReviewFocus,
    selected_review_card_index: usize,
) -> ReviewModel {
    let cards = review_cards_for_mode(review_mode, today_review, week_review, investigation);
    let selected_card_index = if cards.is_empty() {
        None
    } else {
        Some(usize::min(
            selected_review_card_index,
            cards.len().saturating_sub(1),
        ))
    };

    let card_views = cards
        .iter()
        .enumerate()
        .map(|(index, card)| ReviewCardView {
            headline: card.headline.clone(),
            confidence_label: card.confidence_label.clone(),
            section_label: review_section_label(card),
            selected: selected_card_index == Some(index),
        })
        .collect::<Vec<_>>();

    ReviewModel {
        selected_day_label: selected_day.to_owned(),
        mode_tabs: [
            ReviewScreenMode::Today,
            ReviewScreenMode::Week,
            ReviewScreenMode::Investigate,
        ]
        .into_iter()
        .map(|mode| ReviewTab {
            label: mode.label().to_owned(),
            selected: mode == review_mode,
        })
        .collect(),
        selected_mode_index: review_mode.index(),
        focus_tabs: ReviewFocus::ALL
            .into_iter()
            .map(|focus| ReviewTab {
                label: focus.label().to_owned(),
                selected: focus == review_focus,
            })
            .collect(),
        selected_focus_index: ReviewFocus::ALL
            .iter()
            .position(|focus| *focus == review_focus)
            .unwrap_or_default(),
        cards: card_views,
        selected_card_index,
        detail_lines: review_detail_lines(
            review_mode,
            selected_card_index.and_then(|index| cards.get(index).copied()),
            investigation,
        ),
        warning_lines: review_warning_lines(review_mode, today_review, week_review, investigation),
        empty_message: review_empty_message(review_mode, review_focus),
    }
}

fn review_cards_for_mode<'a>(
    review_mode: ReviewScreenMode,
    today_review: &'a ReviewDeck,
    week_review: &'a ReviewDeck,
    investigation: &'a InvestigationReport,
) -> Vec<&'a ReviewCard> {
    match review_mode {
        ReviewScreenMode::Today => ranked_cards(today_review),
        ReviewScreenMode::Week => ranked_cards(week_review),
        ReviewScreenMode::Investigate => {
            let focus_keys = investigation.focus.primary_signal_keys();
            let mut cards = ranked_cards(today_review)
                .into_iter()
                .filter(|card| focus_keys.contains(&card.signal_key.as_str()))
                .collect::<Vec<_>>();
            cards.extend(
                ranked_cards(week_review)
                    .into_iter()
                    .filter(|card| focus_keys.contains(&card.signal_key.as_str())),
            );
            cards.sort_by(|left, right| right.score.cmp(&left.score));
            cards
        }
    }
}

fn review_detail_lines(
    review_mode: ReviewScreenMode,
    selected_card: Option<&ReviewCard>,
    investigation: &InvestigationReport,
) -> Vec<String> {
    let mut lines = Vec::new();

    if matches!(review_mode, ReviewScreenMode::Investigate) {
        lines.push(investigation.headline.clone());
        lines.push(format!(
            "{} confidence / {} data",
            investigation.confidence.label(),
            investigation.sufficiency.label()
        ));
        lines.push(investigation.summary.clone());
        if !investigation.evidence.is_empty() {
            lines.push("Evidence:".to_owned());
            lines.extend(
                investigation
                    .evidence
                    .iter()
                    .map(|line| format!("  - {line}")),
            );
        }
        if !investigation.counterevidence.is_empty() {
            lines.push("Counterevidence:".to_owned());
            lines.extend(
                investigation
                    .counterevidence
                    .iter()
                    .map(|line| format!("  - {line}")),
            );
        }
    }

    if let Some(card) = selected_card {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(card.headline.clone());
        lines.push(card.confidence_label.clone());
        lines.push(card.summary.clone());
        lines.push(card.why_this_is_shown.clone());
        if !card.evidence.is_empty() {
            lines.push("Card evidence:".to_owned());
            lines.extend(card.evidence.iter().map(|line| format!("  - {line}")));
        }
        if !card.counterevidence.is_empty() {
            lines.push("Card uncertainty:".to_owned());
            lines.extend(
                card.counterevidence
                    .iter()
                    .map(|line| format!("  - {line}")),
            );
        }
        if !card.warnings.is_empty() {
            lines.push("Card warnings:".to_owned());
            lines.extend(card.warnings.iter().map(|line| format!("  - {line}")));
        }
    }

    if matches!(review_mode, ReviewScreenMode::Investigate) && !investigation.look_at.is_empty() {
        lines.push(String::new());
        lines.push("Look next:".to_owned());
        lines.extend(
            investigation
                .look_at
                .iter()
                .map(|line| format!("  - {line}")),
        );
    }

    lines
}

fn review_warning_lines(
    review_mode: ReviewScreenMode,
    today_review: &ReviewDeck,
    week_review: &ReviewDeck,
    investigation: &InvestigationReport,
) -> Vec<String> {
    let mut warnings = Vec::new();
    match review_mode {
        ReviewScreenMode::Today => warnings.extend(today_review.warnings.iter().cloned()),
        ReviewScreenMode::Week => warnings.extend(week_review.warnings.iter().cloned()),
        ReviewScreenMode::Investigate => warnings.extend(investigation.warnings.iter().cloned()),
    }
    if warnings.is_empty() {
        warnings.push("Review outputs are built from local persisted data only.".to_owned());
    }
    warnings
}

fn review_empty_message(review_mode: ReviewScreenMode, review_focus: ReviewFocus) -> String {
    match review_mode {
        ReviewScreenMode::Today => {
            "No ranked review cards are available for this day yet.".to_owned()
        }
        ReviewScreenMode::Week => {
            "No weekly review cards are available for this anchor day yet.".to_owned()
        }
        ReviewScreenMode::Investigate => format!(
            "No {} investigation cards are available for this anchor day yet.",
            review_focus.as_str()
        ),
    }
}

fn review_section_label(card: &ReviewCard) -> String {
    match card.section {
        crate::review::engine::ReviewSection::Observation => "Observation".to_owned(),
        crate::review::engine::ReviewSection::PositiveChange => "Positive".to_owned(),
        crate::review::engine::ReviewSection::NegativeDrift => "Drift".to_owned(),
        crate::review::engine::ReviewSection::UnresolvedAnomaly => "Anomaly".to_owned(),
    }
}

fn latest_review_anchor_day(snapshot: &LiveSnapshot) -> String {
    let mut days = available_days(snapshot);
    days.extend(
        snapshot
            .review_signal_days
            .iter()
            .map(|row| row.day.clone()),
    );
    days.extend(snapshot.sleep_time.iter().map(|row| row.day.clone()));
    for period in &snapshot.rest_mode_periods {
        days.push(period.start_day.clone());
        if let Some(end_day) = &period.end_day {
            days.push(end_day.clone());
        }
    }
    days.into_iter()
        .max()
        .unwrap_or_else(current_local_day_string)
}

fn empty_review_deck(mode: ReviewMode, anchor_day: &str, error: impl ToString) -> ReviewDeck {
    ReviewDeck {
        mode,
        anchor_day: anchor_day.to_owned(),
        observations: Vec::new(),
        positive_changes: Vec::new(),
        negative_drifts: Vec::new(),
        unresolved_anomalies: Vec::new(),
        warnings: vec![format!(
            "Review generation fell back to an empty deck: {}",
            error.to_string()
        )],
    }
}

fn empty_investigation_report(
    focus: ReviewFocus,
    anchor_day: &str,
    error: impl ToString,
) -> InvestigationReport {
    InvestigationReport {
        focus,
        anchor_day: anchor_day.to_owned(),
        headline: format!("{} investigation is unavailable right now.", focus.label()),
        summary: "Evidence is limited because the investigation engine could not build a complete report from the current local snapshot."
            .to_owned(),
        confidence: crate::review::engine::ReviewConfidence::Low,
        sufficiency: crate::review::features::ReviewSufficiency::Missing,
        evidence: Vec::new(),
        counterevidence: Vec::new(),
        warnings: vec![format!(
            "Investigation generation fell back to an empty report: {}",
            error.to_string()
        )],
        look_at: vec![
            "Open Ops to confirm sync freshness and granted capabilities.".to_owned(),
            "Run derive rebuild after syncing more history.".to_owned(),
        ],
    }
}

fn receiver_config_complete(snapshot: &LiveSnapshot) -> bool {
    snapshot.webhook.callback_url.is_some()
        && snapshot.webhook.verification_token_configured
        && !snapshot
            .auth_status
            .missing_fields
            .contains(&"client_secret")
}

fn heartbeat_for<'a>(
    snapshot: &'a LiveSnapshot,
    component: &str,
) -> Option<&'a RuntimeHeartbeatRecord> {
    snapshot
        .webhook
        .runtime_heartbeats
        .iter()
        .find(|record| record.component == component)
}

fn heartbeat_is_healthy(snapshot: &LiveSnapshot, record: &RuntimeHeartbeatRecord) -> bool {
    let Some(last_seen_at) = parse_timestamp(&record.last_seen_at) else {
        return false;
    };
    let now = parse_timestamp(&snapshot.captured_at).unwrap_or_else(OffsetDateTime::now_utc);
    let age = (now - last_seen_at).whole_seconds().max(0);
    let max_age = i64::try_from(snapshot.webhook.heartbeat_secs)
        .unwrap_or_default()
        .saturating_mul(3);
    age <= max_age
}

fn heartbeat_is_active(snapshot: &LiveSnapshot, record: &RuntimeHeartbeatRecord) -> bool {
    record.mode != "stopped" && heartbeat_is_healthy(snapshot, record)
}

fn receiver_healthy(snapshot: &LiveSnapshot) -> bool {
    heartbeat_for(snapshot, "webhook.receiver")
        .is_some_and(|record| heartbeat_is_active(snapshot, record))
}

fn format_heartbeat_status(snapshot: &LiveSnapshot, record: &RuntimeHeartbeatRecord) -> String {
    let health = if heartbeat_is_active(snapshot, record) {
        "healthy"
    } else if record.mode == "stopped" {
        "stopped"
    } else {
        "stale"
    };
    let detail = record
        .detail
        .clone()
        .unwrap_or_else(|| "no detail".to_owned());
    format!(
        "{} | mode={} | last_seen={} | {}",
        health, record.mode, record.last_seen_at, detail
    )
}

fn ops_runtime_mode(snapshot: &LiveSnapshot) -> String {
    let receiver = heartbeat_for(snapshot, "webhook.receiver")
        .is_some_and(|record| heartbeat_is_active(snapshot, record));
    let watcher = heartbeat_for(snapshot, "sync.watch")
        .is_some_and(|record| heartbeat_is_active(snapshot, record));

    match (receiver, watcher) {
        (true, true) => "full hybrid".to_owned(),
        (true, false) => "receiver only".to_owned(),
        _ => "scheduler only".to_owned(),
    }
}

fn receiver_status_line(snapshot: &LiveSnapshot) -> String {
    if !receiver_config_complete(snapshot) {
        return "config incomplete".to_owned();
    }

    heartbeat_for(snapshot, "webhook.receiver").map_or_else(
        || "missing heartbeat".to_owned(),
        |record| {
            if heartbeat_is_active(snapshot, record) {
                "healthy".to_owned()
            } else if record.mode == "stopped" {
                format!("stopped ({})", record.last_seen_at)
            } else {
                format!("stale heartbeat ({})", record.last_seen_at)
            }
        },
    )
}

fn recent_health_incidents(snapshot: &LiveSnapshot) -> Vec<String> {
    let mut incidents = Vec::new();

    if let Some(rejection) = &snapshot.webhook.latest_rejected_delivery {
        incidents.push(format!(
            "Latest rejected delivery: {} at {} ({})",
            rejection.reason_code, rejection.received_at, rejection.detail
        ));
    }

    incidents.extend(
        snapshot
            .webhook
            .recent_processing_attempts
            .iter()
            .filter(|attempt| attempt.outcome == "failed")
            .take(3)
            .map(|attempt| {
                format!(
                    "Invalidation {} failed at {}{}",
                    attempt.invalidation_id,
                    attempt
                        .finished_at
                        .clone()
                        .unwrap_or_else(|| attempt.started_at.clone()),
                    attempt
                        .detail
                        .as_ref()
                        .map_or_else(String::new, |detail| format!(" ({detail})"))
                )
            }),
    );

    for component in ["webhook.receiver", "sync.watch"] {
        if let Some(record) = heartbeat_for(snapshot, component)
            && !heartbeat_is_active(snapshot, record)
            && record.mode != "stopped"
        {
            incidents.push(format!(
                "{} heartbeat is stale (last seen {})",
                component, record.last_seen_at
            ));
        }
    }

    incidents.extend(
        snapshot
            .webhook
            .remote_subscriptions
            .iter()
            .filter(|record| !remote_subscription_is_healthy(snapshot, record))
            .take(4)
            .map(|record| {
                format!(
                    "Subscription {} {} {} is {} (expires {})",
                    record.data_type,
                    record.event_type.as_str(),
                    record.subscription_id,
                    record.drift_status,
                    record.expiration_time
                )
            }),
    );

    incidents
}

fn overall_subscription_summary(snapshot: &LiveSnapshot) -> String {
    let desired = snapshot
        .webhook
        .desired_subscriptions
        .iter()
        .filter(|record| record.enabled)
        .count();
    let remote = snapshot.webhook.remote_subscriptions.len();
    let healthy = snapshot
        .webhook
        .remote_subscriptions
        .iter()
        .filter(|record| remote_subscription_is_healthy(snapshot, record))
        .count();
    let drifted = snapshot
        .webhook
        .remote_subscriptions
        .iter()
        .filter(|record| record.drift_status != "matched")
        .count();
    format!(
        "desired_enabled={} remote={} healthy={} drifted={}",
        desired, remote, healthy, drifted
    )
}

fn subscription_horizon_summary(snapshot: &LiveSnapshot) -> String {
    let renewals_due = snapshot
        .webhook
        .remote_subscriptions
        .iter()
        .filter(|record| remote_subscription_needs_renewal(snapshot, record))
        .count();
    let next_expiration = snapshot
        .webhook
        .remote_subscriptions
        .iter()
        .filter_map(|record| {
            parse_timestamp(&record.expiration_time).map(|timestamp| (timestamp, record))
        })
        .min_by_key(|(timestamp, _)| *timestamp)
        .map(|(_, record)| format!("{} {}", record.data_type, record.expiration_time))
        .unwrap_or_else(|| "none".to_owned());

    format!(
        "renewals_due={} next_expiration={}",
        renewals_due, next_expiration
    )
}

fn family_delivery_summary(snapshot: &LiveSnapshot) -> String {
    [
        DataFamily::Daily,
        DataFamily::Workout,
        DataFamily::EnhancedTag,
        DataFamily::Session,
        DataFamily::Heartrate,
    ]
    .into_iter()
    .map(|family| {
        let detail = if family_supports_webhooks(family) {
            family_last_delivery(snapshot, family).map_or_else(
                || "none".to_owned(),
                |record| trim_timestamp(&record.received_at),
            )
        } else {
            "webhook unsupported".to_owned()
        };
        format!("{}={detail}", family.label())
    })
    .collect::<Vec<_>>()
    .join(" | ")
}

fn freshness_source_summary(snapshot: &LiveSnapshot) -> String {
    let mut webhook_count = 0;
    let mut periodic_count = 0;
    let mut other_count = 0;

    for state in &snapshot.sync_states {
        match state.last_trigger_source.as_deref() {
            Some("webhook") => webhook_count += 1,
            Some("periodic_reconcile") => periodic_count += 1,
            Some(_) => other_count += 1,
            None => {}
        }
    }

    format!(
        "webhook={} periodic={} other={}",
        webhook_count, periodic_count, other_count
    )
}

fn last_trigger_summary(snapshot: &LiveSnapshot, trigger_source: &str) -> String {
    snapshot
        .sync_states
        .iter()
        .filter(|state| state.last_trigger_source.as_deref() == Some(trigger_source))
        .max_by_key(|state| sync_state_effective_timestamp(state))
        .map_or_else(
            || "none".to_owned(),
            |state| {
                let timestamp = state
                    .last_completed_at
                    .clone()
                    .unwrap_or_else(|| state.last_attempted_at.clone());
                let detail = state
                    .last_trigger_detail
                    .as_ref()
                    .map_or_else(String::new, |value| format!(" ({value})"));
                format!("{} at {}{}", state.sync_key, timestamp, detail)
            },
        )
}

fn sync_state_effective_timestamp(state: &SyncStateRecord) -> Option<OffsetDateTime> {
    state
        .last_completed_at
        .as_deref()
        .or(Some(state.last_attempted_at.as_str()))
        .and_then(parse_timestamp)
}

fn format_delivery_record(record: &AcceptedWebhookDeliveryRecord) -> String {
    let data_type = record
        .data_type
        .clone()
        .unwrap_or_else(|| "unknown".to_owned());
    let event_type = record
        .event_type
        .map_or_else(|| "unknown".to_owned(), |value| value.as_str().to_owned());
    let object_id = record.object_id.clone().unwrap_or_else(|| "n/a".to_owned());
    format!(
        "{} {} object={} at {}",
        data_type, event_type, object_id, record.received_at
    )
}

fn format_rejection_record(record: &RejectedWebhookDeliveryRecord) -> String {
    format!(
        "{} at {} ({})",
        record.reason_code, record.received_at, record.detail
    )
}

fn family_supports_webhooks(family: DataFamily) -> bool {
    matches!(
        family,
        DataFamily::Daily | DataFamily::Workout | DataFamily::EnhancedTag | DataFamily::Session
    )
}

fn family_matches_data_type(family: DataFamily, data_type: &str) -> bool {
    match family {
        DataFamily::Daily => matches!(
            data_type,
            "daily_sleep" | "daily_readiness" | "daily_activity"
        ),
        DataFamily::Workout => data_type == "workout",
        DataFamily::EnhancedTag => data_type == "enhanced_tag",
        DataFamily::Session => data_type == "session",
        DataFamily::Personal | DataFamily::Heartrate => false,
    }
}

fn family_last_delivery(
    snapshot: &LiveSnapshot,
    family: DataFamily,
) -> Option<&AcceptedWebhookDeliveryRecord> {
    snapshot.webhook.recent_deliveries.iter().find(|record| {
        record
            .data_type
            .as_deref()
            .is_some_and(|data_type| family_matches_data_type(family, data_type))
    })
}

fn family_subscription_ready(snapshot: &LiveSnapshot, family: DataFamily) -> bool {
    let desired = snapshot
        .webhook
        .desired_subscriptions
        .iter()
        .filter(|record| record.enabled && family_matches_data_type(family, &record.data_type))
        .collect::<Vec<_>>();
    if desired.is_empty() {
        return false;
    }

    desired.into_iter().all(|desired_record| {
        snapshot
            .webhook
            .remote_subscriptions
            .iter()
            .any(|remote_record| {
                remote_record.data_type == desired_record.data_type
                    && remote_record.event_type == desired_record.event_type
                    && remote_subscription_is_healthy(snapshot, remote_record)
            })
    })
}

fn remote_subscription_is_healthy(
    snapshot: &LiveSnapshot,
    record: &RemoteWebhookSubscriptionRecord,
) -> bool {
    if record.drift_status != "matched" {
        return false;
    }

    let Some(expiration_time) = parse_timestamp(&record.expiration_time) else {
        return false;
    };
    let now = parse_timestamp(&snapshot.captured_at).unwrap_or_else(OffsetDateTime::now_utc);
    expiration_time > now
}

fn remote_subscription_needs_renewal(
    snapshot: &LiveSnapshot,
    record: &RemoteWebhookSubscriptionRecord,
) -> bool {
    let Some(expiration_time) = parse_timestamp(&record.expiration_time) else {
        return true;
    };
    let now = parse_timestamp(&snapshot.captured_at).unwrap_or_else(OffsetDateTime::now_utc);
    let renewal_lead = time::Duration::seconds(
        i64::try_from(snapshot.webhook.renewal_lead_secs).unwrap_or_default(),
    );
    expiration_time <= now + renewal_lead
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

fn newest_day_index(snapshot: &LiveSnapshot) -> usize {
    available_days(snapshot).len().saturating_sub(1)
}

fn selected_day_label(snapshot: &LiveSnapshot, selected_day_index: usize) -> Option<String> {
    available_days(snapshot).get(selected_day_index).cloned()
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
            kind: FreshnessKind::StaleCapabilityMissing,
            summary: freshness_label(FreshnessKind::StaleCapabilityMissing),
            detail: format!(
                "{} scope was not granted, so {} stay unavailable.",
                family.capability_kind().scope_name(),
                family.label().to_lowercase()
            ),
        };
    }

    let has_data = family_has_data(snapshot, family);
    let sync_state = sync_state_for(&snapshot.sync_states, family);
    let now = parse_timestamp(&snapshot.captured_at).unwrap_or_else(OffsetDateTime::now_utc);
    let supports_webhooks = family_supports_webhooks(family);
    let subscription_ready = !supports_webhooks || family_subscription_ready(snapshot, family);
    let receiver_ready = !supports_webhooks || receiver_config_complete(snapshot);
    let receiver_runtime_healthy = !supports_webhooks || receiver_healthy(snapshot);
    let last_delivery = family_last_delivery(snapshot, family);

    if let Some(sync_state) = sync_state {
        if sync_state.last_error.as_ref().is_some_and(is_auth_problem)
            || matches!(sync_state.status, SyncRunStatus::Failed)
        {
            return FreshnessState {
                family,
                kind: FreshnessKind::StaleSyncFailed,
                summary: freshness_label(FreshnessKind::StaleSyncFailed),
                detail: sync_state.message.clone().unwrap_or_else(|| {
                    sync_state.last_error.as_ref().map_or_else(
                        || format!("{} failed to sync.", family.label()),
                        ToString::to_string,
                    )
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

        if is_fresh
            && matches!(
                sync_state.status,
                SyncRunStatus::Success | SyncRunStatus::Partial
            )
        {
            let kind = if sync_state.last_trigger_source.as_deref() == Some("webhook") {
                FreshnessKind::FreshWebhook
            } else {
                FreshnessKind::FreshPeriodic
            };
            let detail = match (family, latest_day_is_before_today(snapshot)) {
                (DataFamily::Daily, true) => {
                    "Daily closeout is current through the latest fully available upstream day."
                        .to_owned()
                }
                _ => format!(
                    "{} updated at {}.",
                    family.label(),
                    sync_state
                        .last_completed_at
                        .clone()
                        .unwrap_or_else(|| sync_state.last_attempted_at.clone())
                ),
            };
            return FreshnessState {
                family,
                kind,
                summary: freshness_label(kind),
                detail,
            };
        }
    }

    if supports_webhooks && !receiver_ready {
        return FreshnessState {
            family,
            kind: FreshnessKind::StaleReceiverDown,
            summary: freshness_label(FreshnessKind::StaleReceiverDown),
            detail: "Webhook receiver configuration is incomplete for this family.".to_owned(),
        };
    }

    if supports_webhooks && !receiver_runtime_healthy {
        return FreshnessState {
            family,
            kind: FreshnessKind::StaleReceiverDown,
            summary: freshness_label(FreshnessKind::StaleReceiverDown),
            detail: "Webhook receiver heartbeat is stale or missing.".to_owned(),
        };
    }

    if supports_webhooks && !subscription_ready {
        return FreshnessState {
            family,
            kind: FreshnessKind::StaleSubscriptionMissing,
            summary: freshness_label(FreshnessKind::StaleSubscriptionMissing),
            detail: format!(
                "{} subscriptions are missing, drifted, or expired.",
                family.label()
            ),
        };
    }

    if !has_data {
        return FreshnessState {
            family,
            kind: FreshnessKind::StaleUpstreamPending,
            summary: freshness_label(FreshnessKind::StaleUpstreamPending),
            detail: sync_state
                .and_then(|state| state.message.clone())
                .unwrap_or_else(|| {
                    format!(
                        "{} has not produced any persisted records yet.",
                        family.label()
                    )
                }),
        };
    }

    if supports_webhooks {
        return FreshnessState {
            family,
            kind: FreshnessKind::StaleNoRecentDelivery,
            summary: freshness_label(FreshnessKind::StaleNoRecentDelivery),
            detail: last_delivery.map_or_else(
                || {
                    format!(
                        "No recent webhook delivery was recorded for {}.",
                        family.label()
                    )
                },
                |record| {
                    format!(
                        "Last webhook delivery for {} arrived at {}.",
                        family.label(),
                        record.received_at
                    )
                },
            ),
        };
    }

    FreshnessState {
        family,
        kind: FreshnessKind::StaleUnsupportedWebhook,
        summary: freshness_label(FreshnessKind::StaleUnsupportedWebhook),
        detail: format!(
            "{} still relies on scheduled reconcile windows because Oura does not expose webhook invalidations for it.",
            family.label()
        ),
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

fn build_day_metric_insight<F>(
    snapshot: &LiveSnapshot,
    selected_day: &str,
    label: &'static str,
    mut mapper: F,
) -> MetricInsight
where
    F: FnMut(&DailyOverviewRow) -> Option<f64>,
{
    let history = snapshot
        .daily_history
        .iter()
        .filter(|row| row.day.as_str() <= selected_day)
        .filter_map(|row| {
            mapper(row).map(|value| MetricPoint {
                day: row.day.clone(),
                value,
            })
        })
        .collect::<Vec<_>>();
    build_metric_insight(label, &history)
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

fn selected_day_baseline_sentence(
    label: &str,
    selected_day: &str,
    insight: &MetricInsight,
) -> String {
    let Some(today) = insight.today.as_ref() else {
        return format!("{label} has no daily closeout on {}.", selected_day);
    };
    if insight.baseline_30d.sample_count < 4 {
        return format!(
            "{} is {} on {}, but there is not enough history to compare it to your normal yet.",
            label,
            format_float(today.value),
            selected_day
        );
    }

    let baseline = insight.baseline_30d.mean.unwrap_or(today.value);
    let delta = today.value - baseline;
    let relation = if delta >= 1.0 {
        "above"
    } else if delta <= -1.0 {
        "below"
    } else {
        "close to"
    };

    format!(
        "{} is {} your 30-day baseline ({} vs {}).",
        label,
        relation,
        format_float(today.value),
        format_float(baseline)
    )
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

fn visible_timeline(day: &HeartRateDay, window_hours: u16) -> VisibleTimeline {
    let latest_minute = day
        .points
        .last()
        .map(|point| minutes_from_timestamp(&point.recorded_at))
        .unwrap_or(0);
    let window_end = latest_minute.max(window_hours.saturating_mul(60).saturating_sub(1));
    let window_start = window_end.saturating_sub(window_hours.saturating_mul(60).saturating_sub(1));
    let mut visible = Vec::new();
    let mut previous_minute = None;

    for point in &day.points {
        let minute = minutes_from_timestamp(&point.recorded_at);
        if minute < window_start || minute > window_end {
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

    VisibleTimeline {
        points: visible,
        window_start_minute: window_start,
        window_end_minute: window_end,
    }
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
        DataFamily::Workout => snapshot.record_counts.workouts > 0,
        DataFamily::EnhancedTag => {
            snapshot.record_counts.tags + snapshot.record_counts.enhanced_tags > 0
        }
        DataFamily::Session => snapshot.record_counts.sessions > 0,
    }
}

fn latest_day_is_before_today(snapshot: &LiveSnapshot) -> bool {
    latest_day_is_before_reference_day(snapshot, current_local_day_string())
}

fn latest_day_is_before_reference_day(snapshot: &LiveSnapshot, reference_day: String) -> bool {
    snapshot
        .daily_history
        .last()
        .is_some_and(|row| row.day < reference_day)
}

fn current_local_day_string() -> String {
    let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    OffsetDateTime::now_utc()
        .to_offset(local_offset)
        .date()
        .to_string()
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

fn selected_daily_row<'a>(snapshot: &'a LiveSnapshot, day: &str) -> Option<&'a DailyOverviewRow> {
    snapshot.daily_history.iter().find(|row| row.day == day)
}

fn selected_heartrate_day<'a>(snapshot: &'a LiveSnapshot, day: &str) -> Option<&'a HeartRateDay> {
    snapshot.heartrate_days.iter().find(|row| row.day == day)
}

fn filtered_events_for_day<'a>(
    snapshot: &'a LiveSnapshot,
    day: &str,
    filters: &OverlayFilterState,
) -> Vec<&'a ContextEventRecord> {
    snapshot
        .context_events
        .iter()
        .filter(|event| event_overlaps_day(event, day))
        .filter(|event| overlay_filter_matches(filters, event.family))
        .collect()
}

fn supporting_events_for_explain(
    snapshot: &LiveSnapshot,
    day: &str,
    filters: &OverlayFilterState,
    selected_event_id: Option<&str>,
) -> Vec<EventListItem> {
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();

    for event in filtered_events_for_day(snapshot, day, filters) {
        if seen.insert(event.context_event_id.clone()) {
            items.push(event_list_item(day, event, selected_event_id));
        }
    }

    if let Some(previous_day) = previous_daily_day(snapshot, day) {
        for event in filtered_events_for_day(snapshot, &previous_day, filters) {
            if (event.context_event_id == selected_event_id.unwrap_or_default()
                || event_starts_after_hour(event, 18))
                && seen.insert(event.context_event_id.clone())
            {
                items.push(event_list_item(&previous_day, event, selected_event_id));
            }
        }
    }

    items
}

fn previous_daily_day(snapshot: &LiveSnapshot, day: &str) -> Option<String> {
    snapshot
        .daily_history
        .iter()
        .filter(|row| row.day.as_str() < day)
        .map(|row| row.day.clone())
        .next_back()
}

fn top_context_events_for_day(snapshot: &LiveSnapshot, day: &str) -> Vec<EventListItem> {
    filtered_events_for_day(snapshot, day, &OverlayFilterState::all())
        .into_iter()
        .map(|event| event_list_item(day, event, None))
        .collect()
}

fn overlay_filter_matches(filters: &OverlayFilterState, family: ContextEventFamily) -> bool {
    match family {
        ContextEventFamily::Workout => filters.workouts,
        ContextEventFamily::Tag | ContextEventFamily::EnhancedTag => filters.tags,
        ContextEventFamily::Session => filters.sessions,
    }
}

fn nearest_event_for_point<'a>(
    snapshot: &'a LiveSnapshot,
    day: &str,
    window_hours: u16,
    point_index: usize,
    events: &[&'a ContextEventRecord],
) -> Option<&'a ContextEventRecord> {
    let visible_points = selected_heartrate_day(snapshot, day)
        .map(|heartrate_day| visible_timeline(heartrate_day, window_hours).points)
        .unwrap_or_default();
    let point = visible_points.get(point_index)?;
    events
        .iter()
        .min_by_key(|event| event_distance_seconds(event, &point.recorded_at))
        .copied()
}

fn nearest_point_index_to_event(
    points: &[TimelinePoint],
    event: &ContextEventRecord,
) -> Option<usize> {
    points
        .iter()
        .enumerate()
        .min_by_key(|(_, point)| event_distance_seconds(event, &point.recorded_at))
        .map(|(index, _)| index)
}

fn event_distance_seconds(event: &ContextEventRecord, timestamp: &str) -> i64 {
    let target = parse_timestamp(timestamp).unwrap_or_else(OffsetDateTime::now_utc);
    let start = parse_timestamp(&event.start_at).unwrap_or(target);
    let end = event
        .end_at
        .as_deref()
        .and_then(parse_timestamp)
        .unwrap_or(start);

    if target >= start && target <= end {
        0
    } else if target < start {
        (start - target).whole_seconds().abs()
    } else {
        (target - end).whole_seconds().abs()
    }
}

fn overlay_toggle_views(filters: &OverlayFilterState) -> Vec<OverlayToggleView> {
    vec![
        OverlayToggleView {
            label: "Workouts",
            key_hint: "w",
            enabled: filters.workouts,
        },
        OverlayToggleView {
            label: "Tags",
            key_hint: "t",
            enabled: filters.tags,
        },
        OverlayToggleView {
            label: "Sessions",
            key_hint: "s",
            enabled: filters.sessions,
        },
    ]
}

fn build_overlay_groups(
    day: &str,
    events: &[&ContextEventRecord],
    selected_event_id: Option<&str>,
    window_start_minute: u16,
    window_end_minute: u16,
) -> Vec<OverlayFamilyGroup> {
    let families = [
        ContextEventFamily::Workout,
        ContextEventFamily::EnhancedTag,
        ContextEventFamily::Session,
    ];

    families
        .into_iter()
        .filter_map(|family| {
            let blocks = events
                .iter()
                .filter(|event| {
                    event.family == family
                        || (family == ContextEventFamily::EnhancedTag
                            && event.family == ContextEventFamily::Tag)
                })
                .filter_map(|event| {
                    event_bounds_for_day(event, day).and_then(|(start_minute, end_minute)| {
                        if end_minute < window_start_minute || start_minute > window_end_minute {
                            return None;
                        }

                        Some(OverlayBlock {
                            id: event.context_event_id.clone(),
                            start_minute,
                            end_minute,
                            title: event.title.clone(),
                            selected: selected_event_id
                                .is_some_and(|event_id| event.context_event_id == event_id),
                        })
                    })
                })
                .collect::<Vec<_>>();

            if blocks.is_empty() {
                None
            } else {
                Some(OverlayFamilyGroup {
                    family_label: overlay_family_label(family),
                    glyph: overlay_family_glyph(family),
                    item_count: blocks.len(),
                    blocks,
                })
            }
        })
        .collect()
}

fn event_list_item(
    day: &str,
    event: &ContextEventRecord,
    selected_event_id: Option<&str>,
) -> EventListItem {
    EventListItem {
        id: event.context_event_id.clone(),
        family_label: overlay_family_label(event.family),
        glyph: overlay_family_glyph(event.family),
        headline: format!("{} {}", format_event_time(day, event), event.title),
        detail: [
            event.subtype.clone(),
            event.intensity.clone(),
            event.notes.clone(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" | "),
        selected: selected_event_id.is_some_and(|event_id| event.context_event_id == event_id),
    }
}

fn pattern_row_view(summary: &PatternSummaryRecord) -> PatternRowView {
    PatternRowView {
        headline: format!(
            "{}: {}",
            overlay_family_label(summary.family),
            prettify_key(&summary.normalized_key)
        ),
        detail: format!(
            "{} {} {} ({}, n={}, confidence={})",
            relation_phrase(summary.relation_window),
            effect_direction_phrase(summary.effect_direction),
            summary.metric.label().to_lowercase(),
            signed_delta(summary.median_delta),
            summary.sample_count,
            data_sufficiency_label(summary.confidence),
        ),
    }
}

fn measurement_lines_for_day(
    daily: Option<&DailyOverviewRow>,
    heartrate_day: Option<&HeartRateDay>,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(daily) = daily {
        lines.push(format!(
            "Sleep {} | Readiness {} | Activity {}",
            score_text(daily.sleep_score),
            score_text(daily.readiness_score),
            score_text(daily.activity_score),
        ));
    } else {
        lines.push("Daily closeout is not available for this day.".to_owned());
    }

    if let Some(heartrate_day) = heartrate_day {
        if heartrate_day.points.is_empty() {
            lines.push("Heartrate samples are missing for this day.".to_owned());
        } else {
            let mean = heartrate_day
                .points
                .iter()
                .map(|point| f64::from(point.bpm))
                .sum::<f64>()
                / heartrate_day.points.len() as f64;
            lines.push(format!(
                "Heartrate mean {} bpm across {} samples.",
                format_float(mean),
                heartrate_day.points.len()
            ));
        }
    } else {
        lines.push("Heartrate samples are missing for this day.".to_owned());
    }

    lines
}

fn explain_event_detail_lines(day: &str, event: &ContextEventRecord) -> Vec<String> {
    let mut lines = vec![
        format!("{} {}", overlay_family_label(event.family), event.title),
        format!("When: {}", format_event_time(day, event)),
    ];
    if let Some(subtype) = &event.subtype {
        lines.push(format!("Type: {}", subtype));
    }
    if let Some(intensity) = &event.intensity {
        lines.push(format!("Strength: {}", intensity));
    }
    if let Some(notes) = &event.notes {
        lines.push(format!("Notes: {}", notes));
    }
    lines.push(format!("Source id: {}", event.source_id));
    lines
}

fn missing_scope_messages(report: &CapabilityReport) -> Vec<String> {
    [
        CapabilityKind::Workout,
        CapabilityKind::EnhancedTag,
        CapabilityKind::Session,
    ]
    .into_iter()
    .filter(|kind| !report.is_granted(*kind))
    .map(|kind| {
        format!(
            "{} are unavailable because the `{}` scope was not granted.",
            kind.label(),
            kind.scope_name()
        )
    })
    .collect()
}

fn insight_is_thin(insight: &MetricInsight) -> bool {
    matches!(insight.confidence, InsightConfidence::Thin)
}

fn available_days(snapshot: &LiveSnapshot) -> Vec<String> {
    let mut days = BTreeSet::new();
    for row in &snapshot.daily_history {
        days.insert(row.day.clone());
    }
    for day in &snapshot.heartrate_days {
        days.insert(day.day.clone());
    }
    for event in &snapshot.context_events {
        days.insert(event.anchor_day.clone());
    }
    for row in &snapshot.review_signal_days {
        days.insert(row.day.clone());
    }
    for row in &snapshot.sleep_time {
        days.insert(row.day.clone());
    }
    for period in &snapshot.rest_mode_periods {
        days.insert(period.start_day.clone());
        if let Some(end_day) = &period.end_day {
            days.insert(end_day.clone());
        }
    }
    days.into_iter().collect()
}

fn event_overlaps_day(event: &ContextEventRecord, day: &str) -> bool {
    event.anchor_day == day || event_bounds_for_day(event, day).is_some()
}

fn event_bounds_for_day(event: &ContextEventRecord, day: &str) -> Option<(u16, u16)> {
    if matches!(event.time_semantics, TimeSemantics::AllDay) {
        return (event.anchor_day == day).then_some((0, 24 * 60 - 1));
    }

    let (day_start, day_end) = event_local_day_bounds(event, day)?;
    let start = parse_timestamp(&event.start_at).unwrap_or(day_start);
    let end = event
        .end_at
        .as_deref()
        .and_then(parse_timestamp)
        .unwrap_or(start);

    if end < day_start || start > day_end {
        return None;
    }

    let clipped_start = if start < day_start { day_start } else { start };
    let clipped_end = if end > day_end { day_end } else { end };
    Some((
        minute_of_day(clipped_start),
        minute_of_day(clipped_end).max(minute_of_day(clipped_start)),
    ))
}

fn event_local_day_bounds(
    event: &ContextEventRecord,
    day: &str,
) -> Option<(OffsetDateTime, OffsetDateTime)> {
    let reference = parse_timestamp(&event.start_at)
        .or_else(|| event.end_at.as_deref().and_then(parse_timestamp))?;
    let local_day = Date::parse(
        day,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .ok()?;
    let day_start = local_day
        .with_hms(0, 0, 0)
        .ok()?
        .assume_offset(reference.offset());
    let day_end = local_day
        .with_hms(23, 59, 59)
        .ok()?
        .assume_offset(reference.offset());
    Some((day_start, day_end))
}

fn event_starts_after_hour(event: &ContextEventRecord, hour: u8) -> bool {
    parse_timestamp(&event.start_at)
        .map(|timestamp| timestamp.hour() >= hour)
        .unwrap_or(false)
}

fn minute_of_day(value: OffsetDateTime) -> u16 {
    let hour = u16::from(value.hour());
    let minute = u16::from(value.minute());
    hour.saturating_mul(60).saturating_add(minute)
}

fn format_event_time(day: &str, event: &ContextEventRecord) -> String {
    match event.time_semantics {
        TimeSemantics::AllDay => "all day".to_owned(),
        TimeSemantics::Point => event_bounds_for_day(event, day).map_or_else(
            || trim_timestamp(&event.start_at),
            |(start, _)| format_minutes(start),
        ),
        TimeSemantics::Interval => event_bounds_for_day(event, day).map_or_else(
            || trim_timestamp(&event.start_at),
            |(start, end)| format!("{}-{}", format_minutes(start), format_minutes(end)),
        ),
    }
}

fn overlay_family_label(family: ContextEventFamily) -> &'static str {
    match family {
        ContextEventFamily::Workout => "Workout",
        ContextEventFamily::Tag | ContextEventFamily::EnhancedTag => "Tag",
        ContextEventFamily::Session => "Session",
    }
}

fn overlay_family_glyph(family: ContextEventFamily) -> char {
    match family {
        ContextEventFamily::Workout => 'W',
        ContextEventFamily::Tag | ContextEventFamily::EnhancedTag => 'T',
        ContextEventFamily::Session => 'S',
    }
}

fn relation_phrase(window: PatternRelationWindow) -> &'static str {
    match window {
        PatternRelationWindow::SameDayActivity => "after days with",
        PatternRelationWindow::NextDayReadiness => "after days with",
        PatternRelationWindow::SameNightSleep => "after days with",
    }
}

fn effect_direction_phrase(direction: EffectDirection) -> &'static str {
    match direction {
        EffectDirection::Higher => "associated with higher",
        EffectDirection::Lower => "associated with lower",
        EffectDirection::Flat => "co-occurred with flat",
    }
}

fn prettify_key(value: &str) -> String {
    value.replace("::", " / ").replace('_', " ")
}

fn data_sufficiency_label(value: crate::store::queries::DataSufficiency) -> &'static str {
    match value {
        crate::store::queries::DataSufficiency::Thin => "thin",
        crate::store::queries::DataSufficiency::Medium => "medium",
        crate::store::queries::DataSufficiency::Strong => "strong",
    }
}

fn signed_delta(value: f64) -> String {
    format!("{value:+.1}")
}

fn toggle_state(enabled: bool) -> &'static str {
    if enabled { "on" } else { "off" }
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

fn ops_item(label: impl Into<String>, value: String) -> OpsItem {
    OpsItem {
        label: label.into(),
        value,
    }
}

fn score_text(value: Option<u8>) -> String {
    value.map_or_else(|| "--".to_owned(), |score| score.to_string())
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

fn freshness_label(kind: FreshnessKind) -> String {
    match kind {
        FreshnessKind::FreshWebhook => "fresh via webhook".to_owned(),
        FreshnessKind::FreshPeriodic => "fresh via periodic".to_owned(),
        FreshnessKind::StaleNoRecentDelivery => "stale: no recent delivery".to_owned(),
        FreshnessKind::StaleSyncFailed => "stale: sync failed".to_owned(),
        FreshnessKind::StaleUnsupportedWebhook => "stale: webhook unsupported".to_owned(),
        FreshnessKind::StaleReceiverDown => "stale: receiver down".to_owned(),
        FreshnessKind::StaleSubscriptionMissing => "stale: subscription missing".to_owned(),
        FreshnessKind::StaleCapabilityMissing => "stale: capability missing".to_owned(),
        FreshnessKind::StaleUpstreamPending => "stale: upstream pending".to_owned(),
    }
}

fn format_day_selector(day_labels: &[String], selected_index: usize) -> String {
    if day_labels.is_empty() {
        return "no cached days".to_owned();
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

fn format_minutes(value: u16) -> String {
    let hours = value / 60;
    let minutes = value % 60;
    format!("{hours:02}:{minutes:02}")
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
            Self::Workout => "Workouts",
            Self::EnhancedTag => "Enhanced Tags",
            Self::Session => "Sessions",
        }
    }

    fn sync_key(self) -> &'static str {
        match self {
            Self::Personal => SyncFamily::Personal.sync_key(),
            Self::Daily => SyncFamily::Daily.sync_key(),
            Self::Heartrate => SyncFamily::Heartrate.sync_key(),
            Self::Workout => SyncFamily::Workout.sync_key(),
            Self::EnhancedTag => SyncFamily::EnhancedTag.sync_key(),
            Self::Session => SyncFamily::Session.sync_key(),
        }
    }

    fn capability_kind(self) -> CapabilityKind {
        match self {
            Self::Personal => CapabilityKind::Personal,
            Self::Daily => CapabilityKind::Daily,
            Self::Heartrate => CapabilityKind::Heartrate,
            Self::Workout => CapabilityKind::Workout,
            Self::EnhancedTag => CapabilityKind::EnhancedTag,
            Self::Session => CapabilityKind::Session,
        }
    }
}

impl RefreshPolicySnapshot {
    fn from_config(config: &Config) -> Self {
        Self {
            personal_interval_secs: config.refresh.personal_interval_secs,
            daily_interval_secs: config.refresh.daily_interval_secs,
            heartrate_interval_secs: config.refresh.heartrate_interval_secs,
            workout_interval_secs: config.refresh.workout_interval_secs,
            enhanced_tag_interval_secs: config.refresh.enhanced_tag_interval_secs,
            session_interval_secs: config.refresh.session_interval_secs,
            personal_stale_after_secs: config.refresh.personal_stale_after_secs,
            daily_stale_after_secs: config.refresh.daily_stale_after_secs,
            heartrate_stale_after_secs: config.refresh.heartrate_stale_after_secs,
            workout_stale_after_secs: config.refresh.workout_stale_after_secs,
            enhanced_tag_stale_after_secs: config.refresh.enhanced_tag_stale_after_secs,
            session_stale_after_secs: config.refresh.session_stale_after_secs,
        }
    }

    fn stale_after_seconds(&self, family: DataFamily) -> u64 {
        match family {
            DataFamily::Personal => self.personal_stale_after_secs,
            DataFamily::Daily => self.daily_stale_after_secs,
            DataFamily::Heartrate => self.heartrate_stale_after_secs,
            DataFamily::Workout => self.workout_stale_after_secs,
            DataFamily::EnhancedTag => self.enhanced_tag_stale_after_secs,
            DataFamily::Session => self.session_stale_after_secs,
        }
    }

    fn summary(&self) -> String {
        format!(
            "personal={}s daily={}s heartrate={}s workouts={}s tags={}s sessions={}s",
            self.personal_interval_secs,
            self.daily_interval_secs,
            self.heartrate_interval_secs,
            self.workout_interval_secs,
            self.enhanced_tag_interval_secs,
            self.session_interval_secs
        )
    }
}

impl AppModel {
    fn empty() -> Self {
        Self {
            title: "ringmaster".to_owned(),
            dashboard: DashboardModel {
                selected_day_label: String::new(),
                scores: Vec::new(),
                freshness: String::new(),
                capabilities: Vec::new(),
                change_summary: String::new(),
                highlights: Vec::new(),
            },
            timeline: TimelineModel {
                summary: String::new(),
                day_selector: String::new(),
                selected_day_label: String::new(),
                selected_day_index: 0,
                heart_rate: Vec::new(),
                selected_point_index: None,
                window_hours: 24,
                window_start_minute: 0,
                window_end_minute: 24 * 60 - 1,
                overlay_toggles: Vec::new(),
                overlay_groups: Vec::new(),
                events: Vec::new(),
                selected_event_index: None,
                selected_detail: String::new(),
                event_detail_lines: Vec::new(),
            },
            trends: TrendsModel {
                windows: Vec::new(),
                selected_window_index: 0,
                metrics: Vec::new(),
                notes: Vec::new(),
            },
            explain: ExplainModel {
                selected_day_label: String::new(),
                headline: String::new(),
                summary_lines: Vec::new(),
                measurement_lines: Vec::new(),
                evidence_lines: Vec::new(),
                caveat_lines: Vec::new(),
                context_lines: Vec::new(),
            },
            patterns: PatternsModel {
                header: String::new(),
                filter_summary: String::new(),
                rows: Vec::new(),
                notes: Vec::new(),
                empty_message: String::new(),
            },
            ops: OpsModel {
                mode_label: String::new(),
                summary_lines: Vec::new(),
                family_statuses: Vec::new(),
                items: Vec::new(),
                warnings: Vec::new(),
            },
            review: ReviewModel {
                selected_day_label: String::new(),
                mode_tabs: Vec::new(),
                selected_mode_index: 0,
                focus_tabs: Vec::new(),
                selected_focus_index: 0,
                cards: Vec::new(),
                selected_card_index: None,
                detail_lines: Vec::new(),
                warning_lines: Vec::new(),
                empty_message: String::new(),
            },
        }
    }
}

fn demo_snapshot(config: &Config) -> LiveSnapshot {
    let capability_report = CapabilityReport::demo();
    let auth_status = AuthStatus {
        configured: true,
        callback_url: config.oura.callback_url(),
        requested_scopes: config.oura.requested_scopes.clone(),
        granted_scopes: config.oura.requested_scopes.clone(),
        missing_fields: Vec::new(),
        capability_report,
        auth_timeout_secs: config.oura.auth_timeout_secs,
        secret_backend: "demo-memory".to_owned(),
        access_token_stored: true,
        refresh_token_stored: true,
        access_token_expires_at: Some("2026-04-08T08:00:00Z".to_owned()),
        last_authenticated_at: Some("2026-04-08T03:00:00Z".to_owned()),
        last_refresh_at: Some("2026-04-08T03:30:00Z".to_owned()),
        account_id: Some("demo-user".to_owned()),
        account_email: Some("demo@example.com".to_owned()),
        last_error: None,
    };

    let daily_history = vec![
        DailyOverviewRow {
            day: "2026-04-05".to_owned(),
            sleep_score: Some(82),
            readiness_score: Some(80),
            activity_score: Some(72),
            updated_at: "2026-04-05T10:00:00Z".to_owned(),
        },
        DailyOverviewRow {
            day: "2026-04-06".to_owned(),
            sleep_score: Some(84),
            readiness_score: Some(81),
            activity_score: Some(74),
            updated_at: "2026-04-06T10:00:00Z".to_owned(),
        },
        DailyOverviewRow {
            day: "2026-04-07".to_owned(),
            sleep_score: Some(80),
            readiness_score: Some(78),
            activity_score: Some(75),
            updated_at: "2026-04-07T10:00:00Z".to_owned(),
        },
        DailyOverviewRow {
            day: "2026-04-08".to_owned(),
            sleep_score: Some(76),
            readiness_score: Some(74),
            activity_score: Some(88),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
        },
    ];
    let heartrate_days = vec![
        HeartRateDay {
            day: "2026-04-07".to_owned(),
            points: vec![
                HeartRatePoint {
                    recorded_at: "2026-04-07T18:30:00Z".to_owned(),
                    bpm: 92,
                    source_day: Some("2026-04-07".to_owned()),
                },
                HeartRatePoint {
                    recorded_at: "2026-04-07T19:00:00Z".to_owned(),
                    bpm: 118,
                    source_day: Some("2026-04-07".to_owned()),
                },
                HeartRatePoint {
                    recorded_at: "2026-04-07T19:30:00Z".to_owned(),
                    bpm: 84,
                    source_day: Some("2026-04-07".to_owned()),
                },
            ],
        },
        HeartRateDay {
            day: "2026-04-08".to_owned(),
            points: vec![
                HeartRatePoint {
                    recorded_at: "2026-04-08T06:00:00Z".to_owned(),
                    bpm: 58,
                    source_day: Some("2026-04-08".to_owned()),
                },
                HeartRatePoint {
                    recorded_at: "2026-04-08T06:30:00Z".to_owned(),
                    bpm: 57,
                    source_day: Some("2026-04-08".to_owned()),
                },
                HeartRatePoint {
                    recorded_at: "2026-04-08T07:00:00Z".to_owned(),
                    bpm: 59,
                    source_day: Some("2026-04-08".to_owned()),
                },
                HeartRatePoint {
                    recorded_at: "2026-04-08T18:30:00Z".to_owned(),
                    bpm: 96,
                    source_day: Some("2026-04-08".to_owned()),
                },
                HeartRatePoint {
                    recorded_at: "2026-04-08T19:00:00Z".to_owned(),
                    bpm: 122,
                    source_day: Some("2026-04-08".to_owned()),
                },
                HeartRatePoint {
                    recorded_at: "2026-04-08T19:30:00Z".to_owned(),
                    bpm: 88,
                    source_day: Some("2026-04-08".to_owned()),
                },
            ],
        },
    ];
    let heartrate_daily_averages = vec![
        MetricPoint {
            day: "2026-04-05".to_owned(),
            value: 63.0,
        },
        MetricPoint {
            day: "2026-04-06".to_owned(),
            value: 64.0,
        },
        MetricPoint {
            day: "2026-04-07".to_owned(),
            value: 68.0,
        },
        MetricPoint {
            day: "2026-04-08".to_owned(),
            value: 72.0,
        },
    ];
    let context_events = vec![
        ContextEventRecord {
            context_event_id: "workout:demo-run".to_owned(),
            family: ContextEventFamily::Workout,
            source_id: "demo-run".to_owned(),
            anchor_day: "2026-04-08".to_owned(),
            start_at: "2026-04-08T18:20:00Z".to_owned(),
            end_at: Some("2026-04-08T19:05:00Z".to_owned()),
            time_semantics: TimeSemantics::Interval,
            title: "Evening run".to_owned(),
            subtype: Some("running".to_owned()),
            notes: Some("Moderate effort".to_owned()),
            intensity: Some("moderate".to_owned()),
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-08T20:00:00Z".to_owned(),
        },
        ContextEventRecord {
            context_event_id: "enhanced_tag:demo-caffeine".to_owned(),
            family: ContextEventFamily::EnhancedTag,
            source_id: "demo-caffeine".to_owned(),
            anchor_day: "2026-04-08".to_owned(),
            start_at: "2026-04-08T15:00:00Z".to_owned(),
            end_at: None,
            time_semantics: TimeSemantics::Point,
            title: "Coffee".to_owned(),
            subtype: Some("caffeine".to_owned()),
            notes: Some("Later than usual".to_owned()),
            intensity: Some("medium".to_owned()),
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-08T15:10:00Z".to_owned(),
        },
        ContextEventRecord {
            context_event_id: "session:demo-breathwork".to_owned(),
            family: ContextEventFamily::Session,
            source_id: "demo-breathwork".to_owned(),
            anchor_day: "2026-04-08".to_owned(),
            start_at: "2026-04-08T21:15:00Z".to_owned(),
            end_at: Some("2026-04-08T21:35:00Z".to_owned()),
            time_semantics: TimeSemantics::Interval,
            title: "Breathwork".to_owned(),
            subtype: Some("guided".to_owned()),
            notes: Some("Short guided session".to_owned()),
            intensity: Some("light".to_owned()),
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-08T21:40:00Z".to_owned(),
        },
    ];
    let pattern_summaries = vec![
        PatternSummaryRecord {
            summary_id: "pattern:run:readiness".to_owned(),
            family: ContextEventFamily::Workout,
            normalized_key: "running::moderate".to_owned(),
            relation_window: PatternRelationWindow::NextDayReadiness,
            metric: PatternMetric::ReadinessScore,
            sample_count: 6,
            median_delta: -4.0,
            effect_direction: EffectDirection::Lower,
            confidence: crate::store::queries::DataSufficiency::Medium,
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-08T22:00:00Z".to_owned(),
        },
        PatternSummaryRecord {
            summary_id: "pattern:caffeine:sleep".to_owned(),
            family: ContextEventFamily::EnhancedTag,
            normalized_key: "caffeine".to_owned(),
            relation_window: PatternRelationWindow::SameNightSleep,
            metric: PatternMetric::SleepScore,
            sample_count: 4,
            median_delta: -3.0,
            effect_direction: EffectDirection::Lower,
            confidence: crate::store::queries::DataSufficiency::Thin,
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-08T22:00:00Z".to_owned(),
        },
    ];
    let review_signal_days = vec![
        ReviewSignalDayRecord {
            signal_key: "sleep_score".to_owned(),
            day: "2026-04-08".to_owned(),
            numeric_value: Some(76.0),
            text_value: None,
            baseline_mean: Some(82.0),
            baseline_stddev: Some(2.5),
            delta: Some(-6.0),
            z_score: Some(-2.4),
            persistence_days: 2,
            sufficiency: crate::review::features::ReviewSufficiency::Strong,
            stale_days: 0,
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-08T22:30:00Z".to_owned(),
        },
        ReviewSignalDayRecord {
            signal_key: "readiness_score".to_owned(),
            day: "2026-04-08".to_owned(),
            numeric_value: Some(74.0),
            text_value: None,
            baseline_mean: Some(80.0),
            baseline_stddev: Some(2.0),
            delta: Some(-6.0),
            z_score: Some(-3.0),
            persistence_days: 2,
            sufficiency: crate::review::features::ReviewSufficiency::Strong,
            stale_days: 0,
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-08T22:30:00Z".to_owned(),
        },
        ReviewSignalDayRecord {
            signal_key: "activity_score".to_owned(),
            day: "2026-04-08".to_owned(),
            numeric_value: Some(88.0),
            text_value: None,
            baseline_mean: Some(74.0),
            baseline_stddev: Some(4.0),
            delta: Some(14.0),
            z_score: Some(3.5),
            persistence_days: 1,
            sufficiency: crate::review::features::ReviewSufficiency::Medium,
            stale_days: 0,
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-08T22:30:00Z".to_owned(),
        },
        ReviewSignalDayRecord {
            signal_key: "stress_high".to_owned(),
            day: "2026-04-08".to_owned(),
            numeric_value: Some(170.0),
            text_value: None,
            baseline_mean: Some(85.0),
            baseline_stddev: Some(20.0),
            delta: Some(85.0),
            z_score: Some(4.25),
            persistence_days: 3,
            sufficiency: crate::review::features::ReviewSufficiency::Medium,
            stale_days: 0,
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-08T22:30:00Z".to_owned(),
        },
    ];
    let sleep_time = vec![SleepTimeRecord {
        oura_id: Some("demo-sleep-time".to_owned()),
        day: "2026-04-08".to_owned(),
        status: Some("late".to_owned()),
        recommendation: Some("earlier_bedtime".to_owned()),
        optimal_bedtime_start_offset: Some(77400),
        optimal_bedtime_end_offset: Some(81000),
        optimal_bedtime_day_tz: Some(0),
        raw_cache_key: None,
        updated_at: "2026-04-08T22:00:00Z".to_owned(),
    }];
    let rest_mode_periods = vec![RestModePeriodRecord {
        period_id: "demo-rest-mode".to_owned(),
        start_day: "2026-04-07".to_owned(),
        start_time: Some("2026-04-07T00:00:00Z".to_owned()),
        end_day: Some("2026-04-08".to_owned()),
        end_time: Some("2026-04-08T08:00:00Z".to_owned()),
        episode_count: 1,
        tags_json: "[]".to_owned(),
        raw_cache_key: None,
        updated_at: "2026-04-08T08:00:00Z".to_owned(),
    }];

    LiveSnapshot {
        captured_at: "2026-04-08T22:30:00Z".to_owned(),
        refresh_policy: RefreshPolicySnapshot::from_config(config),
        auth_status,
        webhook: WebhookOpsSnapshot {
            bind_address: "127.0.0.1:8799".to_owned(),
            path: "/webhooks/oura".to_owned(),
            callback_url: Some("https://example.ngrok.dev/webhooks/oura".to_owned()),
            verification_token_configured: true,
            signature_tolerance_secs: 300,
            heartbeat_secs: 15,
            renewal_lead_secs: 7 * 24 * 60 * 60,
            desired_subscriptions: Vec::new(),
            remote_subscriptions: Vec::new(),
            recent_deliveries: Vec::new(),
            latest_rejected_delivery: None,
            pending_invalidations: Vec::new(),
            recent_processing_attempts: Vec::new(),
            runtime_heartbeats: Vec::new(),
        },
        personal_info: Some(PersonalInfoRecord {
            profile_id: "demo-user".to_owned(),
            age: Some(34),
            weight: Some(72.4),
            height: Some(178.0),
            biological_sex: Some("male".to_owned()),
            email: Some("demo@example.com".to_owned()),
            raw_cache_key: Some("demo".to_owned()),
            updated_at: "2026-04-08T22:00:00Z".to_owned(),
        }),
        daily_history,
        heartrate_days,
        heartrate_daily_averages,
        context_events,
        pattern_summaries,
        review_signal_days,
        sleep_time,
        rest_mode_periods,
        sync_states: vec![
            demo_sync_state(
                SyncFamily::Personal,
                "Personal info is current.",
                SyncRunStatus::Success,
            ),
            demo_sync_state(
                SyncFamily::Daily,
                "Daily closeout landed for the selected day.",
                SyncRunStatus::Success,
            ),
            demo_sync_state(
                SyncFamily::Heartrate,
                "Heartrate samples are current.",
                SyncRunStatus::Success,
            ),
            demo_sync_state(
                SyncFamily::Workout,
                "Workouts are current.",
                SyncRunStatus::Success,
            ),
            demo_sync_state(
                SyncFamily::EnhancedTag,
                "Enhanced tags are current.",
                SyncRunStatus::Success,
            ),
            demo_sync_state(
                SyncFamily::Session,
                "Sessions are current.",
                SyncRunStatus::Success,
            ),
        ],
        record_counts: RecordCounts {
            raw_payloads: 12,
            personal_info: 1,
            daily_sleep: 4,
            daily_readiness: 4,
            daily_activity: 4,
            heartrate_samples: 9,
            workouts: 1,
            tags: 0,
            enhanced_tags: 1,
            sessions: 1,
            derived_context_events: 3,
            derived_pattern_summaries: 2,
            sleep_time: 1,
            daily_stress: 1,
            rest_mode_periods: 1,
            derived_review_signal_days: 4,
            ..RecordCounts::default()
        },
        schema_version: 11,
        database_path: config.paths.database_file.display().to_string(),
        config_path: config.paths.config_file.display().to_string(),
    }
}

fn demo_sync_state(family: SyncFamily, message: &str, status: SyncRunStatus) -> SyncStateRecord {
    SyncStateRecord {
        sync_key: family.sync_key().to_owned(),
        status,
        cursor: None,
        last_attempted_at: "2026-04-08T22:00:00Z".to_owned(),
        last_completed_at: Some("2026-04-08T22:01:00Z".to_owned()),
        message: Some(message.to_owned()),
        granted_scopes: vec![family.capability_kind().scope_name().to_owned()],
        last_error: None,
        failure_count: 0,
        next_attempt_after: None,
        last_trigger_source: Some("periodic_reconcile".to_owned()),
        last_trigger_detail: Some("demo snapshot".to_owned()),
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::{
        AppState, DataFamily, HeartRateDay, LiveModelOptions, LiveSnapshot, OverlayFilterState,
        PatternMetricFilter, RefreshPolicySnapshot, ReviewScreenMode, RunMode, Screen,
        TrendWindowKind, WebhookOpsSnapshot, build_live_model, newest_day_index,
    };
    use crate::action::Action;
    use crate::insights::MetricPoint;
    use crate::oura::models::{AuthStatus, CapabilityReport};
    use crate::review::{
        InvestigationReport, ReviewCard, ReviewConfidence, ReviewDeck, ReviewFocus, ReviewMode,
        ReviewSection, ReviewSufficiency,
    };
    use crate::store::queries::{
        ContextEventFamily, ContextEventRecord, HeartRatePoint, RecordCounts, TimeSemantics,
    };

    fn make_review_card(id: &str, signal_key: &str, score: i32) -> ReviewCard {
        ReviewCard {
            id: id.to_owned(),
            signal_key: signal_key.to_owned(),
            headline: format!("{signal_key} changed"),
            summary: format!("{signal_key} summary"),
            why_this_is_shown: "why".to_owned(),
            confidence: ReviewConfidence::Medium,
            sufficiency: ReviewSufficiency::Medium,
            confidence_label: "Medium confidence / Medium data".to_owned(),
            section: ReviewSection::NegativeDrift,
            score,
            anchor_day: "2026-04-08".to_owned(),
            evidence: vec![format!("{signal_key} evidence")],
            counterevidence: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn make_snapshot(days: &[&str]) -> LiveSnapshot {
        let heartrate_days = days
            .iter()
            .enumerate()
            .map(|(index, day)| HeartRateDay {
                day: (*day).to_owned(),
                points: vec![HeartRatePoint {
                    recorded_at: format!("{day}T0{}:00:00Z", index + 6),
                    bpm: 60 + index as u16,
                    source_day: Some((*day).to_owned()),
                }],
            })
            .collect();

        LiveSnapshot {
            captured_at: "2026-04-08T12:00:00Z".to_owned(),
            refresh_policy: RefreshPolicySnapshot {
                personal_interval_secs: 3600,
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
            },
            auth_status: AuthStatus {
                configured: true,
                callback_url: "http://127.0.0.1:8788/callback".to_owned(),
                requested_scopes: vec![
                    "personal".to_owned(),
                    "daily".to_owned(),
                    "heartrate".to_owned(),
                    "workout".to_owned(),
                    "enhanced_tag".to_owned(),
                    "session".to_owned(),
                ],
                granted_scopes: vec![
                    "personal".to_owned(),
                    "daily".to_owned(),
                    "heartrate".to_owned(),
                    "workout".to_owned(),
                    "enhanced_tag".to_owned(),
                    "session".to_owned(),
                ],
                missing_fields: Vec::new(),
                capability_report: CapabilityReport::demo(),
                auth_timeout_secs: 120,
                secret_backend: "memory".to_owned(),
                access_token_stored: true,
                refresh_token_stored: true,
                access_token_expires_at: None,
                last_authenticated_at: None,
                last_refresh_at: None,
                account_id: None,
                account_email: None,
                last_error: None,
            },
            webhook: WebhookOpsSnapshot::default(),
            personal_info: None,
            daily_history: days
                .iter()
                .map(|day| crate::store::queries::DailyOverviewRow {
                    day: (*day).to_owned(),
                    sleep_score: Some(80),
                    readiness_score: Some(80),
                    activity_score: Some(70),
                    updated_at: "2026-04-08T12:00:00Z".to_owned(),
                })
                .collect(),
            heartrate_days,
            heartrate_daily_averages: days
                .iter()
                .map(|day| MetricPoint {
                    day: (*day).to_owned(),
                    value: 65.0,
                })
                .collect(),
            context_events: vec![ContextEventRecord {
                context_event_id: "workout:test".to_owned(),
                family: ContextEventFamily::Workout,
                source_id: "test".to_owned(),
                anchor_day: "2026-04-08".to_owned(),
                start_at: "2026-04-08T06:00:00Z".to_owned(),
                end_at: Some("2026-04-08T06:30:00Z".to_owned()),
                time_semantics: TimeSemantics::Interval,
                title: "Workout".to_owned(),
                subtype: Some("running".to_owned()),
                notes: None,
                intensity: Some("moderate".to_owned()),
                metadata_json: "{}".to_owned(),
                updated_at: "2026-04-08T06:40:00Z".to_owned(),
            }],
            pattern_summaries: Vec::new(),
            review_signal_days: Vec::new(),
            sleep_time: Vec::new(),
            rest_mode_periods: Vec::new(),
            sync_states: Vec::new(),
            record_counts: RecordCounts {
                workouts: 1,
                ..RecordCounts::default()
            },
            schema_version: 11,
            database_path: ":memory:".to_owned(),
            config_path: "config.toml".to_owned(),
        }
    }

    fn make_live_app(snapshot: LiveSnapshot) -> AppState {
        let selected_day_index = newest_day_index(&snapshot);
        let mut app = AppState {
            mode: RunMode::Live,
            active_screen: Screen::Timeline,
            model: super::AppModel::empty(),
            status_line: String::new(),
            tick_count: 0,
            should_quit: false,
            refresh_in_flight: false,
            live_snapshot: Some(snapshot),
            selected_day_index,
            selected_timeline_point: 0,
            timeline_window_hours: 24,
            trends_window: TrendWindowKind::Days7,
            selected_event_id: None,
            selected_review_card_index: 0,
            overlay_filters: OverlayFilterState::all(),
            pattern_metric_filter: PatternMetricFilter::All,
            review_mode: ReviewScreenMode::Today,
            review_focus: ReviewFocus::Readiness,
        };
        app.select_default_event_for_selected_day();
        app.rebuild_live_model();
        app
    }

    #[test]
    fn live_selection_defaults_to_newest_available_day() {
        let snapshot = make_snapshot(&["2026-04-06", "2026-04-07", "2026-04-08"]);
        let app = make_live_app(snapshot);

        assert_eq!(app.selected_day_index, 2);
        assert_eq!(app.model.timeline.selected_day_label, "2026-04-08");
    }

    #[test]
    fn day_actions_update_shared_selected_day() {
        let snapshot = make_snapshot(&["2026-04-06", "2026-04-07", "2026-04-08"]);
        let mut app = make_live_app(snapshot);

        app.handle(Action::PreviousDay);
        assert_eq!(app.model.timeline.selected_day_label, "2026-04-07");
        assert_eq!(app.model.dashboard.selected_day_label, "2026-04-07");
        assert_eq!(app.model.explain.selected_day_label, "2026-04-07");

        app.handle(Action::NextDay);
        assert_eq!(app.model.timeline.selected_day_label, "2026-04-08");
    }

    #[test]
    fn timeline_cursor_keeps_context_event_selected() {
        let snapshot = make_snapshot(&["2026-04-08"]);
        let mut app = make_live_app(snapshot);

        app.handle(Action::NextTimelinePoint);
        assert_eq!(app.model.timeline.selected_event_index, Some(0));
    }

    #[test]
    fn tag_filter_hides_non_matching_events() {
        let snapshot = make_snapshot(&["2026-04-08"]);
        let mut app = make_live_app(snapshot);

        app.handle(Action::ToggleWorkoutFilter);
        assert!(app.model.timeline.events.is_empty());
        assert_eq!(app.model.timeline.selected_event_index, None);
    }

    #[test]
    fn offset_timestamp_event_remains_visible_on_anchor_day() {
        let event = ContextEventRecord {
            context_event_id: "workout:late-offset".to_owned(),
            family: ContextEventFamily::Workout,
            source_id: "late-offset".to_owned(),
            anchor_day: "2026-04-08".to_owned(),
            start_at: "2026-04-08T23:30:00-07:00".to_owned(),
            end_at: Some("2026-04-08T23:45:00-07:00".to_owned()),
            time_semantics: TimeSemantics::Interval,
            title: "Late workout".to_owned(),
            subtype: Some("running".to_owned()),
            notes: None,
            intensity: Some("moderate".to_owned()),
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-09T07:00:00Z".to_owned(),
        };

        let bounds = super::event_bounds_for_day(&event, "2026-04-08")
            .unwrap_or_else(|| panic!("event should remain visible on its local anchor day"));
        assert_eq!(bounds, (23 * 60 + 30, 23 * 60 + 45));
    }

    #[test]
    fn all_day_events_do_not_leak_into_other_days() {
        let event = ContextEventRecord {
            context_event_id: "enhanced_tag:all-day".to_owned(),
            family: ContextEventFamily::EnhancedTag,
            source_id: "all-day".to_owned(),
            anchor_day: "2026-04-08".to_owned(),
            start_at: "2026-04-08T00:00:00Z".to_owned(),
            end_at: Some("2026-04-08T23:59:59Z".to_owned()),
            time_semantics: TimeSemantics::AllDay,
            title: "Travel".to_owned(),
            subtype: Some("travel".to_owned()),
            notes: None,
            intensity: None,
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-08T00:00:00Z".to_owned(),
        };

        assert_eq!(
            super::event_bounds_for_day(&event, "2026-04-08"),
            Some((0, 24 * 60 - 1))
        );
        assert_eq!(super::event_bounds_for_day(&event, "2026-04-07"), None);
    }

    #[test]
    fn freshness_knows_context_families() {
        let snapshot = make_snapshot(&["2026-04-08"]);
        let freshness = super::family_freshness(&snapshot, DataFamily::Workout);
        assert_eq!(freshness.summary, "stale: receiver down");
    }

    #[test]
    fn receiver_status_is_config_incomplete_without_client_secret() {
        let mut snapshot = make_snapshot(&["2026-04-08"]);
        snapshot.webhook.callback_url = Some("https://example.ngrok.dev/webhooks/oura".to_owned());
        snapshot.webhook.verification_token_configured = true;
        snapshot.auth_status.missing_fields = vec!["client_secret"];

        assert_eq!(
            super::receiver_status_line(&snapshot),
            "config incomplete".to_owned()
        );
    }

    #[test]
    fn receiver_status_reports_missing_heartbeat_when_config_complete() {
        let mut snapshot = make_snapshot(&["2026-04-08"]);
        snapshot.webhook.callback_url = Some("https://example.ngrok.dev/webhooks/oura".to_owned());
        snapshot.webhook.verification_token_configured = true;
        snapshot.webhook.runtime_heartbeats.clear();

        assert_eq!(
            super::receiver_status_line(&snapshot),
            "missing heartbeat".to_owned()
        );
    }

    #[test]
    fn build_live_model_exposes_patterns_screen() {
        let snapshot = make_snapshot(&["2026-04-08"]);
        let model = build_live_model(
            &snapshot,
            &LiveModelOptions {
                selected_day_index: 0,
                selected_point_index: 0,
                selected_event_id: None,
                overlay_filters: OverlayFilterState::all(),
                window_hours: 24,
                trends_window: TrendWindowKind::Days7,
                pattern_metric_filter: PatternMetricFilter::All,
                refresh_in_flight: false,
                review_mode: ReviewScreenMode::Today,
                review_focus: ReviewFocus::Readiness,
                selected_review_card_index: 0,
            },
        );
        assert!(model.patterns.empty_message.contains("Not enough data yet"));
    }

    #[test]
    fn investigate_mode_includes_focus_cards_outside_top_observations() {
        let today = ReviewDeck {
            mode: ReviewMode::Today,
            anchor_day: "2026-04-08".to_owned(),
            observations: vec![
                make_review_card("1", "sleep_score", 10),
                make_review_card("2", "readiness_score", 9),
                make_review_card("3", "activity_score", 8),
                make_review_card("4", "active_calories", 7),
                make_review_card("5", "steps", 6),
            ],
            positive_changes: Vec::new(),
            negative_drifts: vec![make_review_card("6", "stress_high", 5)],
            unresolved_anomalies: Vec::new(),
            warnings: Vec::new(),
        };
        let week = ReviewDeck {
            mode: ReviewMode::Week,
            anchor_day: "2026-04-08".to_owned(),
            observations: Vec::new(),
            positive_changes: Vec::new(),
            negative_drifts: Vec::new(),
            unresolved_anomalies: Vec::new(),
            warnings: Vec::new(),
        };
        let investigation = InvestigationReport {
            focus: ReviewFocus::Stress,
            anchor_day: "2026-04-08".to_owned(),
            headline: "Stress: stress_high changed".to_owned(),
            summary: "stress summary".to_owned(),
            confidence: ReviewConfidence::Medium,
            sufficiency: ReviewSufficiency::Medium,
            evidence: vec!["stress_high evidence".to_owned()],
            counterevidence: Vec::new(),
            warnings: Vec::new(),
            look_at: Vec::new(),
        };

        let cards = super::review_cards_for_mode(
            ReviewScreenMode::Investigate,
            &today,
            &week,
            &investigation,
        );

        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].signal_key, "stress_high");
    }
}
