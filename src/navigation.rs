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
const TIMELINE_REGIONS: [FocusRegion; 6] = [
    FocusRegion::TopNav,
    FocusRegion::ContextPrimary,
    FocusRegion::ContextSecondary,
    FocusRegion::Primary,
    FocusRegion::Secondary,
    FocusRegion::Tertiary,
];
const TRENDS_REGIONS: [FocusRegion; 3] = [
    FocusRegion::TopNav,
    FocusRegion::ContextPrimary,
    FocusRegion::Primary,
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
const OPS_REGIONS: [FocusRegion; 2] = [FocusRegion::TopNav, FocusRegion::Primary];

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
        Screen::Dashboard | Screen::Ops => FocusRegion::Primary,
        Screen::Explain | Screen::Patterns => FocusRegion::ContextPrimary,
        Screen::Timeline | Screen::Trends | Screen::Review | Screen::Ai => {
            FocusRegion::ContextPrimary
        }
    }
}

#[must_use]
pub const fn region_label(screen: Screen, region: FocusRegion) -> Option<&'static str> {
    match (screen, region) {
        (_, FocusRegion::TopNav) => Some("Views"),
        (Screen::Dashboard, FocusRegion::Primary) => Some("Dashboard body"),
        (Screen::Timeline, FocusRegion::ContextPrimary) => Some("Window presets"),
        (Screen::Timeline, FocusRegion::ContextSecondary)
        | (Screen::Explain, FocusRegion::ContextPrimary) => Some("Overlay filters"),
        (Screen::Timeline, FocusRegion::Primary) => Some("Timeline chart"),
        (Screen::Timeline, FocusRegion::Secondary) => Some("Day events"),
        (Screen::Timeline, FocusRegion::Tertiary) => Some("Selected detail"),
        (Screen::Trends, FocusRegion::ContextPrimary) => Some("Trend windows"),
        (Screen::Trends, FocusRegion::Primary) => Some("Comparison scan"),
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
        (Screen::Ops, FocusRegion::Primary) => Some("Status console"),
        _ => None,
    }
}

#[must_use]
pub const fn search_scope(screen: Screen, region: FocusRegion) -> Option<SearchScope> {
    match (screen, region) {
        (Screen::Timeline, FocusRegion::Secondary) => Some(SearchScope::TimelineEvents),
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

#[cfg(test)]
mod tests {
    use super::{FocusRegion, default_region, screen_regions};
    use crate::app::Screen;

    #[test]
    fn read_mostly_screens_only_expose_real_focus_stops() {
        assert_eq!(
            screen_regions(Screen::Ops),
            &[FocusRegion::TopNav, FocusRegion::Primary]
        );
    }

    #[test]
    fn default_region_matches_the_first_real_body_region() {
        assert_eq!(
            default_region(Screen::Patterns),
            FocusRegion::ContextPrimary
        );
        assert_eq!(default_region(Screen::Explain), FocusRegion::ContextPrimary);
        assert_eq!(default_region(Screen::Ops), FocusRegion::Primary);
    }
}
