#!/usr/bin/env bash
# Package tools/<tool>/dist/<tool> as <tool>-linux-x64 (raw binary), a .tar.gz, and a .sha256.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOL="${1:?tool name required}"
DIST="$ROOT/tools/$TOOL/dist"
BIN="$DIST/$TOOL"
[[ -x "$BIN" ]] || { echo "missing binary $BIN (run tools/$TOOL/build.sh first)" >&2; exit 1; }

NAME="$TOOL-linux-x64"
cp "$BIN" "$DIST/$NAME"
tar -C "$DIST" -czf "$DIST/$NAME.tar.gz" "$TOOL"
(cd "$DIST" && sha256sum "$NAME" "$NAME.tar.gz" >"$NAME.sha256")
cat "$DIST/$NAME.sha256"
