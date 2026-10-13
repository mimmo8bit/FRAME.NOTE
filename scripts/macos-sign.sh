#!/usr/bin/env bash
# Firma FrameNote.app (e il sidecar FFmpeg) con il certificato self-signed stabile
# salvato nei secret di GitHub Actions. Il requisito designato (che contiene l'hash
# del certificato) resta identico tra le build: così macOS mantiene i permessi concessi.
#
# Variabili richieste: MACOS_CERT_P12 (base64), MACOS_CERT_PASSWORD, MACOS_SIGNING_IDENTITY (SHA-1).
# Se mancano e SIGNING_REQUIRED != true, firma ad-hoc con un avviso (solo build di prova).
# Uso: scripts/macos-sign.sh path/FrameNote.app
set -euo pipefail
app="${1:?percorso di FrameNote.app mancante}"
sidecar="$app/Contents/MacOS/ffmpeg"
[[ -x "$sidecar" ]] || { echo "::error::sidecar FFmpeg mancante in $app"; exit 1; }

if [[ -z "${MACOS_CERT_P12:-}" || -z "${MACOS_CERT_PASSWORD:-}" || -z "${MACOS_SIGNING_IDENTITY:-}" ]]; then
  if [[ "${SIGNING_REQUIRED:-false}" == true ]]; then
    echo "::error::certificato di firma assente: esegui una volta il workflow 'Crea certificato di firma'"
    exit 1
  fi
  echo "::warning::certificato di firma assente: firma ad-hoc. I permessi di macOS andranno riconcessi a ogni build."
  codesign --force --options runtime --timestamp=none -s - "$sidecar"
  codesign --force --options runtime --timestamp=none -s - "$app"
  codesign --verify --deep --strict --verbose=2 "$app"
  exit 0
fi

keychain="$RUNNER_TEMP/framenote-signing.keychain-db"
p12="$RUNNER_TEMP/framenote-signing.p12"
kc_password=$(openssl rand -hex 24)
echo "::add-mask::$kc_password"
cleanup() {
  rm -f "$p12"
  security delete-keychain "$keychain" 2>/dev/null || true
}
trap cleanup EXIT

printf '%s' "$MACOS_CERT_P12" | base64 --decode > "$p12"
security create-keychain -p "$kc_password" "$keychain"
security set-keychain-settings -lut 3600 "$keychain"
security unlock-keychain -p "$kc_password" "$keychain"
security import "$p12" -k "$keychain" -f pkcs12 -P "$MACOS_CERT_PASSWORD" -T /usr/bin/codesign >/dev/null
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$kc_password" "$keychain" >/dev/null
# Il keychain temporaneo va nella lista di ricerca, insieme a quelli esistenti.
# (compatibile con bash 3.2 di macOS: niente mapfile)
existing=()
while IFS= read -r kc; do
  kc="${kc//\"/}"
  kc="${kc#"${kc%%[![:space:]]*}"}"
  [[ -n "$kc" ]] && existing+=("$kc")
done < <(security list-keychains -d user)
security list-keychains -d user -s "$keychain" ${existing[@]+"${existing[@]}"}

sign() {
  codesign --force --options runtime --timestamp=none \
    --keychain "$keychain" -s "$MACOS_SIGNING_IDENTITY" "$@"
}
# Prima i binari interni, poi il bundle.
sign "$sidecar"
sign "$app"

codesign --verify --deep --strict --verbose=2 "$app"
echo "Requisito designato (deve restare identico tra le build):"
codesign -d -r- "$app" 2>&1 | grep designated
codesign -d -r- "$sidecar" 2>&1 | grep designated
