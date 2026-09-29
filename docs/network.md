# Network-aware scenarios

Replay captures the session's request traffic and lets steps assert on it —
the app isn't green because the UI rendered if the API call behind it never
fired (or 500'd). This page covers the merged surface; see `verbs.md` for
step anatomy.

## `{"network"}` claim subject

```json
{
  "id": "s4",
  "kind": "check",
  "intent": "the users fetch succeeded",
  "claim": {
    "subject": {
      "network": { "urlMatches": "/api/users", "method": "GET" },
      "ofKind": "status"
    },
    "predicate": "equals",
    "value": "200"
  }
}
```

Matcher fields AND together:

- `urlMatches` — a **regex** on the request URL (escape literal `?`/`&`/`.`
  as `\\?` etc.)
- `method` — HTTP verb, exact match
- `operationName` — substring match on the URL, GraphQL-style
  (`?operationName=ListUsers` or a `/ListUsers` path segment)

`ofKind` picks what the predicate evaluates:

| `ofKind` | Predicates | Semantics |
|---|---|---|
| `fired` (default) | `exists`, `notExists` (+ `isVisible`/`isHidden` aliases) | at least one matching request / none at all |
| `status` | `equals`/`contains`/`matches`/string preds on `"200"`; `gt`/`gte`/`lt`/`lte` numerically | the **latest** matching request's HTTP status |
| `responseJsonPath` | any predicate + `value` | fetches the latest match's response body (`network request <id>`) and evaluates `claim.path` as a JSON path into it |

All forms **poll until the step's timeout** — a request that lands after the
check starts still counts. On timeout the error carries the last observed
state.

```json
{ "subject": {
    "network": { "operationName": "CreateOrder", "method": "POST" },
    "ofKind": "responseJsonPath",
    "path": "order.id"
  },
  "predicate": "exists" }
```

## `do/wait` on a request

Rather than guessing `ms`, block until the page's resource-timing log
records a matching completed load — then assert the UI it fed:

```json
{ "id": "s3", "kind": "do", "verb": "wait",
  "params": { "url": "*/api/search*", "timeoutMs": 10000 },
  "intent": "wait for the results call" }
```

`url` is a glob (`*` wildcard; a plain substring also works). The check
reads `performance.getEntriesByType('resource')`, so it only sees
*completed* loads — an in-flight request does not satisfy it. `timeoutMs`
defaults to 10s; the step fails when nothing matching lands in time. This
is the reliable primitive for SPA transitions: click the filter → wait the
XHR → check the table.

## `{"console"}` claim subject

A page can render fine while its JS crashes. Assert on console output:

```json
{ "claim": {
    "subject": { "console": { "type": "error" } },
    "predicate": "notExists" } }
```

`console` accepts `true` (all messages) or `{ "type": "error", "text":
"<substring>" }` (AND-ed). `exists`/`notExists` check presence; numeric
predicates compare the message count; text predicates pass when **any**
matching message satisfies them.

## `{"a11y"}` claim subject

Gate a step on an axe-core accessibility audit (`agent-browser a11y`):

```json
{ "claim": {
    "subject": { "a11y": { "impact": "serious" } },
    "predicate": "notExists" } }
```

`a11y` accepts `true` (every violation counts) or a matcher:
`{"impact": "minor"|"moderate"|"serious"|"critical"}` is a floor (keeps
that level and everything worse), `"rule": "<axe rule id>"` keeps only that
rule's findings, `"within": "<css>"` scopes the audit to a subtree, and
`"incomplete": true` also counts axe's `incomplete` results (rules needing
manual review). `exists`/`notExists` check presence; numeric predicates
compare the matching count. A failure lists the offending rule ids.

## `crawl` uses both by default

`crawl` drafts each route as goto + `{"shot"}` + a `no console errors` check
(`--no-console-checks` opts out) — see `visual-testing.md`.

## `do/state` — seed page state mid-scenario

`env.open` seeds cookies/storage at session bootstrap; `do/state` does it
*inside* the step list — flip a feature flag between pages, expire a
session mid-flow, pre-seed a cart before checkout:

```json
{ "id": "s2", "kind": "do", "verb": "state",
  "params": {
    "localStorage": { "token": "eyJ…", "cart": "{ \"items\": 3 }" },
    "sessionStorage": { "returnUrl": "/orders" },
    "cookies": [
      { "name": "session", "value": "abc", "path": "/", "sameSite": "Lax" }
    ],
    "clearCookies": true
  },
  "intent": "seed an authenticated cart state" }
```

Keys: `localStorage`/`sessionStorage` (objects of key→value), `cookies`
(array of `{"name","value","path"?,"domain"?,"maxAge"?,"secure"?,"sameSite"?}`),
and the `clearCookies`/`clearLocalStorage`/`clearSessionStorage` clears.
Values run through scenario-var substitution. Cookies are set via
`document.cookie` — `httpOnly` entries can't be seeded (that's what the
auth plugins are for). Order a `goto`/`reload` after it so the app reads
the fresh state.

## `{"storage"}` / `{"cookie"}` claim subjects

Assert the state an app actually wrote — token minted, preference
persisted, logout cleared the session cookie:

```json
{ "claim": { "subject": { "storage": "session" }, "predicate": "exists" } }
{ "claim": {
    "subject": { "storage": { "key": "cart", "scope": "session" }, "path": "$.total" },
    "predicate": "gte", "value": 1 } }
{ "claim": { "subject": { "cookie": "session" }, "predicate": "notExists" } }
```

`storage` takes `"key"` (localStorage) or `{ "key": "…", "scope":
"local"|"session" }`; `path` walks a JSON-encoded value. `cookie` reads
`document.cookie` — `httpOnly` cookies are invisible there.

## `do/state` — `indexeddb` seeding

`params.indexeddb` seeds IndexedDB stores — each entry is
`{"db","store","keyPath"?,"clear"?,"put":[…]}`. With `keyPath` the `put`
entries are full records; without it they are `{"key","value"}` pairs
stored under out-of-line keys. `clear: true` empties the store first.
Missing stores are created (a version bump on the open db):

```json
{ "id": "s2", "kind": "do", "verb": "state",
  "params": { "indexeddb": [
    { "db": "cart", "store": "items", "keyPath": "sku",
      "put": [ { "sku": "sku-1", "qty": 2 } ] },
    { "db": "flags", "store": "out",
      "put": [ { "key": "onboarded", "value": { "done": true } } ] }
  ] } }
```

## `{"indexeddb"}` claim subject

Assert an IndexedDB record — `{"db","store","key"?}`. Without `key` the
subject is the object store itself (exists/notExists); with it, record
predicates apply and `path` walks a JSON-structured value:

```json
{ "claim": {
    "subject": { "indexeddb": { "db": "cart", "store": "items", "key": "sku-1" },
                 "path": "$.qty" },
    "predicate": "gte", "value": 1 } }
{ "claim": { "subject": { "indexeddb": { "db": "flags", "store": "out" } },
             "predicate": "exists" } }
```

The probe lists `indexedDB.databases()` before opening so `notExists`
checks never create the db they're probing.
