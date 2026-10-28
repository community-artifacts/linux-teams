#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    process::Command,
    thread,
};

use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle,
};

const TEAMS_MANIFEST: &str = "https://teams.cloud.microsoft/manifest.json";
const TEAMS_DOCUMENT: &str = "https://teams.cloud.microsoft/";
const TEAMS_START: &str = "https://teams.cloud.microsoft/?clientType=pwa";
/// Local port the bundled extension reports unread counts to.
const UNREAD_PORT: u16 = 47653;

fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("teams-for-linux")
}

fn log_path() -> PathBuf {
    data_dir().join("teams-for-linux.log")
}

fn autostart_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autostart/teams-for-linux.desktop")
}

/// Runs firefoxpwa against the private user dir so the system install is untouched.
fn ffpwa(args: &[&str]) -> Command {
    let bundle = std::env::var("APPDIR").ok().map(|d| PathBuf::from(d).join("usr/lib/teams-for-linux"));
    let bin = match &bundle {
        Some(b) => b.join("firefoxpwa"),
        None => PathBuf::from(std::env::var("FFPWA_BIN").unwrap_or_else(|_| "firefoxpwa".into())),
    };
    let mut cmd = Command::new(bin);
    cmd.env("FFPWA_USERDATA", data_dir()).args(args);
    if let Some(b) = bundle {
        cmd.env("FFPWA_SYSDATA", b.join("sysdata")).env("FFPWA_EXECUTABLES", &b);
    }
    cmd
}

/// Finds the installed Teams site ID in the private firefoxpwa config.
fn site_id() -> Option<String> {
    let raw = fs::read_to_string(data_dir().join("config.json")).ok()?;
    let cfg: serde_json::Value = serde_json::from_str(&raw).ok()?;
    cfg["sites"].as_object()?.iter().find_map(|(id, site)| {
        (site["config"]["document_url"] == TEAMS_DOCUMENT).then(|| id.clone())
    })
}

fn ensure_installed() -> Result<String, String> {
    if let Some(id) = site_id() {
        return Ok(id);
    }
    fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
    // The AppImage ships a pre-patched Firefox runtime. It must live on a writable path
    // (firefoxpwa re-patches it on install), so copy it once instead of downloading.
    if let Ok(appdir) = std::env::var("APPDIR") {
        let runtime = data_dir().join("runtime");
        if runtime.is_symlink() {
            let _ = fs::remove_file(&runtime);
        }
        if !runtime.exists() {
            let src = PathBuf::from(appdir).join("usr/lib/teams-for-linux/runtime");
            let status = Command::new("cp").arg("-a").arg(src).arg(&runtime).status();
            if !matches!(status, Ok(s) if s.success()) {
                return Err("failed to copy the bundled Firefox runtime".into());
            }
        }
    }
    let status = ffpwa(&[
        "site", "install", TEAMS_MANIFEST,
        "--document-url", TEAMS_DOCUMENT,
        "--start-url", TEAMS_START,
        "--name", "Microsoft Teams",
        "--no-system-integration",
    ])
    .status()
    .map_err(|e| format!("cannot run firefoxpwa: {e}"))?;
    if !status.success() {
        return Err(format!("firefoxpwa site install failed: {status}"));
    }
    site_id().ok_or_else(|| "Teams site missing from firefoxpwa config after install".into())
}

/// Launching an already running site focuses its existing window.
fn launch() {
    match ensure_installed() {
        Ok(id) => {
            apply_profile_tweaks(&id);
            install_desktop_entry(&id);
            let log = fs::OpenOptions::new().create(true).append(true).open(log_path());
            let mut cmd = ffpwa(&["site", "launch", &id]);
            if let Ok(log) = log {
                if let Ok(err) = log.try_clone() {
                    cmd.stdout(log).stderr(err);
                }
            }
            if let Err(e) = cmd.spawn() {
                eprintln!("launch failed: {e}");
            }
        }
        Err(e) => eprintln!("{e}"),
    }
}

const USER_JS: &str = r#"user_pref("toolkit.legacyUserProfileCustomizations.stylesheets", true);
user_pref("ui.key.menuAccessKeyFocuses", false);
user_pref("media.navigator.permission.disabled", true);
user_pref("permissions.default.microphone", 1);
user_pref("permissions.default.camera", 1);
user_pref("permissions.default.desktop-notification", 1);
user_pref("privacy.webrtc.legacyGlobalIndicator", false);
user_pref("privacy.webrtc.hideGlobalIndicator", true);
user_pref("browser.shell.checkDefaultBrowser", false);
"#;

const USER_CHROME_CSS: &str = "\
#nav-bar, #toolbar-menubar, #PersonalToolbar, #sidebar-main { visibility: collapse !important; }
#TabsToolbar > *:not(.titlebar-buttonbox-container) { visibility: collapse !important; }
#TabsToolbar { display: flex !important; }
#TabsToolbar .titlebar-buttonbox-container { order: 100 !important; margin-inline-start: auto !important; }
";

/// Hides the toolbar/menus and silences prompts; rewritten on every launch so updates apply.
fn apply_profile_tweaks(site: &str) {
    let Ok(raw) = fs::read_to_string(data_dir().join("config.json")) else { return };
    let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&raw) else { return };
    let Some(profile) = cfg["sites"][site]["profile"].as_str() else { return };
    let dir = data_dir().join("profiles").join(profile);
    let _ = fs::create_dir_all(dir.join("chrome"));
    let _ = fs::write(dir.join("user.js"), USER_JS);
    let _ = fs::write(dir.join("chrome/userChrome.css"), USER_CHROME_CSS);
}

