// Loaded by firefoxpwa's autoconfig as resource://user/boot.sys.mjs in the browser process.
// Teams puts the unread count in the page title, e.g. "(3) Chat | Microsoft Teams"; report it
// to the tray app, which swaps its icon.
import { setInterval, setTimeout, clearTimeout } from "resource://gre/modules/Timer.sys.mjs";

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

// Page-side API for the teams CLI. Stringified and evaluated in a sandbox over the Teams page,
// so it must be self-contained and uses only page globals. It drives Teams' own UI (search box,
// CKEditor compose box, send button); selectors are data-tid values, which Teams keeps
// language-independent.
function pageApi() {
  const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
  const waitFor = async (fn, ms) => {
    const end = Date.now() + ms;
    for (;;) {
      const value = fn();
      if (value || Date.now() > end) {
        return value || null;
      }
      await sleep(100);
    }
  };
  const tid = (root, name) => root.querySelector(`[data-tid=${name}]`);
  const kindOf = id => (id.startsWith("8:") ? "person" : id.endsWith("@thread.tacv2") ? "channel" : id.startsWith("19:") ? "chat" : null);

  function options() {
    const found = new Map();
    for (const el of document.querySelectorAll("[role=option]")) {
      const id = (el.getAttribute("data-tid") || "").match(/^AUTOSUGGEST_SUGGESTION_(?:TOPHITS|PEOPLE)(.+)$/)?.[1];
      const kind = id && kindOf(id);
      if (kind && !found.has(id)) {
        found.set(id, {
          el, id, kind,
          title: tid(el, "AUTOSUGGEST_SUGGESTION_TITLE")?.textContent ?? "",
          desc: tid(el, "AUTOSUGGEST_SUGGESTION_DESCRIPTION")?.textContent ?? "",
        });
      }
    }
    return [...found.values()];
  }

  // With wantId, stops as soon as that result shows up; results arrive in stages.
  async function runSearch(query, wantId) {
    const input = tid(document, "AUTOSUGGEST_INPUT");
    if (!input) {
      throw new Error("search box not found");
    }
    // The popup opens on focus events, which never fire while the window is in the background.
    input.dispatchEvent(new FocusEvent("focus"));
    input.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
    await sleep(500);
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(input, query);
    input.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: query.slice(-1) }));
    let last = "";
    let stable = 0;
    for (let i = 0; i < 60 && (wantId ? !options().some(o => o.id === wantId) : stable < 8); i++) {
      await sleep(150);
      const signature = options().map(o => o.id).join();
      stable = signature && signature === last ? stable + 1 : 0;
      last = signature;
    }
    return options();
  }

  function clearSearch() {
    const input = tid(document, "AUTOSUGGEST_INPUT");
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(input, "");
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }

  async function search({ query }) {
    const results = (await runSearch(query)).map(({ el, ...rest }) => rest);
    clearSearch();
    return results;
  }

  // Opens the conversation and confirms from the page header that it is the right one.
  async function open(query, conv) {
    const hit = (await runSearch(query, conv)).find(o => o.id === conv);
    if (!hit) {
      clearSearch();
      throw new Error("conversation not in the search results any more");
    }
    hit.el.click();
    clearSearch();
    const header = () => `${document.title} ${tid(document, "entity-header")?.textContent ?? ""}`.toLowerCase();
    if (!(await waitFor(() => header().includes(hit.title.toLowerCase()), 8000))) {
      throw new Error(`could not confirm that "${hit.title}" opened`);
    }
    await sleep(800);
    return hit;
  }

  async function read({ query, conv, count }) {
    const hit = await open(query, conv);
    const items = () => [...document.querySelectorAll("[data-tid=chat-pane-item]")].filter(el => tid(el, "chat-pane-message"));
    await waitFor(() => items().length, 8000);
    let author = "";
    const messages = items().map(el => {
      author = tid(el, "message-author-name")?.textContent.trim() || author;
      const time = el.querySelector("time");
      return { author, time: time?.getAttribute("datetime") || time?.getAttribute("title") || "", text: tid(el, "chat-pane-message").innerText.trim() };
    });
    return { title: hit.title, messages: messages.slice(-count) };
  }

  async function send({ query, conv, text }) {
    const hit = await open(query, conv).catch(e => {
      throw new Error(`${e.message}; nothing was sent`);
    });
    const editable = await waitFor(() => tid(document, "ckeditor"), 8000);
    if (!editable) {
      throw new Error("compose box not found; nothing was sent");
    }
    const editor = (editable.wrappedJSObject ?? editable).ckeditorInstance;
    if (editor.getData().replace(/<[^>]*>|&nbsp;/g, "").trim()) {
      throw new Error(`"${hit.title}" already has an unsent draft; not overwriting it`);
    }
    const escape = line => line.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    editor.setData(text.split("\n").map(line => `<p>${escape(line) || "&nbsp;"}</p>`).join(""));
    await sleep(300);
    const button = await waitFor(() => document.querySelector("[data-tid=chat-pane-compose-message-footer] [data-tid$=Commands-send]"), 3000);
    if (!button) {
      editor.setData("");
      throw new Error("send button not found; nothing was sent");
    }
    button.click();
    if (!(await waitFor(() => !editor.getData(), 5000))) {
      throw new Error("could not confirm the message was sent; check Teams");
    }
    return { sent: true, to: hit.title, kind: hit.kind, desc: hit.desc };
  }

  return { search, read, send };
}

