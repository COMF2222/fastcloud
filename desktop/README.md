# Fastcloud desktop

This is the active React 19 and Tauri 2 application. Playback, SoundCloud API
access, cache, and settings run through the Rust backend in `src-tauri/` and
the shared modules in `../src/`.

```sh
npm ci
npm run tauri dev
```

`npm run build` checks TypeScript and bundles the interface. Browser preview
(`npm run dev`) uses simulated data; test real playback in Tauri.

The tagged Windows release downloads and bundles the pinned CLAP model and
worker in GitHub Actions. For a local signed build, prepare the resources with
`../tools/prepare_private_clap.ps1`, set `TAURI_SIGNING_PRIVATE_KEY` to the
private updater key path, then run `npm run tauri:private -- --bundles nsis`.
The regular `npm run tauri build` excludes CLAP.
