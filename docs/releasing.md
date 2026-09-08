# Releasing Fastcloud

GitHub Releases are built by [the CI workflow](../.github/workflows/ci.yml).
The release is triggered by a tag; installers must never be uploaded by hand
from an unverified local build.

## Before tagging

1. Make sure the repository URL is final everywhere (`Cargo.toml`, packaging,
   README, and the update checker when it exists).
2. Set the same version in `Cargo.toml`, `packaging/PKGBUILD`,
   `packaging/Casks/fastcloud.rb`, and the fallback value in
   `packaging/windows/installer.iss`.
3. Run the full local checks:

   ```sh
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features --locked -- -D warnings
   cargo test --all-targets --locked
   cargo build --release --locked
   ```

4. Review `git status --short --ignored`. Build output, logs, local screenshots,
   credentials, editor settings, and session notes must not be committed.
5. Commit and push the release-ready source before creating the tag.

## Create the release

For version `0.1.0`:

```sh
git tag -a v0.1.0 -m "Fastcloud v0.1.0"
git push origin v0.1.0
```

The tag starts CI on Linux, macOS, and Windows. After every job succeeds, the
workflow creates a GitHub Release containing:

- a Windows installer and portable zip;
- an Apple Silicon macOS DMG and `.app` archive;
- a Debian package and portable Linux tarball;
- `checksums.txt`.

Download one artifact from each platform and smoke-test it before announcing
the release. A build produced by CI is not the same thing as a tested install.

## After publishing

- Replace generated release notes with a short human-readable summary of new
  behavior and fixes.
- Put the final SHA-256 from `checksums.txt` into the Homebrew cask instead of
  `sha256 :no_check` before publishing a tap.
- Mark prereleases as prereleases. Do not point package managers or a website
  at assets that do not exist yet.
- When a stable website exists, update its download links only after the
  GitHub Release is live.
