//! Turn a hostname into a scenario-id segment.
//!
//! Crawl and discover both slug a host, and they do not use the same rule.
//! [`HostSlug::AsciiTrim`] is what crawl has always done: ASCII letters and
//! digits stay, every other character becomes `-`, and `-` at either end is
//! removed. [`HostSlug::Unicode`] is what discover has always done: Unicode
//! letters and digits stay, every other character becomes `-`, and `-` at
//! either end stays.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostSlug {
    /// Crawl. ASCII alphanumeric and `-` stay. Ends are trimmed.
    AsciiTrim,
    /// Discover. Unicode alphanumeric and `-` stay. Ends are kept.
    Unicode,
}

pub(crate) fn sanitize_host(host: &str, policy: HostSlug) -> String {
    let mapped: String = host
        .chars()
        .map(|c| {
            let keep = match policy {
                HostSlug::AsciiTrim => c.is_ascii_alphanumeric() || c == '-',
                HostSlug::Unicode => c.is_alphanumeric() || c == '-',
            };
            if keep {
                c
            } else {
                '-'
            }
        })
        .collect();
    match policy {
        HostSlug::AsciiTrim => mapped.trim_matches('-').to_string(),
        HostSlug::Unicode => mapped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_policies_agree_on_plain_ascii_hosts() {
        for policy in [HostSlug::AsciiTrim, HostSlug::Unicode] {
            assert_eq!(sanitize_host("app.example.com", policy), "app-example-com");
            assert_eq!(sanitize_host("localhost", policy), "localhost");
            assert_eq!(sanitize_host("127.0.0.1", policy), "127-0-0-1");
            assert_eq!(sanitize_host("", policy), "");
            assert_eq!(sanitize_host("a--b", policy), "a--b");
        }
    }

    #[test]
    fn crawl_policy_is_ascii_and_trims_dashes() {
        assert_eq!(
            sanitize_host("example.com.", HostSlug::AsciiTrim),
            "example-com"
        );
        assert_eq!(
            sanitize_host(".localhost", HostSlug::AsciiTrim),
            "localhost"
        );
        assert_eq!(sanitize_host("-host-", HostSlug::AsciiTrim), "host");
        assert_eq!(sanitize_host("..", HostSlug::AsciiTrim), "");
        assert_eq!(
            sanitize_host("münchen.de", HostSlug::AsciiTrim),
            "m-nchen-de"
        );
        assert_eq!(sanitize_host("例.com", HostSlug::AsciiTrim), "com");
    }

    #[test]
    fn discover_policy_keeps_unicode_and_edge_dashes() {
        assert_eq!(
            sanitize_host("example.com.", HostSlug::Unicode),
            "example-com-"
        );
        assert_eq!(sanitize_host(".localhost", HostSlug::Unicode), "-localhost");
        assert_eq!(sanitize_host("-host-", HostSlug::Unicode), "-host-");
        assert_eq!(sanitize_host("..", HostSlug::Unicode), "--");
        assert_eq!(sanitize_host("münchen.de", HostSlug::Unicode), "münchen-de");
        assert_eq!(sanitize_host("例.com", HostSlug::Unicode), "例-com");
    }
}
