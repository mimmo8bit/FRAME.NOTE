# Componenti di terze parti inclusi in FrameNote

## FFmpeg

FrameNote include il programma `ffmpeg` come eseguibile separato (sidecar), avviato
come processo esterno. FFmpeg è un marchio di Fabrice Bellard.

- **Versione:** FFmpeg 8.1.2
- **Licenza:** GNU Lesser General Public License v2.1 o successiva (LGPL).
  Testo completo: <https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html>
  (anche nel file `COPYING.LGPLv2.1` dei sorgenti).
- **Sorgenti:** <https://ffmpeg.org/releases/ffmpeg-8.1.2.tar.xz>
  (SHA256 `464beb5e7bf0c311e68b45ae2f04e9cc2af88851abb4082231742a74d97b524c`,
  firmato con la chiave di rilascio FFmpeg `FCF9 86EA 15E6 E293 A564 4F10 B432 2F04 D676 58D8`).
  Una copia identica dei sorgenti e la configurazione esatta di ogni build sono allegate
  alla release `ffmpeg-8.1.2-lgpl-1` del repository <https://github.com/mimmo8bit/FRAME.NOTE>.

### macOS (Apple Silicon)
Compilato dai sorgenti ufficiali nella CI del repository (`.github/workflows/ffmpeg.yml`) con:

```
--disable-autodetect --disable-gpl --disable-nonfree --disable-network
--enable-videotoolbox --enable-audiotoolbox --enable-zlib
--enable-static --disable-shared --disable-doc --disable-debug
--disable-ffplay --disable-ffprobe
```

### Windows (x64)
Build `win64-lgpl` del progetto BtbN/FFmpeg-Builds (<https://github.com/BtbN/FFmpeg-Builds>),
serie 8.1. Le librerie incluse in quella build e le rispettive licenze sono elencate nel
repository di BtbN; la configurazione esatta (`ffmpeg -version`) è allegata alla release
`ffmpeg-8.1.2-lgpl-1`.

## Altri componenti

L'app è scritta in Rust e TypeScript e usa librerie open source con licenze permissive
(MIT, Apache-2.0, BSD, Zlib, Unicode), tra cui Tauri, React, wgpu, SQLite (pubblico dominio),
rusqlite, tracing, sysinfo e zip. Gli elenchi completi sono in `src-tauri/Cargo.lock` e
`package-lock.json` nel repository.
