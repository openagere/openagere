use agere_exec::TopCli;
use clap::Parser;
use pretty_assertions::assert_eq;

#[test]
fn top_cli_parses_resume_prompt_after_config_flag() {
    const PROMPT: &str = "echo resume-with-global-flags-after-subcommand";
    let cli = TopCli::parse_from([
        "agere-exec",
        "resume",
        "--last",
        "--json",
        "--model",
        "gpt-5.3",
        "--config",
        "reasoning_level=xhigh",
        "--skip-git-repo-check",
        PROMPT,
    ]);

    let Some(agere_exec::Command::Resume(args)) = cli.inner.command else {
        panic!("expected resume command");
    };
    let effective_prompt = args.prompt.clone().or_else(|| {
        if args.last {
            args.session_id.clone()
        } else {
            None
        }
    });
    assert_eq!(effective_prompt.as_deref(), Some(PROMPT));
    assert_eq!(cli.config_overrides.raw_overrides.len(), 1);
    assert_eq!(
        cli.config_overrides.raw_overrides[0],
        "reasoning_level=xhigh"
    );
}
