//! Persistent archive of every provider the user has interacted with.
//!
//! `~/.openagere/provider.toml` is the TUI-managed source of truth for provider
//! definitions, API keys, and per-provider models. Runtime never reads this
//! file; it duplicates the active provider's bits into `config.toml` so that
//! the core `Config` loader keeps working without changes.

use agere_config::config_toml::ModelConfig;
use agere_model_provider_info::WireApi;
use agere_utils_fs::path_utils::write_atomically;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml_edit::ArrayOfTables;
use toml_edit::DocumentMut;
use toml_edit::Item as TomlItem;
use toml_edit::Table as TomlTable;
use toml_edit::value;

pub(crate) const PROVIDER_TOML_FILE: &str = "provider.toml";

/// A single provider entry as persisted in `provider.toml`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ProviderEntry {
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub wire_api: SerializableWireApi,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_api_key: Option<String>,
    #[serde(default)]
    pub is_custom: bool,
    #[serde(default)]
    pub models: Vec<ModelConfig>,
}

impl ProviderEntry {
    /// Get the decrypted API key, or empty string if not available.
    pub fn get_api_key(&self, agere_home: &Path) -> String {
        match &self.encrypted_api_key {
            Some(encrypted) if !encrypted.is_empty() => {
                crate::crypto::decrypt_api_key(encrypted, agere_home).unwrap_or_default()
            }
            _ => String::new(),
        }
    }

    /// Check if this entry has a configured API key.
    pub fn has_api_key(&self) -> bool {
        self.encrypted_api_key
            .as_ref()
            .map(|k| !k.is_empty())
            .unwrap_or(false)
    }
}

/// Wrapper around `WireApi` to round-trip through `toml`/`serde_json` cleanly.
///
/// `agere_model_provider_info::WireApi` implements custom `Deserialize`/
/// `Serialize` only against `toml`'s string format. Re-using it through the
/// `toml`/`toml_edit` round-trip works, so we just re-export it as a newtype
/// alias.
pub(crate) type SerializableWireApi = WireApi;

/// Result of loading `provider.toml`.
#[derive(Debug, Default)]
pub(crate) struct ProviderToml {
    pub providers: Vec<ProviderEntry>,
    pub warnings: Vec<String>,
}

impl ProviderToml {
    pub fn find(&self, name: &str) -> Option<&ProviderEntry> {
        self.providers.iter().find(|p| p.name == name)
    }
}

fn provider_toml_path(agere_home: &Path) -> PathBuf {
    agere_home.join(PROVIDER_TOML_FILE)
}

/// Load `provider.toml` with single-entry fault tolerance.
///
/// Each `[[providers]]` table is decoded independently. Failed entries are
/// skipped and surfaced via `warnings`. Duplicate names keep the first
/// occurrence and warn about the rest.
pub(crate) fn load(agere_home: &Path) -> ProviderToml {
    let path = provider_toml_path(agere_home);
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return ProviderToml::default(),
        Err(err) => {
            return ProviderToml {
                providers: Vec::new(),
                warnings: vec![format!("failed to read provider.toml: {err}")],
            };
        }
    };

    parse_provider_toml(&contents)
}

fn parse_provider_toml(contents: &str) -> ProviderToml {
    let mut warnings = Vec::new();

    let raw: toml::Value = match toml::from_str(contents) {
        Ok(v) => v,
        Err(err) => {
            return ProviderToml {
                providers: Vec::new(),
                warnings: vec![format!("provider.toml is not valid TOML: {err}")],
            };
        }
    };

    let raw_providers = raw
        .get("providers")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut seen_names = HashSet::new();
    let mut providers = Vec::new();
    for (idx, raw_entry) in raw_providers.into_iter().enumerate() {
        match raw_entry.try_into::<ProviderEntry>() {
            Ok(entry) => {
                if entry.name.trim().is_empty() {
                    warnings.push(format!("skipped providers[{idx}] with empty name"));
                    continue;
                }
                if !seen_names.insert(entry.name.clone()) {
                    warnings.push(format!(
                        "providers[{idx}] duplicate name '{name}' ignored",
                        idx = idx,
                        name = entry.name,
                    ));
                    continue;
                }
                providers.push(entry);
            }
            Err(err) => {
                warnings.push(format!("skipped providers[{idx}]: {err}"));
            }
        }
    }

    ProviderToml {
        providers,
        warnings,
    }
}

