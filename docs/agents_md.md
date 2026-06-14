# AGENTS.md

AGENTS.md files provide repository-level instructions for AI coding agents like OpenAgere. They tell the agent about coding conventions, project structure, and workflow preferences.

## How AGENTS.md works

- AGENTS.md files can appear anywhere within a repository
- The scope of an AGENTS.md file is the entire directory tree rooted at the folder that contains it
- OpenAgere reads all AGENTS.md files from the CWD up to the repo root
- More-deeply-nested AGENTS.md files take precedence when instructions conflict
- Instructions about code style, structure, naming, etc. apply only to code within the AGENTS.md file scope, unless the file states otherwise
- Direct system/developer/user instructions (as part of a prompt) take precedence over AGENTS.md instructions

## Creating an AGENTS.md

Run `/init` in the TUI to generate an AGENTS.md for your repository:

```
/init
```

Or create one manually at the root of your project:

```markdown
# Project conventions

- Use 4-space indentation
- Prefer async/await over raw promises
- Run `npm test` before committing
```

## Viewing current instructions

Use `/agents` in the TUI to see which AGENTS.md files OpenAgere has loaded for the current session.

## Hierarchical agents message

When the `child_agents_md` feature flag is enabled (via `[features]` in `config.toml`), OpenAgere appends additional guidance about AGENTS.md scope and precedence to the user instructions message, and emits that message even when no AGENTS.md is present.

```toml
# ~/.openagere/config.toml
[features]
child_agents_md = true
```

## External documentation

For the full AGENTS.md specification, see the [upstream documentation](https://developers.openagere.com/agere/guides/agents-md).
