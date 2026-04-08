use crate::app::AppState;
use crate::components::Component;

#[derive(Debug, Default)]
pub struct Timeline;

impl Component for Timeline {
    fn render(&self, state: &AppState) -> String {
        format!(
            "\
[Timeline]
intraday_hr_preview: {}
overlay_placeholders: tags, workouts, sessions
screen: {}",
            state.snapshot.heart_rate_preview.join(" "),
            state.screen_name(),
        )
    }
}
