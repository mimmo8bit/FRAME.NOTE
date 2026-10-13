#!/usr/bin/env bash
# Crea il .dmg con FrameNote.app e il collegamento ad Applicazioni.
# Uso: scripts/macos-dmg.sh path/FrameNote.app path/output.dmg
set -euo pipefail
app="${1:?app mancante}"
dmg="${2:?dmg mancante}"
staging=$(mktemp -d)
trap 'rm -rf "$staging"' EXIT

ditto "$app" "$staging/FrameNote.app"
ln -s /Applications "$staging/Applicazioni"
rm -f "$dmg"
# hdiutil a volte fallisce con "Resource busy" sui runner: qualche tentativo.
for attempt in 1 2 3 4; do
  if hdiutil create -volname "FrameNote" -srcfolder "$staging" -fs HFS+ -format UDZO -ov "$dmg"; then
    break
  fi
  [[ $attempt -lt 4 ]] || { echo "::error::hdiutil non riesce a creare il dmg"; exit 1; }
  sleep $((attempt * 5))
done

# Verifica: l'app dentro il dmg deve avere la firma intatta.
mnt=$(mktemp -d)
hdiutil attach -nobrowse -readonly -mountpoint "$mnt" "$dmg" >/dev/null
codesign --verify --deep --strict "$mnt/FrameNote.app"
hdiutil detach "$mnt" >/dev/null
echo "Creato $dmg"
