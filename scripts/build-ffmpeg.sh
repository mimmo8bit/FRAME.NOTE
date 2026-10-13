#!/usr/bin/env bash
# Compila FFmpeg LGPL statico dai sorgenti ufficiali (usato in CI per macOS arm64).
# Uso: scripts/build-ffmpeg.sh <cartella-di-output>
# Produce: ffmpeg-<target>, BUILD_INFO-<os>.txt, il tarball dei sorgenti e COPYING.LGPLv2.1.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
source "$root/scripts/ffmpeg.env"

out="$(mkdir -p "${1:?cartella di output mancante}" && cd "$1" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
tarball="ffmpeg-$FFMPEG_VERSION.tar.xz"

sha256() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | awk '{print $1}'
  else shasum -a 256 "$1" | awk '{print $1}'; fi
}

# Sorgente ufficiale; il mirror Ubuntu contiene lo stesso identico file firmato
# (orig.tar.xz). Lo SHA256 fissato in ffmpeg.env vale per entrambi.
urls=(
  "https://ffmpeg.org/releases/$tarball"
  "http://archive.ubuntu.com/ubuntu/pool/universe/f/ffmpeg/ffmpeg_$FFMPEG_VERSION.orig.tar.xz"
)
for url in "${urls[@]}"; do
  echo "Scarico $url"
  if curl -fsSL --retry 3 -o "$work/$tarball" "$url" && [[ "$(sha256 "$work/$tarball")" == "$FFMPEG_SHA256" ]]; then
    ok=1; break
  fi
  echo "::warning::download da $url non riuscito o SHA256 diverso"
done
[[ "${ok:-}" == 1 ]] || { echo "::error::sorgenti di FFmpeg $FFMPEG_VERSION non verificabili"; exit 1; }
echo "SHA256 verificato: $FFMPEG_SHA256"

tar -xJf "$work/$tarball" -C "$work"
src="$work/ffmpeg-$FFMPEG_VERSION"

flags=(
  --disable-autodetect
  --disable-gpl --disable-nonfree --disable-network
  --enable-static --disable-shared
  --disable-doc --disable-debug
  --disable-ffplay --disable-ffprobe
  --enable-zlib
)
case "$(uname -s)" in
  Darwin)
    target="$(uname -m | sed 's/arm64/aarch64/')-apple-darwin"
    flags+=(--enable-videotoolbox --enable-audiotoolbox
            --extra-cflags=-mmacosx-version-min=12.3 --extra-ldflags=-mmacosx-version-min=12.3)
    jobs=$(sysctl -n hw.ncpu)
    os=macos ;;
  Linux)
    target="$(uname -m)-unknown-linux-gnu"
    jobs=$(nproc)
    os=linux ;;
  *) echo "::error::sistema non supportato: $(uname -s)"; exit 1 ;;
esac

(cd "$src" && ./configure "${flags[@]}" && make -j"$jobs" ffmpeg)
bin="$src/ffmpeg"

# Il binario deve dipendere solo da librerie di sistema, mai da Homebrew o simili.
echo "Dipendenze dinamiche:"
if [[ "$os" == macos ]]; then
  deps=$(otool -L "$bin" | tail -n +2 | awk '{print $1}')
  echo "$deps"
  bad=$(echo "$deps" | grep -vE '^(/usr/lib/|/System/Library/)' || true)
else
  deps=$(ldd "$bin" | awk '{print ($3 ? $3 : $1)}')
  echo "$deps"
  bad=$(echo "$deps" | grep -vE '^(/lib|/usr/lib|linux-vdso)' || true)
fi
[[ -z "$bad" ]] || { echo "::error::dipendenze non di sistema: $bad"; exit 1; }

"$bin" -hide_banner -version
# (output catturato prima del grep: con pipefail, grep -q chiuderebbe la pipe in anticipo)
license=$("$bin" -hide_banner -L)
grep -q "GNU Lesser General Public" <<<"$license" || { echo "::error::la build non è LGPL"; exit 1; }
if [[ "$os" == macos ]]; then
  encoders=$("$bin" -hide_banner -encoders)
  grep -q h264_videotoolbox <<<"$encoders" || { echo "::error::h264_videotoolbox mancante"; exit 1; }
fi

cp "$bin" "$out/ffmpeg-$target"
cp "$work/$tarball" "$out/$tarball"
cp "$src/COPYING.LGPLv2.1" "$out/COPYING.LGPLv2.1"
{
  echo "FFmpeg $FFMPEG_VERSION per $target"
  echo "Sorgenti: $tarball (sha256 $FFMPEG_SHA256)"
  echo "Configure: ${flags[*]}"
  echo
  "$bin" -hide_banner -version
  echo
  echo "Encoder:"
  "$bin" -hide_banner -encoders
} > "$out/BUILD_INFO-$os.txt"
echo "Pronto: $out/ffmpeg-$target"
