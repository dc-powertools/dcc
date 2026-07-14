# Glossary

| Term | Meaning | Use Instead Of | Source | Last Checked |
| --- | --- | --- | --- | --- |
| dcc | The Dev Container CLI binary produced by this repository. | Devcontainer wrapper, CLI tool when ambiguous | `README.md`; `Cargo.toml` | 2026-07-14 |
| Profile | A named devcontainer configuration loaded from `.devcontainer/<profile>.json`; the default profile is `devcontainer`. | Environment name | `README.md`; `src/profile.rs` | 2026-07-14 |
| Durable cache | Per-profile host cache under `.dcc/<profile>`, mounted into containers at `/cache`. | Shared cache | `README.md`; `src/cache.rs` | 2026-07-14 |
| `extends` | Local devcontainer config inheritance property resolved relative to the child config file. | Include, import | `README.md`; `readme/ARCHITECTURE.md`; `src/config/resolve.rs` | 2026-07-14 |
| `containerEnv` | Build-time environment baked into the image as Docker `ENV` directives. | Runtime env | `README.md`; `readme/ARCHITECTURE.md` | 2026-07-14 |
| `remoteEnv` | Runtime environment passed to `docker run` as `-e` flags and re-evaluated on each run. | Image env | `README.md`; `readme/ARCHITECTURE.md` | 2026-07-14 |
