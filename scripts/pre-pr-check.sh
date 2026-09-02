#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

echo "[1/8] cargo fmt"
cargo fmt --all -- --check
echo "[2/8] cargo clippy"
cargo clippy --workspace --all-targets -- -D warnings
echo "[3/8] cargo build"
cargo build --workspace --locked
echo "[4/8] cargo test"
cargo test --workspace --locked

web_dir="$repo_root/crates/omniproj-desktop/web"
cd "$web_dir"
echo "[5/8] npm ci"
npm ci
echo "[6/8] frontend build"
npm run build
echo "[7/8] frontend unit tests"
npm test -- --run
echo "[8/8] frontend E2E"
npm run test:e2e

cd "$repo_root"
echo "All pre-PR checks passed."
