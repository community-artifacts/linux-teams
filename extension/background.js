browser.runtime.onMessage.addListener(({ unread }) => {
  fetch("http://127.0.0.1:47653/unread", { method: "POST", body: String(unread) }).catch(() => {});
});
