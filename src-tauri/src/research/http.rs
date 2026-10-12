//! HTTP for reading competitors' public web pages.
//!
//! Safety rules (the URLs come from map data and other people's websites, so they're untrusted):
//! - http and https on the standard ports only, no credentials in URLs;
//! - hosts must resolve to public internet addresses (never this computer or the local network),
//!   checked in the DNS resolver and for IP-literal URLs on every redirect hop;
//! - redirects are followed only within the same site; a move to another site is returned to the
//!   crawler, which checks that site's robots.txt and terms first;
//! - bodies are capped, only HTML is read, no cookies are kept and no scripts are run;
//! - an anti-bot challenge or CAPTCHA is reported as "blocked" and never worked around.

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::Url;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

pub const USER_AGENT: &str = concat!("SalonResourceManager/", env!("CARGO_PKG_VERSION"), " (+https://github.com/fleetwoodjohnr/salon-management; user-initiated price research)");
/// The token robots.txt groups are matched against.
pub const ROBOTS_AGENT: &str = "SalonResourceManager";

#[derive(Debug, Clone, PartialEq)]
pub enum PageError {
    /// DNS failure, refused connection or timeout.
    Unreachable(String),
    Status(u16),
    RateLimited,
    /// An anti-bot challenge, CAPTCHA or "access denied" page.
    Blocked,
    /// Not an HTML page (PDF, image…): the content type.
    NotHtml(String),
    /// A URL or address this app won't fetch (local network, odd port or scheme).
    Refused(String),
    /// The site redirected to another site (returned so robots.txt can be checked there).
    Moved(Url),
}

impl std::fmt::Display for PageError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PageError::Unreachable(m) => write!(f, "Couldn't reach the site ({m})."),
            PageError::Status(s) => write!(f, "The site answered with an error (HTTP {s})."),
            PageError::RateLimited => write!(f, "The site asked us to slow down (HTTP 429), so it was left alone."),
            PageError::Blocked => write!(f, "The site shows an anti-bot check or denies automated access. It wasn't bypassed."),
            PageError::NotHtml(t) => write!(f, "Not a web page ({t})."),
            PageError::Refused(m) => write!(f, "{m}"),
            PageError::Moved(u) => write!(f, "Moved to {u}."),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Page {
    pub url: Url,
    pub body: String,
    pub truncated: bool,
}

pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => is_public_v4(v),
        IpAddr::V6(v) => {
            if let Some(v4) = v.to_ipv4_mapped() {
                return is_public_v4(v4);
            }
            let s = v.segments();
            !(v.is_loopback() || v.is_unspecified() || v.is_multicast() || (s[0] & 0xfe00) == 0xfc00 || (s[0] & 0xffc0) == 0xfe80 || (s[0] == 0x2001 && s[1] == 0x0db8) || v == Ipv6Addr::LOCALHOST)
        }
    }
}

fn is_public_v4(v: Ipv4Addr) -> bool {
    let o = v.octets();
    !(v.is_loopback()
        || v.is_private()
        || v.is_link_local()
        || v.is_broadcast()
        || v.is_unspecified()
        || v.is_multicast()
        || v.is_documentation()
        || o[0] == 0
        || o[0] >= 240
        || (o[0] == 100 && (64..128).contains(&o[1])) // carrier-grade NAT
        || (o[0] == 198 && (18..20).contains(&o[1])) // benchmarking
        || (o[0] == 192 && o[1] == 0 && o[2] == 0))
}

/// DNS resolution that drops non-public addresses.
struct PublicOnly {
    allow_loopback: bool,
}

