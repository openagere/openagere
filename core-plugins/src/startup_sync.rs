use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use serde::Deserialize;
use sha2::Digest;
use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tar::Archive;
use tar::EntryType;
use tempfile::TempDir;
use zip::ZipArchive;

const CURATED_PLUGINS_RELATIVE_DIR: &str = ".tmp/plugins";
const CURATED_PLUGINS_SHA_FILE: &str = ".tmp/plugins.sha";
const OPENAGERE_PLUGINS_NPM_PACKAGE: &str = "@openagere/plugins";
const OPENAGERE_PLUGINS_OWNER: &str = "openagere";
const OPENAGERE_PLUGINS_REPO: &str = "plugins";
const DEFAULT_NPM_REGISTRY_URL: &str = "https://registry.npmjs.org";
const HTTP_USER_AGENT: &str = "openagere";
const NPMMIRROR_REGISTRY_URL: &str = "https://registry.npmmirror.com";
const GITHUB_PROBE_TIMEOUT_SECS: u64 = 4;
const GITHUB_ARCHIVE_REQUEST_TIMEOUT_SECS: u64 = 60;
const NPM_METADATA_REQUEST_TIMEOUT_SECS: u64 = 10;
const NPM_TARBALL_REQUEST_TIMEOUT_SECS: u64 = 120;
const GITHUB_PROBE_TIMEOUT: Duration = Duration::from_secs(GITHUB_PROBE_TIMEOUT_SECS);
const GITHUB_ARCHIVE_REQUEST_TIMEOUT: Duration =
    Duration::from_secs(GITHUB_ARCHIVE_REQUEST_TIMEOUT_SECS);
const NPM_METADATA_REQUEST_TIMEOUT: Duration =
    Duration::from_secs(NPM_METADATA_REQUEST_TIMEOUT_SECS);
const NPM_TARBALL_REQUEST_TIMEOUT: Duration = Duration::from_secs(NPM_TARBALL_REQUEST_TIMEOUT_SECS);

pub fn curated_plugins_repo_path(agere_home: &Path) -> PathBuf {
    agere_home.join(CURATED_PLUGINS_RELATIVE_DIR)
}

pub fn read_curated_plugins_sha(agere_home: &Path) -> Option<String> {
    read_sha_file(curated_plugins_sha_path(agere_home).as_path())
}

fn curated_plugins_sha_path(agere_home: &Path) -> PathBuf {
    agere_home.join(CURATED_PLUGINS_SHA_FILE)
}

pub fn sync_openai_plugins_repo(agere_home: &Path) -> Result<String, String> {
    sync_openai_plugins_repo_with_npm_fallback(
        agere_home,
        "git",
        "https://api.github.com",
        NPMMIRROR_REGISTRY_URL,
    )
}

pub fn has_local_curated_plugins_snapshot(agere_home: &Path) -> bool {
    curated_plugins_repo_path(agere_home)
        .join(".agents/plugins/marketplace.json")
        .is_file()
        && agere_home.join(CURATED_PLUGINS_SHA_FILE).is_file()
}

