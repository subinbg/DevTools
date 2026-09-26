#!/usr/bin/env bash
# Build lazygit for Linux x64 into tools/lazygit/dist/lazygit.
# Requires Go (version from src/go.mod; `go` auto-downloads a newer toolchain if needed).
# Set GOOS/GOARCH to override the target (e.g. GOOS=darwin GOARCH=arm64 for a local build).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$HERE/tool.env"
OUT="$HERE/dist"
mkdir -p "$OUT"

COMMIT="$(git -C "$HERE" rev-parse --short HEAD 2>/dev/null || echo unknown)"
DATE="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
VERSION="${UPSTREAM_REF#v}+devtools"

cd "$HERE/src"
GOOS="${GOOS:-linux}" GOARCH="${GOARCH:-amd64}" CGO_ENABLED=0 go build -trimpath \
  -ldflags "-s -w -X main.version=$VERSION -X main.commit=$COMMIT -X main.date=$DATE -X main.buildSource=binaryRelease" \
  -o "$OUT/lazygit" .

echo "built $OUT/lazygit ($VERSION, $COMMIT, ${GOOS:-linux}/${GOARCH:-amd64})"
