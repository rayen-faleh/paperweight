#!/usr/bin/env bash
# Build paperweight-mcp in release mode and place it where Tauri expects the
# sidecar: crates/app/binaries/paperweight-mcp-<target-triple>[.exe]
set -euo pipefail
cd "$(dirname "$0")/.."
triple="${TARGET_TRIPLE:-$(rustc -vV | sed -n 's/^host: //p')}"
ext=""
case "$triple" in *windows*) ext=".exe" ;; esac
cargo build --release -p paperweight-mcp ${CARGO_BUILD_TARGET:+--target "$CARGO_BUILD_TARGET"}
src="target/${CARGO_BUILD_TARGET:+$CARGO_BUILD_TARGET/}release/paperweight-mcp$ext"
mkdir -p crates/app/binaries
cp "$src" "crates/app/binaries/paperweight-mcp-$triple$ext"
echo "sidecar: crates/app/binaries/paperweight-mcp-$triple$ext"