fn read_sha_file(sha_path: &Path) -> Option<String> {
    std::fs::read_to_string(sha_path)
        .ok()
        .map(|sha| sha.trim().to_string())
        .filter(|sha| !sha.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;
    use tar::Builder;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    #[test]
    fn curated_plugins_repo_path_uses_agere_home_tmp_dir() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(
            curated_plugins_repo_path(tmp.path()),
            tmp.path().join(CURATED_PLUGINS_RELATIVE_DIR)
        );
    }

    #[test]
    fn read_curated_plugins_sha_reads_trimmed_sha_file() {
        let tmp = tempfile::tempdir().unwrap();
        let sha_path = tmp.path().join(CURATED_PLUGINS_SHA_FILE);
        std::fs::create_dir_all(
            sha_path
                .parent()
                .expect("plugins.sha path should include a directory"),
        )
        .unwrap();
        std::fs::write(sha_path, "abc123\n").unwrap();
        assert_eq!(
            read_curated_plugins_sha(tmp.path()).as_deref(),
            Some("abc123")
        );
    }

    #[test]
    fn sync_openai_plugins_repo_reports_npm_resolution_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let err = sync_openai_plugins_repo_with_npm_fallbacks(
            tmp.path(),
            "missing-git-binary",
            "http://127.0.0.1:9",
            &["http://127.0.0.1:9"],
        )
        .unwrap_err();
        assert!(
            err.contains("resolve npm marketplace package"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn sync_openai_plugins_repo_falls_back_to_npm_mirror() {
        let tmp = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        let tarball = npm_marketplace_tarball();
        let integrity = integrity_for_bytes(&tarball);

        Mock::given(method("GET"))
            .and(path("/@openagere%2fplugins"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"{{
  "dist-tags": {{"latest": "0.0.1"}},
  "versions": {{
    "0.0.1": {{
      "dist": {{
        "tarball": "{}/@openagere/plugins/-/plugins-0.0.1.tgz",
        "integrity": "{}"
      }}
    }}
  }}
}}"#,
                server.uri(),
                integrity
            )))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/@openagere/plugins/-/plugins-0.0.1.tgz"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball))
            .mount(&server)
            .await;

        let registry_url = server.uri();
        let synced_version = tokio::task::spawn_blocking({
            let agere_home = tmp.path().to_path_buf();
            move || {
                sync_openai_plugins_repo_with_npm_fallbacks(
                    agere_home.as_path(),
                    "missing-git-binary",
                    "http://127.0.0.1:9",
                    &[registry_url.as_str()],
                )
            }
        })
        .await
        .unwrap()
        .unwrap();

        assert_eq!(synced_version, "npm:0.0.1");
        assert!(
            curated_plugins_repo_path(tmp.path())
                .join(".agents/plugins/marketplace.json")
                .is_file()
        );
        assert!(
            curated_plugins_repo_path(tmp.path())
                .join("plugins/gmail/.agere-plugin/plugin.json")
                .is_file()
        );
        assert_eq!(
            read_curated_plugins_sha(tmp.path()).as_deref(),
            Some("npm:0.0.1")
        );
    }

    #[tokio::test]
    async fn sync_openai_plugins_repo_tries_npmmirror_before_npmjs() {
        let tmp = tempfile::tempdir().unwrap();
        let mirror = MockServer::start().await;
        let npmjs = MockServer::start().await;
        let tarball = npm_marketplace_tarball();
        let integrity = integrity_for_bytes(&tarball);

        Mock::given(method("GET"))
            .and(path("/@openagere%2fplugins"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"{{
  "dist-tags": {{"latest": "0.0.2"}},
  "versions": {{
    "0.0.2": {{
      "dist": {{
        "tarball": "{}/@openagere/plugins/-/plugins-0.0.2.tgz",
        "integrity": "{}"
      }}
    }}
  }}
}}"#,
                mirror.uri(),
                integrity
            )))
            .mount(&mirror)
            .await;
        Mock::given(method("GET"))
            .and(path("/@openagere/plugins/-/plugins-0.0.2.tgz"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball))
            .mount(&mirror)
            .await;

        Mock::given(method("GET"))
            .and(path("/@openagere%2fplugins"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"dist-tags":{"latest":"0.0.1"},"versions":{}}"#),
            )
            .mount(&npmjs)
            .await;

        let synced_version = tokio::task::spawn_blocking({
            let agere_home = tmp.path().to_path_buf();
            let mirror_url = mirror.uri();
            let npmjs_url = npmjs.uri();
            move || {
                sync_openai_plugins_repo_with_npm_fallbacks(
                    agere_home.as_path(),
                    "missing-git-binary",
                    "http://127.0.0.1:9",
                    &[mirror_url.as_str(), npmjs_url.as_str()],
                )
            }
        })
        .await
        .unwrap()
        .unwrap();

        assert_eq!(synced_version, "npm:0.0.2");
        assert_eq!(
            read_curated_plugins_sha(tmp.path()).as_deref(),
            Some("npm:0.0.2")
        );
    }

    #[tokio::test]
    async fn sync_openai_plugins_repo_skips_github_archive_when_probe_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let github = MockServer::start().await;
        let mirror = MockServer::start().await;
        let tarball = npm_marketplace_tarball();
        let integrity = integrity_for_bytes(&tarball);

        Mock::given(method("GET"))
            .and(path("/repos/openagere/plugins"))
            .and(header("user-agent", HTTP_USER_AGENT))
            .respond_with(ResponseTemplate::new(503).set_body_string("github unavailable"))
            .mount(&github)
            .await;

        Mock::given(method("GET"))
            .and(path("/@openagere%2fplugins"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"{{
  "dist-tags": {{"latest": "0.0.3"}},
  "versions": {{
    "0.0.3": {{
      "dist": {{
        "tarball": "{}/@openagere/plugins/-/plugins-0.0.3.tgz",
        "integrity": "{}"
      }}
    }}
  }}
}}"#,
                mirror.uri(),
                integrity
            )))
            .mount(&mirror)
            .await;
        Mock::given(method("GET"))
            .and(path("/@openagere/plugins/-/plugins-0.0.3.tgz"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball))
            .mount(&mirror)
            .await;

        let synced_version = tokio::task::spawn_blocking({
            let agere_home = tmp.path().to_path_buf();
            let github_url = github.uri();
            let mirror_url = mirror.uri();
            move || {
                sync_openai_plugins_repo_with_npm_fallbacks(
                    agere_home.as_path(),
                    "missing-git-binary",
                    github_url.as_str(),
                    &[mirror_url.as_str()],
                )
            }
        })
        .await
        .unwrap()
        .unwrap();

        assert_eq!(synced_version, "npm:0.0.3");
        assert_eq!(
            read_curated_plugins_sha(tmp.path()).as_deref(),
            Some("npm:0.0.3")
        );
    }

    #[tokio::test]
    async fn sync_openai_plugins_repo_prefers_github_archive_before_npm() {
        let tmp = tempfile::tempdir().unwrap();
        let github = MockServer::start().await;
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let zipball = github_marketplace_zipball(sha);

        Mock::given(method("GET"))
            .and(path("/repos/openagere/plugins"))
            .and(header("user-agent", HTTP_USER_AGENT))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"default_branch":"main"}"#),
            )
            .mount(&github)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/openagere/plugins/git/ref/heads/main"))
            .and(header("user-agent", HTTP_USER_AGENT))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(format!(r#"{{"object":{{"sha":"{sha}"}}}}"#)),
            )
            .mount(&github)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/repos/openagere/plugins/zipball/{sha}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(zipball))
            .mount(&github)
            .await;

        let github_url = github.uri();
        let synced_version = tokio::task::spawn_blocking({
            let agere_home = tmp.path().to_path_buf();
            move || {
                sync_openai_plugins_repo_with_npm_fallback(
                    agere_home.as_path(),
                    "missing-git-binary",
                    github_url.as_str(),
                    "http://127.0.0.1:9",
                )
            }
        })
        .await
        .unwrap()
        .unwrap();

        assert_eq!(synced_version, sha);
        assert!(
            curated_plugins_repo_path(tmp.path())
                .join("plugins/github/.agere-plugin/plugin.json")
                .is_file()
        );
        assert_eq!(read_curated_plugins_sha(tmp.path()).as_deref(), Some(sha));
    }

    #[tokio::test]
    async fn sync_openai_plugins_repo_rejects_npm_integrity_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/@openagere%2fplugins"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"{{
  "dist-tags": {{"latest": "0.0.1"}},
  "versions": {{
    "0.0.1": {{
      "dist": {{
        "tarball": "{}/@openagere/plugins/-/plugins-0.0.1.tgz",
        "integrity": "sha512-deadbeef"
      }}
    }}
  }}
}}"#,
                server.uri()
            )))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/@openagere/plugins/-/plugins-0.0.1.tgz"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(npm_marketplace_tarball()))
            .mount(&server)
            .await;

        let registry_url = server.uri();
        let err = tokio::task::spawn_blocking({
            let agere_home = tmp.path().to_path_buf();
            move || {
                sync_openai_plugins_repo_with_npm_fallbacks(
                    agere_home.as_path(),
                    "missing-git-binary",
                    "http://127.0.0.1:9",
                    &[registry_url.as_str()],
                )
            }
        })
        .await
        .unwrap()
        .unwrap_err();

        assert!(
            err.contains("integrity"),
            "unexpected error without integrity context: {err}"
        );
    }

    #[tokio::test]
    async fn sync_openai_plugins_repo_rejects_npm_tarball_links() {
        let tmp = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        let tarball = npm_marketplace_tarball_with_symlink();
        let integrity = integrity_for_bytes(&tarball);

        mount_npm_package(&server, "0.0.1", &integrity).await;
        Mock::given(method("GET"))
            .and(path("/@openagere/plugins/-/plugins-0.0.1.tgz"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball))
            .mount(&server)
            .await;

        let registry_url = server.uri();
        let err = tokio::task::spawn_blocking({
            let agere_home = tmp.path().to_path_buf();
            move || {
                sync_openai_plugins_repo_with_npm_fallbacks(
                    agere_home.as_path(),
                    "missing-git-binary",
                    "http://127.0.0.1:9",
                    &[registry_url.as_str()],
                )
            }
        })
        .await
        .unwrap()
        .unwrap_err();

        assert!(err.contains("link"), "unexpected error: {err}");
    }

    #[tokio::test]
    async fn sync_openai_plugins_repo_cleans_staged_dir_on_npm_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        let tarball = npm_tarball_without_marketplace_manifest();
        let integrity = integrity_for_bytes(&tarball);

        mount_npm_package(&server, "0.0.1", &integrity).await;
        Mock::given(method("GET"))
            .and(path("/@openagere/plugins/-/plugins-0.0.1.tgz"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball))
            .mount(&server)
            .await;

        let registry_url = server.uri();
        let err = tokio::task::spawn_blocking({
            let agere_home = tmp.path().to_path_buf();
            move || {
                sync_openai_plugins_repo_with_npm_fallback(
                    agere_home.as_path(),
                    "missing-git-binary",
                    "http://127.0.0.1:9",
                    registry_url.as_str(),
                )
            }
        })
        .await
        .unwrap()
        .unwrap_err();

        assert!(
            err.contains("missing marketplace manifest"),
            "unexpected error: {err}"
        );
        assert_no_staged_plugin_dirs(tmp.path());
    }

    fn npm_marketplace_tarball() -> Vec<u8> {
        let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut archive = Builder::new(&mut gzip);
            append_tar_file(
                &mut archive,
                "package/.agents/plugins/marketplace.json",
                r#"{"name":"openagere-curated","plugins":[{"name":"gmail","source":{"source":"local","path":"./plugins/gmail"}}]}"#,
            );
            append_tar_file(
                &mut archive,
                "package/plugins/gmail/.agere-plugin/plugin.json",
                r#"{"name":"gmail"}"#,
            );
            archive.finish().unwrap();
        }
        gzip.finish().unwrap()
    }

    fn npm_marketplace_tarball_with_symlink() -> Vec<u8> {
        let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut archive = Builder::new(&mut gzip);
            append_tar_file(
                &mut archive,
                "package/.agents/plugins/marketplace.json",
                r#"{"name":"openagere-curated","plugins":[]}"#,
            );
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_mode(0o777);
            header.set_link_name("../../outside").unwrap();
            header.set_cksum();
            archive
                .append_data(&mut header, "package/plugins/link", std::io::empty())
                .unwrap();
            archive.finish().unwrap();
        }
        gzip.finish().unwrap()
    }

    fn npm_tarball_without_marketplace_manifest() -> Vec<u8> {
        let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut archive = Builder::new(&mut gzip);
            append_tar_file(
                &mut archive,
                "package/plugins/gmail/.agere-plugin/plugin.json",
                r#"{"name":"gmail"}"#,
            );
            archive.finish().unwrap();
        }
        gzip.finish().unwrap()
    }

    async fn mount_npm_package(server: &MockServer, version: &str, integrity: &str) {
        Mock::given(method("GET"))
            .and(path("/@openagere%2fplugins"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"{{
  "dist-tags": {{"latest": "{version}"}},
  "versions": {{
    "{version}": {{
      "dist": {{
        "tarball": "{}/@openagere/plugins/-/plugins-{version}.tgz",
        "integrity": "{}"
      }}
    }}
  }}
}}"#,
                server.uri(),
                integrity
            )))
            .mount(server)
            .await;
    }

    fn assert_no_staged_plugin_dirs(agere_home: &Path) {
        let tmp_root = agere_home.join(".tmp");
        if !tmp_root.exists() {
            return;
        }
        let leaked_dirs = std::fs::read_dir(&tmp_root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("plugins-npm-")
            })
            .collect::<Vec<_>>();
        assert!(
            leaked_dirs.is_empty(),
            "leaked staged dirs: {leaked_dirs:?}"
        );
    }

    fn github_marketplace_zipball(sha: &str) -> Vec<u8> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        let root = format!("plugins-{sha}");
        zip.start_file(format!("{root}/.agents/plugins/marketplace.json"), options)
            .unwrap();
        zip.write_all(
            br#"{"name":"openagere-curated","plugins":[{"name":"github","source":{"source":"local","path":"./plugins/github"}}]}"#,
        )
        .unwrap();
        zip.start_file(
            format!("{root}/plugins/github/.agere-plugin/plugin.json"),
            options,
        )
        .unwrap();
        zip.write_all(br#"{"name":"github"}"#).unwrap();
        zip.finish().unwrap().into_inner()
    }

    fn append_tar_file(archive: &mut Builder<&mut GzEncoder<Vec<u8>>>, path: &str, contents: &str) {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive
            .append_data(&mut header, path, contents.as_bytes())
            .unwrap();
    }

    fn integrity_for_bytes(bytes: &[u8]) -> String {
        use base64::Engine;
        let digest = sha2::Sha512::digest(bytes);
        format!("sha512-{}", base64::prelude::BASE64_STANDARD.encode(digest))
    }
}

