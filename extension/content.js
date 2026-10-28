// Teams puts the unread count in the page title, e.g. "(3) Chat | Microsoft Teams".
// UNVERIFIED for teams.cloud.microsoft; check the real title before relying on it.
let last = -1;
function report() {
  const m = document.title.match(/^\((\d+)\)/);
  const count = m ? Number(m[1]) : 0;
  if (count !== last) {
    last = count;
    browser.runtime.sendMessage({ unread: count });
  }
}
new MutationObserver(report).observe(document.querySelector("title") || document.head, {
  childList: true, subtree: true, characterData: true,
});
report();
