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
// A popup opened by a recording tab SHARES its session object (same map
// entry under a second key), so interactions there land in one bundle.
const keyOf = (tabId) => `capture-${tabId}`;

// Replay-side tab naming is open-order (t1, t2, …) — mirror it: the
// recording tab is t1, every attached popup takes the next index.
const tabIndex = (s, tabId) => {
  s.tabs ||= [s.rootTabId ?? tabId];
  let i = s.tabs.indexOf(tabId);
  if (i === -1) {
    s.tabs.push(tabId);
    i = s.tabs.length - 1;
  }
  return i + 1;
};

const tabDraft = (n) => ({
  kind: "do",
  draft: {
    intent: `switch to tab t${n}`,
    verb: "tab",
    value: { from: "literal", literal: `t${n}` },
  },
});

// Steps and navs may arrive from any attached tab — emit the `tab tN`
// switch draft the first time capture follows focus into a different
// tab. Frame context is per-tab, so a switch resets it like a nav does.
const ensureActiveTab = (s, tabId) => {
  const n = tabIndex(s, tabId);
  if (s.activeTab === tabId) return;
  s.activeTab = tabId;
  s.currentFrame = [];
  s.steps.push(tabDraft(n));
};

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
  // case is a lost capture on suspend, same as before. Attached popup
  // tabs share the object — write it under every key it lives at so a
  // worker restart rehydrates the same session on any of them.
  const tabs = Array.isArray(s.tabs) ? s.tabs : [tabId];
  for (const t of new Set([tabId, ...tabs])) {
    chrome.storage.session.set({ [keyOf(t)]: s }).catch(() => {});
  }
}

