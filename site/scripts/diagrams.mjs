// Hand-laid diagrams that replace the docs' mermaid blocks on the site.
// Each renders to a single-line HTML string (raw HTML blocks in markdown end
// at a blank line) styled by src/styles/diagrams.css. `match` is a substring
// of the mermaid source it replaces, so reordering docs/ can't mis-assign.

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")

// kind: step | core | artifact | bad | good
function node({ x, y, w, h = 64, title, sub = [], kind = "step", mono = false, subMono = false }) {
  const lines = [].concat(sub)
  const cx = x + w / 2
  const top = y + h / 2 - (lines.length * 14) / 2 + 4
  const t = `<text class="d-title${mono ? " d-mono" : ""}" x="${cx}" y="${top}">${esc(title)}</text>`
  const s = lines
    .map((l, i) => `<text class="d-sub${subMono ? " d-mono" : ""}" x="${cx}" y="${top + 17 + i * 14}">${esc(l)}</text>`)
    .join("")
  return `<g class="d-node d-${kind}"><rect x="${x}" y="${y}" width="${w}" height="${h}" rx="11"/>${t}${s}</g>`
}

// kind: "" (plain) | flow (animated brand dash) | soft (dashed, secondary)
function edge(d, kind = "") {
  const marker = kind === "flow" ? "b" : "m"
  return `<path class="d-edge${kind ? ` d-${kind}` : ""}" d="${d}" marker-end="url(#ARROW-${marker})"/>`
}

const label = (x, y, text, anchor = "middle") =>
  `<text class="d-label" x="${x}" y="${y}" text-anchor="${anchor}">${esc(text)}</text>`

let seq = 0
function svg(w, h, name, body) {
  const id = `aqa-d${++seq}`
  const defs =
    `<defs>` +
    `<marker id="${id}-m" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path class="d-head" d="M1 1L9 5L1 9z"/></marker>` +
    `<marker id="${id}-b" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path class="d-head d-head-b" d="M1 1L9 5L1 9z"/></marker>` +
    `<linearGradient id="${id}-g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" class="d-g1"/><stop offset="1" class="d-g2"/></linearGradient>` +
    `</defs>`
  const content = body.join("").replaceAll("ARROW", id).replaceAll('class="d-node d-core"', `class="d-node d-core" style="--d-grad:url(#${id}-g)"`)
  return `<figure class="aqa-diagram not-content"><svg viewBox="0 0 ${w} ${h}" role="img" aria-label="${esc(name)}">${defs}${content}</svg></figure>`
}

const row = (xs, y, w, h = 64) => ({ xs, y, w, h, mid: y + h / 2 })
const chain = (xs, widths, y) =>
  xs.slice(0, -1).map((x, i) => edge(`M${x + widths[i] + 2} ${y}H${xs[i + 1] - 3}`, "flow"))

// ── docs/index.md: the loop ─────────────────────────────────────────────
export function loop() {
  const r = row([0, 131, 262, 393, 524, 655], 24, 104, 62)
  const W = Array(6).fill(r.w)
  const top = [
    { title: "Intent", sub: "what you meant" },
    { title: "Record", sub: "observe each step", kind: "core" },
    { title: "scenario.json", sub: "sealed contract", kind: "artifact", mono: true },
    { title: "Replay", sub: "deterministic", kind: "core" },
    { title: "Compare", sub: "diff two runs", kind: "core" },
    { title: "Report", sub: "compare.md", kind: "artifact" },
  ]
  return svg(760, 262, "The agent-qa loop: intent, record, scenario, replay, compare, report; record writes evidence, evidence feeds heal, heal patches the scenario", [
    ...top.map((n, i) => node({ ...n, x: r.xs[i], y: r.y, w: r.w, h: r.h })),
    ...chain(r.xs, W, r.mid),
    node({ x: 113, y: 176, w: 140, h: 72, title: "Evidence", sub: ["snapshots, screenshots,", "probes, network"], kind: "artifact" }),
    node({ x: 290, y: 176, w: 140, h: 72, title: "Heal", sub: ["bounded patch", "with an audit trail"], kind: "core" }),
    edge(`M183 88V172`, "soft"),
    edge(`M255 212H286`),
    edge(`M360 174C360 128 314 132 314 90`),
    label(372, 136, "patches", "start"),
    // legend
    `<g class="d-legend" transform="translate(486 204)">` +
      `<rect class="d-lg-step" x="0" y="0" width="18" height="12" rx="3"/><text x="24" y="10">step</text>` +
      `<rect class="d-lg-core" x="70" y="0" width="18" height="12" rx="3"/><text x="94" y="10">verb</text>` +
      `<rect class="d-lg-art" x="140" y="0" width="18" height="12" rx="3"/><text x="164" y="10">on disk</text>` +
      `<path class="d-edge d-flow" d="M0 36H18"/><text x="24" y="40">replay path, no LLM in the loop</text>` +
      `</g>`,
  ])
}