/// Gives the window a dock/taskbar identity: GNOME matches the window class to a .desktop file.
fn install_desktop_entry(site: &str) {
    let Some(appdir) = std::env::var_os("APPDIR") else { return };
    let icon = data_dir().join("teams.png");
    let _ = fs::copy(PathBuf::from(appdir).join("usr/lib/teams-for-linux/teams.png"), &icon);
    let Some(dir) = dirs::data_dir().map(|d| d.join("applications")) else { return };
    let _ = fs::create_dir_all(&dir);
    let exe = std::env::var("APPIMAGE")
        .ok()
        .or_else(|| std::env::current_exe().ok().map(|p| p.display().to_string()))
        .unwrap_or_default();
    // Must be a visible entry with an Exec line, otherwise GNOME refuses to pin it to the dash.
    let _ = fs::write(
        dir.join(format!("FFPWA-{site}.desktop")),
        format!(
            "[Desktop Entry]\nType=Application\nName=Microsoft Teams\nComment=Teams for Linux\nExec=\"{exe}\"\nIcon={}\nCategories=Network;InstantMessaging;\nStartupWMClass=FFPWA-{site}\nTerminal=false\n",
            icon.display()
        ),
    );
}

fn stop() {
    if let Some(id) = site_id() {
        // Match our private runtime only; the system Firefox and other PWAs must survive.
        let pattern = format!("teams-for-linux/runtime/firefox.*FFPWA-{id}");
        let _ = Command::new("pkill").args(["-f", "--", &pattern]).status();
    }
}

/// When set, a login autostart starts only the tray; Teams opens on "Show Teams".
fn start_minimized_path() -> PathBuf {
    data_dir().join("start-minimized")
}

fn set_start_minimized(on: bool) {
    let path = start_minimized_path();
    if on {
        let _ = fs::create_dir_all(data_dir());
        let _ = fs::write(path, "");
    } else {
        let _ = fs::remove_file(path);
    }
}

fn set_autostart(on: bool) {
    let path = autostart_path();
    if !on {
        let _ = fs::remove_file(path);
        return;
    }
    let exe = std::env::var("APPIMAGE")
        .ok()
        .or_else(|| std::env::current_exe().ok().map(|p| p.display().to_string()))
        .unwrap_or_default();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(
        path,
        format!(
            "[Desktop Entry]\nType=Application\nName=Teams for Linux\nExec=\"{exe}\" --autostart\nX-GNOME-Autostart-enabled=true\n"
        ),
    );
}

fn set_unread(tray: &TrayIcon, count: u32) {
    let bytes: &[u8] = if count > 0 {
        include_bytes!("../icons/tray-unread.png")
    } else {
        include_bytes!("../icons/tray.png")
    };
    if let Ok(icon) = Image::from_bytes(bytes) {
        let _ = tray.set_icon(Some(icon));
    }
    let _ = tray.set_tooltip(Some(if count > 0 {
        format!("Teams — {count} unread")
    } else {
        "Teams".to_string()
    }));
}

/// Minimal HTTP endpoint: the extension POSTs the unread count as the request body.
fn listen_for_unread(app: AppHandle) {
    thread::spawn(move || {
        let Ok(listener) = TcpListener::bind(("127.0.0.1", UNREAD_PORT)) else {
            eprintln!("port {UNREAD_PORT} busy; unread badge disabled");
            return;
        };
        for mut stream in listener.incoming().flatten() {
            let mut buf = [0u8; 2048];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let count = req
                .split("\r\n\r\n")
                .nth(1)
                .and_then(|b| b.trim().parse::<u32>().ok())
                .unwrap_or(0);
            let _ = stream.write_all(
                b"HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: *\r\nConnection: close\r\n\r\n",
            );
            if let Some(tray) = app.tray_by_id("main") {
                set_unread(&tray, count);
            }
        }
    });
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_, _, _| launch()))
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "Show Teams", true, None::<&str>)?;
            let autostart = CheckMenuItem::with_id(
                app, "autostart", "Launch on login", true, autostart_path().exists(), None::<&str>,
            )?;
            let minimized = CheckMenuItem::with_id(
                app, "minimized", "Start minimized (tray only)", true, start_minimized_path().exists(), None::<&str>,
            )?;
            let logs = MenuItem::with_id(app, "logs", "Open logs", true, None::<&str>)?;
            let reset = MenuItem::with_id(app, "reset", "Reset profile…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[&show, &PredefinedMenuItem::separator(app)?, &autostart, &minimized, &logs, &reset,
                  &PredefinedMenuItem::separator(app)?, &quit],
            )?;

            TrayIconBuilder::with_id("main")
                .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
                .tooltip("Teams")
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => launch(),
                    "autostart" => set_autostart(!autostart_path().exists()),
                    "minimized" => set_start_minimized(!start_minimized_path().exists()),
                    "logs" => {
                        let _ = Command::new("xdg-open").arg(log_path()).spawn();
                    }
                    "reset" => {
                        stop();
                        let _ = fs::remove_dir_all(data_dir().join("profiles"));
                        let _ = fs::remove_file(data_dir().join("config.json"));
                        launch();
                    }
                    "quit" => {
                        stop();
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            listen_for_unread(app.handle().clone());
            let autostarted = std::env::args().any(|a| a == "--autostart");
            if !(autostarted && start_minimized_path().exists()) {
                launch();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running teams-tray");
}
