use crate::app::Screen;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusRegion {
    TopNav,
    DashboardReadiness,
    DashboardSleep,
    DashboardActivity,
    DashboardHrv,
    DashboardTemp,
    DashboardHeartRate,
    DashboardSpo2,
    DashboardRespRate,
    DashboardBreakdown,
    DashboardHeatmap,
    TimelineControls,
    TimelineChart,
    TimelineLanes,
    TimelineInspector,
    TimelineEvents,
    TrendsMatrix,
    TrendsInspector,
    OpsSummary,
    OpsCoverage,
    OpsDiagnostics,
    OpsWarnings,
    ContextPrimary,
    ContextSecondary,
    Primary,
    Secondary,
    Tertiary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavMove {
    Previous,
    Next,
    First,
    Last,
    PageBackward,
    PageForward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchScope {
    TimelineEvents,
    ReviewCards,
    AiBrowserItems,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransientLayer {
    Help,
    Search,
    AiPreflight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchState {
    pub scope: SearchScope,
    pub query: String,
    pub active_match_index: usize,
    pub total_matches: usize,
    pub previous_region: FocusRegion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreflightControl {
    Confirm,
    Privacy,
    Cancel,
}

impl PreflightControl {
    pub const ALL: [Self; 3] = [Self::Confirm, Self::Privacy, Self::Cancel];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Confirm => "Confirm",
            Self::Privacy => "Rotate privacy",
            Self::Cancel => "Cancel",
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Confirm => Self::Privacy,
            Self::Privacy => Self::Cancel,
            Self::Cancel => Self::Confirm,
        }
    }

    #[must_use]
    pub const fn previous(self) -> Self {
        match self {
            Self::Confirm => Self::Cancel,
            Self::Privacy => Self::Confirm,
            Self::Cancel => Self::Privacy,
        }
    }
}

const DASHBOARD_REGIONS: [FocusRegion; 11] = [
    FocusRegion::TopNav,
    FocusRegion::DashboardReadiness,
    FocusRegion::DashboardSleep,
    FocusRegion::DashboardActivity,
    FocusRegion::DashboardHrv,
    FocusRegion::DashboardTemp,
    FocusRegion::DashboardHeartRate,
    FocusRegion::DashboardSpo2,
    FocusRegion::DashboardRespRate,
    FocusRegion::DashboardBreakdown,
    FocusRegion::DashboardHeatmap,
];
const TIMELINE_REGIONS: [FocusRegion; 6] = [
    FocusRegion::TopNav,
    FocusRegion::TimelineControls,
    FocusRegion::TimelineChart,
    FocusRegion::TimelineLanes,
    FocusRegion::TimelineInspector,
    FocusRegion::TimelineEvents,
];
const TRENDS_REGIONS: [FocusRegion; 3] = [
    FocusRegion::TopNav,
    FocusRegion::TrendsMatrix,
    FocusRegion::TrendsInspector,
];
const EXPLAIN_REGIONS: [FocusRegion; 3] = [
    FocusRegion::TopNav,
    FocusRegion::ContextPrimary,
    FocusRegion::Primary,
];
const PATTERNS_REGIONS: [FocusRegion; 4] = [
    FocusRegion::TopNav,
    FocusRegion::ContextPrimary,
    FocusRegion::ContextSecondary,
    FocusRegion::Primary,
];
const REVIEW_REGIONS: [FocusRegion; 5] = [
    FocusRegion::TopNav,
    FocusRegion::ContextPrimary,
    FocusRegion::ContextSecondary,
    FocusRegion::Primary,
    FocusRegion::Secondary,
];
const AI_REGIONS: [FocusRegion; 5] = [
    FocusRegion::TopNav,
    FocusRegion::ContextPrimary,
    FocusRegion::Primary,
    FocusRegion::Secondary,
    FocusRegion::Tertiary,
];
const OPS_REGIONS: [FocusRegion; 5] = [
    FocusRegion::TopNav,
    FocusRegion::OpsSummary,
    FocusRegion::OpsCoverage,
    FocusRegion::OpsDiagnostics,
    FocusRegion::OpsWarnings,
];

#[must_use]
pub const fn screen_regions(screen: Screen) -> &'static [FocusRegion] {
    match screen {
        Screen::Dashboard => &DASHBOARD_REGIONS,
        Screen::Timeline => &TIMELINE_REGIONS,
        Screen::Trends => &TRENDS_REGIONS,
        Screen::Explain => &EXPLAIN_REGIONS,
        Screen::Patterns => &PATTERNS_REGIONS,
        Screen::Review => &REVIEW_REGIONS,
        Screen::Ai => &AI_REGIONS,
        Screen::Ops => &OPS_REGIONS,
    }
}

#[must_use]
pub const fn default_region(screen: Screen) -> FocusRegion {
    match screen {
        Screen::Dashboard => FocusRegion::DashboardReadiness,
        Screen::Ops => FocusRegion::OpsSummary,
        Screen::Explain | Screen::Patterns | Screen::Review | Screen::Ai => {
            FocusRegion::ContextPrimary
        }
        Screen::Timeline => FocusRegion::TimelineControls,
        Screen::Trends => FocusRegion::TrendsMatrix,
    }
}

#[must_use]
pub const fn region_label(screen: Screen, region: FocusRegion) -> Option<&'static str> {
    match (screen, region) {
        (_, FocusRegion::TopNav) => Some("Views"),
        (Screen::Dashboard, FocusRegion::DashboardReadiness) => Some("Readiness tile"),
        (Screen::Dashboard, FocusRegion::DashboardSleep) => Some("Sleep tile"),
        (Screen::Dashboard, FocusRegion::DashboardActivity) => Some("Activity tile"),
        (Screen::Dashboard, FocusRegion::DashboardHrv) => Some("HRV trend"),
        (Screen::Dashboard, FocusRegion::DashboardTemp) => Some("Body temperature"),
        (Screen::Dashboard, FocusRegion::DashboardHeartRate) => Some("Heart rate"),
        (Screen::Dashboard, FocusRegion::DashboardSpo2) => Some("SpO2"),
        (Screen::Dashboard, FocusRegion::DashboardRespRate) => Some("Respiratory rate"),
        (Screen::Dashboard, FocusRegion::DashboardBreakdown) => Some("Readiness breakdown"),
        (Screen::Dashboard, FocusRegion::DashboardHeatmap) => Some("Weekly trends"),
        (Screen::Timeline, FocusRegion::TimelineControls) => Some("Timeline controls"),
        (Screen::Timeline, FocusRegion::TimelineChart) => Some("Timeline chart"),
        (Screen::Timeline, FocusRegion::TimelineLanes) => Some("Overlay lanes"),
        (Screen::Timeline, FocusRegion::TimelineInspector) => Some("Timeline detail"),
        (Screen::Timeline, FocusRegion::TimelineEvents) => Some("Event feed"),
        (Screen::Explain, FocusRegion::ContextPrimary)
        | (Screen::Timeline, FocusRegion::ContextSecondary) => Some("Overlay filters"),
        (Screen::Trends, FocusRegion::TrendsMatrix) => Some("Trend matrix"),
        (Screen::Trends, FocusRegion::TrendsInspector) => Some("Trend detail"),
        (Screen::Explain, FocusRegion::Primary) => Some("Explain body"),
        (Screen::Patterns, FocusRegion::ContextPrimary) => Some("Metric filter"),
        (Screen::Patterns, FocusRegion::ContextSecondary) => Some("Family filter"),
        (Screen::Patterns, FocusRegion::Primary) => Some("Patterns browser"),
        (Screen::Review, FocusRegion::ContextPrimary) => Some("Mode"),
        (Screen::Review, FocusRegion::ContextSecondary) => Some("Focus"),
        (Screen::Review, FocusRegion::Primary) => Some("Ranked observations"),
        (Screen::Review, FocusRegion::Secondary) => Some("Selected brief"),
        (Screen::Ai, FocusRegion::ContextPrimary) => Some("Browser"),
        (Screen::Ai, FocusRegion::Primary) => Some("Launch points"),
        (Screen::Ai, FocusRegion::Secondary) => Some("Saved artifacts"),
        (Screen::Ai, FocusRegion::Tertiary) => Some("Artifact actions"),
        (Screen::Ops, FocusRegion::OpsSummary) => Some("Status summary"),
        (Screen::Ops, FocusRegion::OpsCoverage) => Some("Coverage matrix"),
        (Screen::Ops, FocusRegion::OpsDiagnostics) => Some("Diagnostics"),
        (Screen::Ops, FocusRegion::OpsWarnings) => Some("Warnings"),
        _ => None,
    }
}

#[must_use]
pub const fn search_scope(screen: Screen, region: FocusRegion) -> Option<SearchScope> {
    match (screen, region) {
        (Screen::Timeline, FocusRegion::TimelineEvents) => Some(SearchScope::TimelineEvents),
        (Screen::Review, FocusRegion::Primary) => Some(SearchScope::ReviewCards),
        (Screen::Ai, FocusRegion::Secondary) => Some(SearchScope::AiBrowserItems),
        _ => None,
    }
}

#[must_use]
pub const fn default_search_scope(screen: Screen) -> Option<SearchScope> {
    match screen {
        Screen::Timeline => Some(SearchScope::TimelineEvents),
        Screen::Review => Some(SearchScope::ReviewCards),
        Screen::Ai => Some(SearchScope::AiBrowserItems),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{FocusRegion, SearchScope, default_region, default_search_scope, screen_regions};
    use crate::app::Screen;

    #[test]
    fn read_mostly_screens_only_expose_real_focus_stops() {
        assert_eq!(
            screen_regions(Screen::Ops),
            &[
                FocusRegion::TopNav,
                FocusRegion::OpsSummary,
                FocusRegion::OpsCoverage,
                FocusRegion::OpsDiagnostics,
                FocusRegion::OpsWarnings,
            ]
        );
    }

    #[test]
    fn default_region_matches_the_first_real_body_region() {
        assert_eq!(
            default_region(Screen::Patterns),
            FocusRegion::ContextPrimary
        );
        assert_eq!(default_region(Screen::Explain), FocusRegion::ContextPrimary);
        assert_eq!(default_region(Screen::Ops), FocusRegion::OpsSummary);
        assert_eq!(
            default_region(Screen::Dashboard),
            FocusRegion::DashboardReadiness
        );
    }

    #[test]
    fn default_search_scope_points_at_each_screen_primary_list() {
        assert_eq!(
            default_search_scope(Screen::Timeline),
            Some(SearchScope::TimelineEvents)
        );
        assert_eq!(
            default_search_scope(Screen::Review),
            Some(SearchScope::ReviewCards)
        );
        assert_eq!(
            default_search_scope(Screen::Ai),
            Some(SearchScope::AiBrowserItems)
        );
        assert_eq!(default_search_scope(Screen::Patterns), None);
    }
}