/// Insert or replace an entry by name; preserves surrounding formatting/comments.
pub(crate) fn upsert(agere_home: &Path, entry: &ProviderEntry) -> io::Result<()> {
    let path = provider_toml_path(agere_home);
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };

    let mut doc: DocumentMut = if contents.is_empty() {
        DocumentMut::new()
    } else {
        contents
            .parse::<DocumentMut>()
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?
    };

    let array = ensure_providers_array(&mut doc);
    let serialized = serialize_entry(entry);

    let existing_idx = (0..array.len()).find(|idx| {
        array
            .get(*idx)
            .and_then(|t| t.get("name"))
            .and_then(TomlItem::as_str)
            == Some(entry.name.as_str())
    });
    match existing_idx {
        Some(idx) => {
            // Update existing entry in place.
            if let Some(slot) = array.get_mut(idx) {
                *slot = serialized;
            }
        }
        None => {
            // Insert new entry at the front: collect all existing, clear, then prepend new.
            let existing: Vec<TomlTable> = array.iter().cloned().collect();
            array.clear();
            array.push(serialized);
            for table in existing {
                array.push(table);
            }
        }
    }

    write_atomically(&path, &doc.to_string())?;
    chmod_user_only(&path);
    Ok(())
}

/// Remove an entry by name. Returns true if anything was removed.
pub(crate) fn remove(agere_home: &Path, name: &str) -> io::Result<bool> {
    let path = provider_toml_path(agere_home);
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };

    let mut doc: DocumentMut = contents
        .parse::<DocumentMut>()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

    let array = ensure_providers_array(&mut doc);
    let mut removed = false;
    let mut idx = 0;
    while idx < array.len() {
        let matches = array
            .get(idx)
            .and_then(|t| t.get("name"))
            .and_then(TomlItem::as_str)
            == Some(name);
        if matches {
            array.remove(idx);
            removed = true;
        } else {
            idx += 1;
        }
    }

    if removed {
        write_atomically(&path, &doc.to_string())?;
        chmod_user_only(&path);
    }
    Ok(removed)
}

fn ensure_providers_array(doc: &mut DocumentMut) -> &mut ArrayOfTables {
    if !matches!(doc.get("providers"), Some(TomlItem::ArrayOfTables(_))) {
        doc.insert("providers", TomlItem::ArrayOfTables(ArrayOfTables::new()));
    }
    match doc.get_mut("providers") {
        Some(TomlItem::ArrayOfTables(array)) => array,
        _ => panic!("providers key should be an array of tables after insertion"),
    }
}

fn serialize_entry(entry: &ProviderEntry) -> TomlTable {
    let mut table = TomlTable::new();
    table.set_implicit(false);
    table["name"] = value(entry.name.clone());
    table["base_url"] = value(entry.base_url.clone());
    table["wire_api"] = value(entry.wire_api.to_string());
    if let Some(env_key) = &entry.env_key {
        table["env_key"] = value(env_key.clone());
    }
    if let Some(encrypted_api_key) = &entry.encrypted_api_key {
        table["encrypted_api_key"] = value(encrypted_api_key.clone());
    }
    table["is_custom"] = value(entry.is_custom);

    let mut models_array = toml_edit::Array::new();
    for model in &entry.models {
        let mut inline = toml_edit::InlineTable::new();
        inline.insert("name", model.name.clone().into());
        if let Some(cw) = model.context_window {
            inline.insert("context_window", cw.into());
        }
        models_array.push(inline);
    }
    table["models"] = TomlItem::Value(models_array.into());
    table
}

