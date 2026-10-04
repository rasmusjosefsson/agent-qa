// Copies the repo's docs/*.md into Starlight's content dir so docs/ stays the
// single source of truth. Per file: the leading H1 becomes frontmatter
// `title`, ```mermaid fences become <pre class="mermaid"> for the client
// renderer, and relative links are rewritten to site routes (other docs) or
// GitHub URLs (anything else in the repo).
import { mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises"
import { dirname, join, posix } from "node:path"
import { fileURLToPath } from "node:url"
import { DIAGRAMS, fileTree, isArrowChain, isAsciiTree, stepStrip } from "./diagrams.mjs"

const here = dirname(fileURLToPath(import.meta.url))
const repo = join(here, "..", "..")
const out = join(here, "..", "src", "content", "docs", "docs")
const GITHUB = "https://github.com/rasmusjosefsson/agent-qa/blob/main"

// slug → { file, description }. Order is irrelevant; the sidebar lives in
// astro.config.mjs. Internal trackers (gap-map, qa-playground, specs/) stay
// repo-only.
const DOCS = {
  overview: { file: "index.md", description: "The whole system in one read: record, replay, compare, heal." },
  architecture: { file: "architecture.md", description: "Codemap and runtime flows of the agent-qa binary." },
  configuration: { file: "configuration.md", description: "agent-qa.toml, environment variables, and precedence." },
  plugins: { file: "plugins.md", description: "The plugin contract: JSON over stdio, any language." },
  verbs: { file: "verbs.md", description: "Complete CLI verb reference." },
  templates: { file: "templates.md", description: "Reusable sub-scenarios loaded by runTemplate." },
  flow: { file: "flow.md", description: "Author scenarios as one line per step — compile to scenario.json, describe back." },
  "visual-testing": { file: "visual-testing.md", description: "Golden screenshots: shot claims, baselines, re-mint loop." },
  network: { file: "network.md", description: "Network and console claims behind the UI." },
  "github-action": { file: "github-action.md", description: "Replay your scenarios on every PR." },
  extension: { file: "extension.md", description: "One-click capture → ingest → replay." },
  "lint-rules": { file: "lint-rules.md", description: "Scenario lint rule catalogue." },
  "process-hygiene": { file: "process-hygiene.md", description: "Track and reap agent-browser daemons, Chrome trees, and stray profiles." },
  releasing: { file: "releasing.md", description: "Cross-compile, package, and publish." },
}

const slugByFile = Object.fromEntries(Object.entries(DOCS).map(([slug, { file }]) => [file, slug]))

function rewriteLink(href) {
  if (/^([a-z]+:|#|\/)/i.test(href)) return href
  const [path, hash = ""] = href.split("#")
  const anchor = hash ? `#${hash}` : ""
  const normalized = posix.normalize(path)
  if (slugByFile[normalized]) return `../${slugByFile[normalized]}/${anchor}`
  // Resolve against docs/ and point at the file on GitHub.
  return `${GITHUB}/${posix.normalize(posix.join("docs", path))}${anchor}`
}

function transform(src, description) {
  let body = src.replace(/\r\n/g, "\n")
  const h1 = body.match(/^#\s+(.+)\n/m)
  const title = h1 ? h1[1].trim().replace(/`/g, "") : "Untitled"
  if (h1) body = body.replace(h1[0], "")

  // Designed diagram when one matches, a visual file tree for ├── blocks,
  // themed client-side mermaid for any other mermaid block.
  body = body.replace(/```(\w*)\n([\s\S]*?)```/g, (block, lang, src) => {
    const designed = DIAGRAMS.find((d) => src.includes(d.match))
    if (designed) return designed.render()
    if (isAsciiTree(src)) return fileTree(src)
    if (!lang && isArrowChain(src)) return stepStrip(src)
    if (lang !== "mermaid") return block
    const escaped = src.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    return `<pre class="mermaid not-content">${escaped}</pre>`
  })

  // Only rewrite links outside fenced code blocks.
  body = body
    .split(/(```[\s\S]*?```)/g)
    .map((chunk, i) => (i % 2 ? chunk : chunk.replace(/\]\(([^)\s]+)\)/g, (_, href) => `](${rewriteLink(href)})`)))
    .join("")

  const fm = `---\ntitle: ${JSON.stringify(title)}\ndescription: ${JSON.stringify(description)}\n---\n`
  return fm + body.replace(/^\n+/, "\n")
}

// Hand-written pages live alongside the generated ones; keep them.
const HAND_WRITTEN = new Set(["quickstart.mdx"])
await mkdir(out, { recursive: true })
for (const name of await readdir(out)) {
  if (!HAND_WRITTEN.has(name)) await rm(join(out, name), { recursive: true, force: true })
}
for (const [slug, { file, description }] of Object.entries(DOCS)) {
  const src = await readFile(join(repo, "docs", file), "utf8")
  await writeFile(join(out, `${slug}.md`), transform(src, description))
}
console.log(`sync-docs: ${Object.keys(DOCS).length} pages → src/content/docs/docs/`)

// Agent-facing .md mirrors — each doc served verbatim as <slug>.md next to
// its HTML page (docs/foo/ → docs/foo.md), so an agent can read markdown
// without parsing the site chrome. Mirrors keep the raw repo markdown —
// mermaid fences and all — plus a header naming canonical + source.
const SITE_ORIGIN = process.env.SITE_URL ?? "https://rasmusjosefsson.github.io"
const SITE_BASE = process.env.SITE_BASE ?? "/agent-qa"
const mirrorOut = join(here, "..", "public", "docs")
await mkdir(mirrorOut, { recursive: true })
for (const name of await readdir(mirrorOut)) await rm(join(mirrorOut, name), { force: true })
for (const [slug, { file }] of Object.entries(DOCS)) {
  const src = await readFile(join(repo, "docs", file), "utf8")
  const header =
    `<!-- markdown mirror of ${SITE_ORIGIN}${SITE_BASE}/docs/${slug}/ — source: docs/${file} in rasmusjosefsson/agent-qa -->\n\n`
  await writeFile(join(mirrorOut, `${slug}.md`), header + src.replace(/\r\n/g, "\n"))
}
// quickstart is hand-written mdx — mirror its content too (mdx is readable markdown).
const qs = await readFile(join(out, "quickstart.mdx"), "utf8")
await writeFile(
  join(mirrorOut, "quickstart.md"),
  `<!-- markdown mirror of ${SITE_ORIGIN}${SITE_BASE}/docs/quickstart/ — source: site quickstart.mdx -->\n\n` + qs.replace(/\r\n/g, "\n"),
)
console.log(`sync-docs: ${Object.keys(DOCS).length + 1} mirrors → public/docs/`)
