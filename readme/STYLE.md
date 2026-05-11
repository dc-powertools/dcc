# Rust Style Guide

Guidance for writing safe, idiomatic, readable Rust in this codebase.

---

## Errors

Use `anyhow::Result<T>` throughout. Add `.with_context(|| format!("..."))?` at every fallible boundary so errors carry enough context to diagnose without a debugger:

```rust
fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
```

Never swallow errors silently. Never convert a `Result` to `Option` unless absence genuinely means "not found" and failure is impossible.

When developing a library crate alongside a binary crate, the library layer should define a typed error enum with `thiserror::Error` — using `#[from]` for transparent wrapping and `#[source]` for error chains. The binary layer stays on `anyhow` and converts at the boundary. Typed errors belong in libraries because callers can match on them; `anyhow` belongs in binaries because only humans read the output.

## Ownership and Borrowing

Prefer owned types (`String`, `Vec<T>`, `PathBuf`) for values that are stored or returned. Borrow (`&str`, `&[T]`, `&Path`) for read-only inputs that are consumed immediately.

Never hold a `std::sync::Mutex` guard across `.await`. Either drop the guard before the await point, or use `tokio::sync::Mutex`.

## Visibility

Make everything private by default. Widen visibility only when there is a concrete reason, and treat `pub` as a commitment: public items are part of the contract and cannot be freely changed without considering callers.

This applies at every level — fields, methods, types, modules. A private field can be renamed, split, or removed without touching anything else. A `pub` field cannot. Keeping implementation details private is what makes large-scale refactoring possible.

Prefer `pub(crate)` over `pub` for cross-module implementation details that have no reason to be part of a public API.

## Newtype Pattern

Wrap primitives in named structs when a value has a specific domain meaning. A `String` holding a container name and a `String` holding an image reference are the same type to the compiler but different concepts to the domain — confusing them is a logic error the type system can catch for free:

```rust
struct ContainerName(String);
struct ImageRef(String);
```

Newtypes also give associated methods a natural home and make function signatures self-documenting. Use them wherever a primitive is used repeatedly with a specific role, and where mixing up two values of that primitive type would be a bug.

## Traits and Dynamic Dispatch

Default to generics (`fn foo<T: Trait>(x: T)` or `fn foo(x: impl Trait)`): they resolve at compile time, have zero runtime overhead, and allow inlining. Use `&dyn Trait` only when runtime type erasure is genuinely required — heterogeneous collections, plugin systems, or places where the concrete type cannot be known at compile time.

`dyn` trades static resolution for a vtable lookup per call and prevents the compiler from optimizing across call boundaries. Not all traits are object-safe. When in doubt, start with generics — introducing `dyn` later is straightforward; removing it requires changing signatures.

## Unsafe

Treat `unsafe` as off-limits by default. Every `unsafe` block requires a `// SAFETY:` comment immediately above it explaining which invariants hold and why.

## Concurrency

Always keep the `JoinHandle` returned by `tokio::spawn`. Await it or abort it on shutdown — fire-and-forget tasks hide failures and leak resources. Long-running loops must check for cancellation via `tokio::select!` against a shutdown signal.

Never call blocking I/O or long CPU-bound work directly inside an async function. Tokio's scheduler is cooperative — a task that doesn't yield stalls every other task on that thread. Offload blocking work with `tokio::task::spawn_blocking`; use `tokio::task::block_in_place` only when you are already inside a `spawn_blocking` context and need to call back into async code.

## Logging

Consolidate output through `tracing` (`tracing::info!`, `tracing::warn!`, `#[tracing::instrument]`) rather than sprinkling `eprintln!` everywhere.

## Panics

No `unwrap()` or `expect()` outside `#[cfg(test)]`. Use `?` or explicit error handling. The only exception: `expect("...")` where the message documents a compile-time or initialization-time invariant that the type system can't express.

No `todo!()`, `unimplemented!()`, or `panic!()` on any code path reachable from a running binary.

Prefer `.get(i)` (returning `Option`) over direct indexing (`slice[i]`) whenever the index originates from external input.

## Naming

| Case | Used for |
|------|----------|
| `snake_case` | modules, functions, variables, fields |
| `UpperCamelCase` | types, traits, enum variants |
| `SCREAMING_SNAKE_CASE` | `const`, `static` |

No Hungarian notation. No abbreviations beyond common ones (`url`, `id`, `cmd`, `cfg`, `err`).

## Module Organization

Organize by feature, not by type. Put the types, logic, and tests for a feature together in one module rather than scattering them across `models/`, `handlers/`, `utils/`. A module should be easy to delete: if removing a feature means touching files across many directories, the structure is fighting the work.

Keep the module hierarchy shallow — two levels is almost always enough. Use `mod.rs` only for modules with multiple subfiles; a single-file module is just `foo.rs`.

## Formatting and Lints

Run `cargo fmt` before committing — CI rejects unformatted code.

Run `cargo clippy -- -D warnings` before committing. Fix warnings rather than suppressing them. If suppression is genuinely necessary, add a `#[allow(...)]` with a comment explaining why.

## Testing

Unit tests live in a `#[cfg(test)] mod tests { ... }` block inside the same file. This gives access to private items — use it to test internal logic directly without going through the public surface.

Use `#[tokio::test]` for async tests. Tests must exercise real behavior: don't mock away the logic under test just to make assertions trivially pass.

Integration tests belong in a top-level `tests/` directory, where each file is a separate crate that drives the binary through its public interface (via `std::process::Command` or the public API surface). These catch regressions that unit tests miss, particularly around argument parsing, exit codes, and cross-module interactions.

For modules with non-trivial input spaces — the devcontainer.json parser, Dockerfile generation, shell-quoting — add property-based tests with `proptest`. Fuzz the input rather than hand-picking examples: `proptest` finds edge cases that example tests don't.

## Minimize Dependencies with Features

Always minimize project complexity by explicitly setting the `[features]` property on dependencies listed in `Cargo.toml`. Only include features that are required for the project.

## Security

Never log secrets, tokens, or credentials. Redact at the point of output — don't rely on downstream filtering.

Integer arithmetic on untrusted input must use `checked_*`, `saturating_*`, or `wrapping_*` explicitly. Debug-mode overflow panics are not a safety mechanism — overflow checks are silently disabled in release builds by default. If overflow anywhere in the binary would be a logic error, enable them globally by adding `overflow-checks = true` under `[profile.release]` in `Cargo.toml`.
