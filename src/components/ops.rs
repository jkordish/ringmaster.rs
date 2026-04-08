use crate::app::AppState;
use crate::components::Component;

#[derive(Debug, Default)]
pub struct Ops;

impl Component for Ops {
    fn render(&self, state: &AppState) -> String {
        let warning_lines = state
            .warnings
            .iter()
            .map(|warning| format!("  - {warning}"))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "\
[Ops]
active_screen: {}
warnings:
{}",
            state.screen_name(),
            warning_lines
        )
    }
}