// ── docs/architecture.md: where it sits ─────────────────────────────────
export function system() {
  const xs = [0, 142, 364, 518, 656]
  const ws = [108, 112, 120, 104, 104]
  const y = 118
  const nodes = [
    { title: "Coding agent", sub: "QA agent or human" },
    { title: "agent-qa", sub: "Rust CLI", kind: "core", mono: true },
    { title: "agent-browser", sub: "CDP driver", mono: true },
    { title: "Chromium tab", sub: "real browser" },
    { title: "Target app", sub: "any web app" },
  ]
  return svg(760, 300, "agent-qa sits between the agent and agent-browser, calls plugins, and writes evidence to disk", [
    ...nodes.map((n, i) => node({ ...n, x: xs[i], y, w: ws[i] })),
    ...chain(xs, ws, y + 32),
    label(309, 142, "spawn per gesture"),
    node({ x: 103, y: 14, w: 190, h: 62, title: "Plugin", sub: "any language · JSON over stdio" }),
    edge(`M198 116V80`),
    label(206, 102, "auth · session · hooks", "start"),
    node({ x: 196, y: 226, w: 230, h: 62, title: "Evidence on disk", sub: "audit.json, sidecars, screenshots", kind: "artifact" }),
    edge(`M222 184V222`, "soft"),
    edge(`M400 184V222`, "soft"),
  ])
}

// ── docs/architecture.md: module map ────────────────────────────────────
export function modules() {
  const groups = [
    ["Setup", ["skills", "plugins", "scenario validate"]],
    ["Record", ["start", "record-step", "fill-unique", "smart-click", "truncate", "flush", "verify"]],
    ["Replay", ["replay", "list", "compare"]],
    ["Heal", ["heal-respond", "heal-promote", "heal-apply"]],
    ["Profiles", ["profile-add", "profile-status", "profile-bootstrap"]],
    ["Diagnostics", ["doctor", "byo-doctor", "perf-snapshot"]],
  ]
  const infra = ["paths.rs", "scenario.rs", "schema.rs", "sidecar.rs", "value.rs", "browser.rs", "plugin/", "env_ops.rs", "verbs.rs", "verb_shape.rs", "claims.rs"]
  const chips = (xs) => xs.map((x) => `<li>${esc(x)}</li>`).join("")
  const verbs = groups.map(([g, xs]) => `<div class="aqa-mods-group"><span>${g}</span><ul>${chips(xs)}</ul></div>`).join("")
  return (
    `<figure class="aqa-diagram aqa-mods not-content" aria-label="Verb modules call into shared infrastructure">` +
    `<div class="aqa-mods-panel"><h4>Verbs</h4>${verbs}</div>` +
    `<div class="aqa-mods-arrow" aria-hidden="true"><span>call into</span></div>` +
    `<div class="aqa-mods-panel aqa-mods-infra"><h4>Infrastructure</h4><ul>${chips(infra)}</ul></div>` +
    `</figure>`
  )
}

// ── docs/architecture.md: recording flow ────────────────────────────────
export function record() {
  const xs = [0, 158, 316, 474, 632]
  const ws = Array(5).fill(128)
  const y = 70
  const nodes = [
    { title: "agent-qa start", sub: "with an intent", mono: true },
    { title: "Step buffer", sub: "recorder-state.json", kind: "artifact", subMono: true },
    { title: "Drive the page", sub: "via agent-browser" },
    { title: "record-step", sub: ["smart-click,", "fill-unique"], kind: "core", mono: true },
    { title: "Step saved", sub: ["+ snapshot,", "screenshot"], kind: "artifact" },
  ]
  return svg(760, 290, "Recording: start, buffer, drive the page, record-step, save evidence, repeat; truncate to fix up; flush to scenario.json", [
    ...nodes.map((n, i) => node({ ...n, x: xs[i], y, w: 128 })),
    ...chain(xs, ws, y + 32),
    edge(`M696 68C696 20 380 20 380 66`, "soft"),
    label(538, 38, "next gesture"),
    edge(`M672 136C672 190 400 190 400 138`, "soft"),
    label(536, 182, "truncate <N> · optional fix-up"),
    node({ x: 632, y: 214, w: 128, h: 56, title: "flush", sub: "seal the buffer", kind: "core", mono: true }),
    edge(`M730 136V210`, "flow"),
    node({ x: 474, y: 214, w: 128, h: 56, title: "scenario.json", sub: "sealed contract", kind: "artifact", mono: true }),
    edge(`M630 242H605`, "flow"),
  ])
}

