pub mod receiver;
pub mod subscriptions;

use serde::{Deserialize, Serialize};

use crate::refresh::SyncFamily;

pub const SUPPORTED_WEBHOOK_DATA_TYPES: [&str; 6] = [
    "daily_sleep",
    "daily_readiness",
    "daily_activity",
    "workout",
    "enhanced_tag",
    "session",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEventType {
    Create,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredWebhookSubscription {
    pub data_type: String,
    pub event_types: Vec<WebhookEventType>,
    pub enabled: bool,
}

impl WebhookEventType {
    pub const ALL: [Self; 3] = [Self::Create, Self::Update, Self::Delete];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "create" => Some(Self::Create),
            "update" => Some(Self::Update),
            "delete" => Some(Self::Delete),
            _ => None,
        }
    }
}

impl DesiredWebhookSubscription {
    pub fn normalized_event_types(&self) -> Vec<WebhookEventType> {
        let mut event_types = self.event_types.clone();
        event_types.sort_unstable();
        event_types.dedup();
        event_types
    }
}

pub fn default_desired_subscriptions() -> Vec<DesiredWebhookSubscription> {
    SUPPORTED_WEBHOOK_DATA_TYPES
        .into_iter()
        .map(|data_type| DesiredWebhookSubscription {
            data_type: data_type.to_owned(),
            event_types: WebhookEventType::ALL.to_vec(),
            enabled: true,
        })
        .collect()
}

pub fn is_supported_data_type(data_type: &str) -> bool {
    SUPPORTED_WEBHOOK_DATA_TYPES.contains(&data_type)
}

pub fn sync_family_for_data_type(data_type: &str) -> Option<SyncFamily> {
    match data_type {
        "daily_sleep" | "daily_readiness" | "daily_activity" => Some(SyncFamily::Daily),
        "workout" => Some(SyncFamily::Workout),
        "enhanced_tag" => Some(SyncFamily::EnhancedTag),
        "session" => Some(SyncFamily::Session),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SUPPORTED_WEBHOOK_DATA_TYPES, WebhookEventType, default_desired_subscriptions,
        is_supported_data_type, sync_family_for_data_type,
    };
    use crate::refresh::SyncFamily;

    #[test]
    fn defaults_cover_supported_data_types() {
        let subscriptions = default_desired_subscriptions();

        assert_eq!(subscriptions.len(), SUPPORTED_WEBHOOK_DATA_TYPES.len());
        assert!(
            subscriptions
                .iter()
                .all(|subscription| subscription.event_types.len() == 3 && subscription.enabled)
        );
    }

    #[test]
    fn parses_known_event_types() {
        assert_eq!(
            WebhookEventType::parse("create"),
            Some(WebhookEventType::Create)
        );
        assert_eq!(
            WebhookEventType::parse("update"),
            Some(WebhookEventType::Update)
        );
        assert_eq!(
            WebhookEventType::parse("delete"),
            Some(WebhookEventType::Delete)
        );
        assert_eq!(WebhookEventType::parse("noop"), None);
    }

    #[test]
    fn supported_data_types_map_to_sync_families() {
        assert!(is_supported_data_type("daily_sleep"));
        assert_eq!(
            sync_family_for_data_type("daily_sleep"),
            Some(SyncFamily::Daily)
        );
        assert_eq!(
            sync_family_for_data_type("workout"),
            Some(SyncFamily::Workout)
        );
        assert_eq!(sync_family_for_data_type("unsupported"), None);
    }
}
