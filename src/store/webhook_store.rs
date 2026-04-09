use rusqlite::{Connection, OptionalExtension, params};
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

use crate::error::{Result, RingmasterError};
use crate::webhook::WebhookEventType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredWebhookSubscriptionRecord {
    pub data_type: String,
    pub event_type: WebhookEventType,
    pub enabled: bool,
    pub callback_url: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWebhookSubscriptionRecord {
    pub subscription_id: String,
    pub callback_url: String,
    pub event_type: WebhookEventType,
    pub data_type: String,
    pub expiration_time: String,
    pub drift_status: String,
    pub last_seen_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedWebhookDeliveryRecord {
    pub delivery_id: i64,
    pub delivery_fingerprint: String,
    pub received_at: String,
    pub signature_timestamp: Option<String>,
    pub data_type: Option<String>,
    pub event_type: Option<WebhookEventType>,
    pub object_id: Option<String>,
    pub payload_json: String,
    pub headers_json: String,
    pub query_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedWebhookDeliveryInput {
    pub delivery_fingerprint: String,
    pub received_at: String,
    pub signature_timestamp: Option<String>,
    pub data_type: Option<String>,
    pub event_type: Option<WebhookEventType>,
    pub object_id: Option<String>,
    pub payload_json: String,
    pub headers_json: String,
    pub query_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptedWebhookDeliveryResult {
    Inserted(AcceptedWebhookDeliveryRecord),
    Duplicate(AcceptedWebhookDeliveryRecord),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedWebhookDeliveryRecord {
    pub rejection_id: i64,
    pub received_at: String,
    pub reason_code: String,
    pub detail: String,
    pub signature_timestamp: Option<String>,
    pub payload_json: String,
    pub headers_json: String,
    pub query_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedWebhookDeliveryInput {
    pub received_at: String,
    pub reason_code: String,
    pub detail: String,
    pub signature_timestamp: Option<String>,
    pub payload_json: String,
    pub headers_json: String,
    pub query_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidationRecord {
    pub invalidation_id: i64,
    pub queue_key: String,
    pub data_type: String,
    pub event_type: WebhookEventType,
    pub object_id: Option<String>,
    pub delivery_id: i64,
    pub first_queued_at: String,
    pub last_queued_at: String,
    pub available_at: String,
    pub leased_at: Option<String>,
    pub lease_owner: Option<String>,
    pub attempt_count: u32,
    pub last_error: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidationInput {
    pub queue_key: String,
    pub data_type: String,
    pub event_type: WebhookEventType,
    pub object_id: Option<String>,
    pub delivery_id: i64,
    pub queued_at: String,
    pub available_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessingAttemptRecord {
    pub attempt_id: i64,
    pub invalidation_id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub outcome: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHeartbeatRecord {
    pub component: String,
    pub mode: String,
    pub bind_address: Option<String>,
    pub public_base_url: Option<String>,
    pub detail: Option<String>,
    pub last_seen_at: String,
}

pub struct WebhookStore<'connection> {
    connection: &'connection Connection,
}

const SORTABLE_UTC_TIMESTAMP: &[time::format_description::FormatItem<'_>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:9]Z");

impl<'connection> WebhookStore<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn replace_desired_subscriptions(
        &self,
        records: &[DesiredWebhookSubscriptionRecord],
    ) -> Result<()> {
        self.connection
            .execute_batch("BEGIN IMMEDIATE TRANSACTION")?;

        let result = (|| -> Result<()> {
            self.connection
                .execute("DELETE FROM webhook_desired_subscriptions", [])?;
            let mut statement = self.connection.prepare(
                "INSERT INTO webhook_desired_subscriptions (
                    data_type,
                    event_type,
                    enabled,
                    callback_url,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;

            for record in records {
                statement.execute(params![
                    record.data_type,
                    record.event_type.as_str(),
                    i64::from(record.enabled),
                    record.callback_url,
                    record.updated_at,
                ])?;
            }

            Ok(())
        })();

        match result {
            Ok(()) => {
                self.connection.execute_batch("COMMIT")?;
                Ok(())
            }
            Err(error) => {
                let _ = self.connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    pub fn list_desired_subscriptions(&self) -> Result<Vec<DesiredWebhookSubscriptionRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT data_type, event_type, enabled, callback_url, updated_at
             FROM webhook_desired_subscriptions
             ORDER BY data_type ASC, event_type ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(DesiredWebhookSubscriptionRecord {
                data_type: row.get(0)?,
                event_type: parse_event_type(row.get::<_, String>(1)?)?,
                enabled: row.get::<_, i64>(2)? != 0,
                callback_url: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn replace_remote_subscriptions(
        &self,
        records: &[RemoteWebhookSubscriptionRecord],
    ) -> Result<()> {
        self.connection
            .execute_batch("BEGIN IMMEDIATE TRANSACTION")?;

        let result = (|| -> Result<()> {
            self.connection
                .execute("DELETE FROM webhook_remote_subscriptions", [])?;
            let mut statement = self.connection.prepare(
                "INSERT INTO webhook_remote_subscriptions (
                    subscription_id,
                    callback_url,
                    event_type,
                    data_type,
                    expiration_time,
                    drift_status,
                    last_seen_at,
                    created_at,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;

            for record in records {
                statement.execute(params![
                    record.subscription_id,
                    record.callback_url,
                    record.event_type.as_str(),
                    record.data_type,
                    record.expiration_time,
                    record.drift_status,
                    record.last_seen_at,
                    record.created_at,
                    record.updated_at,
                ])?;
            }

            Ok(())
        })();

        match result {
            Ok(()) => {
                self.connection.execute_batch("COMMIT")?;
                Ok(())
            }
            Err(error) => {
                let _ = self.connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    pub fn list_remote_subscriptions(&self) -> Result<Vec<RemoteWebhookSubscriptionRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                subscription_id,
                callback_url,
                event_type,
                data_type,
                expiration_time,
                drift_status,
                last_seen_at,
                created_at,
                updated_at
             FROM webhook_remote_subscriptions
             ORDER BY data_type ASC, event_type ASC, subscription_id ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(RemoteWebhookSubscriptionRecord {
                subscription_id: row.get(0)?,
                callback_url: row.get(1)?,
                event_type: parse_event_type(row.get::<_, String>(2)?)?,
                data_type: row.get(3)?,
                expiration_time: row.get(4)?,
                drift_status: row.get(5)?,
                last_seen_at: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn insert_accepted_delivery(
        &self,
        input: &AcceptedWebhookDeliveryInput,
    ) -> Result<AcceptedWebhookDeliveryResult> {
        self.connection.execute(
            "INSERT INTO webhook_deliveries (
                delivery_fingerprint,
                received_at,
                signature_timestamp,
                data_type,
                event_type,
                object_id,
                payload_json,
                headers_json,
                query_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(delivery_fingerprint) DO NOTHING",
            params![
                input.delivery_fingerprint,
                input.received_at,
                input.signature_timestamp,
                input.data_type,
                input.event_type.map(WebhookEventType::as_str),
                input.object_id,
                input.payload_json,
                input.headers_json,
                input.query_json,
            ],
        )?;
        let inserted = self.connection.changes() > 0;

        let record = self
            .get_delivery_by_fingerprint(&input.delivery_fingerprint)?
            .ok_or_else(|| {
                RingmasterError::Config(
                    "accepted webhook delivery was not available after insert".to_owned(),
                )
            })?;

        if inserted {
            Ok(AcceptedWebhookDeliveryResult::Inserted(record))
        } else {
            Ok(AcceptedWebhookDeliveryResult::Duplicate(record))
        }
    }

    pub fn get_delivery(&self, delivery_id: i64) -> Result<Option<AcceptedWebhookDeliveryRecord>> {
        self.connection
            .query_row(
                "SELECT
                    delivery_id,
                    delivery_fingerprint,
                    received_at,
                    signature_timestamp,
                    data_type,
                    event_type,
                    object_id,
                    payload_json,
                    headers_json,
                    query_json
                 FROM webhook_deliveries
                 WHERE delivery_id = ?1",
                params![delivery_id],
                read_delivery_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_recent_deliveries(
        &self,
        limit: usize,
    ) -> Result<Vec<AcceptedWebhookDeliveryRecord>> {
        let bounded_limit = usize::min(limit, 1000);
        let mut statement = self.connection.prepare(
            "SELECT
                delivery_id,
                delivery_fingerprint,
                received_at,
                signature_timestamp,
                data_type,
                event_type,
                object_id,
                payload_json,
                headers_json,
                query_json
             FROM webhook_deliveries
             ORDER BY delivery_id DESC
             LIMIT ?1",
        )?;
        let rows = statement.query_map(params![bounded_limit as i64], read_delivery_row)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn insert_rejected_delivery(
        &self,
        input: &RejectedWebhookDeliveryInput,
    ) -> Result<RejectedWebhookDeliveryRecord> {
        self.connection.execute(
            "INSERT INTO webhook_delivery_rejections (
                received_at,
                reason_code,
                detail,
                signature_timestamp,
                payload_json,
                headers_json,
                query_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                input.received_at,
                input.reason_code,
                input.detail,
                input.signature_timestamp,
                input.payload_json,
                input.headers_json,
                input.query_json,
            ],
        )?;
        let rejection_id = self.connection.last_insert_rowid();

        self.connection
            .query_row(
                "SELECT
                    rejection_id,
                    received_at,
                    reason_code,
                    detail,
                    signature_timestamp,
                    payload_json,
                    headers_json,
                    query_json
                 FROM webhook_delivery_rejections
                 WHERE rejection_id = ?1",
                params![rejection_id],
                |row| {
                    Ok(RejectedWebhookDeliveryRecord {
                        rejection_id: row.get(0)?,
                        received_at: row.get(1)?,
                        reason_code: row.get(2)?,
                        detail: row.get(3)?,
                        signature_timestamp: row.get(4)?,
                        payload_json: row.get(5)?,
                        headers_json: row.get(6)?,
                        query_json: row.get(7)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn latest_rejected_delivery(&self) -> Result<Option<RejectedWebhookDeliveryRecord>> {
        self.connection
            .query_row(
                "SELECT
                    rejection_id,
                    received_at,
                    reason_code,
                    detail,
                    signature_timestamp,
                    payload_json,
                    headers_json,
                    query_json
                 FROM webhook_delivery_rejections
                 ORDER BY rejection_id DESC
                 LIMIT 1",
                [],
                |row| {
                    Ok(RejectedWebhookDeliveryRecord {
                        rejection_id: row.get(0)?,
                        received_at: row.get(1)?,
                        reason_code: row.get(2)?,
                        detail: row.get(3)?,
                        signature_timestamp: row.get(4)?,
                        payload_json: row.get(5)?,
                        headers_json: row.get(6)?,
                        query_json: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn enqueue_invalidation(&self, input: &InvalidationInput) -> Result<InvalidationRecord> {
        self.connection.execute(
            "INSERT INTO webhook_invalidations (
                queue_key,
                data_type,
                event_type,
                object_id,
                delivery_id,
                first_queued_at,
                last_queued_at,
                available_at,
                leased_at,
                lease_owner,
                attempt_count,
                last_error,
                completed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7, NULL, NULL, 0, NULL, NULL)
            ON CONFLICT(queue_key) DO UPDATE SET
                delivery_id = excluded.delivery_id,
                first_queued_at = CASE
                    WHEN webhook_invalidations.completed_at IS NULL
                    THEN webhook_invalidations.first_queued_at
                    ELSE excluded.first_queued_at
                END,
                last_queued_at = excluded.last_queued_at,
                available_at = CASE
                    WHEN webhook_invalidations.completed_at IS NULL
                        AND webhook_invalidations.lease_owner IS NOT NULL
                    THEN webhook_invalidations.available_at
                    ELSE excluded.available_at
                END,
                leased_at = CASE
                    WHEN webhook_invalidations.completed_at IS NULL
                    THEN webhook_invalidations.leased_at
                    ELSE NULL
                END,
                lease_owner = CASE
                    WHEN webhook_invalidations.completed_at IS NULL
                    THEN webhook_invalidations.lease_owner
                    ELSE NULL
                END,
                attempt_count = CASE
                    WHEN webhook_invalidations.completed_at IS NULL
                    THEN webhook_invalidations.attempt_count
                    ELSE 0
                END,
                last_error = NULL,
                completed_at = NULL",
            params![
                input.queue_key,
                input.data_type,
                input.event_type.as_str(),
                input.object_id,
                input.delivery_id,
                input.queued_at,
                input.available_at,
            ],
        )?;

        self.connection
            .query_row(
                "SELECT
                    invalidation_id,
                    queue_key,
                    data_type,
                    event_type,
                    object_id,
                    delivery_id,
                    first_queued_at,
                    last_queued_at,
                    available_at,
                    leased_at,
                    lease_owner,
                    attempt_count,
                    last_error,
                    completed_at
                 FROM webhook_invalidations
                 WHERE queue_key = ?1",
                params![input.queue_key],
                read_invalidation_row,
            )
            .map_err(Into::into)
    }

    pub fn list_pending_invalidations(&self) -> Result<Vec<InvalidationRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT
                invalidation_id,
                queue_key,
                data_type,
                event_type,
                object_id,
                delivery_id,
                first_queued_at,
                last_queued_at,
                available_at,
                leased_at,
                lease_owner,
                attempt_count,
                last_error,
                completed_at
             FROM webhook_invalidations
             WHERE completed_at IS NULL
             ORDER BY julianday(available_at) ASC, invalidation_id ASC",
        )?;
        let rows = statement.query_map([], read_invalidation_row)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    #[cfg(test)]
    pub fn overwrite_invalidation_available_at(
        &self,
        invalidation_id: i64,
        available_at: &str,
    ) -> Result<()> {
        self.connection.execute(
            "UPDATE webhook_invalidations
             SET available_at = ?1
             WHERE invalidation_id = ?2",
            params![available_at, invalidation_id],
        )?;
        Ok(())
    }

    pub fn claim_available_invalidations(
        &self,
        lease_owner: &str,
        claimed_at: &str,
        lease_until: &str,
        limit: usize,
    ) -> Result<Vec<InvalidationRecord>> {
        self.connection
            .execute_batch("BEGIN IMMEDIATE TRANSACTION")?;

        let result = (|| -> Result<Vec<InvalidationRecord>> {
            let bounded_limit = usize::min(limit, 512);
            let mut statement = self.connection.prepare(
                "SELECT
                    invalidation_id,
                    queue_key,
                    data_type,
                    event_type,
                    object_id,
                    delivery_id,
                    first_queued_at,
                    last_queued_at,
                    available_at,
                    leased_at,
                    lease_owner,
                    attempt_count,
                    last_error,
                    completed_at
                 FROM webhook_invalidations
                 WHERE julianday(available_at) <= julianday(?1)
                   AND completed_at IS NULL
                 ORDER BY julianday(available_at) ASC, invalidation_id ASC
                 LIMIT ?2",
            )?;
            let rows = statement.query_map(
                params![claimed_at, bounded_limit as i64],
                read_invalidation_row,
            )?;
            let mut records = Vec::new();
            for row in rows {
                records.push(row?);
            }

            for record in &records {
                self.connection.execute(
                    "UPDATE webhook_invalidations
                     SET leased_at = ?1,
                         lease_owner = ?2,
                         available_at = ?3
                     WHERE invalidation_id = ?4",
                    params![claimed_at, lease_owner, lease_until, record.invalidation_id],
                )?;
            }

            Ok(records
                .into_iter()
                .map(|mut record| {
                    record.leased_at = Some(claimed_at.to_owned());
                    record.lease_owner = Some(lease_owner.to_owned());
                    lease_until.clone_into(&mut record.available_at);
                    record
                })
                .collect())
        })();

        match result {
            Ok(records) => {
                self.connection.execute_batch("COMMIT")?;
                Ok(records)
            }
            Err(error) => {
                let _ = self.connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    pub fn start_processing_attempt(
        &self,
        invalidation_id: i64,
        started_at: &str,
    ) -> Result<ProcessingAttemptRecord> {
        self.connection.execute(
            "INSERT INTO webhook_processing_attempts (
                invalidation_id,
                started_at,
                finished_at,
                outcome,
                detail
            ) VALUES (?1, ?2, NULL, 'running', NULL)",
            params![invalidation_id, started_at],
        )?;
        let attempt_id = self.connection.last_insert_rowid();

        self.connection
            .query_row(
                "SELECT attempt_id, invalidation_id, started_at, finished_at, outcome, detail
                 FROM webhook_processing_attempts
                 WHERE attempt_id = ?1",
                params![attempt_id],
                |row| {
                    Ok(ProcessingAttemptRecord {
                        attempt_id: row.get(0)?,
                        invalidation_id: row.get(1)?,
                        started_at: row.get(2)?,
                        finished_at: row.get(3)?,
                        outcome: row.get(4)?,
                        detail: row.get(5)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn complete_processing_attempt_success(
        &self,
        invalidation_id: i64,
        attempt_id: i64,
        finished_at: &str,
        detail: Option<&str>,
    ) -> Result<()> {
        self.connection
            .execute_batch("BEGIN IMMEDIATE TRANSACTION")?;

        let result = (|| -> Result<()> {
            let attempt_started_at: String = self.connection.query_row(
                "SELECT started_at
                 FROM webhook_processing_attempts
                 WHERE attempt_id = ?1",
                params![attempt_id],
                |row| row.get(0),
            )?;
            self.connection.execute(
                "UPDATE webhook_processing_attempts
                 SET finished_at = ?1,
                     outcome = 'success',
                     detail = ?2
                 WHERE attempt_id = ?3",
                params![finished_at, detail, attempt_id],
            )?;
            self.connection.execute(
                "UPDATE webhook_invalidations
                 SET completed_at = CASE
                        WHEN julianday(last_queued_at) <= julianday(?3)
                        THEN ?1
                        ELSE NULL
                     END,
                     available_at = CASE
                        WHEN julianday(last_queued_at) <= julianday(?3)
                        THEN available_at
                        ELSE last_queued_at
                     END,
                     leased_at = NULL,
                     lease_owner = NULL,
                     last_error = NULL
                 WHERE invalidation_id = ?2",
                params![finished_at, invalidation_id, attempt_started_at],
            )?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                self.connection.execute_batch("COMMIT")?;
                Ok(())
            }
            Err(error) => {
                let _ = self.connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    pub fn complete_processing_attempt_failure(
        &self,
        invalidation_id: i64,
        attempt_id: i64,
        finished_at: &str,
        next_available_at: &str,
        detail: &str,
    ) -> Result<InvalidationRecord> {
        self.connection
            .execute_batch("BEGIN IMMEDIATE TRANSACTION")?;

        let result = (|| -> Result<InvalidationRecord> {
            self.connection.execute(
                "UPDATE webhook_processing_attempts
                 SET finished_at = ?1,
                     outcome = 'failed',
                     detail = ?2
                 WHERE attempt_id = ?3",
                params![finished_at, detail, attempt_id],
            )?;
            self.connection.execute(
                "UPDATE webhook_invalidations
                 SET attempt_count = attempt_count + 1,
                     available_at = ?1,
                     leased_at = NULL,
                     lease_owner = NULL,
                     last_error = ?2,
                     completed_at = NULL
                 WHERE invalidation_id = ?3",
                params![next_available_at, detail, invalidation_id],
            )?;
            self.connection
                .query_row(
                    "SELECT
                        invalidation_id,
                        queue_key,
                        data_type,
                        event_type,
                        object_id,
                        delivery_id,
                        first_queued_at,
                        last_queued_at,
                        available_at,
                        leased_at,
                        lease_owner,
                        attempt_count,
                        last_error,
                        completed_at
                     FROM webhook_invalidations
                     WHERE invalidation_id = ?1",
                    params![invalidation_id],
                    read_invalidation_row,
                )
                .map_err(Into::into)
        })();

        match result {
            Ok(record) => {
                self.connection.execute_batch("COMMIT")?;
                Ok(record)
            }
            Err(error) => {
                let _ = self.connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    pub fn list_recent_processing_attempts(
        &self,
        limit: usize,
    ) -> Result<Vec<ProcessingAttemptRecord>> {
        let bounded_limit = usize::min(limit, 512);
        let mut statement = self.connection.prepare(
            "SELECT attempt_id, invalidation_id, started_at, finished_at, outcome, detail
             FROM webhook_processing_attempts
             ORDER BY attempt_id DESC
             LIMIT ?1",
        )?;
        let rows = statement.query_map(params![bounded_limit as i64], |row| {
            Ok(ProcessingAttemptRecord {
                attempt_id: row.get(0)?,
                invalidation_id: row.get(1)?,
                started_at: row.get(2)?,
                finished_at: row.get(3)?,
                outcome: row.get(4)?,
                detail: row.get(5)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    pub fn upsert_runtime_heartbeat(&self, record: &RuntimeHeartbeatRecord) -> Result<()> {
        self.connection.execute(
            "INSERT INTO webhook_runtime_heartbeats (
                component,
                mode,
                bind_address,
                public_base_url,
                detail,
                last_seen_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(component) DO UPDATE SET
                mode = excluded.mode,
                bind_address = excluded.bind_address,
                public_base_url = excluded.public_base_url,
                detail = excluded.detail,
                last_seen_at = excluded.last_seen_at",
            params![
                record.component,
                record.mode,
                record.bind_address,
                record.public_base_url,
                record.detail,
                record.last_seen_at,
            ],
        )?;

        Ok(())
    }

    pub fn list_runtime_heartbeats(&self) -> Result<Vec<RuntimeHeartbeatRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT component, mode, bind_address, public_base_url, detail, last_seen_at
             FROM webhook_runtime_heartbeats
             ORDER BY component ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(RuntimeHeartbeatRecord {
                component: row.get(0)?,
                mode: row.get(1)?,
                bind_address: row.get(2)?,
                public_base_url: row.get(3)?,
                detail: row.get(4)?,
                last_seen_at: row.get(5)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    fn get_delivery_by_fingerprint(
        &self,
        delivery_fingerprint: &str,
    ) -> Result<Option<AcceptedWebhookDeliveryRecord>> {
        self.connection
            .query_row(
                "SELECT
                    delivery_id,
                    delivery_fingerprint,
                    received_at,
                    signature_timestamp,
                    data_type,
                    event_type,
                    object_id,
                    payload_json,
                    headers_json,
                    query_json
                 FROM webhook_deliveries
                 WHERE delivery_fingerprint = ?1",
                params![delivery_fingerprint],
                read_delivery_row,
            )
            .optional()
            .map_err(Into::into)
    }
}

fn parse_event_type(value: String) -> rusqlite::Result<WebhookEventType> {
    WebhookEventType::parse(&value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            value.len(),
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::other(format!(
                "unknown webhook event type `{value}`"
            ))),
        )
    })
}

fn read_delivery_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AcceptedWebhookDeliveryRecord> {
    Ok(AcceptedWebhookDeliveryRecord {
        delivery_id: row.get(0)?,
        delivery_fingerprint: row.get(1)?,
        received_at: row.get(2)?,
        signature_timestamp: row.get(3)?,
        data_type: row.get(4)?,
        event_type: row
            .get::<_, Option<String>>(5)?
            .map(parse_event_type)
            .transpose()?,
        object_id: row.get(6)?,
        payload_json: row.get(7)?,
        headers_json: row.get(8)?,
        query_json: row.get(9)?,
    })
}

fn read_invalidation_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<InvalidationRecord> {
    let attempt_count = row.get::<_, i64>(11)?;
    Ok(InvalidationRecord {
        invalidation_id: row.get(0)?,
        queue_key: row.get(1)?,
        data_type: row.get(2)?,
        event_type: parse_event_type(row.get::<_, String>(3)?)?,
        object_id: row.get(4)?,
        delivery_id: row.get(5)?,
        first_queued_at: row.get(6)?,
        last_queued_at: row.get(7)?,
        available_at: row.get(8)?,
        leased_at: row.get(9)?,
        lease_owner: row.get(10)?,
        attempt_count: u32::try_from(attempt_count).unwrap_or_default(),
        last_error: row.get(12)?,
        completed_at: row.get(13)?,
    })
}

pub fn now_rfc3339() -> Result<String> {
    format_rfc3339_utc(OffsetDateTime::now_utc())
}

pub fn format_rfc3339_utc(timestamp: OffsetDateTime) -> Result<String> {
    timestamp
        .to_offset(UtcOffset::UTC)
        .format(SORTABLE_UTC_TIMESTAMP)
        .map_err(|error| RingmasterError::Config(format!("formatting timestamp failed: {error}")))
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::{
        AcceptedWebhookDeliveryInput, AcceptedWebhookDeliveryResult,
        DesiredWebhookSubscriptionRecord, InvalidationInput, RuntimeHeartbeatRecord,
        format_rfc3339_utc, now_rfc3339,
    };
    use crate::store::db::Store;
    use crate::webhook::WebhookEventType;
    use time::OffsetDateTime;

    #[test]
    fn replaces_desired_subscriptions() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let updated_at = now_rfc3339().unwrap_or_else(|error| panic!("timestamp: {error}"));

        store
            .webhook()
            .replace_desired_subscriptions(&[DesiredWebhookSubscriptionRecord {
                data_type: "daily_sleep".to_owned(),
                event_type: WebhookEventType::Create,
                enabled: true,
                callback_url: Some("https://example.test/webhooks/oura".to_owned()),
                updated_at: updated_at.clone(),
            }])
            .unwrap_or_else(|error| panic!("desired subscriptions should persist: {error}"));

        let records = store
            .webhook()
            .list_desired_subscriptions()
            .unwrap_or_else(|error| panic!("desired subscriptions should load: {error}"));
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].data_type, "daily_sleep");
        assert_eq!(records[0].updated_at, updated_at);
    }

    #[test]
    fn dedupes_accepted_deliveries_by_fingerprint() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let first = store
            .webhook()
            .insert_accepted_delivery(&AcceptedWebhookDeliveryInput {
                delivery_fingerprint: "dup".to_owned(),
                received_at: "2026-04-08T00:00:00Z".to_owned(),
                signature_timestamp: Some("2026-04-08T00:00:00Z".to_owned()),
                data_type: Some("daily_sleep".to_owned()),
                event_type: Some(WebhookEventType::Create),
                object_id: Some("sleep_1".to_owned()),
                payload_json: "{}".to_owned(),
                headers_json: "{}".to_owned(),
                query_json: "{}".to_owned(),
            })
            .unwrap_or_else(|error| panic!("accepted delivery should insert: {error}"));
        let second = store
            .webhook()
            .insert_accepted_delivery(&AcceptedWebhookDeliveryInput {
                delivery_fingerprint: "dup".to_owned(),
                received_at: "2026-04-08T00:01:00Z".to_owned(),
                signature_timestamp: Some("2026-04-08T00:00:00Z".to_owned()),
                data_type: Some("daily_sleep".to_owned()),
                event_type: Some(WebhookEventType::Create),
                object_id: Some("sleep_1".to_owned()),
                payload_json: "{}".to_owned(),
                headers_json: "{}".to_owned(),
                query_json: "{}".to_owned(),
            })
            .unwrap_or_else(|error| panic!("duplicate delivery should resolve: {error}"));

        let first_id = match first {
            AcceptedWebhookDeliveryResult::Inserted(record) => record.delivery_id,
            AcceptedWebhookDeliveryResult::Duplicate(_) => panic!("first insert should be new"),
        };
        match second {
            AcceptedWebhookDeliveryResult::Inserted(_) => {
                panic!("duplicate delivery should not insert a new row");
            }
            AcceptedWebhookDeliveryResult::Duplicate(record) => {
                assert_eq!(record.delivery_id, first_id);
            }
        }
    }

    #[test]
    fn coalesces_invalidations_by_queue_key() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let webhook = store.webhook();
        let delivery_id = match webhook
            .insert_accepted_delivery(&AcceptedWebhookDeliveryInput {
                delivery_fingerprint: "queue-source".to_owned(),
                received_at: "2026-04-08T00:00:00Z".to_owned(),
                signature_timestamp: Some("2026-04-08T00:00:00Z".to_owned()),
                data_type: Some("daily_sleep".to_owned()),
                event_type: Some(WebhookEventType::Create),
                object_id: Some("sleep_1".to_owned()),
                payload_json: "{}".to_owned(),
                headers_json: "{}".to_owned(),
                query_json: "{}".to_owned(),
            })
            .unwrap_or_else(|error| panic!("accepted delivery should insert: {error}"))
        {
            AcceptedWebhookDeliveryResult::Inserted(record)
            | AcceptedWebhookDeliveryResult::Duplicate(record) => record.delivery_id,
        };
        let first = webhook
            .enqueue_invalidation(&InvalidationInput {
                queue_key: "daily_sleep:create:sleep_1".to_owned(),
                data_type: "daily_sleep".to_owned(),
                event_type: WebhookEventType::Create,
                object_id: Some("sleep_1".to_owned()),
                delivery_id,
                queued_at: "2026-04-08T00:00:00Z".to_owned(),
                available_at: "2026-04-08T00:00:00Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("invalidation should insert: {error}"));
        let second = webhook
            .enqueue_invalidation(&InvalidationInput {
                queue_key: "daily_sleep:create:sleep_1".to_owned(),
                data_type: "daily_sleep".to_owned(),
                event_type: WebhookEventType::Create,
                object_id: Some("sleep_1".to_owned()),
                delivery_id,
                queued_at: "2026-04-08T00:01:00Z".to_owned(),
                available_at: "2026-04-08T00:01:00Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("duplicate invalidation should coalesce: {error}"));

        assert_eq!(first.invalidation_id, second.invalidation_id);
        assert_eq!(second.delivery_id, delivery_id);
        let pending = webhook
            .list_pending_invalidations()
            .unwrap_or_else(|error| panic!("pending invalidations should load: {error}"));
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn reactivating_completed_invalidation_resets_retry_state() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let webhook = store.webhook();
        let delivery_id = match webhook
            .insert_accepted_delivery(&AcceptedWebhookDeliveryInput {
                delivery_fingerprint: "queue-reactivate".to_owned(),
                received_at: "2026-04-08T00:00:00Z".to_owned(),
                signature_timestamp: Some("2026-04-08T00:00:00Z".to_owned()),
                data_type: Some("daily_sleep".to_owned()),
                event_type: Some(WebhookEventType::Create),
                object_id: Some("sleep_1".to_owned()),
                payload_json: "{}".to_owned(),
                headers_json: "{}".to_owned(),
                query_json: "{}".to_owned(),
            })
            .unwrap_or_else(|error| panic!("accepted delivery should insert: {error}"))
        {
            AcceptedWebhookDeliveryResult::Inserted(record)
            | AcceptedWebhookDeliveryResult::Duplicate(record) => record.delivery_id,
        };
        let queued = webhook
            .enqueue_invalidation(&InvalidationInput {
                queue_key: "daily_sleep:create:sleep_1".to_owned(),
                data_type: "daily_sleep".to_owned(),
                event_type: WebhookEventType::Create,
                object_id: Some("sleep_1".to_owned()),
                delivery_id,
                queued_at: "2026-04-08T00:00:00Z".to_owned(),
                available_at: "2026-04-08T00:00:00Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("invalidation should insert: {error}"));
        let failed_attempt = webhook
            .start_processing_attempt(queued.invalidation_id, "2026-04-08T00:01:00Z")
            .unwrap_or_else(|error| panic!("attempt should start: {error}"));
        let failed = webhook
            .complete_processing_attempt_failure(
                queued.invalidation_id,
                failed_attempt.attempt_id,
                "2026-04-08T00:02:00Z",
                "2026-04-08T00:03:00Z",
                "temporary failure",
            )
            .unwrap_or_else(|error| panic!("attempt should fail: {error}"));
        let success_attempt = webhook
            .start_processing_attempt(failed.invalidation_id, "2026-04-08T00:04:00Z")
            .unwrap_or_else(|error| panic!("second attempt should start: {error}"));
        webhook
            .complete_processing_attempt_success(
                failed.invalidation_id,
                success_attempt.attempt_id,
                "2026-04-08T00:05:00Z",
                Some("processed"),
            )
            .unwrap_or_else(|error| panic!("attempt should complete successfully: {error}"));

        let reactivated = webhook
            .enqueue_invalidation(&InvalidationInput {
                queue_key: "daily_sleep:create:sleep_1".to_owned(),
                data_type: "daily_sleep".to_owned(),
                event_type: WebhookEventType::Create,
                object_id: Some("sleep_1".to_owned()),
                delivery_id,
                queued_at: "2026-04-08T01:00:00Z".to_owned(),
                available_at: "2026-04-08T01:00:00Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("completed invalidation should reactivate: {error}"));

        assert_eq!(reactivated.attempt_count, 0);
        assert_eq!(reactivated.first_queued_at, "2026-04-08T01:00:00Z");
        assert!(reactivated.completed_at.is_none());
        assert!(reactivated.last_error.is_none());
    }

    #[test]
    fn requeue_during_active_lease_stays_pending_for_follow_up_processing() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let webhook = store.webhook();
        let first_delivery_id = match webhook
            .insert_accepted_delivery(&AcceptedWebhookDeliveryInput {
                delivery_fingerprint: "queue-inflight-first".to_owned(),
                received_at: "2026-04-08T00:00:00Z".to_owned(),
                signature_timestamp: Some("2026-04-08T00:00:00Z".to_owned()),
                data_type: Some("daily_sleep".to_owned()),
                event_type: Some(WebhookEventType::Update),
                object_id: Some("sleep_1".to_owned()),
                payload_json: "{}".to_owned(),
                headers_json: "{}".to_owned(),
                query_json: "{}".to_owned(),
            })
            .unwrap_or_else(|error| panic!("first accepted delivery should insert: {error}"))
        {
            AcceptedWebhookDeliveryResult::Inserted(record)
            | AcceptedWebhookDeliveryResult::Duplicate(record) => record.delivery_id,
        };
        let second_delivery_id = match webhook
            .insert_accepted_delivery(&AcceptedWebhookDeliveryInput {
                delivery_fingerprint: "queue-inflight-second".to_owned(),
                received_at: "2026-04-08T00:01:30Z".to_owned(),
                signature_timestamp: Some("2026-04-08T00:01:30Z".to_owned()),
                data_type: Some("daily_sleep".to_owned()),
                event_type: Some(WebhookEventType::Update),
                object_id: Some("sleep_1".to_owned()),
                payload_json: "{}".to_owned(),
                headers_json: "{}".to_owned(),
                query_json: "{}".to_owned(),
            })
            .unwrap_or_else(|error| panic!("second accepted delivery should insert: {error}"))
        {
            AcceptedWebhookDeliveryResult::Inserted(record)
            | AcceptedWebhookDeliveryResult::Duplicate(record) => record.delivery_id,
        };

        let queued = webhook
            .enqueue_invalidation(&InvalidationInput {
                queue_key: "daily_sleep:update:sleep_1".to_owned(),
                data_type: "daily_sleep".to_owned(),
                event_type: WebhookEventType::Update,
                object_id: Some("sleep_1".to_owned()),
                delivery_id: first_delivery_id,
                queued_at: "2026-04-08T00:00:00Z".to_owned(),
                available_at: "2026-04-08T00:00:00Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("invalidation should insert: {error}"));
        let claimed = webhook
            .claim_available_invalidations(
                "watch-test",
                "2026-04-08T00:01:00Z",
                "2026-04-08T00:06:00Z",
                8,
            )
            .unwrap_or_else(|error| panic!("invalidation should claim: {error}"));
        assert_eq!(claimed.len(), 1);
        let attempt = webhook
            .start_processing_attempt(queued.invalidation_id, "2026-04-08T00:01:05Z")
            .unwrap_or_else(|error| panic!("attempt should start: {error}"));

        let requeued = webhook
            .enqueue_invalidation(&InvalidationInput {
                queue_key: "daily_sleep:update:sleep_1".to_owned(),
                data_type: "daily_sleep".to_owned(),
                event_type: WebhookEventType::Update,
                object_id: Some("sleep_1".to_owned()),
                delivery_id: second_delivery_id,
                queued_at: "2026-04-08T00:01:30Z".to_owned(),
                available_at: "2026-04-08T00:01:30Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("in-flight invalidation should requeue: {error}"));
        assert_eq!(requeued.delivery_id, second_delivery_id);
        assert_eq!(requeued.leased_at.as_deref(), Some("2026-04-08T00:01:00Z"));
        assert_eq!(requeued.lease_owner.as_deref(), Some("watch-test"));
        assert_eq!(requeued.available_at, "2026-04-08T00:06:00Z");

        webhook
            .complete_processing_attempt_success(
                queued.invalidation_id,
                attempt.attempt_id,
                "2026-04-08T00:02:00Z",
                Some("processed"),
            )
            .unwrap_or_else(|error| panic!("attempt should complete successfully: {error}"));

        let pending = webhook
            .list_pending_invalidations()
            .unwrap_or_else(|error| panic!("pending invalidations should load: {error}"));
        assert_eq!(pending.len(), 1);
        let pending = &pending[0];
        assert_eq!(pending.delivery_id, second_delivery_id);
        assert_eq!(pending.last_queued_at, "2026-04-08T00:01:30Z");
        assert_eq!(pending.available_at, "2026-04-08T00:01:30Z");
        assert!(pending.leased_at.is_none());
        assert!(pending.lease_owner.is_none());
        assert!(pending.completed_at.is_none());
    }

    #[test]
    fn persists_runtime_heartbeat() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        store
            .webhook()
            .upsert_runtime_heartbeat(&RuntimeHeartbeatRecord {
                component: "receiver".to_owned(),
                mode: "hybrid".to_owned(),
                bind_address: Some("127.0.0.1:8799".to_owned()),
                public_base_url: Some("https://example.test".to_owned()),
                detail: Some("ready".to_owned()),
                last_seen_at: "2026-04-08T00:00:00Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("heartbeat should persist: {error}"));

        let heartbeats = store
            .webhook()
            .list_runtime_heartbeats()
            .unwrap_or_else(|error| panic!("heartbeats should load: {error}"));
        assert_eq!(heartbeats.len(), 1);
        assert_eq!(heartbeats[0].component, "receiver");
    }

    #[test]
    fn now_rfc3339_uses_fixed_width_subseconds() {
        let timestamp =
            now_rfc3339().unwrap_or_else(|error| panic!("timestamp should render: {error}"));

        let (_, fractional) = timestamp
            .split_once('.')
            .unwrap_or_else(|| panic!("timestamp should include fractional seconds"));
        assert_eq!(fractional.len(), 10);
        assert!(fractional.ends_with('Z'));
    }

    #[test]
    fn claim_available_invalidations_handles_mixed_timestamp_precision() {
        let store =
            Store::open_in_memory().unwrap_or_else(|error| panic!("store should open: {error}"));
        let webhook = store.webhook();
        let delivery_id = match webhook
            .insert_accepted_delivery(&AcceptedWebhookDeliveryInput {
                delivery_fingerprint: "queue-mixed-precision".to_owned(),
                received_at: "2026-04-08T00:00:00Z".to_owned(),
                signature_timestamp: Some("2026-04-08T00:00:00Z".to_owned()),
                data_type: Some("daily_sleep".to_owned()),
                event_type: Some(WebhookEventType::Create),
                object_id: Some("sleep_1".to_owned()),
                payload_json: "{}".to_owned(),
                headers_json: "{}".to_owned(),
                query_json: "{}".to_owned(),
            })
            .unwrap_or_else(|error| panic!("accepted delivery should insert: {error}"))
        {
            AcceptedWebhookDeliveryResult::Inserted(record)
            | AcceptedWebhookDeliveryResult::Duplicate(record) => record.delivery_id,
        };
        webhook
            .enqueue_invalidation(&InvalidationInput {
                queue_key: "daily_sleep:create:sleep_1".to_owned(),
                data_type: "daily_sleep".to_owned(),
                event_type: WebhookEventType::Create,
                object_id: Some("sleep_1".to_owned()),
                delivery_id,
                queued_at: "2026-04-08T00:00:00Z".to_owned(),
                available_at: "2026-04-08T00:00:00Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("first invalidation should queue: {error}"));
        webhook
            .enqueue_invalidation(&InvalidationInput {
                queue_key: "daily_sleep:update:sleep_1".to_owned(),
                data_type: "daily_sleep".to_owned(),
                event_type: WebhookEventType::Update,
                object_id: Some("sleep_1".to_owned()),
                delivery_id,
                queued_at: "2026-04-08T00:00:00.100000000Z".to_owned(),
                available_at: "2026-04-08T00:00:00.100000000Z".to_owned(),
            })
            .unwrap_or_else(|error| panic!("second invalidation should queue: {error}"));

        let claimed_at = format_rfc3339_utc(
            OffsetDateTime::parse(
                "2026-04-08T00:00:00.050000000Z",
                &time::format_description::well_known::Rfc3339,
            )
            .unwrap_or_else(|error| panic!("claimed_at should parse: {error}")),
        )
        .unwrap_or_else(|error| panic!("claimed_at should format: {error}"));
        let lease_until = "2026-04-08T00:05:00.000000000Z".to_owned();
        let claimed = webhook
            .claim_available_invalidations("watch-test", &claimed_at, &lease_until, 8)
            .unwrap_or_else(|error| panic!("claim should succeed: {error}"));

        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].event_type, WebhookEventType::Create);
    }
}
