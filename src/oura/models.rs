use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::OuraProblem;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityKind {
    Personal,
    Daily,
    Heartrate,
    Workout,
    Session,
    Tag,
    EnhancedTag,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityEntry {
    pub kind: CapabilityKind,
    pub requested: bool,
    pub granted: bool,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityReport {
    pub entries: Vec<CapabilityEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthStatus {
    pub configured: bool,
    pub callback_url: String,
    pub requested_scopes: Vec<String>,
    pub granted_scopes: Vec<String>,
    pub missing_fields: Vec<&'static str>,
    pub capability_report: CapabilityReport,
    pub auth_timeout_secs: u64,
    pub secret_backend: String,
    pub access_token_stored: bool,
    pub refresh_token_stored: bool,
    pub access_token_expires_at: Option<String>,
    pub last_authenticated_at: Option<String>,
    pub last_refresh_at: Option<String>,
    pub account_id: Option<String>,
    pub account_email: Option<String>,
    pub last_error: Option<OuraProblem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailySummary {
    pub day: String,
    pub sleep_score: Option<u8>,
    pub readiness_score: Option<u8>,
    pub activity_score: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartRateSample {
    pub recorded_at: String,
    pub bpm: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkoutRecord {
    pub workout_id: String,
    pub started_at: String,
    pub sport: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagSource {
    Basic,
    Enhanced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRecord {
    pub tag_id: String,
    pub day: String,
    pub label: String,
    pub source: TagSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub session_id: String,
    pub started_at: String,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonalInfoDocument {
    pub id: String,
    pub age: Option<u16>,
    pub weight: Option<f64>,
    pub height: Option<f64>,
    pub biological_sex: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailySleepDocument {
    pub id: String,
    pub day: String,
    pub score: Option<u8>,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyReadinessDocument {
    pub id: String,
    pub day: String,
    pub score: Option<u8>,
    pub temperature_deviation: Option<f64>,
    pub temperature_trend_deviation: Option<f64>,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyActivityDocument {
    pub id: String,
    pub day: String,
    pub score: Option<u8>,
    pub active_calories: i64,
    pub steps: i64,
    pub total_calories: i64,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartRateDocument {
    pub bpm: u16,
    pub source: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkoutDocument {
    pub id: String,
    #[serde(default)]
    pub day: Option<String>,
    #[serde(default, alias = "start_time")]
    pub start_datetime: Option<String>,
    #[serde(default, alias = "end_time")]
    pub end_datetime: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default, alias = "sport_name")]
    pub sport: Option<String>,
    #[serde(default)]
    pub activity: Option<String>,
    #[serde(default)]
    pub intensity: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnhancedTagDocument {
    pub id: String,
    pub day: String,
    #[serde(default, alias = "start_datetime", alias = "start_date")]
    pub start_time: Option<String>,
    #[serde(default, alias = "end_datetime", alias = "end_date")]
    pub end_time: Option<String>,
    #[serde(default)]
    pub tag_type_code: Option<String>,
    #[serde(default, alias = "label")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub intensity: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionDocument {
    pub id: String,
    pub day: String,
    #[serde(default, alias = "start_time")]
    pub start_datetime: Option<String>,
    #[serde(default, alias = "end_time")]
    pub end_datetime: Option<String>,
    #[serde(default, alias = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub score: Option<i64>,
    #[serde(default, alias = "title")]
    pub label: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PagedCollection<T> {
    pub data: Vec<T>,
    pub next_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeSeriesCollection<T> {
    pub data: Vec<T>,
    pub next_token: Option<String>,
}

impl CapabilityReport {
    pub fn from_scopes(requested_scopes: &[String], granted_scopes: &[String]) -> Self {
        let entries = CapabilityKind::all()
            .into_iter()
            .map(|kind| {
                let requested = requested_scopes
                    .iter()
                    .any(|scope| scope == kind.scope_name());
                let granted = granted_scopes
                    .iter()
                    .any(|scope| scope == kind.scope_name());
                let note = match (requested, granted) {
                    (true, true) => "granted".to_owned(),
                    (true, false) => "missing scope".to_owned(),
                    (false, _) => "not requested".to_owned(),
                };

                CapabilityEntry {
                    kind,
                    requested,
                    granted,
                    note,
                }
            })
            .collect();

        Self { entries }
    }

    pub fn demo() -> Self {
        Self {
            entries: CapabilityKind::all()
                .into_iter()
                .map(|kind| CapabilityEntry {
                    kind,
                    requested: true,
                    granted: true,
                    note: "demo data".to_owned(),
                })
                .collect(),
        }
    }

    pub fn available_labels(&self) -> Vec<&'static str> {
        self.entries
            .iter()
            .filter(|entry| entry.granted)
            .map(|entry| entry.kind.label())
            .collect()
    }

    pub fn missing_scope_names(&self) -> Vec<&'static str> {
        self.entries
            .iter()
            .filter(|entry| entry.requested && !entry.granted)
            .map(|entry| entry.kind.scope_name())
            .collect()
    }

    pub fn is_granted(&self, kind: CapabilityKind) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.kind == kind)
            .is_some_and(|entry| entry.granted)
    }

    pub fn status_for(&self, kind: CapabilityKind) -> Option<&CapabilityEntry> {
        self.entries.iter().find(|entry| entry.kind == kind)
    }
}

impl CapabilityKind {
    pub fn all() -> [Self; 7] {
        [
            Self::Personal,
            Self::Daily,
            Self::Heartrate,
            Self::Workout,
            Self::Session,
            Self::Tag,
            Self::EnhancedTag,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Personal => "Personal",
            Self::Daily => "Daily",
            Self::Heartrate => "Heartrate",
            Self::Workout => "Workouts",
            Self::Session => "Sessions",
            Self::Tag => "Tags",
            Self::EnhancedTag => "Enhanced Tags",
        }
    }

    pub fn scope_name(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Daily => "daily",
            Self::Heartrate => "heartrate",
            Self::Workout => "workout",
            Self::Session => "session",
            Self::Tag => "tag",
            Self::EnhancedTag => "enhanced_tag",
        }
    }
}

impl WorkoutDocument {
    pub fn anchor_day(&self) -> String {
        self.day.clone().unwrap_or_else(|| {
            self.start_datetime
                .as_deref()
                .map(|value| value.chars().take(10).collect())
                .unwrap_or_default()
        })
    }

    pub fn title(&self) -> String {
        self.label
            .clone()
            .or_else(|| self.sport.clone())
            .or_else(|| self.activity.clone())
            .unwrap_or_else(|| "Workout".to_owned())
    }

    pub fn subtype(&self) -> Option<String> {
        self.sport.clone().or_else(|| self.activity.clone())
    }
}

impl EnhancedTagDocument {
    pub fn title(&self) -> String {
        if self.tags.is_empty() {
            self.tag_type_code
                .clone()
                .unwrap_or_else(|| "Enhanced Tag".to_owned())
        } else {
            self.tags.join(", ")
        }
    }

    pub fn subtype(&self) -> Option<String> {
        self.tag_type_code.clone()
    }
}

impl SessionDocument {
    pub fn start_at(&self) -> String {
        self.start_datetime
            .clone()
            .unwrap_or_else(|| format!("{}T00:00:00Z", self.day))
    }

    pub fn title(&self) -> String {
        self.label
            .clone()
            .or_else(|| self.kind.clone())
            .unwrap_or_else(|| "Session".to_owned())
    }
}
