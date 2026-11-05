# TODO

- [ ] **Short install URL.** Put the installer behind a public URL shortener so the README command is easy to type and share, e.g. `curl -fsSL https://<short-url> | bash` instead of the long `raw.githubusercontent.com/community-artifacts/linux-teams/main/install.sh`.
  - Pick a service that supports permanent redirects and edits (e.g. a custom domain, or a shortener with a stable alias), and point it at the raw `install.sh` URL.
  - Check that `curl -fsSL` follows the redirect and that the service doesn't show an interstitial page, which would break `| bash`.
  - Update the Install section of `README.md` (and the header comment in `install.sh`) with the short URL; keep the full URL as a fallback.
  - Consider pinning to a release tag instead of `main` so the installer script matches the AppImage version.

- [ ] **Unit tests.** Add Rust unit tests for the logic in `src-tauri/src/main.rs` that doesn't need a desktop (account/label handling, `--account` argument parsing, unread-count parsing, `labels.json` read/write) and a syntax/regex check for `profile/boot.sys.mjs`. Run them with `cargo test`.

- [ ] **Install tests on Ubuntu desktop LTS.** In GitHub Actions, run `install.sh` on a clean Ubuntu **24.04 LTS** and **26.04 LTS** desktop image (matrix), then check that the AppImage is installed, the checksum passes and the app starts under a virtual display (`xvfb`).
  - Note: 26.10 is an interim release, not an LTS; the current LTS after 24.04 is 26.04. Confirm which one is wanted.
  - GitHub-hosted runners may not offer 26.04 yet; if not, use a container image or a self-hosted runner.

- [ ] **Branching and pull requests.**
  - Create a `develop` branch. All improvements and fixes go to `develop` through pull requests, never straight to `main`.
  - Releases are a pull request from `develop` to `main`; the release workflow then runs from the `v*` tag as today.
  - Add a workflow that runs the unit tests on every pull request (to `develop` and to `main`), and make it a required check.
  - Protect `develop` and `main` in the repository settings: require pull requests and passing checks, block force-pushes.
  - Update `DEVELOPMENT.md` with the branching and release steps once this is in place.
