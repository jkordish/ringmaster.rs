#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryBoundary {
    pub name: &'static str,
    pub purpose: &'static str,
}

pub const QUERY_BOUNDARIES: &[QueryBoundary] = &[
    QueryBoundary {
        name: "daily_snapshot",
        purpose: "Return summary data for dashboard rendering.",
    },
    QueryBoundary {
        name: "sync_health",
        purpose: "Return latest sync timestamps and error surfaces.",
    },
];
