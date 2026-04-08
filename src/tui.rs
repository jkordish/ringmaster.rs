use crate::app::AppState;
use crate::components::{dashboard::Dashboard, ops::Ops, timeline::Timeline, trends::Trends, Component};
use crate::config::Config;

pub fn render_placeholder(config: &Config) -> String {
    let state = AppState::demo();

    format!(
        "\
ringmaster.rs
status: bootstrap placeholder
config_dir: {}
state_dir: {}
active_screen: {}

{}

Tip:
  Run `ringmaster demo` for deterministic sample output.
",
        config.config_dir.display(),
        config.state_dir.display(),
        state.screen_name(),
        render_screen_bundle(&state)
    )
}

pub fn render_demo(config: &Config) -> String {
    let state = AppState::demo();

    format!(
        "\
ringmaster.rs demo
config_dir: {}
state_dir: {}

{}
",
        config.config_dir.display(),
        config.state_dir.display(),
        render_screen_bundle(&state)
    )
}

fn render_screen_bundle(state: &AppState) -> String {
    let components: [&dyn Component; 4] = [&Dashboard, &Timeline, &Trends, &Ops];

    components
        .iter()
        .map(|component| component.render(state))
        .collect::<Vec<_>>()
        .join("\n\n")
}