fn sync_openai_plugins_repo_with_npm_fallback(
    agere_home: &Path,
    _git_binary: &str,
    github_api_base_url: &str,
    npm_registry_url: &str,
) -> Result<String, String> {
    let registry_urls = if npm_registry_url.trim_end_matches('/') == NPMMIRROR_REGISTRY_URL {
        vec![NPMMIRROR_REGISTRY_URL, DEFAULT_NPM_REGISTRY_URL]
    } else {
        vec![npm_registry_url, DEFAULT_NPM_REGISTRY_URL]
    };
    sync_openai_plugins_repo_with_npm_fallbacks(
        agere_home,
        _git_binary,
        github_api_base_url,
        registry_urls.as_slice(),
    )
}

fn sync_openai_plugins_repo_with_npm_fallbacks(
    agere_home: &Path,
    _git_binary: &str,
    github_api_base_url: &str,
    npm_registry_urls: &[&str],
) -> Result<String, String> {
    match probe_github_plugins_api(github_api_base_url) {
        Ok(()) => sync_openagere_plugins_via_github_archive(agere_home, github_api_base_url)
            .or_else(|github_err| {
                sync_openagere_plugins_via_npm_registries(agere_home, npm_registry_urls, github_err)
            }),
        Err(github_err) => {
            sync_openagere_plugins_via_npm_registries(agere_home, npm_registry_urls, github_err)
        }
    }
}

