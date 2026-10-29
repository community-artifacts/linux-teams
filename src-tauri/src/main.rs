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
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle, Manager,
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

const DEFAULT_PROFILE: &str = "00000000000000000000000000";

struct Account {
    site: String,
    profile: String,
    name: String,
}

fn config() -> Option<serde_json::Value> {
    serde_json::from_str(&fs::read_to_string(data_dir().join("config.json")).ok()?).ok()
}

/// Every installed Teams site is one account; its profile name is the account name.
fn accounts() -> Vec<Account> {
    let Some(cfg) = config() else { return vec![] };
    let labels = load_labels();
    let mut list: Vec<Account> = cfg["sites"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, site)| site["config"]["document_url"] == TEAMS_DOCUMENT)
        .map(|(id, site)| Account {
            site: id.clone(),
            profile: site["profile"].as_str().unwrap_or("").to_string(),
            name: labels
                .get(site["profile"].as_str().unwrap_or(""))
                .and_then(|e| e["name"].as_str())
                .or_else(|| cfg["profiles"][site["profile"].as_str().unwrap_or("")]["name"].as_str())
                .unwrap_or("Default")
                .to_string(),
        })
        .collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

/// Display names, kept by us rather than in firefoxpwa so the default profile can be named too.
/// profile id -> {"name": shown name, "email": address the name was last derived from}
fn labels_path() -> PathBuf {
    data_dir().join("labels.json")
}

fn load_labels() -> serde_json::Map<String, serde_json::Value> {
    fs::read_to_string(labels_path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_label(profile: &str, name: &str, email: Option<&str>) {
    let mut labels = load_labels();
    let mut entry = labels.get(profile).cloned().unwrap_or_else(|| serde_json::json!({}));
    entry["name"] = name.into();
    if let Some(email) = email {
        entry["email"] = email.into();
    }
    labels.insert(profile.to_string(), entry);
    let _ = fs::write(labels_path(), serde_json::Value::Object(labels).to_string());
}

/// Called with the address Teams is signed in with. The account is renamed to it once per
/// sign-in identity, so the name always follows the signed-in address.
fn account_signed_in(profile: &str, email: &str) {
    let known = load_labels();
    if known.get(profile).and_then(|e| e["email"].as_str()) == Some(email) {
        return;
    }
    if accounts().iter().any(|a| a.profile != profile && a.name.eq_ignore_ascii_case(email)) {
        return;
    }
    save_label(profile, email, Some(email));
    if let Some(account) = accounts().into_iter().find(|a| a.profile == profile) {
        install_desktop_entry(&account.site);
    }
}

fn last_account_path() -> PathBuf {
    data_dir().join("last-account")
}

fn ensure_runtime() -> Result<(), String> {
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
    write_policies();
    Ok(())
}

const POLICIES_JSON: &str = r#"{
  "policies": {
    "Permissions": {
      "Notifications": {
        "Allow": ["https://teams.cloud.microsoft", "https://teams.microsoft.com", "https://teams.live.com"],
        "BlockNewRequests": false
      }
    },
    "DisableTelemetry": true,
    "DisableFirefoxStudies": true,
    "DontCheckDefaultBrowser": true
  }
}
"#;

/// Pre-grants Teams desktop notifications so no permission prompt is needed.
fn write_policies() {
    let dir = data_dir().join("runtime/distribution");
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join("policies.json"), POLICIES_JSON);
}

/// Installs a Teams site. `None` uses firefoxpwa's shared profile (the first account);
/// a name creates a dedicated profile so the login, cookies and cache stay separate.
fn install_account(name: Option<&str>) -> Result<String, String> {
    ensure_runtime()?;
    let mut args = vec![
        "site", "install", TEAMS_MANIFEST,
        "--document-url", TEAMS_DOCUMENT,
        "--start-url", TEAMS_START,
        "--name", "Microsoft Teams",
        "--no-system-integration",
    ];
    let profile;
    if let Some(name) = name {
        let status = ffpwa(&["profile", "create", "--name", name])
            .status()
            .map_err(|e| format!("cannot run firefoxpwa: {e}"))?;
        if !status.success() {
            return Err(format!("firefoxpwa profile create failed: {status}"));
        }
        profile = config()
            .and_then(|c| {
                c["profiles"].as_object()?.iter().find_map(|(id, p)| (p["name"] == name).then(|| id.clone()))
            })
            .ok_or("new profile missing from firefoxpwa config")?;
        args.extend(["--profile", &profile]);
    }
    let before: Vec<String> = accounts().into_iter().map(|a| a.site).collect();
    let status = ffpwa(&args).status().map_err(|e| format!("cannot run firefoxpwa: {e}"))?;
    if !status.success() {
        return Err(format!("firefoxpwa site install failed: {status}"));
    }
    accounts()
        .into_iter()
        .map(|a| a.site)
        .find(|id| !before.contains(id))
        .ok_or_else(|| "Teams site missing from firefoxpwa config after install".into())
}

