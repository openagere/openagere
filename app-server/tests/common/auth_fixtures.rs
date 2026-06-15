use std::path::Path;

use agere_login::AuthDotJson;
use agere_login::save_auth;
use anyhow::Result;

/// Builder for writing a fake auth.json in tests.
/// Stubbed: only supports API key auth after login crate simplification.
#[derive(Debug, Clone)]
pub struct ChatGptAuthFixture {
    api_key: String,
}

impl ChatGptAuthFixture {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
        }
    }

    // Stubbed methods kept for API compatibility.
    pub fn refresh_token(self, _refresh_token: impl Into<String>) -> Self {
        self
    }
    pub fn account_id(self, _account_id: impl Into<String>) -> Self {
        self
    }
    pub fn plan_type(self, _plan_type: impl Into<String>) -> Self {
        self
    }
    pub fn chatgpt_user_id(self, _chatgpt_user_id: impl Into<String>) -> Self {
        self
    }
    pub fn chatgpt_account_id(self, _chatgpt_account_id: impl Into<String>) -> Self {
        self
    }
    pub fn email(self, _email: impl Into<String>) -> Self {
        self
    }
    pub fn last_refresh(self, _last_refresh: Option<chrono::DateTime<chrono::Utc>>) -> Self {
        self
    }
    pub fn claims(self, _claims: ChatGptIdTokenClaims) -> Self {
        self
    }
}

/// Stub type kept for API compatibility.
#[derive(Debug, Clone, Default)]
pub struct ChatGptIdTokenClaims;

impl ChatGptIdTokenClaims {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn email(self, _email: impl Into<String>) -> Self {
        self
    }
    pub fn plan_type(self, _plan_type: impl Into<String>) -> Self {
        self
    }
    pub fn chatgpt_user_id(self, _chatgpt_user_id: impl Into<String>) -> Self {
        self
    }
    pub fn chatgpt_account_id(self, _chatgpt_account_id: impl Into<String>) -> Self {
        self
    }
}

pub fn encode_id_token(_claims: &ChatGptIdTokenClaims) -> Result<String> {
    Ok("stub-id-token".to_string())
}

pub fn write_chatgpt_auth(
    agere_home: &Path,
    fixture: ChatGptAuthFixture,
    _cli_auth_credentials_store_mode: agere_config::types::AuthCredentialsStoreMode,
) -> Result<()> {
    let auth = AuthDotJson {
        openai_api_key: Some(fixture.api_key),
    };

    save_auth(agere_home, &auth)?;
    Ok(())
}
