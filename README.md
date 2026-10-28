# teams-for-linux (firefoxpwa edition)

Standalone Linux app running Microsoft Teams through [firefoxpwa](https://github.com/filips123/PWAsForFirefox), with a tray menu.

- `src-tauri/` — Tauri tray launcher (no window). Installs/launches the Teams site in a private firefoxpwa user dir (`~/.local/share/teams-for-linux`), menu: Show, Launch on login, Open logs, Reset profile, Quit.
- `extension/` — WebExtension that reports the unread count to the tray over `127.0.0.1:47653`.
- `scripts/` — packaging.

## Status
Scaffold. Not yet done: bundling firefoxpwa + Firefox runtime into the AppImage, preinstalling the extension, Hide (Wayland), presence shortcuts.