// ── docs/architecture.md: replay + compare ──────────────────────────────
export function replay() {
  const xs = [0, 159, 318, 477, 636]
  const ws = Array(5).fill(124)
  const y = 30
  const h = 72
  const nodes = [
    { title: "scenario.json", sub: "sealed contract", kind: "artifact", mono: true },
    { title: "replay", sub: ["--profile p", "--heal-from-run r"], kind: "core", mono: true, subMono: true },
    { title: "replays/<runId>/", sub: ["audit.json +", "sidecars"], kind: "artifact", mono: true },
    { title: "compare", sub: "recording vs replay", kind: "core", mono: true },
    { title: "compare.md", sub: ["+ per-step diffs", "under compare/<run>/"], kind: "artifact", mono: true },
  ]
  return svg(760, 190, "Replay: scenario.json replays to a run folder; compare diffs it against the recording and writes compare.md", [
    ...nodes.map((n, i) => node({ ...n, x: xs[i], y, w: 124, h })),
    ...chain(xs, ws, y + h / 2),
    edge(`M62 104C62 172 539 172 539 106`, "soft"),
    label(300, 164, "recording side"),
  ])
}

// ── docs/architecture.md: heal ──────────────────────────────────────────
export function heal() {
  return svg(760, 254, "Heal: a failed replay step gets a heal-respond value, then either heal-apply and re-record, or replay with --heal-from-run for a fresh run", [
    label(390, 20, "FIX THE RECORDING", "start"),
    label(390, 248, "OVERRIDE AT REPLAY", "start"),
    node({ x: 0, y: 98, w: 140, title: "Replay step fails", sub: "value rejected by app", kind: "bad" }),
    node({ x: 180, y: 98, w: 150, title: "heal-respond", sub: "--value <corrected>", kind: "core", mono: true, subMono: true }),
    node({ x: 390, y: 30, w: 160, title: "heal-apply", sub: "fix up the buffer", mono: true }),
    node({ x: 600, y: 30, w: 160, title: "Re-record", sub: "record-step from N", kind: "good" }),
    node({ x: 390, y: 166, w: 160, title: "replay", sub: "--heal-from-run", mono: true, subMono: true }),
    node({ x: 600, y: 166, w: 160, title: "Fresh run", sub: "new <runId>", kind: "good" }),
    edge(`M142 130H177`, "flow"),
    edge(`M332 130C362 130 358 62 387 62`, "flow"),
    edge(`M332 130C362 130 358 198 387 198`, "flow"),
    edge(`M552 62H597`),
    edge(`M552 198H597`),
  ])
}

export const DIAGRAMS = [
  { match: 'Intent["Intent', render: loop },
  { match: 'Agent["Coding / QA agent', render: system },
  { match: "subgraph Infrastructure", render: modules },
  { match: "recorder-state.json", render: record },
  { match: "ReplayDir[", render: replay },
  { match: 'Respond["agent-qa heal-respond', render: heal },
  { match: "──stdin JSON──▶", render: pluginStdio },
]

