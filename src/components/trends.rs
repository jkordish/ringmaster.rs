use crate::app::AppState;
use crate::components::Component;

#[derive(Debug, Default)]
pub struct Trends;

impl Component for Trends {
    fn render(&self, state: &AppState) -> String {
        format!(
            "\
[Trends]
7d_baseline: {}
30d_baseline: {}
90d_baseline: {}
delta_summary: {}",
            state.snapshot.baseline_7d,
            state.snapshot.baseline_30d,
            state.snapshot.baseline_90d,
            state.snapshot.delta_summary,
        )
    }
}
