/// Analytics/telemetry collectors — fire-and-forget beacons the page is
/// not guaranteed to resend on replay (and whose URLs carry per-visitor
/// nonces). Claiming them produces scenarios that flake on the second run.
const TELEMETRY_HOSTS: &[&str] = &[
    "optimizely.com",
    "google-analytics.com",
    "googletagmanager.com",
    "analytics.google.com",
    "segment.io",
    "segment.com",
    "mixpanel.com",
    "amplitude.com",
    "hotjar.com",
    "datadoghq.com",
    "sentry.io",
    "newrelic.com",
    "nr-data.net",
    "fullstory.com",
    // Feature-flag SDK: evalx/goals/events calls carry per-session
    // base64 contexts — the calls can't refire identically on replay.
    "launchdarkly.com",
    "logrocket.com",
    "pendo.io",
    "heapanalytics.com",
    "doubleclick.net",
    "clarity.ms",
    "bugsnag.com",
    "intercom.io",
    "plausible.io",
    "mouseflow.com",
    "crazyegg.com",
    "luckyorange.com",
    "criteo.com",
    "adservice.google.com",
    "googlesyndication.com",
    "fundingchoicesmessages.google.com",
    "adtrafficquality.google",
    "consent.google.com",
    // Ad + tracker networks whose beacons carry per-impression ids —
    // never app traffic, never refirable on replay.
    "adnxs.com",
    "adform.net",
    "amazon-adsystem.com",
    "adsafeprotected.com",
    "adtrafficquality.google.com",
    "bat.bing.com",
    "chartbeat.com",
    "connect.facebook.net",
    "doubleverify.com",
    "facebook.net",
    "moatads.com",
    "outbrain.com",
    "quantserve.com",
    "scorecardresearch.com",
    "taboola.com",
    "analytics.tiktok.com",
    "tr.snapchat.com",
    "alb.reddit.com",
    "pixel.onaudience.com",
    // Google's client-side instrumentation beacon (csi.gstatic.com/csi) —
    // fires opportunistically off Google-hosted js; a captured POST here
    // is never guaranteed to refire on replay.
    "csi.gstatic.com",
];

/// Same-origin beacon paths a host blocklist can't see. `/cdn-cgi/` is
/// Cloudflare's reserved namespace (rum, challenge-platform, zaraz,
/// scripts) — every path under it is infrastructure, never app traffic,
/// and the URLs carry per-visitor nonces that cannot refire on replay.
/// `/_vercel/` is Vercel's analytics/speed-insights reserved namespace —
/// same deal on Vercel-hosted apps.
const TELEMETRY_PATHS: &[&str] = &["/cdn-cgi/", "/_vercel/"];

pub(crate) fn is_telemetry_url(url: &str) -> bool {
    let after_scheme = url.split("://").nth(1).unwrap_or("");
    let host = after_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    if TELEMETRY_HOSTS
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
    {
        return true;
    }
    let path = after_scheme
        .split_once('/')
        .map(|(_, p)| format!("/{p}"))
        .unwrap_or_default();
    TELEMETRY_PATHS.iter().any(|p| path.starts_with(p))
}

/// Strip `;k=v` matrix params (e.g. `;jsessionid=…`) from each path segment —
/// they are almost always session-scoped, so a claim matcher carrying one can
/// never match a fresh session at replay.
fn strip_matrix_params(url_path: &str) -> String {
    url_path
        .split('/')
        .map(|seg| seg.split(';').next().unwrap_or(seg))
        .collect::<Vec<_>>()
        .join("/")
}

/// Rewrite entity ids in URL path segments into regexes — an auto claim
/// recorded as `…/customers/63827` can never match the fresh id a replay
/// mints, so all-numeric segments become `\d+` and long hex/uuid segments
/// become `[0-9a-fA-F-]{8,}`.
fn normalize_volatile_path_segments(url_path: &str) -> String {
    url_path
        .split('/')
        .map(|seg| {
            if !seg.is_empty() && seg.chars().all(|c| c.is_ascii_digit()) {
                "\\d+".to_string()
            } else if seg.len() >= 8 && seg.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
                "[0-9a-fA-F-]{8,}".to_string()
            } else {
                seg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Origin+path for an auto-minted network claim: drop query/fragment, strip
/// matrix params, rewrite volatile id segments. Shared by crawl drafts and
/// flush `--auto-network` so the two cannot drift.
pub(crate) fn claimable_network_path(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    normalize_volatile_path_segments(&strip_matrix_params(path))
}

#[cfg(test)]
mod tests {
    use super::{claimable_network_path, is_telemetry_url};

    #[test]
    fn claimable_network_path_strips_matrix_and_rewrites_ids() {
        assert_eq!(
            claimable_network_path("https://x/app/login.htm;jsessionid=ABC123?x=1"),
            "https://x/app/login.htm"
        );
        assert_eq!(
            claimable_network_path("https://x/api/contacts/6abbfc58b45a2a0015047afa"),
            "https://x/api/contacts/[0-9a-fA-F-]{8,}"
        );
        assert_eq!(
            claimable_network_path("https://x/api/items/1234567"),
            "https://x/api/items/\\d+"
        );
        assert_eq!(
            claimable_network_path("https://x/api/v2/users"),
            "https://x/api/v2/users"
        );
    }

    #[test]
    fn crawl_source_has_no_private_telemetry_host_list() {
        const SRC: &str = include_str!("crawl.rs");
        assert!(
            !SRC.contains("const TELEMETRY_HOSTS"),
            "crawl must call crate::telemetry::is_telemetry_url — do not reintroduce a private host list"
        );
    }

    #[test]
    fn telemetry_hosts_and_paths() {
        assert!(is_telemetry_url(
            "https://www.google-analytics.com/collect?v=1"
        ));
        assert!(is_telemetry_url("https://cdn.segment.io/x"));
        assert!(is_telemetry_url("https://sub.sentry.io/api/1/envelope/"));
        assert!(is_telemetry_url("https://example.com/cdn-cgi/rum"));
        assert!(is_telemetry_url(
            "https://example.com/cdn-cgi/challenge-platform/h/b/jsd/oneshot/abc"
        ));
        assert!(is_telemetry_url(
            "https://fundingchoicesmessages.google.com/el/ABC123"
        ));
        assert!(is_telemetry_url(
            "https://pagead2.googlesyndication.com/pagead/x"
        ));
        assert!(is_telemetry_url("https://sb.scorecardresearch.com/beacon"));
        assert!(is_telemetry_url(
            "https://app.launchdarkly.com/sdk/evalx/abc123/contexts/xyz"
        ));
        assert!(is_telemetry_url(
            "https://events.launchdarkly.com/events/bulk/abc123"
        ));
        assert!(is_telemetry_url("https://ads.amazon-adsystem.com/aax"));
        assert!(is_telemetry_url(
            "https://app.example.com/_vercel/insights/view"
        ));
        assert!(is_telemetry_url(
            "https://csi.gstatic.com/csi?v=3&s=page&action=login"
        ));
        assert!(!is_telemetry_url("https://api.optimizelyx.com/v1"));
        assert!(!is_telemetry_url("https://example.com/api/customers"));
        assert!(!is_telemetry_url("https://example.com/cdn-cgix/proxy"));
    }
}