impl Resolve for PublicOnly {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        let allow_loopback = self.allow_loopback;
        Box::pin(async move {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0)).await?.filter(|a| is_public(a.ip()) || (allow_loopback && a.ip().is_loopback())).collect();
            if addrs.is_empty() {
                return Err(format!("{host} doesn't resolve to a public internet address").into());
            }
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

/// Same site: equal hosts ignoring a leading "www.", either scheme, and the same explicit port.
pub fn same_site(a: &Url, b: &Url) -> bool {
    let h = |u: &Url| u.host_str().map(|h| h.trim_start_matches("www.").to_lowercase());
    h(a).is_some() && h(a) == h(b) && a.port() == b.port()
}

/// Host key for per-site bookkeeping (robots cache, politeness): host plus non-default port.
pub fn host_key(u: &Url) -> String {
    match u.port() {
        Some(p) => format!("{}:{p}", u.host_str().unwrap_or("")),
        None => u.host_str().unwrap_or("").to_string(),
    }
}

pub struct WebClient {
    client: reqwest::Client,
    allow_loopback: bool,
    max_bytes: usize,
}

impl WebClient {
    pub fn new() -> Self {
        Self::build(false, 3 * 1024 * 1024, Duration::from_secs(20))
    }

    /// For tests against a local server.
    #[cfg(test)]
    pub fn for_tests(max_bytes: usize, timeout: Duration) -> Self {
        Self::build(true, max_bytes, timeout)
    }

    fn build(allow_loopback: bool, max_bytes: usize, timeout: Duration) -> Self {
        let policy = reqwest::redirect::Policy::custom(move |attempt| {
            let (next, prev) = (attempt.url().clone(), attempt.previous().last().cloned());
            if attempt.previous().len() >= 5 {
                attempt.error("too many redirects")
            } else if let Err(e) = Self::check_url(&next, allow_loopback) {
                attempt.error(e)
            } else if prev.is_some_and(|p| !same_site(&p, &next)) {
                attempt.stop()
            } else {
                attempt.follow()
            }
        });
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10).min(timeout))
            .timeout(timeout)
            .redirect(policy)
            .dns_resolver(Arc::new(PublicOnly { allow_loopback }))
            .build()
            .expect("HTTP client");
        WebClient { client, allow_loopback, max_bytes }
    }

    /// Scheme, port and literal-address checks (names are checked by the resolver).
    fn check_url(u: &Url, allow_loopback: bool) -> Result<(), String> {
        if !matches!(u.scheme(), "http" | "https") || !u.username().is_empty() || u.password().is_some() {
            return Err("Only plain http(s) web addresses are read.".into());
        }
        let host = u.host_str().ok_or("That web address has no host.")?;
        let local = "Addresses on this computer or the local network are never read.";
        match host.trim_start_matches('[').trim_end_matches(']').parse::<IpAddr>() {
            Ok(ip) if !is_public(ip) && !(allow_loopback && ip.is_loopback()) => return Err(local.into()),
            Ok(_) => {}
            Err(_) if !allow_loopback && (host == "localhost" || [".localhost", ".local", ".internal", ".lan", ".home.arpa"].iter().any(|s| host.ends_with(s))) => return Err(local.into()),
            Err(_) => {}
        }
        if !allow_loopback && !matches!(u.port(), None | Some(80) | Some(443)) {
            return Err("Only standard web ports (80 and 443) are read.".into());
        }
        Ok(())
    }

    /// GET a page; Ok only for an HTML (or plain-text) body.
    pub async fn get(&self, url: &Url) -> Result<Page, PageError> {
        self.fetch(url, false).await
    }

    /// GET robots.txt or a sitemap: any text, including XML.
    pub async fn get_text(&self, url: &Url) -> Result<Page, PageError> {
        self.fetch(url, true).await
    }

    async fn fetch(&self, url: &Url, any_text: bool) -> Result<Page, PageError> {
        Self::check_url(url, self.allow_loopback).map_err(PageError::Refused)?;
        let mut r = self.client.get(url.clone()).header("Accept", "text/html,application/xhtml+xml;q=0.9,text/plain;q=0.5").send().await.map_err(|e| {
            if e.is_redirect() {
                PageError::Unreachable("too many redirects or a redirect to a refused address".into())
            } else if e.is_timeout() {
                PageError::Unreachable("timed out".into())
            } else {
                // Keep the reason short and free of internals.
                let s = e.to_string();
                PageError::Unreachable(if s.contains("public internet address") { "not a public internet address".into() } else if e.is_connect() { "connection failed".into() } else { "request failed".into() })
            }
        })?;
        let status = r.status().as_u16();
        let final_url = r.url().clone();
        if r.status().is_redirection() {
            let to = r.headers().get("location").and_then(|v| v.to_str().ok()).and_then(|l| final_url.join(l).ok());
            return match to {
                Some(u) => Err(PageError::Moved(u)),
                None => Err(PageError::Status(status)),
            };
        }
        if status == 429 {
            return Err(PageError::RateLimited);
        }
        let ctype = r.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_lowercase();
        let mut buf: Vec<u8> = Vec::new();
        let mut truncated = false;
        while let Some(chunk) = r.chunk().await.map_err(|_| PageError::Unreachable("the connection dropped while reading".into()))? {
            buf.extend_from_slice(&chunk);
            if buf.len() >= self.max_bytes {
                buf.truncate(self.max_bytes);
                truncated = true;
                break;
            }
        }
        let body = String::from_utf8_lossy(&buf).into_owned();
        if looks_blocked(status, &body) {
            return Err(PageError::Blocked);
        }
        if !(200..300).contains(&status) {
            return Err(PageError::Status(status));
        }
        let html = ctype.contains("html") || ctype.starts_with("text/plain") || (ctype.is_empty() && body.trim_start().starts_with('<')) || (any_text && (ctype.starts_with("text/") || ctype.contains("xml") || ctype.is_empty()));
        if !html {
            return Err(PageError::NotHtml(if ctype.contains("pdf") { "PDF".into() } else { ctype.split(';').next().unwrap_or("unknown type").to_string() }));
        }
        Ok(Page { url: final_url, body, truncated })
    }
}

