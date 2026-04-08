use crate::app::{LiveSnapshot, Screen};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Tick,
    Quit,
    NextScreen,
    PreviousScreen,
    ShowScreen(Screen),
    RefreshRequested,
    RefreshStarted {
        families: Vec<String>,
        manual: bool,
    },
    LiveSnapshotLoaded {
        snapshot: Box<LiveSnapshot>,
        summary: String,
    },
    RefreshFailed {
        message: String,
    },
    PreviousDay,
    NextDay,
    PreviousTimelinePoint,
    NextTimelinePoint,
    PreviousEvent,
    NextEvent,
    TimelineZoomIn,
    TimelineZoomOut,
    ToggleWorkoutFilter,
    ToggleTagFilter,
    ToggleSessionFilter,
    PreviousTrendWindow,
    NextTrendWindow,
    CyclePatternMetric,
}