fn sync_openagere_plugins_via_npm_registries(
    agere_home: &Path,
    registry_urls: &[&str],
    github_err: String,
) -> Result<String, String> {
    let mut errors = vec![format!(
        "GitHub archive sync skipped or failed: {github_err}"
    )];
    for registry_url in registry_urls {
        match sync_openagere_plugins_via_npm(agere_home, registry_url) {
            Ok(version) => return Ok(version),
            Err(err) => errors.push(format!("npm registry {registry_url} failed: {err}")),
        }
    }
    Err(errors.join("\n"))
}

fn probe_github_plugins_api(api_base_url: &str) -> Result<(), String> {
    let repo_url = github_repo_url(api_base_url);
    fetch_text_with_timeout(
        &repo_url,
        "probe OpenAgere plugins GitHub API",
        GITHUB_PROBE_TIMEOUT,
    )
    .map(|_| ())
}

fn sync_openagere_plugins_via_github_archive(
    agere_home: &Path,
    api_base_url: &str,
) -> Result<String, String> {
    let remote_sha = fetch_github_remote_sha(api_base_url)?;
    let repo_path = curated_plugins_repo_path(agere_home);
    let sha_path = curated_plugins_sha_path(agere_home);
    if read_sha_file(&sha_path).as_deref() == Some(remote_sha.as_str())
        && repo_path.join(".agents/plugins/marketplace.json").is_file()
    {
        return Ok(remote_sha);
    }

    let zipball = fetch_github_zipball(api_base_url, &remote_sha)?;
    let staged_root = prepare_staged_curated_root(&repo_path)?;
    extract_github_zipball_to_dir(&zipball, staged_root.path())?;
    ensure_marketplace_manifest_exists(staged_root.path())?;
    activate_curated_root(&repo_path, staged_root)?;
    write_curated_plugins_sha(&sha_path, &remote_sha)?;
    Ok(remote_sha)
}

