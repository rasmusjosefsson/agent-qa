# Tooling scan 3: text-first E2E authoring (Oct 2026)

Third pass. The question this round: is there "an easy way to author an E2E
with pure text" worth borrowing? Surveyed the plain-English/declarative
authoring lineage plus the agentic engines that now frame themselves as
text-in/text-out. Products named here are external references for design
context only; nothing vendor-specific enters core code.

The headline finding: "author in pure text" splits cleanly into **compile-time
text** (prose → deterministic steps, model only at authoring time — our exact
model, and where the whole market is converging) and **replay-time text**
(a model interprets prose on every run — a model in the gate, our documented
non-goal). Everything worth adopting lives on the compile-time side.

## What exists

**Plain-text authoring that compiles to deterministic tests**

- *Cypress `cy.prompt()`* (experimental) — natural-language step strings
  inside a spec: the runner interprets each step, generates real Cypress
  commands, executes them, and **shows/exports the generated code** — the
  recommended path is export-then-commit so CI runs plain deterministic code.
  `placeholders` redact dynamic inputs from cache identity (our `{{vars.*}}`,
  parity). Also accepts Gherkin syntax *without* step-definition files —
  proof that prose-line formats still pull demand even inside code-first
  frameworks.
- *Playwright Test Agents* — three chained agents: **planner** explores the
  app and writes a *Markdown test plan*; **generator** turns the plan into
  executable specs, verifying selectors and assertions live as it goes;
  **healer** re-runs failures and patches locators/waits until green. The
  notable artifact isn't the agents — it's the **Markdown plan as an
  intermediate, reviewable source of truth** between exploration and code.
- *testRigor* — the original plain-English platform, and still the clearest
  statement of the idea. Two tiers: a **parsed-English command grammar**
  (`click "Submit"`, `enter "Peter" into "First Name"`, `check that page
  contains "Welcome"` — enormous but bounded, deterministic) plus a
  free-form LLM fallback for arbitrary phrasing ("add to cart"). Reusable
  named rules/macros: write `login` once, call it as a step everywhere —
  prose-defined subroutines.
- *Gauge* (ThoughtWorks → community) — the purest executable-text lineage:
  scenarios are **Markdown files** (`.spec`/`.md`, heading + bullet steps
  with `"quoted"`/`<params>`), concepts = named reusable step groups, tags,
  data tables. Sponsorship ended 2021; still maintained by community, still
  releasing. Its lesson is the failure mode too: prose at the *front* still
  needed a code hook behind every step — a two-engine tax we avoid.
- *Maestro* — mobile/web E2E in **near-English YAML flows** (`- tapOn:
  "Login"`, `- assertVisible: "Welcome"`, `- inputText: ${VAR}`). What makes
  it relevant beyond the syntax: `runFlow` **subflow composition**, `when:
  visible:` **conditional step blocks** (deterministic, authored — no AI),
  `label:` free-text per step for readable reports, env vars, tags, and a
  marketing line that matches ours verbatim: "generate deterministic E2E
  tests — human-readable, not black boxes." Ships an MCP server and a
  tap-tap-tap recorder IDE that emits YAML.
- *Karate* — Gherkin-flavored DSL (`Given url`, `When method get`,
  `Then status 200`) best known for API tests, but its UI driver covers
  browser automation with the same English syntax. Proof the prose-step
  surface works at the protocol layer too.
- *Robot Framework* — twenty years of keyword-driven "business language"
  testing: tabular English syntax, a huge keyword library ecosystem, data-
  driven templates (one test body, N data rows). The oldest proof that
  constrained vocabularies — not free English — are what survives.

**Agentic engines (text in, runs out)**

- *Skyvern* — open-source vision-LLM swarm: NL instruction → plan →
  computer-vision clicks; handles captcha/2FA; sells itself on surviving
  unseen sites. But its own SDK marketing now says "blends LLM reasoning
  with executable code — generates deterministic, repeatable workflows" —
  even the fully-agentic players land on generate-then-freeze.
- *Virtuoso / Testsigma / KaneAI (LambdaTest) / Autify* — the SaaS NLP-
  authoring cluster: describe the test (or feed a requirements doc), the
  platform generates and self-heals it, hosted runs + dashboards. Testsigma
  has an open-source core. Pattern consistent across all: authoring UX is
  the differentiator, the executed artifact is still a step list.
