# Teams for Linux (firefoxpwa edition)

Microsoft Teams as a standalone Linux app. It runs Teams in a clean Firefox window (no address bar or menus) through [firefoxpwa](https://github.com/filips123/PWAsForFirefox), with a tray icon. Everything it needs is inside one AppImage.

Building from source: see [DEVELOPMENT.md](DEVELOPMENT.md). Version history: [CHANGELOG.md](CHANGELOG.md).

## Install
1. Download `Teams_for_Linux-x86_64.AppImage` (and the `.sha256` file) from the [latest release](https://github.com/community-artifacts/linux-teams/releases/latest).
2. Optionally verify it: `sha256sum -c Teams_for_Linux-x86_64.AppImage.sha256`.
3. Make it executable and run it:
   ```bash
   chmod +x Teams_for_Linux-x86_64.AppImage
   ./Teams_for_Linux-x86_64.AppImage
   ```

Requirements: x86_64 Linux, `libfuse2` (to run AppImages) and `zenity` (for the tray's dialogs). On Ubuntu: `sudo apt install libfuse2 zenity`.

On GNOME the tray icon needs the **AppIndicator and KStatusNotifierItem Support** extension. Without it the app still works, but there is no tray menu.

## First run
The first start sets up its own Firefox profile under `~/.local/share/teams-for-linux` and opens Teams. Sign in once; the session is kept. A "Microsoft Teams" entry is added to your applications, so you can pin it to the dock.

## Tray menu
| Item | What it does |
| --- | --- |
| Show Teams | Opens Teams if it is closed. |
| Accounts | Switch between accounts, **Add account…** or **Remove account…**. |
| Launch on login | Starts the app when you log in. |
| Start minimized (tray only) | At login, start only the tray; Teams opens on **Show Teams**. |
| Open logs | Opens the log file. |
| Reset all accounts… | Deletes all profiles and logins and starts fresh. |
| Quit | Closes Teams and the tray. |

The tray icon changes when you have unread messages.

## Accounts
- Only one Teams window is open at a time; switching accounts closes the current one and opens the other.
- Each account has its own login and data.
- Accounts are named after the email address they sign in with. A new account shows as "New account" until you sign in.
- The first account ("Default") can't be removed. Use **Reset all accounts…** to wipe everything.

## Notifications and links
- Teams notifications are allowed automatically.
- Links to other sites open in your default browser. Microsoft sign-in and Teams pages stay in the app.

## Where files are kept
| Path | Content |
| --- | --- |
| `~/.local/share/teams-for-linux/` | Profiles, logins, logs, account names |
| `~/.local/share/applications/FFPWA-*.desktop` | Dock and menu entry per account |
| `~/.config/autostart/teams-for-linux.desktop` | "Launch on login" |

To uninstall, delete the AppImage and these paths.

## Troubleshooting
- **Nothing happens when I start it** — it is probably already running (single instance). Use the tray icon or the dash icon, or run `pkill teams-tray` and start again.
- **No tray icon** — enable the AppIndicator extension on GNOME.
- **Unread badge doesn't update** — it only reflects the account that is open.
- **Calendar asks me to sign in again** — report it with a screenshot; this is under investigation.

## Known limits
- On Wayland the tray can't raise an already-open window: "Show Teams" does nothing in that case. Use the dash icon.
- Switching accounts closes the previous one, so it gets no notifications while inactive.
- The screen-share picker comes from your desktop and can't be customised.
