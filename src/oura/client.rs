use std::future::Future;
use std::pin::Pin;

use reqwest::Client as HttpClient;

use crate::config::Config;
use crate::error::{Result, RingmasterError};
use crate::oura::models::{
    CapabilityReport, DailySummary, HeartRateSample, SessionRecord, TagRecord, WorkoutRecord,
};

type ClientFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

pub trait OuraClient {
    fn capability_report(&self) -> CapabilityReport;

    fn fetch_daily_summaries(&self) -> ClientFuture<'_, Vec<DailySummary>>;
    fn fetch_heartrate(&self) -> ClientFuture<'_, Vec<HeartRateSample>>;
    fn fetch_workouts(&self) -> ClientFuture<'_, Vec<WorkoutRecord>>;
    fn fetch_tags(&self) -> ClientFuture<'_, Vec<TagRecord>>;
    fn fetch_sessions(&self) -> ClientFuture<'_, Vec<SessionRecord>>;
}

#[derive(Debug, Clone)]
pub struct ReqwestOuraClient {
    http: HttpClient,
    capability_report: CapabilityReport,
}

impl ReqwestOuraClient {
    pub fn new(config: &Config) -> Result<Self> {
        let http = HttpClient::builder()
            .user_agent("ringmaster.rs/bootstrap")
            .build()?;
        let capability_report = CapabilityReport::from_scopes(
            &config.oura.requested_scopes,
            &config.oura.granted_scopes,
        );

        Ok(Self {
            http,
            capability_report,
        })
    }

    pub fn http_client(&self) -> &HttpClient {
        &self.http
    }
}

impl OuraClient for ReqwestOuraClient {
    fn capability_report(&self) -> CapabilityReport {
        self.capability_report.clone()
    }

    fn fetch_daily_summaries(&self) -> ClientFuture<'_, Vec<DailySummary>> {
        Box::pin(async {
            Err(RingmasterError::Auth(
                "daily summary fetch is scaffolded but not implemented yet".to_owned(),
            ))
        })
    }

    fn fetch_heartrate(&self) -> ClientFuture<'_, Vec<HeartRateSample>> {
        Box::pin(async {
            Err(RingmasterError::Auth(
                "heartrate fetch is scaffolded but not implemented yet".to_owned(),
            ))
        })
    }

    fn fetch_workouts(&self) -> ClientFuture<'_, Vec<WorkoutRecord>> {
        Box::pin(async {
            Err(RingmasterError::Auth(
                "workout fetch is scaffolded but not implemented yet".to_owned(),
            ))
        })
    }

    fn fetch_tags(&self) -> ClientFuture<'_, Vec<TagRecord>> {
        Box::pin(async {
            Err(RingmasterError::Auth(
                "tag fetch is scaffolded but not implemented yet".to_owned(),
            ))
        })
    }

    fn fetch_sessions(&self) -> ClientFuture<'_, Vec<SessionRecord>> {
        Box::pin(async {
            Err(RingmasterError::Auth(
                "session fetch is scaffolded but not implemented yet".to_owned(),
            ))
        })
    }
}
