// Loaded by firefoxpwa's autoconfig as resource://user/boot.sys.mjs in the browser process.
// Teams puts the unread count in the page title, e.g. "(3) Chat | Microsoft Teams"; report it
// to the tray app, which swaps its icon.
import { setInterval } from "resource://gre/modules/Timer.sys.mjs";

const ENDPOINT = "http://127.0.0.1:47653/unread";

let last = -1;

function post(path, body) {
  const win = Services.wm.getMostRecentWindow("navigator:browser");
  return win?.fetch(`http://127.0.0.1:47653${path}`, { method: "POST", body, mode: "no-cors" });
}

function send(count) {
  if (count === last) {
    return;
  }
  last = count;
  dump(`[tray] unread=${count}\n`);
  // Linux has no hidden DOM window; the browser window (system principal) can fetch instead.
  const win = Services.wm.getMostRecentWindow("navigator:browser");
  win?.fetch(ENDPOINT, { method: "POST", body: String(count), mode: "no-cors" }).catch(error => {
    dump(`[tray] report failed: ${error}\n`);
    last = -1; // retry on the next poll
  });
}

function currentTitle() {
  const win = Services.wm.getMostRecentWindow("navigator:browser");
  return win?.gBrowser?.selectedBrowser?.contentTitle || win?.document?.title || "";
}

// Pre-grant desktop notifications for Teams so Firefox never asks (the policy file is not applied).
for (const origin of ["https://teams.cloud.microsoft", "https://teams.microsoft.com", "https://teams.live.com"]) {
  try {
    const principal = Services.scriptSecurityManager.createContentPrincipalFromOrigin(origin);
    Services.perms.addFromPrincipal(principal, "desktop-notification", Services.perms.ALLOW_ACTION, Services.perms.EXPIRE_NEVER);
    dump(`[tray] notifications ${origin}=${Services.perms.testExactPermissionFromPrincipal(principal, "desktop-notification")}\n`);
  } catch (error) {
    dump(`[tray] notification grant failed for ${origin}: ${error}\n`);
  }
}

let lastTitle = null;

function poll() {
  const title = currentTitle();
  if (title !== lastTitle) {
    lastTitle = title;
    dump(`[tray] title="${title}"\n`);
  }
  send(Number(title.match(/\((\d+)\)/)?.[1] ?? 0));
}

setInterval(poll, 2000);
Services.obs.addObserver(() => send(0), "quit-application");

// Debug: log the host of every top-level page load, so sign-in redirects can be traced.
Services.obs.addObserver(subject => {
  try {
    const channel = subject.QueryInterface(Ci.nsIHttpChannel);
    if (channel.loadInfo.isTopLevelLoad && channel.isMainDocumentChannel) {
      dump(`[tray] load ${channel.URI.scheme}://${channel.URI.host}\n`);
    }
  } catch (_) {}
}, "http-on-modify-request");

// Report the signed-in address so the tray can name the account after it. Content runs in
// another process, so a frame script reads Teams' MSAL data from localStorage.
const FRAME_SCRIPT = `
  const { setInterval } = ChromeUtils.importESModule("resource://gre/modules/Timer.sys.mjs");
  const teams = /(^|\\.)(teams\\.cloud\\.microsoft|teams\\.microsoft\\.com|teams\\.live\\.com)$/;
  const re = /"(?:username|preferred_username|email|upn)"\\s*:\\s*"([^"\\s]+@[^"\\s]+)"/i;
  let sent = "";
  setInterval(() => {
    try {
      if (!teams.test(content.location.hostname)) return;
      const ls = content.localStorage;
      for (let i = 0; i < ls.length; i++) {
        const m = (ls.getItem(ls.key(i)) || "").match(re);
        if (m && m[1].toLowerCase() !== sent) {
          sent = m[1].toLowerCase();
          sendAsyncMessage("tray:email", sent);
          return;
        }
      }
    } catch (_) {}
  }, 5000);
`;

const profileId = Services.dirsvc.get("ProfD", Ci.nsIFile).leafName;

Services.mm.addMessageListener("tray:email", message => {
  dump(`[tray] signed in as ${message.data}\n`);
  post("/account", `${profileId}\n${message.data}`)?.catch(error => dump(`[tray] account report failed: ${error}\n`));
});
Services.mm.loadFrameScript("data:application/javascript;charset=utf-8," + encodeURIComponent(FRAME_SCRIPT), true);
