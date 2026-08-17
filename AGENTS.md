# Agentic Development Entrypoint

This repository uses the portable AI development framework in `.meta/meta/`.

Every primary harness session and every delegated agent must read this file and then
[.meta/meta/README.md](.meta/meta/README.md) before doing task work. If
`.meta/README.md` exists, every agent also reads that bounded project cursor before
following only the process and project documents relevant to its assignment.

If `.meta/README.md` is absent or is not a `# Project State` cursor, or
`.meta/tasks/README.md` is absent or is not a `# Task Catalog`, the framework has not
been fully onboarded for this project. The primary session follows the bootstrap or
collision path in the meta README and
[onboarding procedure](.meta/meta/onboarding.md). A delegated agent reports the issue
to its orchestrator and does not initialize shared project documentation unless that
ownership was assigned explicitly.

