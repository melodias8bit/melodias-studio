#!/usr/bin/env bash
# Installs a pre-push hook that runs the gate. Re-run after pulling hook changes.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
hook="$root/.git/hooks/pre-push"
cat > "$hook" << 'EOF'
#!/usr/bin/env bash
# Pre-push gate: keep main green without waiting for CI.
make -C "$(git rev-parse --show-toplevel)" check
EOF
chmod +x "$hook"
echo "installed $hook"
