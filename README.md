# Fastcloud

Native [SoundCloud](https://soundcloud.com) desktop client written in Rust.

egui/eframe UI, symphonia decoding with a 10-band EQ and limiter, cpal audio
output, HLS streaming with a disk cache, OS media keys, tray icon, and a
remote-control CLI. Same stack as Fastpotify.

![CI](https://github.com/fastcloud/fastcloud/actions/workflows/ci.yml/badge.svg)

## Features

- **Playback**: HLS (MP3/AAC) via a custom downloader → symphonia → 10-band
  EQ → limiter → cpal. Queue with shuffle/repeat, seek, gapless prefetch,
  resume of the last session, disk cache, 30-second preview fallback when a
  full stream is unavailable.
- **Library**: likes, playlists, followings, feed, recently played, search
  (tracks/playlists/people), track/playlist/user pages, reposts, comments,
  related tracks, playlist creation/editing (PUT with the full track list),
  linked-partitioning pagination, HTTP 429 handling with Retry-After.
- **Auth**: OAuth 2.1 + PKCE via `secure.soundcloud.com`, loopback redirect
  on `127.0.0.1:41317/callback`, tokens in the OS keyring, automatic refresh.
- **Desktop**: tray icon (tray-icon/ksni), media keys + now-playing metadata
  (souvlaki/SMTC/MPRIS), single instance, CLI remote control
  (`fastcloud next`, `fastcloud toggle`, …), global hotkeys, light/dark/system
  themes, accent color derived from cover art, drag & drop of tracks into
  playlists.
- **Demo mode**: `fastcloud --demo` runs fully offline with synthesized
  content — no API keys required.

## Install

| Platform | Method |
|----------|--------|
| Arch Linux | `packaging/PKGBUILD` (AUR soon) |
| macOS (arm64) | Homebrew cask: `packaging/Casks/fastcloud.rb` |
| Linux (Flatpak) | `packaging/flatpak/dev.fastcloud.Fastcloud.json` |
| Windows | Inno Setup: `packaging/windows/installer.iss` |
| Any | `cargo install fastcloud` or build from source |

## Build

```sh
cargo build --release
cargo test
```

Linux requires `libgtk-3-dev libasound2-dev libudev-dev libxdo-dev
libappindicator3-dev` (tray + audio).

### Credentials

Fastcloud needs a registered SoundCloud app for full-quality streams:

```sh
export FASTCLOUD_CLIENT_ID=...
export FASTCLOUD_CLIENT_SECRET=...
./target/release/fastcloud
```

The app keys can also be stored via the Settings dialog (kept in the OS
keyring). Registering an app requires [SoundCloud Artist Pro](https://artists.soundcloud.com).
Without credentials Fastcloud still runs in demo mode or falls back to
30-second previews where the public API allows.

## Differences vs Fastpotify (Spotify)

SoundCloud's API has no Spotify Connect equivalent (playback is always
local), no albums/podcasts/lyrics, and app registration requires an Artist
Pro account.

## License

MIT — see [LICENSE](LICENSE).
