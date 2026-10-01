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

  // True once the page-world patch posted its ready beacon on this
  // document. Forwarded to the worker on record:start (and re-forwarded
  // when it arrives mid-recording after a nav) so the export bundle can
  // warn when capture never armed — CSP-strict pages block the inject.
  let netReady = false;

  window.addEventListener("message", (e) => {
    if (e.source !== window || !e.data) return;
    if (e.data.__aqNetReady) {
      netReady = true;
      if (active) send({ t: "netReady" });
      return;
    }
    if (e.data.__aqDialog) {
      if (!active) return;
      const d = e.data.__aqDialog;
      const message = String(d.message ?? "");
      // Same pair the workbench emits off Page.javascriptDialogOpening:
      // a check pinning the message, then the resolve step — but here the
      // user's real answer is known, so confirm/prompt record the actual
      // action instead of an unconditional accept.
      emitStep({
        kind: "check",
        draft: {
          intent: `${d.type} dialog says "${message.slice(0, 80)}"`,
          claim: {
            subject: { dialog: true },
            predicate: "contains",
            value: message,
          },
        },
      });
      const params =
        (d.type === "confirm" && d.result !== true) ||
        (d.type === "prompt" && d.result === null)
          ? { action: "dismiss" }
          : { action: "accept" };
      if (params.action === "accept" && d.type === "prompt") {
        params.text = String(d.result ?? "");
      }
      emitStep({
        kind: "do",
        draft: {
          intent: `${d.type} dialog → ${params.action}`,
          verb: "dialog",
          params,
        },
      });
      return;
    }
    if (!e.data.__aqNet || !active) return;
    send({ t: "net", entry: e.data.__aqNet });
  });

  const send = (msg) => {
    try {
      chrome.runtime.sendMessage(msg).catch(() => {});
    } catch {}
  };

  chrome.runtime.onMessage.addListener((msg) => {
    if (msg.t === "record:start") {
      active = true;
      send({ t: "netReady", ready: netReady });
    }
    if (msg.t === "record:stop") {
      // A scroll sitting inside the debounce window would silently
      // drop — flush it before the flag flips.
      flushScroll();
      active = false;
    }
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
    // No spaces around `>` — the scenario-schema shorthand requires
    // `^(css|xpath|testId|text):\S+$`, so "a > b" fails ingest.
    return parts.join(">");
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

  // A `css:` path stops at shadow boundaries — selectors can't pierce
  // them on replay either, so a click inside a shadow root would record
  // a locator that can never resolve. Emit a role locator instead: it
  // resolves through the a11y tree, which crosses *open* roots.
  const IMPLICIT_ROLE = {
    button: "button",
    summary: "button",
    a: "link",
    select: "combobox",
    textarea: "textbox",
    img: "img",
    option: "option",
    li: "listitem",
  };
  const INPUT_ROLE = {
    checkbox: "checkbox",
    radio: "radio",
    submit: "button",
    button: "button",
    image: "button",
    reset: "button",
    range: "slider",
    search: "searchbox",
  };

  function implicitRole(el) {
    const explicit = el.getAttribute && el.getAttribute("role");
    if (explicit) return explicit;
    if (el.localName === "input") {
      const t = (el.getAttribute("type") || "text").toLowerCase();
      return INPUT_ROLE[t] || "textbox";
    }
    if (/^h[1-6]$/.test(el.localName)) return "heading";
    return IMPLICIT_ROLE[el.localName] || "generic";
  }

  function locator(el) {
    if (el.getRootNode && el.getRootNode() instanceof ShadowRoot) {
      const raw = (
        (el.getAttribute && el.getAttribute("aria-label")) ||
        (el.innerText || "").trim().slice(0, 40) ||
        el.getAttribute?.("name") ||
        ""
      );
      return { role: implicitRole(el), ...(raw ? { name: raw } : {}) };
    }
    return `css:${cssPath(el)}`;
  }

  const doDraft = (intent, verb, extra) => ({
    kind: "do",
    draft: { intent, verb, ...extra },
  });
  const literal = (v) => ({ from: "literal", literal: String(v) });

  // The chain of iframe selectors from the top document down to this
  // one — empty at top level, one selector per same-origin ancestor.
  // Each frame runs its own copy of this script, so a click inside an
  // iframe arrives here with its own document scope; the worker pairs
  // it with `frame` transition drafts so replay can re-enter. A
  // cross-origin hop anywhere in the chain means the embedding element
  // is unreachable — the worker drops those steps and warns, since a
  // locator recorded against the wrong document is worse than none.
  const frameCtx = (() => {
    const chain = [];
    let w = window;
    while (w !== w.top) {
      try {
        if (!w.frameElement) return "cross-origin";
        chain.unshift(cssPath(w.frameElement));
        w = w.parent;
      } catch {
        return "cross-origin";
      }
    }
    return chain;
  })();

  // Cloudflare-class interstitial: interactions captured while the wall
  // is up (its checkbox inside a cross-origin iframe counts as a click on
  // the frame's host) target a widget that won't exist at replay — flag
  // them so the bundle can say so instead of shipping a dead scenario.
  const onChallengePage = () =>
    document.title === "Just a moment" ||
    !!document.querySelector(
      'script[src*="challenges.cloudflare"], iframe[src*="challenges.cloudflare"], #challenge-form, .cf-chl-widget',
    );

  const stepMsg = (item) => ({
    t: "step",
    item,
    frame: frameCtx,
    wall: onChallengePage(),
  });

  // ---------- interaction capture ----------

  // A pending scroll debounce must flush before ANY other step goes
  // out — a scroll that happened before a click but emits after it
  // reorders the recorded flow.
  const emitStep = (item) => {
    flushScroll();
    send(stepMsg(item));
  };

  // Clicks emit at event time — debouncing them to merge dblclicks
  // used to reorder them after their own effects (dialogs, navs) and
  // drop them entirely when a nav tore the document down first. A real
  // dblclick sends click,click,dblclick: the two clicks are already
  // recorded, so the dblclick carries a merge flag and the worker
  // swaps them out.
  document.addEventListener(
    "click",
    (e) => {
      if (!active || !e.isTrusted) return;
      // e.target retargets to the shadow host at document level —
      // composedPath()[0] is the real element the click landed on.
      const real = e.composedPath?.()[0] || e.target;
      const el =
        real.closest?.(
          "a[href],button,[role=button],[role=link],input[type=submit],input[type=button],summary,[onclick]"
        ) || real;
      emitStep(
        doDraft(`click ${label(el)}`, "click", {
          on: locator(el),
        })
      );
    },
    true
  );

  document.addEventListener(
    "dblclick",
    (e) => {
      if (!active || !e.isTrusted) return;
      const real = e.composedPath?.()[0] || e.target;
      const el = real.closest?.("a,button,[role=button],input,summary") || real;
      flushScroll();
      const msg = stepMsg(
        doDraft(`double-click ${label(el)}`, "dblclick", {
          on: locator(el),
        })
      );
      msg.merge = "dblclick";
      send(msg);
    },
    true
  );

  document.addEventListener(
    "change",
    (e) => {
      if (!active || !e.isTrusted) return;
      const el = e.composedPath?.()[0] || e.target;
      const name = el.localName;
      if (name === "select") {
        // el.value is only the FIRST selected option — a multi-select
        // would silently lose the rest, and a valueless option's value
        // is "" which matches nothing. Replay matches values OR text
        // split on ',', so emit the full selection.
        const picked = el.multiple
          ? [...el.selectedOptions].map((o) => o.value || o.text)
          : [el.value || el.options[el.selectedIndex]?.text || ""];
        emitStep(
          doDraft(`select ${label(el)}`, "select", {
            on: locator(el),
            value: literal(picked.join(",")),
          })
        );
        return;
      }
      if (name !== "input" && name !== "textarea") return;
      const type = (el.getAttribute("type") || "text").toLowerCase();
      if (type === "checkbox" || type === "radio") {
        emitStep(
          doDraft(
            `${el.checked ? "check" : "uncheck"} ${label(el)}`,
            el.checked ? "check" : "uncheck",
            { on: locator(el) }
          )
        );
        return;
      }
      if (type === "file") {
        // Record an upload step referencing files/<name> so the flow
        // replays, and inline small file contents into the bundle —
        // input.files IS readable from the isolated world, so ingest can
        // materialize files/<name> and the flow replays end-to-end with
        // no manual file drop. Oversize/unreadable files keep the
        // name-only ref + the export warning that names them.
        const INLINE_CAP = 256 * 1024;
        const files = [...(el.files || [])].filter((f) => f && f.name);
        if (!files.length) return;
        const item = doDraft(`upload ${files.map((f) => f.name).join(", ")}`, "upload", {
          on: locator(el),
          value: {
            from: "literal",
            literal:
              files.length === 1
                ? `files/${files[0].name}`
                : files.map((f) => `files/${f.name}`),
          },
        });
        // The draft lands now (in interaction order); file contents
        // arrive on a follow-up augment message once read — waiting
        // for the reads would let a quick next-click reorder past it.
        // augmentId matches the augment to THIS draft when two
        // uploads race (extra envelope keys pass through ingest).
        const augmentId = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
        item.augmentId = augmentId;
        emitStep(item);
        const reads = files.map(
          (f) =>
            new Promise((res) => {
              if (f.size > INLINE_CAP) {
                res({ name: f.name, skipped: "too large" });
                return;
              }
              const r = new FileReader();
              r.onload = () =>
                res({ name: f.name, type: f.type || "", data: r.result });
              r.onerror = () => res({ name: f.name, skipped: "unreadable" });
              try {
                r.readAsDataURL(f);
              } catch {
                res({ name: f.name, skipped: "unreadable" });
              }
            }),
        );
        Promise.all(reads).then((uploads) => {
          send({ t: "augment", id: augmentId, uploads });
        });
        return;
      }
      if (el.value === "") return;
      emitStep(
        doDraft(`type into ${label(el)}`, "type", {
          on: locator(el),
          value: literal(el.value),
        })
      );
    },
    true
  );

  // Keyboard-driven widgets fire no change/click events — arrow-key
  // sliders, tab lists, listboxes, Escape-dismissed modals would record
  // nothing and replay would diverge from the flow the user performed.
  const WIDGET_ROLES = new Set([
    "slider",
    "spinbutton",
    "tab",
    "option",
    "listbox",
    "menuitem",
    "treeitem",
    "radio",
  ]);
  const NAV_KEYS = new Set([
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Home",
    "End",
    "PageUp",
    "PageDown",
  ]);
  document.addEventListener(
    "keydown",
    (e) => {
      if (!active || !e.isTrusted) return;
      const el = e.composedPath?.()[0] || e.target;
      const role = el && el.getAttribute && el.getAttribute("role");
      const widget =
        (role && WIDGET_ROLES.has(role)) ||
        (el &&
          el.localName === "input" &&
          (el.getAttribute("type") || "").toLowerCase() === "range");
      const record =
        e.key === "Escape" ||
        (NAV_KEYS.has(e.key) && widget) ||
        (e.key === "Enter" &&
          (widget ||
            (el && (el.localName === "input" || el.localName === "textarea"))));
      if (!record) return;
      emitStep(
        doDraft(`press ${e.key}`, "press", { value: literal(e.key) })
      );
    },
    true
  );

  // Scrolls matter when the flow depends on revealed content — lazy
  // lists, anchors, forms below the fold. Debounce bursts into one
  // scrollTo per target; window scrolls replay exactly via params.y /
  // to:"bottom", element scrolls degrade to the verb's scrollIntoView
  // (exact scrollTop isn't expressible by the replay verb).
  let scrollTimer = null;
  let lastScrollEl = null;
  const emitScroll = (target) => {
    const isWindow =
      target === document.scrollingElement ||
      target === document.documentElement ||
      target === document.body;
    if (isWindow) {
      const y = Math.round(window.scrollY);
      const bottom = target.scrollHeight - target.clientHeight;
      const params = bottom - y <= 2 ? { to: "bottom" } : { y };
      send(stepMsg(doDraft("scroll page", "scrollTo", { params })));
    } else {
      // Element scrolls must use css — replay's scrollTo rejects
      // role locators (scrollIntoView path only handles raw).
      send(
        stepMsg(
          doDraft(`scroll ${label(target)}`, "scrollTo", {
            on: `css:${cssPath(target)}`,
          })
        )
      );
    }
  };
  function flushScroll() {
    if (!scrollTimer || !lastScrollEl) return;
    clearTimeout(scrollTimer);
    scrollTimer = null;
    const target = lastScrollEl;
    lastScrollEl = null;
    emitScroll(target);
  }
  document.addEventListener(
    "scroll",
    (e) => {
      if (!active || !e.isTrusted) return;
      const el =
        e.target === document
          ? document.scrollingElement || document.documentElement
          : e.target;
      if (!el || el.nodeType !== 1) return;
      lastScrollEl = el;
      clearTimeout(scrollTimer);
      scrollTimer = setTimeout(() => {
        scrollTimer = null;
        const target = lastScrollEl;
        lastScrollEl = null;
        if (target) emitScroll(target);
      }, 400);
    },
    { capture: true, passive: true }
  );
  // A scroll left inside the debounce window when the page unloads
  // dies with the document — flush on the way out.
  window.addEventListener("pagehide", flushScroll);

  // Service worker answers with the tab's recording flag so a
  // mid-recording full navigation re-arms this document.
  try {
    chrome.runtime.sendMessage({ t: "state" }).then((r) => {
      if (r && r.recording) {
        active = true;
        send({ t: "netReady", ready: netReady });
      }
    });
  } catch {}
})();
