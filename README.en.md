<p align="center">
  <img src="desktop/src/assets/fastcloud-logo.png" width="88" height="88" alt="Fastcloud logo">
</p>

<h1 align="center">Fastcloud — SoundCloud for PC</h1>

<p align="center"><strong>Your music. Your player. Your style.</strong></p>

<p align="center">
  <a href="https://github.com/COMF2222/fastcloud/releases/latest"><strong>Download for Windows</strong></a> ·
  <a href="https://fastcloud.comf.workers.dev/en/">Website and demo</a> ·
  <a href="https://fastcloud.comf.workers.dev/en/changes">Release notes</a> ·
  <a href="README.md">Русский</a>
</p>

<p align="center">
  <a href="https://github.com/COMF2222/fastcloud/releases/latest"><img src="https://img.shields.io/github/v/release/COMF2222/fastcloud?color=ff5519" alt="Latest release"></a>
  <img src="https://img.shields.io/badge/Windows-x64-242424" alt="Windows x64">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-242424" alt="MIT license"></a>
</p>

**Fastcloud is a free SoundCloud desktop client for Windows** with lyrics,
personal recommendations and a customisable interface. Listen to your
favourite tracks, browse your library, import Spotify playlists and choose
how your player looks — from the compact Airwave mini player to a full-screen
view with artwork and lyrics.

![SoundCloud for PC: Fastcloud home with My Wave, the library and lyrics](assets/screenshots/home.png)

<p align="center"><sub>Your library, My Wave and lyrics in one window. Captured from the installed app.</sub></p>

## What you get

| Feature | What it does |
| --- | --- |
| **Your library** | SoundCloud likes, albums, artists and playlists, plus pins and listening history. |
| **My Wave** | Music recommendations based on your taste, likes and history, respecting your dislikes. Track stations help you find similar music. |
| **Lyrics** | Search across LRCLIB, Genius and lyrics.ovh, with synchronized lines when available. Open lyrics in the sidebar or full player. |
| **Your own style** | Colours, fonts, text sizes, panel opacity, custom images and animated backgrounds. Background-only mode hides the interface. |
| **Sound and controls** | Equalizer, balance, playback speed, A–B repeat, queue editing, media keys and keyboard shortcuts. |
| **Imports and integrations** | Import Spotify likes and playlists, import from Yandex Music and share your listening activity with Discord Rich Presence. |
| **Everyday convenience** | Airwave mini player, offline downloads, Russian and English UI, and in-app updates. |

<details>
<summary><strong>Catalog and search — view screenshots</strong></summary>

Explore albums and artists, pick a genre and discover new music in the catalog.

![Fastcloud catalog with albums, artists and music genre filters](assets/screenshots/catalog.png)

Find tracks, playlists, albums and artists without leaving your player.

![SoundCloud search in Fastcloud with genre filters and track results](assets/screenshots/search.png)

</details>

## Music and lyrics on the big screen

Open the full player to focus on your music, with artwork, playback controls
and lyrics side by side.

![Fastcloud full player with artwork, synchronized lyrics and a custom background](assets/screenshots/lyrics.png)

### Airwave mini player

Make room for your other windows by switching to Airwave with the player
button or **Ctrl+M**.

<p align="center"><img src="assets/screenshots/mini-player.png" width="420" alt="Compact Airwave mini player with track information and playback controls"></p>

<details>
<summary><strong>Make it your own — explore appearance settings</strong></summary>

Adjust text and panel colours, interface size and opacity independently.

![Fastcloud text colours, interface sizes and panel opacity settings](assets/screenshots/appearance.png)

Set heading opacity and lyrics size to your liking.

![Heading opacity and lyrics display settings](assets/screenshots/surfaces.png)

Choose your background and adjust its blur, dimming and visibility.

![Custom background, blur and dimming settings in Fastcloud](assets/screenshots/background.png)

Performance profiles and a custom font help adapt the interface to your computer.

![Fastcloud performance profiles and font selection](assets/screenshots/performance-fonts.png)

</details>

## Download and start listening

1. Open the [latest release](https://github.com/COMF2222/fastcloud/releases/latest)
   and download **`Fastcloud_…_x64-setup.exe`**, the full Windows x64 installer.
2. Install Fastcloud and sign in to your SoundCloud account in your browser.
3. Open your library, choose a track or start My Wave.

Install new versions from within the app. The **Read changes** button opens
the release notes before you update.

Want to see the interface first? [Try the browser player](https://fastcloud.comf.workers.dev/en/#interactive-player)
with its demo library.

## Before you install

### Can I listen to SoundCloud in Russia?

After sign-in, the catalog, artwork and audio are relayed through the Fastcloud
server, so your computer does not need a direct connection to SoundCloud
for playback. Access to SoundCloud's browser sign-in page depends on your network.

### Do I need Artist Pro or my own API keys?

Neither is required for the normal Fastcloud sign-in flow. Use your own
SoundCloud account. Go+ and individual recordings follow SoundCloud's access
rules; some tracks may only be available as previews.

### How does Spotify import work?

In integration settings, export your likes or a playlist with Exportify, then
select the CSV/ZIP in Fastcloud. Spotify account-data files are also supported.
Fastcloud searches for matching recordings on SoundCloud and imports the tracks
it finds. [Read about the importer](docs/spotify-import.md).

### Which platforms are supported?

Ready-to-use installers are available for **Windows x64**, with Russian and
English UI. Linux and macOS packages are not currently released.

## Help and contribute

- [Documentation and installation](https://fastcloud.comf.workers.dev/en/docs)
- [Frequently asked questions](https://fastcloud.comf.workers.dev/en/faq)
- [Report a bug or request a feature](https://github.com/COMF2222/fastcloud/issues)
- [Source code, builds and architecture](docs/development.md)
- [Release preparation](docs/releasing.md)

Fastcloud is built with **React, TypeScript, Tauri and Rust** and is open source
under the [MIT license](LICENSE). Settings and cache stay on your computer;
account tokens use the operating system credential store.

If Fastcloud is useful to you, support it with a **GitHub star** and help other
listeners discover the project.

Fastcloud is an independent, unofficial client and is not affiliated with SoundCloud.
