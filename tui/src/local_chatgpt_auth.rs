#![cfg(test)]

use std::path::Path;

use agere_config::types::AuthCredentialsStoreMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalChatgptAuth {
    pub(crate) access_token: String,
    pub(crate) chatgpt_account_id: String,
    pub(crate) chatgpt_plan_type: Option<String>,
}

pub(crate) fn load_local_chatgpt_auth(
    _agere_home: &Path,
    _auth_credentials_store_mode: AuthCredentialsStoreMode,
    _forced_chatgpt_workspace_id: Option<&str>,
) -> Result<LocalChatgptAuth, String> {
    Err("local ChatGPT auth is no longer supported".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    #[test]
    fn rejects_all_local_chatgpt_auth() {
        let agere_home = TempDir::new().expect("tempdir");

        let err = load_local_chatgpt_auth(agere_home.path(), AuthCredentialsStoreMode::File, None)
            .expect_err("should always fail");

        assert_eq!(err, "local ChatGPT auth is no longer supported");
    }
}
