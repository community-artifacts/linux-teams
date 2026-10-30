# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0] - 2026-10-30

First release: Microsoft Teams running through firefoxpwa, packaged as a single AppImage with a tray menu.

### Added
- Tauri tray launcher with no window, bundling firefoxpwa, its connector and a pre-patched Firefox runtime in one AppImage.
- Tray menu: Show Teams, Accounts, Launch on login, Start minimized (tray only), Open logs, Reset all accounts, Quit.
- Single-window multi-account support: switch, add and remove accounts. The default account is protected.
- Account names follow the signed-in email address automatically and return to "Default" after a reset. Adding an account no longer asks for a name.
- Unread badge: the tray icon and tooltip follow the `(N)` count in the Teams window title.
- Teams desktop notifications are allowed without a permission prompt.
- Links that leave Teams open in the default browser.
- Dock icon and pinning through a per-account `.desktop` entry.
- Clean window: no address bar or Firefox menus, window buttons on the right, and the telemetry and "Open previous tabs?" bars hidden.
- GitHub Actions workflow that builds the AppImage and publishes it with a `.sha256` file when a `v*` tag is pushed.
- README with prerequisites, build, run, troubleshooting and known limits.

### Fixed
- Quit now stops the Teams process.
- Cookie partitioning and tracking protection relaxed to address the embedded calendar sign-in errors. Not yet confirmed as fixed.
- Screen-share previews re-enabled.
- AppImage repacking: the app folder is made readable for all users before packing.

### Known limits
- On Wayland the tray can't raise an existing window; use the dash icon.
- Switching accounts closes the previous account's process, so it gets no notifications.
- The unread badge reports only the active account.
- The tray icon needs the AppIndicator extension on GNOME.

[0.1.0]: https://github.com/community-artifacts/linux-teams/releases/tag/v0.1.0