#[derive(Debug, Deserialize)]
struct GitHubRepositorySummary {
    default_branch: String,
}

#[derive(Debug, Deserialize)]
struct GitHubGitRefSummary {
    object: GitHubGitRefObject,
}

#[derive(Debug, Deserialize)]
struct GitHubGitRefObject {
    sha: String,
}

fn fetch_github_remote_sha(api_base_url: &str) -> Result<String, String> {
    let repo_url = github_repo_url(api_base_url);
    let repo_body = fetch_text_with_timeout(
        &repo_url,
        "get OpenAgere plugins repository",
        GITHUB_ARCHIVE_REQUEST_TIMEOUT,
    )?;
    let repo_summary: GitHubRepositorySummary = serde_json::from_str(&repo_body)
        .map_err(|err| format!("failed to parse OpenAgere plugins repository response: {err}"))?;
    let ref_url = format!("{repo_url}/git/ref/heads/{}", repo_summary.default_branch);
    let ref_body = fetch_text_with_timeout(
        &ref_url,
        "get OpenAgere plugins HEAD ref",
        GITHUB_ARCHIVE_REQUEST_TIMEOUT,
    )?;
    let git_ref: GitHubGitRefSummary = serde_json::from_str(&ref_body)
        .map_err(|err| format!("failed to parse OpenAgere plugins ref response: {err}"))?;
    if git_ref.object.sha.trim().is_empty() {
        Err("OpenAgere plugins ref response did not include a HEAD sha".to_string())
    } else {
        Ok(git_ref.object.sha)
    }
}

