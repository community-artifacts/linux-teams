# teams-for-linux (firefoxpwa edition)

Standalone Linux app running Microsoft Teams through [firefoxpwa](https://github.com/filips123/PWAsForFirefox), with a tray menu.

- `src-tauri/` — Tauri tray launcher (no window). Uses a private firefoxpwa dir (`~/.local/share/teams-for-linux`). Tray menu: Show Teams, Accounts (switch / add / remove; names follow the signed-in email; single window, default account is protected), Launch on login, Start minimized (tray only), Open logs, Reset all accounts, Quit.
- `profile/boot.sys.mjs` — privileged script written into each profile (`chrome/user/`); polls the window title for `(N)` and reports the unread count to the tray over `127.0.0.1:47653`. No extension needed, so no signing.
- `scripts/build-appimage.sh` — builds the tray with `cargo tauri build`, then injects firefoxpwa, the connector and a pre-patched Firefox runtime into the AppImage (`dist/`).

Profiles get a `user.js`, `userChrome.css` (hides toolbar/menus, window buttons on the right) and a runtime `policies.json` (Teams notifications allowed, telemetry off) on every launch.

## Build
Requires `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libfuse2 patchelf zenity`, `cargo install tauri-cli`, `appimagetool`, and firefoxpwa installed with a runtime at `~/.local/share/firefoxpwa/runtime`.

## Known limits
- Wayland: the tray can't raise an existing window ("Show Teams" does nothing if it is already running; use the dash icon).
- Switching accounts closes the previous account's process, so it gets no notifications.
- Unread badge depends on Teams putting `(N)` in the window title; check the log lines `[tray] title=...` if it doesn't change.
- GNOME needs the AppIndicator extension for the tray icon.