/// Resolves the account to open: the requested one, else the last used, else the first.
fn ensure_account(wanted: Option<String>) -> Result<String, String> {
    let list = accounts();
    let last = fs::read_to_string(last_account_path()).ok().map(|s| s.trim().to_string());
    let pick = wanted
        .or(last)
        .filter(|id| list.iter().any(|a| &a.site == id))
        .or_else(|| list.first().map(|a| a.site.clone()));
    match pick {
        Some(id) => Ok(id),
        None => install_account(None),
    }
}

/// Launching an already running site focuses its existing window.
fn launch(wanted: Option<String>) {
    match ensure_account(wanted) {
        Ok(id) => {
            let _ = fs::write(last_account_path(), &id);
            stop_others(&id);
            // Relaunching a running site opens a second window and Wayland gives us no way
            // to raise the first, so leave it alone (the dash icon raises it).
            if is_running(&id) {
                return;
            }
            write_policies();
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

/// Value of `--account <site>` from a command line, used by per-account dock shortcuts.
fn account_arg(argv: &[String]) -> Option<String> {
    argv.iter().position(|a| a == "--account").and_then(|i| argv.get(i + 1).cloned())
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
user_pref("browser.startup.page", 0);
user_pref("browser.sessionstore.resume_from_crash", false);
user_pref("browser.sessionstore.restore_on_demand", false);
user_pref("browser.sessionstore.max_tabs_undo", 0);
user_pref("browser.sessionstore.max_windows_undo", 0);
user_pref("browser.sessionstore.warnOnQuit", false);
user_pref("browser.warnOnQuit", false);
user_pref("network.cookie.cookieBehavior", 0);
user_pref("privacy.partition.network_state", false);
user_pref("privacy.trackingprotection.enabled", false);
user_pref("privacy.trackingprotection.socialtracking.enabled", false);
user_pref("privacy.trackingprotection.cryptomining.enabled", false);
user_pref("privacy.trackingprotection.fingerprinting.enabled", false);
user_pref("network.cookie.sameSite.laxByDefault", false);
user_pref("firefoxpwa.openOutOfScopeInDefaultBrowser", true);
user_pref("firefoxpwa.allowedDomains", "*.cloud.microsoft,*.microsoft.com,*.microsoftonline.com,*.microsoftonline-p.com,*.live.com,*.office.com,*.office365.com,*.microsoft365.com,*.outlook.com,*.msauth.net,*.msftauth.net,login.windows.net");
user_pref("browser.dom.window.dump.enabled", true);
user_pref("browser.startup.couldRestoreSession.count", -1);
user_pref("datareporting.policy.dataSubmissionPolicyBypassNotification", true);
user_pref("datareporting.policy.dataSubmissionEnabled", false);
user_pref("datareporting.healthreport.uploadEnabled", false);
user_pref("toolkit.telemetry.reportingpolicy.firstRun", false);
user_pref("toolkit.telemetry.enabled", false);
user_pref("toolkit.telemetry.unified", false);
user_pref("browser.newtabpage.activity-stream.feeds.telemetry", false);
user_pref("browser.newtabpage.activity-stream.telemetry", false);
user_pref("browser.warnOnQuitShortcut", false);
user_pref("browser.tabs.warnOnClose", false);
user_pref("media.getdisplaymedia.previews.enabled", false);
"#;

/// Reports the unread count from the window title to the tray (see profile/boot.sys.mjs).
const BOOT_JS: &str = include_str!("../../profile/boot.sys.mjs");

const USER_CHROME_CSS: &str = "\
#nav-bar, #toolbar-menubar, #PersonalToolbar, #sidebar-main { visibility: collapse !important; }
#TabsToolbar > *:not(.titlebar-buttonbox-container) { visibility: collapse !important; }
#TabsToolbar { display: flex !important; }
.notificationbox-stack, #tab-notification-deck { display: none !important; }
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
    let _ = fs::create_dir_all(dir.join("chrome/user"));
    let _ = fs::write(dir.join("chrome/user/boot.sys.mjs"), BOOT_JS);
}

/// Gives the window a dock/taskbar identity: GNOME matches the window class to a .desktop file.
fn install_desktop_entry(site: &str) {
    let name = accounts().into_iter().find(|a| a.site == site).map(|a| a.name).unwrap_or_default();
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
            "[Desktop Entry]\nType=Application\nName=Microsoft Teams ({name})\nComment=Teams for Linux\nExec=\"{exe}\" --account {site}\nIcon={}\nCategories=Network;InstantMessaging;\nStartupWMClass=FFPWA-{site}\nTerminal=false\n",
            icon.display()
        ),
    );
}

fn is_running(site: &str) -> bool {
    let pattern = format!("teams-for-linux/runtime/firefox.*FFPWA-{site}");
    Command::new("pgrep").args(["-f", "--", &pattern]).output().map_or(false, |o| o.status.success())
}

/// Keeps a single window: stops every account except `keep` before opening it.
fn stop_others(keep: &str) {
    for account in accounts().into_iter().filter(|a| a.site != keep) {
        let pattern = format!("teams-for-linux/runtime/firefox.*FFPWA-{}", account.site);
        if Command::new("pkill").args(["-f", "--", &pattern]).status().map_or(false, |s| s.success()) {
            thread::sleep(std::time::Duration::from_millis(1500));
        }
    }
}

fn stop() {
    for account in accounts() {
        // Match our private runtime only; the system Firefox and other PWAs must survive.
        let pattern = format!("teams-for-linux/runtime/firefox.*FFPWA-{}", account.site);
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
static ACCOUNTS_MENU: std::sync::OnceLock<Submenu<tauri::Wry>> = std::sync::OnceLock::new();

fn listen_for_unread(app: AppHandle) {
    thread::spawn(move || {
        let Ok(listener) = TcpListener::bind(("127.0.0.1", UNREAD_PORT)) else {
            eprintln!("port {UNREAD_PORT} busy; unread badge disabled");
            return;
        };
        for mut stream in listener.incoming().flatten() {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(500)));
            // The body can arrive in a later segment than the headers; read until it is complete.
            let mut req = Vec::new();
            let mut buf = [0u8; 1024];
            while let Ok(n) = stream.read(&mut buf) {
                if n == 0 {
                    break;
                }
                req.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&req);
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let wanted = head
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                        .unwrap_or(0);
                    if body.len() >= wanted {
                        break;
                    }
                }
            }
            let req = String::from_utf8_lossy(&req);
            let path = req.split_whitespace().nth(1).unwrap_or("").to_string();
            let body = req.split("\r\n\r\n").nth(1).unwrap_or("").trim().to_string();
            let _ = stream.write_all(
                b"HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: *\r\nConnection: close\r\n\r\n",
            );
            if path == "/account" {
                // Body: "<profile id>\n<email>"
                if let Some((profile, email)) = body.split_once('\n') {
                    eprintln!("signed in: {email}");
                    account_signed_in(profile.trim(), email.trim());
                    if let Some(menu) = ACCOUNTS_MENU.get() {
                        fill_accounts_menu(menu, &app);
                    }
                }
            } else {
                let count = body.parse::<u32>().unwrap_or(0);
                eprintln!("unread count: {count}");
                if let Some(tray) = app.tray_by_id("main") {
                    set_unread(&tray, count);
                }
            }
        }
    });
}

