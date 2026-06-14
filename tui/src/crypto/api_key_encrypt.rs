//! AES-256-GCM encryption for API keys stored in provider.toml.
//!
//! **Threat model**: This provides machine-binding, not true security.
//! The key is derived from hostname and agere_home path, so encrypted
//! API keys can only be decrypted on the same machine. This prevents
//! other machines from using stolen provider.toml files, but does not
//! protect against attackers with access to this machine.
//!
//! Key derivation: SHA256(hostname + ":" + agere_home_absolute_path)
//! Data format: Base64(nonce[12] + ciphertext + auth_tag[16])

use std::path::Path;

use aes_gcm::Aes256Gcm;
use aes_gcm::Nonce;
use aes_gcm::aead::Aead;
use aes_gcm::aead::KeyInit;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use gethostname::gethostname;
use sha2::Digest;
use sha2::Sha256;

const NONCE_LEN: usize = 12;
const GCM_TAG_LEN: usize = 16;
const MIN_BLOB_LEN: usize = NONCE_LEN + GCM_TAG_LEN;

/// Derive AES-256 key from hostname and agere_home path.
fn derive_key(agere_home: &Path) -> [u8; 32] {
    let hostname = gethostname().to_string_lossy().into_owned();
    let path_str = agere_home.display().to_string();
    let input = format!("{hostname}:{path_str}");
    let hash = Sha256::digest(input.as_bytes());
    hash.into()
}

/// Encrypt an API key and return Base64-encoded result.
pub fn encrypt_api_key(api_key: &str, agere_home: &Path) -> Result<String, anyhow::Error> {
    if api_key.is_empty() {
        return Ok(String::new());
    }

    let key = derive_key(agere_home);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| anyhow::anyhow!("failed to init cipher: {e}"))?;

    // Generate random 12-byte nonce
    let nonce_bytes: [u8; NONCE_LEN] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, api_key.as_bytes())
        .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?;

    // Concatenate nonce + ciphertext and base64 encode
    let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);

    Ok(BASE64_STANDARD.encode(&blob))
}

/// Decrypt a Base64-encoded encrypted API key.
pub fn decrypt_api_key(encrypted: &str, agere_home: &Path) -> Result<String, anyhow::Error> {
    if encrypted.is_empty() {
        return Ok(String::new());
    }

    let blob = BASE64_STANDARD
        .decode(encrypted)
        .map_err(|e| anyhow::anyhow!("base64 decode failed: {e}"))?;

    if blob.len() < MIN_BLOB_LEN {
        return Err(anyhow::anyhow!("encrypted data too short"));
    }

    let nonce_bytes = &blob[0..NONCE_LEN];
    let ciphertext = &blob[NONCE_LEN..];

    let key = derive_key(agere_home);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| anyhow::anyhow!("failed to init cipher: {e}"))?;

    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("decryption failed: {e}"))?;

    String::from_utf8(plaintext)
        .map_err(|e| anyhow::anyhow!("decrypted data is not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let tmp = tempdir().expect("tempdir");
        let agere_home = tmp.path();
        let api_key = "sk-test-12345";

        let encrypted = encrypt_api_key(api_key, agere_home).expect("encrypt");
        let decrypted = decrypt_api_key(&encrypted, agere_home).expect("decrypt");

        assert_eq!(decrypted, api_key);
    }

    #[test]
    fn empty_key_returns_empty() {
        let tmp = tempdir().expect("tempdir");
        let agere_home = tmp.path();

        let encrypted = encrypt_api_key("", agere_home).expect("encrypt");
        assert!(encrypted.is_empty());

        let decrypted = decrypt_api_key("", agere_home).expect("decrypt");
        assert!(decrypted.is_empty());
    }

    #[test]
    fn corrupted_data_fails() {
        let tmp = tempdir().expect("tempdir");
        let agere_home = tmp.path();

        let result = decrypt_api_key("invalid-base64!!!", agere_home);
        assert!(result.is_err());
    }

    #[test]
    fn too_short_data_fails() {
        let tmp = tempdir().expect("tempdir");
        let agere_home = tmp.path();

        // 20 bytes is less than MIN_BLOB_LEN (28), so should fail
        let short = BASE64_STANDARD.encode([0u8; 20]);
        let result = decrypt_api_key(&short, agere_home);
        assert!(result.is_err());
    }

    #[test]
    fn different_path_fails() {
        let tmp1 = tempdir().expect("tempdir1");
        let tmp2 = tempdir().expect("tempdir2");

        let api_key = "sk-test-secret";
        let encrypted = encrypt_api_key(api_key, tmp1.path()).expect("encrypt");

        // Decrypt with different path should fail due to different derived key
        let result = decrypt_api_key(&encrypted, tmp2.path());
        assert!(result.is_err());
    }
}
