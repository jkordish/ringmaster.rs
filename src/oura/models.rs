use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::OuraProblem;
use crate::time_utils::current_local_day_string;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityKind {
    Email,
    Personal,
    Daily,
    Heartrate,
    Workout,
    Session,
    Tag,
    EnhancedTag,
    Spo2,
    RingConfiguration,
    Stress,
    HeartHealth,
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
pub enum TagSource {
    Basic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRecord {
    pub tag_id: String,
    pub day: String,
    pub label: String,
    pub source: TagSource,
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
    #[serde(default, alias = "total_sleep_duration", alias = "duration")]
    pub sleep_duration_seconds: Option<i64>,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SleepDocument {
    pub id: String,
    pub day: String,
    #[serde(default)]
    pub bedtime_start: Option<String>,
    #[serde(default)]
    pub bedtime_end: Option<String>,
    #[serde(default)]
    pub average_heart_rate: Option<f64>,
    #[serde(default)]
    pub average_hrv: Option<f64>,
    #[serde(default)]
    pub average_breath: Option<f64>,
    #[serde(default)]
    pub total_sleep_duration: Option<i64>,
    #[serde(default, rename = "type")]
    pub sleep_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpO2PercentageDocument {
    #[serde(default)]
    pub average: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailySpO2Document {
    pub id: String,
    pub day: String,
    #[serde(default)]
    pub spo2_percentage: Option<SpO2PercentageDocument>,
    #[serde(default)]
    pub breathing_disturbance_index: Option<f64>,
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
pub struct SleepTimeWindow {
    pub day_tz: i32,
    pub end_offset: i32,
    pub start_offset: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SleepTimeRecommendation {
    ImproveEfficiency,
    EarlierBedtime,
    LaterBedtime,
    EarlierWakeUpTime,
    LaterWakeUpTime,
    FollowOptimalBedtime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SleepTimeStatus {
    NotEnoughNights,
    NotEnoughRecentNights,
    BadSleepQuality,
    OnlyRecommendedFound,
    OptimalFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SleepTimeDocument {
    pub id: String,
    pub day: String,
    #[serde(default)]
    pub optimal_bedtime: Option<SleepTimeWindow>,
    #[serde(default)]
    pub recommendation: Option<SleepTimeRecommendation>,
    #[serde(default)]
    pub status: Option<SleepTimeStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyStressDocument {
    pub id: String,
    pub day: String,
    pub stress_high: Option<i64>,
    pub recovery_high: Option<i64>,
    #[serde(default)]
    pub day_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResilienceContributors {
    pub sleep_recovery: f64,
    pub daytime_recovery: f64,
    pub stress: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LongTermResilienceLevel {
    Limited,
    Adequate,
    Solid,
    Strong,
    Exceptional,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyResilienceDocument {
    pub id: String,
    pub day: String,
    pub contributors: ResilienceContributors,
    pub level: LongTermResilienceLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyCardiovascularAgeDocument {
    pub day: String,
    pub vascular_age: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vo2MaxDocument {
    pub id: String,
    pub day: String,
    pub timestamp: String,
    pub vo2_max: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestModeEpisodeDocument {
    pub tags: Vec<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestModePeriodDocument {
    pub id: String,
    pub episodes: Vec<RestModeEpisodeDocument>,
    pub start_day: String,
    pub start_time: Option<String>,
    pub end_day: Option<String>,
    pub end_time: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartRateDocument {
    pub bpm: u16,
    pub source: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnhancedTagDocument {
    pub id: String,
    #[serde(alias = "day")]
    pub start_day: String,
    #[serde(default)]
    pub end_day: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    #[must_use]
    pub fn from_scopes(requested_scopes: &[String], granted_scopes: &[String]) -> Self {
        let requested_scopes = normalize_scopes(requested_scopes);
        let granted_scopes = normalize_scopes(granted_scopes);
        let entries = CapabilityKind::all()
            .into_iter()
            .map(|kind| {
                let requested = requested_scopes
                    .iter()
                    .any(|scope| kind.matches_scope(scope));
                let granted = granted_scopes.iter().any(|scope| kind.matches_scope(scope));
                let note = capability_note(kind, requested, granted, &granted_scopes);

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

    #[must_use]
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

    #[must_use]
    pub fn available_labels(&self) -> Vec<&'static str> {
        self.entries
            .iter()
            .filter(|entry| entry.granted)
            .map(|entry| entry.kind.label())
            .collect()
    }

    #[must_use]
    pub fn missing_scope_names(&self) -> Vec<&'static str> {
        self.scope_names_for(|entry| entry.requested && !entry.granted)
    }

    #[must_use]
    pub fn granted_scope_names(&self) -> Vec<&'static str> {
        self.scope_names_for(|entry| entry.granted)
    }

    fn scope_names_for(&self, predicate: impl Fn(&CapabilityEntry) -> bool) -> Vec<&'static str> {
        self.entries
            .iter()
            .filter(|entry| predicate(entry))
            .map(|entry| entry.kind.scope_name())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    #[must_use]
    pub fn is_granted(&self, kind: CapabilityKind) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.kind == kind)
            .is_some_and(|entry| entry.granted)
    }

    #[must_use]
    pub fn status_for(&self, kind: CapabilityKind) -> Option<&CapabilityEntry> {
        self.entries.iter().find(|entry| entry.kind == kind)
    }
}

impl CapabilityKind {
    #[must_use]
    pub const fn all() -> [Self; 12] {
        [
            Self::Email,
            Self::Personal,
            Self::Daily,
            Self::Heartrate,
            Self::Workout,
            Self::Session,
            Self::Tag,
            Self::EnhancedTag,
            Self::Spo2,
            Self::RingConfiguration,
            Self::Stress,
            Self::HeartHealth,
        ]
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Email => "Email",
            Self::Personal => "Personal",
            Self::Daily => "Daily",
            Self::Heartrate => "Heartrate",
            Self::Workout => "Workouts",
            Self::Session => "Sessions",
            Self::Tag => "Tags",
            Self::EnhancedTag => "Enhanced Tags",
            Self::Spo2 => "SpO2",
            Self::RingConfiguration => "Ring Configuration",
            Self::Stress => "Stress",
            Self::HeartHealth => "Heart Health",
        }
    }

    #[must_use]
    pub const fn scope_name(self) -> &'static str {
        match self {
            Self::Email => "email",
            Self::Personal => "personal",
            Self::Daily => "daily",
            Self::Heartrate => "heartrate",
            Self::Workout => "workout",
            Self::Session => "session",
            Self::Tag | Self::EnhancedTag => "tag",
            Self::Spo2 => "spo2",
            Self::RingConfiguration => "ring_configuration",
            Self::Stress => "stress",
            Self::HeartHealth => "heart_health",
        }
    }

    #[must_use]
    pub fn matches_scope(self, scope: &str) -> bool {
        normalize_scope_name(scope)
            .as_deref()
            .is_some_and(|scope| scope == self.scope_name())
    }

    #[must_use]
    pub const fn is_local_sync_ready(self) -> bool {
        !matches!(self, Self::Email | Self::RingConfiguration)
    }

    #[must_use]
    pub const fn requires_daily_scope_for_local_sync(self) -> bool {
        matches!(self, Self::Spo2 | Self::Stress | Self::HeartHealth)
    }
}

fn capability_note(
    kind: CapabilityKind,
    requested: bool,
    granted: bool,
    granted_scopes: &[String],
) -> String {
    if !requested {
        return "not requested".to_owned();
    }
    if granted {
        if !kind.is_local_sync_ready() {
            return "granted for future support".to_owned();
        }
        if kind.requires_daily_scope_for_local_sync()
            && !granted_scopes
                .iter()
                .any(|scope| CapabilityKind::Daily.matches_scope(scope))
        {
            return "granted; waiting on `daily` scope for local sync".to_owned();
        }
        return "granted".to_owned();
    }
    if kind.is_local_sync_ready() {
        "missing scope".to_owned()
    } else {
        "missing scope; future support only".to_owned()
    }
}

#[must_use]
pub fn normalize_scopes(scopes: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for scope in scopes {
        let Some(scope) = normalize_scope_name(scope) else {
            continue;
        };
        if !normalized.contains(&scope) {
            normalized.push(scope);
        }
    }
    normalized
}

#[must_use]
pub fn normalize_scope_name(scope: &str) -> Option<String> {
    let trimmed = scope.trim();
    if trimmed.is_empty() {
        return None;
    }

    let raw = trimmed.rsplit(':').next().unwrap_or(trimmed).trim();
    let canonical = raw.to_ascii_lowercase().replace([' ', '-'], "_");
    let canonical = match canonical.as_str() {
        "enhanced_tag" => "tag",
        _ => canonical.as_str(),
    };

    Some(canonical.to_owned())
}

impl WorkoutDocument {
    #[must_use]
    pub fn anchor_day(&self) -> String {
        self.day.clone().unwrap_or_else(|| {
            self.start_datetime
                .as_deref()
                .map(|value| value.chars().take(10).collect())
                .unwrap_or_default()
        })
    }

    #[must_use]
    pub fn title(&self) -> String {
        self.label
            .clone()
            .or_else(|| self.sport.clone())
            .or_else(|| self.activity.clone())
            .unwrap_or_else(|| "Workout".to_owned())
    }
}

impl EnhancedTagDocument {
    #[must_use]
    pub const fn anchor_day(&self) -> &str {
        self.start_day.as_str()
    }

    #[must_use]
    pub fn title(&self) -> String {
        if self.tags.is_empty() {
            self.tag_type_code
                .clone()
                .unwrap_or_else(|| "Enhanced Tag".to_owned())
        } else {
            self.tags.join(", ")
        }
    }

    #[must_use]
    pub fn subtype(&self) -> Option<String> {
        self.tag_type_code.clone()
    }
}

impl SessionDocument {
    #[must_use]
    pub fn start_at(&self) -> String {
        self.start_datetime
            .clone()
            .unwrap_or_else(|| format!("{}T00:00:00Z", self.day))
    }

    #[must_use]
    pub fn title(&self) -> String {
        self.label
            .clone()
            .or_else(|| self.kind.clone())
            .unwrap_or_else(|| "Session".to_owned())
    }
}

impl SleepTimeRecommendation {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ImproveEfficiency => "improve_efficiency",
            Self::EarlierBedtime => "earlier_bedtime",
            Self::LaterBedtime => "later_bedtime",
            Self::EarlierWakeUpTime => "earlier_wake_up_time",
            Self::LaterWakeUpTime => "later_wake_up_time",
            Self::FollowOptimalBedtime => "follow_optimal_bedtime",
        }
    }
}

impl SleepTimeStatus {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NotEnoughNights => "not_enough_nights",
            Self::NotEnoughRecentNights => "not_enough_recent_nights",
            Self::BadSleepQuality => "bad_sleep_quality",
            Self::OnlyRecommendedFound => "only_recommended_found",
            Self::OptimalFound => "optimal_found",
        }
    }
}

impl LongTermResilienceLevel {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Limited => "limited",
            Self::Adequate => "adequate",
            Self::Solid => "solid",
            Self::Strong => "strong",
            Self::Exceptional => "exceptional",
        }
    }
}

impl RestModePeriodDocument {
    #[must_use]
    pub fn overlaps_day_window(&self, start_day: &str, end_day: &str) -> bool {
        let current_day = current_local_day_string();
        let effective_end_day = self.end_day.as_deref().unwrap_or(current_day.as_str());
        self.start_day.as_str() <= end_day && effective_end_day >= start_day
    }

    /// # Errors
    ///
    /// Returns an error if the embedded episode tags cannot be serialized to JSON.
    pub fn tags_json(&self) -> Result<String, serde_json::Error> {
        let tags = self
            .episodes
            .iter()
            .map(|episode| episode.tags.clone())
            .collect::<Vec<_>>();
        serde_json::to_string(&tags)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CapabilityKind, CapabilityReport, EnhancedTagDocument, RestModeEpisodeDocument,
        RestModePeriodDocument, normalize_scopes,
    };
    use crate::time_utils::current_local_day_string;

    #[test]
    fn open_rest_mode_period_overlaps_windows_after_start_day() {
        let current_day = current_local_day_string();
        let period = RestModePeriodDocument {
            id: "rest-open".to_owned(),
            episodes: vec![RestModeEpisodeDocument {
                tags: vec!["rest_mode".to_owned()],
                timestamp: "2026-04-02T00:00:00Z".to_owned(),
            }],
            start_day: "2026-04-02".to_owned(),
            start_time: Some("2026-04-02T00:00:00Z".to_owned()),
            end_day: None,
            end_time: None,
        };

        assert!(period.overlaps_day_window("2026-04-03", &current_day));
    }

    #[test]
    fn enhanced_tag_capability_accepts_tag_scope_alias() {
        let report = CapabilityReport::from_scopes(&["tag".to_owned()], &["tag".to_owned()]);

        assert!(report.is_granted(CapabilityKind::Tag));
        assert!(report.is_granted(CapabilityKind::EnhancedTag));
    }

    #[test]
    fn legacy_enhanced_tag_scope_alias_grants_tag_and_enhanced_tag() {
        let report = CapabilityReport::from_scopes(
            &["enhanced_tag".to_owned()],
            &["enhanced_tag".to_owned()],
        );

        assert!(report.is_granted(CapabilityKind::Tag));
        assert!(report.is_granted(CapabilityKind::EnhancedTag));
    }

    #[test]
    fn missing_scope_names_dedupe_shared_tag_scope() {
        let report =
            CapabilityReport::from_scopes(&["tag".to_owned(), "enhanced_tag".to_owned()], &[]);

        assert_eq!(report.missing_scope_names(), vec!["tag"]);
    }

    #[test]
    fn granted_scope_names_dedupe_shared_tag_scope() {
        let report = CapabilityReport::from_scopes(
            &["tag".to_owned(), "enhanced_tag".to_owned()],
            &["tag".to_owned()],
        );

        assert_eq!(report.granted_scope_names(), vec!["tag"]);
    }

    #[test]
    fn capability_notes_reflect_future_ready_and_locally_synced_surfaces() {
        let report = CapabilityReport::from_scopes(
            &["email".to_owned(), "spo2".to_owned()],
            &["email".to_owned()],
        );

        assert_eq!(
            report
                .status_for(CapabilityKind::Email)
                .unwrap_or_else(|| panic!("email capability should exist"))
                .note,
            "granted for future support"
        );
        assert_eq!(
            report
                .status_for(CapabilityKind::Spo2)
                .unwrap_or_else(|| panic!("spo2 capability should exist"))
                .note,
            "missing scope"
        );
    }

    #[test]
    fn daily_derived_capabilities_note_their_daily_sync_dependency() {
        let report = CapabilityReport::from_scopes(
            &["spo2".to_owned(), "stress".to_owned()],
            &["spo2".to_owned(), "stress".to_owned()],
        );

        assert_eq!(
            report
                .status_for(CapabilityKind::Spo2)
                .unwrap_or_else(|| panic!("spo2 capability should exist"))
                .note,
            "granted; waiting on `daily` scope for local sync"
        );
        assert_eq!(
            report
                .status_for(CapabilityKind::Stress)
                .unwrap_or_else(|| panic!("stress capability should exist"))
                .note,
            "granted; waiting on `daily` scope for local sync"
        );
    }

    #[test]
    fn stress_and_heart_health_capabilities_map_to_new_scope_names() {
        let report = CapabilityReport::from_scopes(
            &["stress".to_owned(), "heart_health".to_owned()],
            &["stress".to_owned(), "heart_health".to_owned()],
        );

        assert!(report.is_granted(CapabilityKind::Stress));
        assert!(report.is_granted(CapabilityKind::HeartHealth));
    }

    #[test]
    fn extapi_prefixed_and_display_scopes_normalize_cleanly() {
        let granted = normalize_scopes(&[
            "extapi:daily".to_owned(),
            "SpO2".to_owned(),
            "Ring Configuration".to_owned(),
            "Heart Health".to_owned(),
            "enhanced_tag".to_owned(),
        ]);

        assert_eq!(
            granted,
            vec![
                "daily".to_owned(),
                "spo2".to_owned(),
                "ring_configuration".to_owned(),
                "heart_health".to_owned(),
                "tag".to_owned(),
            ]
        );

        let report = CapabilityReport::from_scopes(&granted, &granted);
        assert!(report.is_granted(CapabilityKind::Daily));
        assert!(report.is_granted(CapabilityKind::Spo2));
        assert!(report.is_granted(CapabilityKind::RingConfiguration));
        assert!(report.is_granted(CapabilityKind::HeartHealth));
        assert!(report.is_granted(CapabilityKind::EnhancedTag));
    }

    #[test]
    fn enhanced_tag_document_accepts_official_start_day_payloads() {
        let document: EnhancedTagDocument = serde_json::from_value(serde_json::json!({
            "id": "etag-1",
            "start_day": "2026-04-10",
            "end_day": "2026-04-10",
            "start_time": "2026-04-10T08:00:00Z",
            "end_time": "2026-04-10T09:00:00Z",
            "tag_type_code": "focus",
            "tags": ["Deep work"]
        }))
        .unwrap_or_else(|error| panic!("official enhanced tag payload should decode: {error}"));

        assert_eq!(document.anchor_day(), "2026-04-10");
        assert_eq!(document.end_day.as_deref(), Some("2026-04-10"));
    }

    #[test]
    fn enhanced_tag_document_accepts_legacy_day_payloads() {
        let document: EnhancedTagDocument = serde_json::from_value(serde_json::json!({
            "id": "etag-legacy",
            "day": "2026-04-11",
            "start_time": "2026-04-11T08:00:00Z",
            "tag_type_code": "caffeine",
            "tags": ["Coffee"]
        }))
        .unwrap_or_else(|error| panic!("legacy enhanced tag payload should decode: {error}"));

        assert_eq!(document.anchor_day(), "2026-04-11");
        assert_eq!(document.end_day, None);
    }
}
