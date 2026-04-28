# M3U8 Downloader

A cross-platform desktop app for downloading HLS (`.m3u8`) streams using ffmpeg. Built with [Tauri](https://tauri.app) + Svelte.

![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-blue)

---

## Features

- Paste any `.m3u8` stream URL and download it as **MP4, MKV, or TS**
- Optional **start time** (`-ss`) and **end time** (`-to`) for trimming
- **Download queue** — add multiple jobs and run them concurrently
- **Live progress** — real-time ffmpeg output with elapsed time
- **Cancel** any in-progress download
- Bundled ffmpeg — no external installation required

---

## Installation

### macOS

1. Download the latest **`M3U8.Downloader_x.x.x_aarch64.dmg`** (Apple Silicon) or **`_x64.dmg`** (Intel) from the [Releases](../../releases) page.
2. Open the `.dmg` file and drag **M3U8 Downloader** into your **Applications** folder.
3. On first launch, macOS may show a security warning because the app is not notarized:
   - Open **System Settings → Privacy & Security**.
   - Scroll down and click **Open Anyway** next to the M3U8 Downloader entry.
4. The app will open normally on subsequent launches.

### Windows

1. Download the latest **`M3U8.Downloader_x.x.x_x64-setup.exe`** from the [Releases](../../releases) page.
2. Run the installer and follow the prompts.
3. Windows SmartScreen may show a warning on first run — click **More info → Run anyway**.
4. Launch **M3U8 Downloader** from the Start menu or desktop shortcut.

---

## How to Use

### 1. Enter the Stream URL

Paste the full `.m3u8` URL into the **Stream URL** field.

```
https://example.com/live/stream/index.m3u8
```

### 2. Set Optional Time Range *(optional)*

| Field | ffmpeg flag | Description |
|---|---|---|
| **Start Time** | `-ss` | Skip to this position before downloading. E.g. `00:01:30` starts at 1 minute 30 seconds. |
| **End Time** | `-to` | Stop downloading at this absolute timestamp. E.g. `00:05:00` stops at 5 minutes. |

Both fields accept `HH:MM:SS` format or plain seconds (e.g. `90`).  
Leave both blank to download the entire stream.

### 3. Choose Output Location

- Click **Browse…** to open a folder picker and select your output directory.
- Enter a **filename** (without extension) in the filename field.

### 4. Select Format

Choose the output container:

| Format | Best for |
|---|---|
| **MP4** | General use, widest compatibility |
| **MKV** | Preserves multiple audio/subtitle tracks |
| **TS** | Raw MPEG transport stream |

### 5. Add to Queue

Click **Add to Queue**. The download starts immediately and appears in the queue below.

- The **progress bar** and elapsed time update in real time.
- Click **Show log** on any job to see the raw ffmpeg output.
- Click **Cancel** to stop an in-progress download.
- Click **Clear finished** to remove completed/errored jobs from the list.

---

## Building from Source

### Prerequisites

| Tool | Version | Install |
|---|---|---|
| Node.js | 22+ | [nodejs.org](https://nodejs.org) or `brew install node` |
| Rust | stable | [rustup.rs](https://rustup.rs) |
| Tauri CLI | 2.x | `cargo install tauri-cli --version "^2" --locked` |

### macOS

```bash
# 1. Clone the repository
git clone https://github.com/your-username/m3u8_downloader.git
cd m3u8_downloader

# 2. Install frontend dependencies
npm install

# 3. Download the bundled ffmpeg binary for your Mac
chmod +x scripts/download-ffmpeg.sh
./scripts/download-ffmpeg.sh

# 4. Start the dev server (opens the app window)
cargo tauri dev

# 5. Build a release .dmg
cargo tauri build
```

### Windows

```powershell
# 1. Clone the repository
git clone https://github.com/your-username/m3u8_downloader.git
cd m3u8_downloader

# 2. Install frontend dependencies
npm install

# 3. Download the bundled ffmpeg binary for Windows
$url = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-lgpl-essentials.zip"
Invoke-WebRequest -Uri $url -OutFile ffmpeg-win64.zip
Expand-Archive ffmpeg-win64.zip -DestinationPath ffmpeg-tmp
$exe = Get-ChildItem -Path ffmpeg-tmp -Recurse -Filter "ffmpeg.exe" | Select-Object -First 1
New-Item -ItemType Directory -Force -Path src-tauri\binaries | Out-Null
Copy-Item $exe.FullName "src-tauri\binaries\ffmpeg-x86_64-pc-windows-msvc.exe"
Remove-Item -Recurse ffmpeg-tmp, ffmpeg-win64.zip

# 4. Start the dev server (opens the app window)
cargo tauri dev

# 5. Build a release installer
cargo tauri build
```

The built installer will be in `src-tauri/target/release/bundle/`.

---

## Releasing

Push a version tag to trigger the GitHub Actions workflow, which builds installers for both platforms and drafts a GitHub Release:

```bash
git tag v1.0.0
git push origin v1.0.0
```

---

## License

MIT
