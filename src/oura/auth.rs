use crate::config::Config;

pub fn login_scaffold(config: &Config) -> String {
    format!(
        "\
ringmaster.rs auth login

status: scaffold
callback_url: {}
next_step:
  replace this scaffold with a loopback OAuth flow for Oura Cloud API v2.
",
        config.oauth_callback
    )
}
