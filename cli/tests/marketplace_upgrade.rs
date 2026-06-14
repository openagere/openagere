use anyhow::Result;
use predicates::str::contains;
use std::path::Path;
use tempfile::TempDir;

fn agere_command(agere_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(agere_utils_cargo_bin::cargo_bin("openagere")?);
    cmd.env("AGERE_HOME", agere_home);
    Ok(cmd)
}

#[tokio::test]
async fn marketplace_upgrade_runs_under_plugin() -> Result<()> {
    let agere_home = TempDir::new()?;

    agere_command(agere_home.path())?
        .args(["plugin", "marketplace", "upgrade"])
        .assert()
        .success()
        .stdout(contains("No configured Git marketplaces to upgrade."));

    Ok(())
}

#[tokio::test]
async fn marketplace_upgrade_no_longer_runs_at_top_level() -> Result<()> {
    let agere_home = TempDir::new()?;

    agere_command(agere_home.path())?
        .args(["marketplace", "upgrade"])
        .assert()
        .failure()
        .stderr(contains("unrecognized subcommand 'upgrade'"));

    Ok(())
}
