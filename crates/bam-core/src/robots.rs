//! Minimal `robots.txt` support for P4.3's harvester (`bam-handoff.md` §7:
//! "robots.txt respected"). Ungated like [`crate::http`]: parsing is pure,
//! and [`fetch_rules`] is generic over [`crate::http::HttpClient`], so it
//! needs neither the `native` feature nor a network to test.
//!
//! Only the wildcard `User-agent: *` group is honoured — bam is a generic
//! polite crawler, not a named one a site would single out — which keeps the
//! parser to the handful of directives that matter (`User-agent`,
//! `Disallow`) rather than a full RFC 9309 implementation (`Allow`
//! overrides, crawl-delay, wildcards within a path) nothing here needs yet.

use crate::http::{HttpClient, HttpRequest};

pub struct RobotsRules {
    disallow: Vec<String>,
}

impl RobotsRules {
    /// The permissive default: no `robots.txt`, or one that failed to fetch,
    /// blocks nothing (the conventional crawler behaviour).
    pub fn allow_all() -> Self {
        Self {
            disallow: Vec::new(),
        }
    }

    pub fn is_allowed(&self, path: &str) -> bool {
        !self.disallow.iter().any(|prefix| path.starts_with(prefix))
    }
}

/// Parses the `Disallow` lines of the `User-agent: *` group. Unknown
/// directives and other groups are ignored rather than rejected — a
/// malformed `robots.txt` should degrade to "fewer rules recognised", not to
/// a hard error that would block every fetch to that origin.
pub fn parse(body: &str) -> RobotsRules {
    let mut in_wildcard_group = false;
    let mut disallow = Vec::new();
    for line in body.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim().to_ascii_lowercase().as_str() {
            "user-agent" => in_wildcard_group = value == "*",
            "disallow" if in_wildcard_group && !value.is_empty() => {
                disallow.push(value.to_string());
            }
            _ => {}
        }
    }
    RobotsRules { disallow }
}

/// Splits a URL into its origin (`scheme://host[:port]`) and path
/// (`/...`, defaulting to `/`). No `url` crate dependency: both callers
/// (this module, P4.3's worker) only ever need this one split.
pub fn origin_and_path(url: &str) -> (&str, &str) {
    let after_scheme = url.find("://").map(|i| i + 3).unwrap_or(0);
    match url[after_scheme..].find('/') {
        Some(rel) => (&url[..after_scheme + rel], &url[after_scheme + rel..]),
        None => (url, "/"),
    }
}

/// Fetches `{origin}/robots.txt` and parses it, falling back to
/// [`RobotsRules::allow_all`] on any non-200 response or transport error —
/// a `robots.txt` fetch failure must never itself block otherwise-permitted
/// fetches.
pub async fn fetch_rules(client: &impl HttpClient, origin: &str) -> RobotsRules {
    let req = HttpRequest {
        url: format!("{origin}/robots.txt"),
        if_none_match: None,
    };
    match client.get(req).await {
        Ok(resp) if resp.status == 200 => parse(&String::from_utf8_lossy(&resp.body)),
        _ => RobotsRules::allow_all(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disallow_prefix_blocks_matching_paths() {
        let rules = parse("User-agent: *\nDisallow: /private/\nDisallow: /tmp\n");
        assert!(!rules.is_allowed("/private/secret.txt"));
        assert!(!rules.is_allowed("/tmp"));
        assert!(rules.is_allowed("/readmes/foo.readme"));
    }

    #[test]
    fn only_wildcard_group_is_honoured() {
        let rules = parse("User-agent: Googlebot\nDisallow: /\nUser-agent: *\nDisallow: /no\n");
        assert!(rules.is_allowed("/yes"));
        assert!(!rules.is_allowed("/no/thing"));
    }

    #[test]
    fn origin_and_path_splits_correctly() {
        assert_eq!(
            origin_and_path("https://ftp.fau.de/aminet/biz/dbase/Foo.readme"),
            ("https://ftp.fau.de", "/aminet/biz/dbase/Foo.readme")
        );
        assert_eq!(
            origin_and_path("https://ftp.fau.de"),
            ("https://ftp.fau.de", "/")
        );
    }
}
