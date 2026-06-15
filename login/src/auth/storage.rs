use serde::Deserialize;
use serde::Serialize;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::path::PathBuf;

/// Expected structure for $AGERE_HOME/auth.json.
/// Simplified: only stores API key.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AuthDotJson {
    #[serde(rename = "OPENAI_API_KEY")]
    pub openai_api_key: Option<String>,
}

pub(super) fn get_auth_file(agere_home: &Path) -> PathBuf {
    agere_home.join("auth.json")
}

pub(super) fn delete_file_if_exists(agere_home: &Path) -> std::io::Result<bool> {
    let auth_file = get_auth_file(agere_home);
    match std::fs::remove_file(&auth_file) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

/// File-based auth storage — the only storage backend.
#[derive(Clone, Debug)]
pub(super) struct FileAuthStorage {
    agere_home: PathBuf,
}

impl FileAuthStorage {
    pub(super) fn new(agere_home: PathBuf) -> Self {
        Self { agere_home }
    }

    pub(super) fn load(&self) -> std::io::Result<Option<AuthDotJson>> {
        let auth_file = get_auth_file(&self.agere_home);
        match self.try_read_auth_json(&auth_file) {
            Ok(auth) => Ok(Some(auth)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub(super) fn save(&self, auth_dot_json: &AuthDotJson) -> std::io::Result<()> {
        let auth_file = get_auth_file(&self.agere_home);

        if let Some(parent) = auth_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json_data = serde_json::to_string_pretty(auth_dot_json)?;
        let mut options = OpenOptions::new();
        options.truncate(true).write(true).create(true);
        #[cfg(unix)]
        {
            options.mode(0o600);
        }
        let mut file = options.open(auth_file)?;
        file.write_all(json_data.as_bytes())?;
        file.flush()?;
        Ok(())
    }

    pub(super) fn delete(&self) -> std::io::Result<bool> {
        delete_file_if_exists(&self.agere_home)
    }

    fn try_read_auth_json(&self, auth_file: &Path) -> std::io::Result<AuthDotJson> {
        let mut file = File::open(auth_file)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let auth_dot_json: AuthDotJson = serde_json::from_str(&contents)?;
        Ok(auth_dot_json)
    }
}

pub(super) fn create_auth_storage(agere_home: PathBuf) -> FileAuthStorage {
    FileAuthStorage::new(agere_home)
}
