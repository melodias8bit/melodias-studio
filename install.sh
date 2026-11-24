#!/usr/bin/env bash
# melodias-studio installer.
#
#   curl -fsSL https://raw.githubusercontent.com/melodias8bit/melodias-studio/main/install.sh | bash
#
# Verifies every prerequisite, clones (or updates) the repository, builds the
# release binary and installs it into ~/.local/bin. Flags:
#
#   --check-only   verify the prerequisites and exit without building
#   --dir=<path>   clone or update the repository at <path>
#                  (default: $MELODIAS_DIR or ~/melodias-studio)
set -euo pipefail

REPO_URL="https://github.com/melodias8bit/melodias-studio.git"
DEST="${MELODIAS_DIR:-$HOME/melodias-studio}"
BINDIR="${PREFIX:-$HOME/.local}/bin"
CHECK_ONLY=0
for arg in "$@"; do
  case "$arg" in
    --check-only) CHECK_ONLY=1 ;;
    --dir=*) DEST="${arg#--dir=}" ;;
    *) printf 'install: unknown flag %s\n' "$arg" >&2; exit 2 ;;
  esac
done

have() { command -v "$1" >/dev/null 2>&1; }

fail() {
  printf 'install: %s\n' "$1" >&2
  exit 1
}

note() { printf 'install: %s\n' "$1"; }

note "checking prerequisites"

missing=""
for tool in git curl ffmpeg ffprobe yt-dlp scp ssh; do
  if have "$tool"; then
    note "ok $tool: $(command -v "$tool")"
  else
    note "missing $tool"
    missing="$missing $tool"
  fi
done

if ! have cargo || ! have rustc; then
  note "installing the Rust toolchain with rustup"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
    sh -s -- -y --profile minimal --default-toolchain stable
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi
have cargo || fail "cargo is still missing after rustup"
note "ok cargo: $(cargo --version)"
note "ok rustc: $(rustc --version)"

if [ -n "$missing" ]; then
  fail "missing tools:$missing. Install them with your package manager, then rerun."
fi

for tool in ffmpeg ffprobe; do
  note "ok $tool: $($tool -version 2>/dev/null | head -n 1)"
done
note "ok yt-dlp: $(yt-dlp --version 2>/dev/null | head -n 1)"

# Optional pieces: only the video renderer and the cloud judge need these.
have node || note "optional: node is missing, only the Remotion renderer needs it"
have pnpm || note "optional: pnpm is missing, only the Remotion renderer needs it"

if [ "$CHECK_ONLY" -eq 1 ]; then
  note "all prerequisites verified"
  exit 0
fi

if [ -d "$DEST/.git" ]; then
  note "updating $DEST"
  git -C "$DEST" pull --ff-only
else
  note "cloning into $DEST"
  git clone "$REPO_URL" "$DEST"
fi

note "building the release binary (first build takes a few minutes)"
(cd "$DEST" && cargo build --release)

mkdir -p "$BINDIR"
install -m 755 "$DEST/target/release/melodias" "$BINDIR/melodias"
note "installed $BINDIR/melodias"

# Smoke test: history on a machine without the database prints a hint and exits 0.
"$BINDIR/melodias" history >/dev/null 2>&1 ||
  fail "the installed binary failed its smoke test"

case ":$PATH:" in
  *":$BINDIR:"*) ;;
  *) note "add $BINDIR to PATH, for example: export PATH=\"$BINDIR:\$PATH\"" ;;
esac

note "done. Next: copy config.example.toml to \$XDG_CONFIG_HOME/melodias-studio/config.toml,"
note "point it at your GPU services, then run: melodias '<url>' '<vibe>'"