/// Rebuilds the account list: one entry per account (last used marked) plus "Add account…".
fn fill_accounts_menu<R: tauri::Runtime>(menu: &Submenu<R>, app: &impl Manager<R>) {
    if let Ok(items) = menu.items() {
        for item in items {
            let _ = menu.remove(&item);
        }
    }
    let last = fs::read_to_string(last_account_path()).unwrap_or_default();
    for account in accounts() {
        let label = if account.site == last.trim() { format!("● {}", account.name) } else { account.name.clone() };
        if let Ok(item) = MenuItem::with_id(app, format!("acct:{}", account.site), label, true, None::<&str>) {
            let _ = menu.append(&item);
        }
    }
    if let Ok(sep) = PredefinedMenuItem::separator(app) {
        let _ = menu.append(&sep);
    }
    for (id, label) in [("add_account", "Add account…"), ("remove_account", "Remove account…")] {
        if let Ok(item) = MenuItem::with_id(app, id, label, true, None::<&str>) {
            let _ = menu.append(&item);
        }
    }
}

/// Creates a dedicated profile + Teams site and opens it. The placeholder name is replaced
/// by the signed-in email once the user has logged in.
fn add_account(app: AppHandle, submenu: Submenu<tauri::Wry>) {
    thread::spawn(move || {
        let existing = accounts();
        let mut name = "New account".to_string();
        let mut n = 2;
        while existing.iter().any(|a| a.name.eq_ignore_ascii_case(&name)) {
            name = format!("New account {n}");
            n += 1;
        }
        match install_account(Some(&name)) {
            Ok(id) => {
                launch(Some(id));
                fill_accounts_menu(&submenu, &app);
            }
            Err(e) => eprintln!("{e}"),
        }
    });
}

