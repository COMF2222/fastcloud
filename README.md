# Fastcloud

**SoundCloud, native and fast.** Fastcloud is a desktop SoundCloud client with
a React and Tauri interface, a Rust audio core, and a compact Airwave player
for when the full window is too much.

[![CI](https://github.com/COMF2222/fastcloud/actions/workflows/ci.yml/badge.svg)](https://github.com/COMF2222/fastcloud/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

The active application is in [`desktop/`](desktop/). The Rust modules under
[`src/`](src/) provide the SoundCloud client, authentication, audio pipeline,
cache, player state, and desktop integrations used by the Tauri application.

## What it does

### Listen

- Plays public SoundCloud tracks locally through the Rust HLS and audio
  pipeline.
- Supports seeking, queue editing, shuffle, repeat, volume, playback speed,
  gapless prefetch, a ten-band equalizer, and a limiter.
- Keeps playback state, queue position, and navigation state across restarts.
- Shows a full now-playing view with lyrics, comments, waveform progress, and
  related tracks when the data is available.

### Find and explore

- Searches tracks, playlists, albums, and artists.
- Browses the catalog with fresh releases, popular collections, filters, and
  genre discovery.
- Provides taste-based discovery through liked tracks, listening history, My
  Wave, and rotating genre recommendations.
- Opens dedicated track, playlist, album, and artist pages without losing the
  player.

### Keep your library

- Syncs track likes and artist follows with the connected SoundCloud account.
- Creates, edits, renames, and deletes playlists.
- Saves quick-access links, history, reposts, offline downloads, and uploaded
  tracks.
- Includes storage reporting and cleanup controls for offline audio, artwork,
  cache data, and CLAP preparation files.

### Desktop integration

- Runs as a native Tauri window with system tray support, media keys, global
  shortcuts, deep links, and optional Discord Rich Presence.
- Switches to the 420×104 Airwave mini player with `Ctrl+M` or the mini-player
  button in the player bar. Press the same control to restore the main window.
- Supports Russian and English, dark and light themes, custom accent colours,
  user backgrounds, custom fonts, reduced motion, and Eco/Balanced/Quality
  performance profiles.
- Opens the SoundCloud sign-in screen until an account is connected. The browser
  preview still uses simulated data for layout work.

## Download and builds

Tagged Windows builds are produced by GitHub Actions with the CLAP model and
worker bundled. Download the signed installer from the
[Releases page](https://github.com/COMF2222/fastcloud/releases). Starting with
v0.1.2, the app checks GitHub Releases for updates and offers an update button
in Settings → General and in the sidebar when a new version is available.

### Build the desktop app from source

You need Node.js 22+, Rust 1.95+, and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your
platform.

```sh
git clone https://github.com/COMF2222/fastcloud
cd fastcloud/desktop
npm ci
cp .env.example .env.local
# Set VITE_FASTCLOUD_SERVER_URL in .env.local to your HTTPS access server.
npm run tauri dev
```

The browser preview is useful for checking layout and demo data:

```sh
npm run dev
```

Browser preview does not provide native playback, account access, tray
integration, or real SoundCloud requests. Verify those behaviours in Tauri.

### Local Windows build with CLAP

The CLAP worker and quantized models are downloaded from a pinned model revision
and kept outside the public source tree. On Windows, prepare them and build:

```powershell
cd D:\projects\fastcloud\desktop
npm ci
Copy-Item .env.example .env.local
# Set VITE_FASTCLOUD_SERVER_URL in .env.local to your HTTPS access server.
../tools/prepare_private_clap.ps1
$env:TAURI_SIGNING_PRIVATE_KEY = "$env:USERPROFILE\.tauri\fastcloud-updater.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''
npm run tauri:private -- --bundles nsis
```

The installer is written to
`desktop/src-tauri/target/release/bundle/nsis/Fastcloud_<version>_x64-setup.exe`.
The regular `npm run tauri build` command does not include CLAP.

## Connect a SoundCloud account

Fastcloud does not ship a SoundCloud client secret. The first screen asks users
to sign in. The normal sign-in button connects to the Fastcloud access server
using each listener's own SoundCloud
account. The owner approves new accounts in Settings → Account. The separate
[backend](https://github.com/COMF2222/fastcloud-backend) holds the shared app
credentials and access decisions.

If you have your own Artist Pro application, use the advanced connection option.
That local flow follows SoundCloud's registration and authorization process:

1. SoundCloud opens in the browser and registers an API application for the
   account.
2. The returned client ID and secret are stored in the operating system keyring,
   never in the repository or a plain-text project file.
3. The browser opens again to authorize profile, likes, playlists, follows, and
   feed access.

If you already have an application, use the redirect URI
`http://127.0.0.1:41317/callback`. You can provide credentials through the
environment without writing them to the repository:

```powershell
$env:FASTCLOUD_CLIENT_ID = '…'
$env:FASTCLOUD_CLIENT_SECRET = '…'
```

Go+ tracks and rightsholder restrictions follow SoundCloud's own access rules.

### Connect through an owner-hosted server

The desktop build receives its HTTPS approval-server URL through
`VITE_FASTCLOUD_SERVER_URL` (`desktop/.env.local` for local builds, the
`FASTCLOUD_SERVER_URL` Actions secret for tagged releases). The URL is compiled
into the client and is visible to users of the installed app. Users authorize
their own SoundCloud account. The server keeps the
owner's API client secret and a small list of approved SoundCloud account IDs;
playback, cache, interface, library actions and SoundCloud API requests remain
local. The owner can open **Access requests** from the same screen while signed
in with the SoundCloud account that owns the API app. A pending connection waits
for approval for up to 15 minutes. The backend is maintained as a separate `fastcloud-backend`
repository with its own Docker deployment instructions.

## Desktop controls

| Shortcut | Action |
| --- | --- |
| `Space` | Play or pause |
| `Ctrl+Left` / `Ctrl+Right` | Previous or next track |
| `Left` / `Right` | Seek five seconds |
| `M` | Mute or unmute |
| `Ctrl+F` or `Ctrl+K` | Focus search |
| `Ctrl+M` | Toggle the Airwave mini player |
| `Esc` | Close the queue or clear selection |
| `F1` | Show keyboard shortcuts |

On macOS, use `Cmd` in place of `Ctrl` where the operating system reserves the
shortcut.

## Data, cache, and privacy

Fastcloud stores settings, account state, artwork, audio cache, offline files,
and My Wave data in the operating system application-data directory. The
Settings → Storage screen reports the local sizes and provides cleanup actions.

Client credentials and account tokens use the operating system keyring. Audio
and artwork caches can be cleared from the app; clearing them does not remove
SoundCloud likes or playlists. Offline tracks are local copies and can be
removed individually or all at once.

## Repository layout

| Path | Purpose |
| --- | --- |
| [`desktop/src/`](desktop/src/) | React pages, player UI, state, themes, and API bridge |
| [`desktop/src-tauri/`](desktop/src-tauri/) | Tauri commands, native window, tray, CLAP, and media integration |
| [`src/api/`](src/api/) | SoundCloud endpoints, models, pagination, and request handling |
| [`src/auth/`](src/auth/) | OAuth, PKCE, application registration, and keyring access |
| [`src/audio/`](src/audio/) | HLS download, decoding, EQ, limiter, and cpal output |
| [`src/player/`](src/player/) | Queue, transport, persistence, and playback state |
| [`src/store.rs`](src/store.rs) | Shared data cache and asynchronous request state |
| [`docs/soundcloud-api.md`](docs/soundcloud-api.md) | Endpoint limits and supported substitutions |
| [`docs/releasing.md`](docs/releasing.md) | Release and packaging notes |
| [`docs/TAURI_PARITY.md`](docs/TAURI_PARITY.md) | Desktop feature parity checklist |

## SoundCloud API limits

The public API is narrower than soundcloud.com. Some web features have no
official endpoint, including direct messages, a progressive MP3 endpoint,
playlist snapshots, and a Spotify Connect equivalent. Fastcloud uses related
tracks, likes, history, and local ranking where an official endpoint is not
available. The endpoint-level details and current substitutions are documented
in [`docs/soundcloud-api.md`](docs/soundcloud-api.md).

## Development checks

From the repository root:

```sh
cargo fmt --all -- --check
cargo check --manifest-path desktop/src-tauri/Cargo.toml --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --locked
```

For the React/Tauri application:

```sh
cd desktop
npm ci
npm run build
```

The single-instance IPC test needs an unused local port. Close Fastcloud before
running that test if it reports a bind error.

## Releases

Pushing a `v*` tag runs the CI release workflow. It checks the desktop build,
formats and checks the Rust backend, runs the cross-platform test matrix, builds
the public Tauri bundles, and writes SHA-256 checksums for the resulting
artifacts. The maintainer checklist is in [`docs/releasing.md`](docs/releasing.md).

## Contributing

Bug reports, fixes, interface improvements, and documentation changes are
welcome. Before opening a pull request:

1. Explain the user-visible behaviour or bug in the PR description.
2. Keep SoundCloud endpoint assumptions aligned with
   [`docs/soundcloud-api.md`](docs/soundcloud-api.md).
3. Run the relevant React and Rust checks above.
4. Include a focused regression test for behaviour changes when a test can
   reasonably cover the bug.

## License

Fastcloud is licensed under the [MIT License](LICENSE). Third-party license
notices for bundled assets remain in their respective license files. Fastcloud
is an independent client and is not affiliated with or endorsed by SoundCloud.