- *Browser Use & the OSS agent-loop family* — per-run LLM browser agents;
  nondeterministic by construction, no committed artifact. Covering the
  bottom of the market we explicitly don't occupy.

## Where agent-qa already stands

| Pattern | Status |
| --- | --- |
| Author once, replay deterministically | core model — cy.prompt export & Skyvern SDK both converge on it |
| Dynamic inputs in prose | `{{vars.*}}` = cy.prompt `placeholders`, Maestro `${ENV}`, parity |
| Deterministic overlay dismissal | `dismiss` verb + `params.dismiss` list — Maestro `when:`'s narrower half |
| Explore → route coverage | `discover` (planner's deterministic slice) |
| Codeless capture → editable artifact | extension record → scenario JSON (Maestro's "tap-tap-tap → YAML", owned) |
| Agent-facing authoring | skills + `agent-qa mcp` — agents already author scenarios in prose terms |
| Healing | `auto_heal` (in-run) + `heal-chronic` (source-level, pull-model) |
| Human-readable step listing | report + run logs show `do`/`text` lines — not a first-class text *file* |

## Gaps worth closing (ranked)

1. **Plain-text scenario format that *compiles* to scenario JSON** — the
   direct answer to "author an E2E with pure text," and Gauge/Maestro's
   proven shape: a constrained line DSL, no model at replay:

   ```text
   open https://todo.app
   click "Add"
   fill "Title" with "Buy milk"
   check "Buy milk" exists
   shot home
   ```

   `agent-qa compile flow.txt` → scenario JSON; `agent-qa describe
   scenario.json` → the text back (round-trip, like our translate). The
   text file *is* the authored artifact — diffable, hand-editable, agent-
   writable, and it removes JSON as the barrier for the "product owner
   writes a test" persona every surveyed tool chases. Bounded grammar =
   deterministic parse, zero model in the gate. This is the scan's main
   recommendation.
2. **Step-group includes** (Maestro `runFlow` / Gauge concepts / testRigor
   rules) — `{"run": "subflows/login"}` or an `include:` step. Authored
   composition: `login` written once, called from every scenario; failures
   point at the caller's step. No equivalent exists in the Verb enum today.
3. **Conditional step blocks** (Maestro `when:`) — `{"when": {"present":
   "<locator>"}, steps: [...]}`: run the block only if the condition holds.
   Deterministic branching for guards ("if logged out, sign in first"),
   optional popups, region/state-dependent flows — the general mechanism
   `dismiss` is a special case of.
4. **Markdown test-plan artifact** (Playwright planner) — `discover`
   already emits a route map; extend it (or an `agent-qa plan`) to a
   reviewable `plan.md` — per-route *intent* (what to verify) that an agent
   or `translate` materializes into scenarios. The plan is the human-
   readable layer between exploration and code, and it's exactly what a
   "describe your coverage in text" request should produce.
5. **Data-row replay** (testRigor datasets, Robot templates) — a `rows:`/
   `datafile:` (CSV/JSONL) binding replayed once per row over `{{vars.*}}`.
   Cheap on top of existing substitution; unlocks the classic "same flow,
   N inputs" case without duplicating scenarios.

## Explicit non-adoptions

- **Free-form NL steps at replay time** (testRigor tier-2, cy.prompt
  in-place, Skyvern per-run) — a model in the verdict path; same non-goal
  as agentic goal-driven replay. Constrained grammar only.
- **Gherkin/Cucumber proper** — a step-definition registry is a second
  execution engine behind the prose; our verbs already *are* the keyword
  vocabulary, and cy.prompt itself dropped step definitions. Borrow the
  surface, skip the machinery.
- **Model-driven healer agent** (Playwright healer) — the deferred half of
  landscape item 10 stands; our in-run `auto_heal` + pull-model
  `heal-chronic` already cover healing without a model in the loop.
- **Mobile/desktop drivers** (Maestro's actual market) — still out of scope
  for a browser-layer tool; their flow format is what we take, not their
  runtime.
- **Hosted authoring dashboards** (Virtuoso/Testsigma/Autify) — SaaS
  polish, not a capability gap.
