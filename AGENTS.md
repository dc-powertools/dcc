## Instructions for agentic coding tools

Several files contain crucial context for this project.
- `README.md`: project overview
- `readme/DEVELOPMENT.md`: development guidelines and workflows
- `readme/STYLE.md`: coding style
- `readme/ARCHITECTURE.md`: high-level architecture for the entire project

Read all of these files before working on this project.

The development guide defines mandatory verification and commit policies that you
must always follow. It is imported here so it is always loaded into context:

@readme/DEVELOPMENT.md
@readme/STYLE.md
@readme/ARCHITECTURE.md

Ignore all directories in `.gitignore`.


# Specific Agent Instructions

## Codex

When asking the user a question, always wait for the answer. NEVER use
auto-resolving questions or default-assumption timeouts. If using
`request_user_input`, omit `autoResolutionMs`.
