# Contributing

Short version: branch from `main`, follow the repo rules, run the gate, open a PR.

## Workflow

1. Branch: `feature/description`, `fix/description`, `docs/description` or `refactor/description`.
2. Commits: Conventional Commits (`feat(scope): ...`, `fix(scope): ...`). No `--no-verify` unless asked.
3. Push and open a PR against `main`. CI runs `make check`; the branch must be green.
4. Releases are semver tags: bump `version` in `Cargo.toml`, commit, `git tag vX.Y.Z`, push the tag. The release workflow checks the tag matches `Cargo.toml` and publishes notes from the commit log.

The full rules live in `AGENTS.md` (repo root and Dyallab base): strict TypeScript-free Rust, no em dashes in owned files, docs updated with behavior changes, no reformatting outside the change.

## Environment

- Rust 1.88+, `ffmpeg`, `ffprobe`, `tesseract` (optional, `IMG_TEXT` skips without it).
- No GPU, service or network needed for `make check`.
- GPU work needs SSH to the GPU box plus `yt-dlp`, `scp`, `pnpm` for the Remotion cut. See `docs/setup.md` and `docs/runbooks/gpu-recovery.md`.

## Gate

`make check` runs `cargo fmt --check`, build, `clippy --all-targets -- -D warnings`, tests and `scripts/check-skills.sh` plus `scripts/check-docs.sh`, in that order. Run `scripts/setup-hooks.sh` once to install a pre-push hook that runs it.
