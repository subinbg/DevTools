#!/usr/bin/env bash
# Build zellij for Linux x64 (static, musl) into tools/zellij/dist/zellij.
# Requires Rust (the toolchain pinned in src/rust-toolchain.toml, or set RUSTUP_TOOLCHAIN to
# use an installed one) and, for the musl target, musl-tools plus a C toolchain, perl and make
# (curl and OpenSSL are compiled in for a static binary).
# Set TARGET to override (e.g. TARGET=aarch64-apple-darwin for a native macOS build, which
# links the system curl instead).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$HERE/tool.env"
OUT="$HERE/dist"
mkdir -p "$OUT"

TARGET="${TARGET:-x86_64-unknown-linux-musl}"
FEATURES="vendored_curl"
case "$TARGET" in
  *-apple-darwin) FEATURES="" ;;
esac

cd "$HERE/src"
# no web server, plugins from the checked-in assets (see zellij-utils/Cargo.toml)
cargo build --release --locked --target "$TARGET" --no-default-features \
  ${FEATURES:+--features "$FEATURES"}

BIN="${CARGO_TARGET_DIR:-$HERE/src/target}/$TARGET/release/zellij"
rm -f "$OUT/zellij"
cp "$BIN" "$OUT/zellij"
echo "built $OUT/zellij ($UPSTREAM_REF+devtools, $TARGET)"
