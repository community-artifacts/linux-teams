# teams-for-linux (firefoxpwa edition)

Standalone Linux app that runs Microsoft Teams through [firefoxpwa](https://github.com/filips123/PWAsForFirefox), with a Tauri tray menu. It ships as one AppImage that bundles firefoxpwa, its connector and a pre-patched Firefox runtime, so end users install nothing else.

## Repository layout
- `src-tauri/` — Tauri (Rust) tray launcher, no window. Uses a private firefoxpwa dir (`~/.local/share/teams-for-linux`).
- `profile/boot.sys.mjs` — privileged script written into each Firefox profile (`chrome/user/`). It polls the window title for `(N)` and reports the unread count to the tray over `127.0.0.1:47653`, grants Teams notifications, and reads the signed-in email. No extension, so no signing needed.
- `scripts/build-appimage.sh` — builds the tray with `cargo tauri build`, then injects firefoxpwa, the connector and the Firefox runtime into the AppImage (`dist/`).
- `ui/` — placeholder frontend required by Tauri (the app has no window).

Each launch rewrites the profile's `user.js`, `userChrome.css` (hides toolbar/menus, window buttons on the right) and `policies.json`.

## Tray menu
Show Teams · Accounts (switch / add / remove; names follow the signed-in email; single window; the default account is protected) · Launch on login · Start minimized (tray only) · Open logs · Reset all accounts · Quit.

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
6. **GNOME only:** the *AppIndicator and KStatusNotifierItem Support* extension, or the tray icon will not appear.

## Build and run the AppImage
```bash
./scripts/build-appimage.sh
./dist/Teams_for_Linux-x86_64.AppImage
```
The build needs network access (appimagetool and the Teams icon are downloaded). On first run the app copies the bundled runtime to `~/.local/share/teams-for-linux/runtime`, installs the Teams site and opens it; sign in once.

Stop any running copy before rebuilding; otherwise the old process keeps the single-instance lock and the new one exits silently (`pkill teams-tray`).

## Run from source (development)
Without the AppImage there is no bundled runtime, so the app uses `firefoxpwa` from `PATH` (override with `FFPWA_BIN`):
```bash
firefoxpwa runtime install          # once; the app uses ~/.local/share/teams-for-linux, so copy it:
mkdir -p ~/.local/share/teams-for-linux && cp -a ~/.local/share/firefoxpwa/runtime ~/.local/share/teams-for-linux/
cd src-tauri && cargo run
```
Pass `--account <site-id>` to open a specific account, `--autostart` behaves like a login start.

## Files and logs
| Path | Content |
| --- | --- |
| `~/.local/share/teams-for-linux/` | firefoxpwa data, runtime, profiles |
| `.../labels.json` | account names (email) |
| `.../teams-for-linux.log` | Firefox/Teams output (`Open logs` in the tray) |
| `~/.local/share/applications/FFPWA-<site>.desktop` | dock/pinning entry per account |
| `~/.config/autostart/` | "Launch on login" entry |

To start over, use **Reset all accounts…** in the tray, or delete `~/.local/share/teams-for-linux`.

## Troubleshooting
- **App starts and exits immediately** — another instance is running: `pkill teams-tray`, then relaunch.
- **No tray icon** — enable the AppIndicator GNOME extension.
- **Unread badge stuck** — it depends on Teams putting `(N)` in the window title; look for `[tray] title=...` lines in the log.
- **Build fails at `firefoxpwa` copy** — the paths in `scripts/build-appimage.sh` (`/usr/bin/firefoxpwa`, `/usr/libexec/firefoxpwa-connector`, `/usr/share/firefoxpwa/userchrome`) must exist; adjust them if your package installs elsewhere.

## Known limits
- Wayland: the tray can't raise an existing window ("Show Teams" does nothing if it is already running; use the dash icon).
- Switching accounts closes the previous account's process, so it gets no notifications.
- The unread badge reports only the active account.
- Screen-share picker comes from the desktop portal and can't be customised.
