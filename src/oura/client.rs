use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use reqwest::{Client as HttpClient, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::config::Config;
use crate::error::{OuraApiError, OuraProblem, Result, RingmasterError};
use crate::oura::models::{
    CapabilityKind, CapabilityReport, DailyActivityDocument, DailyCardiovascularAgeDocument,
    DailyReadinessDocument, DailyResilienceDocument, DailySleepDocument, DailyStressDocument,
    EnhancedTagDocument, HeartRateDocument, PagedCollection, PersonalInfoDocument,
    RestModePeriodDocument, SessionDocument, SleepTimeDocument, TimeSeriesCollection,
    Vo2MaxDocument, WorkoutDocument,
};
use crate::store::queries::RawPayloadRecord;

type ClientFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

const OURA_API_USER_AGENT: &str = "ringmaster.rs/oura-api";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingleFetch<T> {
    pub raw_payload: RawPayloadRecord,
    pub document: T,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageFetch<T> {
    pub raw_payload: RawPayloadRecord,
    pub documents: Vec<T>,
}

pub trait OuraClient {
    fn capability_report(&self) -> CapabilityReport;

    fn fetch_personal_info(&self) -> ClientFuture<'_, SingleFetch<PersonalInfoDocument>>;
    fn fetch_daily_sleep(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailySleepDocument>>>;
    fn fetch_daily_readiness(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailyReadinessDocument>>>;
    fn fetch_daily_activity(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailyActivityDocument>>>;
    fn fetch_sleep_time(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<SleepTimeDocument>>>;
    fn fetch_rest_mode_periods(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<RestModePeriodDocument>>>;
    fn fetch_daily_stress(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailyStressDocument>>>;
    fn fetch_daily_resilience(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailyResilienceDocument>>>;
    fn fetch_daily_cardiovascular_age(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailyCardiovascularAgeDocument>>>;
    fn fetch_vo2_max(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<Vo2MaxDocument>>>;
    fn fetch_heartrate(
        &self,
        start_datetime: String,
        end_datetime: String,
    ) -> ClientFuture<'_, Vec<PageFetch<HeartRateDocument>>>;
    fn fetch_workouts(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<WorkoutDocument>>>;
    fn fetch_enhanced_tags(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<EnhancedTagDocument>>>;
    fn fetch_sessions(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<SessionDocument>>>;
}

#[derive(Debug, Clone)]
pub struct ReqwestOuraClient {
    http: HttpClient,
    api_base_url: String,
    access_token: String,
    capability_report: CapabilityReport,
}

#[derive(Debug, Clone)]
pub struct FixtureOuraClient {
    fixture_dir: PathBuf,
    capability_report: CapabilityReport,
}

impl ReqwestOuraClient {
    pub fn new(config: &Config, access_token: String, granted_scopes: &[String]) -> Result<Self> {
        let http = HttpClient::builder()
            .user_agent(OURA_API_USER_AGENT)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;

        Ok(Self {
            http,
            api_base_url: config.oura.api_base_url.trim_end_matches('/').to_owned(),
            access_token,
            capability_report: CapabilityReport::from_scopes(
                &config.oura.requested_scopes,
                granted_scopes,
            ),
        })
    }

    async fn fetch_single_document<T>(
        &self,
        endpoint: &'static str,
        scope: &'static str,
    ) -> Result<SingleFetch<T>>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let url = format!("{}/v2/usercollection/{endpoint}", self.api_base_url);
        let requested_at = now_rfc3339()?;
        let response = self
            .http
            .get(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await?;
        let status = response.status();
        let payload = response.text().await?;
        if !status.is_success() {
            return Err(OuraApiError::Problem(parse_api_problem(status, &payload)).into());
        }
        let document = serde_json::from_str(&payload)
            .map_err(|source| OuraApiError::Decode { endpoint, source })?;

        Ok(SingleFetch {
            raw_payload: RawPayloadRecord {
                cache_key: format!("{endpoint}|snapshot"),
                endpoint: endpoint.to_owned(),
                requested_at,
                scope: Some(scope.to_owned()),
                etag: None,
                payload,
            },
            document,
        })
    }

    async fn fetch_paged_collection<T>(
        &self,
        endpoint: &'static str,
        scope: &'static str,
        base_params: Vec<(&'static str, String)>,
    ) -> Result<Vec<PageFetch<T>>>
    where
        T: DeserializeOwned + Clone + Send + 'static,
    {
        let mut pages = Vec::new();
        let mut next_token = None;

        loop {
            let mut params = base_params.clone();
            if let Some(token) = next_token.clone() {
                params.push(("next_token", token));
            }

            let url = format!("{}/v2/usercollection/{endpoint}", self.api_base_url);
            let requested_at = now_rfc3339()?;
            let response = self
                .http
                .get(&url)
                .bearer_auth(&self.access_token)
                .query(&params)
                .send()
                .await?;
            let status = response.status();
            let payload = response.text().await?;
            if !status.is_success() {
                return Err(OuraApiError::Problem(parse_api_problem(status, &payload)).into());
            }

            let page: PagedCollection<T> = serde_json::from_str(&payload)
                .map_err(|source| OuraApiError::Decode { endpoint, source })?;
            let current_next_token = page.next_token.clone();
            pages.push(PageFetch {
                raw_payload: RawPayloadRecord {
                    cache_key: cache_key(endpoint, &params),
                    endpoint: endpoint.to_owned(),
                    requested_at,
                    scope: Some(scope.to_owned()),
                    etag: None,
                    payload,
                },
                documents: page.data.clone(),
            });

            if let Some(token) = current_next_token {
                next_token = Some(token);
            } else {
                break;
            }
        }

        Ok(pages)
    }

    async fn fetch_timeseries<T>(
        &self,
        endpoint: &'static str,
        scope: &'static str,
        base_params: Vec<(&'static str, String)>,
    ) -> Result<Vec<PageFetch<T>>>
    where
        T: DeserializeOwned + Clone + Send + 'static,
    {
        let mut pages = Vec::new();
        let mut next_token = None;

        loop {
            let mut params = base_params.clone();
            if let Some(token) = next_token.clone() {
                params.push(("next_token", token));
            }

            let url = format!("{}/v2/usercollection/{endpoint}", self.api_base_url);
            let requested_at = now_rfc3339()?;
            let response = self
                .http
                .get(&url)
                .bearer_auth(&self.access_token)
                .query(&params)
                .send()
                .await?;
            let status = response.status();
            let payload = response.text().await?;
            if !status.is_success() {
                return Err(OuraApiError::Problem(parse_api_problem(status, &payload)).into());
            }

            let page: TimeSeriesCollection<T> = serde_json::from_str(&payload)
                .map_err(|source| OuraApiError::Decode { endpoint, source })?;
            let current_next_token = page.next_token.clone();
            pages.push(PageFetch {
                raw_payload: RawPayloadRecord {
                    cache_key: cache_key(endpoint, &params),
                    endpoint: endpoint.to_owned(),
                    requested_at,
                    scope: Some(scope.to_owned()),
                    etag: None,
                    payload,
                },
                documents: page.data.clone(),
            });

            if let Some(token) = current_next_token {
                next_token = Some(token);
            } else {
                break;
            }
        }

        Ok(pages)
    }
}

impl OuraClient for ReqwestOuraClient {
    fn capability_report(&self) -> CapabilityReport {
        self.capability_report.clone()
    }

    fn fetch_personal_info(&self) -> ClientFuture<'_, SingleFetch<PersonalInfoDocument>> {
        Box::pin(async move {
            self.fetch_single_document("personal_info", "personal")
                .await
        })
    }

    fn fetch_daily_sleep(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailySleepDocument>>> {
        Box::pin(async move {
            self.fetch_paged_collection(
                "daily_sleep",
                "daily",
                vec![("start_date", start_date), ("end_date", end_date)],
            )
            .await
        })
    }

    fn fetch_daily_readiness(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailyReadinessDocument>>> {
        Box::pin(async move {
            self.fetch_paged_collection(
                "daily_readiness",
                "daily",
                vec![("start_date", start_date), ("end_date", end_date)],
            )
            .await
        })
    }

    fn fetch_daily_activity(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailyActivityDocument>>> {
        Box::pin(async move {
            self.fetch_paged_collection(
                "daily_activity",
                "daily",
                vec![("start_date", start_date), ("end_date", end_date)],
            )
            .await
        })
    }

    fn fetch_sleep_time(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<SleepTimeDocument>>> {
        Box::pin(async move {
            self.fetch_paged_collection(
                "sleep_time",
                "daily",
                vec![("start_date", start_date), ("end_date", end_date)],
            )
            .await
        })
    }

    fn fetch_rest_mode_periods(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<RestModePeriodDocument>>> {
        Box::pin(async move {
            self.fetch_paged_collection(
                "rest_mode_period",
                "daily",
                vec![("start_date", start_date), ("end_date", end_date)],
            )
            .await
        })
    }

    fn fetch_daily_stress(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailyStressDocument>>> {
        Box::pin(async move {
            self.fetch_paged_collection(
                "daily_stress",
                "daily",
                vec![("start_date", start_date), ("end_date", end_date)],
            )
            .await
        })
    }

    fn fetch_daily_resilience(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailyResilienceDocument>>> {
        Box::pin(async move {
            self.fetch_paged_collection(
                "daily_resilience",
                "daily",
                vec![("start_date", start_date), ("end_date", end_date)],
            )
            .await
        })
    }

    fn fetch_daily_cardiovascular_age(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailyCardiovascularAgeDocument>>> {
        Box::pin(async move {
            self.fetch_paged_collection(
                "daily_cardiovascular_age",
                "daily",
                vec![("start_date", start_date), ("end_date", end_date)],
            )
            .await
        })
    }

    fn fetch_vo2_max(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<Vo2MaxDocument>>> {
        Box::pin(async move {
            self.fetch_paged_collection(
                "vO2_max",
                "daily",
                vec![("start_date", start_date), ("end_date", end_date)],
            )
            .await
        })
    }

    fn fetch_heartrate(
        &self,
        start_datetime: String,
        end_datetime: String,
    ) -> ClientFuture<'_, Vec<PageFetch<HeartRateDocument>>> {
        Box::pin(async move {
            self.fetch_timeseries(
                "heartrate",
                "heartrate",
                vec![
                    ("start_datetime", start_datetime),
                    ("end_datetime", end_datetime),
                ],
            )
            .await
        })
    }

    fn fetch_workouts(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<WorkoutDocument>>> {
        Box::pin(async move {
            self.fetch_paged_collection(
                "workout",
                "workout",
                vec![("start_date", start_date), ("end_date", end_date)],
            )
            .await
        })
    }

    fn fetch_enhanced_tags(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<EnhancedTagDocument>>> {
        Box::pin(async move {
            self.fetch_paged_collection(
                "enhanced_tag",
                "enhanced_tag",
                vec![("start_date", start_date), ("end_date", end_date)],
            )
            .await
        })
    }

    fn fetch_sessions(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<SessionDocument>>> {
        Box::pin(async move {
            self.fetch_paged_collection(
                "session",
                "session",
                vec![("start_date", start_date), ("end_date", end_date)],
            )
            .await
        })
    }
}

impl FixtureOuraClient {
    pub fn new(config: &Config, fixture_dir: impl Into<PathBuf>) -> Result<Self> {
        let fixture_dir = fixture_dir.into();
        let granted_scopes = available_fixture_scopes(&fixture_dir);
        Ok(Self {
            fixture_dir,
            capability_report: CapabilityReport::from_scopes(
                &config.oura.requested_scopes,
                &granted_scopes,
            ),
        })
    }

    fn load_json(&self, name: &str) -> Result<String> {
        std::fs::read_to_string(self.fixture_dir.join(name))
            .map_err(|error| RingmasterError::io("reading fixture payload", error))
    }

    fn load_single<T>(
        &self,
        file_name: &str,
        endpoint: &'static str,
        scope: &'static str,
    ) -> Result<SingleFetch<T>>
    where
        T: DeserializeOwned,
    {
        let payload = self.load_json(file_name)?;
        let document = serde_json::from_str(&payload)
            .map_err(|source| OuraApiError::Decode { endpoint, source })?;

        Ok(SingleFetch {
            raw_payload: RawPayloadRecord {
                cache_key: format!("{endpoint}|fixture"),
                endpoint: endpoint.to_owned(),
                requested_at: now_rfc3339()?,
                scope: Some(scope.to_owned()),
                etag: None,
                payload,
            },
            document,
        })
    }

    fn load_paged<T>(
        &self,
        file_name: &str,
        endpoint: &'static str,
        scope: &'static str,
        date_field: &'static str,
        start: &str,
        end: &str,
    ) -> Result<Vec<PageFetch<T>>>
    where
        T: DeserializeOwned + Clone + SerializeableDocument,
    {
        if !self.fixture_dir.join(file_name).is_file() {
            return Ok(Vec::new());
        }
        let payload = self.load_json(file_name)?;
        let page: PagedCollection<T> = serde_json::from_str(&payload)
            .map_err(|source| OuraApiError::Decode { endpoint, source })?;
        let documents = page
            .data
            .into_iter()
            .filter(|document| {
                document
                    .field_value(date_field)
                    .is_some_and(|value| value >= start && value <= end)
            })
            .collect::<Vec<_>>();
        let filtered_payload = serde_json::to_string(&PagedCollection {
            data: documents.clone(),
            next_token: None::<String>,
        })?;

        Ok(vec![PageFetch {
            raw_payload: RawPayloadRecord {
                cache_key: format!("{endpoint}|fixture|{start}|{end}"),
                endpoint: endpoint.to_owned(),
                requested_at: now_rfc3339()?,
                scope: Some(scope.to_owned()),
                etag: None,
                payload: filtered_payload,
            },
            documents,
        }])
    }

    fn load_paged_filter<T, F>(
        &self,
        file_name: &str,
        endpoint: &'static str,
        scope: &'static str,
        predicate: F,
    ) -> Result<Vec<PageFetch<T>>>
    where
        T: DeserializeOwned + Clone + SerializeableDocument,
        F: Fn(&T) -> bool,
    {
        if !self.fixture_dir.join(file_name).is_file() {
            return Ok(Vec::new());
        }
        let payload = self.load_json(file_name)?;
        let page: PagedCollection<T> = serde_json::from_str(&payload)
            .map_err(|source| OuraApiError::Decode { endpoint, source })?;
        let documents = page.data.into_iter().filter(predicate).collect::<Vec<_>>();
        let filtered_payload = serde_json::to_string(&PagedCollection {
            data: documents.clone(),
            next_token: None::<String>,
        })?;

        Ok(vec![PageFetch {
            raw_payload: RawPayloadRecord {
                cache_key: format!("{endpoint}|fixture"),
                endpoint: endpoint.to_owned(),
                requested_at: now_rfc3339()?,
                scope: Some(scope.to_owned()),
                etag: None,
                payload: filtered_payload,
            },
            documents,
        }])
    }

    fn load_timeseries<T>(
        &self,
        file_name: &str,
        endpoint: &'static str,
        scope: &'static str,
        timestamp_field: &'static str,
        start: &str,
        end: &str,
    ) -> Result<Vec<PageFetch<T>>>
    where
        T: DeserializeOwned + Clone + SerializeableDocument,
    {
        if !self.fixture_dir.join(file_name).is_file() {
            return Ok(Vec::new());
        }
        let payload = self.load_json(file_name)?;
        let page: TimeSeriesCollection<T> = serde_json::from_str(&payload)
            .map_err(|source| OuraApiError::Decode { endpoint, source })?;
        let documents = page
            .data
            .into_iter()
            .filter(|document| {
                document
                    .field_value(timestamp_field)
                    .is_some_and(|value| value >= start && value <= end)
            })
            .collect::<Vec<_>>();
        let filtered_payload = serde_json::to_string(&TimeSeriesCollection {
            data: documents.clone(),
            next_token: None::<String>,
        })?;

        Ok(vec![PageFetch {
            raw_payload: RawPayloadRecord {
                cache_key: format!("{endpoint}|fixture|{start}|{end}"),
                endpoint: endpoint.to_owned(),
                requested_at: now_rfc3339()?,
                scope: Some(scope.to_owned()),
                etag: None,
                payload: filtered_payload,
            },
            documents,
        }])
    }
}

impl OuraClient for FixtureOuraClient {
    fn capability_report(&self) -> CapabilityReport {
        self.capability_report.clone()
    }

    fn fetch_personal_info(&self) -> ClientFuture<'_, SingleFetch<PersonalInfoDocument>> {
        Box::pin(async move { self.load_single("personal_info.json", "personal_info", "personal") })
    }

    fn fetch_daily_sleep(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailySleepDocument>>> {
        Box::pin(async move {
            self.load_paged(
                "daily_sleep.json",
                "daily_sleep",
                "daily",
                "day",
                &start_date,
                &end_date,
            )
        })
    }

    fn fetch_daily_readiness(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailyReadinessDocument>>> {
        Box::pin(async move {
            self.load_paged(
                "daily_readiness.json",
                "daily_readiness",
                "daily",
                "day",
                &start_date,
                &end_date,
            )
        })
    }

    fn fetch_daily_activity(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailyActivityDocument>>> {
        Box::pin(async move {
            self.load_paged(
                "daily_activity.json",
                "daily_activity",
                "daily",
                "day",
                &start_date,
                &end_date,
            )
        })
    }

    fn fetch_sleep_time(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<SleepTimeDocument>>> {
        Box::pin(async move {
            self.load_paged(
                "sleep_time.json",
                "sleep_time",
                "daily",
                "day",
                &start_date,
                &end_date,
            )
        })
    }

    fn fetch_rest_mode_periods(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<RestModePeriodDocument>>> {
        Box::pin(async move {
            self.load_paged_filter(
                "rest_mode_periods.json",
                "rest_mode_period",
                "daily",
                |document: &RestModePeriodDocument| {
                    document.overlaps_day_window(&start_date, &end_date)
                },
            )
        })
    }

    fn fetch_daily_stress(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailyStressDocument>>> {
        Box::pin(async move {
            self.load_paged(
                "daily_stress.json",
                "daily_stress",
                "daily",
                "day",
                &start_date,
                &end_date,
            )
        })
    }

    fn fetch_daily_resilience(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailyResilienceDocument>>> {
        Box::pin(async move {
            self.load_paged(
                "daily_resilience.json",
                "daily_resilience",
                "daily",
                "day",
                &start_date,
                &end_date,
            )
        })
    }

    fn fetch_daily_cardiovascular_age(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<DailyCardiovascularAgeDocument>>> {
        Box::pin(async move {
            self.load_paged(
                "daily_cardiovascular_age.json",
                "daily_cardiovascular_age",
                "daily",
                "day",
                &start_date,
                &end_date,
            )
        })
    }

    fn fetch_vo2_max(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<Vo2MaxDocument>>> {
        Box::pin(async move {
            self.load_paged(
                "vo2_max.json",
                "vO2_max",
                "daily",
                "day",
                &start_date,
                &end_date,
            )
        })
    }

    fn fetch_heartrate(
        &self,
        start_datetime: String,
        end_datetime: String,
    ) -> ClientFuture<'_, Vec<PageFetch<HeartRateDocument>>> {
        Box::pin(async move {
            self.load_timeseries(
                "heartrate.json",
                "heartrate",
                "heartrate",
                "timestamp",
                &start_datetime,
                &end_datetime,
            )
        })
    }

    fn fetch_workouts(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<WorkoutDocument>>> {
        Box::pin(async move {
            self.load_paged(
                "workouts.json",
                "workout",
                "workout",
                "day",
                &start_date,
                &end_date,
            )
        })
    }

    fn fetch_enhanced_tags(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<EnhancedTagDocument>>> {
        Box::pin(async move {
            self.load_paged(
                "enhanced_tags.json",
                "enhanced_tag",
                "enhanced_tag",
                "day",
                &start_date,
                &end_date,
            )
        })
    }

    fn fetch_sessions(
        &self,
        start_date: String,
        end_date: String,
    ) -> ClientFuture<'_, Vec<PageFetch<SessionDocument>>> {
        Box::pin(async move {
            self.load_paged(
                "sessions.json",
                "session",
                "session",
                "day",
                &start_date,
                &end_date,
            )
        })
    }
}

trait SerializeableDocument: serde::Serialize {
    fn field_value(&self, field_name: &str) -> Option<&str>;
}

impl SerializeableDocument for DailySleepDocument {
    fn field_value(&self, field_name: &str) -> Option<&str> {
        match field_name {
            "day" => Some(self.day.as_str()),
            "timestamp" => Some(self.timestamp.as_str()),
            _ => None,
        }
    }
}

impl SerializeableDocument for DailyReadinessDocument {
    fn field_value(&self, field_name: &str) -> Option<&str> {
        match field_name {
            "day" => Some(self.day.as_str()),
            "timestamp" => Some(self.timestamp.as_str()),
            _ => None,
        }
    }
}

impl SerializeableDocument for DailyActivityDocument {
    fn field_value(&self, field_name: &str) -> Option<&str> {
        match field_name {
            "day" => Some(self.day.as_str()),
            "timestamp" => Some(self.timestamp.as_str()),
            _ => None,
        }
    }
}

impl SerializeableDocument for SleepTimeDocument {
    fn field_value(&self, field_name: &str) -> Option<&str> {
        match field_name {
            "day" => Some(self.day.as_str()),
            _ => None,
        }
    }
}

impl SerializeableDocument for DailyStressDocument {
    fn field_value(&self, field_name: &str) -> Option<&str> {
        match field_name {
            "day" => Some(self.day.as_str()),
            _ => None,
        }
    }
}

impl SerializeableDocument for DailyResilienceDocument {
    fn field_value(&self, field_name: &str) -> Option<&str> {
        match field_name {
            "day" => Some(self.day.as_str()),
            _ => None,
        }
    }
}

impl SerializeableDocument for DailyCardiovascularAgeDocument {
    fn field_value(&self, field_name: &str) -> Option<&str> {
        match field_name {
            "day" => Some(self.day.as_str()),
            _ => None,
        }
    }
}

impl SerializeableDocument for Vo2MaxDocument {
    fn field_value(&self, field_name: &str) -> Option<&str> {
        match field_name {
            "day" => Some(self.day.as_str()),
            "timestamp" => Some(self.timestamp.as_str()),
            _ => None,
        }
    }
}

impl SerializeableDocument for RestModePeriodDocument {
    fn field_value(&self, field_name: &str) -> Option<&str> {
        match field_name {
            "day" => Some(self.start_day.as_str()),
            "timestamp" => self.start_time.as_deref(),
            _ => None,
        }
    }
}

impl SerializeableDocument for HeartRateDocument {
    fn field_value(&self, field_name: &str) -> Option<&str> {
        match field_name {
            "timestamp" => Some(self.timestamp.as_str()),
            _ => None,
        }
    }
}

impl SerializeableDocument for WorkoutDocument {
    fn field_value(&self, field_name: &str) -> Option<&str> {
        match field_name {
            "day" => self.day.as_deref(),
            "timestamp" => self.start_datetime.as_deref(),
            _ => None,
        }
    }
}

impl SerializeableDocument for EnhancedTagDocument {
    fn field_value(&self, field_name: &str) -> Option<&str> {
        match field_name {
            "day" => Some(self.day.as_str()),
            "timestamp" => self.start_time.as_deref(),
            _ => None,
        }
    }
}

impl SerializeableDocument for SessionDocument {
    fn field_value(&self, field_name: &str) -> Option<&str> {
        match field_name {
            "day" => Some(self.day.as_str()),
            "timestamp" => self.start_datetime.as_deref(),
            _ => None,
        }
    }
}

fn available_fixture_scopes(fixture_dir: &Path) -> Vec<String> {
    let mut scopes = Vec::new();
    if fixture_dir.join("personal_info.json").is_file() {
        scopes.push(CapabilityKind::Personal.scope_name().to_owned());
    }
    let daily_files = [
        fixture_dir.join("daily_sleep.json"),
        fixture_dir.join("daily_readiness.json"),
        fixture_dir.join("daily_activity.json"),
        fixture_dir.join("sleep_time.json"),
        fixture_dir.join("rest_mode_periods.json"),
        fixture_dir.join("daily_stress.json"),
        fixture_dir.join("daily_resilience.json"),
        fixture_dir.join("daily_cardiovascular_age.json"),
        fixture_dir.join("vo2_max.json"),
    ];
    if daily_files.iter().any(|path| path.is_file()) {
        scopes.push(CapabilityKind::Daily.scope_name().to_owned());
    }
    let stress_files = [
        fixture_dir.join("sleep_time.json"),
        fixture_dir.join("rest_mode_periods.json"),
        fixture_dir.join("daily_stress.json"),
    ];
    if stress_files.iter().any(|path| path.is_file()) {
        scopes.push(CapabilityKind::Stress.scope_name().to_owned());
    }
    let heart_health_files = [
        fixture_dir.join("daily_resilience.json"),
        fixture_dir.join("daily_cardiovascular_age.json"),
        fixture_dir.join("vo2_max.json"),
    ];
    if heart_health_files.iter().any(|path| path.is_file()) {
        scopes.push(CapabilityKind::HeartHealth.scope_name().to_owned());
    }
    if fixture_dir.join("heartrate.json").is_file() {
        scopes.push(CapabilityKind::Heartrate.scope_name().to_owned());
    }
    if fixture_dir.join("workouts.json").is_file() {
        scopes.push(CapabilityKind::Workout.scope_name().to_owned());
    }
    if fixture_dir.join("enhanced_tags.json").is_file() {
        scopes.push(CapabilityKind::EnhancedTag.scope_name().to_owned());
    }
    if fixture_dir.join("sessions.json").is_file() {
        scopes.push(CapabilityKind::Session.scope_name().to_owned());
    }

    scopes
}

fn parse_api_problem(status: StatusCode, payload: &str) -> OuraProblem {
    let json = serde_json::from_str::<Value>(payload).ok();
    let title = json
        .as_ref()
        .and_then(|value| value.get("title"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("request failed with HTTP {}", status.as_u16()));
    let detail = json
        .as_ref()
        .and_then(|value| value.get("detail"))
        .map(detail_string)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            if payload.trim().is_empty() {
                None
            } else {
                Some(payload.trim().to_owned())
            }
        });

    OuraProblem::new(Some(status.as_u16()), title, detail)
}

fn detail_string(value: &Value) -> String {
    match value {
        Value::String(string) => string.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| "<unprintable detail>".to_owned()),
    }
}

fn cache_key(endpoint: &str, params: &[(&str, String)]) -> String {
    let suffix = params
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    format!("{endpoint}|{suffix}")
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| RingmasterError::Config(format!("formatting timestamp failed: {error}")))
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    #[test]
    fn fixture_scope_detection_includes_stress_and_heart_health_families() {
        let tempdir = tempdir().unwrap_or_else(|error| panic!("tempdir should succeed: {error}"));
        fs::write(tempdir.path().join("daily_sleep.json"), "[]")
            .unwrap_or_else(|error| panic!("daily fixture should write: {error}"));
        fs::write(tempdir.path().join("sleep_time.json"), "[]")
            .unwrap_or_else(|error| panic!("stress fixture should write: {error}"));
        fs::write(tempdir.path().join("daily_resilience.json"), "[]")
            .unwrap_or_else(|error| panic!("heart health fixture should write: {error}"));

        let scopes = super::available_fixture_scopes(tempdir.path());

        assert!(scopes.contains(&"daily".to_owned()));
        assert!(scopes.contains(&"stress".to_owned()));
        assert!(scopes.contains(&"heart_health".to_owned()));
    }
}
