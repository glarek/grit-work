# GRIT Work development

When orchestrating agents for Adrian, use the **currently running Herdr instance**, the same workspace and visible sibling panes. Read `~/.codex/skills/herdr-orchestration/SKILL.md` and `herdr --skill`. Never substitute hidden subagents or detached CLI jobs. Preserve worktrees when handing tasks over.

Never place Spiris credentials in source, prompts, test fixtures, logs or shell arguments. Production credentials belong in the OS keyring. Real time submissions require the application's explicit report approval; integration smoke tests are read-only.

Local evidence must never be automatically deleted until its complete day archive has passed readback and checksum verification. Keep failed or ambiguous uploads locked against automatic retries. Validate project IDs and membership against Spiris immediately before posting.
