# Releasing and auto-update

## Publish a release

1. Bump `version` in `Cargo.toml` (for example `1.3.0`) and commit.
2. `git tag v1.3.0 && git push origin v1.3.0`

`.github/workflows/release.yml` then runs the tests, builds the release binary, builds the
installer with Inno Setup (`installer/poof.iss`) and publishes it as a GitHub release named
`<name>-<version>-setup.exe`. The tag must equal `v<Cargo.toml version>` or the build stops.
"Run workflow" in the Actions tab builds the installer without publishing it.

Installs go to `%LOCALAPPDATA%\Programs\Poof` and need no administrator rights. User data lives in
`%LOCALAPPDATA%\Poof` and is never touched by installing or updating.

## How auto-update works (`src/updater/`)

- The workflow bakes the repository name into the binary (`POOF_UPDATE_REPO`). Local builds
  don't have it, so they never update.
- 45 seconds after start, then every 6 hours, a background thread asks GitHub for the latest
  release. Drafts, pre-releases and tags that are not plain `x.y.z` are ignored.
- A newer release's installer is downloaded to `%LOCALAPPDATA%\Poof\updates`, checked against the
  size and SHA-256 that GitHub reports, and only then marked ready. The download URL must belong
  to the same repository.
- When the browser closes, the installer is started silently (`/VERYSILENT`) and detached. It
  waits a few seconds for the browser to exit, replaces the files and does not relaunch the app.
  The next start runs the new version.
- `POOF_NO_UPDATE=1` disables the checker.

## Notes

- Never change `AppId` in `installer/poof.iss`: it is how an update finds the existing install.
- The updater needs the `sha2` crate in `Cargo.toml`.
- The repository must be public (or the release assets reachable without a token) for clients to
  download updates.
