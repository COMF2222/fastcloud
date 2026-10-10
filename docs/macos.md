# macOS builds and testing

Fastcloud's CI tests the native app on both Apple Silicon (`aarch64`) and Intel
(`x86_64`) runners. Tagged releases package separate DMGs for macOS 13 or newer:

- `Fastcloud_VERSION_macos-aarch64.dmg`: M1, M2, M3 and newer Apple Silicon.
- `Fastcloud_VERSION_macos-x86_64.dmg`: Intel Macs.

Open the DMG, drag Fastcloud to Applications and open that copy. These initial
builds use an ad-hoc signature rather than an Apple Developer certificate and
notarization. For the first launch, macOS may require **System Settings → Privacy
& Security → Open Anyway**. Do not disable Gatekeeper globally. This signing
mode is documented by [Tauri](https://v2.tauri.app/distribute/sign/macos/).

No new GitHub secrets are required. Release builds reuse `FASTCLOUD_SERVER_URL`,
`TAURI_SIGNING_PRIVATE_KEY` and the existing Discord application variable.
The publisher waits for both macOS packages and Windows validation before
publishing any of the new release. Both signed updater archives are included
in the static manifests under `darwin-aarch64` and `darwin-x86_64`.

Each architecture includes its native offline CLAP worker and the same pinned
model weights as Windows. ONNX Runtime 1.22.0 is used for macOS because it
supports both Intel and Apple Silicon on macOS 13. Component updates use the
existing content-addressed cache; native workers retain executable permissions.
The packaging job tests text inference before bundling the worker.

On a first real Mac, verify sign-in and callback, playback and seeking,
crossfade, tray/menu-bar reopening, minimize/maximize/close and dragging,
lyrics, imports, messages, GIF/font files and updates. Compilation and fixture
tests do not replace checking audio output and window behavior on hardware.

To report a problem, include architecture, macOS version, Fastcloud version,
the action that failed and the visible error. Do not send OAuth tokens.
