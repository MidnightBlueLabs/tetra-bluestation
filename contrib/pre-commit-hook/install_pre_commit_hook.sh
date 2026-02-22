#!/usr/bin/env bash

# This script installs the pre-commit hook, that checks whether staged files
# are formatted according to rustfmt.toml

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"

ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null)" || {
  echo "install.sh: not inside a git repository" >&2
  exit 1
}

SRC="$SCRIPT_DIR/pre-commit"
DST="$ROOT/.git/hooks/pre-commit"

cp -f "$SRC" "$DST"
chmod +x "$DST"

echo "Installed: $DST"