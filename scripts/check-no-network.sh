#!/usr/bin/env bash
# Guardiano della rete: FrameNote non deve contenere componenti capaci di
# collegarsi a internet. Fallisce se nelle dipendenze *compilate* (macOS,
# Windows, Linux) compare un client HTTP, WebSocket o TLS.
# Le eccezioni future (modulo net/, PIANO_SICUREZZA.md fase B) vanno approvate
# da Domenico e aggiunte qui in modo esplicito.
set -euo pipefail
cd "$(dirname "$0")/../src-tauri"

forbidden='^(reqwest|hyper|hyper-util|h2|h3|quinn|ureq|curl|curl-sys|isahc|attohttpc|surf|minreq|tungstenite|tokio-tungstenite|async-tungstenite|rustls|native-tls|openssl|openssl-sys|tokio-native-tls|tokio-rustls|hickory-resolver|trust-dns-resolver|tauri-plugin-http|tauri-plugin-updater|tauri-plugin-websocket|tauri-plugin-upload)$'

fail=0
for target in aarch64-apple-darwin x86_64-pc-windows-msvc x86_64-unknown-linux-gnu; do
  found=$(cargo tree --locked --target "$target" -e normal --prefix none --format '{p}' \
    | awk '{print $1}' | sort -u | grep -E "$forbidden" || true)
  if [[ -n "$found" ]]; then
    echo "::error::componenti di rete nelle dipendenze per $target: $(echo "$found" | tr '\n' ' ')"
    for crate in $found; do
      cargo tree --locked --target "$target" -e normal -i "$crate" | head -20
    done
    fail=1
  else
    echo "ok: nessun componente di rete per $target"
  fi
done

# Strumenti per sviluppatori della webview mai nella build.
if grep -Eq 'devtools' Cargo.toml; then
  echo "::error::la funzione devtools di Tauri non deve essere attiva"
  fail=1
fi
exit $fail