/// Lets the user pick one of the non-default accounts (the default one can't be changed).
fn pick_account(title: &str) -> Option<Account> {
    let list: Vec<Account> = accounts().into_iter().filter(|a| a.profile != DEFAULT_PROFILE).collect();
    if list.is_empty() {
        let _ = Command::new("zenity")
            .args(["--info", "--text=There are no extra accounts yet. The default account can't be changed."])
            .status();
        return None;
    }
    let mut cmd = Command::new("zenity");
    cmd.args(["--list", "--title", title, "--column=Account", "--hide-header"]);
    for a in &list {
        cmd.arg(&a.name);
    }
    let out = cmd.output().ok().filter(|o| o.status.success())?;
    let chosen = String::from_utf8_lossy(&out.stdout).trim().to_string();
    list.into_iter().find(|a| a.name == chosen)
}

fn remove_account(app: AppHandle, submenu: Submenu<tauri::Wry>) {
    thread::spawn(move || {
        let Some(account) = pick_account("Remove account") else { return };
        let confirmed = Command::new("zenity")
            .args(["--question", "--title=Remove account", "--text"])
            .arg(format!("Remove \"{}\"? Its login and data will be deleted.", account.name))
            .status()
            .map_or(false, |s| s.success());
        if !confirmed {
            return;
        }
        let pattern = format!("teams-for-linux/runtime/firefox.*FFPWA-{}", account.site);
        let _ = Command::new("pkill").args(["-f", "--", &pattern]).status();
        thread::sleep(std::time::Duration::from_millis(1500));
        let _ = ffpwa(&["profile", "remove", &account.profile, "--quiet"]).status();
        let mut labels = load_labels();
        labels.remove(&account.profile);
        let _ = fs::write(labels_path(), serde_json::Value::Object(labels).to_string());
        if let Some(dir) = dirs::data_dir() {
            let _ = fs::remove_file(dir.join(format!("applications/FFPWA-{}.desktop", account.site)));
        }
        if fs::read_to_string(last_account_path()).map_or(false, |l| l.trim() == account.site) {
            let _ = fs::remove_file(last_account_path());
        }
        fill_accounts_menu(&submenu, &app);
    });
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_, argv, _| launch(account_arg(&argv))))
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "Show Teams", true, None::<&str>)?;
            let accounts_menu = Submenu::with_id(app, "accounts", "Accounts", true)?;
            fill_accounts_menu(&accounts_menu, app);
            let autostart = CheckMenuItem::with_id(
                app, "autostart", "Launch on login", true, autostart_path().exists(), None::<&str>,
            )?;
            let minimized = CheckMenuItem::with_id(
                app, "minimized", "Start minimized (tray only)", true, start_minimized_path().exists(), None::<&str>,
            )?;
            let logs = MenuItem::with_id(app, "logs", "Open logs", true, None::<&str>)?;
            let reset = MenuItem::with_id(app, "reset", "Reset all accounts…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[&show, &accounts_menu, &PredefinedMenuItem::separator(app)?, &autostart, &minimized, &logs, &reset,
                  &PredefinedMenuItem::separator(app)?, &quit],
            )?;

            let submenu = accounts_menu.clone();
            let _ = ACCOUNTS_MENU.set(accounts_menu.clone());
            TrayIconBuilder::with_id("main")
                .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
                .tooltip("Teams")
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => launch(None),
                    "add_account" => add_account(app.clone(), submenu.clone()),
                    "remove_account" => remove_account(app.clone(), submenu.clone()),
                    id if id.starts_with("acct:") => {
                        launch(Some(id["acct:".len()..].to_string()));
                        fill_accounts_menu(&submenu, app);
                    }
                    "autostart" => set_autostart(!autostart_path().exists()),
                    "minimized" => set_start_minimized(!start_minimized_path().exists()),
                    "logs" => {
                        let _ = Command::new("xdg-open").arg(log_path()).spawn();
                    }
                    "reset" => {
                        stop();
                        let _ = fs::remove_dir_all(data_dir().join("profiles"));
                        let _ = fs::remove_file(data_dir().join("config.json"));
                        let _ = fs::remove_file(last_account_path());
                        let _ = fs::remove_file(labels_path());
                        launch(None);
                        fill_accounts_menu(&submenu, app);
                    }
                    "quit" => {
                        stop();
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            listen_for_unread(app.handle().clone());
            let argv: Vec<String> = std::env::args().collect();
            let autostarted = argv.iter().any(|a| a == "--autostart");
            if !(autostarted && start_minimized_path().exists()) {
                launch(account_arg(&argv));
                fill_accounts_menu(&accounts_menu, app);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running teams-tray");
}
