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
      // `frame` is the content script's iframe-selector chain ([] = top
      // document). A step whose frame differs from the last one needs
      // `do/frame` transition drafts or replay runs it against the
      // wrong document. `frame main` exits ALL the way to the top —
      // there's no one-level-up — so a diverging chain re-enters frame
      // by frame. A cross-origin hop can't name its iframe element;
      // those steps are skipped + counted for the bundle warning
      // rather than recorded against the wrong document.
      const frame = msg.frame || [];
      if (frame === "cross-origin") {
        s.frameSkips = (s.frameSkips || 0) + 1;
      } else {
        const cur = s.currentFrame || [];
        if (JSON.stringify(cur) !== JSON.stringify(frame)) {
          const staysInside =
            cur.length <= frame.length &&
            cur.every((sel, i) => sel === frame[i]);
          if (!staysInside) {
            s.steps.push(frameDraft("return to the main document", { main: true }));
          }
          for (const sel of frame.slice(staysInside ? cur.length : 0)) {
            s.steps.push(frameDraft(`into iframe ${sel}`, { selector: sel }));
          }
          s.currentFrame = frame;
        }
        s.steps.push(msg.item);
      }
      persist(tabId);
    }
    return undefined;
  }
  if (msg.t === "net" && tabId != null) {
    const s = await getSession(tabId);
    if (s) {
      s.network.push(msg.entry);
      s.netReady = true; // a delivered entry implies the patch is armed
      persist(tabId);
    }
    return undefined;
  }
  if (msg.t === "netReady" && tabId != null) {
    const s = await getSession(tabId);
    if (s && msg.ready !== false) {
      s.netReady = true;
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
      lastUrl: tab.url,
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
    if (!s) {
      return { error: "nothing was recording" };
    }
    // Stop capture up front (listeners off, badge cleared) but keep the
    // session data until the export is confirmed — a failed download
    // must not take the recorded bundle down with it.
    tell(msg.tabId, { t: "record:stop" });
    chrome.action.setBadgeText({ tabId: msg.tabId, text: "" });
    const bundle = {
      version: 1,
      url: s.url,
      startedAt: s.startedAt,
      intent: `recorded via agent-qa extension on ${safeHost(s.url)}`,
      steps: s.steps,
      network: s.network,
    };
    if (s.captureGaps) {
      bundle.warnings = [
        `capture paused for ${s.captureGaps} non-http navigation(s) — ` +
          `interactions and network traffic on those pages are not in this bundle`,
      ];
      if (s.captureDead) {
        bundle.warnings.push("capture was still dead at stop — the tail is missing");
      }
    }
    // Steps captured but the page-world network patch never reported —
    // the <script src=chrome-extension://> injection is blockable by the
    // page's CSP, so a strict site exports network: [] with no signal.
    if (s.steps.length && !s.netReady) {
      (bundle.warnings ||= []).push(
        "network capture never armed on this page (CSP-blocked injection) — " +
          "steps recorded, but no HAR data; --mock-from replay is unavailable",
      );
    }
    if (s.frameSkips) {
      (bundle.warnings ||= []).push(
        `${s.frameSkips} interaction(s) inside a cross-origin iframe were skipped — ` +
          "the iframe element can't be located from inside it",
      );
    }
    const host = safeHost(s.url) || "page";
    const stamp = s.startedAt.replace(/[:.]/g, "-").slice(0, 19);
    const err = await download(
      `agent-qa-${host}-${stamp}.json`,
      JSON.stringify(bundle, null, 2),
    );
    if (err) {
      // The session is still persisted — keep reporting "recording" so
      // the popup's next click re-enters this handler and re-tries the
      // export against the same bundle instead of starting a fresh
      // recording over it.
      return {
        recording: true,
        error: `export failed: ${err} — click stop again to retry`,
      };
    }
    sessions.delete(msg.tabId);
    try {
      await chrome.storage.session.remove(keyOf(msg.tabId));
    } catch {}
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
      captureDead: !!(s && s.captureDead),
    };
  }

  return undefined;
}

function frameDraft(intent, params) {
  return { kind: "do", draft: { intent, verb: "frame", params } };
}

function safeHost(url) {
  try {
    return new URL(url).hostname.replace(/[^a-z0-9.-]/gi, "");
  } catch {
    return "";
  }
}

// Returns null on success, an error string when the download is
// rejected (permission revoked, save-as dialog dismissed, quota) —
// chrome.downloads resolves the callback with the item id or sets
// runtime.lastError.
function download(filename, text) {
  const b64 = btoa(unescape(encodeURIComponent(text)));
  return new Promise((resolve) => {
    chrome.downloads.download(
      {
        url: `data:application/json;base64,${b64}`,
        filename,
        saveAs: true,
      },
      () => {
        const e = chrome.runtime.lastError;
        resolve(e ? String(e.message || e) : null);
      },
    );
  });
}

// A closing tab silently drops its capture — clean up the persisted
// entry so a reopened tab with the same id doesn't resurrect it.
chrome.tabs.onRemoved.addListener((tabId) => {
  sessions.delete(tabId);
  chrome.storage.session.remove(keyOf(tabId)).catch(() => {});
});

// Navigation capture: a link click that triggers a full nav, a
// hand-typed URL, or an SPA pushState all move the page mid-recording
// — with no matching goto step the bundle replays against a page that
// isn't there. webNavigation is worker-side so it survives the content
// script being torn down by the nav itself.
async function recordNav(tabId, url) {
  const s = await getSession(tabId);
  if (!s) return;
  // Dedupe: onCommitted and onHistoryStateUpdated can both fire for
  // one transition, and the landing URL is already the bundle's `url`.
  if (!url || url === s.lastUrl) return;
  s.lastUrl = url;
  // A fresh document clears every frame context — the next step starts
  // from the top document again.
  s.currentFrame = [];
  s.steps.push({
    kind: "do",
    draft: {
      intent: `navigate to ${safeHost(url) || url}`,
      verb: "goto",
      value: { from: "literal", literal: url },
    },
  });
  // A nav to a page the content script can't run on (file://, chrome://,
  // extension store, pdf viewer) kills capture while the recording stays
  // armed — every interaction there lands nowhere. Flag it on the badge
  // NOW and count the gap so the exported bundle can name it.
  if (/^https?:/.test(url)) {
    if (s.captureDead) {
      s.captureDead = false;
      chrome.action.setBadgeText({ tabId, text: "REC" });
      chrome.action.setBadgeBackgroundColor({ tabId, color: "#d33" });
    }
  } else if (!s.captureDead) {
    s.captureDead = true;
    s.captureGaps = (s.captureGaps || 0) + 1;
    chrome.action.setBadgeText({ tabId, text: "!" });
    chrome.action.setBadgeBackgroundColor({ tabId, color: "#e80" });
  }
  persist(tabId);
}

chrome.webNavigation.onCommitted.addListener((d) => {
  if (d.frameId === 0) recordNav(d.tabId, d.url);
});
chrome.webNavigation.onHistoryStateUpdated.addListener((d) => {
  if (d.frameId === 0) recordNav(d.tabId, d.url);
});
