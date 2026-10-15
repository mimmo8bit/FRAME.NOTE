#!/usr/bin/env bash
# Nel repository (pubblico) non devono mai finire registrazioni, eventi del
# mouse, database, diagnostiche o chiavi: fallisce se ne trova.
set -euo pipefail
cd "$(dirname "$0")/.."

forbidden='\.(mp4|mov|m4v|mkv|webm|wav|m4a|mp3|opus|ogg|jsonl|sqlite|sqlite3|db|p12|pfx|pem|key|keychain|keychain-db|cer|mobileprovision|env\.local)$|(^|/)(framenote-diagnostica-[^/]*\.zip|project\.json|\.env)$'
found=$(git ls-files | grep -Ei "$forbidden" || true)
if [[ -n "$found" ]]; then
  echo "::error::file da non pubblicare nel repository:"
  echo "$found"
  exit 1
fi
echo "ok: nessuna registrazione, diagnostica o chiave nel repository"
