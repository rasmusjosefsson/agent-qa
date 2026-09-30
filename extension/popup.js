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

const boot = async () => {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  tabId = tab && tab.id;
  await status();
  if (recording && !poller) poller = setInterval(status, 1000);
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
    poller = setInterval(status, 1000);
  } else {
    clearInterval(poller);
    poller = null;
    const r = await chrome.runtime
      .sendMessage({ t: "popup:stop", tabId })
      .catch(() => null);
    recording = false;
    paint();
    meta.textContent = r
      ? `saved — ${r.steps} steps, ${r.requests} requests`
      : "saved";
  }
});

boot();
