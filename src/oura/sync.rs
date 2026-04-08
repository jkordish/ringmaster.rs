use crate::config::Config;

pub fn sync_once_scaffold(config: &Config) -> String {
    format!(
        "\
ringmaster.rs sync once

status: scaffold
database_path: {}
next_step:
  replace this scaffold with poll-first Oura sync logic that writes to the local store.
",
        config.state_dir.join("ringmaster.db").display()
    )
}