fn github_repo_url(api_base_url: &str) -> String {
    format!(
        "{}/repos/{OPENAGERE_PLUGINS_OWNER}/{OPENAGERE_PLUGINS_REPO}",
        api_base_url.trim_end_matches('/')
    )
}

fn fetch_github_zipball(api_base_url: &str, remote_sha: &str) -> Result<Vec<u8>, String> {
    let zipball_url = format!(
        "{}/repos/{OPENAGERE_PLUGINS_OWNER}/{OPENAGERE_PLUGINS_REPO}/zipball/{remote_sha}",
        api_base_url.trim_end_matches('/')
    );
    fetch_bytes_with_timeout(
        &zipball_url,
        "download OpenAgere plugins GitHub archive",
        GITHUB_ARCHIVE_REQUEST_TIMEOUT,
    )
}

fn sync_openagere_plugins_via_npm(agere_home: &Path, registry_url: &str) -> Result<String, String> {
    let package = resolve_npm_marketplace_package(registry_url, OPENAGERE_PLUGINS_NPM_PACKAGE)?;
    let version_marker = format!("npm:{}", package.version);
    let repo_path = curated_plugins_repo_path(agere_home);
    let sha_path = curated_plugins_sha_path(agere_home);
    if read_sha_file(&sha_path).as_deref() == Some(version_marker.as_str())
        && repo_path.join(".agents/plugins/marketplace.json").is_file()
    {
        return Ok(version_marker);
    }

    let tarball = fetch_bytes_with_timeout(
        &package.tarball_url,
        "download npm marketplace tarball",
        NPM_TARBALL_REQUEST_TIMEOUT,
    )?;
    if let Some(integrity) = &package.integrity {
        verify_npm_integrity(&tarball, integrity)?;
    }
    let staged_root = prepare_staged_curated_root(&repo_path)?;
    extract_npm_tarball_to_dir(&tarball, staged_root.path())?;
    ensure_marketplace_manifest_exists(staged_root.path())?;
    activate_curated_root(&repo_path, staged_root)?;
    write_curated_plugins_sha(&sha_path, &version_marker)?;
    Ok(version_marker)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedNpmMarketplacePackage {
    version: String,
    tarball_url: String,
    integrity: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NpmPackageMetadata {
    #[serde(rename = "dist-tags")]
    dist_tags: std::collections::HashMap<String, String>,
    versions: std::collections::HashMap<String, NpmPackageVersion>,
}

#[derive(Debug, Deserialize)]
struct NpmPackageVersion {
    dist: NpmPackageDist,
}

#[derive(Debug, Deserialize)]
struct NpmPackageDist {
    tarball: String,
    #[serde(default)]
    integrity: Option<String>,
}

fn resolve_npm_marketplace_package(
    registry_url: &str,
    package: &str,
) -> Result<ResolvedNpmMarketplacePackage, String> {
    let package_url = format!(
        "{}/{}",
        registry_url.trim_end_matches('/'),
        encode_npm_package_for_url(package)
    );
    let metadata = fetch_text_with_timeout(
        &package_url,
        "resolve npm marketplace package",
        NPM_METADATA_REQUEST_TIMEOUT,
    )?;
    let metadata: NpmPackageMetadata = serde_json::from_str(&metadata)
        .map_err(|err| format!("failed to parse npm marketplace metadata: {err}"))?;
    let version = metadata
        .dist_tags
        .get("latest")
        .filter(|version| !version.trim().is_empty())
        .ok_or_else(|| "npm marketplace metadata missing latest dist-tag".to_string())?
        .to_string();
    let package_version = metadata
        .versions
        .get(&version)
        .ok_or_else(|| format!("npm marketplace metadata missing version {version}"))?;
    if package_version.dist.tarball.trim().is_empty() {
        return Err(format!(
            "npm marketplace metadata version {version} missing tarball URL"
        ));
    }
    Ok(ResolvedNpmMarketplacePackage {
        version,
        tarball_url: package_version.dist.tarball.clone(),
        integrity: package_version.dist.integrity.clone(),
    })
}

fn verify_npm_integrity(bytes: &[u8], integrity: &str) -> Result<(), String> {
    let Some(expected) = integrity.strip_prefix("sha512-") else {
        return Err(format!(
            "unsupported npm marketplace integrity algorithm: {integrity}"
        ));
    };
    use base64::Engine;
    let expected = base64::prelude::BASE64_STANDARD
        .decode(expected)
        .map_err(|err| format!("failed to decode npm marketplace integrity: {err}"))?;
    let actual = sha2::Sha512::digest(bytes);
    if expected.as_slice() == actual.as_slice() {
        Ok(())
    } else {
        Err("npm marketplace tarball integrity mismatch".to_string())
    }
}

fn encode_npm_package_for_url(package: &str) -> String {
    package.replacen('/', "%2f", 1)
}

fn fetch_text_with_timeout(url: &str, context: &str, timeout: Duration) -> Result<String, String> {
    let response = http_client(timeout)
        .get(url)
        .send()
        .map_err(|err| format!("failed to {context} from {url}: {err}"))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|err| format!("failed to read {context} response from {url}: {err}"))?;
    if !status.is_success() {
        return Err(format!(
            "{context} from {url} failed with status {status}: {body}"
        ));
    }
    Ok(body)
}

fn fetch_bytes_with_timeout(
    url: &str,
    context: &str,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    let response = http_client(timeout)
        .get(url)
        .send()
        .map_err(|err| format!("failed to {context} from {url}: {err}"))?;
    let status = response.status();
    let body = response
        .bytes()
        .map_err(|err| format!("failed to read {context} response from {url}: {err}"))?;
    if !status.is_success() {
        return Err(format!(
            "{context} from {url} failed with status {status}: {}",
            String::from_utf8_lossy(&body)
        ));
    }
    Ok(body.to_vec())
}

fn http_client(timeout: Duration) -> Client {
    Client::builder()
        .timeout(timeout)
        .user_agent(HTTP_USER_AGENT)
        .build()
        .unwrap_or_else(|err| panic!("failed to build npm marketplace HTTP client: {err}"))
}

fn prepare_staged_curated_root(repo_path: &Path) -> Result<TempDir, String> {
    let Some(parent) = repo_path.parent() else {
        return Err(format!(
            "failed to determine curated plugins parent directory for {}",
            repo_path.display()
        ));
    };
    fs::create_dir_all(parent).map_err(|err| {
        format!(
            "failed to create curated plugins parent directory {}: {err}",
            parent.display()
        )
    })?;
    tempfile::Builder::new()
        .prefix("plugins-npm-")
        .tempdir_in(parent)
        .map_err(|err| format!("failed to create temporary curated plugins directory: {err}"))
}

fn extract_npm_tarball_to_dir(bytes: &[u8], destination: &Path) -> Result<(), String> {
    let decoder = GzDecoder::new(bytes);
    let mut archive = Archive::new(decoder);
    for entry in archive
        .entries()
        .map_err(|err| format!("failed to read npm marketplace tarball: {err}"))?
    {
        let mut entry = entry.map_err(|err| format!("failed to read npm tarball entry: {err}"))?;
        let path = entry
            .path()
            .map_err(|err| format!("failed to read npm tarball entry path: {err}"))?;
        let relative_path = npm_package_relative_path(&path)?;
        if relative_path.as_os_str().is_empty() {
            continue;
        }
        reject_npm_tarball_link_entry(entry.header().entry_type(), &path)?;
        let output_path = destination.join(relative_path);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "failed to create npm marketplace directory {}: {err}",
                    parent.display()
                )
            })?;
        }
        entry.unpack(&output_path).map_err(|err| {
            format!(
                "failed to extract npm marketplace entry {}: {err}",
                output_path.display()
            )
        })?;
    }
    Ok(())
}

