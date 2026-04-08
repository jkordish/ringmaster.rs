use crate::oura::models::{CapabilitySet, DailySnapshot};

pub trait OuraClient {
    fn capability_set(&self) -> CapabilitySet;
    fn latest_snapshot(&self) -> DailySnapshot;
}

#[derive(Debug, Default)]
pub struct DemoClient;

impl OuraClient for DemoClient {
    fn capability_set(&self) -> CapabilitySet {
        CapabilitySet::bootstrap_default()
    }

    fn latest_snapshot(&self) -> DailySnapshot {
        DailySnapshot::demo()
    }
}
