use anyhow::Result;
use predicates::str::contains;
use std::path::Path;
use tempfile::TempDir;

fn agere_command(agere_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(agere_utils_cargo_bin::cargo_bin("openagere")?);
    cmd.env("AGERE_HOME", agere_home);
    Ok(cmd)
}

#[cfg(debug_assertions)]
#[tokio::test]
async fn update_does_not_start_interactive_prompt() -> Result<()> {
    let agere_home = TempDir::new()?;

    agere_command(agere_home.path())?
        .arg("update")
        .assert()
        .failure()
        .stderr(contains("`agere update` is not available in debug builds"));

    Ok(())
}