// Runs in the content process for the Teams tab; serves requests from the control socket.
function controlFrame(pageApiSource) {
  const teams = /(^|\.)(teams\.cloud\.microsoft|teams\.microsoft\.com|teams\.live\.com)$/;
  // Page API calls and their arguments; anything else from the socket is rejected.
  const apiCalls = {
    search: ({ query }) => ({ query: String(query) }),
    read: ({ query, conv, count }) => ({ query: String(query), conv: String(conv), count: Number(count) || 10 }),
    send: ({ query, conv, text }) => ({ query: String(query), conv: String(conv), text: String(text) }),
  };
  addMessageListener("tray:cmd", async ({ data }) => {
    let reply;
    try {
      if (!teams.test(content.location.hostname)) {
        throw new Error("Teams is not loaded: " + content.location.href);
      }
      let result;
      if (data.cmd === "ping") {
        result = { title: content.document.title, url: content.location.href };
      } else if (Object.hasOwn(apiCalls, data.cmd)) {
        // The whole call runs inside the sandbox: Xray wrappers hide its functions from here.
        const sandbox = new Cu.Sandbox(content, { sandboxPrototype: content, wantXrays: false });
        const call = "(" + pageApiSource + ")()." + data.cmd + "(" + JSON.stringify(apiCalls[data.cmd](data)) + ")";
        result = await Cu.evalInSandbox(call, sandbox);
      } else {
        throw new Error("unknown command: " + data.cmd);
      }
      reply = { id: data.id, ok: true, result };
    } catch (e) {
      reply = { id: data.id, ok: false, error: String((e && e.message) || e) };
    }
    sendAsyncMessage("tray:cmd-result", reply);
  });
}

Services.mm.loadFrameScript(
  "data:application/javascript;charset=utf-8," + encodeURIComponent(`(${controlFrame})(${JSON.stringify(String(pageApi))})`),
  true
);

// Control socket for the teams CLI: one JSON request per connection, answered by the Teams page.
// A unix socket (0600, in the runtime dir) keeps web pages and other users out.
const CTL_DIR = PathUtils.join(Services.env.get("XDG_RUNTIME_DIR") || PathUtils.tempDir, "teams-for-linux");
const CTL_SOCK = PathUtils.join(CTL_DIR, `${profileId}.sock`);
const CTL_TIMEOUT_MS = 30000;
const pending = new Map();
let seq = 0;

Services.mm.addMessageListener("tray:cmd-result", message => pending.get(message.data.id)?.(message.data));

function dispatch(request) {
  const browser = Services.wm.getMostRecentWindow("navigator:browser")?.gBrowser?.selectedBrowser;
  if (!browser) {
    return Promise.resolve({ ok: false, error: "no browser window" });
  }
  const id = ++seq;
  return new Promise(resolve => {
    const finish = result => {
      clearTimeout(timer);
      pending.delete(id);
      resolve(result);
    };
    const timer = setTimeout(() => finish({ ok: false, error: "timed out waiting for Teams" }), CTL_TIMEOUT_MS);
    pending.set(id, finish);
    browser.messageManager.sendAsyncMessage("tray:cmd", { ...request, id });
  });
}

function serve(transport) {
  const input = transport.openInputStream(0, 0, 0);
  const output = transport.openOutputStream(Ci.nsITransport.OPEN_BLOCKING, 0, 0);
  const reader = Cc["@mozilla.org/scriptableinputstream;1"].createInstance(Ci.nsIScriptableInputStream);
  reader.init(input);
  let buffer = "";

  const reply = result => {
    // ASCII-only JSON, so the byte-oriented stream needs no encoding.
    const line = JSON.stringify(result).replace(/[\u0080-￿]/g, c => "\\u" + c.charCodeAt(0).toString(16).padStart(4, "0")) + "\n";
    try {
      output.write(line, line.length);
    } finally {
      output.close();
      input.close();
    }
  };

  const watcher = {
    onInputStreamReady(stream) {
      try {
        buffer += reader.read(stream.available());
      } catch (_) {
        return; // client went away
      }
      const end = buffer.indexOf("\n");
      if (end < 0 && buffer.length < 1 << 20) {
        stream.asyncWait(watcher, 0, 0, Services.tm.mainThread);
        return;
      }
      let request;
      try {
        request = JSON.parse(buffer.slice(0, end < 0 ? undefined : end));
      } catch (_) {
        reply({ ok: false, error: "bad request" });
        return;
      }
      dispatch(request).then(reply);
    },
  };
  input.asyncWait(watcher, 0, 0, Services.tm.mainThread);
}

IOUtils.makeDirectory(CTL_DIR, { permissions: 0o700, ignoreExisting: true })
  .then(() => IOUtils.remove(CTL_SOCK, { ignoreAbsent: true }))
  .then(() => {
    const file = Cc["@mozilla.org/file/local;1"].createInstance(Ci.nsIFile);
    file.initWithPath(CTL_SOCK);
    const server = Cc["@mozilla.org/network/server-socket;1"].createInstance(Ci.nsIServerSocket);
    server.initWithFilename(file, 0o600, -1);
    server.asyncListen({
      onSocketAccepted: (_server, transport) => serve(transport),
      onStopListening() {},
    });
    dump(`[tray] control socket ${CTL_SOCK}\n`);
  })
  .catch(error => dump(`[tray] control socket failed: ${error}\n`));
