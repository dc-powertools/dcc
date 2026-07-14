# Agentic Development Entrypoint

This repository uses the portable AI development framework in `readme/meta/`.

Every primary harness session and every delegated agent must read this file and then
[readme/meta/README.md](readme/meta/README.md) before doing task work. If
`readme/README.md` exists, every agent also reads that bounded project cursor before
following only the process and project documents relevant to its assignment.

If `readme/README.md` is absent or is not a `# Project State` cursor, or
`readme/tasks/README.md` is absent or is not a `# Task Catalog`, the framework has not
been fully onboarded for this project. The primary session follows the bootstrap or
collision path in the meta README and
[onboarding procedure](readme/meta/onboarding.md). A delegated agent reports the issue
to its orchestrator and does not initialize shared project documentation unless that
ownership was assigned explicitly.

