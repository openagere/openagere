//! API key encryption module for provider.toml.
//!
//! Uses AES-256-GCM with machine-bound key derivation.

mod api_key_encrypt;

pub use api_key_encrypt::decrypt_api_key;
pub use api_key_encrypt::encrypt_api_key;
