# Skills

Skills extend OpenAgere with specialized knowledge, workflows, or tool integrations. Each skill is defined in a `SKILL.md` file and can be hot-reloaded at runtime.

## How skills work

A skill is a directory containing at minimum a `SKILL.md` file. The frontmatter defines the skill name, description, and trigger conditions. When OpenAgere encounters a task that matches a skill description, it loads the skill instructions into the agent context.

Skills are stored under `$AGERE_HOME/skills/` and are watched for changes.

## Built-in skills

OpenAgere ships with core skills for common workflows:

- **plugin-creator** — Scaffold plugin directories
- **skill-creator** — Create and update custom skills
- **skill-installer** — Install skills from curated lists or GitHub repos
- **imagegen** — Generate or edit raster images

## Installing skills

```shell
openagere skill install <skill-name>
```

## Creating a skill

1. Create a directory under `$AGERE_HOME/skills/<skill-name>/`
2. Add a `SKILL.md` with frontmatter and instructions
3. Optionally add `scripts/`, `references/`, or `assets/` subdirectories

## Skill precedence

Skills in `$AGERE_HOME/skills/` take precedence over built-in skills.
