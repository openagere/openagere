use agere_core::config::Config;
use agere_login::AuthManager;

pub(crate) async fn rate_limits_ok(_auth_manager: &AuthManager, _config: &Config) -> bool {
    true
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
