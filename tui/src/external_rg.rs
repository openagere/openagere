use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use agere_install_context::InstallContext;

/// A single match result from ripgrep.
#[derive(Debug, Clone)]
pub(crate) struct RgMatch {
    pub(crate) path: String,
    pub(crate) line_number: usize,
    pub(crate) line: String,
}

/// Execute an external ripgrep search and return parsed results.
pub(crate) fn run_external_rg(pattern: &str, search_path: &str) -> Result<Vec<RgMatch>, String> {
    let rg_path = InstallContext::current().rg_command();
    if !rg_path_is_available(&rg_path) {
        return Err("rg command not found".to_string());
    }

    let mut cmd = Command::new(&rg_path);
    cmd.arg("--json")
        .arg("--line-number")
        .arg("-m")
        .arg("50")
        .arg(pattern)
        .arg(search_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn rg: {e}"))?;

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for rg: {e}"))?;

    // Exit code 1 means no matches found — return empty vec, not error
    if !output.status.success() && output.status.code() != Some(1) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_rg_json(&stdout)
}

fn rg_path_is_available(path: &PathBuf) -> bool {
    if path.is_absolute() {
        return path.exists();
    }
    which::which(path.to_string_lossy().as_ref()).is_ok()
}

/// Parse ripgrep JSON output into match results.
fn parse_rg_json(stdout: &str) -> Result<Vec<RgMatch>, String> {
    let mut results = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };

        let Some(obj) = value.as_object() else {
            continue;
        };
        let Some(typ) = obj.get("type").and_then(|v| v.as_str()) else {
            continue;
        };
        if typ != "match" {
            continue;
        }

        let data = obj.get("data").and_then(|v| v.as_object());
        let Some(data) = data else { continue };

        let path = data
            .get("path")
            .and_then(|v| v.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let line_number = data
            .get("line_number")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;

        let matched_line = data
            .get("lines")
            .and_then(|v| v.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if !path.is_empty() && line_number > 0 {
            results.push(RgMatch {
                path,
                line_number,
                line: matched_line,
            });
        }
    }

    Ok(results)
}