fn reject_npm_tarball_link_entry(entry_type: EntryType, path: &Path) -> Result<(), String> {
    if entry_type.is_symlink() || entry_type.is_hard_link() {
        Err(format!(
            "npm marketplace tarball link entry is not allowed: {}",
            path.display()
        ))
    } else {
        Ok(())
    }
}

fn extract_github_zipball_to_dir(bytes: &[u8], destination: &Path) -> Result<(), String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|err| format!("failed to open OpenAgere plugins zip archive: {err}"))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| format!("failed to read OpenAgere plugins zip entry: {err}"))?;
        let Some(relative_path) = entry.enclosed_name() else {
            return Err(format!(
                "OpenAgere plugins zip entry `{}` escapes extraction root",
                entry.name()
            ));
        };
        let mut components = relative_path.components();
        let Some(Component::Normal(_)) = components.next() else {
            continue;
        };
        let output_relative = components.fold(PathBuf::new(), |mut path, component| {
            if let Component::Normal(segment) = component {
                path.push(segment);
            }
            path
        });
        if output_relative.as_os_str().is_empty() {
            continue;
        }
        let output_path = destination.join(output_relative);
        if entry.is_dir() {
            fs::create_dir_all(&output_path).map_err(|err| {
                format!(
                    "failed to create OpenAgere plugins directory {}: {err}",
                    output_path.display()
                )
            })?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "failed to create OpenAgere plugins directory {}: {err}",
                    parent.display()
                )
            })?;
        }
        let mut output = fs::File::create(&output_path).map_err(|err| {
            format!(
                "failed to create OpenAgere plugins file {}: {err}",
                output_path.display()
            )
        })?;
        std::io::copy(&mut entry, &mut output).map_err(|err| {
            format!(
                "failed to write OpenAgere plugins file {}: {err}",
                output_path.display()
            )
        })?;
    }
    Ok(())
}

