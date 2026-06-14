//! argv0 alias binary (`agere-linux-helper`) sharing the `agere-exec` implementation.

fn main() -> anyhow::Result<()> {
    agere_exec::run_cli_package_main()
}
