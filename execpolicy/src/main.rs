use agere_execpolicy::ExecPolicyCheckCommand;
use anyhow::Result;
use clap::Parser;

/// CLI for evaluating exec policies
#[derive(Parser)]
#[command(name = "agere-execpolicy")]
enum Cli {
    /// Evaluate a command against a policy.
    Check(ExecPolicyCheckCommand),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli {
        Cli::Check(cmd) => cmd.run(),
    }
}
