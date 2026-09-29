# Releasing Fastcloud

Tagged Windows releases are built by [CI](../.github/workflows/ci.yml). The
workflow downloads the pinned CLAP model, builds its Windows worker, bundles both
with Tauri, signs the NSIS installer for the updater, and publishes the installer,
signature, checksums, and `latest.json` on GitHub Releases. The model and worker
remain ignored in Git; only the installer contains them.

The CLAP model is derived from [LAION's Apache 2.0 model](https://huggingface.co/laion/larger_clap_music_and_speech).
Keep the model attribution and bundled third-party licenses with future releases.

## Signing key

`TAURI_SIGNING_PRIVATE_KEY` is a GitHub Actions repository secret. Its public key
is in `desktop/src-tauri/tauri.conf.json`. The private key must also be backed up
outside the repository. Losing it prevents existing installations from accepting
future updates. Never commit it or put it in a release asset.

## Before tagging

1. Set the same version in `Cargo.toml`, `desktop/package.json`,
   `desktop/src-tauri/Cargo.toml`, and `desktop/src-tauri/tauri.conf.json`.
   Update both lockfiles.
2. Run `npm ci` and `npm run build` from `desktop/`. Run
   `cargo fmt --manifest-path desktop/src-tauri/Cargo.toml -- --check` and
   `cargo check --manifest-path desktop/src-tauri/Cargo.toml --locked`.
3. Review the files being committed. Keep build output, logs, credentials,
   screenshots, and local session data out of Git.
4. Commit and push the source. Create and push a matching annotated tag, for
   example `git tag -a v0.1.2 -m "Fastcloud v0.1.2"` followed by
   `git push origin v0.1.2`.

The tag triggers the Windows release job. Check that the GitHub Release has
`Fastcloud_VERSION_x64-setup.exe`, its `.sig`, `latest.json`, and `checksums.txt`.
Install the new version and check Settings → General. Version 0.1.1 predates
the updater and must be replaced manually once; later signed releases can be
installed with the in-app button.
