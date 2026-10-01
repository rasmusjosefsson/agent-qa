const btn = document.getElementById("btn");
const meta = document.getElementById("meta");
let recording = false;
let tabId = null;
let poller = null;

const paint = () => {
  btn.textContent = recording ? "■ Stop & export" : "Record";
  btn.className = recording ? "live" : "idle";
};

const status = async () => {
  const r = await chrome.runtime
    .sendMessage({ t: "popup:status", tabId })
    .catch(() => null);
  if (!r) return;
  recording = r.recording;
  paint();
  if (recording) {
    meta.textContent = r.captureDead
      ? `${r.steps} steps · paused — this page can't be captured`
      : `${r.steps} steps · ${r.requests} requests`;
  }
};

// Is `agent-qa ingest --listen` up? The popup probes once on open so
// the recorder knows whether Stop will send the bundle straight to the
// CLI or download a file — same button either way, different result.
const daemonUp = async () => {
  try {
    const r = await fetch("http://127.0.0.1:17321/health", {
      signal: AbortSignal.timeout(1500),
    });
    const j = await r.json().catch(() => ({}));
    return r.ok && j.ok === true;
  } catch {
    return false;
  }
};

const boot = async () => {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  tabId = tab && tab.id;
  await status();
  if (recording && !poller) poller = setInterval(status, 1000);
  if (!recording) {
    meta.textContent = (await daemonUp())
      ? "agent-qa daemon online — export sends straight to it"
      : "no daemon — export downloads a file";
  }
};

btn.addEventListener("click", async () => {
  if (!recording) {
    const r = await chrome.runtime
      .sendMessage({ t: "popup:start", tabId })
      .catch(() => null);
    if (r && r.error) {
      meta.textContent = r.error;
      return;
    }
    recording = true;
    paint();
    meta.textContent = "recording…";
    // A fast double-click can reach here twice — without the guard the
    // first interval leaks, keeps polling after stop, and overwrites the
    // "saved" message.
    if (!poller) poller = setInterval(status, 1000);
  } else {
    clearInterval(poller);
    poller = null;
    const r = await chrome.runtime
      .sendMessage({ t: "popup:stop", tabId })
      .catch(() => null);
    if (r && r.error) {
      // Export failed (or nothing was recording) — keep the button in
      // the state the background reports so a retry stays possible.
      recording = !!r.recording;
      paint();
      meta.textContent = r.error;
      return;
    }
    recording = false;
    paint();
    meta.textContent = !r
      ? "saved"
      : r.sentSid
        ? `sent to agent-qa — ${r.sentSid} (${r.steps} steps, ${r.requests} requests` +
          (r.sentWarnings ? `, ${r.sentWarnings} warning(s)` : ``) +
          `)`
        : r.daemonError
          ? `downloaded — daemon said: ${r.daemonError}`
          : `saved — ${r.steps} steps, ${r.requests} requests`;
  }
});

boot();