fn npm_package_relative_path(path: &Path) -> Result<PathBuf, String> {
    let mut components = path.components();
    match components.next() {
        Some(Component::Normal(component)) if component == "package" => {}
        _ => return Ok(PathBuf::new()),
    }
    let mut relative_path = PathBuf::new();
    for component in components {
        match component {
            Component::Normal(segment) => relative_path.push(segment),
            _ => {
                return Err(format!(
                    "npm marketplace tarball entry escapes package root: {}",
                    path.display()
                ));
            }
        }
    }
    Ok(relative_path)
}

fn ensure_marketplace_manifest_exists(repo_path: &Path) -> Result<(), String> {
    if repo_path.join(".agents/plugins/marketplace.json").is_file() {
        Ok(())
    } else {
        Err(format!(
            "curated plugins archive missing marketplace manifest at {}",
            repo_path.join(".agents/plugins/marketplace.json").display()
        ))
    }
}

fn activate_curated_root(repo_path: &Path, staged_root: TempDir) -> Result<(), String> {
    if repo_path.exists() {
        fs::remove_dir_all(repo_path).map_err(|err| {
            format!(
                "failed to remove previous curated plugins root {}: {err}",
                repo_path.display()
            )
        })?;
    }
    let staged_path = staged_root.keep();
    fs::rename(&staged_path, repo_path).map_err(|err| {
        format!(
            "failed to activate curated plugins root {}: {err}",
            repo_path.display()
        )
    })
}

fn write_curated_plugins_sha(sha_path: &Path, version_marker: &str) -> Result<(), String> {
    if let Some(parent) = sha_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create curated plugins sha directory {}: {err}",
                parent.display()
            )
        })?;
    }
    fs::write(sha_path, format!("{version_marker}\n")).map_err(|err| {
        format!(
            "failed to write curated plugins sha file {}: {err}",
            sha_path.display()
        )
    })
}
