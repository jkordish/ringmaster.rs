use crate::app::AppState;
use crate::components::Component;

#[derive(Debug, Default)]
pub struct Dashboard;

impl Component for Dashboard {
    fn render(&self, state: &AppState) -> String {
        format!(
            "\
[Dashboard]
sleep_score: {}
readiness_score: {}
activity_score: {}
freshness: {}
capabilities: {}",
            state.snapshot.sleep_score,
            state.snapshot.readiness_score,
            state.snapshot.activity_score,
            state.freshness.render(),
            state.capabilities.render(),
        )
    }
}
