---
name: melodias-testing
description: Test layout and verification for melodias-studio, covering the inline src/tests modules, the #[path] wiring, local-only fixtures, the make check gate, TUI snapshot dumps and the tests that need real artifacts. Use this whenever you add or change a Rust test, create a module that needs a test file, debug a failing or flaky test, need the single-test command, or wonder why the suite must not touch services, GPU or network.
---

# Melodias testing

The suite is a plain `cargo test` with no custom harness, no dev-dependencies and no external
services. Tests live next to the module they exercise, under `src/tests/`. There is no
`tests/` directory at the repository root and no CI.

## Gate

`make check` runs, in order:

1. `cargo fmt --check`
2. `cargo build`
3. `cargo clippy --all-targets -- -D warnings`
4. `cargo test`
5. `scripts/check-skills.sh` (frontmatter, paths and commands cited by the skills)

Run the full gate before finishing. A single test is `cargo test <name>`; use
`cargo test -- --nocapture` when a test prints diagnostics. The debug build is enough; tests do
not need `--release`.

The suite must pass without a GPU, without llama-server, sd-server, ACE-Step or SSH, and without
network. Keep new tests that way. Do not call `yt-dlp`, `ffmpeg`, `ffprobe`, `pnpm` or `scp` from
a normal test.

## Layout and wiring

Test files live in `src/tests/<module>.rs` and are declared at the bottom of the module they
belong to:

```rust
// src/pipeline.rs
#[cfg(test)]
#[path = "tests/pipeline.rs"]
mod tests;
```

The path is relative to the file that declares it. A file inside a subdirectory needs `../`:

```rust
// src/refs/mod.rs
#[cfg(test)]
#[path = "../tests/refs_concepts.rs"]
mod tests;
```

`src/chip/` and `src/refs/` use that form. `src/main.rs` owns `src/tests/main.rs`, which tests
the headless reporter and the `1/11` stage labels.

To add a module test:

1. Create `src/tests/<name>.rs` and start it with `use super::*;`.
2. Add the `#[cfg(test)]` plus `#[path = ...] mod tests;` block at the end of the owner file.
3. Prefer testing a pure function over the whole stage. Most logic in `score`, `retry`,
   `config`, `history` and `references` is pure on purpose.
4. Run `cargo test <name>`, then `make check`.

Do not add a root `tests/` integration test: the repository convention is inline modules, and a
root test cannot reach private items with `super::*`.

## Test conventions

- The test name is a sentence in snake case that states the behavior, for example
  `the_audio_round_needs_eight_points` or `reads_the_optional_fields`.
- One behavior per test. Arrange, act, assert, with an explicit value in the assertion.
- Use `assert_eq!` for exact values and `assert!` for invariants. Include a short message when
  the failure would be hard to read.
- Tests may be `#[ignore]` when they need real artifacts; leave a reason in the attribute, as
  `src/tests/history.rs` does with `needs a finished run in output/ and ffmpeg`. Run ignored
  tests explicitly with `cargo test -- --ignored`.
- Avoid `Config::from_env()` in tests: it reads the real process environment and the user's
  `config.toml` (or the global config) and will make the result depend on the machine.
  `src/tests/config.rs` parses `FileConfig` with `toml::from_str` instead, which is the pattern
  to copy.
- Do not add `proptest`, `mockall`, `rstest` or `insta` just because a generic skill suggests
  them. They are not dependencies; adding one is a dependency change and needs a reason.
- Do not reformat or touch unrelated files to make a test pass.

## Temporary files

There is no `tempfile` crate. Tests build a unique path under `std::env::temp_dir()` with the
process id and the test name, then clean it up:

```rust
let dir = std::env::temp_dir().join(format!(
    "melodias-<area>-{}-<name>",
    std::process::id()
));
```

`src/tests/history.rs` and `src/tests/dashboard.rs` have small helpers for databases and run
directories; reuse their shape instead of inventing a new one. A test that writes SQLite must
close the connection before removing the file, and WAL mode leaves `-wal` and `-shm` files
behind, so remove all three.

## TUI snapshots

`cargo test draws_without_leaving_the_terminal` redraws the app at 80x24, 120x36 and 100x30 using
`TestBackend`. It does not open a terminal.

- `PRINT_TUI=1` prints each buffer as ASCII, which is useful to inspect a layout change.
- `DUMP_TUI_HTML_DIR=/tmp/tui` writes `tui.html` and `tui-splash.html` next to each other. These
  are the generated screenshots; never hand-capture them.
- The test also renders the review screen, so a change to the `Review` struct or the key hints
  shows up there.

## Common failures

| Symptom | Cause | Fix |
| --- | --- | --- |
| `file not found for module tests` | wrong `#[path]` | make it relative to the owner file; add `../` inside `src/refs/` or `src/chip/` |
| test passes alone, fails in the suite | shared temp path or real env/config | include the process id and the test name, avoid `Config::from_env()` |
| `database is locked` | connection or WAL files not closed | drop the connection and remove the `-wal`/`-shm` siblings |
| a stage-label test fails after adding a stage | hardcoded `1/11` | update `src/tests/main.rs` and `STAGES` together |
| clippy fails only on tests | unused import or `needless_borrow` in a test | fix the test, do not silence the lint |

## Verification

- Everything: `make check`.
- One test: `cargo test <name>`.
- TUI ASCII: `PRINT_TUI=1 cargo test draws_without_leaving_the_terminal -- --nocapture`.
- TUI HTML: `DUMP_TUI_HTML_DIR=/tmp/tui cargo test draws_without_leaving_the_terminal`.
- Ignored, artifact-dependent test: `cargo test -- --ignored`.