#[cfg(unix)]
fn chmod_user_only(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(meta) => {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            if let Err(e) = std::fs::set_permissions(path, perms) {
                tracing::warn!("failed to set permissions on {}: {e}", path.display());
            }
        }
        Err(e) => {
            tracing::warn!("failed to read metadata for {}: {e}", path.display());
        }
    }
}

#[cfg(not(unix))]
fn chmod_user_only(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    fn make_entry(name: &str, api_key: Option<&str>, agere_home: &Path) -> ProviderEntry {
        let encrypted_api_key = api_key
            .filter(|k| !k.is_empty())
            .map(|k| crate::crypto::encrypt_api_key(k, agere_home).expect("encrypt"));
        ProviderEntry {
            name: name.to_string(),
            base_url: format!("https://api.{name}.test/v1"),
            wire_api: WireApi::Chat,
            env_key: Some(format!("{}_API_KEY", name.to_uppercase().replace('-', "_"))),
            encrypted_api_key,
            is_custom: false,
            models: vec![ModelConfig {
                name: format!("{name}-default"),
                context_window: Some(128000),
            }],
        }
    }

    #[test]
    fn load_returns_empty_when_file_missing() {
        let tmp = tempdir().expect("tmpdir");
        let loaded = load(tmp.path());
        assert!(loaded.providers.is_empty());
        assert!(loaded.warnings.is_empty());
    }

    #[test]
    fn upsert_then_load_round_trip() {
        let tmp = tempdir().expect("tmpdir");
        let entry = make_entry("deepseek", Some("sk-abc"), tmp.path());
        upsert(tmp.path(), &entry).expect("upsert");

        let loaded = load(tmp.path());
        assert_eq!(loaded.providers.len(), 1);
        assert_eq!(loaded.providers[0].name, "deepseek");
        assert_eq!(loaded.providers[0].get_api_key(tmp.path()), "sk-abc");
        assert!(loaded.warnings.is_empty());

        let mut updated = entry;
        updated.encrypted_api_key =
            Some(crate::crypto::encrypt_api_key("sk-new", tmp.path()).expect("encrypt"));
        upsert(tmp.path(), &updated).expect("upsert update");

        let loaded2 = load(tmp.path());
        assert_eq!(loaded2.providers.len(), 1);
        assert_eq!(loaded2.providers[0].get_api_key(tmp.path()), "sk-new");
    }

    #[test]
    fn remove_returns_false_when_absent() {
        let tmp = tempdir().expect("tmpdir");
        upsert(tmp.path(), &make_entry("a", Some("k"), tmp.path())).expect("upsert");
        assert!(!remove(tmp.path(), "missing").expect("remove"));
        assert!(remove(tmp.path(), "a").expect("remove"));
        let loaded = load(tmp.path());
        assert!(loaded.providers.is_empty());
    }

    #[test]
    fn duplicate_names_keep_first_and_warn() {
        let contents = r#"
[[providers]]
name = "dup"
base_url = "https://a.test"
wire_api = "chat"

[[providers]]
name = "dup"
base_url = "https://b.test"
wire_api = "chat"
"#;
        let parsed = parse_provider_toml(contents);
        assert_eq!(parsed.providers.len(), 1);
        assert_eq!(parsed.providers[0].base_url, "https://a.test");
        assert_eq!(parsed.warnings.len(), 1);
        assert!(parsed.warnings[0].contains("duplicate"));
    }

    #[test]
    fn skips_malformed_entry_keeps_rest() {
        let contents = r#"
[[providers]]
name = "ok"
base_url = "https://ok.test"
wire_api = "chat"

[[providers]]
name = "bad"
wire_api = "not-a-wire-api"
"#;
        let parsed = parse_provider_toml(contents);
        assert_eq!(parsed.providers.len(), 1);
        assert_eq!(parsed.providers[0].name, "ok");
        assert_eq!(parsed.warnings.len(), 1);
        assert!(parsed.warnings[0].contains("skipped providers[1]"));
    }
}
