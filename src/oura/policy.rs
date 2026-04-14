use time::Duration;

use crate::config::RefreshConfig;
use crate::refresh::SyncFamily;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncSupportMode {
    PeriodicOnly,
    WebhookAssisted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncPolicy {
    pub family: SyncFamily,
    pub overlap: Duration,
    pub reconcile_window: Duration,
    pub startup_catchup_ceiling: Duration,
    pub backfill_chunk: Duration,
    pub support_mode: SyncSupportMode,
}

impl SyncPolicy {
    #[must_use]
    pub fn for_family(refresh: &RefreshConfig, family: SyncFamily) -> Self {
        match family {
            SyncFamily::Personal => Self {
                family,
                overlap: Duration::ZERO,
                reconcile_window: Duration::ZERO,
                startup_catchup_ceiling: Duration::ZERO,
                backfill_chunk: Duration::days(30),
                support_mode: SyncSupportMode::PeriodicOnly,
            },
            SyncFamily::Daily | SyncFamily::Spo2 => Self {
                family,
                overlap: Duration::days(i64::from(refresh.daily_overlap_days)),
                reconcile_window: Duration::days(i64::from(refresh.daily_reconcile_days)),
                startup_catchup_ceiling: Duration::days(i64::from(
                    refresh.daily_startup_catchup_days,
                )),
                backfill_chunk: Duration::days(i64::from(refresh.daily_backfill_chunk_days)),
                support_mode: if family == SyncFamily::Daily {
                    SyncSupportMode::WebhookAssisted
                } else {
                    SyncSupportMode::PeriodicOnly
                },
            },
            SyncFamily::Heartrate => Self {
                family,
                overlap: Duration::minutes(i64::from(refresh.heartrate_overlap_minutes)),
                reconcile_window: Duration::days(i64::from(refresh.heartrate_reconcile_days)),
                startup_catchup_ceiling: Duration::days(i64::from(
                    refresh.heartrate_startup_catchup_days,
                )),
                backfill_chunk: Duration::days(i64::from(refresh.heartrate_backfill_chunk_days)),
                support_mode: SyncSupportMode::PeriodicOnly,
            },
            SyncFamily::Workout => Self {
                family,
                overlap: Duration::days(i64::from(refresh.workout_overlap_days)),
                reconcile_window: Duration::days(i64::from(refresh.workout_reconcile_days)),
                startup_catchup_ceiling: Duration::days(i64::from(
                    refresh.workout_startup_catchup_days,
                )),
                backfill_chunk: Duration::days(i64::from(refresh.workout_backfill_chunk_days)),
                support_mode: SyncSupportMode::WebhookAssisted,
            },
            SyncFamily::EnhancedTag => Self {
                family,
                overlap: Duration::days(i64::from(refresh.enhanced_tag_overlap_days)),
                reconcile_window: Duration::days(i64::from(refresh.enhanced_tag_reconcile_days)),
                startup_catchup_ceiling: Duration::days(i64::from(
                    refresh.enhanced_tag_startup_catchup_days,
                )),
                backfill_chunk: Duration::days(i64::from(refresh.enhanced_tag_backfill_chunk_days)),
                support_mode: SyncSupportMode::WebhookAssisted,
            },
            SyncFamily::Session => Self {
                family,
                overlap: Duration::days(i64::from(refresh.session_overlap_days)),
                reconcile_window: Duration::days(i64::from(refresh.session_reconcile_days)),
                startup_catchup_ceiling: Duration::days(i64::from(
                    refresh.session_startup_catchup_days,
                )),
                backfill_chunk: Duration::days(i64::from(refresh.session_backfill_chunk_days)),
                support_mode: SyncSupportMode::WebhookAssisted,
            },
        }
    }

    #[must_use]
    pub const fn reconcile_days(self) -> i64 {
        self.reconcile_window.whole_days()
    }

    #[must_use]
    pub const fn startup_catchup_days(self) -> i64 {
        self.startup_catchup_ceiling.whole_days()
    }

    #[must_use]
    pub const fn backfill_chunk_days(self) -> i64 {
        self.backfill_chunk.whole_days()
    }
}