// Resolves true when a content script actually received the message —
// sendMessage rejects when the tab has no receiver (a page loaded
// before the extension installed), which record:start turns into a
// visible "reload the page" error instead of a dead REC badge.
const tell = (tabId, msg) => {
  try {
    return chrome.tabs
      .sendMessage(tabId, msg)
      .then(() => true)
      .catch(() => false);
  } catch {
    return Promise.resolve(false);
  }
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
      // Focus may have moved to an attached popup tab — emit the tab
      // switch before frame transitions so replay follows focus first.
      ensureActiveTab(s, tabId);
      if (msg.merge === "dblclick") {
        // The click,click pair a real dblclick produces already landed
        // as two click drafts — swap them for this dblclick draft at
        // the earliest removed position so replay performs one
        // double-click, not two activations.
        const idx = [];
        for (
          let i = s.steps.length - 1;
          i >= 0 && i > s.steps.length - 10 && idx.length < 2;
          i--
        ) {
          const it = s.steps[i];
          if (it && it.kind === "do" && it.draft && it.draft.verb === "click") {
            idx.push(i);
          }
        }
        const at = idx.length ? idx[idx.length - 1] : s.steps.length;
        for (const i of idx) s.steps.splice(i, 1);
        s.steps.splice(at, 0, msg.item);
        persist(tabId);
        return undefined;
      }
      const frame = msg.frame || [];
      if (msg.wall) {
        s.botWallSteps = (s.botWallSteps || 0) + 1;
      }
      if (frame === "cross-origin") {
        // A challenge widget's own iframe is cross-origin — count those
        // taps as wall interactions, not generic frame skips.
        if (!msg.wall) s.frameSkips = (s.frameSkips || 0) + 1;
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
  if (msg.t === "augment" && tabId != null) {
    const s = await getSession(tabId);
    if (s) {
      // The upload draft already landed in interaction order — attach
      // the file contents the page finished reading since then. Match
      // by augmentId so two racing uploads can't swap contents.
      for (let i = s.steps.length - 1; i >= 0; i--) {
        const it = s.steps[i];
        if (
          it &&
          it.kind === "do" &&
          it.draft &&
          it.draft.verb === "upload" &&
          it.augmentId === msg.id
        ) {
          it.uploads = msg.uploads;
          delete it.augmentId;
          break;
        }
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
    // Start is idempotent — a second click before the popup's status
    // poll lands must not wipe an in-progress session's steps.
    if (await getSession(msg.tabId)) {
      return { recording: true };
    }
    const s = {
      steps: [],
      network: [],
      url: tab.url,
      lastUrls: { [msg.tabId]: tab.url },
      tabs: [msg.tabId],
      rootTabId: msg.tabId,
      activeTab: msg.tabId,
      startedAt: new Date().toISOString(),
    };
    sessions.set(msg.tabId, s);
    // The content script is absent on pages that loaded before the
    // extension (install, update, or a restored tab) — nothing it does
    // there can be captured, so fail the start rather than a silent
    // dead recording.
    if (!(await tell(msg.tabId, { t: "record:start" }))) {
      sessions.delete(msg.tabId);
      return {
        error:
          "this page was loaded before the extension — reload it, then record",
      };
    }
    persist(msg.tabId);
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
    // Every attached popup tab shares the session — stop capture on
    // all of them and clear every badge.
    for (const t of s.tabs || [msg.tabId]) {
      tell(t, { t: "record:stop" });
      chrome.action.setBadgeText({ tabId: t, text: "" });
    }
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
    // Upload steps carry `files/<name>` references. Contents ≤256KB are
    // inlined into the bundle (item.uploads[].data) — name any that
    // arrived without data so fixing them is a copy, not a guess.
    const uploadMissing = [];
    for (const st of s.steps) {
      const d = st && st.draft;
      if (!d || d.verb !== "upload") continue;
      const lit = d.value && d.value.literal;
      const names = Array.isArray(lit) ? lit : [lit].filter(Boolean);
      const got = new Set(
        (Array.isArray(st.uploads) ? st.uploads : [])
          .filter((u) => u && u.data)
          .map((u) => u.name),
      );
      for (const n of names) {
        if (!got.has(String(n).replace(/^files\//, ""))) uploadMissing.push(n);
      }
    }
    if (uploadMissing.length) {
      (bundle.warnings ||= []).push(
        `file upload(s) recorded without contents — ` +
          `drop ${uploadMissing.join(", ")} under the scenario dir before replaying`,
      );
    }
    if (s.botWallSteps) {
      (bundle.warnings ||= []).push(
        `${s.botWallSteps} interaction(s) captured while the site showed a ` +
          "security-verification page (Cloudflare class) — those steps drive a " +
          "challenge widget that won't exist at replay; delete them after ingest " +
          "or re-record past the wall",
      );
    }
    const host = safeHost(s.url) || "page";
    const stamp = s.startedAt.replace(/[:.]/g, "-").slice(0, 19);
    // One-press UX: when `agent-qa ingest --listen` is running the
    // bundle posts straight to it — no file to find, attach, or send.
    // Nothing listening (or a refused ingest) falls back to the
    // download the flow always used; the daemon's refusal is worth
    // surfacing, a missing daemon is not.
    const sent = (await tryDaemon(bundle)) || {};
    const err = sent.sid
      ? null
      : await download(
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
    for (const t of s.tabs || [msg.tabId]) {
      sessions.delete(t);
      await chrome.storage.session.remove(keyOf(t)).catch(() => {});
    }
    return {
      recording: false,
      steps: s.steps.length,
      requests: s.network.length,
      sentSid: sent.sid || null,
      daemonError: sent.error || null,
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

// POST the bundle to `agent-qa ingest --listen` when one runs —
// returns {sid, ...} on acceptance, {error} when the daemon refused,
// null when nothing listens (the caller falls back to a download).
// A daemon answer — even a refusal — is a real outcome: the caller
// surfaces it. Only the absent-daemon case is silent.
async function tryDaemon(bundle) {
  try {
    const r = await fetch("http://127.0.0.1:17321/ingest", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(bundle),
    });
    const j = await r.json().catch(() => ({}));
    if (r.ok && j.sid) return j;
    return {
      error:
        j.error || `agent-qa daemon refused the bundle (HTTP ${r.status})`,
    };
  } catch {
    return null;
  }
}

// A closing tab silently drops its capture — but the close itself IS
// part of the flow (window.close(), a finished payment popup, the user
// tidying up): record it as `tab close tN` so replay closes the tab too
// instead of leaving a stale window open. Emit BEFORE deleting the
// in-memory session — s.tabs order still mirrors the refs already
// recorded, so tN names the right tab. Splice it out after so later
// popups renumber consistently with replay's open-order naming.
chrome.tabs.onRemoved.addListener((tabId) => {
  const s = sessions.get(tabId);
  if (s && Array.isArray(s.tabs)) {
    const n = s.tabs.indexOf(tabId) + 1;
    if (n > 0) {
      s.steps.push({
        kind: "do",
        draft: {
          intent: `close tab t${n}`,
          verb: "tab",
          value: { from: "literal", literal: `close t${n}` },
        },
      });
      s.tabs = s.tabs.filter((t) => t !== tabId);
      if (s.lastUrls) delete s.lastUrls[tabId];
      // The next step arriving from a surviving tab needs a switch-back
      // draft — replay is still pointed at the closed handle.
      if (s.activeTab === tabId) s.activeTab = null;
      // persist fans out to every s.tabs key — one call covers all
      // remaining tabs.
      if (s.tabs.length) persist(s.tabs[0]);
    }
  }
  sessions.delete(tabId);
  chrome.storage.session.remove(keyOf(tabId)).catch(() => {});
});

// Navigation capture: a link click that triggers a full nav, a
// hand-typed URL, or an SPA pushState all move the page mid-recording
// — with no matching goto step the bundle replays against a page that
// isn't there. webNavigation is worker-side so it survives the content
// script being torn down by the nav itself.
async function recordNav(tabId, url, transitionType) {
  const s = await getSession(tabId);
  if (!s) return;
  // The nav that opens an attached popup is implied by the recorded
  // opener click — skip it so replay doesn't goto the popup's URL a
  // second time on the freshly-switched tab.
  if (s.popupNavPending && s.popupNavPending.includes(tabId)) {
    s.popupNavPending = s.popupNavPending.filter((t) => t !== tabId);
    (s.lastUrls ||= {})[tabId] = url;
    persist(tabId);
    return;
  }
  // Dedupe: onCommitted and onHistoryStateUpdated can both fire for
  // one transition, and the landing URL is already the bundle's `url`.
  // Dedupe is per-tab — a popup's URL must not mask the main tab's.
  const lastUrls = (s.lastUrls ||= {});
  if (!url || lastUrls[tabId] === url) {
    // A same-URL commit is usually the dedupe hit — EXCEPT a real
    // reload (F5/Ctrl+R/form resubmit), which is a flow event: page
    // state resets and replay needs it too. transitionType names it.
    if (transitionType === "reload") {
      ensureActiveTab(s, tabId);
      s.currentFrame = [];
      s.steps.push({
        kind: "do",
        draft: {
          intent: "reload the page",
          verb: "reload",
        },
      });
      persist(tabId);
    } else if (transitionType === "form_submit") {
      // A click/press-driven submit needs no step — replaying the
      // recorded interaction resubmits live. A JS form.submit() leaves
      // nothing recorded, so without this the nav vanishes silently;
      // do/reload re-sends the POST — the closest replayable shape.
      const last = s.steps[s.steps.length - 1];
      if (!(last && last.kind === "do")) {
        ensureActiveTab(s, tabId);
        s.currentFrame = [];
        s.steps.push({
          kind: "do",
          draft: {
            intent: "form submission reloaded the page",
            verb: "reload",
          },
        });
        persist(tabId);
      }
    }
    return;
  }
  lastUrls[tabId] = url;
  // A nav on a non-active tab means focus moved — switch drafts first.
  ensureActiveTab(s, tabId);
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
  if (d.frameId === 0) recordNav(d.tabId, d.url, d.transitionType);
});
chrome.webNavigation.onHistoryStateUpdated.addListener((d) => {
  if (d.frameId === 0) recordNav(d.tabId, d.url, d.transitionType);
});

// A target=_blank link or window.open mid-recording spawns a new tab.
// Attach it to the SAME session so popup interactions (SSO, payment,
// confirmation dialogs-as-pages) land in the bundle — the first step or
// nav arriving from it emits the `tab tN` switch draft replay needs.
// Its content script self-arms via the state ping (getSession resolves
// the shared session), and its opening nav is skipped via
// popupNavPending since the recorded opener click already implies it.
chrome.webNavigation.onCreatedNavigationTarget.addListener((d) => {
  getSession(d.sourceTabId).then((s) => {
    if (!s || d.sourceTabId === d.tabId) return;
    tabIndex(s, d.tabId);
    sessions.set(d.tabId, s);
    (s.popupNavPending ||= []).push(d.tabId);
    persist(d.tabId);
    chrome.action.setBadgeText({ tabId: d.tabId, text: "REC" });
    chrome.action.setBadgeBackgroundColor({ tabId: d.tabId, color: "#d33" });
  });
});
