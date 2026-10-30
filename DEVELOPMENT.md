# Development

How to build, run and release the app from source. For using the app, see the [README](README.md).

## Repository layout
- `src-tauri/` — Tauri (Rust) tray launcher, no window. Uses a private firefoxpwa dir (`~/.local/share/teams-for-linux`).
- `profile/boot.sys.mjs` — privileged script written into each Firefox profile (`chrome/user/`). It polls the window title for `(N)` and reports the unread count to the tray over `127.0.0.1:47653`, grants Teams notifications, and reads the signed-in email. No extension, so no signing needed.
- `scripts/build-appimage.sh` — builds the tray with `cargo tauri build`, then injects firefoxpwa, the connector and the Firefox runtime into the AppImage (`dist/`).
- `ui/` — placeholder frontend required by Tauri (the app has no window).
- `install.sh` — one-line installer for end users (prerequisites + latest release AppImage). Test it without a release with `TEAMS_BASE_URL=file:///dir/with/appimage INSTALL_DIR=/tmp/x ./install.sh --no-run`.
- `.github/workflows/release.yml` — release build.

Each launch rewrites the profile's `user.js`, `userChrome.css` (hides toolbar/menus, window buttons on the right) and `policies.json`.

## Prerequisites
Tested on Ubuntu with GNOME on Wayland (x86_64).

1. **System packages**
   ```bash
   sudo apt install build-essential curl wget file patchelf zenity libfuse2 \
     libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
   ```
2. **Rust and the Tauri CLI**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   cargo install tauri-cli --version "^2"
   ```
3. **firefoxpwa**, installed system-wide so that `/usr/bin/firefoxpwa`, `/usr/libexec/firefoxpwa-connector` and `/usr/share/firefoxpwa/userchrome` exist. Follow the [official install guide](https://pwasforfirefox.filips.si/user-guide/installation/) (a `.deb` or the apt repository).
4. **A Firefox runtime for firefoxpwa** (the build copies it into the AppImage):
   ```bash
   firefoxpwa runtime install      # installs to ~/.local/share/firefoxpwa/runtime
   ```
   Use `SRC_RUNTIME=/path/to/runtime` if it lives elsewhere.
5. **appimagetool** is downloaded to `~/.cache/teams-for-linux/appimagetool` on first build. Override with `APPIMAGETOOL=/path/to/appimagetool`.

## Build the AppImage
```bash
./scripts/build-appimage.sh
./dist/Teams_for_Linux-x86_64.AppImage
```
The build needs network access (appimagetool and the Teams icon are downloaded). On first run the app copies the bundled runtime to `~/.local/share/teams-for-linux/runtime`, installs the Teams site and opens it.

Stop any running copy before rebuilding (`pkill teams-tray`); otherwise the old process keeps the single-instance lock and the new one exits silently.

## Run from source
Without the AppImage there is no bundled runtime, so the app uses `firefoxpwa` from `PATH` (override with `FFPWA_BIN`). This path is untested:
```bash
firefoxpwa runtime install
mkdir -p ~/.local/share/teams-for-linux && cp -a ~/.local/share/firefoxpwa/runtime ~/.local/share/teams-for-linux/
cd src-tauri && cargo run
```
Flags: `--account <site-id>` opens a specific account; `--autostart` behaves like a login start.

## Releases
Pushing a tag such as `v0.1.0` runs `.github/workflows/release.yml`. It builds the AppImage on `ubuntu-22.04` (firefoxpwa pinned by `FIREFOXPWA_VERSION`) and publishes it with a `.sha256` file to GitHub Releases. Starting the workflow manually only uploads the AppImage as a build artifact.

To release:
1. Bump `version` in `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.
2. Add an entry to `CHANGELOG.md`.
3. Commit, then `git tag -a vX.Y.Z -m vX.Y.Z && git push origin main vX.Y.Z`.

## Debugging
- Logs: `~/.local/share/teams-for-linux/teams-for-linux.log` (Firefox and Teams output, `[tray]` lines from `boot.sys.mjs`).
- Unread badge stuck: look for `[tray] title=...` lines; it depends on Teams putting `(N)` in the window title.
- Build fails at the firefoxpwa copy: the paths in `scripts/build-appimage.sh` must exist; adjust them if your package installs elsewhere.
