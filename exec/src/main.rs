//! Entry-point for the `agere-exec` binary (shared with `agere-linux-helper`).
fn main() -> anyhow::Result<()> {
    agere_exec::run_cli_package_main()
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