impl Default for WebClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Anti-bot interstitials and CAPTCHAs. Many normal pages load Cloudflare scripts, so a 200
/// answer only counts as blocked when the page itself is the challenge.
pub fn looks_blocked(status: u16, body: &str) -> bool {
    let head: String = body.chars().take(20_000).collect::<String>().to_lowercase();
    let challenge = ["just a moment...", "attention required! | cloudflare", "cf-browser-verification", "cf_chl_", "px-captcha", "_incapsula_resource", "ddos-guard", "captcha-delivery", "are you a robot", "verify you are human", "g-recaptcha", "h-captcha"]
        .iter()
        .any(|m| head.contains(m));
    match status {
        403 => true,
        503 | 401 => challenge,
        200..=299 => challenge && body.len() < 60_000,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_addresses_only() {
        for ip in ["127.0.0.1", "10.1.2.3", "172.16.0.9", "192.168.1.1", "169.254.169.254", "100.64.0.1", "0.0.0.0", "::1", "fe80::1", "fd00::5", "::ffff:192.168.0.1", "224.0.0.1"] {
            assert!(!is_public(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["93.184.216.34", "1.1.1.1", "2606:4700::1111"] {
            assert!(is_public(ip.parse().unwrap()), "{ip}");
        }
        let bad = ["http://192.168.1.1/", "http://localhost/", "http://router.local/", "https://x.example:8443/", "ftp://x.example/", "https://u:p@x.example/", "http://[::1]/"];
        for u in bad {
            assert!(WebClient::check_url(&Url::parse(u).unwrap(), false).is_err(), "{u}");
        }
        assert!(WebClient::check_url(&Url::parse("https://www.salon.example/menu").unwrap(), false).is_ok());
    }

    #[test]
    fn same_site_and_blocking() {
        let u = |s: &str| Url::parse(s).unwrap();
        assert!(same_site(&u("http://salon.example/"), &u("https://www.salon.example/menu")));
        assert!(!same_site(&u("https://salon.example/"), &u("https://salon.square.site/")));
        assert!(looks_blocked(503, "<title>Just a moment...</title>"));
        assert!(looks_blocked(403, "Forbidden"));
        assert!(!looks_blocked(200, "<script src='/cdn-cgi/challenge-platform/x.js'></script><p>Haircut $40</p>"));
        assert!(!looks_blocked(503, "Service Unavailable"));
    }
}
