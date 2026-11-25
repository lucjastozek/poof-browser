/* ---------- settings: make poof the default browser ----------
   `defaultBrowserSettingsField()` builds the settings row. The native side answers
   `default_browser_status` with `window.__setDefaultBrowser({supported, is_default})`. On
   Windows the user confirms the change in the system settings, so the status is checked again
   whenever this window gets focus back. */
const DEFAULT_BROWSER_TEXT = {
  checking: "Checking…",
  yes: "poof is your default browser. Links and web files you open from other apps come here.",
  no: "poof is not your default browser. Make it the one that opens links and web files from other apps.",
  unsupported: "Setting the default browser is not available on this system yet.",
};

function defaultBrowserSettingsField() {
  const text = el("small", "t", DEFAULT_BROWSER_TEXT.checking);
  const action = btn("btn sm", "Make default", () => {
    action.disabled = true;
    send({ cmd: "set_default_browser" });
  });
  action.hidden = true;
  const row = el("div", "default-browser");
  row.append(text, action);

  const recheck = () => {
    if (!row.isConnected) {
      window.removeEventListener("focus", recheck);
      return;
    }
    send({ cmd: "default_browser_status" });
  };
  window.__setDefaultBrowser = (status) => {
    if (!row.isConnected) return;
    const supported = !!status.supported;
    const isDefault = supported && !!status.is_default;
    text.textContent = !supported
      ? DEFAULT_BROWSER_TEXT.unsupported
      : isDefault
        ? DEFAULT_BROWSER_TEXT.yes
        : DEFAULT_BROWSER_TEXT.no;
    action.hidden = !supported || isDefault;
    action.disabled = false;
  };
  window.addEventListener("focus", recheck);
  send({ cmd: "default_browser_status" });
  return field("Default browser", row);
}
