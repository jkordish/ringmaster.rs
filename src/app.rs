use crate::oura::models::{CapabilitySet, DailySnapshot, Freshness};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    Timeline,
    Trends,
    Ops,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub title: String,
    pub active_screen: Screen,
    pub freshness: Freshness,
    pub capabilities: CapabilitySet,
    pub snapshot: DailySnapshot,
    pub warnings: Vec<String>,
}

impl AppState {
    pub fn demo() -> Self {
        Self {
            title: "ringmaster.rs".to_owned(),
            active_screen: Screen::Dashboard,
            freshness: Freshness::minutes(4),
            capabilities: CapabilitySet::bootstrap_default(),
            snapshot: DailySnapshot::demo(),
            warnings: vec![
                "poll-first bootstrap in effect".to_owned(),
                "webhooks intentionally deferred".to_owned(),
            ],
        }
    }

    pub fn screen_name(&self) -> &'static str {
        match self.active_screen {
            Screen::Dashboard => "Dashboard",
            Screen::Timeline => "Timeline",
            Screen::Trends => "Trends",
            Screen::Ops => "Ops",
        }
    }
}
