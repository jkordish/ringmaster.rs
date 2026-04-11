use crate::app::Screen;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusRegion {
    TopNav,
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

const DASHBOARD_REGIONS: [FocusRegion; 2] = [FocusRegion::TopNav, FocusRegion::Primary];
const TIMELINE_REGIONS: [FocusRegion; 4] = [
    FocusRegion::TopNav,
    FocusRegion::ContextPrimary,
    FocusRegion::Primary,
    FocusRegion::Secondary,
];
const TRENDS_REGIONS: [FocusRegion; 3] = [
    FocusRegion::TopNav,
    FocusRegion::ContextPrimary,
    FocusRegion::Primary,
];
const EXPLAIN_REGIONS: [FocusRegion; 3] = [
    FocusRegion::TopNav,
    FocusRegion::Primary,
    FocusRegion::Secondary,
];
const PATTERNS_REGIONS: [FocusRegion; 3] = [
    FocusRegion::TopNav,
    FocusRegion::ContextPrimary,
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
const OPS_REGIONS: [FocusRegion; 3] = [
    FocusRegion::TopNav,
    FocusRegion::Primary,
    FocusRegion::Secondary,
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
        Screen::Dashboard | Screen::Explain | Screen::Ops => FocusRegion::Primary,
        Screen::Timeline | Screen::Trends | Screen::Patterns | Screen::Review | Screen::Ai => {
            FocusRegion::ContextPrimary
        }
    }
}

#[must_use]
pub const fn region_label(screen: Screen, region: FocusRegion) -> Option<&'static str> {
    match (screen, region) {
        (_, FocusRegion::TopNav) => Some("Views"),
        (Screen::Dashboard, FocusRegion::Primary) => Some("Drill-down cues"),
        (Screen::Timeline, FocusRegion::ContextPrimary) => Some("Timeline chart"),
        (Screen::Timeline, FocusRegion::Primary) => Some("Day events"),
        (Screen::Timeline, FocusRegion::Secondary) => Some("Selected detail"),
        (Screen::Trends, FocusRegion::ContextPrimary) => Some("Trend windows"),
        (Screen::Trends, FocusRegion::Primary) => Some("Comparison scan"),
        (Screen::Explain, FocusRegion::Primary) => Some("Supporting evidence"),
        (Screen::Explain, FocusRegion::Secondary) => Some("AI launch"),
        (Screen::Patterns, FocusRegion::ContextPrimary) => Some("Pattern filters"),
        (Screen::Patterns, FocusRegion::Primary) => Some("Grouped findings"),
        (Screen::Review, FocusRegion::ContextPrimary) => Some("Mode"),
        (Screen::Review, FocusRegion::ContextSecondary) => Some("Focus"),
        (Screen::Review, FocusRegion::Primary) => Some("Ranked observations"),
        (Screen::Review, FocusRegion::Secondary) => Some("Selected brief"),
        (Screen::Ai, FocusRegion::ContextPrimary) => Some("Browser"),
        (Screen::Ai, FocusRegion::Primary) => Some("Launch points"),
        (Screen::Ai, FocusRegion::Secondary) => Some("Saved artifacts"),
        (Screen::Ai, FocusRegion::Tertiary) => Some("Artifact detail"),
        (Screen::Ops, FocusRegion::Primary) => Some("Family status"),
        (Screen::Ops, FocusRegion::Secondary) => Some("Diagnostics"),
        _ => None,
    }
}

#[must_use]
pub const fn search_scope(screen: Screen, region: FocusRegion) -> Option<SearchScope> {
    match (screen, region) {
        (Screen::Timeline, FocusRegion::Primary) => Some(SearchScope::TimelineEvents),
        (Screen::Review, FocusRegion::Primary) => Some(SearchScope::ReviewCards),
        (Screen::Ai, FocusRegion::Secondary) => Some(SearchScope::AiBrowserItems),
        _ => None,
    }
}

#[must_use]
pub fn next_region(screen: Screen, current: FocusRegion) -> FocusRegion {
    let regions = screen_regions(screen);
    let index = regions
        .iter()
        .position(|region| *region == current)
        .unwrap_or(0);
    regions[(index + 1) % regions.len()]
}

#[must_use]
pub fn previous_region(screen: Screen, current: FocusRegion) -> FocusRegion {
    let regions = screen_regions(screen);
    let index = regions
        .iter()
        .position(|region| *region == current)
        .unwrap_or(0);
    regions[(index + regions.len() - 1) % regions.len()]
}
