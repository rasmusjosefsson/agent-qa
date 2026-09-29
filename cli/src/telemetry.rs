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
];

/// Same-origin beacon paths a host blocklist can't see — e.g. Cloudflare
/// RUM posts to `/cdn-cgi/rum` on the page's own domain.
const TELEMETRY_PATHS: &[&str] = &["/cdn-cgi/rum"];

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

#[cfg(test)]
mod tests {
    use super::is_telemetry_url;

    #[test]
    fn telemetry_hosts_and_paths() {
        assert!(is_telemetry_url(
            "https://www.google-analytics.com/collect?v=1"
        ));
        assert!(is_telemetry_url("https://cdn.segment.io/x"));
        assert!(is_telemetry_url("https://sub.sentry.io/api/1/envelope/"));
        assert!(is_telemetry_url("https://example.com/cdn-cgi/rum"));
        assert!(!is_telemetry_url("https://api.optimizelyx.com/v1"));
        assert!(!is_telemetry_url("https://example.com/api/customers"));
        assert!(!is_telemetry_url("https://example.com/cdn-cgi/challenge"));
    }
}
