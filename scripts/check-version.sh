#!/usr/bin/env bash
# La versione dell'app vive in src-tauri/tauri.conf.json. Cargo.toml e package.json
# devono coincidere; su un tag vX.Y.Z anche il tag.
set -euo pipefail
cd "$(dirname "$0")/.."

conf=$(node -p "require('./src-tauri/tauri.conf.json').version")
pkg=$(node -p "require('./package.json').version")
cargo=$(sed -n 's/^version = "\(.*\)"$/\1/p' src-tauri/Cargo.toml | head -n1)

fail=0
if [[ "$pkg" != "$conf" ]]; then
  echo "::error::package.json ha $pkg, tauri.conf.json ha $conf"; fail=1
fi
if [[ "$cargo" != "$conf" ]]; then
  echo "::error::src-tauri/Cargo.toml ha $cargo, tauri.conf.json ha $conf"; fail=1
fi
if [[ "${GITHUB_REF_TYPE:-}" == "tag" ]]; then
  tag="${GITHUB_REF_NAME#v}"
  if [[ "$tag" != "$conf" ]]; then
    echo "::error::il tag è v$tag ma tauri.conf.json ha $conf"; fail=1
  fi
fi
[[ $fail -eq 0 ]] && echo "Versione $conf coerente"
exit $fail
