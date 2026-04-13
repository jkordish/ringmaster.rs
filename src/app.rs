use std::collections::{BTreeMap, BTreeSet};
use std::env;

use crate::action::Action;
use crate::ai::{
    self, AiRequestPreview, AiRequestPreviewSnapshot, GuidedFollowUpKind, StoredArtifact,
};
use crate::ai_prompts::{COMPARE_PROMPT_VERSION, REVIEW_PROMPT_VERSION};
use crate::config::{AiRequestMode, Config};
use crate::eval::{
    EvalArtifactLineage, EvalExpectations, EvalScoreSummary, PersistedEvalArtifactDetail,
    PersistedEvalCaseDetail, PersistedEvalGraderResult, PersistedEvalRunDetails,
    parse_persisted_eval_details,
};
use crate::evidence::policy::{claim_language_spec, evidence_badges, guidance_comparison_text};
use crate::evidence::{
    PopulationProfile, PopulationSupportStatus, evidence_registry_version, stale_evidence_warnings,
};
use crate::insights::{InsightConfidence, MetricInsight, MetricPoint, build_metric_insight};
use crate::keybindings::BindingContext;
use crate::navigation::{
    self, FocusRegion, NavMove, PreflightControl, SearchScope, SearchState, TransientLayer,
};
use crate::oura::models::{AuthStatus, CapabilityKind, CapabilityReport};
use crate::refresh::SyncFamily;
use crate::review::{
    InvestigationReport, ReviewCard, ReviewDeck, ReviewFocus, ReviewInputs, ReviewMode,
    build_investigation_report, build_review_deck, ranked_cards,
};
use crate::snapshot::PrivacyProfile;
use crate::store::Store;
use crate::store::queries::{
    AiArtifactDaySummaryRecord, AiArtifactRecord, AiEvalRunRecord, AiRunRecord, ContextEventFamily,
    ContextEventRecord, DailyActivityRecord, DailyCardiovascularAgeRecord, DailyOverviewRow,
    DailyReadinessRecord, DailyResilienceRecord, DailySpO2Record, DailyStressRecord,
    EffectDirection, HeartRatePoint, PatternMetric, PatternRelationWindow, PatternSummaryRecord,
    PersonalInfoRecord, RecordCounts, ReportExportRecord, RestModePeriodRecord,
    ReviewSignalDayRecord, SleepPeriodRecord, SleepTimeRecord, SnapshotCatalogEntry, SyncRunStatus,
    SyncStateRecord, TimeSemantics, Vo2MaxRecord,
};
use crate::store::webhook_store::{
    AcceptedWebhookDeliveryRecord, DesiredWebhookSubscriptionRecord, InvalidationRecord,
    ProcessingAttemptRecord, RejectedWebhookDeliveryRecord, RemoteWebhookSubscriptionRecord,
    RuntimeHeartbeatRecord,
};
use crate::time_utils::current_local_day_string;
use crate::ui::{
    layout::ViewportClass,
    telemetry::{TelemetryAvailability, footer_inspector},
};
use serde::Serialize;
use time::{Date, OffsetDateTime, format_description::well_known::Rfc3339};

const LIVE_REVIEW_SIGNAL_LOOKBACK_DAYS: i64 = 60;
const LIVE_REVIEW_SLEEP_LOOKBACK_DAYS: i64 = 60;
const LIVE_REVIEW_CONTEXT_LOOKBACK_DAYS: i64 = 90;
const LIVE_REVIEW_REST_MODE_LOOKBACK_DAYS: i64 = 180;
const LIVE_REVIEW_CONTEXT_FORWARD_DAYS: i64 = 7;
const TIMELINE_WINDOW_PRESETS: [u16; 3] = [6, 12, 24];

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoverageFamily {
    Daily,
    Heartrate,
    Workout,
    Tag,
    Session,
    Spo2,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveSnapshot {
    pub captured_at: String,
    pub refresh_policy: RefreshPolicySnapshot,
    pub auth_status: AuthStatus,
    pub active_population_profile: PopulationProfile,
    pub guidance_profile_source: String,
    pub evidence_registry_version: String,
    pub stale_evidence_entries: Vec<String>,
    pub ai_ops: AiOpsSnapshot,
    pub webhook: WebhookOpsSnapshot,
    pub personal_info: Option<PersonalInfoRecord>,
    pub daily_history: Vec<DailyOverviewRow>,
    pub daily_activity: Vec<DailyActivityRecord>,
    pub daily_readiness: Vec<DailyReadinessRecord>,
    pub daily_stress: Vec<DailyStressRecord>,
    pub sleep_periods: Vec<SleepPeriodRecord>,
    pub daily_spo2: Vec<DailySpO2Record>,
    pub heartrate_days: Vec<HeartRateDay>,
    pub heartrate_daily_averages: Vec<MetricPoint>,
    pub context_events: Vec<ContextEventRecord>,
    pub pattern_summaries: Vec<PatternSummaryRecord>,
    pub review_signal_days: Vec<ReviewSignalDayRecord>,
    pub sleep_time: Vec<SleepTimeRecord>,
    pub rest_mode_periods: Vec<RestModePeriodRecord>,
    pub daily_resilience: Vec<DailyResilienceRecord>,
    pub daily_cardiovascular_age: Vec<DailyCardiovascularAgeRecord>,
    pub vo2_max: Vec<Vo2MaxRecord>,
    pub ai_artifacts_by_day: BTreeMap<String, AiArtifactDaySummaryRecord>,
    pub snapshot_catalog: Vec<SnapshotCatalogEntry>,
    pub ai_runs: Vec<AiRunRecord>,
    pub ai_artifact_records: Vec<AiArtifactRecord>,
    pub report_exports: Vec<ReportExportRecord>,
    pub ai_eval_runs: Vec<AiEvalRunRecord>,
    pub sync_states: Vec<SyncStateRecord>,
    pub record_counts: RecordCounts,
    pub schema_version: u32,
    pub database_path: String,
    pub config_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiOpsSnapshot {
    pub enabled: bool,
    pub provider: String,
    pub api_key_env: String,
    pub api_key_ready: bool,
    pub default_model: String,
    pub reasoning_effort: String,
    pub request_mode: String,
    pub input_transport: String,
    pub prompt_cache: String,
    pub review_prompt_version: String,
    pub compare_prompt_version: String,
    pub tools_disabled: bool,
    pub snapshot_catalog_count: usize,
    pub ai_run_count: usize,
    pub ai_artifact_count: usize,
    pub report_export_count: usize,
    pub ai_eval_run_count: usize,
    pub last_successful_run: Option<String>,
    pub last_failed_run: Option<String>,
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
    Review,
    Ai,
    Ops,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiBrowserTab {
    Runs,
    Snapshots,
    Reports,
    Evals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiLaunchIntent {
    ReviewSelectedDay,
    CompareSelectedWeek,
    ChallengeSelectedDay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendWindowKind {
    Days7,
    Days30,
    Days90,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendSortMode {
    Concern,
    Anomaly,
    Recovery,
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
    focused_region: FocusRegion,
    screen_focus_memory: [FocusRegion; 8],
    focused_top_nav_screen: Screen,
    help_open: bool,
    focus_before_help: Option<FocusRegion>,
    search: Option<SearchState>,
    selected_day_index: usize,
    selected_timeline_point: usize,
    timeline_window_hours: u16,
    selected_overlay_toggle_index: usize,
    trends_window: TrendWindowKind,
    trend_sort_mode: TrendSortMode,
    selected_trend_row_index: usize,
    selected_event_id: Option<String>,
    selected_dashboard_breakdown_index: usize,
    expanded_region: Option<FocusRegion>,
    selected_review_card_index: usize,
    ai_preflight: Option<AiPreflightState>,
    ai_preflight_control: PreflightControl,
    ai_browser_tab: AiBrowserTab,
    selected_ai_launch_index: usize,
    selected_ai_run_index: usize,
    selected_snapshot_catalog_index: usize,
    selected_report_export_index: usize,
    selected_ai_eval_run_index: usize,
    selected_ai_artifact_action_index: usize,
    overlay_filters: OverlayFilterState,
    pattern_metric_filter: PatternMetricFilter,
    review_mode: ReviewScreenMode,
    review_focus: ReviewFocus,
}

#[derive(Debug, Clone)]
pub struct AppModel {
    pub title: String,
    pub dashboard: DashboardModel,
    pub timeline: TimelineModel,
    pub trends: TrendsModel,
    pub explain: ExplainModel,
    pub patterns: PatternsModel,
    pub review: ReviewModel,
    pub ai: AiWorkbenchModel,
    pub ops: OpsModel,
}

impl PartialEq for AppModel {
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title
            && self.dashboard == other.dashboard
            && self.timeline == other.timeline
            && self.trends == other.trends
            && self.explain == other.explain
            && self.patterns == other.patterns
            && self.review == other.review
            && self.ai == other.ai
            && self.ops == other.ops
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardModel {
    pub header: HeaderStripModel,
    pub selected_day_label: String,
    pub readiness: DashboardScoreTile,
    pub sleep: DashboardSleepTile,
    pub activity: DashboardScoreTile,
    pub hrv: DashboardTrendPanel,
    pub body_temp: DashboardThermometerPanel,
    pub heart_rate: DashboardTrendPanel,
    pub spo2: DashboardTrendPanel,
    pub respiratory_rate: DashboardHistogramPanel,
    pub breakdown: DashboardBreakdownPanel,
    pub weekly: DashboardWeeklyHeatmap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineModel {
    pub summary: String,
    pub breadcrumb: String,
    pub day_selector: String,
    pub window_presets: Vec<TimelineWindowPresetView>,
    pub selected_window_preset_index: usize,
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
    pub sort_tabs: Vec<TrendSortTab>,
    pub selected_sort_index: usize,
    pub rows: Vec<TrendMatrixRow>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplainModel {
    pub selected_day_label: String,
    pub breadcrumb: String,
    pub headline: String,
    pub overlay_toggles: Vec<OverlayToggleView>,
    pub selected_overlay_toggle_index: usize,
    pub claim_availability: TelemetryAvailability,
    pub summary_lines: Vec<String>,
    pub measurements_availability: TelemetryAvailability,
    pub evidence_badges: Vec<String>,
    pub measurement_lines: Vec<String>,
    pub evidence_availability: TelemetryAvailability,
    pub evidence_lines: Vec<String>,
    pub uncertainty_availability: TelemetryAvailability,
    pub caveat_lines: Vec<String>,
    pub context_availability: TelemetryAvailability,
    pub context_lines: Vec<String>,
    pub ai_availability: TelemetryAvailability,
    pub ai_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternsModel {
    pub header: String,
    pub metric_filters: Vec<PatternFilterTab>,
    pub selected_filter_index: usize,
    pub overlay_toggles: Vec<OverlayToggleView>,
    pub selected_overlay_toggle_index: usize,
    pub filter_summary: String,
    pub findings_availability: TelemetryAvailability,
    pub rows: Vec<PatternRowView>,
    pub guide_availability: TelemetryAvailability,
    pub notes: Vec<String>,
    pub interpretation_availability: TelemetryAvailability,
    pub empty_message: String,
    pub ai_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsModel {
    pub mode_label: String,
    pub summary_lines: Vec<String>,
    pub coverage: Vec<CoverageCellView>,
    pub family_statuses: Vec<FamilyStatusView>,
    pub items: Vec<OpsItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderStripModel {
    pub app_title: String,
    pub selected_period: String,
    pub freshness_badge: String,
    pub sync_status: String,
    pub capability_summary: Vec<String>,
    pub coverage: Vec<CoverageCellView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageCellView {
    pub label: &'static str,
    pub availability: TelemetryAvailability,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardScoreTile {
    pub availability: TelemetryAvailability,
    pub primary_value: String,
    pub secondary_lines: Vec<String>,
    pub delta_label: String,
    pub trend: Vec<u64>,
    pub ring_fill_percent: u16,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardSleepTile {
    pub availability: TelemetryAvailability,
    pub duration_label: String,
    pub score_label: String,
    pub trend: Vec<u64>,
    pub strip_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardTrendPanel {
    pub availability: TelemetryAvailability,
    pub primary_label: String,
    pub baseline_label: String,
    pub range_label: String,
    pub values: Vec<u64>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardThermometerPanel {
    pub availability: TelemetryAvailability,
    pub deviation_tenths: Option<i16>,
    pub value_label: String,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardHistogramPanel {
    pub availability: TelemetryAvailability,
    pub primary_label: String,
    pub bars: Vec<u64>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardBreakdownPanel {
    pub availability: TelemetryAvailability,
    pub rails: Vec<DashboardBreakdownRail>,
    pub waveform: Vec<u64>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardBreakdownRail {
    pub label: String,
    pub availability: TelemetryAvailability,
    pub fill_percent: u16,
    pub delta_label: String,
    pub note: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardHeatmapGrid {
    pub day_labels: Vec<String>,
    pub rows: Vec<Vec<Option<u8>>>,
    pub selected_cell: Option<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardWeeklyHeatmap {
    pub availability: TelemetryAvailability,
    pub row_labels: Vec<String>,
    pub recent: DashboardHeatmapGrid,
    pub history: DashboardHeatmapGrid,
    pub note: String,
}

impl DashboardWeeklyHeatmap {
    #[must_use]
    pub const fn grid_for_viewport(&self, viewport: ViewportClass) -> &DashboardHeatmapGrid {
        if viewport.is_wide() && self.history.day_labels.len() > self.recent.day_labels.len() {
            &self.history
        } else {
            &self.recent
        }
    }

    #[must_use]
    pub fn selected_summary_for_viewport(&self, viewport: ViewportClass) -> String {
        let grid = self.grid_for_viewport(viewport);
        grid.selected_cell
            .and_then(|(row_index, column_index)| {
                let row = grid.rows.get(row_index)?;
                let value = row.get(column_index).copied().flatten()?;
                let row_label = self.row_labels.get(row_index)?;
                let day_label = grid.day_labels.get(column_index)?;
                Some(format!("{row_label} {value} on {day_label}"))
            })
            .unwrap_or_else(|| self.note.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrendSortTab {
    pub label: &'static str,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrendMatrixRow {
    pub label: &'static str,
    pub current_value: String,
    pub concern_label: String,
    pub selected: bool,
    pub cells: Vec<TrendMatrixCell>,
    pub sparkline: Vec<u64>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrendMatrixCell {
    pub label: &'static str,
    pub delta_label: String,
    pub fill_percent: u16,
    pub availability: TelemetryAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewModel {
    pub selected_day_label: String,
    pub breadcrumb: String,
    pub mode_tabs: Vec<ReviewTab>,
    pub selected_mode_index: usize,
    pub focus_tabs: Vec<ReviewTab>,
    pub selected_focus_index: usize,
    pub cards_availability: TelemetryAvailability,
    pub cards: Vec<ReviewCardView>,
    pub selected_card_index: Option<usize>,
    pub ai_artifact: AiArtifactSummaryView,
    pub detail_availability: TelemetryAvailability,
    pub detail_lines: Vec<String>,
    pub warnings_availability: TelemetryAvailability,
    pub warning_lines: Vec<String>,
    pub empty_message: String,
    pub ai_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiWorkbenchModel {
    pub headline: String,
    pub summary_lines: Vec<String>,
    pub launch_points: Vec<AiLaunchPointView>,
    pub browser_tabs: Vec<AiBrowserTabView>,
    pub selected_tab_index: usize,
    pub browser_items: Vec<AiBrowserItemView>,
    pub selected_item_index: Option<usize>,
    pub artifact_actions: Vec<AiArtifactActionView>,
    pub selected_action_index: Option<usize>,
    pub detail_title: String,
    pub detail_lines: Vec<String>,
    pub trust_lines: Vec<String>,
    pub warning_lines: Vec<String>,
    pub preflight: Option<AiPreflightView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiLaunchPointView {
    pub intent: AiLaunchIntent,
    pub label: String,
    pub detail: String,
    pub key_hint: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiPreflightState {
    pub intent: AiLaunchIntent,
    pub source_screen: Screen,
    pub snapshot_scope: String,
    pub snapshot_paths: Vec<String>,
    pub request_preview: AiRequestPreview,
    pub privacy_profile: PrivacyProfile,
    pub model_override: Option<String>,
    pub source_ai_artifact_id: Option<String>,
    pub follow_up_kind: Option<GuidedFollowUpKind>,
    pub warning_lines: Vec<String>,
    pub confirm_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiPreflightView {
    pub title: String,
    pub body_lines: Vec<String>,
    pub warning_lines: Vec<String>,
    pub controls: Vec<AiPreflightControlView>,
    pub selected_control_index: usize,
    pub confirm_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiBrowserTabView {
    pub label: String,
    pub count: usize,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiBrowserItemView {
    pub headline: String,
    pub detail: String,
    pub status_badge: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiArtifactActionView {
    pub label: String,
    pub detail: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiPreflightControlView {
    pub label: &'static str,
    pub detail: String,
    pub selected: bool,
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
    pub badges: Vec<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiArtifactSummaryView {
    pub availability: TelemetryAvailability,
    pub status_label: String,
    pub metadata_lines: Vec<String>,
    pub summary_text: String,
    pub lineage_lines: Vec<String>,
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
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineWindowPresetView {
    pub label: &'static str,
    pub selected: bool,
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
struct ExplainSupportingEvent {
    family_label: &'static str,
    headline: String,
    detail: String,
    selected: bool,
    source_day: String,
    carried_forward: bool,
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
pub struct PatternFilterTab {
    pub label: &'static str,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternRowView {
    pub headline: String,
    pub detail: String,
    pub badges: Vec<String>,
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
    ai_preflight: Option<AiPreflightState>,
    ai_preflight_control: PreflightControl,
    ai_browser_tab: AiBrowserTab,
    selected_ai_launch_index: usize,
    selected_ai_run_index: usize,
    selected_snapshot_catalog_index: usize,
    selected_report_export_index: usize,
    selected_ai_eval_run_index: usize,
    selected_ai_artifact_action_index: usize,
    overlay_filters: OverlayFilterState,
    selected_overlay_toggle_index: usize,
    window_hours: u16,
    trends_window: TrendWindowKind,
    trend_sort_mode: TrendSortMode,
    selected_trend_row_index: usize,
    pattern_metric_filter: PatternMetricFilter,
    refresh_in_flight: bool,
    review_mode: ReviewScreenMode,
    review_focus: ReviewFocus,
    selected_review_card_index: usize,
    selected_dashboard_breakdown_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct EvalComparisonCounts {
    improvements: usize,
    regressions: usize,
    matched: usize,
    candidate_only: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvalHealthSummary {
    created_at: String,
    labels: String,
    failed_cases: u32,
    regression_count: usize,
    improvement_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiBrowserContent {
    browser_items: Vec<AiBrowserItemView>,
    selected_item_index: Option<usize>,
    artifact_actions: Vec<AiArtifactActionView>,
    selected_action_index: Option<usize>,
    detail_title: String,
    detail_lines: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiArtifactActionKind {
    CancelRun,
    ExpandEvidence,
    ShowCounterevidence,
    ExplainRanking,
    SuggestDrilldown,
    GenerateReport,
    RerunNextPrivacy,
    RerunNextModel,
    ComparePreviousSnapshot,
    OpenLinkedEvidence,
}

#[derive(Debug, Clone)]
struct ReviewViewContext<'a> {
    selected_day: &'a str,
    ai_artifact: &'a AiArtifactSummaryView,
    review_mode: ReviewScreenMode,
    review_focus: ReviewFocus,
    selected_review_card_index: usize,
}

impl AiArtifactActionKind {
    const fn label(self) -> &'static str {
        match self {
            Self::CancelRun => "Cancel run",
            Self::ExpandEvidence => "Expand evidence",
            Self::ShowCounterevidence => "Show counterevidence",
            Self::ExplainRanking => "Explain ranking",
            Self::SuggestDrilldown => "Suggest drill-down",
            Self::GenerateReport => "Generate report",
            Self::RerunNextPrivacy => "Rerun with next privacy",
            Self::RerunNextModel => "Rerun with next model",
            Self::ComparePreviousSnapshot => "Compare previous snapshot",
            Self::OpenLinkedEvidence => "Open linked evidence",
        }
    }

    const fn detail(self) -> &'static str {
        match self {
            Self::CancelRun => "Stop the queued or running AI job.",
            Self::ExpandEvidence => "Ask for stronger support tied to the saved artifact.",
            Self::ShowCounterevidence => "Surface conflicting evidence from the same saved run.",
            Self::ExplainRanking => "Explain why the saved run ranked findings the way it did.",
            Self::SuggestDrilldown => "Ask for the next bounded local investigation to run.",
            Self::GenerateReport => "Export a readable report from the selected artifact.",
            Self::RerunNextPrivacy => "Reuse the selection with the next privacy profile.",
            Self::RerunNextModel => "Reuse the selection with the next configured model.",
            Self::ComparePreviousSnapshot => "Prepare a compare using the nearest prior snapshot.",
            Self::OpenLinkedEvidence => "Jump to linked local evidence without leaving the TUI.",
        }
    }

    const fn action(self) -> Action {
        match self {
            Self::CancelRun => Action::RequestCancelAiRun,
            Self::ExpandEvidence => {
                Action::RequestAiGuidedFollowUp(GuidedFollowUpKind::ExpandEvidence)
            }
            Self::ShowCounterevidence => {
                Action::RequestAiGuidedFollowUp(GuidedFollowUpKind::ShowCounterevidence)
            }
            Self::ExplainRanking => {
                Action::RequestAiGuidedFollowUp(GuidedFollowUpKind::ExplainRanking)
            }
            Self::SuggestDrilldown => {
                Action::RequestAiGuidedFollowUp(GuidedFollowUpKind::SuggestLocalDrilldown)
            }
            Self::GenerateReport => Action::RequestAiGenerateReport,
            Self::RerunNextPrivacy => Action::RequestAiRerunNextPrivacy,
            Self::RerunNextModel => Action::RequestAiRerunNextModel,
            Self::ComparePreviousSnapshot => Action::RequestAiComparePreviousSnapshot,
            Self::OpenLinkedEvidence => Action::RequestJumpToAiEvidence,
        }
    }
}

impl AppState {
    pub fn handle(&mut self, action: Action) -> Vec<Action> {
        let mut emitted = Vec::new();
        match &action {
            Action::FocusNextRegion
            | Action::FocusPreviousRegion
            | Action::MoveFocusedRegion(_)
            | Action::ActivateFocusedRegion
            | Action::Back
            | Action::ToggleHelp
            | Action::OpenSearch
            | Action::CloseSearch
            | Action::SearchAppend(_)
            | Action::SearchBackspace
            | Action::SearchNextResult
            | Action::SearchPreviousResult => self.handle_focus_action(&action, &mut emitted),
            Action::Tick
            | Action::Quit
            | Action::NextScreen
            | Action::PreviousScreen
            | Action::ShowScreen(_)
            | Action::RefreshRequested
            | Action::RefreshStarted { .. }
            | Action::LiveSnapshotLoaded { .. }
            | Action::RefreshFailed { .. }
            | Action::StatusMessage { .. } => self.handle_lifecycle_action(action),
            Action::PreviousDay
            | Action::NextDay
            | Action::PreviousTimelinePoint
            | Action::NextTimelinePoint
            | Action::PreviousEvent
            | Action::NextEvent
            | Action::TimelineZoomIn
            | Action::TimelineZoomOut
            | Action::ToggleWorkoutFilter
            | Action::ToggleTagFilter
            | Action::ToggleSessionFilter => self.handle_day_timeline_action(&action),
            Action::PreviousTrendWindow
            | Action::NextTrendWindow
            | Action::CyclePatternMetric
            | Action::CycleReviewMode
            | Action::CycleReviewFocus
            | Action::PreviousReviewCard
            | Action::NextReviewCard => self.handle_review_action(&action),
            _ => self.handle_ai_action(action),
        }
        emitted
    }

    fn handle_focus_action(&mut self, action: &Action, emitted: &mut Vec<Action>) {
        match action {
            Action::FocusNextRegion => {
                if self.current_transient().is_none() {
                    let next = navigation::next_region(self.active_screen, self.focused_region);
                    self.set_focused_region(next);
                    self.status_line = format!(
                        "Focused {}.",
                        navigation::region_label(self.active_screen, next).unwrap_or("next region")
                    );
                }
            }
            Action::FocusPreviousRegion => {
                if self.current_transient().is_none() {
                    let previous =
                        navigation::previous_region(self.active_screen, self.focused_region);
                    self.set_focused_region(previous);
                    self.status_line = format!(
                        "Focused {}.",
                        navigation::region_label(self.active_screen, previous)
                            .unwrap_or("previous region")
                    );
                }
            }
            Action::MoveFocusedRegion(movement) => self.move_focused_region(*movement),
            Action::ActivateFocusedRegion => self.activate_focused_region(emitted),
            Action::Back => self.back_out(),
            Action::ToggleHelp => self.toggle_help(),
            Action::OpenSearch => self.open_search(),
            Action::CloseSearch => self.close_search(),
            Action::SearchAppend(character) => self.append_search_character(*character),
            Action::SearchBackspace => self.backspace_search(),
            Action::SearchNextResult => self.advance_search(true),
            Action::SearchPreviousResult => self.advance_search(false),
            _ => unreachable!("focus handler only receives focus/search actions"),
        }
    }

    fn dispatch_emitted_action(&mut self, action: Action, emitted: &mut Vec<Action>) {
        emitted.push(action.clone());
        emitted.extend(self.handle(action));
    }

    fn handle_lifecycle_action(&mut self, action: Action) {
        match action {
            Action::Tick => {
                self.tick_count = self.tick_count.saturating_add(1);
            }
            Action::Quit => {
                self.should_quit = true;
            }
            Action::NextScreen => {
                self.switch_screen(
                    self.active_screen.next(),
                    format!("Switched to {}", self.active_screen.next().title()),
                    false,
                );
            }
            Action::PreviousScreen => {
                self.switch_screen(
                    self.active_screen.previous(),
                    format!("Switched to {}", self.active_screen.previous().title()),
                    false,
                );
            }
            Action::ShowScreen(screen) => {
                self.switch_screen(screen, format!("Switched to {}", screen.title()), false);
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
            Action::RefreshFailed { message } | Action::StatusMessage { message } => {
                self.refresh_in_flight = false;
                self.status_line = message;
                self.rebuild_live_model();
            }
            _ => unreachable!("lifecycle handler only receives lifecycle actions"),
        }
    }

    fn handle_day_timeline_action(&mut self, action: &Action) {
        match action {
            Action::PreviousDay => {
                if self.selected_day_index > 0 {
                    self.selected_day_index -= 1;
                    self.reset_day_navigation();
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
                    self.reset_day_navigation();
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
                self.handle_relative_event(-1, "Moved to an earlier context event.");
            }
            Action::NextEvent => self.handle_relative_event(1, "Moved to a later context event."),
            Action::TimelineZoomIn => {
                self.set_timeline_window_hours(match self.timeline_window_hours {
                    24 => 12,
                    _ => 6,
                });
            }
            Action::TimelineZoomOut => {
                self.set_timeline_window_hours(match self.timeline_window_hours {
                    6 => 12,
                    _ => 24,
                });
            }
            Action::ToggleWorkoutFilter => self.select_overlay_toggle_and_toggle(0),
            Action::ToggleTagFilter => self.select_overlay_toggle_and_toggle(1),
            Action::ToggleSessionFilter => self.select_overlay_toggle_and_toggle(2),
            _ => unreachable!("day/timeline handler only receives day/timeline actions"),
        }
    }

    fn handle_review_action(&mut self, action: &Action) {
        match action {
            Action::PreviousTrendWindow => {
                self.set_trend_sort_mode(self.trend_sort_mode.previous());
            }
            Action::NextTrendWindow => {
                self.set_trend_sort_mode(self.trend_sort_mode.next());
            }
            Action::CyclePatternMetric => self.move_pattern_metric(NavMove::Next),
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
            _ => unreachable!("review handler only receives trend/review actions"),
        }
    }

    fn handle_ai_action(&mut self, action: Action) {
        match action {
            Action::RequestAiLaunch(intent) => {
                self.ai_preflight = None;
                self.enter_ai_screen(format!("Preparing {} preflight.", intent.label()), true);
            }
            Action::AiPreflightPrepared {
                preflight,
                status_line,
            } => {
                self.enter_ai_screen(status_line, false);
                self.ai_preflight = Some(*preflight);
                self.ai_preflight_control = PreflightControl::Confirm;
                self.rebuild_live_model();
            }
            Action::AiPreflightFailed { message } => {
                self.enter_ai_screen(message, false);
                self.ai_preflight = None;
                self.ai_preflight_control = PreflightControl::Confirm;
                self.rebuild_live_model();
            }
            Action::DismissAiPreflight => {
                if self.ai_preflight.take().is_some() {
                    self.ai_preflight_control = PreflightControl::Confirm;
                    "AI preflight dismissed.".clone_into(&mut self.status_line);
                    self.rebuild_live_model();
                }
            }
            Action::CycleAiPreflightPrivacyProfile => {
                if self.ai_preflight.is_some() {
                    "Cycling AI preflight privacy profile.".clone_into(&mut self.status_line);
                }
            }
            Action::ConfirmAiPreflight => {
                if self
                    .ai_preflight
                    .as_ref()
                    .is_some_and(|preflight| preflight.confirm_enabled)
                {
                    self.ai_preflight = None;
                    self.rebuild_live_model();
                    "Queueing AI run from preflight.".clone_into(&mut self.status_line);
                } else if self.ai_preflight.is_some() {
                    "AI preflight is blocked until provider readiness issues are resolved."
                        .clone_into(&mut self.status_line);
                }
            }
            Action::RequestCancelAiRun => {
                "Requesting AI run cancellation.".clone_into(&mut self.status_line);
            }
            Action::RequestAiGuidedFollowUp(kind) => {
                self.enter_ai_screen(format!("Preparing {} follow-up.", kind.label()), false);
            }
            Action::RequestAiRerunNextPrivacy => {
                self.enter_ai_screen(
                    "Preparing rerun with another privacy profile.".to_owned(),
                    false,
                );
            }
            Action::RequestAiRerunNextModel => {
                self.enter_ai_screen("Preparing rerun with another model.".to_owned(), false);
            }
            Action::RequestAiComparePreviousSnapshot => {
                self.enter_ai_screen(
                    "Preparing compare against the nearest previous similar snapshot.".to_owned(),
                    false,
                );
            }
            Action::RequestAiGenerateReport => {
                self.enter_ai_screen(
                    "Exporting a local report for the selected AI artifact.".to_owned(),
                    false,
                );
            }
            Action::RequestJumpToAiEvidence => {
                self.enter_ai_screen(
                    "Resolving saved evidence back into the local investigation views.".to_owned(),
                    false,
                );
            }
            Action::JumpToDayAndScreen {
                day,
                screen,
                status_line,
            } => self.handle_jump_to_day_and_screen(&day, screen, status_line),
            Action::JumpToAiBrowserRecord {
                tab,
                record_id,
                status_line,
            } => self.handle_jump_to_ai_browser_record(tab, &record_id, status_line),
            Action::PreviousAiBrowserTab => self.move_ai_browser_tab(AiBrowserTab::previous),
            Action::NextAiBrowserTab => self.move_ai_browser_tab(AiBrowserTab::next),
            Action::PreviousAiBrowserItem => self.move_ai_browser_item(-1),
            Action::NextAiBrowserItem => self.move_ai_browser_item(1),
            _ => unreachable!("AI handler only receives AI-specific actions"),
        }
    }

    fn switch_screen(&mut self, screen: Screen, status_line: String, rebuild: bool) {
        if screen != Screen::Ai && self.ai_preflight.is_some() {
            self.ai_preflight = None;
            self.ai_preflight_control = PreflightControl::Confirm;
        }
        self.active_screen = screen;
        self.focused_top_nav_screen = screen;
        self.expanded_region = None;
        self.restore_screen_focus();
        self.status_line = status_line;
        if rebuild {
            self.rebuild_live_model();
        }
    }

    fn enter_ai_screen(&mut self, status_line: String, rebuild: bool) {
        self.switch_screen(Screen::Ai, status_line, rebuild);
    }

    fn reset_day_navigation(&mut self) {
        self.selected_timeline_point = 0;
        self.selected_review_card_index = 0;
        self.select_default_event_for_selected_day();
        self.align_point_to_selected_event();
    }

    fn handle_relative_event(&mut self, delta: isize, status_line: &str) {
        if self.select_relative_event(delta) {
            self.align_point_to_selected_event();
            status_line.clone_into(&mut self.status_line);
            self.rebuild_live_model();
        }
    }

    fn select_overlay_toggle_and_toggle(&mut self, index: usize) {
        self.selected_overlay_toggle_index = index;
        self.toggle_overlay_filter(index);
    }

    fn handle_jump_to_day_and_screen(&mut self, day: &str, screen: Screen, status_line: String) {
        if self.select_day_by_label(day) {
            self.switch_screen(screen, status_line, true);
        } else {
            self.status_line =
                format!("Could not resolve saved evidence day `{day}` back into the local views.");
        }
    }

    fn handle_jump_to_ai_browser_record(
        &mut self,
        tab: AiBrowserTab,
        record_id: &str,
        status_line: String,
    ) {
        if self.select_ai_browser_record(tab, record_id) {
            self.enter_ai_screen(status_line, true);
        } else {
            self.status_line = format!(
                "Could not resolve saved {} `{record_id}` back into the local AI registry.",
                tab.label()
            );
        }
    }

    fn move_ai_browser_tab(&mut self, movement: impl FnOnce(AiBrowserTab) -> AiBrowserTab) {
        self.ai_browser_tab = movement(self.ai_browser_tab);
        self.selected_ai_artifact_action_index = 0;
        self.status_line = format!("AI browser switched to {}.", self.ai_browser_tab.label());
        self.rebuild_live_model();
    }

    fn move_ai_browser_item(&mut self, delta: isize) {
        if self.adjust_ai_browser_index(delta) {
            self.selected_ai_artifact_action_index = 0;
            self.status_line =
                format!("AI selection moved within {}.", self.ai_browser_tab.label());
            self.rebuild_live_model();
        }
    }

    #[must_use]
    pub fn footer(&self, viewport: ViewportClass) -> String {
        let hints = crate::keybindings::footer_hints(self.binding_context());
        let hint_text = if hints.is_empty() {
            "No contextual keys".to_owned()
        } else {
            hints.join(" | ")
        };
        let (label, exact, delta, freshness) = self.focused_footer_details(viewport);
        footer_inspector(&label, &exact, &delta, &freshness, &hint_text)
    }

    #[must_use]
    pub const fn active_tab_index(&self) -> usize {
        self.active_screen.index()
    }

    #[must_use]
    pub const fn focused_region(&self) -> FocusRegion {
        self.focused_region
    }

    #[must_use]
    pub const fn focused_top_nav_screen(&self) -> Screen {
        self.focused_top_nav_screen
    }

    #[must_use]
    pub const fn expanded_region(&self) -> Option<FocusRegion> {
        self.expanded_region
    }

    #[must_use]
    pub const fn help_open(&self) -> bool {
        self.help_open
    }

    #[must_use]
    pub const fn search_state(&self) -> Option<&SearchState> {
        self.search.as_ref()
    }

    #[must_use]
    pub const fn ai_preflight_control(&self) -> PreflightControl {
        self.ai_preflight_control
    }

    #[must_use]
    pub const fn binding_context(&self) -> BindingContext {
        BindingContext {
            active_screen: self.active_screen,
            focused_region: self.focused_region,
            search_open: self.search.is_some(),
            help_open: self.help_open,
            ai_preflight_open: self.ai_preflight.is_some(),
        }
    }

    fn focused_footer_details(&self, viewport: ViewportClass) -> (String, String, String, String) {
        let label = navigation::region_label(self.active_screen, self.focused_region)
            .unwrap_or_else(|| self.active_screen.title())
            .to_owned();
        let refreshing = if self.refresh_in_flight {
            "refreshing".to_owned()
        } else {
            "steady".to_owned()
        };

        match (self.active_screen, self.focused_region) {
            (_, FocusRegion::TopNav) => (
                label,
                self.active_screen.title().to_owned(),
                self.status_line.clone(),
                refreshing,
            ),
            (Screen::Dashboard, FocusRegion::DashboardReadiness) => (
                label,
                format!("score {}", self.model.dashboard.readiness.primary_value),
                self.model.dashboard.readiness.delta_label.clone(),
                self.dashboard_freshness(self.model.dashboard.readiness.availability),
            ),
            (Screen::Dashboard, FocusRegion::DashboardSleep) => (
                label,
                format!(
                    "{} | {}",
                    self.model.dashboard.sleep.duration_label,
                    self.model.dashboard.sleep.score_label
                ),
                self.model.dashboard.sleep.strip_note.clone(),
                self.dashboard_freshness(self.model.dashboard.sleep.availability),
            ),
            (Screen::Dashboard, FocusRegion::DashboardActivity) => (
                label,
                format!("activity {}", self.model.dashboard.activity.primary_value),
                self.model.dashboard.activity.delta_label.clone(),
                self.dashboard_freshness(self.model.dashboard.activity.availability),
            ),
            (Screen::Dashboard, FocusRegion::DashboardHrv) => (
                label,
                self.model.dashboard.hrv.primary_label.clone(),
                format!(
                    "{} | {}",
                    self.model.dashboard.hrv.baseline_label, self.model.dashboard.hrv.range_label
                ),
                self.dashboard_freshness(self.model.dashboard.hrv.availability),
            ),
            (Screen::Dashboard, FocusRegion::DashboardTemp) => (
                label,
                self.model.dashboard.body_temp.value_label.clone(),
                self.model.dashboard.body_temp.note.clone(),
                self.dashboard_freshness(self.model.dashboard.body_temp.availability),
            ),
            (Screen::Dashboard, FocusRegion::DashboardHeartRate) => (
                label,
                self.model.dashboard.heart_rate.primary_label.clone(),
                format!(
                    "{} | {}",
                    self.model.dashboard.heart_rate.baseline_label,
                    self.model.dashboard.heart_rate.range_label
                ),
                self.dashboard_freshness(self.model.dashboard.heart_rate.availability),
            ),
            (Screen::Dashboard, FocusRegion::DashboardSpo2) => (
                label,
                self.model.dashboard.spo2.primary_label.clone(),
                format!(
                    "{} | {}",
                    self.model.dashboard.spo2.baseline_label, self.model.dashboard.spo2.range_label
                ),
                self.dashboard_freshness(self.model.dashboard.spo2.availability),
            ),
            (Screen::Dashboard, FocusRegion::DashboardRespRate) => (
                label,
                self.model.dashboard.respiratory_rate.primary_label.clone(),
                self.model.dashboard.respiratory_rate.note.clone(),
                self.dashboard_freshness(self.model.dashboard.respiratory_rate.availability),
            ),
            (Screen::Dashboard, FocusRegion::DashboardBreakdown) => {
                self.focused_dashboard_breakdown_rail().map_or_else(
                    || {
                        (
                            label.clone(),
                            self.model.dashboard.breakdown.note.clone(),
                            "Δ --".to_owned(),
                            self.dashboard_freshness(self.model.dashboard.breakdown.availability),
                        )
                    },
                    |rail| {
                        (
                            label.clone(),
                            rail.label.clone(),
                            rail.delta_label.clone(),
                            self.dashboard_freshness(rail.availability),
                        )
                    },
                )
            }
            (Screen::Dashboard, FocusRegion::DashboardHeatmap) => {
                let exact = self
                    .model
                    .dashboard
                    .weekly
                    .selected_summary_for_viewport(viewport);
                (
                    label,
                    exact,
                    self.model.dashboard.weekly.note.clone(),
                    self.dashboard_freshness(self.model.dashboard.weekly.availability),
                )
            }
            (Screen::Timeline, FocusRegion::TimelineControls) => (
                label,
                format!(
                    "{} | {}h window",
                    self.model.timeline.selected_day_label, self.model.timeline.window_hours
                ),
                self.model.timeline.day_selector.clone(),
                self.coverage_footer(CoverageFamily::Heartrate),
            ),
            (Screen::Timeline, FocusRegion::TimelineChart) => (
                label,
                self.model.timeline.selected_detail.clone(),
                self.model.timeline.breadcrumb.clone(),
                self.coverage_footer(CoverageFamily::Heartrate),
            ),
            (Screen::Timeline, FocusRegion::TimelineLanes) => {
                let overlay = self
                    .model
                    .timeline
                    .overlay_toggles
                    .get(self.selected_overlay_toggle_index)
                    .map_or_else(
                        || "overlays".to_owned(),
                        |toggle| {
                            format!(
                                "{} {}",
                                toggle.label,
                                if toggle.enabled { "enabled" } else { "hidden" }
                            )
                        },
                    );
                (
                    label,
                    overlay,
                    format!(
                        "{} visible lane groups",
                        self.model.timeline.overlay_groups.len()
                    ),
                    self.coverage_footer(CoverageFamily::Tag),
                )
            }
            (Screen::Timeline, FocusRegion::TimelineInspector) => (
                label,
                self.model.timeline.selected_detail.clone(),
                self.model
                    .timeline
                    .event_detail_lines
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "No linked event".to_owned()),
                self.coverage_footer(CoverageFamily::Heartrate),
            ),
            (Screen::Timeline, FocusRegion::TimelineEvents) => (
                label,
                self.model
                    .timeline
                    .events
                    .iter()
                    .find(|event| event.selected)
                    .map_or_else(
                        || "No matching event".to_owned(),
                        |event| event.headline.clone(),
                    ),
                self.model.timeline.breadcrumb.clone(),
                self.coverage_footer(CoverageFamily::Session),
            ),
            (Screen::Trends, FocusRegion::TrendsMatrix | FocusRegion::TrendsInspector) => {
                self.focused_trend_row().map_or_else(
                    || {
                        (
                            label.clone(),
                            self.status_line.clone(),
                            "Δ --".to_owned(),
                            refreshing.clone(),
                        )
                    },
                    |row| {
                        let delta = row
                            .cells
                            .iter()
                            .map(|cell| format!("{} {}", cell.label, cell.delta_label))
                            .collect::<Vec<_>>()
                            .join(" | ");
                        let freshness = row
                            .cells
                            .iter()
                            .find(|cell| cell.availability != TelemetryAvailability::Fresh)
                            .map_or_else(
                                || "FRESH".to_owned(),
                                |cell| cell.availability.label().to_owned(),
                            );
                        (
                            label.clone(),
                            format!("{} {}", row.label, row.current_value),
                            delta,
                            freshness,
                        )
                    },
                )
            }
            (Screen::Ops, FocusRegion::OpsSummary) => (
                label,
                self.model
                    .ops
                    .summary_lines
                    .first()
                    .cloned()
                    .unwrap_or_else(|| self.status_line.clone()),
                self.model.ops.mode_label.clone(),
                refreshing,
            ),
            (Screen::Ops, FocusRegion::OpsCoverage) => {
                let coverage = self.model.ops.coverage.first().map_or_else(
                    || "coverage unavailable".to_owned(),
                    |cell| format!("{} {}", cell.label, cell.availability.label()),
                );
                (
                    label,
                    coverage,
                    format!("{} families tracked", self.model.ops.coverage.len()),
                    refreshing,
                )
            }
            (Screen::Ops, FocusRegion::OpsDiagnostics) => (
                label,
                self.model.ops.items.first().map_or_else(
                    || "No diagnostics".to_owned(),
                    |item| format!("{} {}", item.label, item.value),
                ),
                format!("{} diagnostic items", self.model.ops.items.len()),
                refreshing,
            ),
            (Screen::Ops, FocusRegion::OpsWarnings) => (
                label,
                self.model
                    .ops
                    .warnings
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "No warnings".to_owned()),
                format!("{} warning entries", self.model.ops.warnings.len()),
                refreshing,
            ),
            _ => (
                label,
                self.status_line.clone(),
                "Δ --".to_owned(),
                refreshing,
            ),
        }
    }

    fn focused_dashboard_breakdown_rail(&self) -> Option<&DashboardBreakdownRail> {
        self.model
            .dashboard
            .breakdown
            .rails
            .iter()
            .find(|rail| rail.selected)
    }

    fn focused_trend_row(&self) -> Option<&TrendMatrixRow> {
        self.model.trends.rows.iter().find(|row| row.selected)
    }

    fn dashboard_freshness(&self, availability: TelemetryAvailability) -> String {
        format!(
            "{} | {}",
            self.model.dashboard.header.freshness_badge,
            availability.label()
        )
    }

    fn coverage_footer(&self, family: CoverageFamily) -> String {
        self.live_snapshot.as_ref().map_or_else(
            || "NO DATA".to_owned(),
            |snapshot| coverage_availability(snapshot, family).label().to_owned(),
        )
    }

    #[must_use]
    pub fn is_region_focused(&self, region: FocusRegion) -> bool {
        self.focused_region == region && self.current_transient().is_none()
    }

    #[must_use]
    pub const fn current_transient(&self) -> Option<TransientLayer> {
        if self.search.is_some() {
            Some(TransientLayer::Search)
        } else if self.help_open {
            Some(TransientLayer::Help)
        } else if self.ai_preflight.is_some() {
            Some(TransientLayer::AiPreflight)
        } else {
            None
        }
    }

    fn replace_live_snapshot(&mut self, snapshot: LiveSnapshot) {
        let previous_day = self.selected_day_label();
        let previous_event = self.selected_event_id.clone();
        self.live_snapshot = Some(snapshot);

        if let Some(snapshot) = &self.live_snapshot {
            let day_labels = available_days(snapshot);
            self.selected_day_index = previous_day.as_deref().map_or_else(
                || newest_day_index(snapshot),
                |selected_day| restored_day_index(&day_labels, selected_day),
            );
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
                    ai_preflight: self.ai_preflight.clone(),
                    ai_preflight_control: self.ai_preflight_control,
                    ai_browser_tab: self.ai_browser_tab,
                    selected_ai_launch_index: self.selected_ai_launch_index,
                    selected_ai_run_index: self.selected_ai_run_index,
                    selected_snapshot_catalog_index: self.selected_snapshot_catalog_index,
                    selected_report_export_index: self.selected_report_export_index,
                    selected_ai_eval_run_index: self.selected_ai_eval_run_index,
                    selected_ai_artifact_action_index: self.selected_ai_artifact_action_index,
                    overlay_filters: self.overlay_filters.clone(),
                    selected_overlay_toggle_index: self.selected_overlay_toggle_index,
                    window_hours: self.timeline_window_hours,
                    trends_window: self.trends_window,
                    trend_sort_mode: self.trend_sort_mode,
                    selected_trend_row_index: self.selected_trend_row_index,
                    pattern_metric_filter: self.pattern_metric_filter,
                    refresh_in_flight: self.refresh_in_flight,
                    review_mode: self.review_mode,
                    review_focus: self.review_focus,
                    selected_review_card_index: self.selected_review_card_index,
                    selected_dashboard_breakdown_index: self.selected_dashboard_breakdown_index,
                },
            );
        }
    }

    fn available_day_count(&self) -> usize {
        self.live_snapshot
            .as_ref()
            .map_or(0, |snapshot| available_days(snapshot).len())
    }

    pub(crate) fn selected_day_label(&self) -> Option<String> {
        self.live_snapshot.as_ref().and_then(|snapshot| {
            available_days(snapshot)
                .get(self.selected_day_index)
                .cloned()
        })
    }

    pub(crate) const fn ai_preflight_state(&self) -> Option<&AiPreflightState> {
        self.ai_preflight.as_ref()
    }

    pub(crate) fn selected_ai_run_record(&self) -> Option<AiRunRecord> {
        self.live_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.ai_runs.get(self.selected_ai_run_index))
            .cloned()
    }

    pub(crate) fn selected_snapshot_catalog_entry(&self) -> Option<SnapshotCatalogEntry> {
        self.live_snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .snapshot_catalog
                    .get(self.selected_snapshot_catalog_index)
            })
            .cloned()
    }

    pub(crate) fn selected_report_export_record(&self) -> Option<ReportExportRecord> {
        self.live_snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .report_exports
                    .get(self.selected_report_export_index)
            })
            .cloned()
    }

    pub(crate) fn selected_ai_eval_run_record(&self) -> Option<AiEvalRunRecord> {
        self.live_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.ai_eval_runs.get(self.selected_ai_eval_run_index))
            .cloned()
    }

    pub(crate) const fn selected_ai_browser_tab(&self) -> AiBrowserTab {
        self.ai_browser_tab
    }

    fn select_ai_browser_record(&mut self, tab: AiBrowserTab, record_id: &str) -> bool {
        let Some(snapshot) = &self.live_snapshot else {
            return false;
        };
        let found = match tab {
            AiBrowserTab::Runs => snapshot
                .ai_runs
                .iter()
                .position(|record| record.run_id == record_id)
                .map(|index| self.selected_ai_run_index = index),
            AiBrowserTab::Snapshots => snapshot
                .snapshot_catalog
                .iter()
                .position(|record| record.snapshot_hash == record_id)
                .map(|index| self.selected_snapshot_catalog_index = index),
            AiBrowserTab::Reports => snapshot
                .report_exports
                .iter()
                .position(|record| record.report_id == record_id)
                .map(|index| self.selected_report_export_index = index),
            AiBrowserTab::Evals => snapshot
                .ai_eval_runs
                .iter()
                .position(|record| record.eval_run_id == record_id)
                .map(|index| self.selected_ai_eval_run_index = index),
        };
        if found.is_some() {
            self.ai_browser_tab = tab;
            self.selected_ai_artifact_action_index = 0;
            true
        } else {
            false
        }
    }

    fn select_day_by_label(&mut self, day: &str) -> bool {
        let Some(snapshot) = &self.live_snapshot else {
            return false;
        };
        let day_labels = available_days(snapshot);
        let Some(index) = day_labels.iter().position(|candidate| candidate == day) else {
            return false;
        };
        self.selected_day_index = index;
        self.selected_timeline_point = 0;
        self.selected_review_card_index = 0;
        self.select_default_event_for_selected_day();
        self.align_point_to_selected_event();
        true
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

    const fn current_review_card_count(&self) -> usize {
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
                current_index.saturating_add(delta.cast_unsigned()),
                events.len().saturating_sub(1),
            )
        };

        self.selected_event_id = Some(events[new_index].context_event_id.clone());
        true
    }

    fn adjust_ai_browser_index(&mut self, delta: isize) -> bool {
        let len = match self.ai_browser_tab {
            AiBrowserTab::Runs => self
                .live_snapshot
                .as_ref()
                .map_or(0, |snapshot| snapshot.ai_runs.len()),
            AiBrowserTab::Snapshots => self
                .live_snapshot
                .as_ref()
                .map_or(0, |snapshot| snapshot.snapshot_catalog.len()),
            AiBrowserTab::Reports => self
                .live_snapshot
                .as_ref()
                .map_or(0, |snapshot| snapshot.report_exports.len()),
            AiBrowserTab::Evals => self
                .live_snapshot
                .as_ref()
                .map_or(0, |snapshot| snapshot.ai_eval_runs.len()),
        };
        if len == 0 {
            return false;
        }

        let selected = match self.ai_browser_tab {
            AiBrowserTab::Runs => &mut self.selected_ai_run_index,
            AiBrowserTab::Snapshots => &mut self.selected_snapshot_catalog_index,
            AiBrowserTab::Reports => &mut self.selected_report_export_index,
            AiBrowserTab::Evals => &mut self.selected_ai_eval_run_index,
        };
        let new_index = if delta.is_negative() {
            selected.saturating_sub(delta.unsigned_abs())
        } else {
            usize::min(
                selected.saturating_add(delta.cast_unsigned()),
                len.saturating_sub(1),
            )
        };
        let changed = *selected != new_index;
        *selected = new_index;
        changed
    }

    fn set_focused_region(&mut self, region: FocusRegion) {
        self.focused_region = region;
        if region != FocusRegion::TopNav {
            self.screen_focus_memory[self.active_screen.index()] = region;
        }
        if region == FocusRegion::TopNav {
            self.focused_top_nav_screen = self.active_screen;
        }
    }

    const fn restore_screen_focus(&mut self) {
        let region = self.screen_focus_memory[self.active_screen.index()];
        self.focused_region = region;
    }

    fn move_focused_region(&mut self, movement: NavMove) {
        if self.search.is_some() {
            match movement {
                NavMove::Previous | NavMove::PageBackward => self.advance_search(false),
                NavMove::Next | NavMove::PageForward => self.advance_search(true),
                NavMove::First => self.advance_search_to_edge(true),
                NavMove::Last => self.advance_search_to_edge(false),
            }
            return;
        }
        if self.help_open {
            return;
        }
        if self.ai_preflight.is_some() {
            self.ai_preflight_control = match movement {
                NavMove::Previous => self.ai_preflight_control.previous(),
                NavMove::Next => self.ai_preflight_control.next(),
                NavMove::First | NavMove::PageBackward => PreflightControl::Confirm,
                NavMove::Last | NavMove::PageForward => PreflightControl::Cancel,
            };
            self.status_line = format!(
                "Focused {}.",
                self.ai_preflight_control.label().to_ascii_lowercase()
            );
            return;
        }

        match (self.active_screen, self.focused_region) {
            (_, FocusRegion::TopNav) => self.move_top_nav_focus(movement),
            (Screen::Dashboard, FocusRegion::DashboardBreakdown) => {
                self.move_dashboard_breakdown_selection(movement);
            }
            (Screen::Dashboard, FocusRegion::DashboardHeatmap) => {
                self.move_dashboard_heatmap_selection(movement);
            }
            (Screen::Timeline, FocusRegion::TimelineControls) => {
                self.move_timeline_window_preset(movement);
            }
            (Screen::Timeline, FocusRegion::TimelineLanes)
            | (Screen::Patterns, FocusRegion::ContextSecondary)
            | (Screen::Explain, FocusRegion::ContextPrimary) => {
                self.move_overlay_toggle_selection(movement);
            }
            (Screen::Timeline, FocusRegion::TimelineChart) => {
                self.move_timeline_chart(movement);
            }
            (Screen::Timeline, FocusRegion::TimelineEvents) => {
                self.move_timeline_events(movement);
            }
            (Screen::Timeline, FocusRegion::TimelineInspector) => match movement {
                NavMove::PageBackward => {
                    self.handle(Action::PreviousDay);
                }
                NavMove::PageForward => {
                    self.handle(Action::NextDay);
                }
                _ => self.move_timeline_events(movement),
            },
            (Screen::Trends, FocusRegion::TrendsMatrix | FocusRegion::TrendsInspector) => {
                self.move_trend_row(movement);
            }
            (Screen::Patterns, FocusRegion::ContextPrimary) => {
                self.move_pattern_metric(movement);
            }
            (Screen::Review, FocusRegion::ContextPrimary) => {
                self.move_review_mode(movement);
            }
            (Screen::Review, FocusRegion::ContextSecondary) => {
                self.move_review_focus(movement);
            }
            (Screen::Review, FocusRegion::Primary) => {
                self.move_review_cards(movement);
            }
            (Screen::Ai, FocusRegion::ContextPrimary) => {
                self.move_ai_browser_tabs(movement);
            }
            (Screen::Ai, FocusRegion::Primary) => {
                self.move_ai_launch_points(movement);
            }
            (Screen::Ai, FocusRegion::Secondary) => {
                self.move_ai_browser_items(movement);
            }
            (Screen::Ai, FocusRegion::Tertiary) => {
                self.move_ai_artifact_actions(movement);
            }
            (screen, FocusRegion::Primary) => {
                if matches!(movement, NavMove::PageBackward)
                    && Self::screen_supports_day_paging(screen)
                {
                    self.handle(Action::PreviousDay);
                } else if matches!(movement, NavMove::PageForward)
                    && Self::screen_supports_day_paging(screen)
                {
                    self.handle(Action::NextDay);
                }
            }
            (screen, _) if Self::screen_supports_day_paging(screen) => match movement {
                NavMove::PageBackward => {
                    self.handle(Action::PreviousDay);
                }
                NavMove::PageForward => {
                    self.handle(Action::NextDay);
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn activate_focused_region(&mut self, emitted: &mut Vec<Action>) {
        if self.search.is_some() {
            self.advance_search(true);
            return;
        }
        if self.help_open {
            self.toggle_help();
            return;
        }
        if self.ai_preflight.is_some() {
            match self.ai_preflight_control {
                PreflightControl::Confirm => {
                    self.dispatch_emitted_action(Action::ConfirmAiPreflight, emitted);
                }
                PreflightControl::Privacy => {
                    self.dispatch_emitted_action(Action::CycleAiPreflightPrivacyProfile, emitted);
                }
                PreflightControl::Cancel => {
                    self.dispatch_emitted_action(Action::DismissAiPreflight, emitted);
                }
            }
            return;
        }

        match (self.active_screen, self.focused_region) {
            (_, FocusRegion::TopNav) => {
                self.active_screen = self.focused_top_nav_screen;
                self.restore_screen_focus();
                self.expanded_region = None;
                self.status_line = format!("Switched to {}.", self.active_screen.title());
            }
            (Screen::Dashboard, FocusRegion::DashboardReadiness) => {
                self.switch_screen(
                    Screen::Explain,
                    "Opened readiness explanation.".to_owned(),
                    false,
                );
                self.set_focused_region(FocusRegion::Primary);
            }
            (Screen::Dashboard, FocusRegion::DashboardSleep) => {
                self.switch_screen(Screen::Trends, "Opened sleep trends.".to_owned(), false);
                self.set_focused_region(FocusRegion::TrendsMatrix);
                self.focus_trend_row_by_label("Sleep");
            }
            (Screen::Dashboard, FocusRegion::DashboardActivity) => {
                self.switch_screen(
                    Screen::Timeline,
                    "Opened activity timeline.".to_owned(),
                    false,
                );
                self.set_focused_region(FocusRegion::TimelineChart);
            }
            (Screen::Dashboard, FocusRegion::DashboardHeartRate) => {
                self.switch_screen(
                    Screen::Trends,
                    "Opened heart-rate trends.".to_owned(),
                    false,
                );
                self.set_focused_region(FocusRegion::TrendsMatrix);
                self.focus_trend_row_by_label("Heart Rate");
            }
            (Screen::Dashboard, FocusRegion::DashboardTemp) => {
                self.switch_screen(
                    Screen::Trends,
                    "Opened temperature trends.".to_owned(),
                    false,
                );
                self.set_focused_region(FocusRegion::TrendsMatrix);
                self.focus_trend_row_by_label("Temp Dev");
            }
            (Screen::Dashboard, FocusRegion::DashboardHrv) => {
                self.toggle_region_expansion(FocusRegion::DashboardHrv, "HRV panel");
            }
            (Screen::Dashboard, FocusRegion::DashboardRespRate) => {
                self.toggle_region_expansion(FocusRegion::DashboardRespRate, "Respiratory panel");
            }
            (Screen::Dashboard, FocusRegion::DashboardBreakdown) => {
                self.toggle_region_expansion(FocusRegion::DashboardBreakdown, "Driver breakdown");
            }
            (Screen::Dashboard, FocusRegion::DashboardHeatmap) => {
                self.switch_screen(
                    Screen::Timeline,
                    "Opened selected-day timeline from the weekly heatmap.".to_owned(),
                    false,
                );
                self.set_focused_region(FocusRegion::TimelineChart);
            }
            (Screen::Timeline, FocusRegion::TimelineLanes)
            | (Screen::Patterns, FocusRegion::ContextSecondary)
            | (Screen::Explain, FocusRegion::ContextPrimary) => {
                self.toggle_selected_overlay_filter();
            }
            (Screen::Timeline, FocusRegion::TimelineChart) => {
                self.toggle_region_expansion(FocusRegion::TimelineChart, "Timeline chart");
            }
            (Screen::Timeline, FocusRegion::TimelineEvents) => {
                self.set_focused_region(FocusRegion::TimelineInspector);
                "Inspecting selected event details.".clone_into(&mut self.status_line);
            }
            (Screen::Timeline, FocusRegion::TimelineInspector) => {
                self.toggle_region_expansion(FocusRegion::TimelineInspector, "Timeline detail");
            }
            (Screen::Review, FocusRegion::Primary) => {
                self.set_focused_region(FocusRegion::Secondary);
                "Inspecting selected review brief.".clone_into(&mut self.status_line);
            }
            (Screen::Review, FocusRegion::Secondary) => {
                self.set_focused_region(FocusRegion::Primary);
                "Returned to ranked observations.".clone_into(&mut self.status_line);
            }
            (Screen::Ai, FocusRegion::Primary) => match self.selected_ai_launch_index() {
                0 => self.dispatch_emitted_action(
                    Action::RequestAiLaunch(AiLaunchIntent::ReviewSelectedDay),
                    emitted,
                ),
                1 => self.dispatch_emitted_action(
                    Action::RequestAiLaunch(AiLaunchIntent::CompareSelectedWeek),
                    emitted,
                ),
                _ => {}
            },
            (Screen::Ai, FocusRegion::Secondary) => {
                self.set_focused_region(FocusRegion::Tertiary);
                "Focused artifact actions for the selected saved item."
                    .clone_into(&mut self.status_line);
            }
            (Screen::Ai, FocusRegion::Tertiary) => {
                if let Some(action) = self.current_ai_artifact_action() {
                    self.dispatch_emitted_action(action, emitted);
                } else {
                    "No direct actions are available for the selected saved artifact."
                        .clone_into(&mut self.status_line);
                }
            }
            (Screen::Trends, FocusRegion::TrendsMatrix) => {
                self.toggle_region_expansion(FocusRegion::TrendsMatrix, "Trend matrix");
            }
            (Screen::Trends, FocusRegion::TrendsInspector) => {
                self.toggle_region_expansion(FocusRegion::TrendsInspector, "Trend inspector");
            }
            (Screen::Ops, FocusRegion::OpsSummary) => {
                self.toggle_region_expansion(FocusRegion::OpsSummary, "Status summary");
            }
            (Screen::Ops, FocusRegion::OpsCoverage) => {
                self.toggle_region_expansion(FocusRegion::OpsCoverage, "Coverage matrix");
            }
            (Screen::Ops, FocusRegion::OpsDiagnostics) => {
                self.toggle_region_expansion(FocusRegion::OpsDiagnostics, "Diagnostics");
            }
            (Screen::Ops, FocusRegion::OpsWarnings) => {
                self.toggle_region_expansion(FocusRegion::OpsWarnings, "Warnings");
            }
            _ => {}
        }
    }

    fn back_out(&mut self) {
        if self.search.is_some() {
            self.close_search();
            return;
        }
        if self.help_open {
            self.toggle_help();
            return;
        }
        if self.ai_preflight.take().is_some() {
            self.ai_preflight_control = PreflightControl::Confirm;
            "AI preflight dismissed.".clone_into(&mut self.status_line);
            self.rebuild_live_model();
            return;
        }

        if self.expanded_region.take().is_some() {
            self.status_line = format!(
                "Collapsed {}.",
                navigation::region_label(self.active_screen, self.focused_region)
                    .unwrap_or("panel")
                    .to_ascii_lowercase()
            );
            return;
        }

        if matches!(
            (self.active_screen, self.focused_region),
            (Screen::Timeline, FocusRegion::TimelineInspector)
        ) {
            self.set_focused_region(FocusRegion::TimelineEvents);
            "Returned to day events.".clone_into(&mut self.status_line);
            return;
        }

        if self.focused_region != FocusRegion::TopNav {
            let region = navigation::previous_region(self.active_screen, self.focused_region);
            self.set_focused_region(region);
            self.status_line = format!(
                "Focused {}.",
                navigation::region_label(self.active_screen, region).unwrap_or("navigation")
            );
        }
    }

    fn toggle_help(&mut self) {
        if self.help_open {
            self.help_open = false;
            if let Some(region) = self.focus_before_help.take() {
                self.set_focused_region(region);
            }
            "Closed keyboard help.".clone_into(&mut self.status_line);
        } else {
            if let Some(search) = self.search.take() {
                self.set_focused_region(search.previous_region);
            }
            self.focus_before_help = Some(self.focused_region);
            self.help_open = true;
            "Opened keyboard help.".clone_into(&mut self.status_line);
        }
    }

    fn open_search(&mut self) {
        let Some(scope) = navigation::search_scope(self.active_screen, self.focused_region)
            .or_else(|| navigation::default_search_scope(self.active_screen))
        else {
            self.status_line = format!(
                "Search is not available in {}.",
                navigation::region_label(self.active_screen, self.focused_region)
                    .unwrap_or_else(|| self.active_screen.title())
            );
            return;
        };
        self.help_open = false;
        self.focus_before_help = None;

        let previous_region = self.focused_region;
        self.search = Some(SearchState {
            scope,
            query: String::new(),
            active_match_index: 0,
            total_matches: 0,
            previous_region,
        });
        "Find opened. Type to search the current list.".clone_into(&mut self.status_line);
    }

    fn close_search(&mut self) {
        if let Some(search) = self.search.take() {
            self.set_focused_region(search.previous_region);
            "Closed search.".clone_into(&mut self.status_line);
        }
    }

    fn append_search_character(&mut self, character: char) {
        if let Some(search) = self.search.as_mut() {
            search.query.push(character);
            self.advance_search_to_edge(true);
        }
    }

    fn backspace_search(&mut self) {
        if let Some(search) = self.search.as_mut() {
            search.query.pop();
            self.advance_search_to_edge(true);
        }
    }

    fn advance_search_to_edge(&mut self, first: bool) {
        let Some((scope, query)) = self
            .search
            .as_ref()
            .map(|search| (search.scope, search.query.clone()))
        else {
            return;
        };
        let matches = self.search_matches(scope, &query);
        if matches.is_empty() {
            if let Some(search) = self.search.as_mut() {
                search.total_matches = 0;
                search.active_match_index = 0;
            }
            self.status_line = format!("No matches for `{query}`.");
            return;
        }
        let match_index = if first {
            0
        } else {
            matches.len().saturating_sub(1)
        };
        self.apply_search_match(scope, &matches, match_index);
    }

    fn advance_search(&mut self, forward: bool) {
        let Some((scope, query, current)) = self.search.as_ref().map(|search| {
            (
                search.scope,
                search.query.clone(),
                search.active_match_index,
            )
        }) else {
            return;
        };
        let matches = self.search_matches(scope, &query);
        if matches.is_empty() {
            self.status_line = format!("No matches for `{query}`.");
            return;
        }
        let next = if forward {
            (current + 1) % matches.len()
        } else if current == 0 {
            matches.len().saturating_sub(1)
        } else {
            current - 1
        };
        self.apply_search_match(scope, &matches, next);
    }

    fn apply_search_match(&mut self, scope: SearchScope, matches: &[usize], match_index: usize) {
        let Some(item_index) = matches.get(match_index).copied() else {
            return;
        };
        match scope {
            SearchScope::TimelineEvents => {
                self.select_timeline_event_at(item_index);
            }
            SearchScope::ReviewCards => {
                self.selected_review_card_index = item_index;
                self.rebuild_live_model();
            }
            SearchScope::AiBrowserItems => {
                self.set_ai_browser_index(item_index);
                self.rebuild_live_model();
            }
        }
        if let Some(search) = self.search.as_mut() {
            search.total_matches = matches.len();
            search.active_match_index = match_index;
        }
        let query = self
            .search
            .as_ref()
            .map(|search| search.query.clone())
            .unwrap_or_default();
        self.status_line = format!(
            "Match {} of {} for `{}`.",
            match_index + 1,
            matches.len(),
            query
        );
    }

    fn search_matches(&self, scope: SearchScope, query: &str) -> Vec<usize> {
        if query.is_empty() {
            return Vec::new();
        }
        let needle = query.to_ascii_lowercase();
        match scope {
            SearchScope::TimelineEvents => self
                .model
                .timeline
                .events
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    format!("{} {}", item.headline, item.detail)
                        .to_ascii_lowercase()
                        .contains(&needle)
                })
                .map(|(index, _)| index)
                .collect(),
            SearchScope::ReviewCards => self
                .model
                .review
                .cards
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    format!(
                        "{} {} {}",
                        item.headline, item.confidence_label, item.section_label
                    )
                    .to_ascii_lowercase()
                    .contains(&needle)
                })
                .map(|(index, _)| index)
                .collect(),
            SearchScope::AiBrowserItems => self
                .model
                .ai
                .browser_items
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    format!("{} {}", item.headline, item.detail)
                        .to_ascii_lowercase()
                        .contains(&needle)
                })
                .map(|(index, _)| index)
                .collect(),
        }
    }

    fn move_top_nav_focus(&mut self, movement: NavMove) {
        self.focused_top_nav_screen = match movement {
            NavMove::Previous => self.focused_top_nav_screen.previous(),
            NavMove::Next => self.focused_top_nav_screen.next(),
            NavMove::First | NavMove::PageBackward => Screen::Dashboard,
            NavMove::Last | NavMove::PageForward => Screen::Ops,
        };
        self.status_line = format!(
            "Focused {} in primary navigation.",
            self.focused_top_nav_screen.title()
        );
    }

    fn move_timeline_window_preset(&mut self, movement: NavMove) {
        let current = timeline_window_preset_index(self.timeline_window_hours);
        let last = TIMELINE_WINDOW_PRESETS.len().saturating_sub(1);
        let next = match movement {
            NavMove::Previous => current.saturating_sub(1),
            NavMove::Next => usize::min(current + 1, last),
            NavMove::First | NavMove::PageBackward => 0,
            NavMove::Last | NavMove::PageForward => last,
        };
        self.set_timeline_window_hours(TIMELINE_WINDOW_PRESETS[next]);
    }

    fn move_overlay_toggle_selection(&mut self, movement: NavMove) {
        let current = self
            .selected_overlay_toggle_index
            .min(overlay_toggle_count() - 1);
        let last = overlay_toggle_count().saturating_sub(1);
        self.selected_overlay_toggle_index = match movement {
            NavMove::Previous => current.saturating_sub(1),
            NavMove::Next => usize::min(current + 1, last),
            NavMove::First | NavMove::PageBackward => 0,
            NavMove::Last | NavMove::PageForward => last,
        };
        let selected = overlay_toggle_descriptor(self.selected_overlay_toggle_index);
        self.status_line = format!("Focused {} overlays.", selected.label.to_ascii_lowercase());
        self.rebuild_live_model();
    }

    fn set_timeline_window_hours(&mut self, window_hours: u16) {
        self.timeline_window_hours = window_hours;
        self.align_point_to_selected_event();
        self.status_line = format!("Timeline window set to {}h.", self.timeline_window_hours);
        self.rebuild_live_model();
    }

    fn toggle_selected_overlay_filter(&mut self) {
        self.toggle_overlay_filter(self.selected_overlay_toggle_index);
    }

    fn toggle_overlay_filter(&mut self, index: usize) {
        let descriptor = overlay_toggle_descriptor(index);
        let enabled = match descriptor.family {
            ContextEventFamily::Workout => {
                self.overlay_filters.workouts = !self.overlay_filters.workouts;
                self.overlay_filters.workouts
            }
            ContextEventFamily::Tag | ContextEventFamily::EnhancedTag => {
                self.overlay_filters.tags = !self.overlay_filters.tags;
                self.overlay_filters.tags
            }
            ContextEventFamily::Session => {
                self.overlay_filters.sessions = !self.overlay_filters.sessions;
                self.overlay_filters.sessions
            }
        };
        self.selected_overlay_toggle_index = index.min(overlay_toggle_count() - 1);
        self.normalize_event_selection();
        self.rebuild_live_model();
        self.status_line = format!(
            "{} overlays {}.",
            descriptor.label,
            if enabled { "enabled" } else { "hidden" }
        );
    }

    fn move_timeline_chart(&mut self, movement: NavMove) {
        match movement {
            NavMove::Previous => {
                self.handle(Action::PreviousTimelinePoint);
            }
            NavMove::Next => {
                self.handle(Action::NextTimelinePoint);
            }
            NavMove::First => {
                self.selected_timeline_point = 0;
                self.select_nearest_event_for_current_point();
                self.rebuild_live_model();
                "Moved to the first heartrate point.".clone_into(&mut self.status_line);
            }
            NavMove::Last => {
                self.selected_timeline_point =
                    self.visible_timeline_point_count().saturating_sub(1);
                self.select_nearest_event_for_current_point();
                self.rebuild_live_model();
                "Moved to the last heartrate point.".clone_into(&mut self.status_line);
            }
            NavMove::PageBackward => {
                self.handle(Action::PreviousDay);
            }
            NavMove::PageForward => {
                self.handle(Action::NextDay);
            }
        }
    }

    fn move_timeline_events(&mut self, movement: NavMove) {
        match movement {
            NavMove::Previous => {
                self.handle(Action::PreviousEvent);
            }
            NavMove::Next => {
                self.handle(Action::NextEvent);
            }
            NavMove::First => {
                self.select_timeline_event_at(0);
            }
            NavMove::Last => {
                let last = self.model.timeline.events.len().saturating_sub(1);
                self.select_timeline_event_at(last);
            }
            NavMove::PageBackward => {
                self.select_relative_event(-5);
                self.align_point_to_selected_event();
                self.rebuild_live_model();
            }
            NavMove::PageForward => {
                self.select_relative_event(5);
                self.align_point_to_selected_event();
                self.rebuild_live_model();
            }
        }
    }

    fn move_pattern_metric(&mut self, movement: NavMove) {
        self.pattern_metric_filter = match movement {
            NavMove::Previous => self.pattern_metric_filter.previous(),
            NavMove::Next => self.pattern_metric_filter.next(),
            NavMove::First | NavMove::PageBackward => PatternMetricFilter::All,
            NavMove::Last | NavMove::PageForward => PatternMetricFilter::Sleep,
        };
        self.status_line = format!(
            "Pattern metric filter changed to {}.",
            self.pattern_metric_filter.label()
        );
        self.rebuild_live_model();
    }

    fn move_review_mode(&mut self, movement: NavMove) {
        self.review_mode = match movement {
            NavMove::Previous => match self.review_mode {
                ReviewScreenMode::Today => ReviewScreenMode::Investigate,
                ReviewScreenMode::Week => ReviewScreenMode::Today,
                ReviewScreenMode::Investigate => ReviewScreenMode::Week,
            },
            NavMove::Next => self.review_mode.next(),
            NavMove::First | NavMove::PageBackward => ReviewScreenMode::Today,
            NavMove::Last | NavMove::PageForward => ReviewScreenMode::Investigate,
        };
        self.selected_review_card_index = 0;
        self.status_line = format!("Review mode changed to {}.", self.review_mode.label());
        self.rebuild_live_model();
    }

    fn move_review_focus(&mut self, movement: NavMove) {
        let all = ReviewFocus::ALL;
        let current = all
            .iter()
            .position(|focus| *focus == self.review_focus)
            .unwrap_or(0);
        let next = match movement {
            NavMove::Previous => {
                if current == 0 {
                    all.len().saturating_sub(1)
                } else {
                    current - 1
                }
            }
            NavMove::Next => (current + 1) % all.len(),
            NavMove::First | NavMove::PageBackward => 0,
            NavMove::Last | NavMove::PageForward => all.len().saturating_sub(1),
        };
        self.review_focus = all[next];
        self.selected_review_card_index = 0;
        self.status_line = format!(
            "Investigation focus changed to {}.",
            self.review_focus.label()
        );
        self.rebuild_live_model();
    }

    fn move_review_cards(&mut self, movement: NavMove) {
        match movement {
            NavMove::Previous => {
                self.handle(Action::PreviousReviewCard);
            }
            NavMove::Next => {
                self.handle(Action::NextReviewCard);
            }
            NavMove::First => {
                self.selected_review_card_index = 0;
                self.rebuild_live_model();
                "Moved to the first review card.".clone_into(&mut self.status_line);
            }
            NavMove::Last => {
                self.selected_review_card_index =
                    self.current_review_card_count().saturating_sub(1);
                self.rebuild_live_model();
                "Moved to the last review card.".clone_into(&mut self.status_line);
            }
            NavMove::PageBackward => {
                self.selected_review_card_index = self.selected_review_card_index.saturating_sub(5);
                self.rebuild_live_model();
                "Jumped earlier in the review deck.".clone_into(&mut self.status_line);
            }
            NavMove::PageForward => {
                let max_index = self.current_review_card_count().saturating_sub(1);
                self.selected_review_card_index =
                    usize::min(self.selected_review_card_index.saturating_add(5), max_index);
                self.rebuild_live_model();
                "Jumped later in the review deck.".clone_into(&mut self.status_line);
            }
        }
    }

    fn move_ai_browser_tabs(&mut self, movement: NavMove) {
        self.ai_browser_tab = match movement {
            NavMove::Previous => self.ai_browser_tab.previous(),
            NavMove::Next => self.ai_browser_tab.next(),
            NavMove::First | NavMove::PageBackward => AiBrowserTab::Runs,
            NavMove::Last | NavMove::PageForward => AiBrowserTab::Evals,
        };
        self.selected_ai_artifact_action_index = 0;
        self.status_line = format!("AI browser switched to {}.", self.ai_browser_tab.label());
        self.rebuild_live_model();
    }

    fn move_ai_launch_points(&mut self, movement: NavMove) {
        let count = self.model.ai.launch_points.len();
        if count == 0 {
            return;
        }
        let current = self.selected_ai_launch_index();
        let next = match movement {
            NavMove::Previous => current.saturating_sub(1),
            NavMove::Next => usize::min(current + 1, count.saturating_sub(1)),
            NavMove::First => 0,
            NavMove::Last => count.saturating_sub(1),
            NavMove::PageBackward => current.saturating_sub(5),
            NavMove::PageForward => usize::min(current.saturating_add(5), count.saturating_sub(1)),
        };
        self.set_selected_ai_launch_index(next);
        self.status_line = format!("Focused AI launch point {}.", next + 1);
    }

    fn move_ai_browser_items(&mut self, movement: NavMove) {
        match movement {
            NavMove::Previous => {
                self.handle(Action::PreviousAiBrowserItem);
            }
            NavMove::Next => {
                self.handle(Action::NextAiBrowserItem);
            }
            NavMove::First => {
                self.set_ai_browser_index(0);
                self.selected_ai_artifact_action_index = 0;
                self.rebuild_live_model();
                "Moved to the first saved artifact.".clone_into(&mut self.status_line);
            }
            NavMove::Last => {
                let last = self.ai_browser_item_count().saturating_sub(1);
                self.set_ai_browser_index(last);
                self.selected_ai_artifact_action_index = 0;
                self.rebuild_live_model();
                "Moved to the last saved artifact.".clone_into(&mut self.status_line);
            }
            NavMove::PageBackward => {
                self.adjust_ai_browser_index(-5);
                self.selected_ai_artifact_action_index = 0;
                self.rebuild_live_model();
                "Jumped earlier in saved artifacts.".clone_into(&mut self.status_line);
            }
            NavMove::PageForward => {
                self.adjust_ai_browser_index(5);
                self.selected_ai_artifact_action_index = 0;
                self.rebuild_live_model();
                "Jumped later in saved artifacts.".clone_into(&mut self.status_line);
            }
        }
    }

    fn move_ai_artifact_actions(&mut self, movement: NavMove) {
        let count = self.model.ai.artifact_actions.len();
        if count == 0 {
            "No direct actions are available for the selected saved artifact."
                .clone_into(&mut self.status_line);
            return;
        }
        let current = self.selected_ai_artifact_action_index();
        let next = match movement {
            NavMove::Previous => current.saturating_sub(1),
            NavMove::Next => usize::min(current + 1, count.saturating_sub(1)),
            NavMove::First => 0,
            NavMove::Last => count.saturating_sub(1),
            NavMove::PageBackward => current.saturating_sub(5),
            NavMove::PageForward => usize::min(current.saturating_add(5), count.saturating_sub(1)),
        };
        self.set_selected_ai_artifact_action_index(next);
        self.status_line = format!("Focused artifact action {}.", next + 1);
    }

    const fn screen_supports_day_paging(screen: Screen) -> bool {
        matches!(
            screen,
            Screen::Dashboard | Screen::Timeline | Screen::Explain | Screen::Review
        )
    }

    fn selected_ai_launch_index(&self) -> usize {
        let count = self.model.ai.launch_points.len();
        if count == 0 {
            0
        } else {
            usize::min(self.selected_ai_launch_index, count.saturating_sub(1))
        }
    }

    fn set_selected_ai_launch_index(&mut self, index: usize) {
        let count = self.model.ai.launch_points.len();
        if count == 0 {
            return;
        }
        self.selected_ai_launch_index = usize::min(index, count.saturating_sub(1));
        self.rebuild_live_model();
    }

    fn selected_ai_artifact_action_index(&self) -> usize {
        let count = self.model.ai.artifact_actions.len();
        if count == 0 {
            0
        } else {
            usize::min(
                self.selected_ai_artifact_action_index,
                count.saturating_sub(1),
            )
        }
    }

    fn set_selected_ai_artifact_action_index(&mut self, index: usize) {
        let count = self.model.ai.artifact_actions.len();
        if count == 0 {
            self.selected_ai_artifact_action_index = 0;
            return;
        }
        self.selected_ai_artifact_action_index = usize::min(index, count.saturating_sub(1));
        self.rebuild_live_model();
    }

    fn current_ai_artifact_action(&self) -> Option<Action> {
        let action_kind = self
            .current_ai_artifact_action_kinds()
            .get(self.selected_ai_artifact_action_index())
            .copied()?;
        Some(action_kind.action())
    }

    fn current_ai_artifact_action_kinds(&self) -> Vec<AiArtifactActionKind> {
        let snapshot = self.live_snapshot.as_ref();
        ai_artifact_action_kinds(
            self.ai_browser_tab,
            snapshot.and_then(|data| data.ai_runs.get(self.selected_ai_run_index)),
            snapshot.and_then(|data| {
                data.snapshot_catalog
                    .get(self.selected_snapshot_catalog_index)
            }),
            snapshot.and_then(|data| data.report_exports.get(self.selected_report_export_index)),
            snapshot.and_then(|data| data.ai_eval_runs.get(self.selected_ai_eval_run_index)),
        )
    }

    const fn ai_browser_item_count(&self) -> usize {
        self.model.ai.browser_items.len()
    }

    const fn set_ai_browser_index(&mut self, index: usize) {
        match self.ai_browser_tab {
            AiBrowserTab::Runs => self.selected_ai_run_index = index,
            AiBrowserTab::Snapshots => self.selected_snapshot_catalog_index = index,
            AiBrowserTab::Reports => self.selected_report_export_index = index,
            AiBrowserTab::Evals => self.selected_ai_eval_run_index = index,
        }
    }

    fn select_timeline_event_at(&mut self, index: usize) {
        let Some(snapshot) = &self.live_snapshot else {
            return;
        };
        let Some(day) = self.selected_day_label() else {
            return;
        };
        let events = filtered_events_for_day(snapshot, &day, &self.overlay_filters);
        if let Some(event) = events.get(index) {
            let selected_id = event.context_event_id.clone();
            let title = event.title.clone();
            self.selected_event_id = Some(selected_id);
            self.align_point_to_selected_event();
            self.rebuild_live_model();
            self.status_line = format!("Selected {title}.");
        }
    }

    fn move_dashboard_breakdown_selection(&mut self, movement: NavMove) {
        let count = self.model.dashboard.breakdown.rails.len();
        if count == 0 {
            return;
        }
        let current = self
            .selected_dashboard_breakdown_index
            .min(count.saturating_sub(1));
        let next = match movement {
            NavMove::Previous => current.saturating_sub(1),
            NavMove::Next => usize::min(current + 1, count.saturating_sub(1)),
            NavMove::First => 0,
            NavMove::Last => count.saturating_sub(1),
            NavMove::PageBackward => current.saturating_sub(2),
            NavMove::PageForward => usize::min(current.saturating_add(2), count.saturating_sub(1)),
        };
        self.selected_dashboard_breakdown_index = next;
        if let Some(rail) = self.model.dashboard.breakdown.rails.get(next) {
            self.status_line = format!("Focused {}.", rail.label);
        }
        self.rebuild_live_model();
    }

    fn move_dashboard_heatmap_selection(&mut self, movement: NavMove) {
        let count = self.available_day_count();
        if count == 0 {
            return;
        }
        let next = match movement {
            NavMove::Previous => self.selected_day_index.saturating_sub(1),
            NavMove::Next => usize::min(self.selected_day_index + 1, count.saturating_sub(1)),
            NavMove::First => 0,
            NavMove::Last => count.saturating_sub(1),
            NavMove::PageBackward => self.selected_day_index.saturating_sub(7),
            NavMove::PageForward => usize::min(
                self.selected_day_index.saturating_add(7),
                count.saturating_sub(1),
            ),
        };
        if next != self.selected_day_index {
            self.selected_day_index = next;
            self.reset_day_navigation();
            self.status_line = format!(
                "Heatmap selected {}.",
                self.selected_day_label()
                    .unwrap_or_else(|| "the current day".to_owned())
            );
            self.rebuild_live_model();
        }
    }

    fn move_trend_row(&mut self, movement: NavMove) {
        let count = self.model.trends.rows.len();
        if count == 0 {
            return;
        }
        let current = self.selected_trend_row_index.min(count.saturating_sub(1));
        let next = match movement {
            NavMove::Previous => current.saturating_sub(1),
            NavMove::Next => usize::min(current + 1, count.saturating_sub(1)),
            NavMove::First => 0,
            NavMove::Last => count.saturating_sub(1),
            NavMove::PageBackward => current.saturating_sub(3),
            NavMove::PageForward => usize::min(current.saturating_add(3), count.saturating_sub(1)),
        };
        self.selected_trend_row_index = next;
        if let Some(row) = self.model.trends.rows.get(next) {
            self.status_line = format!("Focused {} trend row.", row.label);
        }
        self.rebuild_live_model();
    }

    fn set_trend_sort_mode(&mut self, mode: TrendSortMode) {
        let selected_label = self.focused_trend_row().map(|row| row.label);
        self.trend_sort_mode = mode;
        self.status_line = format!("Trend sort changed to {}.", self.trend_sort_mode.label());
        self.rebuild_live_model();
        if let Some(label) = selected_label
            && let Some(index) = self
                .model
                .trends
                .rows
                .iter()
                .position(|row| row.label == label)
        {
            self.selected_trend_row_index = index;
            self.rebuild_live_model();
        }
    }

    fn focus_trend_row_by_label(&mut self, label: &str) {
        self.rebuild_live_model();
        if let Some(index) = self
            .model
            .trends
            .rows
            .iter()
            .position(|row| row.label == label)
        {
            self.selected_trend_row_index = index;
            self.rebuild_live_model();
        }
    }

    fn toggle_region_expansion(&mut self, region: FocusRegion, label: &str) {
        if self.expanded_region == Some(region) {
            self.expanded_region = None;
            self.status_line = format!("Collapsed {label}.");
        } else {
            self.expanded_region = Some(region);
            self.status_line = format!("Expanded {label}.");
        }
    }
}

impl Screen {
    pub const ALL: [Self; 8] = [
        Self::Dashboard,
        Self::Timeline,
        Self::Trends,
        Self::Explain,
        Self::Patterns,
        Self::Review,
        Self::Ai,
        Self::Ops,
    ];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Timeline => "Timeline",
            Self::Trends => "Trends",
            Self::Explain => "Explain",
            Self::Patterns => "Patterns",
            Self::Review => "Review",
            Self::Ai => "AI",
            Self::Ops => "Status",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Dashboard => 0,
            Self::Timeline => 1,
            Self::Trends => 2,
            Self::Explain => 3,
            Self::Patterns => 4,
            Self::Review => 5,
            Self::Ai => 6,
            Self::Ops => 7,
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Dashboard => Self::Timeline,
            Self::Timeline => Self::Trends,
            Self::Trends => Self::Explain,
            Self::Explain => Self::Patterns,
            Self::Patterns => Self::Review,
            Self::Review => Self::Ai,
            Self::Ai => Self::Ops,
            Self::Ops => Self::Dashboard,
        }
    }

    const fn previous(self) -> Self {
        match self {
            Self::Dashboard => Self::Ops,
            Self::Timeline => Self::Dashboard,
            Self::Trends => Self::Timeline,
            Self::Explain => Self::Trends,
            Self::Patterns => Self::Explain,
            Self::Review => Self::Patterns,
            Self::Ai => Self::Review,
            Self::Ops => Self::Ai,
        }
    }
}

impl AiBrowserTab {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Runs => "runs",
            Self::Snapshots => "snapshots",
            Self::Reports => "reports",
            Self::Evals => "evals",
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Runs => 0,
            Self::Snapshots => 1,
            Self::Reports => 2,
            Self::Evals => 3,
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Runs => Self::Snapshots,
            Self::Snapshots => Self::Reports,
            Self::Reports => Self::Evals,
            Self::Evals => Self::Runs,
        }
    }

    #[must_use]
    pub const fn previous(self) -> Self {
        match self {
            Self::Runs => Self::Evals,
            Self::Snapshots => Self::Runs,
            Self::Reports => Self::Snapshots,
            Self::Evals => Self::Reports,
        }
    }
}

impl AiLaunchIntent {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReviewSelectedDay => "AI review for the selected day",
            Self::CompareSelectedWeek => "AI compare for the selected week",
            Self::ChallengeSelectedDay => "AI challenge for the selected day",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::ReviewSelectedDay => "Review this day",
            Self::CompareSelectedWeek => "Compare this week",
            Self::ChallengeSelectedDay => "Challenge this view",
        }
    }
}

impl ReviewScreenMode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Week => "Week",
            Self::Investigate => "Investigate",
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Today => Self::Week,
            Self::Week => Self::Investigate,
            Self::Investigate => Self::Today,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Today => 0,
            Self::Week => 1,
            Self::Investigate => 2,
        }
    }
}

impl TrendWindowKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Days7 => "7d",
            Self::Days30 => "30d",
            Self::Days90 => "90d",
        }
    }

    #[must_use]
    pub const fn days(self) -> usize {
        match self {
            Self::Days7 => 7,
            Self::Days30 => 30,
            Self::Days90 => 90,
        }
    }
}

impl TrendSortMode {
    const ALL: [Self; 3] = [Self::Concern, Self::Anomaly, Self::Recovery];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Concern => "Concern",
            Self::Anomaly => "Anomaly",
            Self::Recovery => "Recovery",
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Concern => Self::Anomaly,
            Self::Anomaly => Self::Recovery,
            Self::Recovery => Self::Concern,
        }
    }

    const fn previous(self) -> Self {
        match self {
            Self::Concern => Self::Recovery,
            Self::Anomaly => Self::Concern,
            Self::Recovery => Self::Anomaly,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Concern => 0,
            Self::Anomaly => 1,
            Self::Recovery => 2,
        }
    }
}

impl PatternMetricFilter {
    const ALL: [Self; 4] = [Self::All, Self::Activity, Self::Readiness, Self::Sleep];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all metrics",
            Self::Activity => "activity",
            Self::Readiness => "next-day readiness",
            Self::Sleep => "same-night sleep",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Activity => "Activity",
            Self::Readiness => "Readiness",
            Self::Sleep => "Sleep",
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::All => Self::Activity,
            Self::Activity => Self::Readiness,
            Self::Readiness => Self::Sleep,
            Self::Sleep => Self::All,
        }
    }

    const fn previous(self) -> Self {
        match self {
            Self::All => Self::Sleep,
            Self::Activity => Self::All,
            Self::Readiness => Self::Activity,
            Self::Sleep => Self::Readiness,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::All => 0,
            Self::Activity => 1,
            Self::Readiness => 2,
            Self::Sleep => 3,
        }
    }

    const fn metric(self) -> Option<PatternMetric> {
        match self {
            Self::All => None,
            Self::Activity => Some(PatternMetric::Activity),
            Self::Readiness => Some(PatternMetric::Readiness),
            Self::Sleep => Some(PatternMetric::Sleep),
        }
    }
}

impl OverlayFilterState {
    #[must_use]
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

pub fn build_state_from_snapshot(
    mode: RunMode,
    status_line: impl Into<String>,
    snapshot: LiveSnapshot,
) -> AppState {
    let selected_day_index = newest_day_index(&snapshot);
    let screen_focus_memory =
        std::array::from_fn(|index| navigation::default_region(Screen::ALL[index]));
    let mut app = AppState {
        mode,
        active_screen: Screen::Dashboard,
        model: AppModel::empty(),
        status_line: status_line.into(),
        tick_count: 0,
        should_quit: false,
        refresh_in_flight: false,
        live_snapshot: Some(snapshot),
        focused_region: navigation::default_region(Screen::Dashboard),
        screen_focus_memory,
        focused_top_nav_screen: Screen::Dashboard,
        help_open: false,
        focus_before_help: None,
        search: None,
        selected_day_index,
        selected_timeline_point: 0,
        timeline_window_hours: 24,
        selected_overlay_toggle_index: 0,
        trends_window: TrendWindowKind::Days7,
        trend_sort_mode: TrendSortMode::Concern,
        selected_trend_row_index: 0,
        selected_event_id: None,
        selected_dashboard_breakdown_index: 0,
        expanded_region: None,
        selected_review_card_index: 0,
        ai_preflight: None,
        ai_preflight_control: PreflightControl::Confirm,
        ai_browser_tab: AiBrowserTab::Runs,
        selected_ai_launch_index: 0,
        selected_ai_run_index: 0,
        selected_snapshot_catalog_index: 0,
        selected_report_export_index: 0,
        selected_ai_eval_run_index: 0,
        selected_ai_artifact_action_index: 0,
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

/// Build an application state from the current local store snapshot.
///
/// # Errors
///
/// Returns an error when loading the current live snapshot from the store fails.
pub fn build_live_state(
    config: &Config,
    store: &Store,
    auth_status: &AuthStatus,
) -> crate::error::Result<AppState> {
    interrupt_stale_ai_runs(store)?;
    build_read_only_live_state(config, store, auth_status)
}

pub(crate) fn build_read_only_live_state(
    config: &Config,
    store: &Store,
    auth_status: &AuthStatus,
) -> crate::error::Result<AppState> {
    let snapshot = load_live_snapshot(config, store, auth_status)?;
    Ok(build_state_from_snapshot(
        RunMode::Live,
        "Live mode is reading from the local store.",
        snapshot,
    ))
}

fn interrupt_stale_ai_runs(store: &Store) -> crate::error::Result<()> {
    let interrupted_at = now_rfc3339();
    for mut run in store.analysis().list_ai_run_records()? {
        if matches!(run.run_status.as_str(), "queued" | "running") {
            ai::AiRunStatus::Interrupted
                .as_str()
                .clone_into(&mut run.run_status);
            run.error_message.get_or_insert_with(|| {
                "Interrupted when a previous TUI session ended before the run completed.".to_owned()
            });
            run.ended_at.get_or_insert_with(|| interrupted_at.clone());
            run.updated_at.clone_from(&interrupted_at);
            store.analysis().upsert_ai_run(&run)?;
        }
    }
    Ok(())
}

/// Load the live snapshot that drives the terminal UI from local persisted state.
///
/// # Errors
///
/// Returns an error when store-backed snapshot queries fail.
pub fn load_live_snapshot(
    config: &Config,
    store: &Store,
    auth_status: &AuthStatus,
) -> crate::error::Result<LiveSnapshot> {
    let daily_history = store
        .views()
        .daily_history(usize::from(config.refresh.daily_history_days))?;
    let daily_bounds = daily_history
        .first()
        .zip(daily_history.last())
        .map(|(start, end)| (start.day.clone(), end.day.clone()));
    let (
        daily_activity,
        daily_readiness,
        daily_stress,
        sleep_periods,
        daily_spo2,
        daily_resilience,
        daily_cardiovascular_age,
        vo2_max,
    ) = if let Some((start_day, end_day)) = daily_bounds {
        (
            store
                .views()
                .daily_activity_between_days(&start_day, &end_day)?,
            store
                .views()
                .daily_readiness_between_days(&start_day, &end_day)?,
            store
                .views()
                .daily_stress_between_days(&start_day, &end_day)?,
            store
                .views()
                .sleep_periods_between_days(&start_day, &end_day)?,
            store
                .views()
                .daily_spo2_between_days(&start_day, &end_day)?,
            store
                .views()
                .daily_resilience_between_days(&start_day, &end_day)?,
            store
                .views()
                .daily_cardiovascular_age_between_days(&start_day, &end_day)?,
            store.views().vo2_max_between_days(&start_day, &end_day)?,
        )
    } else {
        (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    };
    let heartrate_days = load_heartrate_days(store, 14)?;
    let heartrate_daily_averages = load_heartrate_daily_averages(store, 90)?;
    let pattern_summaries = store.views().pattern_summaries(None, None)?;
    let latest_review_day = store.views().latest_review_day()?;
    let review_load_bounds = live_review_load_bounds(
        &daily_history,
        &heartrate_days,
        latest_review_day.as_deref(),
    )?;

    let (review_signal_days, sleep_time, context_events, rest_mode_periods) = if let Some(bounds) =
        review_load_bounds
    {
        (
            store
                .views()
                .review_signal_days_between_days(&bounds.signal_start, &bounds.signal_end)?,
            store
                .views()
                .sleep_time_between_days(&bounds.sleep_start, &bounds.sleep_end)?,
            store
                .views()
                .context_events_between_days(&bounds.context_start, &bounds.context_end)?,
            store
                .views()
                .rest_mode_periods_between_days(&bounds.rest_mode_start, &bounds.rest_mode_end)?,
        )
    } else {
        (Vec::new(), Vec::new(), Vec::new(), Vec::new())
    };
    let candidate_days = live_snapshot_day_candidates(
        &daily_history,
        &heartrate_days,
        &context_events,
        latest_review_day.as_deref(),
    );
    let mut ai_artifacts_by_day = BTreeMap::new();
    for day in candidate_days {
        if let Some(artifact) = store.analysis().latest_ai_artifact_for_anchor_day(&day)? {
            ai_artifacts_by_day.insert(day, artifact);
        }
    }
    let snapshot_catalog = store.analysis().list_snapshot_exports()?;
    let ai_runs = store.analysis().list_ai_run_records()?;
    let ai_artifact_records = store.analysis().list_ai_artifact_records()?;
    let report_exports = store.analysis().list_report_exports()?;
    let ai_eval_runs = store.analysis().list_ai_eval_runs()?;
    let ai_ops = build_ai_ops_snapshot(
        config,
        &snapshot_catalog,
        &ai_runs,
        &ai_artifact_records,
        &report_exports,
        &ai_eval_runs,
    );

    let captured_at = now_rfc3339();
    let stale_evidence_entries = stale_evidence_warnings(OffsetDateTime::now_utc().date());

    Ok(LiveSnapshot {
        captured_at,
        refresh_policy: RefreshPolicySnapshot::from_config(config),
        auth_status: auth_status.clone(),
        active_population_profile: config.guidance.active_population_profile,
        guidance_profile_source: config.guidance.source_label().to_owned(),
        evidence_registry_version: evidence_registry_version().to_owned(),
        stale_evidence_entries,
        ai_ops,
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
        daily_activity,
        daily_readiness,
        daily_stress,
        sleep_periods,
        daily_spo2,
        heartrate_days,
        heartrate_daily_averages,
        context_events,
        pattern_summaries,
        review_signal_days,
        sleep_time,
        rest_mode_periods,
        daily_resilience,
        daily_cardiovascular_age,
        vo2_max,
        ai_artifacts_by_day,
        snapshot_catalog,
        ai_runs,
        ai_artifact_records,
        report_exports,
        ai_eval_runs,
        sync_states: store.sync_state().list()?,
        record_counts: store.views().record_counts()?,
        schema_version: store.metadata().schema_version()?,
        database_path: store.plan().db_path.display().to_string(),
        config_path: config.paths.config_file.display().to_string(),
    })
}

#[must_use]
pub fn build_demo_state(config: &Config) -> AppState {
    let snapshot = demo_snapshot(config);
    build_state_from_snapshot(RunMode::Demo, "Demo mode ready.", snapshot)
}

fn serialize_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{}".to_owned())
}

fn serialize_pretty_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_owned())
}

fn build_ai_ops_snapshot(
    config: &Config,
    snapshot_catalog: &[SnapshotCatalogEntry],
    ai_runs: &[AiRunRecord],
    ai_artifact_records: &[AiArtifactRecord],
    report_exports: &[ReportExportRecord],
    ai_eval_runs: &[AiEvalRunRecord],
) -> AiOpsSnapshot {
    let last_successful_run = ai_runs
        .iter()
        .find(|run| run.run_status == "succeeded")
        .map(|run| format!("{} {}", run.run_kind, run.created_at));
    let last_failed_run = ai_runs
        .iter()
        .find(|run| {
            matches!(
                run.run_status.as_str(),
                "failed" | "cancelled" | "interrupted"
            )
        })
        .map(|run| format!("{} {} ({})", run.run_kind, run.created_at, run.run_status));

    AiOpsSnapshot {
        enabled: config.ai.enabled,
        provider: format!("{:?}", config.ai.provider).to_ascii_lowercase(),
        api_key_env: config.ai.api_key_env.clone(),
        api_key_ready: env::var_os(&config.ai.api_key_env).is_some(),
        default_model: config.ai.model.clone(),
        reasoning_effort: config
            .ai
            .reasoning_effort
            .clone()
            .unwrap_or_else(|| "default".to_owned()),
        request_mode: match config.ai.request_mode {
            AiRequestMode::Stateless => "stateless".to_owned(),
            AiRequestMode::Stateful => "stateful".to_owned(),
        },
        input_transport: match config.ai.input_transport {
            crate::config::AiInputTransport::Inline => "inline".to_owned(),
            crate::config::AiInputTransport::FileUpload => "file_upload".to_owned(),
        },
        prompt_cache: match config.ai.prompt_cache {
            crate::config::PromptCacheMode::Off => "off".to_owned(),
            crate::config::PromptCacheMode::Auto => "auto".to_owned(),
        },
        review_prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
        compare_prompt_version: COMPARE_PROMPT_VERSION.to_owned(),
        tools_disabled: true,
        snapshot_catalog_count: snapshot_catalog.len(),
        ai_run_count: ai_runs.len(),
        ai_artifact_count: ai_artifact_records.len(),
        report_export_count: report_exports.len(),
        ai_eval_run_count: ai_eval_runs.len(),
        last_successful_run,
        last_failed_run,
    }
}

fn build_live_model(snapshot: &LiveSnapshot, options: &LiveModelOptions) -> AppModel {
    let selected_day = selected_day_label(snapshot, options.selected_day_index)
        .unwrap_or_else(|| latest_review_anchor_day(snapshot));
    let ai_artifact = snapshot.ai_artifacts_by_day.get(&selected_day).map_or_else(
        empty_ai_artifact_summary_view,
        build_ai_artifact_summary_view,
    );
    let review_inputs = ReviewInputs {
        auth_status: &snapshot.auth_status,
        active_population_profile: snapshot.active_population_profile,
        signal_days: &snapshot.review_signal_days,
        context_events: &snapshot.context_events,
        pattern_summaries: &snapshot.pattern_summaries,
        sleep_time: &snapshot.sleep_time,
        rest_mode_periods: &snapshot.rest_mode_periods,
    };
    let today_review = build_review_deck(ReviewMode::Today, &selected_day, &review_inputs)
        .unwrap_or_else(|error| empty_review_deck(ReviewMode::Today, &selected_day, &error));
    let week_review = build_review_deck(ReviewMode::Week, &selected_day, &review_inputs)
        .unwrap_or_else(|error| empty_review_deck(ReviewMode::Week, &selected_day, &error));
    let investigation =
        build_investigation_report(options.review_focus, &selected_day, &review_inputs)
            .unwrap_or_else(|error| {
                empty_investigation_report(options.review_focus, &selected_day, &error)
            });
    let review_context = ReviewViewContext {
        selected_day: &selected_day,
        ai_artifact: &ai_artifact,
        review_mode: options.review_mode,
        review_focus: options.review_focus,
        selected_review_card_index: options.selected_review_card_index,
    };

    AppModel {
        title: build_app_title(snapshot, &selected_day, options.refresh_in_flight),
        dashboard: build_dashboard_model(
            snapshot,
            options.selected_day_index,
            options.refresh_in_flight,
            &today_review,
            options.selected_dashboard_breakdown_index,
        ),
        timeline: build_timeline_model(
            snapshot,
            options.selected_day_index,
            options.selected_point_index,
            options.selected_event_id.as_deref(),
            &options.overlay_filters,
            options.selected_overlay_toggle_index,
            options.window_hours,
        ),
        trends: build_trends_model(
            snapshot,
            options.trend_sort_mode,
            options.selected_trend_row_index,
            &week_review,
        ),
        explain: build_explain_model(
            snapshot,
            options.selected_day_index,
            options.selected_event_id.as_deref(),
            &options.overlay_filters,
            options.selected_overlay_toggle_index,
            &today_review,
        ),
        patterns: build_patterns_model(
            snapshot,
            &options.overlay_filters,
            options.selected_overlay_toggle_index,
            options.pattern_metric_filter,
        ),
        review: build_review_model(
            snapshot,
            &today_review,
            &week_review,
            &investigation,
            &review_context,
        ),
        ai: build_ai_workbench_model(snapshot, options),
        ops: build_ops_model(snapshot, options.refresh_in_flight),
    }
}

fn build_dashboard_model(
    snapshot: &LiveSnapshot,
    selected_day_index: usize,
    refresh_in_flight: bool,
    today_review: &ReviewDeck,
    selected_breakdown_index: usize,
) -> DashboardModel {
    let selected_day = selected_day_label(snapshot, selected_day_index)
        .unwrap_or_else(|| "no selected day".to_owned());
    let selected_daily = selected_daily_row(snapshot, &selected_day);
    let selected_activity = selected_daily_activity(snapshot, &selected_day);
    let selected_readiness = selected_daily_readiness(snapshot, &selected_day);
    let selected_stress = selected_daily_stress(snapshot, &selected_day);
    let selected_sleep_period = selected_primary_sleep_period(snapshot, &selected_day);
    let selected_spo2 = selected_daily_spo2(snapshot, &selected_day);
    let sleep_insight = build_day_metric_insight(snapshot, &selected_day, "sleep", |row| {
        row.sleep_score.map(f64::from)
    });
    let readiness_insight = build_day_metric_insight(snapshot, &selected_day, "readiness", |row| {
        row.readiness_score.map(f64::from)
    });
    let activity_insight = build_metric_insight(
        "activity",
        &metric_points_from_activity(&snapshot.daily_activity),
    );
    let heartrate_insight = build_metric_insight("heartrate", &snapshot.heartrate_daily_averages);
    let hrv_points =
        metric_points_from_sleep_periods(&snapshot.sleep_periods, |record| record.average_hrv);
    let hrv_insight = build_metric_insight_from_points(&hrv_points, &selected_day, "hrv");
    let respiratory_points =
        metric_points_from_sleep_periods(&snapshot.sleep_periods, |record| record.average_breath);
    let respiratory_insight =
        build_metric_insight_from_points(&respiratory_points, &selected_day, "respiratory rate");
    let spo2_points = metric_points_from_daily_spo2(&snapshot.daily_spo2);
    let spo2_insight = build_metric_insight_from_points(&spo2_points, &selected_day, "spo2");
    let daily_availability =
        availability_from_freshness(&family_freshness(snapshot, DataFamily::Daily));
    let heartrate_availability =
        availability_from_freshness(&family_freshness(snapshot, DataFamily::Heartrate));
    let activity_availability = if selected_activity.is_some() {
        daily_availability
    } else {
        TelemetryAvailability::NoData
    };
    let readiness_availability = if selected_readiness.is_some() || selected_daily.is_some() {
        daily_availability
    } else {
        TelemetryAvailability::NoData
    };
    let sleep_availability = if selected_daily
        .and_then(|row| row.sleep_duration_seconds)
        .is_some()
    {
        daily_availability
    } else {
        TelemetryAvailability::NoData
    };
    let hrv_availability = telemetry_availability_for_daily_metric(
        snapshot,
        CapabilityKind::Daily,
        !hrv_points.is_empty(),
    );
    let respiratory_availability = telemetry_availability_for_daily_metric(
        snapshot,
        CapabilityKind::Daily,
        !respiratory_points.is_empty(),
    );
    let spo2_availability = telemetry_availability_for_daily_metric(
        snapshot,
        CapabilityKind::Spo2,
        !spo2_points.is_empty(),
    );
    let capability_summary = dashboard_capability_summary(snapshot);
    let coverage = coverage_cell_views(snapshot);

    DashboardModel {
        header: HeaderStripModel {
            app_title: "ringmaster.rs".to_owned(),
            selected_period: format!("DAY {selected_day}"),
            freshness_badge: dashboard_header_freshness(snapshot),
            sync_status: if refresh_in_flight {
                "SYNCING".to_owned()
            } else {
                "LOCAL CACHE".to_owned()
            },
            capability_summary,
            coverage,
        },
        selected_day_label: selected_day.clone(),
        readiness: DashboardScoreTile {
            availability: readiness_availability,
            primary_value: selected_daily
                .and_then(|row| row.readiness_score)
                .map_or_else(|| "--".to_owned(), |value| value.to_string()),
            secondary_lines: vec![
                selected_daily.and_then(|row| row.sleep_score).map_or_else(
                    || "sleep score --".to_owned(),
                    |value| format!("sleep {value}"),
                ),
                selected_stress
                    .and_then(|row| row.day_summary.clone())
                    .unwrap_or_else(|| "recovery state pending".to_owned()),
            ],
            delta_label: metric_delta_label(&readiness_insight),
            trend: values_from_metric_points(&metric_points_from_daily(
                &snapshot.daily_history,
                |row| row.readiness_score.map(f64::from),
            )),
            ring_fill_percent: selected_daily
                .and_then(|row| row.readiness_score)
                .map_or(0, u16::from),
            note: today_review.observations.first().map_or_else(
                || selected_day_baseline_sentence("Readiness", &selected_day, &readiness_insight),
                |card| format!("Review: {}", card.headline),
            ),
        },
        sleep: DashboardSleepTile {
            availability: sleep_availability,
            duration_label: selected_daily
                .and_then(|row| row.sleep_duration_seconds)
                .map_or_else(|| "--".to_owned(), format_duration_compact),
            score_label: selected_daily
                .and_then(|row| row.sleep_score)
                .map_or_else(|| "score --".to_owned(), |value| format!("score {value}")),
            trend: values_from_metric_points(&metric_points_from_daily(
                &snapshot.daily_history,
                |row| row.sleep_duration_seconds.map(crate::numeric::i64_to_f64),
            )),
            strip_note: selected_day_baseline_sentence("Sleep", &selected_day, &sleep_insight),
        },
        activity: DashboardScoreTile {
            availability: activity_availability,
            primary_value: selected_activity.map_or_else(
                || {
                    selected_daily
                        .and_then(|row| row.activity_score)
                        .map_or_else(|| "--".to_owned(), |value| value.to_string())
                },
                |record| format_number(record.steps),
            ),
            secondary_lines: vec![
                selected_activity.map_or_else(
                    || "active kcal --".to_owned(),
                    |record| format!("{} kcal", record.active_calories),
                ),
                selected_activity
                    .and_then(|record| record.activity_score)
                    .map_or_else(|| "score --".to_owned(), |value| format!("score {value}")),
            ],
            delta_label: activity_delta_label(snapshot, &selected_day),
            trend: values_from_metric_points(&metric_points_from_activity(
                &snapshot.daily_activity,
            )),
            ring_fill_percent: selected_activity
                .and_then(|record| record.activity_score)
                .map_or_else(
                    || {
                        activity_ring_fill_from_steps(
                            selected_activity.map_or(0, |record| record.steps),
                        )
                    },
                    u16::from,
                ),
            note: selected_day_baseline_sentence("Activity", &selected_day, &activity_insight),
        },
        hrv: DashboardTrendPanel {
            availability: hrv_availability,
            primary_label: selected_sleep_period
                .and_then(|record| record.average_hrv)
                .map_or_else(|| "--".to_owned(), |value| format!("{value:.0} ms")),
            baseline_label: metric_delta_label(&hrv_insight),
            range_label: metric_range_label(&hrv_points),
            values: values_from_metric_points(&hrv_points),
            note: selected_metric_note(
                "HRV",
                &selected_day,
                selected_sleep_period
                    .and_then(|record| record.average_hrv)
                    .is_some(),
                &hrv_insight,
            ),
        },
        body_temp: DashboardThermometerPanel {
            availability: if selected_readiness
                .and_then(|row| row.temperature_deviation)
                .is_some()
            {
                readiness_availability
            } else {
                TelemetryAvailability::NoData
            },
            deviation_tenths: selected_readiness
                .and_then(|row| row.temperature_deviation)
                .map(|value| {
                    crate::numeric::rounded_clamped_f64_to_i16(
                        value * 10.0,
                        f64::from(i16::MIN),
                        f64::from(i16::MAX),
                    )
                }),
            value_label: selected_readiness
                .and_then(|row| row.temperature_deviation)
                .map_or_else(|| "--".to_owned(), |value| format!("{value:+.1}°C")),
            note: selected_readiness
                .and_then(|row| row.temperature_trend_deviation)
                .map_or_else(
                    || "deviation vs baseline pending".to_owned(),
                    |value| format!("trend {value:+.1}°C"),
                ),
        },
        heart_rate: DashboardTrendPanel {
            availability: if snapshot.heartrate_daily_averages.is_empty() {
                TelemetryAvailability::NoData
            } else {
                heartrate_availability
            },
            primary_label: heart_rate_primary_label(snapshot, &selected_day),
            baseline_label: metric_delta_label(&heartrate_insight),
            range_label: metric_range_label(&snapshot.heartrate_daily_averages),
            values: values_from_metric_points(&snapshot.heartrate_daily_averages),
            note: heartrate_insight.summary.clone(),
        },
        spo2: DashboardTrendPanel {
            availability: spo2_availability,
            primary_label: selected_spo2
                .and_then(|record| record.average_spo2)
                .map_or_else(|| "--".to_owned(), |value| format!("{value:.1}%")),
            baseline_label: metric_delta_label(&spo2_insight),
            range_label: metric_range_label(&spo2_points),
            values: values_from_metric_points(&spo2_points),
            note: selected_spo2
                .and_then(|record| record.breathing_disturbance_index)
                .map_or_else(
                    || {
                        selected_metric_note(
                            "SpO2",
                            &selected_day,
                            selected_spo2
                                .and_then(|record| record.average_spo2)
                                .is_some(),
                            &spo2_insight,
                        )
                    },
                    |value| format!("BDI {value:.1} | {}", spo2_insight.summary),
                ),
        },
        respiratory_rate: DashboardHistogramPanel {
            availability: respiratory_availability,
            primary_label: selected_sleep_period
                .and_then(|record| record.average_breath)
                .map_or_else(|| "--".to_owned(), |value| format!("{value:.1} br/min")),
            bars: values_from_metric_points(&respiratory_points),
            note: selected_metric_note(
                "respiratory-rate",
                &selected_day,
                selected_sleep_period
                    .and_then(|record| record.average_breath)
                    .is_some(),
                &respiratory_insight,
            ),
        },
        breakdown: DashboardBreakdownPanel {
            availability: readiness_availability,
            rails: build_dashboard_breakdown_rails(&DashboardBreakdownInputs {
                snapshot,
                selected_day: &selected_day,
                sleep_insight: &sleep_insight,
                readiness_insight: &readiness_insight,
                heartrate_insight: &heartrate_insight,
                selected_readiness,
                selected_stress,
                selected_breakdown_index,
            }),
            waveform: recent_dashboard_waveform(snapshot),
            note: selected_stress
                .and_then(|row| row.day_summary.clone())
                .unwrap_or_else(|| "Driver rails explain the top-line recovery state.".to_owned()),
        },
        weekly: build_dashboard_weekly_heatmap(snapshot, &selected_day),
    }
}

fn build_timeline_model(
    snapshot: &LiveSnapshot,
    selected_day_index: usize,
    selected_point_index: usize,
    selected_event_id: Option<&str>,
    overlay_filters: &OverlayFilterState,
    selected_overlay_toggle_index: usize,
    window_hours: u16,
) -> TimelineModel {
    let freshness = family_freshness(snapshot, DataFamily::Heartrate);
    let day_labels = available_days(snapshot);
    let (clamped_day_index, selected_day) = timeline_day_selection(&day_labels, selected_day_index);
    let visible = visible_timeline_for_selected_day(snapshot, &selected_day, window_hours);
    let selected_point_index = clamp_selected_index(selected_point_index, visible.points.len());
    let events = filtered_events_for_day(snapshot, &selected_day, overlay_filters);
    let selected_event_index = timeline_selected_event_index(&events, selected_event_id);
    let selected_detail =
        timeline_selected_point_detail(&visible.points, selected_point_index, &freshness);

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
        window_presets: timeline_window_preset_views(window_hours),
        selected_window_preset_index: timeline_window_preset_index(window_hours),
        selected_day_label: selected_day.clone(),
        selected_day_index: clamped_day_index,
        heart_rate: visible.points,
        selected_point_index,
        window_hours,
        window_start_minute: visible.window_start_minute,
        window_end_minute: visible.window_end_minute,
        overlay_toggles: overlay_toggle_views(overlay_filters, selected_overlay_toggle_index),
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
        selected_detail,
        event_detail_lines: timeline_event_detail_lines(
            &selected_day,
            &events,
            selected_event_index,
        ),
        breadcrumb: timeline_breadcrumb(&selected_day, &events, selected_event_index),
    }
}

fn timeline_day_selection(day_labels: &[String], selected_day_index: usize) -> (usize, String) {
    let clamped_day_index = if day_labels.is_empty() {
        0
    } else {
        usize::min(selected_day_index, day_labels.len().saturating_sub(1))
    };
    let selected_day = day_labels
        .get(clamped_day_index)
        .cloned()
        .unwrap_or_else(|| "no day selected".to_owned());
    (clamped_day_index, selected_day)
}

fn visible_timeline_for_selected_day(
    snapshot: &LiveSnapshot,
    selected_day: &str,
    window_hours: u16,
) -> VisibleTimeline {
    selected_heartrate_day(snapshot, selected_day).map_or_else(
        || VisibleTimeline {
            points: Vec::new(),
            window_start_minute: 0,
            window_end_minute: 24 * 60 - 1,
        },
        |day| visible_timeline(day, window_hours),
    )
}

fn clamp_selected_index(selected_index: usize, len: usize) -> Option<usize> {
    if len == 0 {
        None
    } else {
        Some(usize::min(selected_index, len.saturating_sub(1)))
    }
}

fn timeline_selected_event_index(
    events: &[&ContextEventRecord],
    selected_event_id: Option<&str>,
) -> Option<usize> {
    selected_event_id.and_then(|event_id| {
        events
            .iter()
            .position(|event| event.context_event_id == event_id)
    })
}

fn timeline_window_preset_views(window_hours: u16) -> Vec<TimelineWindowPresetView> {
    TIMELINE_WINDOW_PRESETS
        .into_iter()
        .map(|preset| TimelineWindowPresetView {
            label: match preset {
                6 => "6h",
                12 => "12h",
                _ => "24h",
            },
            selected: preset == window_hours,
        })
        .collect()
}

fn timeline_selected_point_detail(
    points: &[TimelinePoint],
    selected_point_index: Option<usize>,
    freshness: &FreshnessState,
) -> String {
    selected_point_index
        .and_then(|index| points.get(index))
        .map_or_else(
            || freshness.detail.clone(),
            |point| {
                format!(
                    "Heartrate cursor: {} at {} bpm.",
                    point.recorded_at, point.bpm
                )
            },
        )
}

fn timeline_event_detail_lines(
    selected_day: &str,
    events: &[&ContextEventRecord],
    selected_event_index: Option<usize>,
) -> Vec<String> {
    selected_event_index
        .and_then(|index| events.get(index))
        .map_or_else(
            || {
                vec![
                    "No context event is selected for this day.".to_owned(),
                    "Use j/k or move the heartrate cursor to inspect nearby events.".to_owned(),
                ]
            },
            |event| explain_event_detail_lines(selected_day, event),
        )
}

fn timeline_breadcrumb(
    selected_day: &str,
    events: &[&ContextEventRecord],
    selected_event_index: Option<usize>,
) -> String {
    selected_event_index
        .and_then(|index| events.get(index))
        .map_or_else(
            || {
                format!(
                    "Day {} -> {} matching context event{}",
                    selected_day,
                    events.len(),
                    if events.len() == 1 { "" } else { "s" }
                )
            },
            |event| {
                let carryover = if event.anchor_day == selected_day {
                    String::new()
                } else {
                    format!(" | carries over from {}", event.anchor_day)
                };
                format!(
                    "Day {} -> {} {}{}",
                    selected_day,
                    overlay_family_label(event.family),
                    event.title,
                    carryover
                )
            },
        )
}

fn build_trends_model(
    snapshot: &LiveSnapshot,
    trend_sort_mode: TrendSortMode,
    selected_row_index: usize,
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
    let temperature_points = metric_points_from_readiness_temperature(&snapshot.daily_readiness);
    let stress_points = metric_points_from_stress(&snapshot.daily_stress);

    let mut rows = vec![
        trend_matrix_row(
            "Sleep",
            &sleep_points,
            coverage_availability(snapshot, CoverageFamily::Daily),
            false,
        ),
        trend_matrix_row(
            "Readiness",
            &readiness_points,
            coverage_availability(snapshot, CoverageFamily::Daily),
            false,
        ),
        trend_matrix_row(
            "Activity",
            &activity_points,
            coverage_availability(snapshot, CoverageFamily::Daily),
            false,
        ),
        trend_matrix_row(
            "Heart Rate",
            &heartrate_points,
            coverage_availability(snapshot, CoverageFamily::Heartrate),
            true,
        ),
        trend_matrix_row(
            "Temp Dev",
            &temperature_points,
            coverage_availability(snapshot, CoverageFamily::Daily),
            true,
        ),
        trend_matrix_row(
            "Stress",
            &stress_points,
            coverage_availability(snapshot, CoverageFamily::Daily),
            true,
        ),
    ];

    rows.sort_by(|left, right| {
        let left_score = trend_row_sort_score(left, trend_sort_mode);
        let right_score = trend_row_sort_score(right, trend_sort_mode);
        right_score.total_cmp(&left_score)
    });
    let selected_row_index = selected_row_index.min(rows.len().saturating_sub(1));
    if let Some(selected_row) = rows.get_mut(selected_row_index) {
        selected_row.selected = true;
    }

    let mut notes = vec![format!(
        "Sorted by {} so the most actionable telemetry rises to the top.",
        trend_sort_mode.label().to_ascii_lowercase()
    )];
    if let Some(card) = week_review.negative_drifts.first() {
        notes.push(format!("Weekly review: {}", card.headline));
    }
    if rows.iter().all(|row| {
        row.cells
            .iter()
            .all(|cell| cell.availability != TelemetryAvailability::Fresh)
    }) {
        notes.push("Trends stay sparse until more daily history is cached locally.".to_owned());
    }

    TrendsModel {
        sort_tabs: TrendSortMode::ALL
            .into_iter()
            .map(|mode| TrendSortTab {
                label: mode.label(),
                selected: mode == trend_sort_mode,
            })
            .collect(),
        selected_sort_index: trend_sort_mode.index(),
        rows,
        notes,
    }
}

fn trend_matrix_row(
    label: &'static str,
    history: &[MetricPoint],
    availability: TelemetryAvailability,
    higher_is_concerning: bool,
) -> TrendMatrixRow {
    let insight = build_metric_insight(label, history);
    let current_value = insight
        .today
        .as_ref()
        .map_or_else(|| "--".to_owned(), |point| format_float(point.value));
    let concern_label = trend_concern_label(&insight, higher_is_concerning);
    let detail = if matches!(
        availability,
        TelemetryAvailability::Fresh | TelemetryAvailability::Stale
    ) {
        insight.summary.clone()
    } else {
        availability.label().to_owned()
    };

    TrendMatrixRow {
        label,
        current_value,
        concern_label,
        selected: false,
        cells: vec![
            trend_matrix_cell("7d", &insight, availability, 7, higher_is_concerning),
            trend_matrix_cell("30d", &insight, availability, 30, higher_is_concerning),
            trend_matrix_cell("90d", &insight, availability, 90, higher_is_concerning),
        ],
        sparkline: window_sparkline(history, 14),
        detail,
    }
}

fn trend_matrix_cell(
    label: &'static str,
    insight: &MetricInsight,
    availability: TelemetryAvailability,
    window_days: usize,
    higher_is_concerning: bool,
) -> TrendMatrixCell {
    let baseline = if window_days == 7 {
        &insight.baseline_7d
    } else {
        &insight.baseline_30d
    };
    let delta = baseline
        .delta_from_today
        .or(insight.day_over_day_delta)
        .unwrap_or_default();
    let delta_label = if baseline.sample_count >= 4 || insight.day_over_day_delta.is_some() {
        format!("{delta:+.1}")
    } else {
        "--".to_owned()
    };
    let concern_fill = if higher_is_concerning {
        (50.0 + delta * 12.0).clamp(0.0, 100.0)
    } else {
        (50.0 - delta * 12.0).clamp(0.0, 100.0)
    };

    TrendMatrixCell {
        label,
        delta_label,
        fill_percent: crate::numeric::rounded_clamped_f64_to_u16(concern_fill, 0.0, 100.0),
        availability,
    }
}

fn trend_concern_label(insight: &MetricInsight, higher_is_concerning: bool) -> String {
    let delta = insight
        .baseline_7d
        .delta_from_today
        .or(insight.day_over_day_delta)
        .unwrap_or_default();
    let concern = if higher_is_concerning { delta } else { -delta };
    if concern >= 3.0 {
        "watch".to_owned()
    } else if concern <= -3.0 {
        "recovered".to_owned()
    } else {
        "stable".to_owned()
    }
}

fn trend_row_sort_score(row: &TrendMatrixRow, mode: TrendSortMode) -> f64 {
    let primary_fill = row
        .cells
        .first()
        .map_or(0.0, |cell| f64::from(cell.fill_percent));
    let anomaly = row
        .cells
        .iter()
        .filter_map(|cell| cell.delta_label.parse::<f64>().ok().map(f64::abs))
        .fold(0.0, f64::max);

    match mode {
        TrendSortMode::Concern => primary_fill,
        TrendSortMode::Anomaly => anomaly * 10.0 + primary_fill / 10.0,
        TrendSortMode::Recovery => 100.0 - primary_fill,
    }
}

fn build_explain_model(
    snapshot: &LiveSnapshot,
    selected_day_index: usize,
    selected_event_id: Option<&str>,
    overlay_filters: &OverlayFilterState,
    selected_overlay_toggle_index: usize,
    today_review: &ReviewDeck,
) -> ExplainModel {
    let selected_day = selected_day_label(snapshot, selected_day_index)
        .unwrap_or_else(|| "no selected day".to_owned());
    let [sleep_insight, readiness_insight, activity_insight] =
        explain_metric_insights(snapshot, &selected_day);
    let selected_daily = selected_daily_row(snapshot, &selected_day);
    let heartrate = selected_heartrate_day(snapshot, &selected_day);
    let supporting_events =
        supporting_events_for_explain(snapshot, &selected_day, overlay_filters, selected_event_id);
    let summary_lines = explain_summary_lines(
        snapshot,
        &selected_day,
        selected_daily,
        [&sleep_insight, &readiness_insight, &activity_insight],
    );
    let evidence_badges = explain_evidence_badges(snapshot, selected_daily);
    let measurement_lines = measurement_lines_for_day(selected_daily, heartrate);
    let evidence_lines = explain_evidence_lines(&supporting_events);
    let caveat_lines = explain_caveat_lines(
        snapshot,
        &selected_day,
        [&sleep_insight, &readiness_insight, &activity_insight],
        selected_daily,
        heartrate,
        &supporting_events,
        today_review,
    );
    let context_lines = explain_context_lines(&supporting_events);

    ExplainModel {
        selected_day_label: selected_day.clone(),
        breadcrumb: explain_breadcrumb(&selected_day, &supporting_events),
        headline: format!("Day story for {selected_day}"),
        overlay_toggles: overlay_toggle_views(overlay_filters, selected_overlay_toggle_index),
        selected_overlay_toggle_index,
        claim_availability: telemetry_availability_for_daily_metric(
            snapshot,
            CapabilityKind::Daily,
            selected_daily.is_some(),
        ),
        summary_lines,
        measurements_availability: explain_measurements_availability(
            snapshot,
            selected_daily,
            heartrate,
        ),
        evidence_badges,
        measurement_lines,
        evidence_availability: availability_for_items(&supporting_events),
        evidence_lines,
        uncertainty_availability: availability_for_lines(&caveat_lines),
        caveat_lines,
        context_availability: availability_for_items(&supporting_events),
        context_lines,
        ai_availability: ai_launch_availability(snapshot),
        ai_actions: vec![
            "[ai] Review this day from the AI launch region.".to_owned(),
            "[ai] Open the AI workbench from Views for saved runs and reports.".to_owned(),
        ],
    }
}

const fn availability_for_lines(lines: &[String]) -> TelemetryAvailability {
    if lines.is_empty() {
        TelemetryAvailability::NoData
    } else {
        TelemetryAvailability::Fresh
    }
}

const fn availability_for_items<T>(items: &[T]) -> TelemetryAvailability {
    if items.is_empty() {
        TelemetryAvailability::NoData
    } else {
        TelemetryAvailability::Fresh
    }
}

const fn ai_launch_availability(snapshot: &LiveSnapshot) -> TelemetryAvailability {
    if snapshot.ai_ops.enabled {
        TelemetryAvailability::Fresh
    } else {
        TelemetryAvailability::NoData
    }
}

fn explain_measurements_availability(
    snapshot: &LiveSnapshot,
    selected_daily: Option<&DailyOverviewRow>,
    heartrate: Option<&HeartRateDay>,
) -> TelemetryAvailability {
    let daily = telemetry_availability_for_daily_metric(
        snapshot,
        CapabilityKind::Daily,
        selected_daily.is_some(),
    );
    let heartrate = telemetry_availability_for_daily_metric(
        snapshot,
        CapabilityKind::Daily,
        heartrate.is_some(),
    );
    combine_availability(daily, heartrate)
}

const fn combine_availability(
    primary: TelemetryAvailability,
    secondary: TelemetryAvailability,
) -> TelemetryAvailability {
    match (primary, secondary) {
        (TelemetryAvailability::Fresh, _) | (_, TelemetryAvailability::Fresh) => {
            TelemetryAvailability::Fresh
        }
        (TelemetryAvailability::Stale, _) | (_, TelemetryAvailability::Stale) => {
            TelemetryAvailability::Stale
        }
        (TelemetryAvailability::RateLimited, _) | (_, TelemetryAvailability::RateLimited) => {
            TelemetryAvailability::RateLimited
        }
        (TelemetryAvailability::Error, _) | (_, TelemetryAvailability::Error) => {
            TelemetryAvailability::Error
        }
        (TelemetryAvailability::MissingScope, _) | (_, TelemetryAvailability::MissingScope) => {
            TelemetryAvailability::MissingScope
        }
        (TelemetryAvailability::Unsupported, _) | (_, TelemetryAvailability::Unsupported) => {
            TelemetryAvailability::Unsupported
        }
        _ => TelemetryAvailability::NoData,
    }
}

fn explain_metric_insights(snapshot: &LiveSnapshot, selected_day: &str) -> [MetricInsight; 3] {
    [
        build_day_metric_insight(snapshot, selected_day, "sleep", |row| {
            row.sleep_score.map(f64::from)
        }),
        build_day_metric_insight(snapshot, selected_day, "readiness", |row| {
            row.readiness_score.map(f64::from)
        }),
        build_day_metric_insight(snapshot, selected_day, "activity", |row| {
            row.activity_score.map(f64::from)
        }),
    ]
}

fn explain_summary_lines(
    snapshot: &LiveSnapshot,
    selected_day: &str,
    selected_daily: Option<&DailyOverviewRow>,
    insights: [&MetricInsight; 3],
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(guidance_line) = selected_daily
        .and_then(|row| row.sleep_duration_seconds.map(crate::numeric::i64_to_f64))
        .map(|seconds| seconds / 3600.0)
        .and_then(|hours| {
            guidance_comparison_text(
                "sleep_duration",
                snapshot.active_population_profile,
                Some(hours),
            )
        })
        .map(|guidance| guidance.summary)
    {
        lines.push(guidance_line);
    }
    lines.push(selected_day_baseline_sentence(
        "Sleep",
        selected_day,
        insights[0],
    ));
    lines.push(selected_day_baseline_sentence(
        "Readiness",
        selected_day,
        insights[1],
    ));
    lines.push(selected_day_baseline_sentence(
        "Activity",
        selected_day,
        insights[2],
    ));
    lines
}

fn explain_evidence_badges(
    snapshot: &LiveSnapshot,
    selected_daily: Option<&DailyOverviewRow>,
) -> Vec<String> {
    let mut badges = Vec::new();
    if selected_daily.is_some_and(|row| row.sleep_duration_seconds.is_some()) {
        badges.extend(evidence_badges(
            "sleep_duration",
            snapshot.active_population_profile,
        ));
    }
    badges.extend(evidence_badges(
        "sleep_score",
        snapshot.active_population_profile,
    ));
    badges.extend(evidence_badges(
        "readiness_score",
        snapshot.active_population_profile,
    ));
    badges.extend(evidence_badges(
        "activity_score",
        snapshot.active_population_profile,
    ));
    dedupe_preserving_order(badges)
}

fn dedupe_preserving_order(lines: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for line in lines {
        if seen.insert(line.clone()) {
            deduped.push(line);
        }
    }
    deduped
}

fn explain_caveat_lines(
    snapshot: &LiveSnapshot,
    selected_day: &str,
    insights: [&MetricInsight; 3],
    selected_daily: Option<&DailyOverviewRow>,
    heartrate: Option<&HeartRateDay>,
    supporting_events: &[ExplainSupportingEvent],
    today_review: &ReviewDeck,
) -> Vec<String> {
    let mut caveat_lines = missing_scope_messages(&snapshot.auth_status.capability_report);
    if insights.iter().any(|insight| insight_is_thin(insight)) {
        caveat_lines.push(
            "Baseline comparisons are still tentative because local history is thin for this day."
                .to_owned(),
        );
    }
    if selected_daily.is_none() {
        caveat_lines.push(format!(
            "No daily closeout has been cached for {selected_day} yet."
        ));
    }
    if heartrate.is_none() {
        caveat_lines.push(format!(
            "No heartrate samples are cached for {selected_day} yet."
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
        caveat_lines.push(format!("Today's review also flagged: {}", card.headline));
    }
    for claim_key in [
        "sleep_duration",
        "sleep_score",
        "readiness_score",
        "activity_score",
    ] {
        if let Some(spec) = claim_language_spec(claim_key, snapshot.active_population_profile) {
            caveat_lines.extend(spec.disclaimer_lines);
        }
    }
    dedupe_preserving_order(caveat_lines)
}

fn explain_evidence_lines(supporting_events: &[ExplainSupportingEvent]) -> Vec<String> {
    if supporting_events.is_empty() {
        vec!["Evidence is still sparse for this day.".to_owned()]
    } else {
        supporting_events
            .iter()
            .map(|event| {
                if event.carried_forward {
                    format!(
                        "{} carryover from {}: {}.",
                        event.family_label, event.source_day, event.headline
                    )
                } else {
                    format!("{} {}.", event.family_label, event.headline)
                }
            })
            .collect()
    }
}

fn explain_context_lines(supporting_events: &[ExplainSupportingEvent]) -> Vec<String> {
    if supporting_events.is_empty() {
        vec!["Open Timeline after a sync to inspect raw context entries.".to_owned()]
    } else {
        let mut lines = supporting_events
            .iter()
            .map(|event| {
                let breadcrumb = if event.carried_forward {
                    format!("Carryover from {}: ", event.source_day)
                } else {
                    String::new()
                };
                if event.selected {
                    format!("> {}{} ({})", breadcrumb, event.headline, event.detail)
                } else {
                    format!("  {}{} ({})", breadcrumb, event.headline, event.detail)
                }
            })
            .collect::<Vec<_>>();
        lines.push(
            "Move to Views and activate Timeline to inspect the same selected event.".to_owned(),
        );
        lines
    }
}

fn build_patterns_model(
    snapshot: &LiveSnapshot,
    overlay_filters: &OverlayFilterState,
    selected_overlay_toggle_index: usize,
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
        metric_filters: PatternMetricFilter::ALL
            .into_iter()
            .map(|filter| PatternFilterTab {
                label: filter.short_label(),
                selected: filter == metric_filter,
            })
            .collect(),
        selected_filter_index: metric_filter.index(),
        overlay_toggles: overlay_toggle_views(overlay_filters, selected_overlay_toggle_index),
        selected_overlay_toggle_index,
        filter_summary: format!(
            "Families {} | metric {}",
            overlay_filters.summary(),
            metric_filter.label()
        ),
        findings_availability: availability_for_items(&rows),
        rows,
        guide_availability: TelemetryAvailability::Fresh,
        notes: vec![
            "Patterns are descriptive associations, not causal claims.".to_owned(),
            "Every row on this screen is exploratory and trend-only by design.".to_owned(),
            "Rows appear after at least 3 comparable days; same-night sleep refers to the following closeout day.".to_owned(),
        ],
        interpretation_availability: TelemetryAvailability::Fresh,
        empty_message:
            "Not enough data yet. Patterns appear after at least 3 comparable days.".to_owned(),
        ai_actions: vec![
            "[ai] Compare this week with the previous week from the AI launch region."
                .to_owned(),
            "[ai] Open the AI workbench from Views for saved runs and reports.".to_owned(),
        ],
    }
}

fn build_ops_model(snapshot: &LiveSnapshot, refresh_in_flight: bool) -> OpsModel {
    let family_statuses = build_ops_family_statuses(snapshot);
    let queue_oldest = ops_queue_oldest(snapshot);
    let recent_failures = ops_recent_failures(snapshot);
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
    let warnings = build_ops_warning_lines(snapshot, refresh_in_flight, &family_statuses);

    OpsModel {
        mode_label: ops_runtime_mode(snapshot),
        summary_lines: build_ops_summary_lines(snapshot, &queue_oldest, recent_failures),
        coverage: coverage_cell_views(snapshot),
        family_statuses,
        items: build_ops_items(
            snapshot,
            &last_accepted_delivery,
            &last_rejected_delivery,
            &queue_oldest,
            queue_depth,
            recent_failures,
        ),
        warnings,
    }
}

fn build_ops_family_statuses(snapshot: &LiveSnapshot) -> Vec<FamilyStatusView> {
    [
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
    .collect()
}

fn ops_queue_oldest(snapshot: &LiveSnapshot) -> String {
    snapshot
        .webhook
        .pending_invalidations
        .iter()
        .map(|record| record.first_queued_at.as_str())
        .min()
        .unwrap_or("n/a")
        .to_owned()
}

fn ops_recent_failures(snapshot: &LiveSnapshot) -> usize {
    snapshot
        .webhook
        .recent_processing_attempts
        .iter()
        .filter(|attempt| attempt.outcome == "failed")
        .count()
}

fn build_ops_summary_lines(
    snapshot: &LiveSnapshot,
    queue_oldest: &str,
    recent_failures: usize,
) -> Vec<String> {
    let mut summary_lines = vec![
        format!("Mode: {}", ops_runtime_mode(snapshot)),
        format!("Receiver: {}", receiver_status_line(snapshot)),
        format!(
            "Queue: pending={} oldest={} failed_attempts={}",
            snapshot.webhook.pending_invalidations.len(),
            queue_oldest,
            recent_failures
        ),
        format!(
            "AI: enabled={} key_ready={} model={}",
            yes_no(snapshot.ai_ops.enabled),
            yes_no(snapshot.ai_ops.api_key_ready),
            snapshot.ai_ops.default_model
        ),
        format!(
            "Evidence: registry={} status={}",
            snapshot.evidence_registry_version,
            evidence_review_status_line(snapshot)
        ),
    ];
    if let Some(summary) = latest_eval_health_summary(&snapshot.ai_eval_runs) {
        summary_lines.push(format!(
            "Latest eval: {} | {} | failed_cases={} regressions={} improvements={}",
            summary.created_at,
            summary.labels,
            summary.failed_cases,
            summary.regression_count,
            summary.improvement_count
        ));
    }
    summary_lines
}

fn build_ops_warning_lines(
    snapshot: &LiveSnapshot,
    refresh_in_flight: bool,
    family_statuses: &[FamilyStatusView],
) -> Vec<String> {
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
    if snapshot.ai_ops.enabled && !snapshot.ai_ops.api_key_ready {
        warnings.push(format!(
            "AI is enabled but `{}` is missing, so launches will stop at readiness checks.",
            snapshot.ai_ops.api_key_env
        ));
    }
    if snapshot.ai_eval_runs.is_empty() {
        warnings.push(
            "No persisted eval runs yet. Use `ringmaster ai eval --fixture-dir ...` to populate the local regression console."
                .to_owned(),
        );
    } else if let Some(eval_summary) = latest_eval_health_summary(&snapshot.ai_eval_runs)
        && (eval_summary.failed_cases > 0 || eval_summary.regression_count > 0)
    {
        warnings.push(format!(
            "Latest eval needs attention: {} failed case(s) and {} regression(s).",
            eval_summary.failed_cases, eval_summary.regression_count
        ));
    }
    if !snapshot.stale_evidence_entries.is_empty() {
        warnings.push(format!(
            "Evidence registry review needs attention: {}.",
            evidence_review_status_line(snapshot)
        ));
        warnings.extend(snapshot.stale_evidence_entries.iter().cloned());
    }
    warnings.extend(recent_health_incidents(snapshot));
    warnings
}

fn build_ops_items(
    snapshot: &LiveSnapshot,
    last_accepted_delivery: &str,
    last_rejected_delivery: &str,
    queue_oldest: &str,
    queue_depth: usize,
    recent_failures: usize,
) -> Vec<OpsItem> {
    let mut items = build_ops_core_items(snapshot);
    items.extend(build_ops_ai_items(snapshot));
    items.extend(build_ops_webhook_items(
        snapshot,
        last_accepted_delivery,
        last_rejected_delivery,
        queue_oldest,
        queue_depth,
        recent_failures,
    ));
    items.extend(build_ops_refresh_items(snapshot));
    items
}

fn build_ops_core_items(snapshot: &LiveSnapshot) -> Vec<OpsItem> {
    vec![
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
                    snapshot.webhook.signature_tolerance_secs, snapshot.webhook.renewal_lead_secs
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
            "Evidence registry",
            snapshot.evidence_registry_version.clone(),
        ),
        ops_item(
            "Evidence review status",
            evidence_review_status_line(snapshot),
        ),
    ]
}

fn build_ops_ai_items(snapshot: &LiveSnapshot) -> Vec<OpsItem> {
    vec![
        ops_item(
            "AI provider",
            format!(
                "{} | enabled={} | key_ready={}",
                snapshot.ai_ops.provider,
                yes_no(snapshot.ai_ops.enabled),
                yes_no(snapshot.ai_ops.api_key_ready)
            ),
        ),
        ops_item("AI default model", snapshot.ai_ops.default_model.clone()),
        ops_item(
            "AI request mode",
            format!(
                "{} | transport={} | prompt_cache={}",
                snapshot.ai_ops.request_mode,
                snapshot.ai_ops.input_transport,
                snapshot.ai_ops.prompt_cache
            ),
        ),
        ops_item(
            "Guidance profile",
            format!(
                "{} | source={}",
                snapshot.active_population_profile.label(),
                snapshot.guidance_profile_source,
            ),
        ),
        ops_item(
            "AI prompt/schema",
            format!(
                "review={} | compare={}",
                snapshot.ai_ops.review_prompt_version, snapshot.ai_ops.compare_prompt_version
            ),
        ),
        ops_item(
            "Latest eval",
            latest_eval_health_summary(&snapshot.ai_eval_runs).map_or_else(
                || "none".to_owned(),
                |summary| format!("{} | {}", summary.created_at, summary.labels),
            ),
        ),
        ops_item(
            "Eval health",
            latest_eval_health_summary(&snapshot.ai_eval_runs).map_or_else(
                || "no eval history".to_owned(),
                |summary| {
                    format!(
                        "failed_cases={} regressions={} improvements={}",
                        summary.failed_cases, summary.regression_count, summary.improvement_count
                    )
                },
            ),
        ),
        ops_item(
            "AI last successful run",
            snapshot
                .ai_ops
                .last_successful_run
                .clone()
                .unwrap_or_else(|| "none".to_owned()),
        ),
        ops_item(
            "AI last failed run",
            snapshot
                .ai_ops
                .last_failed_run
                .clone()
                .unwrap_or_else(|| "none".to_owned()),
        ),
        ops_item(
            "Artifact registry",
            format!(
                "snapshots={} runs={} artifacts={} reports={} evals={}",
                snapshot.ai_ops.snapshot_catalog_count,
                snapshot.ai_ops.ai_run_count,
                snapshot.ai_ops.ai_artifact_count,
                snapshot.ai_ops.report_export_count,
                snapshot.ai_ops.ai_eval_run_count,
            ),
        ),
    ]
}

fn build_ops_webhook_items(
    snapshot: &LiveSnapshot,
    last_accepted_delivery: &str,
    last_rejected_delivery: &str,
    queue_oldest: &str,
    queue_depth: usize,
    recent_failures: usize,
) -> Vec<OpsItem> {
    vec![
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
        ops_item("Last accepted delivery", last_accepted_delivery.to_owned()),
        ops_item("Last rejected delivery", last_rejected_delivery.to_owned()),
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
    ]
}

fn build_ops_refresh_items(snapshot: &LiveSnapshot) -> Vec<OpsItem> {
    vec![
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
    ]
}

fn build_review_model(
    snapshot: &LiveSnapshot,
    today_review: &ReviewDeck,
    week_review: &ReviewDeck,
    investigation: &InvestigationReport,
    context: &ReviewViewContext<'_>,
) -> ReviewModel {
    let cards = review_cards_for_mode(
        context.review_mode,
        today_review,
        week_review,
        investigation,
    );
    let selected_card_index = if cards.is_empty() {
        None
    } else {
        Some(usize::min(
            context.selected_review_card_index,
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
            badges: review_card_badges(card, snapshot.active_population_profile),
            selected: selected_card_index == Some(index),
        })
        .collect::<Vec<_>>();
    let detail_lines = review_detail_lines(
        context.review_mode,
        selected_card_index.and_then(|index| cards.get(index).copied()),
        investigation,
        snapshot.active_population_profile,
    );
    let warning_lines = review_warning_lines(
        context.review_mode,
        today_review,
        week_review,
        investigation,
    );

    ReviewModel {
        selected_day_label: context.selected_day.to_owned(),
        breadcrumb: format!(
            "Day {} -> {} mode -> focus {}",
            context.selected_day,
            context.review_mode.label(),
            context.review_focus.label()
        ),
        mode_tabs: [
            ReviewScreenMode::Today,
            ReviewScreenMode::Week,
            ReviewScreenMode::Investigate,
        ]
        .into_iter()
        .map(|mode| ReviewTab {
            label: mode.label().to_owned(),
            selected: mode == context.review_mode,
        })
        .collect(),
        selected_mode_index: context.review_mode.index(),
        focus_tabs: ReviewFocus::ALL
            .into_iter()
            .map(|focus| ReviewTab {
                label: focus.label().to_owned(),
                selected: focus == context.review_focus,
            })
            .collect(),
        selected_focus_index: ReviewFocus::ALL
            .iter()
            .position(|focus| *focus == context.review_focus)
            .unwrap_or_default(),
        cards_availability: availability_for_items(&card_views),
        cards: card_views,
        selected_card_index,
        ai_artifact: context.ai_artifact.clone(),
        detail_availability: availability_for_lines(&detail_lines),
        detail_lines,
        warnings_availability: availability_for_lines(&warning_lines),
        warning_lines,
        empty_message: review_empty_message(context.review_mode, context.review_focus),
        ai_actions: vec![
            "[ai] Review this day from the AI launch region.".to_owned(),
            "[ai] Compare this week with the previous week from the AI launch region.".to_owned(),
            "[ai] Open the AI workbench from Views for saved runs and reports.".to_owned(),
        ],
    }
}

fn build_ai_workbench_model(
    snapshot: &LiveSnapshot,
    options: &LiveModelOptions,
) -> AiWorkbenchModel {
    let selected_day = selected_day_label(snapshot, options.selected_day_index)
        .unwrap_or_else(|| latest_review_anchor_day(snapshot));
    let launch_points = build_ai_launch_points(&selected_day, options.selected_ai_launch_index);
    let browser_content = build_ai_browser_content(snapshot, options);
    let preflight = options
        .ai_preflight
        .as_ref()
        .map(|preflight| build_ai_preflight_view(preflight, options.ai_preflight_control));

    AiWorkbenchModel {
        headline: format!("AI workbench for {selected_day}"),
        summary_lines: build_ai_workbench_summary_lines(snapshot),
        launch_points,
        browser_tabs: build_ai_browser_tabs(snapshot, options.ai_browser_tab),
        selected_tab_index: options.ai_browser_tab.index(),
        browser_items: browser_content.browser_items,
        selected_item_index: browser_content.selected_item_index,
        artifact_actions: browser_content.artifact_actions,
        selected_action_index: browser_content.selected_action_index,
        detail_title: browser_content.detail_title,
        detail_lines: browser_content.detail_lines,
        trust_lines: build_ai_workbench_trust_lines(snapshot),
        warning_lines: build_ai_workbench_warning_lines(snapshot),
        preflight,
    }
}

fn build_ai_browser_tabs(
    snapshot: &LiveSnapshot,
    selected_tab: AiBrowserTab,
) -> Vec<AiBrowserTabView> {
    [
        (AiBrowserTab::Runs, "Runs", snapshot.ai_runs.len()),
        (
            AiBrowserTab::Snapshots,
            "Snapshots",
            snapshot.snapshot_catalog.len(),
        ),
        (
            AiBrowserTab::Reports,
            "Reports",
            snapshot.report_exports.len(),
        ),
        (AiBrowserTab::Evals, "Evals", snapshot.ai_eval_runs.len()),
    ]
    .into_iter()
    .map(|(tab, label, count)| AiBrowserTabView {
        label: label.to_owned(),
        count,
        selected: selected_tab == tab,
    })
    .collect()
}

fn build_ai_workbench_summary_lines(snapshot: &LiveSnapshot) -> Vec<String> {
    let mut summary_lines = vec![
        format!(
            "Snapshot-first AI is {} and {} by default.",
            if snapshot.ai_ops.enabled {
                "available"
            } else {
                "disabled"
            },
            if snapshot.ai_ops.request_mode == AiRequestMode::Stateless.as_str() {
                "stateless"
            } else {
                "stateful"
            }
        ),
        format!(
            "Tools disabled: {} | payload path always starts from an exported snapshot artifact.",
            yes_no(snapshot.ai_ops.tools_disabled)
        ),
        format!(
            "Catalog: {} snapshots, {} runs, {} reports, {} evals.",
            snapshot.ai_ops.snapshot_catalog_count,
            snapshot.ai_ops.ai_run_count,
            snapshot.ai_ops.report_export_count,
            snapshot.ai_ops.ai_eval_run_count
        ),
    ];
    if let Some(last_successful_run) = &snapshot.ai_ops.last_successful_run {
        summary_lines.push(format!("Last successful run: {last_successful_run}"));
    }
    if let Some(eval_summary) = latest_eval_health_summary(&snapshot.ai_eval_runs) {
        summary_lines.push(format!(
            "Latest eval: {} | {} | failed_cases={} regressions={} improvements={}",
            eval_summary.created_at,
            eval_summary.labels,
            eval_summary.failed_cases,
            eval_summary.regression_count,
            eval_summary.improvement_count
        ));
    }
    summary_lines
}

fn build_ai_workbench_trust_lines(snapshot: &LiveSnapshot) -> Vec<String> {
    let mut trust_lines = vec![
        format!(
            "Provider: {} | API key ready: {}",
            snapshot.ai_ops.provider,
            yes_no(snapshot.ai_ops.api_key_ready)
        ),
        format!(
            "Model: {} | reasoning_effort: {}",
            snapshot.ai_ops.default_model, snapshot.ai_ops.reasoning_effort
        ),
        format!(
            "Request mode: {} | transport: {} | prompt cache: {}",
            snapshot.ai_ops.request_mode,
            snapshot.ai_ops.input_transport,
            snapshot.ai_ops.prompt_cache
        ),
        format!(
            "Prompt/schema: review={} compare={}",
            snapshot.ai_ops.review_prompt_version, snapshot.ai_ops.compare_prompt_version
        ),
        format!(
            "Artifacts: {} structured outputs | {} eval summaries",
            snapshot.ai_ops.ai_artifact_count, snapshot.ai_ops.ai_eval_run_count
        ),
    ];
    if let Some(eval_summary) = latest_eval_health_summary(&snapshot.ai_eval_runs) {
        trust_lines.push(format!(
            "Eval health: {} failed case(s) | {} regression(s) | {} improvement(s)",
            eval_summary.failed_cases,
            eval_summary.regression_count,
            eval_summary.improvement_count
        ));
    }
    trust_lines
}

fn build_ai_workbench_warning_lines(snapshot: &LiveSnapshot) -> Vec<String> {
    let mut warning_lines = Vec::new();
    if !snapshot.ai_ops.enabled {
        warning_lines.push(
            "Provider is disabled. The workbench remains browseable, but launches stay local-only until AI is enabled."
                .to_owned(),
        );
    }
    if snapshot.ai_ops.enabled && !snapshot.ai_ops.api_key_ready {
        warning_lines.push(format!(
            "The configured API key env `{}` is not present, so new AI runs will fail preflight.",
            snapshot.ai_ops.api_key_env
        ));
    }
    if snapshot.ai_runs.is_empty() {
        warning_lines.push(
            "No persisted AI runs yet. Launch from Review, Explain, Patterns, Dashboard, or this workbench to seed the local registry."
                .to_owned(),
        );
    }
    if let Some(last_failed_run) = &snapshot.ai_ops.last_failed_run {
        warning_lines.push(format!("Most recent failed run: {last_failed_run}"));
    }
    if snapshot.ai_eval_runs.is_empty() {
        warning_lines.push(
            "No persisted eval runs yet. Use `ringmaster ai eval --fixture-dir ...` to populate the local regression console."
                .to_owned(),
        );
    } else if let Some(eval_summary) = latest_eval_health_summary(&snapshot.ai_eval_runs)
        && (eval_summary.failed_cases > 0 || eval_summary.regression_count > 0)
    {
        warning_lines.push(format!(
            "Latest eval needs attention: {} failed case(s) and {} regression(s).",
            eval_summary.failed_cases, eval_summary.regression_count
        ));
    }
    warning_lines
}

fn build_ai_launch_points(selected_day: &str, selected_index: usize) -> Vec<AiLaunchPointView> {
    vec![
        AiLaunchPointView {
            intent: AiLaunchIntent::ReviewSelectedDay,
            label: "Review this day".to_owned(),
            detail: format!(
                "Prepare a snapshot-scoped review for day:{selected_day}, then confirm the exact payload in preflight before any upload."
            ),
            key_hint: "a".to_owned(),
            selected: selected_index == 0,
        },
        AiLaunchPointView {
            intent: AiLaunchIntent::CompareSelectedWeek,
            label: "Compare this week".to_owned(),
            detail:
                "Prepare a week-to-week compare with explicit snapshot A/B provenance and model/privacy choices."
                    .to_owned(),
            key_hint: "c".to_owned(),
            selected: selected_index == 1,
        },
    ]
}

fn build_ai_preflight_view(
    preflight: &AiPreflightState,
    selected_control: PreflightControl,
) -> AiPreflightView {
    let mut body_lines = vec![
        format!("intent: {}", preflight.intent.label()),
        format!("source: {}", preflight.source_screen.title()),
        format!("scope: {}", preflight.snapshot_scope),
        format!("privacy profile: {}", preflight.privacy_profile.as_str()),
        format!(
            "provider/model: {} / {}",
            preflight.request_preview.provider, preflight.request_preview.model
        ),
        format!(
            "request mode: {} | stateless: {} | tools disabled: {}",
            preflight.request_preview.request_mode,
            yes_no(preflight.request_preview.stateless),
            yes_no(preflight.request_preview.tools_disabled)
        ),
        format!(
            "artifact payload: {} byte(s) (~{} tokens)",
            preflight.request_preview.snapshot_bytes,
            preflight.request_preview.approximate_input_tokens
        ),
        format!(
            "notes/free-text included: {}",
            yes_no(preflight.request_preview.includes_notes_or_free_text)
        ),
        format!(
            "content classes: {}",
            preflight.request_preview.content_classes.join(", ")
        ),
    ];
    if let Some(model_override) = &preflight.model_override {
        body_lines.push(format!("model override: {model_override}"));
    }
    if let Some(follow_up_kind) = preflight.follow_up_kind {
        body_lines.push(format!("follow_up_kind: {}", follow_up_kind.as_str()));
    }
    if !preflight.snapshot_paths.is_empty() {
        body_lines.push("artifact paths:".to_owned());
        body_lines.extend(
            preflight
                .snapshot_paths
                .iter()
                .map(|path| format!("  - {path}")),
        );
    }
    body_lines.push(
        "Tab moves focus | Left/Right selects control | Enter activates | Esc closes".to_owned(),
    );

    AiPreflightView {
        title: format!("Preflight | {}", preflight.intent.short_label()),
        body_lines,
        warning_lines: preflight.warning_lines.clone(),
        controls: ai_preflight_controls(preflight.confirm_enabled, selected_control),
        selected_control_index: preflight_control_index(selected_control),
        confirm_enabled: preflight.confirm_enabled,
    }
}

fn ai_preflight_controls(
    confirm_enabled: bool,
    selected_control: PreflightControl,
) -> Vec<AiPreflightControlView> {
    PreflightControl::ALL
        .into_iter()
        .map(|control| AiPreflightControlView {
            label: control.label(),
            detail: match control {
                PreflightControl::Confirm if confirm_enabled => {
                    "Send the prepared snapshot-bounded request.".to_owned()
                }
                PreflightControl::Confirm => {
                    "Blocked until readiness issues are resolved.".to_owned()
                }
                PreflightControl::Privacy => {
                    "Cycle the privacy profile before confirming.".to_owned()
                }
                PreflightControl::Cancel => {
                    "Dismiss this preflight without sending anything.".to_owned()
                }
            },
            selected: control == selected_control,
        })
        .collect()
}

const fn preflight_control_index(control: PreflightControl) -> usize {
    match control {
        PreflightControl::Confirm => 0,
        PreflightControl::Privacy => 1,
        PreflightControl::Cancel => 2,
    }
}

fn build_ai_browser_content(
    snapshot: &LiveSnapshot,
    options: &LiveModelOptions,
) -> AiBrowserContent {
    let (browser_items, selected_item_index, detail_title, detail_lines) =
        match options.ai_browser_tab {
            AiBrowserTab::Runs => build_ai_run_browser(snapshot, options.selected_ai_run_index),
            AiBrowserTab::Snapshots => {
                build_snapshot_browser(snapshot, options.selected_snapshot_catalog_index)
            }
            AiBrowserTab::Reports => {
                build_report_browser(snapshot, options.selected_report_export_index)
            }
            AiBrowserTab::Evals => build_eval_browser(snapshot, options.selected_ai_eval_run_index),
        };
    let action_kinds = ai_artifact_action_kinds(
        options.ai_browser_tab,
        selected_item_index.and_then(|index| snapshot.ai_runs.get(index)),
        selected_item_index.and_then(|index| snapshot.snapshot_catalog.get(index)),
        selected_item_index.and_then(|index| snapshot.report_exports.get(index)),
        selected_item_index.and_then(|index| snapshot.ai_eval_runs.get(index)),
    );
    let selected_action_index = if action_kinds.is_empty() {
        None
    } else {
        Some(usize::min(
            options.selected_ai_artifact_action_index,
            action_kinds.len().saturating_sub(1),
        ))
    };
    let artifact_actions = action_kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| AiArtifactActionView {
            label: kind.label().to_owned(),
            detail: kind.detail().to_owned(),
            selected: selected_action_index == Some(index),
        })
        .collect::<Vec<_>>();

    AiBrowserContent {
        browser_items,
        selected_item_index,
        artifact_actions,
        selected_action_index,
        detail_title,
        detail_lines,
    }
}

fn ai_artifact_action_kinds(
    tab: AiBrowserTab,
    selected_run: Option<&AiRunRecord>,
    selected_snapshot: Option<&SnapshotCatalogEntry>,
    _selected_report: Option<&ReportExportRecord>,
    selected_eval: Option<&AiEvalRunRecord>,
) -> Vec<AiArtifactActionKind> {
    match tab {
        AiBrowserTab::Runs => selected_run.map_or_else(Vec::new, |run| {
            let mut actions = Vec::new();
            if ai_run_is_cancellable(run) {
                actions.push(AiArtifactActionKind::CancelRun);
            }
            if ai_run_has_follow_up_source(run) {
                actions.extend([
                    AiArtifactActionKind::ExpandEvidence,
                    AiArtifactActionKind::ShowCounterevidence,
                    AiArtifactActionKind::ExplainRanking,
                    AiArtifactActionKind::SuggestDrilldown,
                ]);
            }
            if run.artifact_id.is_some() {
                actions.push(AiArtifactActionKind::GenerateReport);
            }
            actions.push(AiArtifactActionKind::RerunNextPrivacy);
            actions.push(AiArtifactActionKind::RerunNextModel);
            actions.push(AiArtifactActionKind::ComparePreviousSnapshot);
            if run.artifact_id.is_some() {
                actions.push(AiArtifactActionKind::OpenLinkedEvidence);
            }
            actions
        }),
        AiBrowserTab::Snapshots => selected_snapshot.map_or_else(Vec::new, |_| {
            vec![
                AiArtifactActionKind::ComparePreviousSnapshot,
                AiArtifactActionKind::GenerateReport,
            ]
        }),
        AiBrowserTab::Reports => Vec::new(),
        AiBrowserTab::Evals => selected_eval.map_or_else(Vec::new, |eval| {
            if eval_has_linked_evidence(eval) {
                vec![AiArtifactActionKind::OpenLinkedEvidence]
            } else {
                Vec::new()
            }
        }),
    }
}

fn ai_run_is_cancellable(run: &AiRunRecord) -> bool {
    matches!(run.run_status.as_str(), "queued" | "running")
}

fn ai_run_has_follow_up_source(run: &AiRunRecord) -> bool {
    if run.run_kind == "follow_up" {
        run.source_ai_artifact_id.is_some() || run.artifact_id.is_some()
    } else {
        run.artifact_id.is_some()
    }
}

fn eval_has_linked_evidence(eval: &AiEvalRunRecord) -> bool {
    let Some(details) = parse_persisted_eval_details(&eval.details_json) else {
        return false;
    };
    details.cases.iter().any(|case| {
        case.snapshot_hash_a.is_some()
            || case.snapshot_hash_b.is_some()
            || case.candidate.lineage.ai_run_id.is_some()
            || case.candidate.lineage.ai_artifact_id.is_some()
            || case.candidate.lineage.report_id.is_some()
            || case.baseline.as_ref().is_some_and(|baseline| {
                baseline.lineage.ai_run_id.is_some()
                    || baseline.lineage.ai_artifact_id.is_some()
                    || baseline.lineage.report_id.is_some()
            })
    })
}

fn build_ai_run_browser(
    snapshot: &LiveSnapshot,
    selected_index: usize,
) -> (Vec<AiBrowserItemView>, Option<usize>, String, Vec<String>) {
    let selected_item_index = clamp_selected_index(selected_index, snapshot.ai_runs.len());
    let browser_items = build_ai_run_browser_items(&snapshot.ai_runs, selected_item_index);

    let Some(selected_item_index) = selected_item_index else {
        return (
            browser_items,
            None,
            "Saved AI runs".to_owned(),
            vec!["No persisted AI runs yet.".to_owned()],
        );
    };
    let run = &snapshot.ai_runs[selected_item_index];
    (
        browser_items,
        Some(selected_item_index),
        "Saved AI run".to_owned(),
        ai_run_detail_lines(snapshot, run),
    )
}

fn build_ai_run_browser_items(
    ai_runs: &[AiRunRecord],
    selected_item_index: Option<usize>,
) -> Vec<AiBrowserItemView> {
    ai_runs
        .iter()
        .enumerate()
        .map(|(index, run)| AiBrowserItemView {
            headline: format!(
                "{} | {} | {}",
                run.run_kind,
                run.run_status,
                abbreviate_id(&run.run_id, 12)
            ),
            detail: format!(
                "{} | {} | {}",
                run.created_at, run.privacy_profile, run.snapshot_scope
            ),
            status_badge: run.run_status.clone(),
            selected: selected_item_index == Some(index),
        })
        .collect()
}

fn ai_run_detail_lines(snapshot: &LiveSnapshot, run: &AiRunRecord) -> Vec<String> {
    let mut detail_lines = ai_run_metadata_lines(run);
    if let Ok(preview) = serde_json::from_str::<AiRequestPreview>(&run.request_preview_json) {
        detail_lines.push(String::new());
        detail_lines.push("request preview:".to_owned());
        detail_lines.extend(ai_request_preview_lines(&preview));
    }
    if let Some(artifact_id) = &run.artifact_id {
        detail_lines.extend(ai_run_artifact_lines(snapshot, artifact_id));
    }
    detail_lines
}

fn ai_run_metadata_lines(run: &AiRunRecord) -> Vec<String> {
    let mut detail_lines = vec![
        format!("run_id: {}", run.run_id),
        format!("kind: {} | status: {}", run.run_kind, run.run_status),
        format!("provider/model: {} / {}", run.provider, run.model),
        format!(
            "privacy/profile: {} | scope: {}",
            run.privacy_profile, run.snapshot_scope
        ),
        format!(
            "request mode: {} | transport: {} | run mode: {}",
            run.request_mode, run.input_transport, run.run_mode
        ),
        format!(
            "prompt/schema: {} / {}",
            run.prompt_version, run.output_schema_version
        ),
        format!("snapshot_a: {}", run.snapshot_hash_a),
    ];
    if let Some(snapshot_hash_b) = &run.snapshot_hash_b {
        detail_lines.push(format!("snapshot_b: {snapshot_hash_b}"));
    }
    if let Some(source_ai_artifact_id) = &run.source_ai_artifact_id {
        detail_lines.push(format!("source_run: {source_ai_artifact_id}"));
    }
    if let Some(follow_up_kind) = &run.follow_up_kind {
        detail_lines.push(format!("follow_up_kind: {follow_up_kind}"));
    }
    if let Some(request_fingerprint) = &run.request_fingerprint {
        detail_lines.push(format!(
            "request_fingerprint: {}",
            abbreviate_id(request_fingerprint, 16)
        ));
    }
    detail_lines.push(format!("created_at: {}", run.created_at));
    if let Some(started_at) = &run.started_at {
        detail_lines.push(format!("started_at: {started_at}"));
    }
    if let Some(ended_at) = &run.ended_at {
        detail_lines.push(format!("ended_at: {ended_at}"));
    }
    if let Some(error_message) = &run.error_message {
        detail_lines.push(String::new());
        detail_lines.push(format!("error: {error_message}"));
    }
    detail_lines
}

fn ai_run_artifact_lines(snapshot: &LiveSnapshot, artifact_id: &str) -> Vec<String> {
    let mut detail_lines = vec![String::new(), format!("linked_artifact: {artifact_id}")];
    if let Some(artifact_record) = snapshot
        .ai_artifact_records
        .iter()
        .find(|record| record.artifact_id == artifact_id)
    {
        detail_lines.extend(ai_artifact_detail_lines(artifact_record));
    }
    let linked_reports = snapshot
        .report_exports
        .iter()
        .filter(|record| record.source_ai_artifact_id.as_deref() == Some(artifact_id))
        .collect::<Vec<_>>();
    if !linked_reports.is_empty() {
        detail_lines.push(String::new());
        detail_lines.push("linked_reports:".to_owned());
        detail_lines.extend(linked_reports.iter().map(|report| {
            format!(
                "  - {} | {} | {}",
                report.title, report.format, report.output_path
            )
        }));
    }
    detail_lines
}

fn build_snapshot_browser(
    snapshot: &LiveSnapshot,
    selected_index: usize,
) -> (Vec<AiBrowserItemView>, Option<usize>, String, Vec<String>) {
    let selected_item_index = if snapshot.snapshot_catalog.is_empty() {
        None
    } else {
        Some(usize::min(
            selected_index,
            snapshot.snapshot_catalog.len().saturating_sub(1),
        ))
    };
    let browser_items = snapshot
        .snapshot_catalog
        .iter()
        .enumerate()
        .map(|(index, record)| AiBrowserItemView {
            headline: format!(
                "{} | {}",
                record.scope,
                abbreviate_id(&record.snapshot_hash, 12)
            ),
            detail: format!(
                "{} | {} | {} day(s)",
                record.generated_at, record.privacy_profile, record.day_count
            ),
            status_badge: record.freshness_summary.clone(),
            selected: selected_item_index == Some(index),
        })
        .collect::<Vec<_>>();

    let Some(selected_item_index) = selected_item_index else {
        return (
            browser_items,
            None,
            "Snapshot catalog".to_owned(),
            vec!["No snapshot exports are cataloged yet.".to_owned()],
        );
    };
    let record = &snapshot.snapshot_catalog[selected_item_index];
    let run_count = snapshot
        .ai_runs
        .iter()
        .filter(|run| {
            run.snapshot_hash_a == record.snapshot_hash
                || run.snapshot_hash_b.as_deref() == Some(record.snapshot_hash.as_str())
        })
        .count();
    let report_count = snapshot
        .report_exports
        .iter()
        .filter(|report| {
            report.source_snapshot_hash_a.as_deref() == Some(record.snapshot_hash.as_str())
                || report.source_snapshot_hash_b.as_deref() == Some(record.snapshot_hash.as_str())
        })
        .count();

    (
        browser_items,
        Some(selected_item_index),
        "Snapshot artifact".to_owned(),
        vec![
            format!("snapshot_hash: {}", record.snapshot_hash),
            format!("scope: {}", record.scope),
            format!(
                "day_range: {} -> {} (anchor {})",
                record.start_day, record.end_day, record.anchor_day
            ),
            format!("privacy_profile: {}", record.privacy_profile),
            format!("source_mode: {}", record.source_mode),
            format!("freshness: {}", record.freshness_summary),
            format!("trust: {}", record.trust_summary),
            format!("capabilities: {}", record.capability_summary),
            format!("provenance: {}", record.provenance_summary),
            format!(
                "linked_runs: {} | linked_reports: {}",
                run_count, report_count
            ),
        ],
    )
}

fn build_report_browser(
    snapshot: &LiveSnapshot,
    selected_index: usize,
) -> (Vec<AiBrowserItemView>, Option<usize>, String, Vec<String>) {
    let selected_item_index = if snapshot.report_exports.is_empty() {
        None
    } else {
        Some(usize::min(
            selected_index,
            snapshot.report_exports.len().saturating_sub(1),
        ))
    };
    let browser_items = snapshot
        .report_exports
        .iter()
        .enumerate()
        .map(|(index, report)| AiBrowserItemView {
            headline: format!("{} | {}", report.report_kind, report.title),
            detail: format!(
                "{} | {} | {}",
                report.created_at, report.format, report.output_path
            ),
            status_badge: report.export_status.clone(),
            selected: selected_item_index == Some(index),
        })
        .collect::<Vec<_>>();

    let Some(selected_item_index) = selected_item_index else {
        return (
            browser_items,
            None,
            "Exported reports".to_owned(),
            vec!["No report exports are cataloged yet.".to_owned()],
        );
    };
    let report = &snapshot.report_exports[selected_item_index];
    let mut detail_lines = vec![
        format!("report_id: {}", report.report_id),
        format!("title: {}", report.title),
        format!("kind/format: {} / {}", report.report_kind, report.format),
        format!("status: {}", report.export_status),
        format!("output_path: {}", report.output_path),
        format!("privacy_profile: {}", report.privacy_profile),
        format!("content_hash: {}", abbreviate_id(&report.content_hash, 16)),
        format!(
            "last_verified: {} ({})",
            yes_no(report.last_verified_exists),
            report.last_verified_at
        ),
    ];
    if let Some(source_snapshot_hash_a) = &report.source_snapshot_hash_a {
        detail_lines.push(format!("source_snapshot_a: {source_snapshot_hash_a}"));
    }
    if let Some(source_snapshot_hash_b) = &report.source_snapshot_hash_b {
        detail_lines.push(format!("source_snapshot_b: {source_snapshot_hash_b}"));
    }
    if let Some(source_ai_artifact_id) = &report.source_ai_artifact_id {
        detail_lines.push(format!("source_ai_artifact: {source_ai_artifact_id}"));
    }
    if let Some(provider) = &report.provider {
        detail_lines.push(format!("provider: {provider}"));
    }
    if let Some(model) = &report.model {
        detail_lines.push(format!("model: {model}"));
    }
    if let Some(prompt_version) = &report.prompt_version {
        detail_lines.push(format!("prompt_version: {prompt_version}"));
    }
    if let Some(output_schema_version) = &report.output_schema_version {
        detail_lines.push(format!("output_schema_version: {output_schema_version}"));
    }

    (
        browser_items,
        Some(selected_item_index),
        "Report export".to_owned(),
        detail_lines,
    )
}

fn build_eval_browser(
    snapshot: &LiveSnapshot,
    selected_index: usize,
) -> (Vec<AiBrowserItemView>, Option<usize>, String, Vec<String>) {
    let selected_item_index = if snapshot.ai_eval_runs.is_empty() {
        None
    } else {
        Some(usize::min(
            selected_index,
            snapshot.ai_eval_runs.len().saturating_sub(1),
        ))
    };
    let browser_items = snapshot
        .ai_eval_runs
        .iter()
        .enumerate()
        .map(|(index, record)| {
            let labels = eval_labels(record);
            AiBrowserItemView {
                headline: format!("{} | {}", record.task_family, labels),
                detail: format!(
                    "{} | {} case(s) | {}",
                    record.created_at, record.total_cases, record.fixture_dir
                ),
                status_badge: eval_status_badge(record),
                selected: selected_item_index == Some(index),
            }
        })
        .collect::<Vec<_>>();

    let Some(selected_item_index) = selected_item_index else {
        return (
            browser_items,
            None,
            "Eval runs".to_owned(),
            vec!["No persisted eval runs yet.".to_owned()],
        );
    };
    let record = &snapshot.ai_eval_runs[selected_item_index];
    let detail_lines = parse_persisted_eval_details(&record.details_json).map_or_else(
        || {
            vec![
                format!("eval_run_id: {}", record.eval_run_id),
                format!("fixture_dir: {}", record.fixture_dir),
                format!("candidate/baseline: {}", eval_labels(record)),
                format!(
                    "cases: total={} passed={} failed={}",
                    record.total_cases, record.passed_cases, record.failed_cases
                ),
                format!("regression_summary: {}", record.regression_summary),
                "Persisted eval detail is unavailable for this run.".to_owned(),
            ]
        },
        |details| render_eval_detail_lines(snapshot, record, &details),
    );

    (
        browser_items,
        Some(selected_item_index),
        "Eval run".to_owned(),
        detail_lines,
    )
}

fn latest_eval_health_summary(ai_eval_runs: &[AiEvalRunRecord]) -> Option<EvalHealthSummary> {
    ai_eval_runs
        .iter()
        .max_by_key(|record| record.created_at.as_str())
        .map(eval_record_health_summary)
}

fn eval_record_health_summary(record: &AiEvalRunRecord) -> EvalHealthSummary {
    let (regression_count, improvement_count) = parse_persisted_eval_details(&record.details_json)
        .map_or((0, 0), |details| {
            let counts = eval_comparison_counts(&details.cases);
            (counts.regressions, counts.improvements)
        });
    EvalHealthSummary {
        created_at: record.created_at.clone(),
        labels: eval_labels(record),
        failed_cases: record.failed_cases,
        regression_count,
        improvement_count,
    }
}

fn eval_labels(record: &AiEvalRunRecord) -> String {
    record.baseline_label.as_ref().map_or_else(
        || record.candidate_label.clone(),
        |baseline| format!("{} vs {}", record.candidate_label, baseline),
    )
}

fn eval_status_badge(record: &AiEvalRunRecord) -> String {
    let health = eval_record_health_summary(record);
    if health.failed_cases > 0 {
        format!("{} fail", health.failed_cases)
    } else if health.regression_count > 0 {
        format!("{} reg", health.regression_count)
    } else {
        "pass".to_owned()
    }
}

fn eval_comparison_counts(cases: &[PersistedEvalCaseDetail]) -> EvalComparisonCounts {
    let mut counts = EvalComparisonCounts::default();
    for grader in cases.iter().flat_map(|case| &case.graders) {
        match grader.comparison.as_str() {
            "improved" => counts.improvements += 1,
            "regressed" => counts.regressions += 1,
            "candidate_only" => counts.candidate_only += 1,
            _ => counts.matched += 1,
        }
    }
    counts
}

fn render_eval_detail_lines(
    snapshot: &LiveSnapshot,
    record: &AiEvalRunRecord,
    details: &PersistedEvalRunDetails,
) -> Vec<String> {
    let comparison_counts = eval_comparison_counts(&details.cases);
    let mut lines = render_eval_overview_lines(record, details, comparison_counts);
    lines.extend(render_eval_change_lines(
        "regressions",
        &details.regressions,
    ));
    lines.extend(render_eval_change_lines(
        "improvements",
        &details.improvements,
    ));
    lines.extend(render_failing_eval_case_lines(snapshot, &details.cases));
    lines.extend(render_passing_eval_case_lines(&details.cases));
    lines
}

fn render_eval_overview_lines(
    record: &AiEvalRunRecord,
    details: &PersistedEvalRunDetails,
    comparison_counts: EvalComparisonCounts,
) -> Vec<String> {
    vec![
        format!("eval_run_id: {}", record.eval_run_id),
        format!(
            "fixture_manifest: {} | {}",
            details.fixture_dir, details.fixture_schema_version
        ),
        format!(
            "candidate/baseline: {} / {}",
            details.candidate_label,
            details
                .baseline_label
                .clone()
                .unwrap_or_else(|| "none".to_owned())
        ),
        format!(
            "cases: total={} passed={} failed={}",
            details.total_cases, details.passed_cases, details.failed_cases
        ),
        format!(
            "baseline_vs_candidate: regressions={} improvements={} matched={} candidate_only={}",
            comparison_counts.regressions,
            comparison_counts.improvements,
            comparison_counts.matched,
            comparison_counts.candidate_only
        ),
        format!(
            "score_rollup: schema={:.2} completeness={:.2} evidence={:.2} honesty={:.2}",
            details.scores.schema_validity,
            details.scores.completeness,
            details.scores.evidence,
            details.scores.honesty
        ),
        format!("regression_summary: {}", details.regression_summary),
    ]
}

fn render_eval_change_lines(title: &str, items: &[String]) -> Vec<String> {
    if items.is_empty() {
        Vec::new()
    } else {
        let mut lines = vec![format!("{title}:")];
        lines.extend(items.iter().map(|item| format!("  - {item}")));
        lines
    }
}

fn render_failing_eval_case_lines(
    snapshot: &LiveSnapshot,
    cases: &[PersistedEvalCaseDetail],
) -> Vec<String> {
    let failing_cases = cases
        .iter()
        .filter(|case| case.graders.iter().any(|grader| !grader.candidate_passed))
        .collect::<Vec<_>>();
    if failing_cases.is_empty() {
        Vec::new()
    } else {
        let mut lines = vec![String::new(), "failing_graders:".to_owned()];
        for case in failing_cases {
            lines.extend(render_failing_eval_case(snapshot, case));
        }
        lines
    }
}

fn render_failing_eval_case(
    snapshot: &LiveSnapshot,
    case: &PersistedEvalCaseDetail,
) -> Vec<String> {
    let mut lines = vec![
        format!(
            "case {} | {} | {}",
            case.case_id, case.task_family, case.candidate.label
        ),
        format!("  snapshot_a_fixture: {}", case.snapshot_a_path),
        format!("  candidate_artifact: {}", case.candidate.artifact_path),
    ];
    if let Some(snapshot_b_path) = &case.snapshot_b_path {
        lines.push(format!("  snapshot_b_fixture: {snapshot_b_path}"));
    }
    if let Some(baseline) = &case.baseline {
        lines.push(format!("  baseline_artifact: {}", baseline.artifact_path));
    }
    lines.extend(render_eval_case_link_lines(snapshot, case));
    for grader in case
        .graders
        .iter()
        .filter(|grader| !grader.candidate_passed)
    {
        lines.push(format!(
            "  - {} [{}] | candidate={} | note={}",
            grader.grader,
            grader.comparison,
            pass_fail_word(grader.candidate_passed),
            grader.candidate_note
        ));
        if let Some(baseline_passed) = grader.baseline_passed {
            lines.push(format!(
                "    baseline={} | baseline_note={}",
                pass_fail_word(baseline_passed),
                grader.baseline_note.as_deref().unwrap_or("none")
            ));
        }
    }
    lines
}

fn render_passing_eval_case_lines(cases: &[PersistedEvalCaseDetail]) -> Vec<String> {
    let passing_cases = cases
        .iter()
        .filter(|case| case.graders.iter().all(|grader| grader.candidate_passed))
        .collect::<Vec<_>>();
    if passing_cases.is_empty() {
        Vec::new()
    } else {
        let mut lines = vec![String::new(), "passing_remainder:".to_owned()];
        lines.extend(passing_cases.iter().map(|case| {
            format!(
                "  - {} | {} | {}",
                case.case_id, case.task_family, case.candidate.label
            )
        }));
        lines
    }
}

fn render_eval_case_link_lines(
    snapshot: &LiveSnapshot,
    case: &PersistedEvalCaseDetail,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(snapshot_hash_a) = &case.snapshot_hash_a {
        lines.push(format!(
            "  linked_snapshot_a: {}",
            resolve_snapshot_reference(snapshot, snapshot_hash_a)
        ));
    }
    if let Some(snapshot_hash_b) = &case.snapshot_hash_b {
        lines.push(format!(
            "  linked_snapshot_b: {}",
            resolve_snapshot_reference(snapshot, snapshot_hash_b)
        ));
    }
    lines.extend(render_eval_artifact_lineage(
        snapshot,
        "candidate",
        &case.candidate.lineage,
    ));
    if let Some(baseline) = &case.baseline {
        lines.extend(render_eval_artifact_lineage(
            snapshot,
            "baseline",
            &baseline.lineage,
        ));
    }
    if lines.is_empty() {
        lines.push("  linked_artifacts: none".to_owned());
    }
    lines
}

fn render_eval_artifact_lineage(
    snapshot: &LiveSnapshot,
    label: &str,
    lineage: &EvalArtifactLineage,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(ai_run_id) = &lineage.ai_run_id {
        lines.push(format!(
            "  {label}_run: {}",
            resolve_ai_run_reference(snapshot, ai_run_id)
        ));
    }
    if let Some(report_id) = &lineage.report_id {
        lines.push(format!(
            "  {label}_report: {}",
            resolve_report_reference(snapshot, report_id)
        ));
    }
    if let Some(ai_artifact_id) = &lineage.ai_artifact_id {
        lines.push(format!("  {label}_artifact: {ai_artifact_id}"));
    }
    lines
}

fn resolve_snapshot_reference(snapshot: &LiveSnapshot, snapshot_hash: &str) -> String {
    snapshot
        .snapshot_catalog
        .iter()
        .find(|record| record.snapshot_hash == snapshot_hash)
        .map_or_else(
            || format!("{snapshot_hash} (not in catalog)"),
            |record| {
                format!(
                    "{} ({}, {} -> {})",
                    record.snapshot_hash, record.scope, record.start_day, record.end_day
                )
            },
        )
}

fn resolve_ai_run_reference(snapshot: &LiveSnapshot, ai_run_id: &str) -> String {
    snapshot
        .ai_runs
        .iter()
        .find(|record| record.run_id == ai_run_id)
        .map_or_else(
            || format!("{ai_run_id} (not in registry)"),
            |record| {
                format!(
                    "{} ({} | {} | {})",
                    record.run_id, record.run_kind, record.run_status, record.created_at
                )
            },
        )
}

fn resolve_report_reference(snapshot: &LiveSnapshot, report_id: &str) -> String {
    snapshot
        .report_exports
        .iter()
        .find(|record| record.report_id == report_id)
        .map_or_else(
            || format!("{report_id} (not in registry)"),
            |record| {
                format!(
                    "{} ({} | {})",
                    record.report_id, record.title, record.output_path
                )
            },
        )
}

const fn pass_fail_word(passed: bool) -> &'static str {
    if passed { "pass" } else { "fail" }
}

fn ai_request_preview_lines(preview: &AiRequestPreview) -> Vec<String> {
    let mut lines = vec![
        format!("task_family: {}", preview.task_family),
        format!("provider/model: {} / {}", preview.provider, preview.model),
        format!(
            "mode: {} | transport: {} | stateless: {}",
            preview.request_mode,
            preview.input_transport,
            yes_no(preview.stateless)
        ),
        format!(
            "tools_disabled: {} | notes/free-text: {}",
            yes_no(preview.tools_disabled),
            yes_no(preview.includes_notes_or_free_text)
        ),
        format!(
            "payload size: {} bytes (~{} tokens)",
            preview.snapshot_bytes, preview.approximate_input_tokens
        ),
        format!("content classes: {}", preview.content_classes.join(", ")),
    ];
    if !preview.snapshots.is_empty() {
        lines.push("snapshots:".to_owned());
        lines.extend(preview.snapshots.iter().map(|snapshot| {
            format!(
                "  - {} | {} | {} | {} day(s) | {}",
                snapshot.label,
                snapshot.scope,
                snapshot.anchor_day,
                snapshot.day_count,
                snapshot.privacy_profile.as_str()
            )
        }));
    }
    lines
}

fn ai_artifact_detail_lines(record: &AiArtifactRecord) -> Vec<String> {
    let mut lines = vec![format!("overview: {}", record.overview)];
    if !record.summary_cache.is_empty() {
        lines.push(format!("summary: {}", record.summary_cache));
    }
    match ai::parse_stored_artifact(record) {
        Ok(StoredArtifact::Review(artifact)) => {
            lines.push(String::new());
            lines.push("review findings:".to_owned());
            lines.extend(render_structured_findings(&artifact.headline_findings));
            lines.extend(render_structured_findings(&artifact.positive_findings));
            lines.extend(render_structured_findings(&artifact.negative_findings));
            if !artifact.unresolved_questions.is_empty() {
                lines.push("unresolved_questions:".to_owned());
                lines.extend(
                    artifact
                        .unresolved_questions
                        .iter()
                        .map(|question| format!("  - {question}")),
                );
            }
            if !artifact.follow_up_targets.is_empty() {
                lines.push("guided_follow_ups:".to_owned());
                lines.extend(artifact.follow_up_targets.iter().map(|target| {
                    format!(
                        "  - {} => {} ({})",
                        target.label, target.command, target.reason
                    )
                }));
            }
        }
        Ok(StoredArtifact::Compare(artifact)) => {
            lines.push(String::new());
            lines.push("material_differences:".to_owned());
            lines.extend(render_structured_findings(&artifact.material_differences));
            if !artifact.uncertainty_warnings.is_empty() {
                lines.push("uncertainty_warnings:".to_owned());
                lines.extend(
                    artifact
                        .uncertainty_warnings
                        .iter()
                        .map(|warning| format!("  - {warning}")),
                );
            }
            if !artifact.investigation_targets.is_empty() {
                lines.push("guided_follow_ups:".to_owned());
                lines.extend(artifact.investigation_targets.iter().map(|target| {
                    format!(
                        "  - {} => {} ({})",
                        target.label, target.command, target.reason
                    )
                }));
            }
        }
        Ok(StoredArtifact::FollowUp(artifact)) => {
            lines.push(String::new());
            lines.push(format!(
                "follow_up_kind: {}",
                artifact.follow_up_kind.as_str()
            ));
            lines.extend(render_structured_findings(&artifact.focal_findings));
            if !artifact.reasoning_steps.is_empty() {
                lines.push("reasoning_steps:".to_owned());
                lines.extend(
                    artifact
                        .reasoning_steps
                        .iter()
                        .map(|step| format!("  - {step}")),
                );
            }
            if !artifact.unresolved_questions.is_empty() {
                lines.push("unresolved_questions:".to_owned());
                lines.extend(
                    artifact
                        .unresolved_questions
                        .iter()
                        .map(|question| format!("  - {question}")),
                );
            }
            if !artifact.suggested_local_targets.is_empty() {
                lines.push("guided_follow_ups:".to_owned());
                lines.extend(artifact.suggested_local_targets.iter().map(|target| {
                    format!(
                        "  - {} => {} ({})",
                        target.label, target.command, target.reason
                    )
                }));
            }
        }
        Err(error) => {
            lines.push(format!("artifact_parse_error: {error}"));
        }
    }
    lines
}

fn render_structured_findings(findings: &[ai::ArtifactFinding]) -> Vec<String> {
    findings
        .iter()
        .flat_map(|finding| {
            let mut lines = vec![format!(
                "  - {} | confidence={} | sufficiency={}",
                finding.title,
                finding.confidence.as_str(),
                finding.sufficiency.as_str()
            )];
            if !finding.summary.is_empty() {
                lines.push(format!("    {}", finding.summary));
            }
            if !finding.evidence_refs.is_empty() {
                lines.push(format!(
                    "    evidence: {}",
                    finding
                        .evidence_refs
                        .iter()
                        .map(|reference| reference.export_ref.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if !finding.counterevidence_refs.is_empty() {
                lines.push(format!(
                    "    counterevidence: {}",
                    finding
                        .counterevidence_refs
                        .iter()
                        .map(|reference| reference.export_ref.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            lines
        })
        .collect()
}

fn abbreviate_id(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        value.to_owned()
    } else {
        value.chars().take(max_len).collect()
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
            cards.sort_by(|left, right| {
                right
                    .score
                    .cmp(&left.score)
                    .then_with(|| right.confidence.cmp(&left.confidence))
                    .then_with(|| left.signal_key.cmp(&right.signal_key))
                    .then_with(|| left.id.cmp(&right.id))
            });
            cards
        }
    }
}

fn review_detail_lines(
    review_mode: ReviewScreenMode,
    selected_card: Option<&ReviewCard>,
    investigation: &InvestigationReport,
    active_population: PopulationProfile,
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
        if let Some(spec) = claim_language_spec(&card.signal_key, active_population) {
            lines.push(format!("Evidence tier: {}", spec.tier_label));
            lines.push(format!("Interpretation: {}", spec.interpretation_label));
            lines.push(format!(
                "Population scope: {}",
                review_population_scope_line(&spec)
            ));
            if let Some(guidance_label) = spec.guidance_label {
                lines.push(format!("Guidance anchor: {guidance_label}"));
            }
            if !spec.caution_labels.is_empty() {
                lines.push(format!(
                    "Caution rails: {}",
                    spec.caution_labels.join(" | ")
                ));
            }
        }
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

fn review_card_badges(card: &ReviewCard, active_population: PopulationProfile) -> Vec<String> {
    let badges = evidence_badges(&card.signal_key, active_population);
    let Some(spec) = claim_language_spec(&card.signal_key, active_population) else {
        return badges.into_iter().take(3).collect();
    };

    if badges.is_empty() {
        return Vec::new();
    }

    let has_primary_caution = spec
        .caution_labels
        .iter()
        .any(|label| is_primary_review_caution_badge(label));
    let max_badges = if has_primary_caution {
        5
    } else if spec.population_support_status != PopulationSupportStatus::PopulationSpecific {
        4
    } else {
        3
    };

    let mut prioritized = Vec::new();
    push_review_badge(&mut prioritized, spec.tier_label);
    if let Some(guidance_label) = spec.guidance_label.clone() {
        push_review_badge(&mut prioritized, guidance_label);
    }
    if let Some(interpretation_badge) = badges
        .iter()
        .find(|badge| is_interpretation_badge(badge))
        .cloned()
    {
        push_review_badge(&mut prioritized, interpretation_badge);
    }
    if spec.population_support_status != PopulationSupportStatus::PopulationSpecific {
        push_review_badge(
            &mut prioritized,
            spec.population_support_status.badge_label().to_owned(),
        );
    }
    for badge in [
        "Sensitive metric",
        "Not for screening",
        "Not diagnostic",
        "Consumer wearable limitation",
    ] {
        if spec.caution_labels.iter().any(|label| label == badge) {
            push_review_badge(&mut prioritized, badge.to_owned());
        }
    }
    for badge in badges {
        push_review_badge(&mut prioritized, badge);
    }
    prioritized.truncate(max_badges);
    prioritized
}

fn evidence_review_status_line(snapshot: &LiveSnapshot) -> String {
    match snapshot.stale_evidence_entries.as_slice() {
        [] => "current".to_owned(),
        [entry] => format!("1 stale entry | {entry}"),
        entries => format!("{} stale entries | {}", entries.len(), entries[0]),
    }
}

fn review_population_scope_line(spec: &crate::evidence::policy::ClaimLanguageSpec) -> String {
    match spec.population_support_status {
        PopulationSupportStatus::PopulationSpecific => {
            format!(
                "{} guidance/profile support available",
                spec.active_population_profile.label()
            )
        }
        PopulationSupportStatus::GeneralAdultOnlyFallback => {
            let fallback_label = spec
                .fallback_population_profile
                .map_or("General adult", PopulationProfile::label);
            format!(
                "{} profile uses {fallback_label} guidance as a fallback",
                spec.active_population_profile.label()
            )
        }
        PopulationSupportStatus::Unavailable => format!(
            "{} profile has no supported interpretation; keep this context-only",
            spec.active_population_profile.label()
        ),
    }
}

fn is_interpretation_badge(badge: &str) -> bool {
    matches!(badge, "Guidance-backed" | "Trend-only" | "Context-only")
}

fn is_primary_review_caution_badge(badge: &str) -> bool {
    matches!(
        badge,
        "Sensitive metric"
            | "Not for screening"
            | "Not diagnostic"
            | "Consumer wearable limitation"
    )
}

fn push_review_badge(badges: &mut Vec<String>, badge: String) {
    if !badges.iter().any(|existing| existing == &badge) {
        badges.push(badge);
    }
}

fn empty_ai_artifact_summary_view() -> AiArtifactSummaryView {
    AiArtifactSummaryView {
        availability: TelemetryAvailability::NoData,
        status_label: "none".to_owned(),
        metadata_lines: Vec::new(),
        summary_text: "No saved AI artifact is linked to this day yet.".to_owned(),
        lineage_lines: Vec::new(),
    }
}

fn build_ai_artifact_summary_view(record: &AiArtifactDaySummaryRecord) -> AiArtifactSummaryView {
    let mut summary_parts = Vec::new();
    if !record.summary_cache.trim().is_empty() {
        summary_parts.push(record.summary_cache.trim().to_owned());
    }
    if !record.overview.trim().is_empty()
        && summary_parts
            .last()
            .is_none_or(|summary| summary.as_str() != record.overview.trim())
    {
        summary_parts.push(record.overview.trim().to_owned());
    }

    let summary_text = if summary_parts.is_empty() {
        "Saved artifact text is unavailable for this run.".to_owned()
    } else {
        summary_parts.join("\n")
    };

    let mut lineage_lines = vec![
        format!("Run id: {}", record.artifact_id),
        format!("Snapshot hash: {}", record.matched_snapshot_hash),
    ];
    if let Some(peer_snapshot_hash) = &record.peer_snapshot_hash {
        lineage_lines.push(format!("Peer snapshot: {peer_snapshot_hash}"));
    }

    AiArtifactSummaryView {
        availability: TelemetryAvailability::Fresh,
        status_label: "available".to_owned(),
        metadata_lines: vec![
            format!(
                "Kind / created: {} / {}",
                record.artifact_kind,
                trim_date_time(&record.created_at)
            ),
            format!("Provider / model: {} / {}", record.provider, record.model),
            format!(
                "Prompt / schema: {} / {}",
                record.prompt_version, record.output_schema_version
            ),
            format!("Privacy profile: {}", record.privacy_profile),
        ],
        summary_text,
        lineage_lines,
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
    let current_day = current_local_day_string();
    for period in &snapshot.rest_mode_periods {
        days.push(period.start_day.clone());
        if let Some(end_day) = &period.end_day {
            days.push(end_day.clone());
        } else {
            days.push(current_day.clone());
        }
    }
    days.into_iter().max().unwrap_or(current_day)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveReviewLoadBounds {
    signal_start: String,
    signal_end: String,
    sleep_start: String,
    sleep_end: String,
    context_start: String,
    context_end: String,
    rest_mode_start: String,
    rest_mode_end: String,
}

fn live_review_load_bounds(
    daily_history: &[DailyOverviewRow],
    heartrate_days: &[HeartRateDay],
    latest_review_day: Option<&str>,
) -> crate::error::Result<Option<LiveReviewLoadBounds>> {
    let mut anchor_days = daily_history
        .iter()
        .map(|row| row.day.as_str())
        .chain(heartrate_days.iter().map(|day| day.day.as_str()))
        .collect::<Vec<_>>();
    if let Some(latest_review_day) = latest_review_day {
        anchor_days.push(latest_review_day);
    }

    let Some(oldest_anchor) = anchor_days.iter().min().copied() else {
        return Ok(None);
    };
    let newest_anchor = anchor_days.iter().max().copied().unwrap_or(oldest_anchor);

    let (signal_start_day, signal_end_day) = bounded_day_range(
        oldest_anchor,
        newest_anchor,
        LIVE_REVIEW_SIGNAL_LOOKBACK_DAYS,
        0,
    )?;
    let (sleep_start_day, sleep_end_day) = bounded_day_range(
        oldest_anchor,
        newest_anchor,
        LIVE_REVIEW_SLEEP_LOOKBACK_DAYS,
        0,
    )?;
    let (context_start_day, context_end_day) = bounded_day_range(
        oldest_anchor,
        newest_anchor,
        LIVE_REVIEW_CONTEXT_LOOKBACK_DAYS,
        LIVE_REVIEW_CONTEXT_FORWARD_DAYS,
    )?;
    let (rest_mode_start_day, rest_mode_end_day) = bounded_day_range(
        oldest_anchor,
        newest_anchor,
        LIVE_REVIEW_REST_MODE_LOOKBACK_DAYS,
        LIVE_REVIEW_CONTEXT_FORWARD_DAYS,
    )?;

    Ok(Some(LiveReviewLoadBounds {
        signal_start: signal_start_day,
        signal_end: signal_end_day,
        sleep_start: sleep_start_day,
        sleep_end: sleep_end_day,
        context_start: context_start_day,
        context_end: context_end_day,
        rest_mode_start: rest_mode_start_day,
        rest_mode_end: rest_mode_end_day,
    }))
}

fn bounded_day_range(
    oldest_anchor: &str,
    newest_anchor: &str,
    lookback_days: i64,
    forward_days: i64,
) -> crate::error::Result<(String, String)> {
    let oldest_anchor = parse_app_day(oldest_anchor)?;
    let newest_anchor = parse_app_day(newest_anchor)?;
    let start_day = oldest_anchor
        .checked_sub(time::Duration::days(lookback_days))
        .unwrap_or(oldest_anchor);
    let end_day = newest_anchor
        .checked_add(time::Duration::days(forward_days))
        .unwrap_or(newest_anchor);
    Ok((start_day.to_string(), end_day.to_string()))
}

fn parse_app_day(day: &str) -> crate::error::Result<Date> {
    Date::parse(
        day,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .map_err(|error| {
        crate::error::RingmasterError::Config(format!(
            "failed to parse app review day `{day}`: {error}"
        ))
    })
}

fn empty_review_deck(mode: ReviewMode, anchor_day: &str, error: &impl ToString) -> ReviewDeck {
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
    error: &impl ToString,
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
            "Open Status to confirm sync freshness and granted capabilities.".to_owned(),
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
    format!("desired_enabled={desired} remote={remote} healthy={healthy} drifted={drifted}")
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
        .map_or_else(
            || "none".to_owned(),
            |(_, record)| format!("{} {}", record.data_type, record.expiration_time),
        );

    format!("renewals_due={renewals_due} next_expiration={next_expiration}")
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

    format!("webhook={webhook_count} periodic={periodic_count} other={other_count}")
}

fn build_app_title(snapshot: &LiveSnapshot, selected_day: &str, refresh_in_flight: bool) -> String {
    let connection_state = header_connection_label(&snapshot.auth_status);
    let daily_freshness = freshness_badge(&family_freshness(snapshot, DataFamily::Daily));
    let refresh_state = if refresh_in_flight { "Running" } else { "Idle" };
    let granted_scope_count = snapshot.auth_status.granted_scopes.len();

    [
        format!(
            "Connection: {connection_state} | Daily status: {daily_freshness} | Viewing: {selected_day} | Sync: {refresh_state}"
        ),
        format!(
            "Latest sync: {} | Access: {granted_scope_count} scopes granted | Triggers: {}",
            latest_sync_summary(snapshot),
            freshness_source_summary(snapshot)
        ),
    ]
    .join("\n")
}

fn latest_sync_summary(snapshot: &LiveSnapshot) -> String {
    snapshot
        .sync_states
        .iter()
        .filter_map(|state| {
            sync_state_effective_timestamp(state).map(|timestamp| (timestamp, state))
        })
        .max_by_key(|(timestamp, _)| *timestamp)
        .map_or_else(
            || "none".to_owned(),
            |(_, state)| {
                let timestamp = state
                    .last_completed_at
                    .as_deref()
                    .unwrap_or(&state.last_attempted_at);
                format!("{} at {}", state.sync_key, trim_date_time(timestamp))
            },
        )
}

const fn header_connection_label(auth_status: &AuthStatus) -> &'static str {
    if auth_status.access_token_stored || auth_status.refresh_token_stored {
        "Connected"
    } else if auth_status.configured {
        "Configured, no session"
    } else {
        "Setup needed"
    }
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

const fn family_supports_webhooks(family: DataFamily) -> bool {
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
            / crate::numeric::usize_to_f64(samples.len());
        points.push(MetricPoint {
            day,
            value: mean_bpm,
        });
    }

    Ok(points)
}

fn family_freshness(snapshot: &LiveSnapshot, family: DataFamily) -> FreshnessState {
    let capability_report = &snapshot.auth_status.capability_report;
    if let Some(state) = freshness_capability_state(capability_report, family) {
        return state;
    }

    let has_data = family_has_data(snapshot, family);
    let sync_state = sync_state_for(&snapshot.sync_states, family);
    let now = parse_timestamp(&snapshot.captured_at).unwrap_or_else(OffsetDateTime::now_utc);
    let webhook_state = freshness_webhook_state(snapshot, family);
    let last_delivery = family_last_delivery(snapshot, family);

    if let Some(state) = freshness_from_sync_state(snapshot, family, sync_state, now) {
        return state;
    }

    if let Some(state) = freshness_webhook_guard_state(family, webhook_state) {
        return state;
    }

    if !has_data {
        return freshness_missing_data_state(family, sync_state);
    }

    freshness_delivery_state(family, webhook_state.supports_webhooks(), last_delivery)
}

fn freshness_capability_state(
    capability_report: &CapabilityReport,
    family: DataFamily,
) -> Option<FreshnessState> {
    (!capability_report.is_granted(family.capability_kind())).then(|| FreshnessState {
        family,
        kind: FreshnessKind::StaleCapabilityMissing,
        summary: freshness_label(FreshnessKind::StaleCapabilityMissing),
        detail: format!(
            "{} scope was not granted, so {} stay unavailable.",
            family.capability_kind().scope_name(),
            family.label().to_lowercase()
        ),
    })
}

fn freshness_from_sync_state(
    snapshot: &LiveSnapshot,
    family: DataFamily,
    sync_state: Option<&SyncStateRecord>,
    now: OffsetDateTime,
) -> Option<FreshnessState> {
    let sync_state = sync_state?;
    if sync_state.last_error.as_ref().is_some_and(is_auth_problem)
        || matches!(sync_state.status, SyncRunStatus::Failed)
    {
        return Some(FreshnessState {
            family,
            kind: FreshnessKind::StaleSyncFailed,
            summary: freshness_label(FreshnessKind::StaleSyncFailed),
            detail: sync_state.message.clone().unwrap_or_else(|| {
                sync_state.last_error.as_ref().map_or_else(
                    || format!("{} failed to sync.", family.label()),
                    ToString::to_string,
                )
            }),
        });
    }

    let reference = sync_state
        .last_completed_at
        .as_deref()
        .or(Some(sync_state.last_attempted_at.as_str()));
    let is_fresh = reference
        .and_then(parse_timestamp)
        .is_some_and(|timestamp| {
            now - timestamp
                <= time::Duration::seconds(
                    snapshot
                        .refresh_policy
                        .stale_after_seconds(family)
                        .cast_signed(),
                )
        });
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
        Some(FreshnessState {
            family,
            kind,
            summary: freshness_label(kind),
            detail: fresh_sync_detail(snapshot, family, sync_state),
        })
    } else {
        None
    }
}

fn fresh_sync_detail(
    snapshot: &LiveSnapshot,
    family: DataFamily,
    sync_state: &SyncStateRecord,
) -> String {
    match (family, latest_day_is_before_today(snapshot)) {
        (DataFamily::Daily, true) => {
            "Daily closeout is current through the latest fully available upstream day.".to_owned()
        }
        _ => format!(
            "{} updated at {}.",
            family.label(),
            sync_state
                .last_completed_at
                .clone()
                .unwrap_or_else(|| sync_state.last_attempted_at.clone())
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FreshnessWebhookState {
    Unsupported,
    ReceiverConfigMissing,
    ReceiverRuntimeUnhealthy,
    SubscriptionMissing,
    Ready,
}

impl FreshnessWebhookState {
    const fn supports_webhooks(self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

fn freshness_webhook_state(snapshot: &LiveSnapshot, family: DataFamily) -> FreshnessWebhookState {
    if !family_supports_webhooks(family) {
        return FreshnessWebhookState::Unsupported;
    }

    if !receiver_config_complete(snapshot) {
        return FreshnessWebhookState::ReceiverConfigMissing;
    }

    if !receiver_healthy(snapshot) {
        return FreshnessWebhookState::ReceiverRuntimeUnhealthy;
    }

    if !family_subscription_ready(snapshot, family) {
        return FreshnessWebhookState::SubscriptionMissing;
    }

    FreshnessWebhookState::Ready
}

fn freshness_webhook_guard_state(
    family: DataFamily,
    webhook_state: FreshnessWebhookState,
) -> Option<FreshnessState> {
    match webhook_state {
        FreshnessWebhookState::ReceiverConfigMissing => Some(FreshnessState {
            family,
            kind: FreshnessKind::StaleReceiverDown,
            summary: freshness_label(FreshnessKind::StaleReceiverDown),
            detail: "Webhook receiver configuration is incomplete for this family.".to_owned(),
        }),
        FreshnessWebhookState::ReceiverRuntimeUnhealthy => Some(FreshnessState {
            family,
            kind: FreshnessKind::StaleReceiverDown,
            summary: freshness_label(FreshnessKind::StaleReceiverDown),
            detail: "Webhook receiver heartbeat is stale or missing.".to_owned(),
        }),
        FreshnessWebhookState::SubscriptionMissing => Some(FreshnessState {
            family,
            kind: FreshnessKind::StaleSubscriptionMissing,
            summary: freshness_label(FreshnessKind::StaleSubscriptionMissing),
            detail: format!(
                "{} subscriptions are missing, drifted, or expired.",
                family.label()
            ),
        }),
        FreshnessWebhookState::Unsupported | FreshnessWebhookState::Ready => None,
    }
}

fn freshness_missing_data_state(
    family: DataFamily,
    sync_state: Option<&SyncStateRecord>,
) -> FreshnessState {
    FreshnessState {
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
    }
}

fn freshness_delivery_state(
    family: DataFamily,
    supports_webhooks: bool,
    last_delivery: Option<&AcceptedWebhookDeliveryRecord>,
) -> FreshnessState {
    if supports_webhooks {
        FreshnessState {
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
        }
    } else {
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

fn selected_day_baseline_sentence(
    label: &str,
    selected_day: &str,
    insight: &MetricInsight,
) -> String {
    let Some(today) = insight.today.as_ref() else {
        return format!("{label} has no daily closeout on {selected_day}.");
    };
    if insight.baseline_30d.sample_count < 4 {
        return format!(
            "{} is {} on {}, but local history is still too thin to compare it to your normal yet.",
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

fn visible_timeline(day: &HeartRateDay, window_hours: u16) -> VisibleTimeline {
    let latest_minute = day
        .points
        .last()
        .map_or(0, |point| minutes_from_timestamp(&point.recorded_at));
    let window_end = latest_minute.max(window_hours.saturating_mul(60).saturating_sub(1));
    let window_start = window_end.saturating_sub(window_hours.saturating_mul(60).saturating_sub(1));
    let mut visible = Vec::new();
    let mut previous_minute = None;

    for point in &day.points {
        let minute = minutes_from_timestamp(&point.recorded_at);
        if minute < window_start || minute > window_end {
            continue;
        }

        let gap_before =
            previous_minute.is_some_and(|previous| minute.saturating_sub(previous) > 30);
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
    let reference_day = current_local_day_string();
    latest_day_is_before_reference_day(snapshot, &reference_day)
}

fn latest_day_is_before_reference_day(snapshot: &LiveSnapshot, reference_day: &str) -> bool {
    snapshot
        .daily_history
        .last()
        .is_some_and(|row| row.day.as_str() < reference_day)
}

fn selected_daily_row<'a>(snapshot: &'a LiveSnapshot, day: &str) -> Option<&'a DailyOverviewRow> {
    snapshot.daily_history.iter().find(|row| row.day == day)
}

fn selected_daily_activity<'a>(
    snapshot: &'a LiveSnapshot,
    day: &str,
) -> Option<&'a DailyActivityRecord> {
    snapshot.daily_activity.iter().find(|row| row.day == day)
}

fn selected_daily_readiness<'a>(
    snapshot: &'a LiveSnapshot,
    day: &str,
) -> Option<&'a DailyReadinessRecord> {
    snapshot.daily_readiness.iter().find(|row| row.day == day)
}

fn selected_daily_stress<'a>(
    snapshot: &'a LiveSnapshot,
    day: &str,
) -> Option<&'a DailyStressRecord> {
    snapshot.daily_stress.iter().find(|row| row.day == day)
}

fn selected_primary_sleep_period<'a>(
    snapshot: &'a LiveSnapshot,
    day: &str,
) -> Option<&'a SleepPeriodRecord> {
    snapshot
        .sleep_periods
        .iter()
        .filter(|record| record.day == day)
        .max_by(|left, right| {
            let left_rank = primary_sleep_rank(left.sleep_type.as_deref());
            let right_rank = primary_sleep_rank(right.sleep_type.as_deref());
            left_rank
                .cmp(&right_rank)
                .then_with(|| {
                    left.total_sleep_duration
                        .unwrap_or_default()
                        .cmp(&right.total_sleep_duration.unwrap_or_default())
                })
                .then_with(|| right.bedtime_start.cmp(&left.bedtime_start))
        })
}

fn selected_daily_spo2<'a>(snapshot: &'a LiveSnapshot, day: &str) -> Option<&'a DailySpO2Record> {
    snapshot.daily_spo2.iter().find(|row| row.day == day)
}

fn selected_heartrate_day<'a>(snapshot: &'a LiveSnapshot, day: &str) -> Option<&'a HeartRateDay> {
    snapshot.heartrate_days.iter().find(|row| row.day == day)
}

fn metric_points_from_activity(history: &[DailyActivityRecord]) -> Vec<MetricPoint> {
    history
        .iter()
        .map(|row| MetricPoint {
            day: row.day.clone(),
            value: crate::numeric::i64_to_f64(row.steps),
        })
        .collect()
}

fn metric_points_from_sleep_periods<F>(
    history: &[SleepPeriodRecord],
    mut mapper: F,
) -> Vec<MetricPoint>
where
    F: FnMut(&SleepPeriodRecord) -> Option<f64>,
{
    history
        .iter()
        .filter(|record| is_primary_sleep_type(record.sleep_type.as_deref()))
        .filter_map(|record| {
            mapper(record).map(|value| MetricPoint {
                day: record.day.clone(),
                value,
            })
        })
        .collect()
}

fn metric_points_from_daily_spo2(history: &[DailySpO2Record]) -> Vec<MetricPoint> {
    history
        .iter()
        .filter_map(|record| {
            record.average_spo2.map(|value| MetricPoint {
                day: record.day.clone(),
                value,
            })
        })
        .collect()
}

fn build_metric_insight_from_points(
    history: &[MetricPoint],
    selected_day: &str,
    label: &'static str,
) -> MetricInsight {
    let filtered = history
        .iter()
        .filter(|point| point.day.as_str() <= selected_day)
        .cloned()
        .collect::<Vec<_>>();
    build_metric_insight(label, &filtered)
}

fn metric_points_from_readiness_temperature(history: &[DailyReadinessRecord]) -> Vec<MetricPoint> {
    history
        .iter()
        .filter_map(|row| {
            row.temperature_deviation.map(|value| MetricPoint {
                day: row.day.clone(),
                value,
            })
        })
        .collect()
}

fn metric_points_from_stress(history: &[DailyStressRecord]) -> Vec<MetricPoint> {
    history
        .iter()
        .filter_map(|row| {
            row.stress_high.map(|value| MetricPoint {
                day: row.day.clone(),
                value: crate::numeric::i64_to_f64(value),
            })
        })
        .collect()
}

fn values_from_metric_points(history: &[MetricPoint]) -> Vec<u64> {
    history
        .iter()
        .map(|point| crate::numeric::rounded_nonnegative_f64_to_u64(point.value))
        .collect()
}

fn availability_from_freshness(freshness: &FreshnessState) -> TelemetryAvailability {
    match freshness.kind {
        FreshnessKind::FreshWebhook | FreshnessKind::FreshPeriodic => TelemetryAvailability::Fresh,
        FreshnessKind::StaleCapabilityMissing => TelemetryAvailability::MissingScope,
        FreshnessKind::StaleSyncFailed => {
            let detail = freshness.detail.to_ascii_lowercase();
            if detail.contains("429") || detail.contains("rate limit") {
                TelemetryAvailability::RateLimited
            } else {
                TelemetryAvailability::Error
            }
        }
        FreshnessKind::StaleUpstreamPending => TelemetryAvailability::NoData,
        FreshnessKind::StaleNoRecentDelivery
        | FreshnessKind::StaleUnsupportedWebhook
        | FreshnessKind::StaleReceiverDown
        | FreshnessKind::StaleSubscriptionMissing => TelemetryAvailability::Stale,
    }
}

fn telemetry_availability_for_daily_metric(
    snapshot: &LiveSnapshot,
    capability: CapabilityKind,
    has_records: bool,
) -> TelemetryAvailability {
    let status = snapshot
        .auth_status
        .capability_report
        .status_for(capability);
    match status {
        Some(entry) if entry.granted => {
            if has_records {
                availability_from_freshness(&family_freshness(snapshot, DataFamily::Daily))
            } else {
                TelemetryAvailability::NoData
            }
        }
        Some(entry) if entry.requested => TelemetryAvailability::MissingScope,
        _ => TelemetryAvailability::Unsupported,
    }
}

fn selected_metric_note(
    label: &str,
    selected_day: &str,
    value_present: bool,
    insight: &MetricInsight,
) -> String {
    if value_present {
        insight.summary.clone()
    } else {
        format!("No {label} reading is available for {selected_day}.")
    }
}

fn format_duration_compact(seconds: i64) -> String {
    if seconds <= 0 {
        return "--".to_owned();
    }

    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else {
        format!("{minutes}m")
    }
}

fn format_number(value: i64) -> String {
    let negative = value.is_negative();
    let digits = value.unsigned_abs().to_string();
    let grouped = digits
        .chars()
        .rev()
        .enumerate()
        .fold(String::new(), |mut acc, (index, ch)| {
            if index > 0 && index % 3 == 0 {
                acc.push(',');
            }
            acc.push(ch);
            acc
        })
        .chars()
        .rev()
        .collect::<String>();

    if negative {
        format!("-{grouped}")
    } else {
        grouped
    }
}

fn metric_delta_label(insight: &MetricInsight) -> String {
    insight.baseline_7d.delta_from_today.map_or_else(
        || {
            insight.day_over_day_delta.map_or_else(
                || "baseline --".to_owned(),
                |delta| format!("d/d {delta:+.1}"),
            )
        },
        |delta| format!("vs 7d {delta:+.1}"),
    )
}

fn metric_range_label(history: &[MetricPoint]) -> String {
    let mut values = history.iter().map(|point| point.value);
    let Some(first) = values.next() else {
        return "range --".to_owned();
    };
    let (min_value, max_value) = values.fold((first, first), |(min_value, max_value), value| {
        (min_value.min(value), max_value.max(value))
    });
    format!("{}-{}", format_float(min_value), format_float(max_value))
}

fn primary_sleep_rank(sleep_type: Option<&str>) -> u8 {
    match sleep_type {
        Some("long_sleep") => 3,
        Some("sleep") => 2,
        Some("rest") => 1,
        _ => 0,
    }
}

fn is_primary_sleep_type(sleep_type: Option<&str>) -> bool {
    primary_sleep_rank(sleep_type) > 0
}

fn activity_ring_fill_from_steps(steps: i64) -> u16 {
    let capped = steps.clamp(0, 12_000);
    let fill = (crate::numeric::i64_to_f64(capped) / 12_000.0) * 100.0;
    crate::numeric::rounded_clamped_f64_to_u16(fill, 0.0, 100.0)
}

fn activity_delta_label(snapshot: &LiveSnapshot, selected_day: &str) -> String {
    let history = snapshot
        .daily_activity
        .iter()
        .filter(|row| row.day.as_str() <= selected_day)
        .map(|row| MetricPoint {
            day: row.day.clone(),
            value: crate::numeric::i64_to_f64(row.steps),
        })
        .collect::<Vec<_>>();
    let insight = build_metric_insight("activity", &history);
    if let Some(delta) = insight.baseline_7d.delta_from_today {
        let rounded = crate::numeric::rounded_nonnegative_f64_to_u64(delta.abs());
        format!(
            "vs 7d {}{}",
            if delta.is_sign_negative() { "-" } else { "+" },
            format_number(i64::try_from(rounded).unwrap_or(i64::MAX))
        )
    } else if let Some(delta) = insight.day_over_day_delta {
        format!("d/d {delta:+.0}")
    } else {
        "baseline --".to_owned()
    }
}

fn heart_rate_primary_label(snapshot: &LiveSnapshot, selected_day: &str) -> String {
    snapshot
        .heartrate_daily_averages
        .iter()
        .find(|point| point.day == selected_day)
        .map_or_else(
            || {
                selected_heartrate_day(snapshot, selected_day)
                    .and_then(|day| day.points.last())
                    .map_or_else(|| "--".to_owned(), |point| format!("{} bpm", point.bpm))
            },
            |point| format!("{} bpm avg", format_float(point.value)),
        )
}

fn recent_dashboard_waveform(snapshot: &LiveSnapshot) -> Vec<u64> {
    let readiness = metric_points_from_daily(&snapshot.daily_history, |row| {
        row.readiness_score.map(f64::from)
    });
    if readiness.is_empty() {
        values_from_metric_points(&metric_points_from_daily(&snapshot.daily_history, |row| {
            row.sleep_score.map(f64::from)
        }))
    } else {
        values_from_metric_points(&readiness)
    }
}

fn dashboard_capability_summary(snapshot: &LiveSnapshot) -> Vec<String> {
    CoverageFamily::ALL
        .into_iter()
        .map(|family| {
            let status = snapshot
                .auth_status
                .capability_report
                .status_for(family.capability_kind());
            let state = match status {
                Some(entry) if entry.granted => "ok",
                Some(entry) if entry.requested => "scope",
                _ => "n/a",
            };
            format!("{}:{state}", family.label())
        })
        .collect()
}

fn dashboard_header_freshness(snapshot: &LiveSnapshot) -> String {
    freshness_badge(&family_freshness(snapshot, DataFamily::Daily)).to_ascii_uppercase()
}

fn coverage_availability(snapshot: &LiveSnapshot, family: CoverageFamily) -> TelemetryAvailability {
    match family {
        CoverageFamily::Daily => {
            availability_from_freshness(&family_freshness(snapshot, DataFamily::Daily))
        }
        CoverageFamily::Heartrate => {
            availability_from_freshness(&family_freshness(snapshot, DataFamily::Heartrate))
        }
        CoverageFamily::Workout => {
            availability_from_freshness(&family_freshness(snapshot, DataFamily::Workout))
        }
        CoverageFamily::Session => {
            availability_from_freshness(&family_freshness(snapshot, DataFamily::Session))
        }
        CoverageFamily::Tag => {
            let tag_status = snapshot
                .auth_status
                .capability_report
                .status_for(CapabilityKind::Tag);
            let enhanced_status = snapshot
                .auth_status
                .capability_report
                .status_for(CapabilityKind::EnhancedTag);
            if tag_status.is_some_and(|entry| entry.granted)
                || enhanced_status.is_some_and(|entry| entry.granted)
            {
                if snapshot.record_counts.tags + snapshot.record_counts.enhanced_tags > 0 {
                    availability_from_freshness(&family_freshness(
                        snapshot,
                        DataFamily::EnhancedTag,
                    ))
                } else {
                    TelemetryAvailability::NoData
                }
            } else if tag_status.is_some_and(|entry| entry.requested)
                || enhanced_status.is_some_and(|entry| entry.requested)
            {
                TelemetryAvailability::MissingScope
            } else {
                TelemetryAvailability::Unsupported
            }
        }
        CoverageFamily::Spo2 => telemetry_availability_for_daily_metric(
            snapshot,
            CapabilityKind::Spo2,
            !snapshot.daily_spo2.is_empty(),
        ),
    }
}

fn coverage_cell_views(snapshot: &LiveSnapshot) -> Vec<CoverageCellView> {
    CoverageFamily::ALL
        .into_iter()
        .map(|family| CoverageCellView {
            label: family.label(),
            availability: coverage_availability(snapshot, family),
            detail: coverage_detail(snapshot, family),
        })
        .collect()
}

fn coverage_detail(snapshot: &LiveSnapshot, family: CoverageFamily) -> String {
    match family {
        CoverageFamily::Daily => family_freshness(snapshot, DataFamily::Daily).detail,
        CoverageFamily::Heartrate => family_freshness(snapshot, DataFamily::Heartrate).detail,
        CoverageFamily::Workout => family_freshness(snapshot, DataFamily::Workout).detail,
        CoverageFamily::Session => family_freshness(snapshot, DataFamily::Session).detail,
        CoverageFamily::Tag => {
            if snapshot.record_counts.tags + snapshot.record_counts.enhanced_tags > 0 {
                family_freshness(snapshot, DataFamily::EnhancedTag).detail
            } else {
                "Tag coverage is available but there are no cached tag records yet.".to_owned()
            }
        }
        CoverageFamily::Spo2 => snapshot
            .auth_status
            .capability_report
            .status_for(CapabilityKind::Spo2)
            .map_or_else(
                || "SpO2 is not configured in the current local model.".to_owned(),
                |entry| {
                    if entry.granted {
                        if snapshot.daily_spo2.is_empty() {
                            "SpO2 scope is granted, but there are no cached SpO2 readings yet."
                                .to_owned()
                        } else {
                            family_freshness(snapshot, DataFamily::Daily).detail
                        }
                    } else {
                        entry.note.clone()
                    }
                },
            ),
    }
}

struct DashboardBreakdownInputs<'a> {
    snapshot: &'a LiveSnapshot,
    selected_day: &'a str,
    sleep_insight: &'a MetricInsight,
    readiness_insight: &'a MetricInsight,
    heartrate_insight: &'a MetricInsight,
    selected_readiness: Option<&'a DailyReadinessRecord>,
    selected_stress: Option<&'a DailyStressRecord>,
    selected_breakdown_index: usize,
}

fn build_dashboard_breakdown_rails(
    inputs: &DashboardBreakdownInputs<'_>,
) -> Vec<DashboardBreakdownRail> {
    let sleep_fill = inputs.sleep_insight.today.as_ref().map_or(0, |point| {
        crate::numeric::rounded_clamped_f64_to_u16(point.value, 0.0, 100.0)
    });
    let recovery_fill = inputs.readiness_insight.today.as_ref().map_or(0, |point| {
        crate::numeric::rounded_clamped_f64_to_u16(point.value, 0.0, 100.0)
    });
    let heartrate_fill = inputs
        .heartrate_insight
        .baseline_7d
        .delta_from_today
        .map_or(0, |delta| {
            crate::numeric::rounded_clamped_f64_to_u16(delta.abs().mul_add(-8.0, 100.0), 0.0, 100.0)
        });
    let temp_fill = inputs
        .selected_readiness
        .and_then(|row| row.temperature_deviation)
        .map_or(0, |value| {
            crate::numeric::rounded_clamped_f64_to_u16(
                value.abs().mul_add(-40.0, 100.0),
                0.0,
                100.0,
            )
        });

    let mut rails = vec![
        DashboardBreakdownRail {
            label: "HRV Balance".to_owned(),
            availability: TelemetryAvailability::Unsupported,
            fill_percent: 0,
            delta_label: "scope pending".to_owned(),
            note: "HRV balance is reserved until HRV history is stored locally.".to_owned(),
            selected: false,
        },
        DashboardBreakdownRail {
            label: "Resting HR".to_owned(),
            availability: if inputs.snapshot.heartrate_daily_averages.is_empty() {
                TelemetryAvailability::NoData
            } else {
                availability_from_freshness(&family_freshness(
                    inputs.snapshot,
                    DataFamily::Heartrate,
                ))
            },
            fill_percent: heartrate_fill,
            delta_label: metric_delta_label(inputs.heartrate_insight),
            note: inputs.heartrate_insight.summary.clone(),
            selected: false,
        },
        DashboardBreakdownRail {
            label: "Sleep Balance".to_owned(),
            availability: if inputs.sleep_insight.today.is_some() {
                availability_from_freshness(&family_freshness(inputs.snapshot, DataFamily::Daily))
            } else {
                TelemetryAvailability::NoData
            },
            fill_percent: sleep_fill,
            delta_label: metric_delta_label(inputs.sleep_insight),
            note: selected_day_baseline_sentence(
                "Sleep",
                inputs.selected_day,
                inputs.sleep_insight,
            ),
            selected: false,
        },
        DashboardBreakdownRail {
            label: "Recovery Index".to_owned(),
            availability: if inputs.readiness_insight.today.is_some() {
                availability_from_freshness(&family_freshness(inputs.snapshot, DataFamily::Daily))
            } else {
                TelemetryAvailability::NoData
            },
            fill_percent: recovery_fill.max(temp_fill),
            delta_label: metric_delta_label(inputs.readiness_insight),
            note: inputs
                .selected_stress
                .and_then(|row| row.day_summary.clone())
                .or_else(|| {
                    inputs.selected_readiness.and_then(|row| {
                        row.temperature_deviation.map(|value| {
                            format!("Temperature deviation {value:+.1}°C vs baseline.")
                        })
                    })
                })
                .unwrap_or_else(|| inputs.readiness_insight.summary.clone()),
            selected: false,
        },
    ];

    let selected_index = usize::min(
        inputs.selected_breakdown_index,
        rails.len().saturating_sub(1),
    );
    if let Some(rail) = rails.get_mut(selected_index) {
        rail.selected = true;
    }
    rails
}

fn build_dashboard_weekly_heatmap(
    snapshot: &LiveSnapshot,
    selected_day: &str,
) -> DashboardWeeklyHeatmap {
    let recent_rows = latest_daily_rows(snapshot, 7);
    let history_rows = latest_daily_rows(snapshot, 14);

    DashboardWeeklyHeatmap {
        availability: if recent_rows.is_empty() {
            TelemetryAvailability::NoData
        } else {
            availability_from_freshness(&family_freshness(snapshot, DataFamily::Daily))
        },
        row_labels: vec![
            "Sleep".to_owned(),
            "Readiness".to_owned(),
            "Activity".to_owned(),
        ],
        recent: build_dashboard_heatmap_grid(&recent_rows, selected_day),
        history: build_dashboard_heatmap_grid(&history_rows, selected_day),
        note: "Recent score bands for sleep, readiness, and activity.".to_owned(),
    }
}

fn latest_daily_rows(snapshot: &LiveSnapshot, limit: usize) -> Vec<&DailyOverviewRow> {
    snapshot
        .daily_history
        .iter()
        .rev()
        .take(limit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn build_dashboard_heatmap_grid(
    rows: &[&DailyOverviewRow],
    selected_day: &str,
) -> DashboardHeatmapGrid {
    let day_labels = rows
        .iter()
        .map(|row| row.day.get(5..10).unwrap_or(row.day.as_str()).to_owned())
        .collect::<Vec<_>>();
    let selected_col = rows.iter().position(|row| row.day == selected_day);

    DashboardHeatmapGrid {
        day_labels,
        rows: vec![
            rows.iter().map(|row| row.sleep_score).collect(),
            rows.iter().map(|row| row.readiness_score).collect(),
            rows.iter().map(|row| row.activity_score).collect(),
        ],
        selected_cell: selected_col.map(|column| (0, column)),
    }
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
) -> Vec<ExplainSupportingEvent> {
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();

    for event in filtered_events_for_day(snapshot, day, filters) {
        if seen.insert(event.context_event_id.clone()) {
            items.push(explain_supporting_event(day, day, event, selected_event_id));
        }
    }

    if let Some(previous_day) = previous_daily_day(snapshot, day) {
        for event in filtered_events_for_day(snapshot, &previous_day, filters) {
            if (event.context_event_id == selected_event_id.unwrap_or_default()
                || event_starts_after_hour(event, 18))
                && seen.insert(event.context_event_id.clone())
            {
                items.push(explain_supporting_event(
                    day,
                    &previous_day,
                    event,
                    selected_event_id,
                ));
            }
        }
    }

    items
}

fn explain_supporting_event(
    selected_day: &str,
    display_day: &str,
    event: &ContextEventRecord,
    selected_event_id: Option<&str>,
) -> ExplainSupportingEvent {
    ExplainSupportingEvent {
        family_label: overlay_family_label(event.family),
        headline: format!("{} {}", format_event_time(display_day, event), event.title),
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
        source_day: event.anchor_day.clone(),
        carried_forward: event.anchor_day != selected_day,
    }
}

fn explain_breadcrumb(selected_day: &str, supporting_events: &[ExplainSupportingEvent]) -> String {
    if let Some(event) = supporting_events.iter().find(|event| event.selected) {
        if event.carried_forward {
            return format!(
                "Day {selected_day} -> linked event carries forward from {}",
                event.source_day
            );
        }
        return format!("Day {selected_day} -> linked event stays in focus");
    }
    if let Some(event) = supporting_events.iter().find(|event| event.carried_forward) {
        return format!(
            "Day {selected_day} -> includes carryover context from {}",
            event.source_day
        );
    }
    format!(
        "Day {} -> {} local evidence item{}",
        selected_day,
        supporting_events.len(),
        if supporting_events.len() == 1 {
            ""
        } else {
            "s"
        }
    )
}

fn previous_daily_day(snapshot: &LiveSnapshot, day: &str) -> Option<String> {
    snapshot
        .daily_history
        .iter()
        .filter(|row| row.day.as_str() < day)
        .map(|row| row.day.clone())
        .next_back()
}

const fn overlay_filter_matches(filters: &OverlayFilterState, family: ContextEventFamily) -> bool {
    match family {
        ContextEventFamily::Workout => filters.workouts,
        ContextEventFamily::Tag | ContextEventFamily::EnhancedTag => filters.tags,
        ContextEventFamily::Session => filters.sessions,
    }
}

struct OverlayToggleDescriptor {
    label: &'static str,
    key_hint: &'static str,
    family: ContextEventFamily,
}

const fn overlay_toggle_count() -> usize {
    3
}

fn overlay_toggle_descriptor(index: usize) -> OverlayToggleDescriptor {
    match index.min(overlay_toggle_count().saturating_sub(1)) {
        0 => OverlayToggleDescriptor {
            label: "Workouts",
            key_hint: "w",
            family: ContextEventFamily::Workout,
        },
        1 => OverlayToggleDescriptor {
            label: "Tags",
            key_hint: "t",
            family: ContextEventFamily::EnhancedTag,
        },
        _ => OverlayToggleDescriptor {
            label: "Sessions",
            key_hint: "s",
            family: ContextEventFamily::Session,
        },
    }
}

fn timeline_window_preset_index(window_hours: u16) -> usize {
    TIMELINE_WINDOW_PRESETS
        .iter()
        .position(|preset| *preset == window_hours)
        .unwrap_or(TIMELINE_WINDOW_PRESETS.len().saturating_sub(1))
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

fn overlay_toggle_views(
    filters: &OverlayFilterState,
    selected_index: usize,
) -> Vec<OverlayToggleView> {
    (0..overlay_toggle_count())
        .map(|index| {
            let descriptor = overlay_toggle_descriptor(index);
            let enabled = match descriptor.family {
                ContextEventFamily::Workout => filters.workouts,
                ContextEventFamily::Tag | ContextEventFamily::EnhancedTag => filters.tags,
                ContextEventFamily::Session => filters.sessions,
            };
            OverlayToggleView {
                label: descriptor.label,
                key_hint: descriptor.key_hint,
                enabled,
                selected: index == selected_index.min(overlay_toggle_count().saturating_sub(1)),
            }
        })
        .collect()
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
            "{} trended toward {} ({}, n={}, confidence={})",
            relation_phrase(summary.relation_window),
            effect_direction_phrase(summary.effect_direction, summary.metric),
            signed_delta(summary.median_delta),
            summary.sample_count,
            data_sufficiency_label(summary.confidence),
        ),
        badges: vec!["Exploratory".to_owned(), "Trend-only".to_owned()],
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
        lines.push("Daily closeout has not been cached for this day yet.".to_owned());
    }

    if let Some(heartrate_day) = heartrate_day {
        if heartrate_day.points.is_empty() {
            lines.push("Heartrate samples have not been cached for this day yet.".to_owned());
        } else {
            let mean = heartrate_day
                .points
                .iter()
                .map(|point| f64::from(point.bpm))
                .sum::<f64>()
                / crate::numeric::usize_to_f64(heartrate_day.points.len());
            lines.push(format!(
                "Heartrate mean {} bpm across {} samples.",
                format_float(mean),
                heartrate_day.points.len()
            ));
        }
    } else {
        lines.push("Heartrate samples have not been cached for this day yet.".to_owned());
    }

    lines
}

fn explain_event_detail_lines(day: &str, event: &ContextEventRecord) -> Vec<String> {
    let mut lines = vec![
        format!("{} {}", overlay_family_label(event.family), event.title),
        format!("When: {}", format_event_time(day, event)),
    ];
    if let Some(subtype) = &event.subtype {
        lines.push(format!("Type: {subtype}"));
    }
    if let Some(intensity) = &event.intensity {
        lines.push(format!("Strength: {intensity}"));
    }
    if let Some(notes) = &event.notes {
        lines.push(format!("Notes: {notes}"));
    }
    if event.anchor_day != day {
        lines.push(format!("Carries over from: {}", event.anchor_day));
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
            "{} context is unavailable because the `{}` scope is missing.",
            kind.label(),
            kind.scope_name()
        )
    })
    .collect()
}

const fn insight_is_thin(insight: &MetricInsight) -> bool {
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
    days.into_iter().collect()
}

fn live_snapshot_day_candidates(
    daily_history: &[DailyOverviewRow],
    heartrate_days: &[HeartRateDay],
    context_events: &[ContextEventRecord],
    latest_review_day: Option<&str>,
) -> Vec<String> {
    let mut days = BTreeSet::new();
    for row in daily_history {
        days.insert(row.day.clone());
    }
    for day in heartrate_days {
        days.insert(day.day.clone());
    }
    for event in context_events {
        days.insert(event.anchor_day.clone());
    }
    if let Some(latest_review_day) = latest_review_day {
        days.insert(latest_review_day.to_owned());
    }
    days.into_iter().collect()
}

fn restored_day_index(day_labels: &[String], selected_day: &str) -> usize {
    if let Some(index) = day_labels.iter().position(|day| day == selected_day) {
        return index;
    }
    if let Some((index, _)) = day_labels
        .iter()
        .enumerate()
        .rev()
        .find(|(_, day)| day.as_str() < selected_day)
    {
        return index;
    }
    if let Some((index, _)) = day_labels
        .iter()
        .enumerate()
        .find(|(_, day)| day.as_str() > selected_day)
    {
        return index;
    }
    day_labels.len().saturating_sub(1)
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
    parse_timestamp(&event.start_at).is_some_and(|timestamp| timestamp.hour() >= hour)
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

const fn overlay_family_label(family: ContextEventFamily) -> &'static str {
    match family {
        ContextEventFamily::Workout => "Workout",
        ContextEventFamily::Tag | ContextEventFamily::EnhancedTag => "Tag",
        ContextEventFamily::Session => "Session",
    }
}

const fn overlay_family_glyph(family: ContextEventFamily) -> char {
    match family {
        ContextEventFamily::Workout => 'W',
        ContextEventFamily::Tag | ContextEventFamily::EnhancedTag => 'T',
        ContextEventFamily::Session => 'S',
    }
}

const fn relation_phrase(window: PatternRelationWindow) -> &'static str {
    match window {
        PatternRelationWindow::SameDayActivity => "same-day activity",
        PatternRelationWindow::NextDayReadiness => "next-day readiness",
        PatternRelationWindow::SameNightSleep => "same-night sleep",
    }
}

fn effect_direction_phrase(direction: EffectDirection, metric: PatternMetric) -> String {
    let metric_label = match metric {
        PatternMetric::Activity => "activity score",
        PatternMetric::Readiness => "readiness score",
        PatternMetric::Sleep => "sleep score",
    };
    match direction {
        EffectDirection::Higher => format!("higher {metric_label}"),
        EffectDirection::Lower => format!("lower {metric_label}"),
        EffectDirection::Flat => format!("flat {metric_label}"),
    }
}

fn prettify_key(value: &str) -> String {
    value.replace("::", " / ").replace('_', " ")
}

const fn data_sufficiency_label(value: crate::store::queries::DataSufficiency) -> &'static str {
    match value {
        crate::store::queries::DataSufficiency::Thin => "thin",
        crate::store::queries::DataSufficiency::Medium => "medium",
        crate::store::queries::DataSufficiency::Strong => "strong",
    }
}

fn signed_delta(value: f64) -> String {
    format!("{value:+.1}")
}

const fn toggle_state(enabled: bool) -> &'static str {
    if enabled { "on" } else { "off" }
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
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

fn window_sparkline(history: &[MetricPoint], days: usize) -> Vec<u64> {
    history
        .iter()
        .rev()
        .take(days)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|point| crate::numeric::rounded_nonnegative_f64_to_u64(point.value))
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

fn trim_date_time(value: &str) -> String {
    if value.len() >= 16 {
        format!("{} {}", &value[..10], &value[11..16])
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
    const fn label(self) -> &'static str {
        match self {
            Self::Personal => "Personal",
            Self::Daily => "Daily",
            Self::Heartrate => "Heartrate",
            Self::Workout => "Workouts",
            Self::EnhancedTag => "Enhanced Tags",
            Self::Session => "Sessions",
        }
    }

    const fn sync_key(self) -> &'static str {
        match self {
            Self::Personal => SyncFamily::Personal.sync_key(),
            Self::Daily => SyncFamily::Daily.sync_key(),
            Self::Heartrate => SyncFamily::Heartrate.sync_key(),
            Self::Workout => SyncFamily::Workout.sync_key(),
            Self::EnhancedTag => SyncFamily::EnhancedTag.sync_key(),
            Self::Session => SyncFamily::Session.sync_key(),
        }
    }

    const fn capability_kind(self) -> CapabilityKind {
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

impl CoverageFamily {
    const ALL: [Self; 6] = [
        Self::Daily,
        Self::Heartrate,
        Self::Workout,
        Self::Tag,
        Self::Session,
        Self::Spo2,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Heartrate => "heartrate",
            Self::Workout => "workout",
            Self::Tag => "tag",
            Self::Session => "session",
            Self::Spo2 => "spo2",
        }
    }

    const fn capability_kind(self) -> CapabilityKind {
        match self {
            Self::Daily => CapabilityKind::Daily,
            Self::Heartrate => CapabilityKind::Heartrate,
            Self::Workout => CapabilityKind::Workout,
            Self::Tag => CapabilityKind::Tag,
            Self::Session => CapabilityKind::Session,
            Self::Spo2 => CapabilityKind::Spo2,
        }
    }
}

impl RefreshPolicySnapshot {
    const fn from_config(config: &Config) -> Self {
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

    const fn stale_after_seconds(&self, family: DataFamily) -> u64 {
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
            dashboard: empty_dashboard_model(),
            timeline: empty_timeline_model(),
            trends: empty_trends_model(),
            explain: empty_explain_model(),
            patterns: empty_patterns_model(),
            ops: empty_ops_model(),
            review: empty_review_model(),
            ai: empty_ai_workbench_model(),
        }
    }
}

const fn empty_dashboard_model() -> DashboardModel {
    DashboardModel {
        header: HeaderStripModel {
            app_title: String::new(),
            selected_period: String::new(),
            freshness_badge: String::new(),
            sync_status: String::new(),
            capability_summary: Vec::new(),
            coverage: Vec::new(),
        },
        selected_day_label: String::new(),
        readiness: DashboardScoreTile {
            availability: TelemetryAvailability::NoData,
            primary_value: String::new(),
            secondary_lines: Vec::new(),
            delta_label: String::new(),
            trend: Vec::new(),
            ring_fill_percent: 0,
            note: String::new(),
        },
        sleep: DashboardSleepTile {
            availability: TelemetryAvailability::NoData,
            duration_label: String::new(),
            score_label: String::new(),
            trend: Vec::new(),
            strip_note: String::new(),
        },
        activity: DashboardScoreTile {
            availability: TelemetryAvailability::NoData,
            primary_value: String::new(),
            secondary_lines: Vec::new(),
            delta_label: String::new(),
            trend: Vec::new(),
            ring_fill_percent: 0,
            note: String::new(),
        },
        hrv: DashboardTrendPanel {
            availability: TelemetryAvailability::Unsupported,
            primary_label: String::new(),
            baseline_label: String::new(),
            range_label: String::new(),
            values: Vec::new(),
            note: String::new(),
        },
        body_temp: DashboardThermometerPanel {
            availability: TelemetryAvailability::NoData,
            deviation_tenths: None,
            value_label: String::new(),
            note: String::new(),
        },
        heart_rate: DashboardTrendPanel {
            availability: TelemetryAvailability::NoData,
            primary_label: String::new(),
            baseline_label: String::new(),
            range_label: String::new(),
            values: Vec::new(),
            note: String::new(),
        },
        respiratory_rate: DashboardHistogramPanel {
            availability: TelemetryAvailability::Unsupported,
            primary_label: String::new(),
            bars: Vec::new(),
            note: String::new(),
        },
        spo2: DashboardTrendPanel {
            availability: TelemetryAvailability::NoData,
            primary_label: String::new(),
            baseline_label: String::new(),
            range_label: String::new(),
            values: Vec::new(),
            note: String::new(),
        },
        breakdown: DashboardBreakdownPanel {
            availability: TelemetryAvailability::NoData,
            rails: Vec::new(),
            waveform: Vec::new(),
            note: String::new(),
        },
        weekly: DashboardWeeklyHeatmap {
            availability: TelemetryAvailability::NoData,
            row_labels: Vec::new(),
            recent: DashboardHeatmapGrid {
                day_labels: Vec::new(),
                rows: Vec::new(),
                selected_cell: None,
            },
            history: DashboardHeatmapGrid {
                day_labels: Vec::new(),
                rows: Vec::new(),
                selected_cell: None,
            },
            note: String::new(),
        },
    }
}

const fn empty_timeline_model() -> TimelineModel {
    TimelineModel {
        summary: String::new(),
        breadcrumb: String::new(),
        day_selector: String::new(),
        window_presets: Vec::new(),
        selected_window_preset_index: 0,
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
    }
}

const fn empty_trends_model() -> TrendsModel {
    TrendsModel {
        sort_tabs: Vec::new(),
        selected_sort_index: 0,
        rows: Vec::new(),
        notes: Vec::new(),
    }
}

const fn empty_explain_model() -> ExplainModel {
    ExplainModel {
        selected_day_label: String::new(),
        breadcrumb: String::new(),
        headline: String::new(),
        overlay_toggles: Vec::new(),
        selected_overlay_toggle_index: 0,
        claim_availability: TelemetryAvailability::NoData,
        summary_lines: Vec::new(),
        measurements_availability: TelemetryAvailability::NoData,
        evidence_badges: Vec::new(),
        measurement_lines: Vec::new(),
        evidence_availability: TelemetryAvailability::NoData,
        evidence_lines: Vec::new(),
        uncertainty_availability: TelemetryAvailability::NoData,
        caveat_lines: Vec::new(),
        context_availability: TelemetryAvailability::NoData,
        context_lines: Vec::new(),
        ai_availability: TelemetryAvailability::NoData,
        ai_actions: Vec::new(),
    }
}

const fn empty_patterns_model() -> PatternsModel {
    PatternsModel {
        header: String::new(),
        metric_filters: Vec::new(),
        selected_filter_index: 0,
        overlay_toggles: Vec::new(),
        selected_overlay_toggle_index: 0,
        filter_summary: String::new(),
        findings_availability: TelemetryAvailability::NoData,
        rows: Vec::new(),
        guide_availability: TelemetryAvailability::NoData,
        notes: Vec::new(),
        interpretation_availability: TelemetryAvailability::NoData,
        empty_message: String::new(),
        ai_actions: Vec::new(),
    }
}

const fn empty_ops_model() -> OpsModel {
    OpsModel {
        mode_label: String::new(),
        summary_lines: Vec::new(),
        coverage: Vec::new(),
        family_statuses: Vec::new(),
        items: Vec::new(),
        warnings: Vec::new(),
    }
}

fn empty_review_model() -> ReviewModel {
    ReviewModel {
        selected_day_label: String::new(),
        breadcrumb: String::new(),
        mode_tabs: Vec::new(),
        selected_mode_index: 0,
        focus_tabs: Vec::new(),
        selected_focus_index: 0,
        cards_availability: TelemetryAvailability::NoData,
        cards: Vec::new(),
        selected_card_index: None,
        ai_artifact: empty_ai_artifact_summary_view(),
        detail_availability: TelemetryAvailability::NoData,
        detail_lines: Vec::new(),
        warnings_availability: TelemetryAvailability::NoData,
        warning_lines: Vec::new(),
        empty_message: String::new(),
        ai_actions: Vec::new(),
    }
}

const fn empty_ai_workbench_model() -> AiWorkbenchModel {
    AiWorkbenchModel {
        headline: String::new(),
        summary_lines: Vec::new(),
        launch_points: Vec::new(),
        browser_tabs: Vec::new(),
        selected_tab_index: 0,
        browser_items: Vec::new(),
        selected_item_index: None,
        artifact_actions: Vec::new(),
        selected_action_index: None,
        detail_title: String::new(),
        detail_lines: Vec::new(),
        trust_lines: Vec::new(),
        warning_lines: Vec::new(),
        preflight: None,
    }
}

fn demo_snapshot(config: &Config) -> LiveSnapshot {
    let ai_fixture = demo_ai_fixture_data();
    let ai_ops = build_ai_ops_snapshot(
        config,
        &ai_fixture.snapshot_catalog,
        &ai_fixture.ai_runs,
        &ai_fixture.ai_artifact_records,
        &ai_fixture.report_exports,
        &ai_fixture.ai_eval_runs,
    );

    LiveSnapshot {
        captured_at: "2026-04-08T22:30:00Z".to_owned(),
        refresh_policy: demo_refresh_policy_snapshot(),
        auth_status: demo_auth_status(),
        active_population_profile: config.guidance.active_population_profile,
        guidance_profile_source: config.guidance.source_label().to_owned(),
        evidence_registry_version: evidence_registry_version().to_owned(),
        stale_evidence_entries: stale_evidence_warnings(OffsetDateTime::now_utc().date()),
        ai_ops,
        webhook: demo_webhook_snapshot(),
        personal_info: Some(demo_personal_info()),
        daily_history: demo_daily_history(),
        daily_activity: demo_daily_activity_records(),
        daily_readiness: demo_daily_readiness_records(),
        daily_stress: demo_daily_stress_records(),
        sleep_periods: demo_sleep_period_records(),
        daily_spo2: demo_daily_spo2_records(),
        heartrate_days: demo_heartrate_days(),
        heartrate_daily_averages: demo_heartrate_daily_averages(),
        context_events: demo_context_events(),
        pattern_summaries: demo_pattern_summaries(),
        review_signal_days: demo_review_signal_days(),
        sleep_time: demo_sleep_time_records(),
        rest_mode_periods: demo_rest_mode_periods(),
        daily_resilience: demo_daily_resilience_records(),
        daily_cardiovascular_age: demo_daily_cardiovascular_age_records(),
        vo2_max: demo_vo2_max_records(),
        ai_artifacts_by_day: demo_ai_artifacts_by_day(),
        snapshot_catalog: ai_fixture.snapshot_catalog,
        ai_runs: ai_fixture.ai_runs,
        ai_artifact_records: ai_fixture.ai_artifact_records,
        report_exports: ai_fixture.report_exports,
        ai_eval_runs: ai_fixture.ai_eval_runs,
        sync_states: demo_sync_states(),
        record_counts: demo_record_counts(),
        schema_version: crate::store::migrations::current_version(),
        database_path: "~/.local/share/ringmaster/demo/ringmaster.db".to_owned(),
        config_path: "~/.config/ringmaster/demo-config.toml".to_owned(),
    }
}

struct DemoAiFixtureData {
    snapshot_catalog: Vec<SnapshotCatalogEntry>,
    ai_runs: Vec<AiRunRecord>,
    ai_artifact_records: Vec<AiArtifactRecord>,
    report_exports: Vec<ReportExportRecord>,
    ai_eval_runs: Vec<AiEvalRunRecord>,
}

fn demo_auth_status() -> AuthStatus {
    let capability_report = CapabilityReport::demo();
    AuthStatus {
        configured: true,
        callback_url: "http://localhost:8788/callback".to_owned(),
        requested_scopes: demo_requested_scopes(),
        granted_scopes: demo_requested_scopes(),
        missing_fields: Vec::new(),
        capability_report,
        auth_timeout_secs: 120,
        secret_backend: "demo-memory".to_owned(),
        access_token_stored: true,
        refresh_token_stored: true,
        access_token_expires_at: Some("2026-04-08T08:00:00Z".to_owned()),
        last_authenticated_at: Some("2026-04-08T03:00:00Z".to_owned()),
        last_refresh_at: Some("2026-04-08T03:30:00Z".to_owned()),
        account_id: Some("demo-user".to_owned()),
        account_email: Some("demo@example.com".to_owned()),
        last_error: None,
    }
}

fn demo_daily_history() -> Vec<DailyOverviewRow> {
    vec![
        DailyOverviewRow {
            day: "2026-04-05".to_owned(),
            sleep_score: Some(82),
            sleep_duration_seconds: Some(27_000),
            readiness_score: Some(80),
            activity_score: Some(72),
            updated_at: "2026-04-05T10:00:00Z".to_owned(),
        },
        DailyOverviewRow {
            day: "2026-04-06".to_owned(),
            sleep_score: Some(84),
            sleep_duration_seconds: Some(27_600),
            readiness_score: Some(81),
            activity_score: Some(74),
            updated_at: "2026-04-06T10:00:00Z".to_owned(),
        },
        DailyOverviewRow {
            day: "2026-04-07".to_owned(),
            sleep_score: Some(80),
            sleep_duration_seconds: Some(26_400),
            readiness_score: Some(78),
            activity_score: Some(75),
            updated_at: "2026-04-07T10:00:00Z".to_owned(),
        },
        DailyOverviewRow {
            day: "2026-04-08".to_owned(),
            sleep_score: Some(76),
            sleep_duration_seconds: Some(24_900),
            readiness_score: Some(74),
            activity_score: Some(88),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
        },
    ]
}

fn demo_daily_activity_records() -> Vec<DailyActivityRecord> {
    vec![
        DailyActivityRecord {
            oura_id: Some("demo-activity-2026-04-05".to_owned()),
            day: "2026-04-05".to_owned(),
            activity_score: Some(72),
            active_calories: 392,
            steps: 8_420,
            total_calories: 2_121,
            raw_cache_key: Some("demo".to_owned()),
            updated_at: "2026-04-05T10:00:00Z".to_owned(),
        },
        DailyActivityRecord {
            oura_id: Some("demo-activity-2026-04-06".to_owned()),
            day: "2026-04-06".to_owned(),
            activity_score: Some(74),
            active_calories: 415,
            steps: 9_180,
            total_calories: 2_210,
            raw_cache_key: Some("demo".to_owned()),
            updated_at: "2026-04-06T10:00:00Z".to_owned(),
        },
        DailyActivityRecord {
            oura_id: Some("demo-activity-2026-04-07".to_owned()),
            day: "2026-04-07".to_owned(),
            activity_score: Some(75),
            active_calories: 438,
            steps: 9_860,
            total_calories: 2_284,
            raw_cache_key: Some("demo".to_owned()),
            updated_at: "2026-04-07T10:00:00Z".to_owned(),
        },
        DailyActivityRecord {
            oura_id: Some("demo-activity-2026-04-08".to_owned()),
            day: "2026-04-08".to_owned(),
            activity_score: Some(88),
            active_calories: 586,
            steps: 13_420,
            total_calories: 2_498,
            raw_cache_key: Some("demo".to_owned()),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
        },
    ]
}

fn demo_daily_readiness_records() -> Vec<DailyReadinessRecord> {
    vec![
        DailyReadinessRecord {
            oura_id: Some("demo-readiness-2026-04-05".to_owned()),
            day: "2026-04-05".to_owned(),
            readiness_score: Some(80),
            temperature_deviation: Some(-0.1),
            temperature_trend_deviation: Some(-0.1),
            raw_cache_key: Some("demo".to_owned()),
            updated_at: "2026-04-05T10:00:00Z".to_owned(),
        },
        DailyReadinessRecord {
            oura_id: Some("demo-readiness-2026-04-06".to_owned()),
            day: "2026-04-06".to_owned(),
            readiness_score: Some(81),
            temperature_deviation: Some(0.0),
            temperature_trend_deviation: Some(0.0),
            raw_cache_key: Some("demo".to_owned()),
            updated_at: "2026-04-06T10:00:00Z".to_owned(),
        },
        DailyReadinessRecord {
            oura_id: Some("demo-readiness-2026-04-07".to_owned()),
            day: "2026-04-07".to_owned(),
            readiness_score: Some(78),
            temperature_deviation: Some(0.1),
            temperature_trend_deviation: Some(0.1),
            raw_cache_key: Some("demo".to_owned()),
            updated_at: "2026-04-07T10:00:00Z".to_owned(),
        },
        DailyReadinessRecord {
            oura_id: Some("demo-readiness-2026-04-08".to_owned()),
            day: "2026-04-08".to_owned(),
            readiness_score: Some(74),
            temperature_deviation: Some(0.3),
            temperature_trend_deviation: Some(0.3),
            raw_cache_key: Some("demo".to_owned()),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
        },
    ]
}

fn demo_daily_stress_records() -> Vec<DailyStressRecord> {
    vec![
        DailyStressRecord {
            oura_id: Some("demo-stress-2026-04-05".to_owned()),
            day: "2026-04-05".to_owned(),
            stress_high: Some(90),
            recovery_high: Some(132),
            day_summary: Some("steady".to_owned()),
            raw_cache_key: Some("demo".to_owned()),
            updated_at: "2026-04-05T10:00:00Z".to_owned(),
        },
        DailyStressRecord {
            oura_id: Some("demo-stress-2026-04-06".to_owned()),
            day: "2026-04-06".to_owned(),
            stress_high: Some(96),
            recovery_high: Some(126),
            day_summary: Some("balanced".to_owned()),
            raw_cache_key: Some("demo".to_owned()),
            updated_at: "2026-04-06T10:00:00Z".to_owned(),
        },
        DailyStressRecord {
            oura_id: Some("demo-stress-2026-04-07".to_owned()),
            day: "2026-04-07".to_owned(),
            stress_high: Some(124),
            recovery_high: Some(118),
            day_summary: Some("strained".to_owned()),
            raw_cache_key: Some("demo".to_owned()),
            updated_at: "2026-04-07T10:00:00Z".to_owned(),
        },
        DailyStressRecord {
            oura_id: Some("demo-stress-2026-04-08".to_owned()),
            day: "2026-04-08".to_owned(),
            stress_high: Some(170),
            recovery_high: Some(92),
            day_summary: Some("elevated".to_owned()),
            raw_cache_key: Some("demo".to_owned()),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
        },
    ]
}

fn demo_heartrate_days() -> Vec<HeartRateDay> {
    vec![
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
    ]
}

fn demo_heartrate_daily_averages() -> Vec<MetricPoint> {
    vec![
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
    ]
}

fn demo_context_events() -> Vec<ContextEventRecord> {
    vec![
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
    ]
}

fn demo_pattern_summaries() -> Vec<PatternSummaryRecord> {
    vec![
        PatternSummaryRecord {
            summary_id: "pattern:run:readiness".to_owned(),
            family: ContextEventFamily::Workout,
            normalized_key: "running::moderate".to_owned(),
            relation_window: PatternRelationWindow::NextDayReadiness,
            metric: PatternMetric::Readiness,
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
            metric: PatternMetric::Sleep,
            sample_count: 4,
            median_delta: -3.0,
            effect_direction: EffectDirection::Lower,
            confidence: crate::store::queries::DataSufficiency::Thin,
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-08T22:00:00Z".to_owned(),
        },
    ]
}

fn demo_review_signal_days() -> Vec<ReviewSignalDayRecord> {
    vec![
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
    ]
}

fn demo_sleep_time_records() -> Vec<SleepTimeRecord> {
    vec![SleepTimeRecord {
        oura_id: Some("demo-sleep-time".to_owned()),
        day: "2026-04-08".to_owned(),
        status: Some("late".to_owned()),
        recommendation: Some("earlier_bedtime".to_owned()),
        optimal_bedtime_start_offset: Some(77400),
        optimal_bedtime_end_offset: Some(81000),
        optimal_bedtime_day_tz: Some(0),
        raw_cache_key: None,
        updated_at: "2026-04-08T22:00:00Z".to_owned(),
    }]
}

fn demo_sleep_period_records() -> Vec<SleepPeriodRecord> {
    vec![
        SleepPeriodRecord {
            oura_id: "demo-sleep-20260405".to_owned(),
            day: "2026-04-05".to_owned(),
            bedtime_start: Some("2026-04-04T23:12:00Z".to_owned()),
            bedtime_end: Some("2026-04-05T06:48:00Z".to_owned()),
            sleep_type: Some("long_sleep".to_owned()),
            average_heart_rate: Some(56.0),
            average_hrv: Some(39.0),
            average_breath: Some(14.3),
            total_sleep_duration: Some(27_360),
            raw_cache_key: None,
            updated_at: "2026-04-05T07:10:00Z".to_owned(),
        },
        SleepPeriodRecord {
            oura_id: "demo-sleep-20260406".to_owned(),
            day: "2026-04-06".to_owned(),
            bedtime_start: Some("2026-04-05T23:25:00Z".to_owned()),
            bedtime_end: Some("2026-04-06T06:41:00Z".to_owned()),
            sleep_type: Some("long_sleep".to_owned()),
            average_heart_rate: Some(57.0),
            average_hrv: Some(41.0),
            average_breath: Some(14.0),
            total_sleep_duration: Some(26_160),
            raw_cache_key: None,
            updated_at: "2026-04-06T07:05:00Z".to_owned(),
        },
        SleepPeriodRecord {
            oura_id: "demo-sleep-20260407".to_owned(),
            day: "2026-04-07".to_owned(),
            bedtime_start: Some("2026-04-06T23:58:00Z".to_owned()),
            bedtime_end: Some("2026-04-07T06:35:00Z".to_owned()),
            sleep_type: Some("long_sleep".to_owned()),
            average_heart_rate: Some(58.0),
            average_hrv: Some(37.0),
            average_breath: Some(14.6),
            total_sleep_duration: Some(23_820),
            raw_cache_key: None,
            updated_at: "2026-04-07T06:58:00Z".to_owned(),
        },
        SleepPeriodRecord {
            oura_id: "demo-sleep-20260408".to_owned(),
            day: "2026-04-08".to_owned(),
            bedtime_start: Some("2026-04-07T23:47:00Z".to_owned()),
            bedtime_end: Some("2026-04-08T06:19:00Z".to_owned()),
            sleep_type: Some("long_sleep".to_owned()),
            average_heart_rate: Some(59.0),
            average_hrv: Some(34.0),
            average_breath: Some(15.1),
            total_sleep_duration: Some(23_520),
            raw_cache_key: None,
            updated_at: "2026-04-08T06:44:00Z".to_owned(),
        },
    ]
}

fn demo_daily_spo2_records() -> Vec<DailySpO2Record> {
    vec![
        DailySpO2Record {
            oura_id: Some("demo-spo2-20260405".to_owned()),
            day: "2026-04-05".to_owned(),
            average_spo2: Some(97.8),
            breathing_disturbance_index: Some(0.4),
            raw_cache_key: None,
            updated_at: "2026-04-05T07:10:00Z".to_owned(),
        },
        DailySpO2Record {
            oura_id: Some("demo-spo2-20260406".to_owned()),
            day: "2026-04-06".to_owned(),
            average_spo2: Some(98.0),
            breathing_disturbance_index: Some(0.3),
            raw_cache_key: None,
            updated_at: "2026-04-06T07:05:00Z".to_owned(),
        },
        DailySpO2Record {
            oura_id: Some("demo-spo2-20260407".to_owned()),
            day: "2026-04-07".to_owned(),
            average_spo2: Some(97.4),
            breathing_disturbance_index: Some(0.6),
            raw_cache_key: None,
            updated_at: "2026-04-07T06:58:00Z".to_owned(),
        },
        DailySpO2Record {
            oura_id: Some("demo-spo2-20260408".to_owned()),
            day: "2026-04-08".to_owned(),
            average_spo2: Some(97.1),
            breathing_disturbance_index: Some(0.8),
            raw_cache_key: None,
            updated_at: "2026-04-08T06:44:00Z".to_owned(),
        },
    ]
}

fn demo_daily_resilience_records() -> Vec<DailyResilienceRecord> {
    vec![DailyResilienceRecord {
        oura_id: Some("demo-resilience-20260408".to_owned()),
        day: "2026-04-08".to_owned(),
        level: "adequate".to_owned(),
        sleep_recovery: 0.61,
        daytime_recovery: 0.58,
        stress: 0.44,
        raw_cache_key: None,
        updated_at: "2026-04-08T22:00:00Z".to_owned(),
    }]
}

fn demo_daily_cardiovascular_age_records() -> Vec<DailyCardiovascularAgeRecord> {
    vec![DailyCardiovascularAgeRecord {
        day: "2026-04-08".to_owned(),
        vascular_age: Some(32),
        raw_cache_key: None,
        updated_at: "2026-04-08T22:00:00Z".to_owned(),
    }]
}

fn demo_vo2_max_records() -> Vec<Vo2MaxRecord> {
    vec![Vo2MaxRecord {
        oura_id: Some("demo-vo2max-20260408".to_owned()),
        day: "2026-04-08".to_owned(),
        recorded_at: "2026-04-08T22:00:00Z".to_owned(),
        vo2_max: Some(44.2),
        raw_cache_key: None,
        updated_at: "2026-04-08T22:00:00Z".to_owned(),
    }]
}

fn demo_rest_mode_periods() -> Vec<RestModePeriodRecord> {
    vec![RestModePeriodRecord {
        period_id: "demo-rest-mode".to_owned(),
        start_day: "2026-04-07".to_owned(),
        start_time: Some("2026-04-07T00:00:00Z".to_owned()),
        end_day: Some("2026-04-08".to_owned()),
        end_time: Some("2026-04-08T08:00:00Z".to_owned()),
        episode_count: 1,
        tags_json: "[]".to_owned(),
        raw_cache_key: None,
        updated_at: "2026-04-08T08:00:00Z".to_owned(),
    }]
}

fn demo_webhook_snapshot() -> WebhookOpsSnapshot {
    WebhookOpsSnapshot {
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
    }
}

fn demo_personal_info() -> PersonalInfoRecord {
    PersonalInfoRecord {
        profile_id: "demo-user".to_owned(),
        age: Some(34),
        weight: Some(72.4),
        height: Some(178.0),
        biological_sex: Some("male".to_owned()),
        email: Some("demo@example.com".to_owned()),
        raw_cache_key: Some("demo".to_owned()),
        updated_at: "2026-04-08T22:00:00Z".to_owned(),
    }
}

fn demo_ai_artifacts_by_day() -> BTreeMap<String, AiArtifactDaySummaryRecord> {
    BTreeMap::from([(
        "2026-04-08".to_owned(),
        AiArtifactDaySummaryRecord {
            artifact_id: "run-demo-review-20260408".to_owned(),
            artifact_kind: "review".to_owned(),
            created_at: "2026-04-08T22:20:00Z".to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-4o-2024-08-06".to_owned(),
            prompt_version: "review_prompt_v1".to_owned(),
            output_schema_version: "ringmaster.ai.review.v3".to_owned(),
            privacy_profile: "redacted".to_owned(),
            summary_cache:
                "Sleep debt and elevated stress likely drove the readiness dip.".to_owned(),
            overview:
                "Workout load held up, but the bedtime drift means the saved review still recommends an earlier wind-down tonight."
                    .to_owned(),
            matched_snapshot_hash: "demo-snapshot-20260408".to_owned(),
            peer_snapshot_hash: None,
        },
    )])
}

fn demo_sync_states() -> Vec<SyncStateRecord> {
    vec![
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
    ]
}

const fn demo_record_counts() -> RecordCounts {
    RecordCounts {
        raw_payloads: 12,
        personal_info: 1,
        daily_sleep: 4,
        sleep_periods: 4,
        daily_readiness: 4,
        daily_activity: 4,
        daily_spo2: 4,
        heartrate_samples: 9,
        workouts: 1,
        tags: 0,
        enhanced_tags: 1,
        sessions: 1,
        derived_context_events: 3,
        derived_pattern_summaries: 2,
        sleep_time: 1,
        daily_stress: 1,
        daily_resilience: 1,
        daily_cardiovascular_age: 1,
        vo2_max: 1,
        rest_mode_periods: 1,
        derived_review_signal_days: 4,
    }
}

fn demo_ai_fixture_data() -> DemoAiFixtureData {
    let review_preview = demo_review_preview();
    let compare_preview = demo_compare_preview();
    let snapshot_catalog = demo_snapshot_catalog();
    DemoAiFixtureData {
        ai_runs: demo_ai_runs(&review_preview, &compare_preview),
        ai_artifact_records: vec![demo_review_artifact()],
        report_exports: demo_report_exports(),
        ai_eval_runs: demo_ai_eval_runs(),
        snapshot_catalog,
    }
}

fn demo_review_preview() -> AiRequestPreview {
    AiRequestPreview {
        task_family: "review".to_owned(),
        provider: "openai".to_owned(),
        model: "gpt-5-mini".to_owned(),
        request_mode: "stateless".to_owned(),
        input_transport: "inline".to_owned(),
        prompt_cache: "auto".to_owned(),
        prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
        output_schema_version: "ringmaster.ai.review.v3".to_owned(),
        snapshots: vec![AiRequestPreviewSnapshot {
            label: "primary".to_owned(),
            snapshot_hash: "demo-snapshot-20260408".to_owned(),
            scope: "day:2026-04-08".to_owned(),
            anchor_day: "2026-04-08".to_owned(),
            privacy_profile: PrivacyProfile::Redacted,
            active_population_profile: PopulationProfile::GeneralAdult,
            day_count: 1,
        }],
        snapshot_bytes: 52_000,
        approximate_input_tokens: 13_000,
        stateless: true,
        tools_disabled: true,
        includes_notes_or_free_text: true,
        content_classes: vec![
            "summary".to_owned(),
            "review_signals".to_owned(),
            "follow_up_targets".to_owned(),
        ],
        prefix_fingerprint: "demo-review-prefix".to_owned(),
        payload_fingerprint: "demo-review-payload".to_owned(),
        request_fingerprint: "demo-review-request".to_owned(),
    }
}

fn demo_review_artifact() -> AiArtifactRecord {
    AiArtifactRecord {
        artifact_id: "run-demo-review-20260408".to_owned(),
        artifact_kind: "review".to_owned(),
        artifact_status: "dry_run".to_owned(),
        provider: "openai".to_owned(),
        model: "gpt-5-mini".to_owned(),
        reasoning_effort: Some("medium".to_owned()),
        request_mode: "stateless".to_owned(),
        input_transport: "inline".to_owned(),
        run_mode: "dry_run".to_owned(),
        prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
        output_schema_version: "ringmaster.ai.review.v3".to_owned(),
        created_at: "2026-04-08T22:20:00Z".to_owned(),
        snapshot_hash_a: "demo-snapshot-20260408".to_owned(),
        snapshot_hash_b: None,
        privacy_profile: "redacted".to_owned(),
        overview: "Sleep debt and elevated stress likely drove the readiness dip.".to_owned(),
        summary_cache:
            "Saved review: bedtime drift plus higher stress load explained the weaker morning readiness."
                .to_owned(),
        request_fingerprint: Some("demo-review-request".to_owned()),
        payload_json: serialize_pretty_json(&ai::ReviewArtifactV1 {
            schema_version: "ringmaster.ai.review.v3".to_owned(),
            prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
            status: ai::ArtifactStatus::DryRun,
            overview: "Sleep debt and elevated stress likely drove the readiness dip.".to_owned(),
            headline_findings: vec![ai::ArtifactFinding {
                finding_id: "sleep-drift".to_owned(),
                title: "Bedtime drift undercut readiness".to_owned(),
                summary:
                    "The selected day closed later than the surrounding baseline and the saved review linked that drift to weaker next-morning readiness."
                        .to_owned(),
                claim_key: Some("sleep_time_status".to_owned()),
                evidence_tier: Some(crate::evidence::registry::EvidenceTier::EvidenceInformed),
                interpretation_scope: Some(
                    crate::evidence::registry::InterpretationScope::WithinPersonTrendOnly,
                ),
                active_population_profile: Some(PopulationProfile::GeneralAdult),
                population_support_status: Some(
                    crate::evidence::registry::PopulationSupportStatus::PopulationSpecific,
                ),
                fallback_population_profile: None,
                caution_labels: evidence_badges("sleep_time_status", PopulationProfile::GeneralAdult),
                confidence: ai::ConfidenceLevel::Medium,
                sufficiency: ai::SufficiencyLevel::Medium,
                evidence_refs: vec![ai::ArtifactEvidenceRef {
                    export_ref: "sleep_time:2026-04-08".to_owned(),
                    note: "Late sleep window".to_owned(),
                }],
                counterevidence_refs: vec![ai::ArtifactEvidenceRef {
                    export_ref: "daily_activity:2026-04-08".to_owned(),
                    note: "Activity held steady, so the downturn was not purely load-driven."
                        .to_owned(),
                }],
            }],
            positive_findings: Vec::new(),
            negative_findings: vec![ai::ArtifactFinding {
                finding_id: "stress".to_owned(),
                title: "Stress remained elevated".to_owned(),
                summary: "The saved run still flags stress carryover as a compounding factor."
                    .to_owned(),
                claim_key: Some("stress_high".to_owned()),
                evidence_tier: Some(crate::evidence::registry::EvidenceTier::Exploratory),
                interpretation_scope: Some(
                    crate::evidence::registry::InterpretationScope::WithinPersonTrendOnly,
                ),
                active_population_profile: Some(PopulationProfile::GeneralAdult),
                population_support_status: Some(
                    crate::evidence::registry::PopulationSupportStatus::PopulationSpecific,
                ),
                fallback_population_profile: None,
                caution_labels: evidence_badges("stress_high", PopulationProfile::GeneralAdult),
                confidence: ai::ConfidenceLevel::Medium,
                sufficiency: ai::SufficiencyLevel::Thin,
                evidence_refs: vec![ai::ArtifactEvidenceRef {
                    export_ref: "daily_stress:2026-04-08".to_owned(),
                    note: "Stress score remained soft.".to_owned(),
                }],
                counterevidence_refs: Vec::new(),
            }],
            unresolved_questions: vec![
                "Would an earlier wind-down reverse the readiness dip over the next three days?"
                    .to_owned(),
            ],
            limitations: vec![ai::ArtifactLimitation {
                code: "thin_window".to_owned(),
                message:
                    "The review only had one directly comparable late bedtime in the recent window."
                        .to_owned(),
            }],
            follow_up_targets: vec![ai::ArtifactFollowUpTarget {
                label: "Expand evidence".to_owned(),
                command: "ai follow-up expand-evidence".to_owned(),
                reason: "Show the strongest supporting export refs before rerunning.".to_owned(),
            }],
        }),
        rendered_briefing:
            "ringmaster ai review\n\nstatus: dry_run\noverview: Sleep debt and elevated stress likely drove the readiness dip."
                .to_owned(),
    }
}

fn demo_compare_preview() -> AiRequestPreview {
    AiRequestPreview {
        task_family: "compare".to_owned(),
        provider: "openai".to_owned(),
        model: "gpt-5-mini".to_owned(),
        request_mode: "stateless".to_owned(),
        input_transport: "inline".to_owned(),
        prompt_cache: "auto".to_owned(),
        prompt_version: COMPARE_PROMPT_VERSION.to_owned(),
        output_schema_version: "ringmaster.ai.compare.v3".to_owned(),
        snapshots: vec![
            AiRequestPreviewSnapshot {
                label: "snapshot_a".to_owned(),
                snapshot_hash: "demo-snapshot-20260408".to_owned(),
                scope: "day:2026-04-08".to_owned(),
                anchor_day: "2026-04-08".to_owned(),
                privacy_profile: PrivacyProfile::Redacted,
                active_population_profile: PopulationProfile::GeneralAdult,
                day_count: 1,
            },
            AiRequestPreviewSnapshot {
                label: "snapshot_b".to_owned(),
                snapshot_hash: "demo-snapshot-20260401-20260408".to_owned(),
                scope: "week".to_owned(),
                anchor_day: "2026-04-08".to_owned(),
                privacy_profile: PrivacyProfile::Redacted,
                active_population_profile: PopulationProfile::GeneralAdult,
                day_count: 7,
            },
        ],
        snapshot_bytes: 96_000,
        approximate_input_tokens: 24_000,
        stateless: true,
        tools_disabled: true,
        includes_notes_or_free_text: true,
        content_classes: vec![
            "summary".to_owned(),
            "findings".to_owned(),
            "follow_up_targets".to_owned(),
        ],
        prefix_fingerprint: "demo-compare-prefix".to_owned(),
        payload_fingerprint: "demo-compare-payload".to_owned(),
        request_fingerprint: "demo-compare-request".to_owned(),
    }
}

fn demo_snapshot_catalog() -> Vec<SnapshotCatalogEntry> {
    vec![
        SnapshotCatalogEntry {
            snapshot_hash: "demo-snapshot-20260408".to_owned(),
            schema_version: "ringmaster.snapshot.v3".to_owned(),
            generated_at: "2026-04-08T22:18:00Z".to_owned(),
            scope: "day:2026-04-08".to_owned(),
            start_day: "2026-04-08".to_owned(),
            end_day: "2026-04-08".to_owned(),
            anchor_day: "2026-04-08".to_owned(),
            day_count: 1,
            privacy_profile: "redacted".to_owned(),
            source_mode: "demo".to_owned(),
            fixture_dir: None,
            latest_source_day: Some("2026-04-08".to_owned()),
            latest_review_day: Some("2026-04-08".to_owned()),
            freshness_summary: "current day with local sync coverage".to_owned(),
            trust_summary: "explicit snapshot export, redacted profile".to_owned(),
            capability_summary: "personal,daily,heartrate,workout,tag,session".to_owned(),
            provenance_summary: "sleep_time + review_signal + context refs".to_owned(),
            created_at: "2026-04-08T22:18:00Z".to_owned(),
        },
        SnapshotCatalogEntry {
            snapshot_hash: "demo-snapshot-20260401-20260408".to_owned(),
            schema_version: "ringmaster.snapshot.v3".to_owned(),
            generated_at: "2026-04-08T22:19:00Z".to_owned(),
            scope: "week".to_owned(),
            start_day: "2026-04-02".to_owned(),
            end_day: "2026-04-08".to_owned(),
            anchor_day: "2026-04-08".to_owned(),
            day_count: 7,
            privacy_profile: "redacted".to_owned(),
            source_mode: "demo".to_owned(),
            fixture_dir: None,
            latest_source_day: Some("2026-04-08".to_owned()),
            latest_review_day: Some("2026-04-08".to_owned()),
            freshness_summary: "seven-day comparison window".to_owned(),
            trust_summary: "explicit snapshot export, redacted profile".to_owned(),
            capability_summary: "daily + heartrate + contextual overlays".to_owned(),
            provenance_summary: "review refs + pattern summaries".to_owned(),
            created_at: "2026-04-08T22:19:00Z".to_owned(),
        },
    ]
}

fn demo_ai_runs(
    review_preview: &AiRequestPreview,
    compare_preview: &AiRequestPreview,
) -> Vec<AiRunRecord> {
    vec![
        AiRunRecord {
            run_id: "airun-demo-review-20260408".to_owned(),
            run_kind: "review".to_owned(),
            run_status: "succeeded".to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-5-mini".to_owned(),
            reasoning_effort: Some("medium".to_owned()),
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            run_mode: "dry_run".to_owned(),
            prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
            output_schema_version: "ringmaster.ai.review.v3".to_owned(),
            privacy_profile: "redacted".to_owned(),
            snapshot_scope: "day:2026-04-08".to_owned(),
            snapshot_hash_a: "demo-snapshot-20260408".to_owned(),
            snapshot_hash_b: None,
            source_ai_artifact_id: None,
            follow_up_kind: None,
            request_fingerprint: Some("demo-review-request".to_owned()),
            request_preview_json: serialize_json(review_preview),
            artifact_id: Some("run-demo-review-20260408".to_owned()),
            error_message: None,
            created_at: "2026-04-08T22:20:00Z".to_owned(),
            started_at: Some("2026-04-08T22:20:00Z".to_owned()),
            ended_at: Some("2026-04-08T22:20:04Z".to_owned()),
            updated_at: "2026-04-08T22:20:04Z".to_owned(),
        },
        AiRunRecord {
            run_id: "airun-demo-compare-20260408".to_owned(),
            run_kind: "compare".to_owned(),
            run_status: "failed".to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-5-mini".to_owned(),
            reasoning_effort: Some("medium".to_owned()),
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            run_mode: "dry_run".to_owned(),
            prompt_version: COMPARE_PROMPT_VERSION.to_owned(),
            output_schema_version: "ringmaster.ai.compare.v3".to_owned(),
            privacy_profile: "redacted".to_owned(),
            snapshot_scope: "week".to_owned(),
            snapshot_hash_a: "demo-snapshot-20260408".to_owned(),
            snapshot_hash_b: Some("demo-snapshot-20260401-20260408".to_owned()),
            source_ai_artifact_id: None,
            follow_up_kind: Some("explain_ranking".to_owned()),
            request_fingerprint: Some("demo-compare-request".to_owned()),
            request_preview_json: serialize_json(compare_preview),
            artifact_id: None,
            error_message: Some("Provider disabled in this deterministic fixture.".to_owned()),
            created_at: "2026-04-08T22:24:00Z".to_owned(),
            started_at: Some("2026-04-08T22:24:00Z".to_owned()),
            ended_at: Some("2026-04-08T22:24:02Z".to_owned()),
            updated_at: "2026-04-08T22:24:02Z".to_owned(),
        },
    ]
}

fn demo_report_exports() -> Vec<ReportExportRecord> {
    vec![ReportExportRecord {
        report_id: "demo-report-review".to_owned(),
        report_kind: "ai_review".to_owned(),
        title: "Daily review briefing".to_owned(),
        format: "markdown".to_owned(),
        output_path: "/tmp/ringmaster-demo-review.md".to_owned(),
        content_hash: "report-hash-demo-review".to_owned(),
        privacy_profile: "redacted".to_owned(),
        created_at: "2026-04-08T22:26:00Z".to_owned(),
        source_snapshot_hash_a: Some("demo-snapshot-20260408".to_owned()),
        source_snapshot_hash_b: None,
        source_ai_artifact_id: Some("run-demo-review-20260408".to_owned()),
        provider: Some("openai".to_owned()),
        model: Some("gpt-5-mini".to_owned()),
        prompt_version: Some(REVIEW_PROMPT_VERSION.to_owned()),
        output_schema_version: Some("ringmaster.ai.review.v2".to_owned()),
        export_status: "written".to_owned(),
        last_verified_exists: true,
        last_verified_at: "2026-04-08T22:26:00Z".to_owned(),
    }]
}

fn demo_ai_eval_runs() -> Vec<AiEvalRunRecord> {
    let demo_eval_details = demo_eval_run_details();
    vec![AiEvalRunRecord {
        eval_run_id: "demo-eval-review".to_owned(),
        task_family: "mixed".to_owned(),
        fixture_dir: demo_eval_details.fixture_dir.clone(),
        candidate_label: demo_eval_details.candidate_label.clone(),
        baseline_label: demo_eval_details.baseline_label.clone(),
        provider: "openai".to_owned(),
        model: "gpt-5-mini".to_owned(),
        prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
        output_schema_version: "mixed".to_owned(),
        created_at: "2026-04-08T22:27:00Z".to_owned(),
        total_cases: u32::try_from(demo_eval_details.total_cases).unwrap_or(u32::MAX),
        passed_cases: u32::try_from(demo_eval_details.passed_cases).unwrap_or(u32::MAX),
        failed_cases: u32::try_from(demo_eval_details.failed_cases).unwrap_or(u32::MAX),
        schema_validity_score: demo_eval_details.scores.schema_validity,
        completeness_score: demo_eval_details.scores.completeness,
        overclaiming_score: 0.98,
        medical_safety_score: 1.0,
        privacy_score: 1.0,
        evidence_score: demo_eval_details.scores.evidence,
        honesty_score: demo_eval_details.scores.honesty,
        regression_summary: demo_eval_details.regression_summary.clone(),
        details_json: serialize_json(&demo_eval_details),
    }]
}

fn demo_eval_run_details() -> PersistedEvalRunDetails {
    PersistedEvalRunDetails {
        fixture_dir: "tests/fixtures/ai".to_owned(),
        fixture_schema_version: "ringmaster.ai.eval.fixtures.v1".to_owned(),
        candidate_label: "gpt-5-mini".to_owned(),
        baseline_label: Some("fixture".to_owned()),
        total_cases: 2,
        passed_cases: 1,
        failed_cases: 1,
        scores: EvalScoreSummary {
            schema_validity: 1.0,
            completeness: 1.0,
            overclaiming: 1.0,
            medical_safety: 1.0,
            privacy: 1.0,
            evidence: 0.5,
            honesty: 1.0,
        },
        regression_summary:
            "Improvements: review-stale-snapshot:honesty; regressions: compare-evidence-regression:evidence."
                .to_owned(),
        improvements: vec!["review-stale-snapshot:honesty".to_owned()],
        regressions: vec!["compare-evidence-regression:evidence".to_owned()],
        cases: vec![demo_eval_review_case(), demo_eval_compare_case()],
    }
}

fn demo_eval_review_case() -> PersistedEvalCaseDetail {
    PersistedEvalCaseDetail {
        case_id: "review-stale-snapshot".to_owned(),
        task_family: "review".to_owned(),
        snapshot_a_path: "review-snapshot.json".to_owned(),
        snapshot_b_path: None,
        snapshot_hash_a: Some("demo-snapshot-20260408".to_owned()),
        snapshot_hash_b: None,
        expectations: EvalExpectations {
            min_primary_findings: Some(1),
            expected_primary_title: Some("Sleep score remained elevated".to_owned()),
            required_substrings: Vec::new(),
            forbidden_substrings: vec![
                "user@example.com".to_owned(),
                "refresh_token".to_owned(),
                "client_secret".to_owned(),
            ],
            expected_follow_up_commands: Vec::new(),
            require_distinct_finding_titles: false,
            honesty_required: true,
        },
        overall_pass: true,
        candidate: PersistedEvalArtifactDetail {
            label: "gpt-5-mini".to_owned(),
            artifact_path: "review-candidate.json".to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-5-mini".to_owned(),
            prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
            output_schema_version: "ringmaster.ai.review.v3".to_owned(),
            lineage: EvalArtifactLineage {
                ai_run_id: Some("airun-demo-review-20260408".to_owned()),
                ai_artifact_id: Some("run-demo-review-20260408".to_owned()),
                report_id: Some("demo-report-review".to_owned()),
            },
        },
        baseline: Some(PersistedEvalArtifactDetail {
            label: "fixture".to_owned(),
            artifact_path: "review-baseline.json".to_owned(),
            provider: "fixture".to_owned(),
            model: "fixture".to_owned(),
            prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
            output_schema_version: "ringmaster.ai.review.v3".to_owned(),
            lineage: EvalArtifactLineage::default(),
        }),
        graders: vec![
            PersistedEvalGraderResult {
                grader: "schema_validity".to_owned(),
                candidate_passed: true,
                candidate_note: "matched schema `ringmaster.ai.review.v2`".to_owned(),
                baseline_passed: Some(true),
                baseline_note: Some("matched schema `ringmaster.ai.review.v2`".to_owned()),
                comparison: "matched".to_owned(),
            },
            PersistedEvalGraderResult {
                grader: "honesty".to_owned(),
                candidate_passed: true,
                candidate_note: "artifact acknowledged freshness or capability limits".to_owned(),
                baseline_passed: Some(false),
                baseline_note: Some(
                    "artifact did not acknowledge stale or missing-data caveats".to_owned(),
                ),
                comparison: "improved".to_owned(),
            },
        ],
    }
}

fn demo_eval_compare_case() -> PersistedEvalCaseDetail {
    PersistedEvalCaseDetail {
        case_id: "compare-evidence-regression".to_owned(),
        task_family: "compare".to_owned(),
        snapshot_a_path: "compare-snapshot-a.json".to_owned(),
        snapshot_b_path: Some("compare-snapshot-b.json".to_owned()),
        snapshot_hash_a: Some("demo-snapshot-20260408".to_owned()),
        snapshot_hash_b: Some("demo-snapshot-20260401-20260408".to_owned()),
        expectations: EvalExpectations {
            min_primary_findings: Some(1),
            expected_primary_title: Some("Average daily score increased".to_owned()),
            required_substrings: Vec::new(),
            forbidden_substrings: vec![
                "user@example.com".to_owned(),
                "refresh_token".to_owned(),
                "client_secret".to_owned(),
            ],
            expected_follow_up_commands: Vec::new(),
            require_distinct_finding_titles: false,
            honesty_required: false,
        },
        overall_pass: false,
        candidate: PersistedEvalArtifactDetail {
            label: "gpt-5-mini".to_owned(),
            artifact_path: "compare-candidate.json".to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-5-mini".to_owned(),
            prompt_version: COMPARE_PROMPT_VERSION.to_owned(),
            output_schema_version: "ringmaster.ai.compare.v3".to_owned(),
            lineage: EvalArtifactLineage {
                ai_run_id: Some("airun-demo-compare-20260408".to_owned()),
                ai_artifact_id: None,
                report_id: None,
            },
        },
        baseline: Some(PersistedEvalArtifactDetail {
            label: "fixture".to_owned(),
            artifact_path: "compare-baseline.json".to_owned(),
            provider: "fixture".to_owned(),
            model: "fixture".to_owned(),
            prompt_version: COMPARE_PROMPT_VERSION.to_owned(),
            output_schema_version: "ringmaster.ai.compare.v3".to_owned(),
            lineage: EvalArtifactLineage::default(),
        }),
        graders: vec![
            PersistedEvalGraderResult {
                grader: "schema_validity".to_owned(),
                candidate_passed: true,
                candidate_note: "matched schema `ringmaster.ai.compare.v2`".to_owned(),
                baseline_passed: Some(true),
                baseline_note: Some("matched schema `ringmaster.ai.compare.v2`".to_owned()),
                comparison: "matched".to_owned(),
            },
            PersistedEvalGraderResult {
                grader: "evidence".to_owned(),
                candidate_passed: false,
                candidate_note: "missing evidence reference `stress:2026-04-08`".to_owned(),
                baseline_passed: Some(true),
                baseline_note: Some("validated 3 evidence references".to_owned()),
                comparison: "regressed".to_owned(),
            },
        ],
    }
}

fn demo_requested_scopes() -> Vec<String> {
    vec![
        "email".to_owned(),
        "personal".to_owned(),
        "daily".to_owned(),
        "heartrate".to_owned(),
        "tag".to_owned(),
        "workout".to_owned(),
        "session".to_owned(),
        "spo2".to_owned(),
        "ring_configuration".to_owned(),
        "stress".to_owned(),
        "heart_health".to_owned(),
    ]
}

const fn demo_refresh_policy_snapshot() -> RefreshPolicySnapshot {
    RefreshPolicySnapshot {
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
mod tests {
    use std::collections::BTreeMap;

    use super::{
        AiBrowserTab, AiLaunchIntent, AiOpsSnapshot, AiPreflightState, AppState,
        COMPARE_PROMPT_VERSION, DataFamily, HeartRateDay, LiveModelOptions, LiveSnapshot,
        OverlayFilterState, PatternMetricFilter, REVIEW_PROMPT_VERSION, RefreshPolicySnapshot,
        ReviewScreenMode, RunMode, Screen, TrendSortMode, TrendWindowKind, WebhookOpsSnapshot,
        build_ai_artifact_summary_view, build_live_model, build_ops_model,
        build_state_from_snapshot, demo_eval_run_details, empty_investigation_report,
        newest_day_index, review_card_badges, review_detail_lines, serialize_json,
    };
    use crate::action::Action;
    use crate::ai::{
        AiRequestPreview, AiRequestPreviewSnapshot, ArtifactFinding, ArtifactFollowUpTarget,
        ArtifactStatus, ConfidenceLevel, GuidedFollowUpKind, ReviewArtifactV1, SufficiencyLevel,
    };
    use crate::evidence::policy::evidence_badges;
    use crate::evidence::{PopulationProfile, evidence_registry_version};
    use crate::insights::MetricPoint;
    use crate::navigation::{self, FocusRegion, PreflightControl, SearchScope, TransientLayer};
    use crate::oura::models::{AuthStatus, CapabilityKind, CapabilityReport};
    use crate::review::{
        InvestigationReport, ReviewCard, ReviewConfidence, ReviewDeck, ReviewFocus, ReviewMode,
        ReviewSection, ReviewSufficiency,
    };
    use crate::snapshot::PrivacyProfile;
    use crate::store::queries::{
        AiArtifactDaySummaryRecord, AiArtifactRecord, AiEvalRunRecord, AiRunRecord,
        ContextEventFamily, ContextEventRecord, DailySpO2Record, DataSufficiency, EffectDirection,
        HeartRatePoint, PatternMetric, PatternRelationWindow, PatternSummaryRecord, RecordCounts,
        ReportExportRecord, RestModePeriodRecord, ReviewSignalDayRecord, SleepPeriodRecord,
        SleepTimeRecord, SnapshotCatalogEntry, TimeSemantics,
    };
    use crate::test_support::{ok, some};
    use crate::ui::layout::ViewportClass;

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

    fn test_refresh_policy() -> RefreshPolicySnapshot {
        RefreshPolicySnapshot {
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
        }
    }

    fn test_auth_status() -> AuthStatus {
        let scopes = vec![
            "email".to_owned(),
            "personal".to_owned(),
            "daily".to_owned(),
            "heartrate".to_owned(),
            "tag".to_owned(),
            "workout".to_owned(),
            "session".to_owned(),
            "spo2".to_owned(),
            "ring_configuration".to_owned(),
            "stress".to_owned(),
            "heart_health".to_owned(),
        ];
        AuthStatus {
            configured: true,
            callback_url: "http://localhost:8788/callback".to_owned(),
            requested_scopes: scopes.clone(),
            granted_scopes: scopes,
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
        }
    }

    fn test_ai_ops_snapshot() -> AiOpsSnapshot {
        AiOpsSnapshot {
            enabled: false,
            provider: "openai".to_owned(),
            api_key_env: "OPENAI_API_KEY".to_owned(),
            api_key_ready: false,
            default_model: "gpt-5-mini".to_owned(),
            reasoning_effort: "default".to_owned(),
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            prompt_cache: "off".to_owned(),
            review_prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
            compare_prompt_version: COMPARE_PROMPT_VERSION.to_owned(),
            tools_disabled: true,
            snapshot_catalog_count: 0,
            ai_run_count: 0,
            ai_artifact_count: 0,
            report_export_count: 0,
            ai_eval_run_count: 0,
            last_successful_run: None,
            last_failed_run: None,
        }
    }

    fn test_context_event() -> ContextEventRecord {
        ContextEventRecord {
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
                    bpm: 60 + u16::try_from(index).unwrap_or(u16::MAX),
                    source_day: Some((*day).to_owned()),
                }],
            })
            .collect();

        LiveSnapshot {
            captured_at: "2026-04-08T12:00:00Z".to_owned(),
            refresh_policy: test_refresh_policy(),
            auth_status: test_auth_status(),
            active_population_profile: PopulationProfile::GeneralAdult,
            guidance_profile_source: "default".to_owned(),
            evidence_registry_version: evidence_registry_version().to_owned(),
            stale_evidence_entries: Vec::new(),
            ai_ops: test_ai_ops_snapshot(),
            webhook: WebhookOpsSnapshot::default(),
            personal_info: None,
            daily_history: days
                .iter()
                .map(|day| crate::store::queries::DailyOverviewRow {
                    day: (*day).to_owned(),
                    sleep_score: Some(80),
                    sleep_duration_seconds: Some(27_000),
                    readiness_score: Some(80),
                    activity_score: Some(70),
                    updated_at: "2026-04-08T12:00:00Z".to_owned(),
                })
                .collect(),
            daily_activity: super::demo_daily_activity_records(),
            daily_readiness: super::demo_daily_readiness_records(),
            daily_stress: super::demo_daily_stress_records(),
            sleep_periods: days
                .iter()
                .enumerate()
                .map(|(index, day)| {
                    let offset = u32::try_from(index).unwrap_or(0);
                    let offset_f64 = f64::from(offset);
                    SleepPeriodRecord {
                        oura_id: format!("test-sleep-{day}"),
                        day: (*day).to_owned(),
                        bedtime_start: Some(format!("{day}T23:{:02}:00Z", 10 + index)),
                        bedtime_end: Some(format!("{day}T06:{:02}:00Z", 30 + index)),
                        sleep_type: Some("long_sleep".to_owned()),
                        average_heart_rate: Some(56.0 + offset_f64),
                        average_hrv: Some(40.0 - offset_f64),
                        average_breath: Some(offset_f64.mul_add(0.2, 14.0)),
                        total_sleep_duration: Some(
                            27_000 - i64::try_from(index).unwrap_or(0) * 900,
                        ),
                        raw_cache_key: None,
                        updated_at: format!("{day}T07:00:00Z"),
                    }
                })
                .collect(),
            daily_spo2: days
                .iter()
                .enumerate()
                .map(|(index, day)| {
                    let offset = u32::try_from(index).unwrap_or(0);
                    let offset_f64 = f64::from(offset);
                    DailySpO2Record {
                        oura_id: Some(format!("test-spo2-{day}")),
                        day: (*day).to_owned(),
                        average_spo2: Some(offset_f64.mul_add(-0.2, 97.5)),
                        breathing_disturbance_index: Some(offset_f64.mul_add(0.1, 0.4)),
                        raw_cache_key: None,
                        updated_at: format!("{day}T07:00:00Z"),
                    }
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
            context_events: vec![test_context_event()],
            pattern_summaries: Vec::new(),
            review_signal_days: Vec::new(),
            sleep_time: Vec::new(),
            rest_mode_periods: Vec::new(),
            daily_resilience: Vec::new(),
            daily_cardiovascular_age: Vec::new(),
            vo2_max: Vec::new(),
            ai_artifacts_by_day: BTreeMap::new(),
            snapshot_catalog: Vec::new(),
            ai_runs: Vec::new(),
            ai_artifact_records: Vec::new(),
            report_exports: Vec::new(),
            ai_eval_runs: Vec::new(),
            sync_states: Vec::new(),
            record_counts: RecordCounts {
                workouts: 1,
                ..RecordCounts::default()
            },
            schema_version: crate::store::migrations::current_version(),
            database_path: ":memory:".to_owned(),
            config_path: "config.toml".to_owned(),
        }
    }

    fn make_live_app(snapshot: LiveSnapshot) -> AppState {
        let selected_day_index = newest_day_index(&snapshot);
        let screen_focus_memory =
            std::array::from_fn(|index| navigation::default_region(Screen::ALL[index]));
        let mut app = AppState {
            mode: RunMode::Live,
            active_screen: Screen::Timeline,
            model: super::AppModel::empty(),
            status_line: String::new(),
            tick_count: 0,
            should_quit: false,
            refresh_in_flight: false,
            live_snapshot: Some(snapshot),
            focused_region: navigation::default_region(Screen::Timeline),
            screen_focus_memory,
            focused_top_nav_screen: Screen::Timeline,
            help_open: false,
            focus_before_help: None,
            search: None,
            selected_day_index,
            selected_timeline_point: 0,
            timeline_window_hours: 24,
            selected_overlay_toggle_index: 0,
            trends_window: TrendWindowKind::Days7,
            trend_sort_mode: TrendSortMode::Concern,
            selected_trend_row_index: 0,
            selected_event_id: None,
            selected_dashboard_breakdown_index: 0,
            expanded_region: None,
            selected_review_card_index: 0,
            ai_preflight: None,
            ai_preflight_control: PreflightControl::Confirm,
            ai_browser_tab: AiBrowserTab::Runs,
            selected_ai_launch_index: 0,
            selected_ai_run_index: 0,
            selected_snapshot_catalog_index: 0,
            selected_report_export_index: 0,
            selected_ai_eval_run_index: 0,
            selected_ai_artifact_action_index: 0,
            overlay_filters: OverlayFilterState::all(),
            pattern_metric_filter: PatternMetricFilter::All,
            review_mode: ReviewScreenMode::Today,
            review_focus: ReviewFocus::Readiness,
        };
        app.select_default_event_for_selected_day();
        app.rebuild_live_model();
        app
    }

    fn make_ai_preview(snapshot_hash: &str) -> AiRequestPreview {
        AiRequestPreview {
            task_family: "review".to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-5-mini".to_owned(),
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            prompt_cache: "auto".to_owned(),
            prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
            output_schema_version: "ringmaster.ai.review.v3".to_owned(),
            snapshots: vec![AiRequestPreviewSnapshot {
                label: "primary".to_owned(),
                snapshot_hash: snapshot_hash.to_owned(),
                scope: "day:2026-04-08".to_owned(),
                anchor_day: "2026-04-08".to_owned(),
                privacy_profile: PrivacyProfile::Redacted,
                active_population_profile: PopulationProfile::GeneralAdult,
                day_count: 1,
            }],
            snapshot_bytes: 48_000,
            approximate_input_tokens: 12_000,
            stateless: true,
            tools_disabled: true,
            includes_notes_or_free_text: true,
            content_classes: vec![
                "summary".to_owned(),
                "review_signals".to_owned(),
                "follow_up_targets".to_owned(),
            ],
            prefix_fingerprint: "test-preview-prefix".to_owned(),
            payload_fingerprint: "test-preview-payload".to_owned(),
            request_fingerprint: "test-preview-request".to_owned(),
        }
    }

    fn make_snapshot_catalog_entry(snapshot_hash: &str) -> SnapshotCatalogEntry {
        SnapshotCatalogEntry {
            snapshot_hash: snapshot_hash.to_owned(),
            schema_version: "ringmaster.snapshot.v3".to_owned(),
            generated_at: "2026-04-08T22:18:00Z".to_owned(),
            scope: "day:2026-04-08".to_owned(),
            start_day: "2026-04-08".to_owned(),
            end_day: "2026-04-08".to_owned(),
            anchor_day: "2026-04-08".to_owned(),
            day_count: 1,
            privacy_profile: "redacted".to_owned(),
            source_mode: "demo".to_owned(),
            fixture_dir: None,
            latest_source_day: Some("2026-04-08".to_owned()),
            latest_review_day: Some("2026-04-08".to_owned()),
            freshness_summary: "current day with local sync coverage".to_owned(),
            trust_summary: "explicit snapshot export, redacted profile".to_owned(),
            capability_summary: "personal,daily,heartrate,workout,tag,session".to_owned(),
            provenance_summary: "review_signal + context refs".to_owned(),
            created_at: "2026-04-08T22:18:00Z".to_owned(),
        }
    }

    fn make_ai_artifact_record(artifact_id: &str, snapshot_hash: &str) -> AiArtifactRecord {
        let payload = ReviewArtifactV1 {
            schema_version: "ringmaster.ai.review.v3".to_owned(),
            prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
            status: ArtifactStatus::Success,
            overview: "Stress softened after an earlier wind-down.".to_owned(),
            headline_findings: vec![ArtifactFinding {
                finding_id: "earlier-bedtime".to_owned(),
                title: "Earlier bedtime improved readiness".to_owned(),
                summary: "The saved artifact links a steadier wind-down to stronger readiness."
                    .to_owned(),
                claim_key: Some("sleep_time_status".to_owned()),
                evidence_tier: Some(crate::evidence::registry::EvidenceTier::EvidenceInformed),
                interpretation_scope: Some(
                    crate::evidence::registry::InterpretationScope::WithinPersonTrendOnly,
                ),
                active_population_profile: Some(PopulationProfile::GeneralAdult),
                population_support_status: Some(
                    crate::evidence::registry::PopulationSupportStatus::PopulationSpecific,
                ),
                fallback_population_profile: None,
                caution_labels: evidence_badges(
                    "sleep_time_status",
                    PopulationProfile::GeneralAdult,
                ),
                confidence: ConfidenceLevel::Medium,
                sufficiency: SufficiencyLevel::Medium,
                evidence_refs: vec![crate::ai::ArtifactEvidenceRef {
                    export_ref: "sleep_time:2026-04-08".to_owned(),
                    note: "Earlier sleep window".to_owned(),
                }],
                counterevidence_refs: Vec::new(),
            }],
            positive_findings: Vec::new(),
            negative_findings: Vec::new(),
            unresolved_questions: vec!["Would this hold across a full week?".to_owned()],
            limitations: Vec::new(),
            follow_up_targets: vec![ArtifactFollowUpTarget {
                label: "Expand evidence".to_owned(),
                command: "ai follow-up expand-evidence".to_owned(),
                reason: "Inspect the strongest local export refs before rerunning.".to_owned(),
            }],
        };

        AiArtifactRecord {
            artifact_id: artifact_id.to_owned(),
            artifact_kind: "review".to_owned(),
            output_schema_version: "ringmaster.ai.review.v3".to_owned(),
            prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-5-mini".to_owned(),
            reasoning_effort: Some("medium".to_owned()),
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            run_mode: "real".to_owned(),
            created_at: "2026-04-08T22:20:00Z".to_owned(),
            snapshot_hash_a: snapshot_hash.to_owned(),
            snapshot_hash_b: None,
            privacy_profile: "redacted".to_owned(),
            artifact_status: "succeeded".to_owned(),
            overview: "Stress softened after an earlier wind-down.".to_owned(),
            summary_cache: "Saved review: steadier bedtime correlated with stronger readiness."
                .to_owned(),
            request_fingerprint: Some("test-preview-request".to_owned()),
            payload_json: ok(
                serde_json::to_string(&payload),
                "artifact payload should serialize",
            ),
            rendered_briefing: "ringmaster ai review\n\noverview: steadier bedtime correlated with stronger readiness."
                .to_owned(),
        }
    }

    fn make_ai_run_record(
        run_id: &str,
        status: &str,
        artifact_id: Option<&str>,
        error_message: Option<&str>,
    ) -> AiRunRecord {
        make_ai_run_record_with_shape(
            run_id,
            "review",
            status,
            "2026-04-08T22:30:00Z",
            artifact_id,
            error_message,
        )
    }

    fn make_ai_run_record_with_shape(
        run_id: &str,
        run_kind: &str,
        status: &str,
        created_at: &str,
        artifact_id: Option<&str>,
        error_message: Option<&str>,
    ) -> AiRunRecord {
        AiRunRecord {
            run_id: run_id.to_owned(),
            run_kind: run_kind.to_owned(),
            run_status: status.to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-5-mini".to_owned(),
            reasoning_effort: Some("medium".to_owned()),
            request_mode: "stateless".to_owned(),
            input_transport: "inline".to_owned(),
            run_mode: "real".to_owned(),
            prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
            output_schema_version: "ringmaster.ai.review.v3".to_owned(),
            privacy_profile: "redacted".to_owned(),
            snapshot_scope: "day:2026-04-08".to_owned(),
            snapshot_hash_a: "snapshot-ai-20260408".to_owned(),
            snapshot_hash_b: None,
            source_ai_artifact_id: None,
            follow_up_kind: None,
            request_fingerprint: Some("test-preview-request".to_owned()),
            request_preview_json: ok(
                serde_json::to_string(&make_ai_preview("snapshot-ai-20260408")),
                "request preview should serialize",
            ),
            artifact_id: artifact_id.map(str::to_owned),
            error_message: error_message.map(str::to_owned),
            created_at: created_at.to_owned(),
            started_at: Some(created_at.to_owned()),
            ended_at: if matches!(status, "queued" | "running") {
                None
            } else {
                Some("2026-04-08T22:30:05Z".to_owned())
            },
            updated_at: "2026-04-08T22:30:05Z".to_owned(),
        }
    }

    fn make_report_export_record(artifact_id: &str, snapshot_hash: &str) -> ReportExportRecord {
        ReportExportRecord {
            report_id: "report-ai-20260408".to_owned(),
            report_kind: "ai_review".to_owned(),
            title: "Daily review briefing".to_owned(),
            format: "markdown".to_owned(),
            output_path: "/tmp/ringmaster-test-ai-report.md".to_owned(),
            content_hash: "content-hash".to_owned(),
            privacy_profile: "redacted".to_owned(),
            created_at: "2026-04-08T22:35:00Z".to_owned(),
            source_snapshot_hash_a: Some(snapshot_hash.to_owned()),
            source_snapshot_hash_b: None,
            source_ai_artifact_id: Some(artifact_id.to_owned()),
            provider: Some("openai".to_owned()),
            model: Some("gpt-5-mini".to_owned()),
            prompt_version: Some(REVIEW_PROMPT_VERSION.to_owned()),
            output_schema_version: Some("ringmaster.ai.review.v2".to_owned()),
            export_status: "written".to_owned(),
            last_verified_exists: true,
            last_verified_at: "2026-04-08T22:35:00Z".to_owned(),
        }
    }

    fn base_live_model_options() -> LiveModelOptions {
        LiveModelOptions {
            selected_day_index: 0,
            selected_point_index: 0,
            selected_event_id: None,
            ai_preflight: None,
            ai_preflight_control: PreflightControl::Confirm,
            ai_browser_tab: AiBrowserTab::Runs,
            selected_ai_launch_index: 0,
            selected_ai_run_index: 0,
            selected_snapshot_catalog_index: 0,
            selected_report_export_index: 0,
            selected_ai_eval_run_index: 0,
            selected_ai_artifact_action_index: 0,
            overlay_filters: OverlayFilterState::all(),
            selected_overlay_toggle_index: 0,
            window_hours: 24,
            trends_window: TrendWindowKind::Days7,
            trend_sort_mode: TrendSortMode::Concern,
            selected_trend_row_index: 0,
            pattern_metric_filter: PatternMetricFilter::All,
            refresh_in_flight: false,
            review_mode: ReviewScreenMode::Today,
            review_focus: ReviewFocus::Readiness,
            selected_review_card_index: 0,
            selected_dashboard_breakdown_index: 0,
        }
    }

    fn make_ai_run_browser_snapshot() -> LiveSnapshot {
        let mut snapshot = make_snapshot(&["2026-04-08"]);
        snapshot.ai_ops.enabled = true;
        snapshot.ai_ops.api_key_ready = true;
        snapshot.ai_ops.ai_run_count = 5;
        snapshot.ai_ops.ai_artifact_count = 1;
        snapshot.ai_ops.report_export_count = 1;
        snapshot.snapshot_catalog = vec![make_snapshot_catalog_entry("snapshot-ai-20260408")];
        snapshot.ai_artifact_records = vec![make_ai_artifact_record(
            "artifact-ai-succeeded",
            "snapshot-ai-20260408",
        )];
        snapshot.report_exports = vec![make_report_export_record(
            "artifact-ai-succeeded",
            "snapshot-ai-20260408",
        )];
        snapshot.ai_runs = vec![
            make_ai_run_record("run-ai-queued", "queued", None, None),
            make_ai_run_record("run-ai-running", "running", None, None),
            make_ai_run_record(
                "run-ai-succeeded",
                "succeeded",
                Some("artifact-ai-succeeded"),
                None,
            ),
            make_ai_run_record(
                "run-ai-failed",
                "failed",
                None,
                Some("Provider returned a structured error."),
            ),
            make_ai_run_record(
                "run-ai-cancelled",
                "cancelled",
                None,
                Some("Cancelled from the AI workbench."),
            ),
        ];
        snapshot
    }

    fn build_ai_model_for_run(
        snapshot: &LiveSnapshot,
        selected_ai_run_index: usize,
    ) -> super::AppModel {
        let mut options = base_live_model_options();
        options.ai_browser_tab = AiBrowserTab::Runs;
        options.selected_ai_run_index = selected_ai_run_index;
        build_live_model(snapshot, &options)
    }

    fn assert_ai_detail_contains(model: &super::AppModel, expected: &str) {
        assert!(
            model.ai.detail_lines.iter().any(|line| line == expected),
            "detail should contain `{expected}`"
        );
    }

    fn assert_ai_action_present(model: &super::AppModel, expected: &str) {
        assert!(
            model
                .ai
                .artifact_actions
                .iter()
                .any(|action| action.label == expected),
            "artifact actions should contain `{expected}`"
        );
    }

    #[test]
    fn live_selection_defaults_to_newest_available_day() {
        let snapshot = make_snapshot(&["2026-04-06", "2026-04-07", "2026-04-08"]);
        let app = make_live_app(snapshot);

        assert_eq!(app.selected_day_index, 2);
        assert_eq!(app.model.timeline.selected_day_label, "2026-04-08");
    }

    #[test]
    fn ai_run_browser_surfaces_lifecycle_statuses_and_saved_actions() {
        let snapshot = make_ai_run_browser_snapshot();

        for (index, status) in ["queued", "running", "succeeded", "failed", "cancelled"]
            .iter()
            .enumerate()
        {
            let model = build_ai_model_for_run(&snapshot, index);

            assert_eq!(model.ai.browser_items[index].status_badge, *status);
            assert!(
                model
                    .ai
                    .detail_lines
                    .iter()
                    .any(|line| line.contains(&format!("status: {status}"))),
                "detail should mention the selected run status `{status}`"
            );
        }

        let succeeded_model = build_ai_model_for_run(&snapshot, 2);
        assert_ai_detail_contains(&succeeded_model, "linked_artifact: artifact-ai-succeeded");
        assert_ai_detail_contains(&succeeded_model, "guided_follow_ups:");
        assert_ai_action_present(&succeeded_model, "Expand evidence");
        assert_ai_action_present(&succeeded_model, "Generate report");

        let failed_model = build_ai_model_for_run(&snapshot, 3);
        assert_ai_detail_contains(
            &failed_model,
            "error: Provider returned a structured error.",
        );

        let cancelled_model = build_ai_model_for_run(&snapshot, 4);
        assert_ai_detail_contains(&cancelled_model, "error: Cancelled from the AI workbench.");
    }

    #[test]
    fn ai_workbench_preflight_view_surfaces_model_override_and_follow_up_kind() {
        let snapshot = make_snapshot(&["2026-04-08"]);
        let mut options = base_live_model_options();
        options.ai_preflight = Some(AiPreflightState {
            intent: super::AiLaunchIntent::ReviewSelectedDay,
            source_screen: Screen::Review,
            snapshot_scope: "day:2026-04-08".to_owned(),
            snapshot_paths: vec![
                "/tmp/cache/ringmaster/ai-workbench/snapshots/review-20260408-redacted.json"
                    .to_owned(),
            ],
            request_preview: make_ai_preview("snapshot-ai-20260408"),
            privacy_profile: PrivacyProfile::Redacted,
            model_override: Some("gpt-5-mini".to_owned()),
            source_ai_artifact_id: Some("artifact-ai-succeeded".to_owned()),
            follow_up_kind: Some(GuidedFollowUpKind::ExpandEvidence),
            warning_lines: Vec::new(),
            confirm_enabled: true,
        });
        let model = build_live_model(&snapshot, &options);
        let preflight = some(model.ai.preflight, "preflight view should be present");

        assert_eq!(preflight.title, "Preflight | Review this day");
        assert!(
            preflight
                .body_lines
                .iter()
                .any(|line| line == "model override: gpt-5-mini")
        );
        assert!(
            preflight
                .body_lines
                .iter()
                .any(|line| line == "follow_up_kind: expand_evidence")
        );
        assert!(
            preflight
                .body_lines
                .iter()
                .any(|line| line.contains("/tmp/cache/ringmaster/ai-workbench/snapshots"))
        );
    }

    #[test]
    fn ai_workbench_warning_lines_call_out_disabled_provider_and_recent_failures() {
        let mut snapshot = make_snapshot(&["2026-04-08"]);
        snapshot.ai_ops.last_failed_run = Some("2026-04-08T22:24:02Z compare failed".to_owned());
        let model = build_live_model(&snapshot, &base_live_model_options());

        assert!(
            model
                .ai
                .warning_lines
                .iter()
                .any(|line| line.contains("Provider is disabled."))
        );
        assert!(
            model
                .ai
                .warning_lines
                .iter()
                .any(|line| line.contains("Most recent failed run"))
        );
    }

    #[test]
    fn ai_eval_browser_surfaces_failing_graders_and_saved_links() {
        let mut snapshot = make_snapshot(&["2026-04-08"]);
        snapshot.snapshot_catalog = vec![
            make_snapshot_catalog_entry("demo-snapshot-20260408"),
            make_snapshot_catalog_entry("demo-snapshot-20260401-20260408"),
        ];
        snapshot.ai_runs = vec![
            make_ai_run_record(
                "airun-demo-review-20260408",
                "succeeded",
                Some("run-demo-review-20260408"),
                None,
            ),
            make_ai_run_record_with_shape(
                "airun-demo-compare-20260408",
                "compare",
                "failed",
                "2026-04-08T22:24:00Z",
                None,
                None,
            ),
        ];
        snapshot.report_exports = vec![make_report_export_record(
            "run-demo-review-20260408",
            "demo-snapshot-20260408",
        )];
        let details = demo_eval_run_details();
        snapshot.ai_eval_runs = vec![AiEvalRunRecord {
            eval_run_id: "demo-eval-review".to_owned(),
            task_family: "mixed".to_owned(),
            fixture_dir: details.fixture_dir.clone(),
            candidate_label: details.candidate_label.clone(),
            baseline_label: details.baseline_label.clone(),
            provider: "openai".to_owned(),
            model: "gpt-5-mini".to_owned(),
            prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
            output_schema_version: "mixed".to_owned(),
            created_at: "2026-04-08T22:27:00Z".to_owned(),
            total_cases: 2,
            passed_cases: 1,
            failed_cases: 1,
            schema_validity_score: details.scores.schema_validity,
            completeness_score: details.scores.completeness,
            overclaiming_score: details.scores.overclaiming,
            medical_safety_score: details.scores.medical_safety,
            privacy_score: details.scores.privacy,
            evidence_score: details.scores.evidence,
            honesty_score: details.scores.honesty,
            regression_summary: details.regression_summary.clone(),
            details_json: serialize_json(&details),
        }];
        let mut options = base_live_model_options();
        options.ai_browser_tab = AiBrowserTab::Evals;
        let model = build_live_model(&snapshot, &options);

        assert_eq!(model.ai.detail_title, "Eval run");
        assert!(
            model
                .ai
                .browser_items
                .iter()
                .any(|item| item.headline.contains("gpt-5-mini vs fixture"))
        );
        assert!(
            model
                .ai
                .detail_lines
                .iter()
                .any(|line| line == "failing_graders:")
        );
        assert!(
            model.ai.detail_lines.iter().any(|line| {
                line == "  candidate_run: airun-demo-compare-20260408 (compare | failed | 2026-04-08T22:24:00Z)"
            })
        );
        assert!(
            model.ai.detail_lines.iter().any(|line| {
                line.contains("linked_snapshot_b: demo-snapshot-20260401-20260408")
            })
        );
    }

    #[test]
    fn ops_model_surfaces_latest_eval_health() {
        let mut snapshot = make_snapshot(&["2026-04-08"]);
        let details = demo_eval_run_details();
        snapshot.ai_eval_runs = vec![AiEvalRunRecord {
            eval_run_id: "demo-eval-review".to_owned(),
            task_family: "mixed".to_owned(),
            fixture_dir: details.fixture_dir.clone(),
            candidate_label: details.candidate_label.clone(),
            baseline_label: details.baseline_label.clone(),
            provider: "openai".to_owned(),
            model: "gpt-5-mini".to_owned(),
            prompt_version: REVIEW_PROMPT_VERSION.to_owned(),
            output_schema_version: "mixed".to_owned(),
            created_at: "2026-04-08T22:27:00Z".to_owned(),
            total_cases: 2,
            passed_cases: 1,
            failed_cases: 1,
            schema_validity_score: details.scores.schema_validity,
            completeness_score: details.scores.completeness,
            overclaiming_score: details.scores.overclaiming,
            medical_safety_score: details.scores.medical_safety,
            privacy_score: details.scores.privacy,
            evidence_score: details.scores.evidence,
            honesty_score: details.scores.honesty,
            regression_summary: details.regression_summary.clone(),
            details_json: serialize_json(&details),
        }];
        let model = build_ops_model(&snapshot, false);

        assert!(model.summary_lines.iter().any(|line| {
            line.contains("Latest eval: 2026-04-08T22:27:00Z")
                && line.contains("failed_cases=1 regressions=1 improvements=1")
        }));
        assert!(model.items.iter().any(|item| {
            item.label == "Eval health"
                && item
                    .value
                    .contains("failed_cases=1 regressions=1 improvements=1")
        }));
        assert!(
            model
                .warnings
                .iter()
                .any(|line| line.contains("Latest eval needs attention"))
        );
    }

    #[test]
    fn review_card_badges_keep_sensitive_cautions_visible() {
        let card = make_review_card("spo2-card", "spo2", 80);

        let badges = review_card_badges(&card, PopulationProfile::OlderAdult);

        assert!(badges.iter().any(|badge| badge == "Evidence-informed"));
        assert!(badges.iter().any(|badge| badge == "Context-only"));
        assert!(badges.iter().any(|badge| badge == "Unavailable"));
        assert!(badges.iter().any(|badge| badge == "Sensitive metric"));
        assert!(badges.iter().any(|badge| badge == "Not for screening"));
        assert!(badges.len() <= 5);
    }

    #[test]
    fn review_detail_lines_surface_population_fallback_scope() {
        let card = make_review_card("sleep-duration", "sleep_duration", 70);
        let investigation =
            empty_investigation_report(ReviewFocus::Readiness, "2026-04-08", &"test");

        let lines = review_detail_lines(
            ReviewScreenMode::Today,
            Some(&card),
            &investigation,
            PopulationProfile::ShiftWorker,
        );

        assert!(lines.iter().any(|line| {
            line == "Population scope: Shift worker profile uses General adult guidance as a fallback"
        }));
    }

    #[test]
    fn review_detail_lines_surface_unavailable_population_scope() {
        let card = make_review_card("spo2-detail", "spo2", 65);
        let investigation =
            empty_investigation_report(ReviewFocus::Readiness, "2026-04-08", &"test");

        let lines = review_detail_lines(
            ReviewScreenMode::Today,
            Some(&card),
            &investigation,
            PopulationProfile::OlderAdult,
        );

        assert!(lines.iter().any(|line| {
            line == "Population scope: Older adult profile has no supported interpretation; keep this context-only"
        }));
    }

    #[test]
    fn ops_model_surfaces_evidence_runtime_health() {
        let mut snapshot = make_snapshot(&["2026-04-08"]);
        snapshot.stale_evidence_entries = vec![
            "`spo2` last reviewed on 2025-12-01 and is stale for Quarterly cadence (133 days old)"
                .to_owned(),
        ];

        let model = build_ops_model(&snapshot, false);

        assert!(model.summary_lines.iter().any(|line| {
            line.contains("Evidence: registry=ringmaster.evidence.v2")
                && line.contains("1 stale entry")
        }));
        assert!(model.items.iter().any(|item| {
            item.label == "Evidence registry" && item.value == evidence_registry_version()
        }));
        assert!(model.items.iter().any(|item| {
            item.label == "Evidence review status" && item.value.contains("1 stale entry")
        }));
        assert!(
            model
                .warnings
                .iter()
                .any(|line| { line.contains("Evidence registry review needs attention") })
        );
        assert!(
            model
                .warnings
                .iter()
                .any(|line| { line.contains("`spo2` last reviewed on 2025-12-01") })
        );
    }

    #[test]
    fn day_actions_update_shared_selected_day() {
        let mut snapshot = make_snapshot(&["2026-04-06", "2026-04-07", "2026-04-08"]);
        snapshot.ai_artifacts_by_day = BTreeMap::from([
            (
                "2026-04-07".to_owned(),
                AiArtifactDaySummaryRecord {
                    artifact_id: "run-20260407".to_owned(),
                    artifact_kind: "review".to_owned(),
                    created_at: "2026-04-07T12:00:00Z".to_owned(),
                    provider: "openai".to_owned(),
                    model: "gpt-4o-mini".to_owned(),
                    prompt_version: "review_prompt_v1".to_owned(),
                    output_schema_version: "ringmaster.ai.review.v3".to_owned(),
                    privacy_profile: "redacted".to_owned(),
                    summary_cache: "Readiness recovered after a quieter evening.".to_owned(),
                    overview: "The saved review still notes a mild activity dip.".to_owned(),
                    matched_snapshot_hash: "snapshot-20260407".to_owned(),
                    peer_snapshot_hash: None,
                },
            ),
            (
                "2026-04-08".to_owned(),
                AiArtifactDaySummaryRecord {
                    artifact_id: "run-20260408".to_owned(),
                    artifact_kind: "compare".to_owned(),
                    created_at: "2026-04-08T12:00:00Z".to_owned(),
                    provider: "openai".to_owned(),
                    model: "gpt-4o-mini".to_owned(),
                    prompt_version: "compare_prompt_v2".to_owned(),
                    output_schema_version: "ringmaster.ai.compare.v3".to_owned(),
                    privacy_profile: "redacted".to_owned(),
                    summary_cache: "Stress softened versus the previous snapshot.".to_owned(),
                    overview: "Sleep remains the main explanation for the day-to-day drift."
                        .to_owned(),
                    matched_snapshot_hash: "snapshot-20260408".to_owned(),
                    peer_snapshot_hash: Some("snapshot-20260407".to_owned()),
                },
            ),
        ]);
        let mut app = make_live_app(snapshot);

        app.handle(Action::PreviousDay);
        assert_eq!(app.model.timeline.selected_day_label, "2026-04-07");
        assert_eq!(app.model.dashboard.selected_day_label, "2026-04-07");
        assert_eq!(app.model.explain.selected_day_label, "2026-04-07");
        assert_eq!(app.model.review.ai_artifact.status_label, "available");
        assert!(
            app.model
                .review
                .ai_artifact
                .lineage_lines
                .iter()
                .any(|line| line == "Run id: run-20260407")
        );

        app.handle(Action::NextDay);
        assert_eq!(app.model.timeline.selected_day_label, "2026-04-08");
        assert!(
            app.model
                .review
                .ai_artifact
                .lineage_lines
                .iter()
                .any(|line| line == "Peer snapshot: snapshot-20260407")
        );
    }

    #[test]
    fn ai_artifact_summary_view_prefers_summary_cache_and_distinct_overview() {
        let view = build_ai_artifact_summary_view(&AiArtifactDaySummaryRecord {
            artifact_id: "run-1".to_owned(),
            artifact_kind: "review".to_owned(),
            created_at: "2026-04-08T22:20:00Z".to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-4o-mini".to_owned(),
            prompt_version: "review_prompt_v1".to_owned(),
            output_schema_version: "ringmaster.ai.review.v3".to_owned(),
            privacy_profile: "redacted".to_owned(),
            summary_cache: "Primary saved summary.".to_owned(),
            overview: "Secondary overview.".to_owned(),
            matched_snapshot_hash: "snapshot-1".to_owned(),
            peer_snapshot_hash: None,
        });

        assert_eq!(view.status_label, "available");
        assert_eq!(
            view.metadata_lines[0],
            "Kind / created: review / 2026-04-08 22:20"
        );
        assert_eq!(
            view.summary_text,
            "Primary saved summary.\nSecondary overview."
        );
        assert_eq!(view.lineage_lines[0], "Run id: run-1");
    }

    #[test]
    fn replace_live_snapshot_prefers_nearest_earlier_day_when_the_selected_day_disappears() {
        let mut app = make_live_app(make_snapshot(&["2026-04-07", "2026-04-08", "2026-04-09"]));

        app.replace_live_snapshot(make_snapshot(&["2026-04-07", "2026-04-08", "2026-04-10"]));

        assert_eq!(app.model.timeline.selected_day_label, "2026-04-08");
        assert_eq!(app.model.dashboard.selected_day_label, "2026-04-08");
    }

    #[test]
    fn replace_live_snapshot_falls_forward_when_no_earlier_day_exists() {
        let mut app = make_live_app(make_snapshot(&["2026-04-06", "2026-04-07"]));
        app.handle(Action::PreviousDay);

        app.replace_live_snapshot(make_snapshot(&["2026-04-07", "2026-04-08"]));

        assert_eq!(app.model.timeline.selected_day_label, "2026-04-07");
    }

    #[test]
    fn available_days_ignore_review_only_baseline_rows() {
        let mut snapshot = make_snapshot(&["2026-04-07", "2026-04-08"]);
        snapshot.review_signal_days = vec![ReviewSignalDayRecord {
            signal_key: "sleep_score".to_owned(),
            day: "2026-02-01".to_owned(),
            numeric_value: Some(82.0),
            text_value: None,
            baseline_mean: Some(80.0),
            baseline_stddev: Some(2.0),
            delta: Some(2.0),
            z_score: Some(1.0),
            persistence_days: 1,
            sufficiency: ReviewSufficiency::Strong,
            stale_days: 0,
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-08T12:00:00Z".to_owned(),
        }];
        snapshot.sleep_time = vec![SleepTimeRecord {
            oura_id: Some("baseline-sleep".to_owned()),
            day: "2026-02-01".to_owned(),
            status: Some("late".to_owned()),
            recommendation: None,
            optimal_bedtime_start_offset: None,
            optimal_bedtime_end_offset: None,
            optimal_bedtime_day_tz: None,
            raw_cache_key: None,
            updated_at: "2026-04-08T12:00:00Z".to_owned(),
        }];
        snapshot.rest_mode_periods = vec![RestModePeriodRecord {
            period_id: "baseline-rest-mode".to_owned(),
            start_day: "2026-02-01".to_owned(),
            start_time: None,
            end_day: Some("2026-02-03".to_owned()),
            end_time: None,
            episode_count: 1,
            tags_json: "[]".to_owned(),
            raw_cache_key: None,
            updated_at: "2026-04-08T12:00:00Z".to_owned(),
        }];

        assert_eq!(
            super::available_days(&snapshot),
            vec!["2026-04-07".to_owned(), "2026-04-08".to_owned()]
        );
    }

    #[test]
    fn latest_review_anchor_day_treats_open_rest_mode_as_current() {
        let mut snapshot = make_snapshot(&[]);
        snapshot.rest_mode_periods = vec![RestModePeriodRecord {
            period_id: "open-rest-mode".to_owned(),
            start_day: "2026-02-01".to_owned(),
            start_time: None,
            end_day: None,
            end_time: None,
            episode_count: 1,
            tags_json: "[]".to_owned(),
            raw_cache_key: None,
            updated_at: "2026-04-08T12:00:00Z".to_owned(),
        }];

        assert_eq!(
            super::latest_review_anchor_day(&snapshot),
            crate::time_utils::current_local_day_string()
        );
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

        let bounds = some(
            super::event_bounds_for_day(&event, "2026-04-08"),
            "event should remain visible on its local anchor day",
        );
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
    fn explain_marks_prior_day_carryover_in_context_and_breadcrumbs() {
        let mut snapshot = make_snapshot(&["2026-04-07", "2026-04-08"]);
        snapshot.context_events.push(ContextEventRecord {
            context_event_id: "session:late".to_owned(),
            family: ContextEventFamily::Session,
            source_id: "late-session".to_owned(),
            anchor_day: "2026-04-07".to_owned(),
            start_at: "2026-04-07T19:30:00Z".to_owned(),
            end_at: Some("2026-04-07T20:15:00Z".to_owned()),
            time_semantics: TimeSemantics::Interval,
            title: "Late session".to_owned(),
            subtype: Some("focus".to_owned()),
            notes: Some("carryover context".to_owned()),
            intensity: Some("light".to_owned()),
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-07T20:20:00Z".to_owned(),
        });

        let model = build_live_model(
            &snapshot,
            &LiveModelOptions {
                selected_day_index: 1,
                selected_point_index: 0,
                selected_event_id: None,
                ai_preflight: None,
                ai_preflight_control: PreflightControl::Confirm,
                ai_browser_tab: AiBrowserTab::Runs,
                selected_ai_launch_index: 0,
                selected_ai_run_index: 0,
                selected_snapshot_catalog_index: 0,
                selected_report_export_index: 0,
                selected_ai_eval_run_index: 0,
                selected_ai_artifact_action_index: 0,
                overlay_filters: OverlayFilterState::all(),
                selected_overlay_toggle_index: 0,
                window_hours: 24,
                trends_window: TrendWindowKind::Days7,
                trend_sort_mode: TrendSortMode::Concern,
                selected_trend_row_index: 0,
                pattern_metric_filter: PatternMetricFilter::All,
                refresh_in_flight: false,
                review_mode: ReviewScreenMode::Today,
                review_focus: ReviewFocus::Readiness,
                selected_review_card_index: 0,
                selected_dashboard_breakdown_index: 0,
            },
        );

        assert!(
            model
                .explain
                .evidence_lines
                .iter()
                .any(|line| line.contains("carryover from 2026-04-07"))
        );
        assert!(
            model
                .explain
                .context_lines
                .iter()
                .any(|line| line.contains("Carryover from 2026-04-07"))
        );
        assert!(model.explain.breadcrumb.contains("carryover"));
    }

    #[test]
    fn explain_marks_cross_midnight_events_as_carryover_on_the_selected_day() {
        let mut snapshot = make_snapshot(&["2026-04-07", "2026-04-08"]);
        snapshot.context_events.push(ContextEventRecord {
            context_event_id: "workout:overnight".to_owned(),
            family: ContextEventFamily::Workout,
            source_id: "overnight-workout".to_owned(),
            anchor_day: "2026-04-07".to_owned(),
            start_at: "2026-04-07T23:30:00Z".to_owned(),
            end_at: Some("2026-04-08T00:30:00Z".to_owned()),
            time_semantics: TimeSemantics::Interval,
            title: "Overnight workout".to_owned(),
            subtype: Some("run".to_owned()),
            notes: Some("crosses midnight".to_owned()),
            intensity: Some("moderate".to_owned()),
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-08T00:35:00Z".to_owned(),
        });

        let model = build_live_model(
            &snapshot,
            &LiveModelOptions {
                selected_day_index: 1,
                selected_point_index: 0,
                selected_event_id: None,
                ai_preflight: None,
                ai_preflight_control: PreflightControl::Confirm,
                ai_browser_tab: AiBrowserTab::Runs,
                selected_ai_launch_index: 0,
                selected_ai_run_index: 0,
                selected_snapshot_catalog_index: 0,
                selected_report_export_index: 0,
                selected_ai_eval_run_index: 0,
                selected_ai_artifact_action_index: 0,
                overlay_filters: OverlayFilterState::all(),
                selected_overlay_toggle_index: 0,
                window_hours: 24,
                trends_window: TrendWindowKind::Days7,
                trend_sort_mode: TrendSortMode::Concern,
                selected_trend_row_index: 0,
                pattern_metric_filter: PatternMetricFilter::All,
                refresh_in_flight: false,
                review_mode: ReviewScreenMode::Today,
                review_focus: ReviewFocus::Readiness,
                selected_review_card_index: 0,
                selected_dashboard_breakdown_index: 0,
            },
        );

        assert!(
            model
                .explain
                .context_lines
                .iter()
                .any(|line| line.contains("Carryover from 2026-04-07"))
        );
        assert!(model.explain.breadcrumb.contains("carryover"));
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
    fn missing_scope_messages_are_capability_specific() {
        let report = CapabilityReport::from_scopes(
            &[
                CapabilityKind::Workout.scope_name().to_owned(),
                CapabilityKind::EnhancedTag.scope_name().to_owned(),
                CapabilityKind::Session.scope_name().to_owned(),
            ],
            &[],
        );

        assert_eq!(
            super::missing_scope_messages(&report),
            vec![
                "Workouts context is unavailable because the `workout` scope is missing."
                    .to_owned(),
                "Enhanced Tags context is unavailable because the `tag` scope is missing."
                    .to_owned(),
                "Sessions context is unavailable because the `session` scope is missing."
                    .to_owned(),
            ]
        );
    }

    #[test]
    fn pattern_row_copy_reads_cleanly() {
        let row = super::pattern_row_view(&PatternSummaryRecord {
            summary_id: "pattern-1".to_owned(),
            family: ContextEventFamily::Workout,
            normalized_key: "strength_builder".to_owned(),
            relation_window: PatternRelationWindow::NextDayReadiness,
            metric: PatternMetric::Readiness,
            sample_count: 5,
            median_delta: -2.4,
            effect_direction: EffectDirection::Lower,
            confidence: DataSufficiency::Medium,
            metadata_json: "{}".to_owned(),
            updated_at: "2026-04-08T12:00:00Z".to_owned(),
        });

        assert_eq!(
            row.detail,
            "next-day readiness trended toward lower readiness score (-2.4, n=5, confidence=medium)"
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
                ai_preflight: None,
                ai_preflight_control: PreflightControl::Confirm,
                ai_browser_tab: AiBrowserTab::Runs,
                selected_ai_launch_index: 0,
                selected_ai_run_index: 0,
                selected_snapshot_catalog_index: 0,
                selected_report_export_index: 0,
                selected_ai_eval_run_index: 0,
                selected_ai_artifact_action_index: 0,
                overlay_filters: OverlayFilterState::all(),
                selected_overlay_toggle_index: 0,
                window_hours: 24,
                trends_window: TrendWindowKind::Days7,
                trend_sort_mode: TrendSortMode::Concern,
                selected_trend_row_index: 0,
                pattern_metric_filter: PatternMetricFilter::All,
                refresh_in_flight: false,
                review_mode: ReviewScreenMode::Today,
                review_focus: ReviewFocus::Readiness,
                selected_review_card_index: 0,
                selected_dashboard_breakdown_index: 0,
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

    #[test]
    fn investigate_mode_sorts_equal_scores_deterministically() {
        let today = ReviewDeck {
            mode: ReviewMode::Today,
            anchor_day: "2026-04-08".to_owned(),
            observations: vec![
                make_review_card("today-b", "stress_high", 5),
                make_review_card("today-a", "recovery_high", 5),
            ],
            positive_changes: Vec::new(),
            negative_drifts: Vec::new(),
            unresolved_anomalies: Vec::new(),
            warnings: Vec::new(),
        };
        let week = ReviewDeck {
            mode: ReviewMode::Week,
            anchor_day: "2026-04-08".to_owned(),
            observations: vec![make_review_card("week-a", "stress_high", 5)],
            positive_changes: Vec::new(),
            negative_drifts: Vec::new(),
            unresolved_anomalies: Vec::new(),
            warnings: Vec::new(),
        };
        let investigation = InvestigationReport {
            focus: ReviewFocus::Stress,
            anchor_day: "2026-04-08".to_owned(),
            headline: "Stress investigation".to_owned(),
            summary: "stress summary".to_owned(),
            confidence: ReviewConfidence::Medium,
            sufficiency: ReviewSufficiency::Medium,
            evidence: Vec::new(),
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

        assert_eq!(
            cards
                .iter()
                .map(|card| card.id.as_str())
                .collect::<Vec<_>>(),
            vec!["today-a", "today-b", "week-a"]
        );
    }

    #[test]
    fn live_review_load_bounds_stay_bounded_to_visible_anchor_span() {
        let daily_history = vec![crate::store::queries::DailyOverviewRow {
            day: "2026-04-08".to_owned(),
            sleep_score: Some(80),
            sleep_duration_seconds: Some(27_000),
            readiness_score: Some(81),
            activity_score: Some(79),
            updated_at: "2026-04-08T12:00:00Z".to_owned(),
        }];
        let heartrate_days = vec![HeartRateDay {
            day: "2026-04-10".to_owned(),
            points: Vec::new(),
        }];

        let bounds = some(
            ok(
                super::live_review_load_bounds(&daily_history, &heartrate_days, Some("2026-04-12")),
                "load bounds should build",
            ),
            "load bounds should exist",
        );

        assert_eq!(bounds.signal_start, "2026-02-07");
        assert_eq!(bounds.signal_end, "2026-04-12");
        assert_eq!(bounds.context_start, "2026-01-08");
        assert_eq!(bounds.context_end, "2026-04-19");
        assert_eq!(bounds.rest_mode_start, "2025-10-10");
        assert_eq!(bounds.rest_mode_end, "2026-04-19");
    }

    #[test]
    fn help_toggle_restores_the_previous_region() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Review;
        app.set_focused_region(FocusRegion::ContextPrimary);
        assert_eq!(app.focused_region(), FocusRegion::ContextPrimary);

        app.handle(Action::ToggleHelp);
        assert!(app.help_open());

        app.handle(Action::ToggleHelp);
        assert!(!app.help_open());
        assert_eq!(app.focused_region(), FocusRegion::ContextPrimary);
    }

    #[test]
    fn closing_search_restores_the_previous_region() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Review;
        app.set_focused_region(FocusRegion::Primary);

        assert_eq!(app.focused_region(), FocusRegion::Primary);

        app.handle(Action::OpenSearch);
        assert!(app.search_state().is_some());

        app.handle(Action::CloseSearch);
        assert!(app.search_state().is_none());
        assert_eq!(app.focused_region(), FocusRegion::Primary);
    }

    #[test]
    fn visible_transient_takes_priority_over_underlying_preflight() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Ai;
        app.handle(Action::AiPreflightPrepared {
            preflight: Box::new(AiPreflightState {
                intent: AiLaunchIntent::ReviewSelectedDay,
                source_screen: Screen::Review,
                snapshot_scope: "day:2026-04-08".to_owned(),
                snapshot_paths: vec!["/tmp/preflight-snapshot.json".to_owned()],
                request_preview: make_ai_preview("demo-snapshot-20260408"),
                privacy_profile: PrivacyProfile::Redacted,
                model_override: Some("gpt-5-mini".to_owned()),
                source_ai_artifact_id: Some("run-demo-review-20260408".to_owned()),
                follow_up_kind: None,
                warning_lines: Vec::new(),
                confirm_enabled: true,
            }),
            status_line: "Prepared review preflight.".to_owned(),
        });
        app.handle(Action::ToggleHelp);
        assert_eq!(app.current_transient(), Some(TransientLayer::Help));

        app.handle(Action::OpenSearch);
        assert_eq!(app.current_transient(), Some(TransientLayer::Search));
    }

    #[test]
    fn back_closes_topmost_overlay_before_preflight() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Ai;
        app.handle(Action::AiPreflightPrepared {
            preflight: Box::new(AiPreflightState {
                intent: AiLaunchIntent::ReviewSelectedDay,
                source_screen: Screen::Review,
                snapshot_scope: "day:2026-04-08".to_owned(),
                snapshot_paths: vec!["/tmp/preflight-snapshot.json".to_owned()],
                request_preview: make_ai_preview("demo-snapshot-20260408"),
                privacy_profile: PrivacyProfile::Redacted,
                model_override: Some("gpt-5-mini".to_owned()),
                source_ai_artifact_id: Some("run-demo-review-20260408".to_owned()),
                follow_up_kind: None,
                warning_lines: Vec::new(),
                confirm_enabled: true,
            }),
            status_line: "Prepared review preflight.".to_owned(),
        });

        app.handle(Action::ToggleHelp);
        app.handle(Action::Back);
        assert_eq!(app.current_transient(), Some(TransientLayer::AiPreflight));

        app.handle(Action::OpenSearch);
        app.handle(Action::Back);
        assert_eq!(app.current_transient(), Some(TransientLayer::AiPreflight));

        app.handle(Action::Back);
        assert_eq!(app.current_transient(), None);
    }

    #[test]
    fn switching_away_from_ai_clears_hidden_preflight_transient() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Ai;
        app.handle(Action::AiPreflightPrepared {
            preflight: Box::new(AiPreflightState {
                intent: AiLaunchIntent::ReviewSelectedDay,
                source_screen: Screen::Review,
                snapshot_scope: "day:2026-04-08".to_owned(),
                snapshot_paths: vec!["/tmp/preflight-snapshot.json".to_owned()],
                request_preview: make_ai_preview("demo-snapshot-20260408"),
                privacy_profile: PrivacyProfile::Redacted,
                model_override: Some("gpt-5-mini".to_owned()),
                source_ai_artifact_id: Some("run-demo-review-20260408".to_owned()),
                follow_up_kind: None,
                warning_lines: Vec::new(),
                confirm_enabled: true,
            }),
            status_line: "Prepared review preflight.".to_owned(),
        });

        assert_eq!(app.current_transient(), Some(TransientLayer::AiPreflight));
        assert!(app.binding_context().ai_preflight_open);

        app.handle(Action::ShowScreen(Screen::Review));

        assert_eq!(app.active_screen, Screen::Review);
        assert!(app.ai_preflight_state().is_none());
        assert_eq!(app.current_transient(), None);
        assert!(!app.binding_context().ai_preflight_open);
    }

    #[test]
    fn open_search_falls_back_to_the_screen_primary_list() {
        let mut timeline = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        timeline.active_screen = Screen::Timeline;
        timeline.set_focused_region(FocusRegion::TimelineControls);

        timeline.handle(Action::OpenSearch);
        assert_eq!(
            timeline.search_state().map(|search| search.scope),
            Some(SearchScope::TimelineEvents)
        );
        assert_eq!(timeline.focused_region(), FocusRegion::TimelineControls);

        let mut review = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        review.active_screen = Screen::Review;
        review.set_focused_region(FocusRegion::ContextPrimary);

        review.handle(Action::OpenSearch);
        assert_eq!(
            review.search_state().map(|search| search.scope),
            Some(SearchScope::ReviewCards)
        );
        assert_eq!(review.focused_region(), FocusRegion::ContextPrimary);

        let mut ai = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        ai.active_screen = Screen::Ai;
        ai.set_focused_region(FocusRegion::ContextPrimary);

        ai.handle(Action::OpenSearch);
        assert_eq!(
            ai.search_state().map(|search| search.scope),
            Some(SearchScope::AiBrowserItems)
        );
        assert_eq!(ai.focused_region(), FocusRegion::ContextPrimary);
    }

    #[test]
    fn top_nav_activation_switches_screens_and_restores_screen_focus() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Review;
        app.set_focused_region(FocusRegion::ContextPrimary);

        app.handle(Action::Back);
        app.handle(Action::Back);
        app.handle(Action::Back);
        assert_eq!(app.focused_region(), FocusRegion::TopNav);

        app.handle(Action::MoveFocusedRegion(navigation::NavMove::Next));
        assert_eq!(app.focused_top_nav_screen(), Screen::Ai);

        app.handle(Action::ActivateFocusedRegion);
        assert_eq!(app.active_screen, Screen::Ai);
        assert_eq!(app.focused_region(), FocusRegion::ContextPrimary);
    }

    #[test]
    fn navigation_smoke_path_covers_screen_switching_search_help_and_back_out() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );

        app.handle(Action::Back);
        assert_eq!(app.focused_region(), FocusRegion::TopNav);

        app.handle(Action::MoveFocusedRegion(navigation::NavMove::Next));
        assert_eq!(app.focused_top_nav_screen(), Screen::Timeline);

        app.handle(Action::ActivateFocusedRegion);
        assert_eq!(app.active_screen, Screen::Timeline);
        assert_eq!(app.focused_region(), FocusRegion::TimelineControls);

        app.handle(Action::FocusNextRegion);
        assert_eq!(app.focused_region(), FocusRegion::TimelineChart);

        app.handle(Action::FocusNextRegion);
        assert_eq!(app.focused_region(), FocusRegion::TimelineLanes);

        app.handle(Action::FocusNextRegion);
        assert_eq!(app.focused_region(), FocusRegion::TimelineInspector);

        app.handle(Action::FocusNextRegion);
        assert_eq!(app.focused_region(), FocusRegion::TimelineEvents);

        app.handle(Action::OpenSearch);
        assert!(app.search_state().is_some());
        app.handle(Action::SearchAppend('c'));
        assert_eq!(
            app.search_state().map(|search| search.query.as_str()),
            Some("c")
        );

        app.handle(Action::Back);
        assert!(app.search_state().is_none());
        assert_eq!(app.focused_region(), FocusRegion::TimelineEvents);

        app.handle(Action::ToggleHelp);
        assert!(app.help_open());

        app.handle(Action::Back);
        assert!(!app.help_open());
        assert_eq!(app.focused_region(), FocusRegion::TimelineEvents);

        app.handle(Action::ActivateFocusedRegion);
        assert_eq!(app.focused_region(), FocusRegion::TimelineInspector);

        app.handle(Action::Back);
        assert_eq!(app.focused_region(), FocusRegion::TimelineEvents);
    }

    #[test]
    fn back_out_walks_back_through_screen_region_order() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Review;
        app.set_focused_region(FocusRegion::ContextPrimary);

        assert_eq!(app.focused_region(), FocusRegion::ContextPrimary);

        app.handle(Action::FocusNextRegion);
        app.handle(Action::FocusNextRegion);
        assert_eq!(app.focused_region(), FocusRegion::Primary);

        app.handle(Action::Back);
        assert_eq!(app.focused_region(), FocusRegion::ContextSecondary);

        app.handle(Action::Back);
        assert_eq!(app.focused_region(), FocusRegion::ContextPrimary);

        app.handle(Action::Back);
        assert_eq!(app.focused_region(), FocusRegion::TopNav);
    }

    #[test]
    fn selector_page_navigation_jumps_to_selector_edges() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Trends;
        app.set_focused_region(FocusRegion::TrendsMatrix);

        assert_eq!(app.focused_region(), FocusRegion::TrendsMatrix);

        app.handle(Action::NextTrendWindow);
        assert_eq!(app.trend_sort_mode, TrendSortMode::Anomaly);

        app.handle(Action::PreviousTrendWindow);
        assert_eq!(app.trend_sort_mode, TrendSortMode::Concern);
    }

    #[test]
    fn pattern_metric_selector_uses_the_shared_selector_contract() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Patterns;
        app.set_focused_region(FocusRegion::ContextPrimary);

        assert_eq!(app.focused_region(), FocusRegion::ContextPrimary);
        assert_eq!(app.pattern_metric_filter, PatternMetricFilter::All);

        app.handle(Action::MoveFocusedRegion(navigation::NavMove::Next));
        assert_eq!(app.pattern_metric_filter, PatternMetricFilter::Activity);

        app.handle(Action::MoveFocusedRegion(navigation::NavMove::Last));
        assert_eq!(app.pattern_metric_filter, PatternMetricFilter::Sleep);

        app.handle(Action::MoveFocusedRegion(navigation::NavMove::PageBackward));
        assert_eq!(app.pattern_metric_filter, PatternMetricFilter::All);
    }

    #[test]
    fn timeline_window_selector_uses_the_shared_selector_contract() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Timeline;
        app.set_focused_region(FocusRegion::TimelineControls);

        assert_eq!(app.timeline_window_hours, 24);

        app.handle(Action::MoveFocusedRegion(navigation::NavMove::Previous));
        assert_eq!(app.timeline_window_hours, 12);

        app.handle(Action::MoveFocusedRegion(navigation::NavMove::First));
        assert_eq!(app.timeline_window_hours, 6);

        app.handle(Action::MoveFocusedRegion(navigation::NavMove::PageForward));
        assert_eq!(app.timeline_window_hours, 24);
    }

    #[test]
    fn explain_overlay_selector_toggles_the_selected_family() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Explain;
        app.set_focused_region(FocusRegion::ContextPrimary);

        assert!(app.overlay_filters.workouts);
        assert!(app.overlay_filters.tags);
        assert!(app.overlay_filters.sessions);

        app.handle(Action::ActivateFocusedRegion);
        assert!(!app.overlay_filters.workouts);

        app.handle(Action::MoveFocusedRegion(navigation::NavMove::Next));
        app.handle(Action::ActivateFocusedRegion);
        assert!(!app.overlay_filters.tags);

        app.handle(Action::MoveFocusedRegion(navigation::NavMove::Next));
        app.handle(Action::ActivateFocusedRegion);
        assert!(!app.overlay_filters.sessions);
    }

    #[test]
    fn patterns_family_selector_uses_the_shared_selector_contract() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Patterns;
        app.set_focused_region(FocusRegion::ContextSecondary);

        assert_eq!(app.selected_overlay_toggle_index, 0);
        assert!(app.overlay_filters.workouts);

        app.handle(Action::MoveFocusedRegion(navigation::NavMove::Last));
        assert_eq!(app.selected_overlay_toggle_index, 2);

        app.handle(Action::ActivateFocusedRegion);
        assert!(!app.overlay_filters.sessions);

        app.handle(Action::MoveFocusedRegion(navigation::NavMove::PageBackward));
        assert_eq!(app.selected_overlay_toggle_index, 0);
    }

    #[test]
    fn ai_launch_points_follow_list_style_navigation() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Ai;
        app.set_focused_region(FocusRegion::ContextPrimary);

        assert_eq!(app.focused_region(), FocusRegion::ContextPrimary);

        app.handle(Action::FocusNextRegion);
        assert_eq!(app.focused_region(), FocusRegion::Primary);
        assert_eq!(app.selected_ai_launch_index(), 0);

        app.handle(Action::MoveFocusedRegion(navigation::NavMove::Next));
        assert_eq!(app.selected_ai_launch_index(), 1);

        app.handle(Action::MoveFocusedRegion(navigation::NavMove::PageForward));
        assert_eq!(
            app.selected_ai_launch_index(),
            app.model.ai.launch_points.len().saturating_sub(1)
        );

        app.handle(Action::MoveFocusedRegion(navigation::NavMove::First));
        assert_eq!(app.selected_ai_launch_index(), 0);
    }

    #[test]
    fn dashboard_footer_updates_when_focus_changes() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Dashboard;

        app.set_focused_region(FocusRegion::DashboardReadiness);
        let readiness_footer = app.footer(ViewportClass::Wide);
        assert!(readiness_footer.contains("Readiness tile"));

        app.set_focused_region(FocusRegion::DashboardSleep);
        let sleep_footer = app.footer(ViewportClass::Wide);
        assert!(sleep_footer.contains("Sleep tile"));
        assert_ne!(sleep_footer, readiness_footer);

        app.set_focused_region(FocusRegion::DashboardHeartRate);
        let heart_rate_footer = app.footer(ViewportClass::Wide);
        assert_ne!(heart_rate_footer, sleep_footer);
        assert!(!heart_rate_footer.contains("Sleep tile"));
        assert!(heart_rate_footer.contains("bpm"));
    }

    #[test]
    fn dashboard_weekly_heatmap_uses_recent_and_history_windows_by_viewport() {
        let mut days = Vec::new();
        for day in 1..=14 {
            days.push(format!("2026-04-{day:02}"));
        }
        let day_refs = days.iter().map(String::as_str).collect::<Vec<_>>();
        let app =
            build_state_from_snapshot(RunMode::Demo, "Demo mode ready.", make_snapshot(&day_refs));

        let weekly = &app.model.dashboard.weekly;
        assert_eq!(weekly.recent.day_labels.len(), 7);
        assert_eq!(weekly.history.day_labels.len(), 14);
        assert_eq!(
            weekly
                .grid_for_viewport(ViewportClass::Medium)
                .day_labels
                .len(),
            7
        );
        assert_eq!(
            weekly
                .grid_for_viewport(ViewportClass::Wide)
                .day_labels
                .len(),
            14
        );
    }

    #[test]
    fn dashboard_breakdown_expansion_is_reversible_with_back() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Dashboard;
        app.set_focused_region(FocusRegion::DashboardBreakdown);

        app.handle(Action::ActivateFocusedRegion);
        assert_eq!(app.expanded_region(), Some(FocusRegion::DashboardBreakdown));

        app.handle(Action::Back);
        assert_eq!(app.expanded_region(), None);
        assert_eq!(app.focused_region(), FocusRegion::DashboardBreakdown);
    }

    #[test]
    fn activating_ai_launch_point_emits_request_ai_launch() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Ai;
        app.set_focused_region(FocusRegion::Primary);

        let emitted = app.handle(Action::ActivateFocusedRegion);

        assert_eq!(
            emitted,
            vec![Action::RequestAiLaunch(AiLaunchIntent::ReviewSelectedDay)]
        );
        assert_eq!(
            app.status_line,
            "Preparing AI review for the selected day preflight."
        );
    }

    #[test]
    fn activating_ai_preflight_confirm_emits_confirm_action() {
        let mut app = build_state_from_snapshot(
            RunMode::Demo,
            "Demo mode ready.",
            make_snapshot(&["2026-04-08"]),
        );
        app.active_screen = Screen::Ai;
        app.ai_preflight = Some(AiPreflightState {
            intent: AiLaunchIntent::ReviewSelectedDay,
            source_screen: Screen::Review,
            snapshot_scope: "day:2026-04-08".to_owned(),
            snapshot_paths: vec!["/tmp/preflight-snapshot.json".to_owned()],
            request_preview: make_ai_preview("demo-snapshot-20260408"),
            privacy_profile: PrivacyProfile::Redacted,
            model_override: Some("gpt-5-mini".to_owned()),
            source_ai_artifact_id: Some("run-demo-review-20260408".to_owned()),
            follow_up_kind: None,
            warning_lines: Vec::new(),
            confirm_enabled: true,
        });
        app.ai_preflight_control = PreflightControl::Confirm;

        let emitted = app.handle(Action::ActivateFocusedRegion);

        assert_eq!(emitted, vec![Action::ConfirmAiPreflight]);
        assert_eq!(app.status_line, "Queueing AI run from preflight.");
        assert!(app.ai_preflight.is_none());
    }

    #[test]
    fn activating_ai_artifact_action_emits_selected_action() {
        let mut snapshot = make_snapshot(&["2026-04-08"]);
        snapshot.ai_runs = vec![make_ai_run_record(
            "run-review-queued",
            "queued",
            Some("artifact-ai-queued"),
            None,
        )];
        let mut app = build_state_from_snapshot(RunMode::Demo, "Demo mode ready.", snapshot);
        app.active_screen = Screen::Ai;
        app.set_focused_region(FocusRegion::Tertiary);

        let emitted = app.handle(Action::ActivateFocusedRegion);

        assert_eq!(
            app.current_ai_artifact_action(),
            Some(Action::RequestCancelAiRun)
        );
        assert_eq!(emitted, vec![Action::RequestCancelAiRun]);
        assert_eq!(app.status_line, "Requesting AI run cancellation.");
    }
}
