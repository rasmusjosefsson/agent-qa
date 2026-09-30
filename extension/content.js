// Isolated-world content script, loaded at document_start on every
// http(s) page. Two jobs: relay network entries the injected
// page-world script posts, and turn user interactions into
// record-step drafts the CLI's `ingest` verb validates.
//
// Drafts follow the same shape `agent-qa record-step` accepts:
//   {kind: "do", draft: {intent, verb, on: "css:sel", value: {from,literal}}}
(() => {
  let active = false;

  // Install the page-world network patch. It rides every document so
  // toggling recording on never needs a reload; the patch itself is
  // inert unless the relay below forwards entries while active.
  const script = document.createElement("script");
  script.src = chrome.runtime.getURL("inject.js");
  script.onload = () => script.remove();
  (document.documentElement || document.head).appendChild(script);

  window.addEventListener("message", (e) => {
    if (e.source !== window || !e.data || !e.data.__aqNet) return;
    if (!active) return;
    send({ t: "net", entry: e.data.__aqNet });
  });

  const send = (msg) => {
    try {
      chrome.runtime.sendMessage(msg).catch(() => {});
    } catch {}
  };

  chrome.runtime.onMessage.addListener((msg) => {
    if (msg.t === "record:start") active = true;
    if (msg.t === "record:stop") active = false;
  });
  // Page may have loaded mid-recording (full nav); ask the worker
  // whether this tab is being recorded (the .then() below re-arms).

  // ---------- selector / label helpers ----------

  const UNSTABLE_CLASS = /(^|-)[0-9a-f]{5,}$/i; // tailwind/css-in-js hash noise

  function cssPath(el) {
    if (!el || el.nodeType !== 1) return "html";
    const id = el.getAttribute && el.getAttribute("id");
    if (id) return "#" + CSS.escape(id);
    const qa =
      el.getAttribute &&
      (el.getAttribute("data-qa") ||
        el.getAttribute("data-testid") ||
        el.getAttribute("data-test-id") ||
        el.getAttribute("data-test"));
    if (qa) {
      const attr = el.getAttribute("data-qa")
        ? "data-qa"
        : el.getAttribute("data-testid")
          ? "data-testid"
          : el.getAttribute("data-test-id")
            ? "data-test-id"
            : "data-test";
      return `[${attr}="${qa.replace(/"/g, '\\"')}"]`;
    }
    const parts = [];
    let node = el;
    while (node && node.nodeType === 1 && node !== document.documentElement) {
      let sel = node.localName;
      if (node.id) {
        parts.unshift("#" + CSS.escape(node.id));
        break;
      }
      const stable = [...node.classList].filter(
        (c) => c && c.length < 32 && !UNSTABLE_CLASS.test(c)
      );
      if (stable.length) sel += "." + stable.slice(0, 2).map(CSS.escape).join(".");
      const parent = node.parentElement;
      if (parent) {
        const same = [...parent.children].filter(
          (c) => c.localName === node.localName
        );
        if (same.length > 1) sel += `:nth-of-type(${same.indexOf(node) + 1})`;
      }
      parts.unshift(sel);
      if (parts.length >= 4) break;
      node = parent;
    }
    return parts.join(" > ");
  }

  function label(el) {
    const t =
      (el.getAttribute && el.getAttribute("aria-label")) ||
      (el.innerText || "").trim().slice(0, 40) ||
      el.getAttribute?.("name") ||
      el.getAttribute?.("value") ||
      el.localName;
    return JSON.stringify(t);
  }

  const doDraft = (intent, verb, extra) => ({
    kind: "do",
    draft: { intent, verb, ...extra },
  });
  const literal = (v) => ({ from: "literal", literal: String(v) });

  // ---------- interaction capture ----------

  document.addEventListener(
    "click",
    (e) => {
      if (!active || !e.isTrusted) return;
      const el =
        e.target.closest?.(
          "a[href],button,[role=button],[role=link],input[type=submit],input[type=button],summary,[onclick]"
        ) || e.target;
      send({
        t: "step",
        item: doDraft(`click ${label(el)}`, "click", {
          on: `css:${cssPath(el)}`,
        }),
      });
    },
    true
  );

  document.addEventListener(
    "change",
    (e) => {
      if (!active || !e.isTrusted) return;
      const el = e.target;
      const name = el.localName;
      if (name === "select") {
        send({
          t: "step",
          item: doDraft(`select ${label(el)}`, "select", {
            on: `css:${cssPath(el)}`,
            value: literal(el.value),
          }),
        });
        return;
      }
      if (name !== "input" && name !== "textarea") return;
      const type = (el.getAttribute("type") || "text").toLowerCase();
      if (type === "checkbox" || type === "radio") {
        send({
          t: "step",
          item: doDraft(
            `${el.checked ? "check" : "uncheck"} ${label(el)}`,
            el.checked ? "check" : "uncheck",
            { on: `css:${cssPath(el)}` }
          ),
        });
        return;
      }
      if (type === "file") return; // native pickers can't be captured
      if (el.value === "") return;
      send({
        t: "step",
        item: doDraft(`type into ${label(el)}`, "type", {
          on: `css:${cssPath(el)}`,
          value: literal(el.value),
        }),
      });
    },
    true
  );

  document.addEventListener(
    "keydown",
    (e) => {
      if (!active || !e.isTrusted) return;
      if (e.key !== "Enter") return;
      const el = e.target;
      if (!el || (el.localName !== "input" && el.localName !== "textarea"))
        return;
      send({
        t: "step",
        item: doDraft("press Enter", "press", { value: literal("Enter") }),
      });
    },
    true
  );

  // Service worker answers with the tab's recording flag so a
  // mid-recording full navigation re-arms this document.
  try {
    chrome.runtime.sendMessage({ t: "state" }).then((r) => {
      if (r && r.recording) active = true;
    });
  } catch {}
})();
