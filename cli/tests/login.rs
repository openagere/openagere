use std::path::Path;

use anyhow::Result;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tempfile::TempDir;

fn agere_command(agere_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(agere_utils_cargo_bin::cargo_bin("openagere")?);
    cmd.env("AGERE_HOME", agere_home);
    Ok(cmd)
}

fn write_file_auth_config(agere_home: &Path) -> Result<()> {
    std::fs::write(
        agere_home.join("config.toml"),
        "cli_auth_credentials_store = \"file\"\n",
    )?;
    Ok(())
}

fn read_auth_json(agere_home: &Path) -> Result<Value> {
    let auth_json = std::fs::read_to_string(agere_home.join("auth.json"))?;
    Ok(serde_json::from_str(&auth_json)?)
}

#[test]
fn login_with_api_key_reads_stdin_and_writes_auth_json() -> Result<()> {
    let agere_home = TempDir::new()?;
    write_file_auth_config(agere_home.path())?;

    let mut cmd = agere_command(agere_home.path())?;
    cmd.args([
        "-c",
        "forced_login_method=\"api\"",
        "login",
        "--with-api-key",
    ])
    .write_stdin("sk-test\n")
    .assert()
    .success()
    .stderr(contains("Successfully logged in"));

    let auth = read_auth_json(agere_home.path())?;
    assert_eq!(auth["OPENAI_API_KEY"], "sk-test");
    assert!(auth.get("tokens").is_none());
    assert!(auth.get("agent_identity").is_none());

    Ok(())
}

#[test]
fn login_with_agent_identity_rejects_invalid_jwt() -> Result<()> {
    let agere_home = TempDir::new()?;
    write_file_auth_config(agere_home.path())?;

    let mut cmd = agere_command(agere_home.path())?;
    cmd.args(["login", "--with-agent-identity"])
        .write_stdin("not-a-jwt\n")
        .assert()
        .failure()
        .stderr(contains("Error logging in with Agent Identity"));

    Ok(())
}
