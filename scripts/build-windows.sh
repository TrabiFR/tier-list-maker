#!/usr/bin/env bash
# Cross-compiles the Windows app from macOS/Linux and wraps it in an installer.
# Needs: brew install mingw-w64 makensis
# Output: dist/TierListMaker-<version>-windows-setup.exe   (installer)
#         dist/TierListMaker-<version>-windows-x64.exe     (portable, no install needed)
#
# On a Windows PC you can instead run:  cargo build --release
# and take target\release\tierlist-maker.exe
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
TARGET="x86_64-pc-windows-gnu"
ROOT="$(pwd)"

for tool in x86_64-w64-mingw32-gcc makensis; do
  if ! command -v "$tool" >/dev/null; then
    echo "$tool is missing. Install it with: brew install mingw-w64 makensis" >&2
    exit 1
  fi
done

echo "==> Rendering icons"
cargo run --quiet --example make_icon

echo "==> Building $TARGET"
rustup target add "$TARGET" >/dev/null
cargo build --release --target "$TARGET"

mkdir -p dist
EXE="$ROOT/target/$TARGET/release/tierlist-maker.exe"
cp "$EXE" "dist/TierListMaker-$VERSION-windows-x64.exe"

echo "==> Building installer"
# makensis crashes (std::bad_alloc) without a UTF-8 locale.
LC_ALL=en_US.UTF-8 LANG=en_US.UTF-8 makensis -V2 \
  -DVERSION="$VERSION" \
  -DEXE="$EXE" \
  -DROOT="$ROOT" \
  -DOUTFILE="$ROOT/dist/TierListMaker-$VERSION-windows-setup.exe" \
  installer/windows.nsi

echo "Done:"
echo "  dist/TierListMaker-$VERSION-windows-setup.exe"
echo "  dist/TierListMaker-$VERSION-windows-x64.exe"