// ── docs/plugins.md: stdio protocol as a sequence diagram ───────────────
export function pluginStdio() {
  const a = 150
  const p = 590
  const msg = (y, from, to, text, kind = "flow") =>
    edge(`M${from + (to > from ? 8 : -8)} ${y}H${to + (to > from ? -10 : 10)}`, kind) + label(370, y - 9, text)
  return svg(760, 236, "agent-qa spawns the plugin binary, writes a JSON request on stdin, and reads one JSON response from stdout", [
    node({ x: a - 90, y: 8, w: 180, h: 52, title: "agent-qa", sub: "host", kind: "core", mono: true }),
    node({ x: p - 130, y: 8, w: 260, h: 52, title: "<plugin-binary> <kind> [<op>]", sub: "any language", mono: true }),
    `<path class="d-edge d-soft" d="M${a} 62V226"/><path class="d-edge d-soft" d="M${p} 62V226"/>`,
    `<rect class="d-activation" x="${p - 5}" y="92" width="10" height="112" rx="3"/>`,
    msg(100, a, p, "spawn with positional args"),
    msg(146, a, p, "request · JSON on stdin"),
    msg(196, p, a, "response · one JSON object on stdout", ""),
  ])
}

// ── Any ├── / └── block: a visual file tree ─────────────────────────────
const FILE_TINT = { json: "json", jsonl: "json", png: "img", txt: "text", md: "doc", diff: "diff" }
const ICON_DIR = `<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M1.5 4.25c0-.97.78-1.75 1.75-1.75h2.9c.46 0 .9.18 1.23.51l.87.87c.14.14.33.22.53.22h4c.97 0 1.72.78 1.72 1.75v5.9c0 .97-.78 1.75-1.75 1.75H3.25c-.97 0-1.75-.78-1.75-1.75z"/></svg>`
const ICON_FILE = `<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 1.5h5.1c.33 0 .65.13.88.37l2.65 2.65c.24.23.37.55.37.88v8.35c0 .69-.56 1.25-1.25 1.25H4c-.69 0-1.25-.56-1.25-1.25V2.75c0-.69.56-1.25 1.25-1.25z"/></svg>`

const fmtName = (s) => esc(s).replace(/&lt;([^&]+?)&gt;/g, `<span class="ph">&lt;$1&gt;</span>`)

export function isAsciiTree(src) {
  return /^[│\s]*[├└]──/m.test(src)
}

export function fileTree(src) {
  const lines = src.replace(/\n+$/, "").split("\n")
  const root = { name: lines[0].trim(), children: [], depth: 0 }
  const stack = [root]
  for (const raw of lines.slice(1)) {
    const m = raw.match(/^([│ \t]*)[├└]──\s?(.*)$/)
    if (!m) continue
    const depth = Math.round(m[1].replace(/\t/g, "    ").length / 4) + 1
    // "name   note" (2+ spaces) or "name (note)"
    let [, name, note = ""] = m[2].match(/^(\S+)(?:\s{2,}(.+)|\s+(\(.+\)))?$/) ?? [null, m[2].trim()]
    if (!note) note = (m[2].match(/^\S+\s+(\(.+\))$/) ?? [])[1] ?? ""
    const item = { name, note: note.trim(), children: [], depth }
    while (stack.length > depth) stack.pop()
    stack[stack.length - 1].children.push(item)
    stack.push(item)
  }
  const render = (n) => {
    const dir = n.name.endsWith("/") || n.children.length > 0
    const ext = dir ? "dir" : (FILE_TINT[n.name.split(".").pop()] ?? "file")
    const row =
      `<div class="tree-row t-${ext}">${dir ? ICON_DIR : ICON_FILE}` +
      `<span class="tree-name">${fmtName(n.name)}</span>` +
      (n.note ? `<span class="tree-note">${esc(n.note.replace(/^\((.*)\)$/, "$1"))}</span>` : "") +
      `</div>`
    const kids = n.children.length ? `<ul>${n.children.map((c) => `<li>${render(c)}</li>`).join("")}</ul>` : ""
    return row + kids
  }
  return `<figure class="aqa-tree not-content" aria-label="Directory layout">${render(root)}</figure>`
}

// ── Single-line "a → b → c" blocks: a numbered step strip ───────────────
export function isArrowChain(src) {
  const lines = src.trim().split("\n")
  return lines.length === 1 && lines[0].split(" → ").length > 2
}

export function stepStrip(src) {
  const steps = src.trim().split(" → ")
  const looksLiteral = (s) => /[:/.]/.test(s) // URLs, paths → mono chip
  const items = steps
    .map(
      (s, i) =>
        `<li><span class="strip-n">${i + 1}</span><span class="${looksLiteral(s) ? "strip-code" : "strip-text"}">${esc(s)}</span></li>`,
    )
    .join("")
  return `<figure class="aqa-strip not-content" aria-label="${esc(steps.join(", then "))}"><ol>${items}</ol></figure>`
}
