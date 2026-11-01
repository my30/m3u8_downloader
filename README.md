# M3U8 Downloader

A cross-platform desktop app for downloading HLS (`.m3u8`) streams using ffmpeg. Built with [Tauri](https://tauri.app) + Svelte.

![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-blue)

---

## Features

- Paste any `.m3u8` stream URL and download it as **MP4, MKV, or TS**
- Optional **start time** and **end time** for trimming (both are positions in the source)
- **Download queue** — add as many jobs as you like; up to 3 run at once, the rest wait
- **Live progress** — percentage when the length is known (VOD or trimmed), elapsed time otherwise
- **Custom HTTP headers** — Referer, User-Agent, cookies, tokens for streams that need them
- **Cancel** stops ffmpeg gracefully, so whatever was downloaded so far stays playable (handy for live streams)
- Bundled ffmpeg — no external installation required

---

## Installation

### macOS

1. Download the latest **`M3U8.Downloader_x.x.x_aarch64.dmg`** (Apple Silicon) or **`_x64.dmg`** (Intel) from the [Releases](../../releases) page.
2. Open the `.dmg` file and drag **M3U8 Downloader** into your **Applications** folder.
3. The app is not notarized by Apple, so the first launch is blocked:
   - Try to open the app once, then open **System Settings → Privacy & Security**.
   - Scroll down and click **Open Anyway** next to the M3U8 Downloader entry, then confirm with your password.
   - If macOS instead says the app **"is damaged and can't be opened"**, run this once in Terminal:
     ```bash
     xattr -cr "/Applications/M3U8 Downloader.app"
     ```
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
| **Start Time** | `-ss` (input) | Skip to this position before downloading. E.g. `00:01:30` starts at 1 minute 30 seconds. |
| **End Time** | `-to` (input) | Stop at this position in the source. E.g. start `00:01:30` + end `00:05:00` gives a 3 m 30 s clip. |

Both fields accept `HH:MM:SS`, `MM:SS` or plain seconds (e.g. `90`).  
Leave both blank to download the entire stream.

> Streams are copied without re-encoding (`-c copy`), so the start snaps to the nearest keyframe and can be a few seconds early.

### HTTP headers *(optional)*

Some sites only serve the playlist when the request looks like it comes from their player. Open **HTTP headers** and fill in:

- **Referer** — usually the URL of the page the video is embedded in
- **User-Agent** — your browser's user agent
- **Extra headers** — one `Name: value` per line, e.g. `Cookie: session=…` or `Authorization: Bearer …`

Headers are also sent for every segment and key request.

### 3. Choose Output Location

- Click **Browse…** to open a folder picker and select your output directory.
- Enter a **filename** (without extension) in the filename field.

### 4. Select Format

Choose the output container:

| Format | Best for |
|---|---|
| **MP4** | General use, widest compatibility |
| **MKV** | Robust container; still playable if the download is interrupted |
| **TS** | Raw MPEG transport stream |

### 5. Add to Queue

Click **Add to Queue**. Up to 3 downloads run at once; extra jobs wait as *Pending*.

- The **progress bar** shows a percentage when the length is known (VOD playlists or a trim range), and the elapsed time otherwise (live streams).
- Click **Show log** on any job to see the last 500 lines of ffmpeg output.
- Click **Cancel** to stop a download. ffmpeg is asked to finish cleanly, so the partial file stays playable.
- Click **Clear finished** to remove completed/errored jobs from the list.
- Existing files are never overwritten; pick a different filename instead.

ffmpeg picks one video and one audio stream (for multi-quality playlists, the highest-resolution variant). Separate subtitle or alternate-audio tracks aren't downloaded.

---

## Building from Source

### Prerequisites

| Tool | Version | Install |
|---|---|---|
| Node.js | 22+ | [nodejs.org](https://nodejs.org) or `brew install node` |
| Rust | stable | [rustup.rs](https://rustup.rs) |
| Tauri CLI | 2.x | included as an npm dev dependency (`npx tauri …`) |

On Linux (development only) you also need the [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/#linux).

### macOS

```bash
# 1. Clone the repository
git clone https://github.com/my30/m3u8_downloader.git
cd m3u8_downloader

# 2. Install frontend dependencies
npm install

# 3. Download the bundled ffmpeg binary for your Mac
./scripts/download-ffmpeg.sh

# 4. Start the dev server (opens the app window)
npx tauri dev

# 5. Build a release .dmg
npx tauri build
```

### Windows

```powershell
# 1. Clone the repository
git clone https://github.com/my30/m3u8_downloader.git
cd m3u8_downloader

# 2. Install frontend dependencies
npm install

# 3. Download the bundled ffmpeg binary for Windows (run from Git Bash)
bash scripts/download-ffmpeg.sh x86_64-pc-windows-msvc

# 4. Start the dev server (opens the app window)
npx tauri dev

# 5. Build a release installer
npx tauri build
```

The built installer will be in `src-tauri/target/release/bundle/`.

### Checks

```bash
npm run check                          # Svelte / TypeScript
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

`cargo test` includes an end-to-end test that runs a real `ffmpeg` from your `PATH` (skipped if there is none).

---

## Releasing

1. Bump the version in **both** `package.json` and `src-tauri/Cargo.toml`. `tauri.conf.json` reads it from `package.json`, and CI fails if the two differ.
2. Push a version tag. GitHub Actions builds macOS (Apple Silicon + Intel) and Windows installers and attaches them to a single **draft** release, which you publish by hand:

```bash
git tag v1.0.0
git push origin v1.0.0
```

---

Running the workflow manually (*Actions → Release → Run workflow*) builds everything and uploads workflow artifacts without creating a release.

---

## License

The app's source is [MIT](LICENSE).

The bundled **ffmpeg** is a separate program under its own license. The app only runs it as an external process and does not link against it:

- **macOS** builds bundle ffmpeg from [Martin Riedl's build server](https://ffmpeg.martin-riedl.de) (**GPL**; includes GPL libraries such as x264).
- **Windows** builds bundle the **LGPL** build from [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds).

FFmpeg source code is available from [ffmpeg.org](https://ffmpeg.org/download.html) and the build scripts from the links above. If you redistribute release binaries, you take on the obligations of those licenses, including offering the matching source for the GPL build.

Only download content you have the right to download.
