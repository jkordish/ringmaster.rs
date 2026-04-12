use std::sync::Once;

use time::{OffsetDateTime, UtcOffset};
use tracing::warn;

static LOCAL_OFFSET_FALLBACK_WARNED: Once = Once::new();

pub fn current_local_day_string() -> String {
    current_day_string_at(OffsetDateTime::now_utc(), resolve_local_offset())
}

pub fn current_day_string_at(now_utc: OffsetDateTime, local_offset: Option<UtcOffset>) -> String {
    now_utc
        .to_offset(local_offset.unwrap_or(UtcOffset::UTC))
        .date()
        .to_string()
}

fn resolve_local_offset() -> Option<UtcOffset> {
    match UtcOffset::current_local_offset() {
        Ok(offset) => Some(offset),
        Err(error) => {
            LOCAL_OFFSET_FALLBACK_WARNED.call_once(|| {
                warn!(
                    error = %error,
                    "failed to determine the local UTC offset; falling back to UTC day boundaries"
                );
            });
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use time::macros::{datetime, offset};

    use super::current_day_string_at;

    #[test]
    fn current_day_string_uses_the_supplied_local_offset() {
        let now = datetime!(2026-04-12 00:30:00 UTC);

        assert_eq!(
            current_day_string_at(now, Some(offset!(-05:00))),
            "2026-04-11"
        );
        assert_eq!(
            current_day_string_at(now, Some(offset!(+02:00))),
            "2026-04-12"
        );
    }

    #[test]
    fn current_day_string_falls_back_to_utc_when_offset_is_unavailable() {
        let now = datetime!(2026-04-12 00:30:00 UTC);

        assert_eq!(current_day_string_at(now, None), "2026-04-12");
    }
}
