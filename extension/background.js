// Service worker: owns per-tab capture state, relays record:start/stop
// to the tab's content script, and downloads the bundle on stop.
//
// Bundle shape (what `agent-qa ingest` parses):
//   {version, url, startedAt, steps: [{kind,draft}], network: [entries]}
//
// MV3 suspends this worker after ~30s of inactivity, wiping module
// state — so every mutation write-throughs to chrome.storage.session
// and handlers rehydrate on wake. Without that, pausing mid-recording
// silently destroys the capture while the REC badge stays lit.

const sessions = new Map(); // tabId -> {steps, network, url, startedAt}
const keyOf = (tabId) => `capture-${tabId}`;

async function getSession(tabId) {
  if (sessions.has(tabId)) return sessions.get(tabId);
  try {
    const got = await chrome.storage.session.get(keyOf(tabId));
    const s = got[keyOf(tabId)];
    if (s) {
      sessions.set(tabId, s);
      return s;
    }
  } catch {}
  return undefined;
}

function persist(tabId) {
  const s = sessions.get(tabId);
  if (!s) return;
  // Fire-and-forget: quota errors keep the in-memory copy; the worst
  // case is a lost capture on suspend, same as before.
  chrome.storage.session.set({ [keyOf(tabId)]: s }).catch(() => {});
}

const tell = (tabId, msg) => {
  try {
    chrome.tabs.sendMessage(tabId, msg).catch(() => {});
  } catch {}
};

chrome.runtime.onMessage.addListener((msg, sender, sendResponse) => {
  handle(msg, sender)
    .then((resp) => {
      if (resp !== undefined) sendResponse(resp);
    })
    .catch(() => {});
  return true; // response is always async now
});

async function handle(msg, sender) {
  const tabId = sender.tab ? sender.tab.id : null;

  if (msg.t === "state") {
    if (tabId == null) return { recording: false };
    const s = await getSession(tabId);
    return { recording: !!s };
  }
  if (msg.t === "step" && tabId != null) {
    const s = await getSession(tabId);
    if (s) {
      s.steps.push(msg.item);
      persist(tabId);
    }
    return undefined;
  }
  if (msg.t === "net" && tabId != null) {
    const s = await getSession(tabId);
    if (s) {
      s.network.push(msg.entry);
      persist(tabId);
    }
    return undefined;
  }

  if (msg.t === "popup:start") {
    const tab = await chrome.tabs.get(msg.tabId).catch(() => null);
    if (!tab || !/^https?:/.test(tab.url || "")) {
      return { error: "can't record on this page" };
    }
    sessions.set(msg.tabId, {
      steps: [],
      network: [],
      url: tab.url,
      startedAt: new Date().toISOString(),
    });
    persist(msg.tabId);
    tell(msg.tabId, { t: "record:start" });
    chrome.action.setBadgeText({ tabId: msg.tabId, text: "REC" });
    chrome.action.setBadgeBackgroundColor({ tabId: msg.tabId, color: "#d33" });
    return { recording: true };
  }

  if (msg.t === "popup:stop") {
    const s = await getSession(msg.tabId);
    sessions.delete(msg.tabId);
    try {
      await chrome.storage.session.remove(keyOf(msg.tabId));
    } catch {}
    tell(msg.tabId, { t: "record:stop" });
    chrome.action.setBadgeText({ tabId: msg.tabId, text: "" });
    if (!s) {
      return { error: "nothing was recording" };
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
    return {
      recording: false,
      steps: s.steps.length,
      requests: s.network.length,
    };
  }

  if (msg.t === "popup:status") {
    const s = await getSession(msg.tabId);
    return {
      recording: !!s,
      steps: s ? s.steps.length : 0,
      requests: s ? s.network.length : 0,
    };
  }

  return undefined;
}

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

// A closing tab silently drops its capture — clean up the persisted
// entry so a reopened tab with the same id doesn't resurrect it.
chrome.tabs.onRemoved.addListener((tabId) => {
  sessions.delete(tabId);
  chrome.storage.session.remove(keyOf(tabId)).catch(() => {});
});
