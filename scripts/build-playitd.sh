#!/usr/bin/env bash
# Builds the playit.gg agent (playitd) from source for macOS. playit.gg publishes no
# macOS binary, so Lodestar ships this build inside the app as a Tauri sidecar.
#
#   scripts/build-playitd.sh            # the target Tauri is building, else this Mac's
#   scripts/build-playitd.sh universal  # Apple Silicon + Intel, for a universal build
#
# Output: src-tauri/binaries/playitd-<target-triple>, the name Tauri's externalBin
# expects. A universal build needs all three (each architecture is compiled on its
# own first, then bundled with the merged one). Existing outputs are kept; set FORCE=1
# to rebuild.
set -euo pipefail

VERSION="v1.0.10"
COMMIT="9e7b9a1cb42d057e7993e21ef4fe32348d1e7fcd" # the v1.0.10 tag, pinned

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/src-tauri/binaries"
SRC="$ROOT/src-tauri/target/playit-agent-$VERSION"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "build-playitd: only needed on macOS (Windows downloads the signed agent)." >&2
  exit 0
fi

host="$(rustc -vV | sed -n 's/^host: //p')"
# Tauri passes the target it is building for to beforeBuildCommand/beforeDevCommand.
want="${1:-${TAURI_ENV_TARGET_TRIPLE:-$host}}"
if [[ "$want" == "universal" || "$want" == "universal-apple-darwin" ]]; then
  final="universal-apple-darwin"
  targets=(aarch64-apple-darwin x86_64-apple-darwin)
else
  final="$want"
  targets=("$want")
fi

outputs=("$final")
[[ "$final" == "universal-apple-darwin" ]] && outputs+=("${targets[@]}")
missing=0
for o in "${outputs[@]}"; do [[ -f "$OUT/playitd-$o" ]] || missing=1; done
if [[ "$missing" == "0" && "${FORCE:-0}" != "1" ]]; then
  echo "build-playitd: playitd-$final already built (FORCE=1 to rebuild)."
  exit 0
fi

if [[ ! -d "$SRC/.git" ]]; then
  git clone --quiet --depth 1 --branch "$VERSION" https://github.com/playit-cloud/playit-agent "$SRC"
fi
actual="$(git -C "$SRC" rev-parse HEAD)"
if [[ "$actual" != "$COMMIT" ]]; then
  echo "build-playitd: $VERSION is $actual, expected $COMMIT. Refusing to build." >&2
  exit 1
fi

mkdir -p "$OUT"
built=()
for t in "${targets[@]}"; do
  rustup target add "$t" >/dev/null 2>&1 || true
  cargo build --manifest-path "$SRC/Cargo.toml" --locked --release -p playitd --bin playitd --target "$t"
  built+=("$SRC/target/$t/release/playitd")
  cp "$SRC/target/$t/release/playitd" "$OUT/playitd-$t"
  chmod 755 "$OUT/playitd-$t"
done

if [[ "$final" == "universal-apple-darwin" ]]; then
  lipo -create -output "$OUT/playitd-$final" "${built[@]}"
  chmod 755 "$OUT/playitd-$final"
fi
echo "build-playitd: wrote $OUT/playitd-$final"
