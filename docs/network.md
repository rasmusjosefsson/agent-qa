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

## `crawl` uses both by default

`crawl` drafts each route as goto + `{"shot"}` + a `no console errors` check
(`--no-console-checks` opts out) — see `visual-testing.md`.
