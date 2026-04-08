use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::config::Config;
use crate::error::Result;
use crate::oura::auth;
use crate::oura::client::{OuraClient, ReqwestOuraClient};
use crate::oura::models::CapabilityReport;
use crate::store::Store;
use crate::store::queries::{SyncRunStatus, SyncStateRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncReport {
    pub status: SyncRunStatus,
    pub started_at: String,
    pub finished_at: String,
    pub database_path: String,
    pub notes: Vec<String>,
    pub capability_report: CapabilityReport,
}

pub async fn sync_once(config: &Config, store: &Store) -> Result<SyncReport> {
    let started_at = now_rfc3339()?;
    let auth_status = auth::inspect_auth(config);
    let capability_report = auth_status.capability_report.clone();

    let (status, notes) = if !auth_status.configured {
        (
            SyncRunStatus::Blocked,
            vec![
                "Oura OAuth client credentials are not configured yet.".to_owned(),
                "Run `ringmaster auth login` after adding client credentials to continue."
                    .to_owned(),
            ],
        )
    } else if auth_status.granted_scopes.is_empty() {
        (
            SyncRunStatus::Blocked,
            vec![
                "No granted scopes are available yet, so sync cannot pull user data.".to_owned(),
                "Finish the OAuth loopback flow once token persistence lands, or set granted scopes for development."
                    .to_owned(),
            ],
        )
    } else {
        let client = ReqwestOuraClient::new(config)?;
        let missing_scopes = client.capability_report().missing_scope_names();
        let mut notes = vec![
            "Poll-first sync orchestration is wired through the typed Oura client interface."
                .to_owned(),
            "Endpoint fetches remain scaffolded for the next milestone, so this run records readiness instead of importing data."
                .to_owned(),
        ];

        if !missing_scopes.is_empty() {
            notes.push(format!(
                "Granted scopes are partial; unavailable capability scopes: {}.",
                missing_scopes.join(", ")
            ));
        }

        (SyncRunStatus::Partial, notes)
    };

    let finished_at = now_rfc3339()?;
    let record = SyncStateRecord {
        sync_key: "oura_poll".to_owned(),
        status: status.clone(),
        cursor: None,
        last_attempted_at: started_at.clone(),
        last_completed_at: Some(finished_at.clone()),
        message: notes.first().cloned(),
        granted_scopes: auth_status.granted_scopes.clone(),
    };
    store.sync_state().upsert(&record)?;

    Ok(SyncReport {
        status,
        started_at,
        finished_at,
        database_path: store.plan().db_path.display().to_string(),
        notes,
        capability_report,
    })
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc().format(&Rfc3339).map_err(|error| {
        crate::error::RingmasterError::Config(format!("formatting timestamp failed: {error}"))
    })
}
