//! `scenario/2` contract types — Rust port of `cli/src/scenario.ts`.
//!
//! These structs round-trip through `serde_json` byte-for-byte against any
//! valid `scenario.json`. They are the *typed* view; the *authoritative*
//! shape lives in `schema/scenario-schema.json`, validated at load time
//! (see [`crate::schema`]).
//!
//! Conventions (mirrors the TS source):
//!   - tagged unions on `kind` for [`Step`], [`EnvOp`]
//!   - tagged union on `from` for [`Value`]
//!   - untagged union for [`Locator`] / [`ClaimSubject`] (TS used
//!     "shape-distinguished" unions there)
//!   - every field is optional unless the schema marks it required
//!   - extra fields are rejected at the schema layer; serde ignores them
//!     here so we don't double-reject
//!
//! Types are present but not yet consumed by a runner. Allowing
//! dead_code at module scope keeps the warning surface clean while later
//! slices wire each variant into replay / record / heal.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::BTreeMap;

pub const SCENARIO_SCHEMA_ID: &str = "scenario/2";

// ---------- supporting unions ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Producer {
    AutomatedCapture,
    LlmAuthor,
    AgentRecorder,
    Human,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnFailure {
    Abort,
    Continue,
    Ignore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    String,
    Number,
    Boolean,
    Array,
    Object,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MintScope {
    Scenario,
    Loop,
    Template,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NameMatchMode {
    Exact,
    Contains,
    Regex,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RawLocatorKind {
    Css,
    Xpath,
    TestId,
    /// Visible text content. Maps to `agent-browser find text <v> <act>`
    /// at replay time. Use when role+name lookup is unreliable (e.g.
    /// Radix portals, virtualised lists) but a unique visible label
    /// exists. The recorder emits this kind for `clickByText`.
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Predicate {
    IsVisible,
    IsHidden,
    IsEnabled,
    IsDisabled,
    IsChecked,
    IsUnchecked,
    Exists,
    NotExists,
    Equals,
    Contains,
    Matches,
    StartsWith,
    EndsWith,
    Gt,
    Gte,
    Lt,
    Lte,
    CountEquals,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Verb {
    Goto,
    Reload,
    Back,
    Forward,
    Click,
    Type,
    Clear,
    Press,
    Hover,
    Select,
    Check,
    Uncheck,
    Upload,
    ScrollTo,
    Focus,
    Blur,
    Read,
    CallGql,
    Wait,
    /// Resolve a pending native dialog (alert/confirm/prompt):
    /// `params.action` = "accept" | "dismiss", `params.text` = prompt input.
    Dialog,
    /// Click an element that triggers a file download and save it.
    /// `on` = the triggering element (css/testId), `value` = destination path
    /// (absolute, or relative resolved against the scenario dir).
    Download,
    /// Remove elements that block interaction — consent walls, overlays,
    /// sticky banners. `on` = the blocking element (css/testId/xpath).
    /// Beyond removing matching nodes at dispatch, the selector is kept on
    /// a per-run dismissal list re-applied before every interactive step,
    /// so an overlay that mounts later (delayed CMP dialogs) still can't
    /// intercept the hit-test. Absent matches are a no-op, not a failure.
    Dismiss,
    /// Double-click an element (css/testId locator via `on`).
    #[serde(rename = "dblclick")]
    DblClick,
    /// Secondary-button click — opens context menus: dispatches the
    /// pointer/mouse chain with `button: 2` then `contextmenu`. Any locator
    /// kind via `on` (resolved in-page like `drag` endpoints).
    #[serde(rename = "rightclick")]
    RightClick,
    /// Browser tab management: `value` is the `tab` subcommand tail —
    /// `new <url>`, `list`, `close <ref>`, or `<ref>` to switch focus.
    /// Claims afterwards evaluate against the now-active tab.
    Tab,
    /// Resize the viewport: `params` = `{ "width": <px>, "height": <px> }`.
    Viewport,
    /// Arm native file-chooser interception and inject files: `params` =
    /// `{ "files": [<path>, ...] }` (paths resolve like upload's). The next
    /// click that would open the OS picker resolves with these files instead.
    /// An empty `files` array simulates cancelling the picker.
    FileChooser,
    /// Drag `on` onto `params.to` — both locators. Dispatches the HTML5
    /// `dragstart → dragenter/over → drop → dragend` chain with a real
    /// `DataTransfer` plus the pointer/mouse sequence pointer-based
    /// drag libraries listen for.
    Drag,
    /// Stub network calls: `params` = `{ "url": "<glob>", "status": <n>,
    /// "json": {...} | "body": "…", "delayMs": <n> }`. Wraps fetch + XHR in
    /// the live page; registered rules re-apply automatically after
    /// goto/reload/back/forward (navigation wipes the JS world).
    Mock,
    /// Remove mock rules: `params.url` (optional) drops that rule, absent
    /// clears all.
    Unmock,
    /// Seed page state without a UI round-trip: `params` may carry
    /// `localStorage`/`sessionStorage` objects (key→value), a `cookies`
    /// array (`{"name","value","path"?,"domain"?,"maxAge"?}` — set via
    /// `document.cookie`, so httpOnly entries can't be seeded), and the
    /// `clearCookies`/`clearLocalStorage`/`clearSessionStorage` boolean
    /// clears. Values go through scenario-var substitution. Apply before
    /// `goto` (or before `reload`) for the app to observe the state.
    State,
    /// Press-and-hold an element: dispatches pointerdown/mousedown (and
    /// touchstart where the constructor exists) at the element's center,
    /// sleeps `params.ms` (default 500), then releases with
    /// pointerup/mouseup/touchend — deliberately no click. For long-press
    /// menus and press-to-confirm buttons that arm on down events.
    Hold,
    /// Swipe gesture: `params.direction` (up/down/left/right — the
    /// direction the finger travels), `params.distance` px (default 300).
    /// `on` picks the origin element; absent `on` swipes from the viewport
    /// center. Dispatches the touch event chain plus pointer/mouse events
    /// so both touch- and pointer-driven handlers fire.
    Swipe,
    /// Two-finger pinch: `params.direction` = "in" (fingers together →
    /// zoom out) or "out" (fingers apart → zoom in); `params.distance` px
    /// (default 150) is each finger's travel. `on` centers the gesture on
    /// an element; absent `on` centers the viewport. Dispatches a
    /// two-touch TouchEvent chain plus a ctrlKey wheel event so both
    /// native pinch handlers and desktop trackpad-zoom conventions fire.
    Pinch,
    /// Two-finger rotate: `params.degrees` (required, signed — positive is
    /// clockwise) and `params.radius` px (optional, default 120 — each
    /// finger's orbit radius). `on` centers the gesture on an element;
    /// absent `on` centers the viewport. Dispatches a two-touch TouchEvent
    /// chain; browsers without Touch/TouchEvent fail with `no-touch`
    /// (there is no desktop fallback convention for rotate).
    Rotate,
    /// Switch the session's frame context: `params.selector` = a CSS
    /// selector for the iframe to enter, or `params.main = true` to return
    /// to the top document. Locators on later steps resolve inside the
    /// selected frame.
    Frame,
    /// Browser emulation — maps onto `agent-browser set …`. `params` may
    /// carry any of: `device` (preset name, e.g. "iPhone 12"), `geo`
    /// `{lat,lng}` (also grants the geolocation permission — the override
    /// alone leaves `navigator.geolocation` hanging), `offline` (bool),
    /// `colorScheme` ("dark"|"light"), `reducedMotion` (bool),
    /// `permissions` (array of CDP permission names, e.g.
    /// `["geolocation","clipboardRead"]`), `headers` ({name:value}),
    /// `credentials` `{user,pass}` (HTTP auth). Applied in a fixed order
    /// (device first, since it resets viewport+UA). String values go
    /// through scenario-var substitution.
    Emulate,
    #[serde(rename = "loop")]
    Loop,
    Group,
    UseTemplate,
}

// ---------- Locator ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NameMatch {
    Plain(String),
    Pattern {
        pattern: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        r#match: Option<NameMatchMode>,
    },
    I18n {
        #[serde(rename = "i18nKey")]
        i18n_key: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocatorTolerance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digits: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_suffix: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_augmented_label: Option<bool>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocatorRole {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<NameMatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Vec<Locator>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerate: Option<LocatorTolerance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocatorRawSpec {
    pub kind: RawLocatorKind,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocatorRaw {
    pub raw: LocatorRawSpec,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Locator {
    Role(LocatorRole),
    Raw(LocatorRaw),
}

// `on` also accepts a one-string shorthand — "css:sel", "xpath:expr",
// "testId:key", "text:txt" — which lowers to the named raw locator with a
// canned reason. Hand-authored steps (record-step, scenario insert, PR
// review) shouldn't have to spell the full {"raw":{kind,value},reason}
// shape for a plain selector.
impl<'de> Deserialize<'de> for Locator {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = Json::deserialize(de)?;
        if let Some(short) = v.as_str() {
            let (prefix, value) = short.split_once(':').ok_or_else(|| {
                serde::de::Error::custom(
                    "locator shorthand needs a kind prefix — \
                         \"css:sel\", \"xpath:expr\", \"testId:key\", or \"text:txt\"",
                )
            })?;
            let kind = match prefix {
                "css" => RawLocatorKind::Css,
                "xpath" => RawLocatorKind::Xpath,
                "testId" => RawLocatorKind::TestId,
                "text" => RawLocatorKind::Text,
                other => {
                    return Err(serde::de::Error::custom(format!(
                        "unknown locator prefix \"{other}:\" — \
                         use css:, xpath:, testId:, or text:"
                    )))
                }
            };
            if value.is_empty() {
                return Err(serde::de::Error::custom(
                    "locator shorthand has an empty selector",
                ));
            }
            return Ok(Locator::Raw(LocatorRaw {
                raw: LocatorRawSpec {
                    kind,
                    value: value.to_string(),
                },
                reason: "locator shorthand".to_string(),
            }));
        }
        if let Ok(role) = serde_json::from_value::<LocatorRole>(v.clone()) {
            return Ok(Locator::Role(role));
        }
        serde_json::from_value::<LocatorRaw>(v)
            .map(Locator::Raw)
            .map_err(|e| serde::de::Error::custom(format!("invalid locator: {e}")))
    }
}

// ---------- Value ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MintSpec {
    pub template: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MintScope>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "lowercase")]
pub enum Value {
    Literal {
        literal: Json,
    },
    Input {
        input: String,
    },
    #[serde(rename_all = "camelCase")]
    Step {
        step_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    Mint {
        mint: MintSpec,
    },
    #[serde(rename_all = "camelCase")]
    Loop {
        loop_var: String,
    },
}

// ---------- Claim ----------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkMatcher {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_matches: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<HttpMethod>,
    /// Substring match on the request's POST body (fetched via
    /// `network request <id>` per candidate — url/method narrow first).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_data_contains: Option<String>,
    /// Substring match on any WebSocket frame payload — narrows to
    /// `cdpws-*` socket entries (only they carry frames).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ws_payload_contains: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ElementClaimKind {
    Text,
    Value,
    Count,
    Attribute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NetworkClaimKind {
    Status,
    ResponseJsonPath,
    Fired,
}

/// `{"console": true}` matches every message; `{"type": "error"}` narrows
/// to one console level (verbatim — "error", "warn", "log", ...);
/// `{"text": "<substring>"}` prefilters message text.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleMatcher {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// `console` accepts either `true` (all messages) or a matcher object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConsoleSubject {
    Flag(bool),
    Matcher(ConsoleMatcher),
}

/// `{"pageError": true}` matches every uncaught exception; a matcher narrows
/// on the rendered text (error message + stack).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageErrorMatcher {
    /// Error text contains this substring.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The exception was raised on a document URL containing this substring.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// `pageError` accepts either `true` (all page errors) or a matcher object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PageErrorSubject {
    Flag(bool),
    Matcher(PageErrorMatcher),
}

/// `{"a11y": true}` audits the whole page; the matcher narrows which axe
/// findings count:
///   `{"impact": "serious"}`       violations at that impact or worse
///                                 (minor < moderate < serious < critical)
///   `{"rule": "color-contrast"}`  only findings of that axe rule id
///   `{"within": "#app"}`          scope the audit to a subtree
///   `{"incomplete": true}`        also count axe's `incomplete` results
///                                 (rules needing manual review)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct A11yMatcher {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(default)]
    pub incomplete: bool,
}

/// `a11y` accepts either `true` (all violations) or a matcher object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum A11ySubject {
    Flag(bool),
    Matcher(A11yMatcher),
}

/// `storage` accepts `"key"` (localStorage) or a matcher object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StorageSubject {
    Key(String),
    Matcher(StorageMatcher),
}

/// `{"key": "k", "scope": "local"|"session"}` — scope defaults to "local".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageMatcher {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// `{"db": "d", "store": "s", "key": "k"}` — `key` is optional; without
/// it the subject is the object store itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedDbMatcher {
    pub db: String,
    pub store: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClaimSubject {
    #[serde(rename_all = "camelCase")]
    Element {
        element: Locator,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attribute: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        of_kind: Option<ElementClaimKind>,
    },
    Url {
        url: bool,
    },
    #[serde(rename_all = "camelCase")]
    Network {
        network: NetworkMatcher,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        of_kind: Option<NetworkClaimKind>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    Data {
        data: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    Flag {
        flag: String,
    },
    /// `{"dialog": true}` — assert on a pending native dialog
    /// (alert/confirm/prompt): `exists`/`notExists` for presence,
    /// text predicates match the dialog's message.
    Dialog {
        dialog: bool,
    },
    /// `{"file": "<name-or-path>"}` — assert on a file in the scenario's
    /// download output (relative paths resolve against the scenario dir).
    /// `exists`/`notExists` check presence; `gt`/`gte`/`lt`/`lte` compare the
    /// file size in bytes; string predicates match the file name by default,
    /// or the file's UTF-8 text when `attribute` is `"content"`.
    File {
        file: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attribute: Option<String>,
    },
    /// `{"shot": "<stepId>"}` — compare this run's screenshot for the named
    /// do-step against the checked-in baseline at `<scenario>/baselines/
    /// <stepId>.png`. Only predicate is `matches` — pass when the fraction of
    /// differing pixels ≤ `tolerance.pixels` (default 0.01). On mismatch the
    /// delta map lands at `<run>/shots-diff/<stepId>.diff.png` and the claim
    /// fails with the diff ratio. Baselines are minted with `shot-accept`.
    ///
    /// `clip` (optional) restricts the diff to a single element's box —
    /// `{"shot": "s3", "clip": {"raw": {"kind": "css", "value": "#card"}, "reason": ".."}}`. The
    /// element's rect is read live at claim time and applied to BOTH images,
    /// so keep the viewport pinned; baselines stay full-page.
    ///
    /// `mask` lists CSS selectors hidden (visibility:hidden) while any step
    /// screenshot is captured — the ignore-regions feature for volatile UI
    /// (timestamps, live badges, user avatars). Masks from every shot claim
    /// in the scenario apply to every screenshot, so baselines and replays
    /// stay consistent.
    Shot {
        shot: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        clip: Option<Locator>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        mask: Vec<String>,
    },
    /// `{"storage": "key"}` or `{"storage": {"key": "k", "scope":
    /// "local"|"session"}}` — assert on a web-storage entry. `exists`/
    /// `notExists` check key presence; string predicates compare the
    /// stored value; `path` walks into a JSON-encoded value.
    Storage {
        storage: StorageSubject,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    /// `{"cookie": "<name>"}` — assert on a cookie visible to the page
    /// (`document.cookie`; httpOnly cookies never appear here). `exists`/
    /// `notExists` for presence; string predicates compare the value.
    Cookie {
        cookie: String,
    },
    /// `{"indexeddb": {"db": "d", "store": "s", "key": "k"}}` — assert on
    /// an IndexedDB record. `exists`/`notExists` check record presence (the
    /// store's presence when `key` is omitted); string predicates compare
    /// the stored value; `path` walks a JSON-structured value. The db is
    /// probed via `indexedDB.databases()` first so a missing db is reported
    /// as absent rather than created by the check.
    IndexedDb {
        indexeddb: IndexedDbMatcher,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    /// `{"console": true}` or `{"console": {"type": "error"}}` — assert on
    /// messages the page logged this session. `exists`/`notExists` on
    /// presence of a matching message; numeric predicates
    /// (`countEquals`/`gt`/`gte`/`lt`/`lte`) compare the count; text
    /// predicates pass when ANY matching message's text satisfies them.
    Console {
        console: ConsoleSubject,
    },
    /// `{"pageError": true}` or `{"pageError": {"text": "<substring>"}}` —
    /// assert on uncaught exceptions the page raised this session
    /// (`agent-browser errors`; a different channel from `console`). Same
    /// predicate set as `console`. `notExists` is the "page threw nothing"
    /// gate.
    #[serde(rename_all = "camelCase")]
    PageError {
        page_error: PageErrorSubject,
    },
    /// `{"timing": "<stepId>"}` — assert on a step's recorded duration in
    /// this run (`<run>/events.jsonl`). Numeric predicates
    /// (`gt`/`gte`/`lt`/`lte`/`equals`) compare milliseconds against
    /// `value`; `exists`/`notExists` test whether the step has a timing
    /// row at all. Only meaningful inside a replay (there is no run dir
    /// under run-step).
    Timing {
        timing: String,
    },
    /// `{"a11y": true}` or `{"a11y": {"impact": "serious"}}` — run an
    /// axe-core accessibility audit (`agent-browser a11y`) and count the
    /// matching violations. `exists`/`notExists` on ≥1/zero violations;
    /// numeric predicates (`countEquals`/`gt`/`gte`/`lt`/`lte`) compare the
    /// count. The matcher filters findings: `impact` floor, `rule` id,
    /// `within` CSS scope, `incomplete` to include axe's incomplete results.
    A11y {
        a11y: A11ySubject,
    },
    Var {
        kind: String, // always "var" — kept literal to disambiguate untagged
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct Claim {
    pub subject: ClaimSubject,
    pub predicate: Predicate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Json>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<BTreeMap<String, Json>>,
}

/// Predicates that take an argument (the claim's `value` field). The rest
/// are unary — an object-form predicate for one of them is an error.
const PREDICATES_WITH_ARG: &[&str] = &[
    "equals",
    "contains",
    "matches",
    "startsWith",
    "endsWith",
    "gt",
    "gte",
    "lt",
    "lte",
    "countEquals",
];

impl<'de> Deserialize<'de> for Claim {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Raw {
            subject: ClaimSubject,
            predicate: Json,
            #[serde(default)]
            value: Option<Json>,
            #[serde(default)]
            tolerance: Option<BTreeMap<String, Json>>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let (predicate, inline_value) =
            match &raw.predicate {
                Json::String(name) => (
                    serde_json::from_value::<Predicate>(raw.predicate.clone())
                        .map_err(|_| D::Error::custom(format!("unknown predicate {name:?}")))?,
                    None,
                ),
                // Sugar: {"predicate": {"contains": "x"}} lowers to
                // predicate:"contains" + value:"x" — the arg moves off the
                // predicate key onto the claim's `value` field.
                Json::Object(map) if map.len() == 1 => {
                    let (name, arg) = map.iter().next().unwrap();
                    if !PREDICATES_WITH_ARG.contains(&name.as_str()) {
                        return Err(D::Error::custom(format!(
                            "predicate {name:?} takes no argument — write it as a string"
                        )));
                    }
                    (
                        serde_json::from_value::<Predicate>(Json::String(name.clone()))
                            .map_err(|_| D::Error::custom(format!("unknown predicate {name:?}")))?,
                        Some(arg.clone()),
                    )
                }
                _ => return Err(D::Error::custom(
                    "predicate must be a name string or a single-key object {\"contains\": <arg>}",
                )),
            };
        let value = match (raw.value, inline_value) {
            (Some(_), Some(_)) => {
                return Err(D::Error::custom(
                    "claim carries the argument twice: predicate object AND value",
                ))
            }
            (Some(v), None) | (None, Some(v)) => Some(v),
            (None, None) => None,
        };
        Ok(Claim {
            subject: raw.subject,
            predicate,
            value,
            tolerance: raw.tolerance,
        })
    }
}

// ---------- Step ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabMatcher {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opens_from_step_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<OnFailure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_as: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_for: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_for: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect_fail_for: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabMatcher>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Step {
    #[serde(rename_all = "camelCase")]
    Do {
        id: String,
        intent: String,
        verb: Verb,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on: Option<Locator>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        save_as: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        params: Option<BTreeMap<String, Json>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<StepContext>,
    },
    #[serde(rename_all = "camelCase")]
    Check {
        id: String,
        intent: String,
        claim: Claim,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<StepContext>,
    },
}

impl Step {
    pub fn id(&self) -> &str {
        match self {
            Step::Do { id, .. } | Step::Check { id, .. } => id,
        }
    }
    pub fn intent(&self) -> &str {
        match self {
            Step::Do { intent, .. } | Step::Check { intent, .. } => intent,
        }
    }

    pub(crate) fn set_id(&mut self, id: String) {
        match self {
            Step::Do { id: current, .. } | Step::Check { id: current, .. } => *current = id,
        }
    }
}

// ---------- Env ----------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvOpPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always_run: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_abort: Option<OnAbort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<OnFailureContinue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnAbort {
    Run,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnFailureContinue {
    Abort,
    Continue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EnvOp {
    /// Reset browser state for the active session: clear cookies +
    /// localStorage + sessionStorage. Default `env.open[0]` for
    /// anonymous recordings — makes replay reproduce the same blank
    /// starting slate the recording ran against.
    Fresh {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<EnvOpPolicy>,
    },
    /// Re-bootstrap a named profile. Used as `env.open[0]` for
    /// recordings made under `--profile <name>` — replay re-runs the
    /// same bootstrap so the session reaches the same authenticated
    /// baseline regardless of cookies left over from earlier runs.
    UseProfile {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<EnvOpPolicy>,
    },
    Nav {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<EnvOpPolicy>,
    },
    Cookie {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        domain: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<EnvOpPolicy>,
    },
    LocalStorage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<EnvOpPolicy>,
    },
    Gql {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,
        url: String,
        query: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        variables: Option<BTreeMap<String, Json>>,
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "forEach")]
        for_each: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "saveAs")]
        save_as: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<EnvOpPolicy>,
    },
    Flag {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,
        name: String,
        enabled: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<EnvOpPolicy>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Env {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<Vec<EnvOp>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close: Option<Vec<EnvOp>>,
}

// ---------- Inputs / Provenance / Template ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDecl {
    #[serde(rename = "type")]
    pub ty: InputType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Json>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitive: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<BTreeMap<String, Json>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<BTreeMap<String, Json>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub producer: Producer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub produced_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<BTreeMap<String, InputDecl>>,
    pub steps: Vec<Step>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub templates: Option<BTreeMap<String, Template>>,
}

// ---------- Scenario root ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    pub schema: String, // must equal "scenario/2"; enforced by JSON Schema
    pub id: String,
    pub intent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<BTreeMap<String, InputDecl>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<Env>,
    pub steps: Vec<Step>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub templates: Option<BTreeMap<String, Template>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub produced_by: Option<Provenance>,
}

/// `replay --base-url <origin>` retargeting: rewrite every absolute URL
/// rooted at the scenario's recorded origin onto `to_origin`, so a scenario
/// recorded on prod/staging replays against a preview deploy without editing
/// the file. Covers `env.open`/`env.close` `nav` urls and `do/goto` literal
/// values — the URLs replay actually navigates to. Claim-side URL patterns
/// (e.g. a `urlMatches` glob) are left alone: a host embedded there is
/// deliberate specificity, and inputs/templates are the escape hatch for
/// scenario-authored variability.
///
/// The recorded origin is the first `nav` op's origin, falling back to the
/// first `goto` literal. Returns that origin, or None when the scenario has
/// no absolute URL to retarget from.
pub fn retarget_origin(scenario: &mut Scenario, to_origin: &str) -> Option<String> {
    let from = recorded_origin(scenario)?;
    for ops in scenario
        .env
        .iter_mut()
        .flat_map(|e| e.open.iter_mut().chain(e.close.iter_mut()))
        .flatten()
    {
        if let EnvOp::Nav { url: Some(u), .. } = ops {
            rewrite_url_at_origin(u, &from, to_origin);
        }
    }
    for step in scenario.steps.iter_mut() {
        if let Step::Do {
            verb: Verb::Goto,
            value: Some(Value::Literal { literal }),
            ..
        } = step
        {
            if let Some(u) = literal.as_str() {
                if let Some(new) = rewritten_at_origin(u, &from, to_origin) {
                    *literal = Json::String(new);
                }
            }
        }
    }
    Some(from)
}

/// `url` rewritten onto `to` when it is exactly `from` + path: a bare
/// `starts_with` would also match `prod.example.com:8443` (a different
/// origin) and `prod.example.com.evil.io` (a different host).
fn rewritten_at_origin(url: &str, from: &str, to: &str) -> Option<String> {
    let rest = url.strip_prefix(from)?;
    if rest.is_empty() || rest.starts_with(['/', '?', '#']) {
        Some(format!("{to}{rest}"))
    } else {
        None
    }
}

fn rewrite_url_at_origin(url: &mut String, from: &str, to: &str) {
    if let Some(new) = rewritten_at_origin(url, from, to) {
        *url = new;
    }
}

/// The scenario's recorded origin: first `env` nav op's origin, falling back
/// to the first `goto` literal's. None when no absolute URL exists.
fn recorded_origin(scenario: &Scenario) -> Option<String> {
    for op in scenario
        .env
        .iter()
        .flat_map(|e| e.open.iter().chain(e.close.iter()))
        .flatten()
    {
        if let EnvOp::Nav { url: Some(u), .. } = op {
            if let Some(o) = origin_of(u) {
                return Some(o);
            }
        }
    }
    for step in &scenario.steps {
        if let Step::Do {
            verb: Verb::Goto,
            value: Some(Value::Literal { literal }),
            ..
        } = step
        {
            if let Some(o) = literal.as_str().and_then(origin_of) {
                return Some(o);
            }
        }
    }
    None
}

/// `scheme://host[:port]` of an absolute URL, None for relative/bare input.
fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let hostport = rest.split('/').next()?;
    if hostport.is_empty() {
        return None;
    }
    Some(format!("{scheme}://{hostport}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn locator_shorthand_lowers_to_named_raw() {
        let loc: Locator = serde_json::from_value(json!("css:p > a")).unwrap();
        assert!(matches!(
            loc,
            Locator::Raw(LocatorRaw {
                raw: LocatorRawSpec {
                    kind: RawLocatorKind::Css,
                    ..
                },
                ..
            })
        ));
        if let Locator::Raw(r) = loc {
            assert_eq!(r.raw.value, "p > a");
            assert_eq!(r.reason, "locator shorthand");
        }
        // every prefix
        for (short, want) in [
            ("xpath://a", RawLocatorKind::Xpath),
            ("testId:save-btn", RawLocatorKind::TestId),
            ("text:Sign in", RawLocatorKind::Text),
        ] {
            let loc: Locator = serde_json::from_value(json!(short)).unwrap();
            assert!(matches!(
                loc,
                Locator::Raw(LocatorRaw {
                    raw: LocatorRawSpec { kind, .. },
                    ..
                }) if kind == want
            ));
        }
        // missing / unknown prefixes fail with a hint
        assert!(serde_json::from_value::<Locator>(json!("p > a"))
            .unwrap_err()
            .to_string()
            .contains("kind prefix"));
        assert!(serde_json::from_value::<Locator>(json!("id:#x"))
            .unwrap_err()
            .to_string()
            .contains("unknown locator prefix"));
        // colons inside the selector survive (split at the FIRST colon only)
        let loc: Locator = serde_json::from_value(json!("css:a:not(.x)")).unwrap();
        assert!(matches!(
            loc,
            Locator::Raw(LocatorRaw {
                raw: LocatorRawSpec { value, .. },
                ..
            }) if value == "a:not(.x)"
        ));
        // object forms unchanged
        assert!(matches!(
            serde_json::from_value::<Locator>(json!({"role": "link", "name": "x"})).unwrap(),
            Locator::Role(_)
        ));
        assert!(matches!(
            serde_json::from_value::<Locator>(
                json!({"raw": {"kind": "css", "value": "#y"}, "reason": "r"})
            )
            .unwrap(),
            Locator::Raw(_)
        ));
    }

    #[test]
    fn retarget_origin_rewrites_navs_and_gotos() {
        let mut s: Scenario = serde_json::from_value(json!({
            "schema": "scenario/2", "id": "t", "intent": "x",
            "env": { "open": [
                { "kind": "nav", "url": "https://prod.example.com/app?x=1" },
                { "kind": "nav", "url": "https://cdn.other.com/asset" }
            ]},
            "steps": [
                { "id": "s1", "intent": "go", "kind": "do", "verb": "goto",
                  "value": { "from": "literal", "literal": "https://prod.example.com/dash" } },
                { "id": "s2", "intent": "ext", "kind": "do", "verb": "goto",
                  "value": { "from": "literal", "literal": "https://elsewhere.io/x" } },
                { "id": "s3", "intent": "port stays", "kind": "do", "verb": "goto",
                  "value": { "from": "literal", "literal": "https://prod.example.com:8443/deep" } }
            ]
        }))
        .unwrap();
        let from = retarget_origin(&mut s, "https://pr-7.preview.app").unwrap();
        assert_eq!(from, "https://prod.example.com");
        let ops = s.env.as_ref().unwrap().open.as_ref().unwrap();
        assert!(
            matches!(&ops[0], EnvOp::Nav { url: Some(u), .. } if u == "https://pr-7.preview.app/app?x=1")
        );
        // Foreign-origin nav is untouched.
        assert!(
            matches!(&ops[1], EnvOp::Nav { url: Some(u), .. } if u == "https://cdn.other.com/asset")
        );
        assert!(
            matches!(&s.steps[0], Step::Do { value: Some(Value::Literal { literal }), .. } if literal == "https://pr-7.preview.app/dash")
        );
        assert!(
            matches!(&s.steps[1], Step::Do { value: Some(Value::Literal { literal }), .. } if literal == "https://elsewhere.io/x")
        );
        // Same host but a different port is a different origin — untouched.
        assert!(
            matches!(&s.steps[2], Step::Do { value: Some(Value::Literal { literal }), .. } if literal == "https://prod.example.com:8443/deep")
        );
    }

    #[test]
    fn retarget_origin_falls_back_to_goto_and_reports_none_when_nothing() {
        // No env nav ops: the recorded origin comes from the first goto.
        let mut s: Scenario = serde_json::from_value(json!({
            "schema": "scenario/2", "id": "t", "intent": "x",
            "steps": [
                { "id": "s1", "intent": "go", "kind": "do", "verb": "goto",
                  "value": { "from": "literal", "literal": "https://a.io/" } }
            ]
        }))
        .unwrap();
        assert_eq!(
            retarget_origin(&mut s, "https://b.io"),
            Some("https://a.io".into())
        );
        assert!(
            matches!(&s.steps[0], Step::Do { value: Some(Value::Literal { literal }), .. } if literal == "https://b.io/")
        );
        // No absolute URL anywhere → nothing to retarget.
        let mut s2: Scenario = serde_json::from_value(json!({
            "schema": "scenario/2", "id": "t", "intent": "x",
            "steps": [
                { "id": "s1", "intent": "rel", "kind": "do", "verb": "goto",
                  "value": { "from": "literal", "literal": "/relative/path" } }
            ]
        }))
        .unwrap();
        assert_eq!(retarget_origin(&mut s2, "https://b.io"), None);
    }

    #[test]
    fn minimal_scenario_roundtrips() {
        let j = json!({
            "schema": "scenario/2",
            "id": "j1",
            "intent": "open the homepage",
            "steps": [
                {
                    "id": "s1",
                    "intent": "go to /",
                    "kind": "do",
                    "verb": "goto"
                },
                {
                    "id": "s2",
                    "intent": "url is set",
                    "kind": "check",
                    "claim": {
                        "subject": { "url": true },
                        "predicate": "exists"
                    }
                }
            ]
        });
        let parsed: Scenario = serde_json::from_value(j.clone()).unwrap();
        assert_eq!(parsed.id, "j1");
        assert_eq!(parsed.steps.len(), 2);
        assert!(matches!(parsed.steps[0], Step::Do { .. }));
        assert!(matches!(parsed.steps[1], Step::Check { .. }));

        let back = serde_json::to_value(&parsed).unwrap();
        // Round-trip preserves the meaningful subset (we strip None fields).
        assert_eq!(back["id"], j["id"]);
        assert_eq!(back["steps"][0]["verb"], "goto");
    }

    #[test]
    fn shot_subject_mask_roundtrips() {
        let parsed: Scenario = serde_json::from_value(json!({
            "schema": "scenario/2",
            "id": "j1",
            "intent": "visual",
            "steps": [
                {
                    "id": "s9",
                    "intent": "visual baseline",
                    "kind": "check",
                    "claim": {
                        "subject": { "shot": "s3", "mask": [".ts", "[data-qa-volatile]"] },
                        "predicate": "matches"
                    }
                },
                {
                    "id": "s10",
                    "intent": "plain shot",
                    "kind": "check",
                    "claim": {
                        "subject": { "shot": "s4" },
                        "predicate": "matches"
                    }
                }
            ]
        }))
        .unwrap();
        match &parsed.steps[0] {
            Step::Check { claim, .. } => match &claim.subject {
                ClaimSubject::Shot { shot, mask, .. } => {
                    assert_eq!(shot, "s3");
                    assert_eq!(mask, &[".ts", "[data-qa-volatile]"]);
                }
                other => panic!("expected shot subject, got {other:?}"),
            },
            other => panic!("expected check step, got {other:?}"),
        }
        // No mask key → empty vec, and serialisation drops the empty key.
        match &parsed.steps[1] {
            Step::Check { claim, .. } => match &claim.subject {
                ClaimSubject::Shot { shot, mask, .. } => {
                    assert_eq!(shot, "s4");
                    assert!(mask.is_empty());
                }
                other => panic!("expected shot subject, got {other:?}"),
            },
            other => panic!("expected check step, got {other:?}"),
        }
        let back = serde_json::to_value(&parsed).unwrap();
        assert!(back["steps"][1]["claim"]["subject"].get("mask").is_none());
        assert_eq!(
            back["steps"][0]["claim"]["subject"]["mask"],
            json!([".ts", "[data-qa-volatile]"])
        );
    }

    #[test]
    fn value_tagged_union_roundtrips() {
        let cases = [
            json!({ "from": "literal", "literal": "hi" }),
            json!({ "from": "input", "input": "email" }),
            json!({ "from": "step", "stepId": "s1", "path": "data.id" }),
            json!({ "from": "mint", "mint": { "template": "qa-{{vars._unique}}", "scope": "scenario" } }),
            json!({ "from": "loop", "loopVar": "row" }),
        ];
        for c in cases {
            let v: Value = serde_json::from_value(c.clone()).unwrap();
            let back = serde_json::to_value(&v).unwrap();
            assert_eq!(back, c, "value did not round-trip cleanly: {c}");
        }
    }

    #[test]
    fn locator_role_and_raw_distinguish() {
        let role: Locator = serde_json::from_value(json!({
            "role": "button",
            "name": "Save"
        }))
        .unwrap();
        assert!(matches!(role, Locator::Role(_)));

        let raw: Locator = serde_json::from_value(json!({
            "raw": { "kind": "css", "value": "button.save" },
            "reason": "design system uses non-accessible buttons"
        }))
        .unwrap();
        assert!(matches!(raw, Locator::Raw(_)));
    }

    #[test]
    fn env_op_kind_roundtrips() {
        let op: EnvOp = serde_json::from_value(json!({
            "kind": "nav",
            "url": "https://app.example.com/"
        }))
        .unwrap();
        match &op {
            EnvOp::Nav { url, .. } => assert_eq!(url.as_deref(), Some("https://app.example.com/")),
            _ => panic!("expected Nav variant"),
        }
    }

    #[test]
    fn claim_predicate_object_sugar_lowers_to_value() {
        // {"predicate": {"contains": "x"}} → predicate:Contains + value:"x"
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "url": true },
            "predicate": { "contains": "iana" }
        }))
        .unwrap();
        assert_eq!(claim.predicate, Predicate::Contains);
        assert_eq!(claim.value, Some(json!("iana")));
        // round-trips to the canonical string form
        let ser = serde_json::to_value(&claim).unwrap();
        assert_eq!(ser["predicate"], json!("contains"));

        // canonical string still works
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "url": true },
            "predicate": "exists"
        }))
        .unwrap();
        assert_eq!(claim.predicate, Predicate::Exists);
        assert_eq!(claim.value, None);

        // arg carried twice → error
        let err = serde_json::from_value::<Claim>(json!({
            "subject": { "url": true },
            "predicate": { "contains": "x" },
            "value": "y"
        }))
        .unwrap_err()
        .to_string();
        assert!(err.contains("twice"), "{err}");

        // unary predicate in object form → error pointing at the string form
        let err = serde_json::from_value::<Claim>(json!({
            "subject": { "url": true },
            "predicate": { "exists": "x" }
        }))
        .unwrap_err()
        .to_string();
        assert!(err.contains("takes no argument"), "{err}");

        // multi-key or non-string/object predicates → error
        assert!(serde_json::from_value::<Claim>(json!({
            "subject": { "url": true },
            "predicate": { "contains": "x", "equals": "y" }
        }))
        .is_err());
        assert!(serde_json::from_value::<Claim>(json!({
            "subject": { "url": true },
            "predicate": 42
        }))
        .is_err());
    }
}
