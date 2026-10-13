#!/usr/bin/env bash
# Scarica il sidecar FFmpeg per un target dalla release del repository,
# ne verifica lo SHA256 e lo copia in src-tauri/binaries/ffmpeg-<target>[.exe].
# Uso: scripts/fetch-ffmpeg.sh aarch64-apple-darwin | x86_64-pc-windows-msvc
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/ffmpeg.env

target="${1:?target mancante}"
case "$target" in
  *windows*) name="ffmpeg-$target.exe" ;;
  *) name="ffmpeg-$target" ;;
esac

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
gh release download "$FFMPEG_RELEASE_TAG" --pattern "$name" --pattern SHA256SUMS.txt --dir "$tmp"

expected=$(awk -v n="$name" '$2 == n { print $1 }' "$tmp/SHA256SUMS.txt")
if [[ -z "$expected" ]]; then
  echo "::error::$name non è elencato in SHA256SUMS.txt della release $FFMPEG_RELEASE_TAG"; exit 1
fi
if command -v sha256sum >/dev/null; then
  actual=$(sha256sum "$tmp/$name" | awk '{print $1}')
else
  actual=$(shasum -a 256 "$tmp/$name" | awk '{print $1}')
fi
if [[ "$actual" != "$expected" ]]; then
  echo "::error::SHA256 di $name non corrisponde (atteso $expected, trovato $actual)"; exit 1
fi

mkdir -p src-tauri/binaries
cp "$tmp/$name" "src-tauri/binaries/$name"
chmod +x "src-tauri/binaries/$name"
echo "FFmpeg $FFMPEG_VERSION per $target pronto (sha256 $actual)"
