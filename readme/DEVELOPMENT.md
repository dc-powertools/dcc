# Development Guide

Instructions for developing this Rust project.

---

## Before Writing Code

Read the relevant source files before making any changes. Look for existing types, utilities, and patterns that already address the requirement — don't duplicate what's there. Understand the full scope of the change: a function signature change touches every caller, a new error variant may need to propagate through several layers, a new dependency has compile-time and binary-size costs.

If the requirement is ambiguous, ask before implementing. The cost of a wrong assumption grows with every line written on top of it.

## The Development Loop

Work in small, verifiable steps. After each meaningful change:

```bash
cargo check        # fast feedback on types and borrows
cargo test         # full suite, not just the affected module
```

Fix errors and warnings before moving forward. Don't accumulate a pile of problems to untangle at the end.

## Before Finishing

Run all three checks and confirm they pass cleanly:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Then read the full diff critically — not to confirm that changes were made, but to evaluate whether they are correct and complete:

- **Fully satisfies the requirement.** Reread the original request. Does the implementation actually do what was asked, including edge cases? Compilation and passing tests are necessary but not sufficient.
- **No leftover scope.** Every changed line should serve the requirement. Remove anything that doesn't — incidental refactors, renamed variables that weren't mentioned, reformatted blocks that weren't touched for functional reasons.
- **All callers updated.** A function signature change that compiles but leaves call sites with subtly wrong semantics is a latent bug. Search for every call site.
- **Errors carry context.** Every `?` at a meaningful boundary should have `.with_context(|| ...)`. An error message that reaches the user should be diagnosable without access to the source code.
- **Tests exercise real behavior.** A test that cannot fail is not a test. Assert on concrete outputs; test error paths, not just the happy path.

## Scope

Do exactly what was asked. Don't silently fix nearby issues, rename things that weren't mentioned, or add features that weren't requested. If you notice a real problem in adjacent code, surface it as an observation — don't fold it into the change without agreement.

## Common Pitfalls

**Don't suppress warnings.** `#[allow(clippy::...)]` is almost never the right answer. Fix the underlying issue. If suppression is genuinely warranted, add a comment explaining why — not just what.

**Don't leave stubs.** `todo!()`, `unimplemented!()`, and `// TODO` comments in submitted code mean the work is incomplete. Either implement it or explicitly say it's out of scope.

**Don't paper over errors.** An `.unwrap()` that "should never fail" will, under conditions you didn't anticipate. Use `?` and propagate errors with context.

**Don't add dependencies without justification.** If the standard library or an existing dependency covers the need, use it. A new `Cargo.toml` entry has real costs: compile time, binary size, and supply chain exposure.

**Don't mistake structure for behavior.** Code that is well-organized and compiles cleanly can still be wrong. The final check is always: does it do what was asked?
