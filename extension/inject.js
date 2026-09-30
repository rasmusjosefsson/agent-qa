// Runs in the page's MAIN world (injected by content.js). Patches
// fetch + XHR and posts every completed request back to the content
// script via window.postMessage. Captures response bodies — the piece
// webRequest cannot see — so the bundle can seed --mock-from.
(() => {
  if (window.__aqNetInstalled) return;
  window.__aqNetInstalled = true;

  const BODY_CAP = 256 * 1024; // keep the bundle downloadable
  const post = (entry) => {
    try {
      window.postMessage({ __aqNet: entry }, "*");
    } catch {}
  };
  const clip = (s) =>
    typeof s === "string" && s.length > BODY_CAP ? s.slice(0, BODY_CAP) : s;

  const origFetch = window.fetch;
  window.fetch = function (...args) {
    const startedAt = new Date().toISOString();
    const started = Date.now();
    let url = "";
    let method = "GET";
    let postData;
    try {
      const input = args[0];
      const init = args[1] || {};
      url = typeof input === "string" ? input : input.url;
      method = String(
        init.method || (typeof input !== "string" ? input.method : "GET") || "GET"
      ).toUpperCase();
      postData = typeof init.body === "string" ? init.body : undefined;
    } catch {}
    return origFetch.apply(this, args).then(
      (resp) => {
        const copy = resp.clone();
        copy
          .text()
          .then((body) =>
            post({
              url: copy.url || url,
              method,
              status: copy.status,
              body: clip(body),
              postData: clip(postData),
              startedAt,
              durationMs: Date.now() - started,
            })
          )
          .catch(() =>
            post({ url, method, status: resp.status, postData, startedAt })
          );
        return resp;
      },
      (err) => {
        post({ url, method, status: 0, error: String(err), startedAt });
        throw err;
      }
    );
  };

  const origOpen = XMLHttpRequest.prototype.open;
  const origSend = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function (m, u, ...rest) {
    this.__aq = { method: String(m).toUpperCase(), url: u };
    return origOpen.call(this, m, u, ...rest);
  };
  XMLHttpRequest.prototype.send = function (body) {
    const meta = this.__aq || {};
    const startedAt = new Date().toISOString();
    const started = Date.now();
    this.addEventListener("loadend", () => {
      let respBody;
      try {
        if (this.responseType === "" || this.responseType === "text") {
          respBody = this.responseText;
        }
      } catch {}
      post({
        url: String(meta.url || ""),
        method: meta.method || "GET",
        status: this.status,
        body: clip(respBody),
        postData: clip(typeof body === "string" ? body : undefined),
        startedAt,
        durationMs: Date.now() - started,
      });
    });
    return origSend.call(this, body);
  };

  // Beacon so the content script (and on record:stop, the export bundle)
  // can tell capture is armed — a CSP-strict page blocks the <script src>
  // injection and without this the bundle silently exports network: [].
  try {
    window.postMessage({ __aqNetReady: true }, "*");
  } catch {}
})();
