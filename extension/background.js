// Service worker: owns per-tab capture state, relays record:start/stop
// to the tab's content script, and downloads the bundle on stop.
//
// Bundle shape (what `agent-qa ingest` parses):
//   {version, url, startedAt, steps: [{kind,draft}], network: [entries]}

const sessions = new Map(); // tabId -> {steps, network, url, startedAt}

const tell = (tabId, msg) => {
  try {
    chrome.tabs.sendMessage(tabId, msg).catch(() => {});
  } catch {}
};

chrome.runtime.onMessage.addListener((msg, sender, sendResponse) => {
  const tabId = sender.tab ? sender.tab.id : null;

  if (msg.t === "state") {
    sendResponse({ recording: tabId != null && sessions.has(tabId) });
    return;
  }
  if (msg.t === "step" && tabId != null) {
    sessions.get(tabId)?.steps.push(msg.item);
    return;
  }
  if (msg.t === "net" && tabId != null) {
    sessions.get(tabId)?.network.push(msg.entry);
    return;
  }

  if (msg.t === "popup:start") {
    chrome.tabs
      .get(msg.tabId)
      .then((tab) => {
        if (!/^https?:/.test(tab.url || "")) {
          sendResponse({ error: "can't record on this page" });
          return;
        }
        sessions.set(msg.tabId, {
          steps: [],
          network: [],
          url: tab.url,
          startedAt: new Date().toISOString(),
        });
        tell(msg.tabId, { t: "record:start" });
        chrome.action.setBadgeText({ tabId: msg.tabId, text: "REC" });
        chrome.action.setBadgeBackgroundColor({
          tabId: msg.tabId,
          color: "#d33",
        });
        sendResponse({ recording: true });
      })
      .catch(() => sendResponse({ error: "no active tab" }));
    return true; // async sendResponse
  }

  if (msg.t === "popup:stop") {
    const s = sessions.get(msg.tabId);
    sessions.delete(msg.tabId);
    tell(msg.tabId, { t: "record:stop" });
    chrome.action.setBadgeText({ tabId: msg.tabId, text: "" });
    if (!s) {
      sendResponse({ error: "nothing was recording" });
      return;
    }
    const bundle = {
      version: 1,
      url: s.url,
      startedAt: s.startedAt,
      intent: `recorded via agent-qa extension on ${safeHost(s.url)}`,
      steps: s.steps,
      network: s.network,
    };
    const host = safeHost(s.url) || "page";
    const stamp = s.startedAt.replace(/[:.]/g, "-").slice(0, 19);
    download(`agent-qa-${host}-${stamp}.json`, JSON.stringify(bundle, null, 2));
    sendResponse({
      recording: false,
      steps: s.steps.length,
      requests: s.network.length,
    });
    return true;
  }

  if (msg.t === "popup:status") {
    const s = sessions.get(msg.tabId);
    sendResponse({
      recording: !!s,
      steps: s ? s.steps.length : 0,
      requests: s ? s.network.length : 0,
    });
    return;
  }
});

function safeHost(url) {
  try {
    return new URL(url).hostname.replace(/[^a-z0-9.-]/gi, "");
  } catch {
    return "";
  }
}

function download(filename, text) {
  const b64 = btoa(unescape(encodeURIComponent(text)));
  chrome.downloads.download({
    url: `data:application/json;base64,${b64}`,
    filename,
    saveAs: true,
  });
}

// A closing tab silently drops its capture — nothing to clean up beyond
// the map entry.
chrome.tabs.onRemoved.addListener((tabId) => sessions.delete(tabId));
