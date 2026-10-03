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

Lettered hotfix `0.2.1a` uses the valid SemVer version `0.2.1-a` in manifests
and lockfiles and the GitHub tag `v0.2.1a`. CI accepts this compact tag and uses
the actual tag in `latest.json` download URLs. The release is published as a
normal latest release so the existing updater endpoint continues to work.

1. Set the same version in `Cargo.toml`, `desktop/package.json`,
   `desktop/src-tauri/Cargo.toml`, and `desktop/src-tauri/tauri.conf.json`.
   Update both lockfiles.
   Add that version to `docs/release-notes.json`: a short title and user-facing
   changes in both `ru` and `en`. CI rejects a tagged release without its notes
   before packaging. `python tools/prepare_release_notes.py --check` validates
   every entry without building an installer.
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

## Immediate release notifications

The last step of CI posts the published version to the approval broker's
`/v1/updates/published` endpoint. The broker persists the version and pushes it
over `/v1/updates/events` to running clients. A reconnect receives the last
published version. Clients still use the signed GitHub manifest to decide
whether an update can be installed; the signal never installs anything.
Periodic checks every 30 minutes remain as a fallback.

## Website release history

CI writes `desktop/release-notes.md` from `docs/release-notes.json` and adds it
to the GitHub release body. The first link is the full Windows installer; the
description then contains readable Russian and English notes. A hidden
`fastcloud-notes:v1` JSON comment carries the same description for the website.
Do not remove that comment when editing a release body.

The website reads published releases from the public GitHub API. Publishing a
new client release updates its history without deploying the website again or
adding a secret to the website repository. An open history page checks again
every five minutes and when returning to the tab. English text is authored
alongside the Russian description; it is not inferred from commit messages.

The many `clap-*.gz` assets are intentional, content-addressed update files.
Existing clients request these exact filenames. Keep them, signatures and
manifests in every release; removing or combining them breaks component updates.
Users only need the full `Fastcloud_*_x64-setup.exe` installer for manual setup.

One-time setup:

1. Generate a random notification token, for example with
   `python -c "import secrets; print(secrets.token_urlsafe(32))"`.
2. Add it to the backend's existing `.env` as
   `FASTCLOUD_RELEASE_NOTIFY_TOKEN`. Preserve all existing settings and volumes.
3. Add the same value to the **desktop** repository's GitHub Actions secrets,
   also named `FASTCLOUD_RELEASE_NOTIFY_TOKEN`.
4. Deploy the backend changes before releasing a client with push support.
   Existing clients get this behavior only after installing that client release.

Without the new GitHub secret CI publishes normally and reports that push was
skipped. If the secret is present but delivery fails, the notification step
reports a failure after three attempts; the published release remains available
through the normal updater. It can be announced again with the same version.
