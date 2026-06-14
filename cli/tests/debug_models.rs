use std::path::Path;

use anyhow::Result;
use tempfile::TempDir;

fn agere_command(agere_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(agere_utils_cargo_bin::cargo_bin("openagere")?);
    cmd.env("AGERE_HOME", agere_home);
    Ok(cmd)
}

#[test]
fn debug_models_bundled_prints_json() -> Result<()> {
    let agere_home = TempDir::new()?;
    let mut cmd = agere_command(agere_home.path())?;
    let output = cmd.args(["debug", "models", "--bundled"]).output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    let value: serde_json::Value = serde_json::from_str(&stdout)?;
    assert!(value["models"].is_array());
    assert!(!value["models"].as_array().unwrap_or(&Vec::new()).is_empty());

    Ok(())
}

#[test]
fn debug_models_default_prints_json_without_auth() -> Result<()> {
    let agere_home = TempDir::new()?;
    let mut cmd = agere_command(agere_home.path())?;
    let output = cmd.args(["debug", "models"]).output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    let value: serde_json::Value = serde_json::from_str(&stdout)?;
    assert!(value["models"].is_array());
    assert!(!value["models"].as_array().unwrap_or(&Vec::new()).is_empty());

    Ok(())
}
