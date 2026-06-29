# AGENTS.md

## Build & Test

```bash
cargo test                          # all tests (unit + integration)
cargo test -p jvm                   # same, by package name
cargo test --lib                    # unit tests only (src/ inline)
cargo test --test integration       # integration tests only (tests/)
cargo build --release               # release build
```

No formatter/linter config exists — use plain `cargo fmt` / `cargo clippy`.

## Pre-Commit Checklist

Run these in order before committing — all must pass:

```bash
cargo fmt --all -- --check    # format check
cargo clippy --all-targets    # lint check
cargo test                    # all tests
```

To auto-fix formatting: `cargo fmt --all`, then re-check.

## Test Conventions

- Tests that set or remove the `JVM_DIR` env var **must** be annotated `#[serial]` (from the `serial_test` crate). See `src/config.rs:215`, `src/switch.rs:182`, `src/dirs.rs:86`.
- Integration tests (`tests/integration.rs`) use `tempfile::TempDir` + `JVM_DIR` for isolation and `assert_cmd::CommandCargoExt` to run the binary. The `create_fake_jdk()` helper at `tests/integration.rs:6` creates a minimal JDK tree with `bin/java` and `release` file.
- Unit tests in `src/` are internal `#[cfg(test)] mod tests` blocks — the preferred pattern.

## Architecture

Single binary crate. Entrypoint: `src/main.rs` — clap derive CLI dispatches to `cmd_*` functions.

| Module | Purpose |
|---|---|
| `main.rs` | CLI definition + command dispatch |
| `config.rs` | `config.json` CRUD, atomic save (write `.tmp` then rename) |
| `jdk.rs` | Version detection (`release` file → `java -version` fallback), alias generation |
| `switch.rs` | `jvm use`: atomic symlink replacement with rollback on config write failure |
| `install.rs` | Download/extract/register JDKs from Temurin (Adoptium API v3); proxy support via `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY` env vars or `--proxy` flag |
| `dirs.rs` | XDG-based path resolution: config, runtime, managed JDKs. `$JVM_DIR` overrides everything |
| `init.rs` | Shell hook generator (bash/zsh/fish/powershell) |
| `completion.rs` | Shell completion via `clap_complete` |

## Key Implementation Details

- **Atomic config save**: writes to `config.json.tmp` then renames to `config.json` (`src/config.rs:51-54`)
- **Atomic switch**: creates symlink first → updates config → rolls back symlink on config write failure (`src/switch.rs:76-104`)
- **Windows symlink fallback**: if `symlink_dir` fails with error 1314 (privilege), falls back to `junction::create` (`src/switch.rs:35-45`)
- **Path normalization on Windows**: `\\?\` verbatim prefix stripped in both `main.rs:168-175` (add) and `dirs.rs:72-80` (display)
- **JDK version detection**: tries `release` file first, then `java -version` stderr parsing (`src/jdk.rs:52-78`)
- **Only Temurin** is supported as a distribution source for `jvm install` (`src/install.rs:15-18`)
- **`jvm remove`** unregisters (no file deletion); **`jvm uninstall`** deletes managed JDK files + config entry

## Config / State

All state lives in `$JVM_DIR` (defaults follow XDG):
- `config.json` — registered JDKs, aliases, current version
- `current` — symlink to active JDK (shell hooks read this)
- `managed/` — JDKs installed via `jvm install`

Override all paths with `JVM_DIR` env var.
