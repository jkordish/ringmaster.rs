#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySet {
    pub daily: bool,
    pub heartrate: bool,
    pub workouts: bool,
    pub sessions: bool,
    pub tags: bool,
}

impl CapabilitySet {
    pub fn bootstrap_default() -> Self {
        Self {
            daily: true,
            heartrate: false,
            workouts: false,
            sessions: false,
            tags: false,
        }
    }

    pub fn render(&self) -> String {
        let capabilities = [
            ("daily", self.daily),
            ("heartrate", self.heartrate),
            ("workouts", self.workouts),
            ("sessions", self.sessions),
            ("tags", self.tags),
        ];

        capabilities
            .into_iter()
            .filter_map(|(name, enabled)| enabled.then_some(name))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Freshness {
    pub minutes_since_sync: u32,
}

impl Freshness {
    pub fn minutes(minutes_since_sync: u32) -> Self {
        Self { minutes_since_sync }
    }

    pub fn render(&self) -> String {
        format!("synced {}m ago", self.minutes_since_sync)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailySnapshot {
    pub sleep_score: u8,
    pub readiness_score: u8,
    pub activity_score: u8,
    pub baseline_7d: String,
    pub baseline_30d: String,
    pub baseline_90d: String,
    pub delta_summary: String,
    pub heart_rate_preview: Vec<String>,
}

impl DailySnapshot {
    pub fn demo() -> Self {
        Self {
            sleep_score: 86,
            readiness_score: 79,
            activity_score: 72,
            baseline_7d: "sleep +2, readiness -1, activity +4".to_owned(),
            baseline_30d: "sleep stable, readiness slightly down".to_owned(),
            baseline_90d: "activity trending up".to_owned(),
            delta_summary: "HRV softer than baseline; late workout likely contributor".to_owned(),
            heart_rate_preview: vec![
                "61".to_owned(),
                "60".to_owned(),
                "59".to_owned(),
                "62".to_owned(),
                "64".to_owned(),
            ],
        }
    }
}
