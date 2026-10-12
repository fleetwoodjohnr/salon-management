//! Check one competitor's website for advertised prices, politely: robots.txt first, one request
//! at a time with a pause between them, at most a handful of pages chosen for looking like a
//! service menu, and a hard stop on rate limits or anti-bot checks.

use super::extract::{self, Item};
use super::http::{host_key, same_site, PageError, WebClient, ROBOTS_AGENT};
use super::robots::Robots;
use crate::db::now_utc;
use regex::Regex;
use reqwest::Url;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

pub const MAX_PAGES: usize = 8;
const MIN_DELAY: Duration = Duration::from_millis(1500);
const MAX_CRAWL_DELAY_S: f64 = 30.0;
pub const SITE_BUDGET: Duration = Duration::from_secs(90);

/// Sites whose terms of use don't allow automated reading, or that need a login. Prices on them
/// can still be recorded by hand.
const TERMS_RESTRICTED: &[(&str, &str)] = &[
    ("facebook.com", "Facebook"),
    ("fb.com", "Facebook"),
    ("instagram.com", "Instagram"),
    ("tiktok.com", "TikTok"),
    ("x.com", "X"),
    ("twitter.com", "X"),
    ("yelp.com", "Yelp"),
    ("google.com", "Google"),
    ("goo.gl", "Google"),
    ("g.page", "Google"),
    ("yellowpages.com", "Yellow Pages"),
    ("linktr.ee", "Linktree"),
    ("vagaro.com", "Vagaro"),
    ("booksy.com", "Booksy"),
    ("fresha.com", "Fresha"),
    ("styleseat.com", "StyleSeat"),
    ("glossgenius.com", "GlossGenius"),
    ("schedulicity.com", "Schedulicity"),
    ("mindbodyonline.com", "Mindbody"),
    ("mindbody.io", "Mindbody"),
    ("squareup.com", "Square Appointments"),
    ("joinblvd.com", "Boulevard"),
    ("blvd.co", "Boulevard"),
    ("zenoti.com", "Zenoti"),
    ("booker.com", "Booker"),
    ("salonbiz.com", "SalonBiz"),
    ("meevo.com", "Meevo"),
    ("phorest.com", "Phorest"),
    ("acuityscheduling.com", "Acuity"),
];

pub fn terms_restricted(u: &Url) -> Option<&'static str> {
    let h = u.host_str()?.to_lowercase();
    TERMS_RESTRICTED.iter().find(|(d, _)| h == *d || h.ends_with(&format!(".{d}"))).map(|(_, n)| *n)
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageRecord {
    pub url: String,
    /// research_pages.status
    pub status: &'static str,
    pub http_status: Option<u16>,
    pub prices_found: usize,
    pub consultation: Vec<String>,
    pub note: String,
    pub checked_at: String,
}

#[derive(Debug, Clone)]
pub struct Found {
    pub page_url: String,
    pub retrieved_at: String,
    pub item: Item,
}

#[derive(Debug, Default)]
pub struct SiteResult {
    pub pages: Vec<PageRecord>,
    pub found: Vec<Found>,
    /// Priced lines skipped as fees, retail, memberships etc.
    pub skipped_lines: usize,
}

impl SiteResult {
    /// The site-level outcome shown to the person.
    pub fn status(&self) -> Option<&'static str> {
        site_status(self.pages.iter().map(|p| (p.status, p.prices_found)))
    }
    /// Whether the site was actually read (so its earlier prices may be replaced).
    pub fn reached(&self) -> bool {
        matches!(self.status(), Some("prices_found" | "no_prices" | "not_html"))
    }
}

/// Combine page outcomes: prices anywhere win; then terms; then a site that was read but had no
/// prices (a PDF menu is worth saying); otherwise the first problem.
pub fn site_status<'a>(pages: impl Iterator<Item = (&'a str, usize)>) -> Option<&'static str> {
    let pages: Vec<(&str, usize)> = pages.collect();
    let has = |s: &str| pages.iter().any(|(p, _)| *p == s);
    if pages.is_empty() {
        None
    } else if pages.iter().any(|(_, n)| *n > 0) {
        Some("prices_found")
    } else if has("terms_restricted") {
        Some("terms_restricted")
    } else if has("no_prices") {
        Some(if has("not_html") { "not_html" } else { "no_prices" })
    } else {
        ["robots_disallowed", "blocked", "rate_limited", "unreachable", "http_error", "not_html"].into_iter().find(|s| has(s))
    }
}

/// robots.txt per host for one research run: (rules, note when it couldn't be read).
pub type RobotsCache = Arc<tokio::sync::Mutex<HashMap<String, (Arc<Robots>, Option<String>)>>>;

async fn robots_for(web: &WebClient, cache: &RobotsCache, url: &Url) -> (Arc<Robots>, Option<String>) {
    let key = host_key(url);
    if let Some(hit) = cache.lock().await.get(&key) {
        return hit.clone();
    }
    let unreadable = || (Robots::disallow_all(), Some("robots.txt couldn't be read (the server had an error, blocked the request or didn't answer), so the site was left alone to be safe.".to_string()));
    let robots_url = url.join("/robots.txt").expect("robots url");
    let mut result = web.get_text(&robots_url).await;
    if let Err(PageError::Moved(to)) = &result {
        result = web.get_text(to).await; // one cross-site hop (e.g. to a CDN)
    }
    let (r, note) = match result {
        Ok(p) => (Robots::parse(&p.body, ROBOTS_AGENT), None),
        Err(PageError::Status(s)) if (400..500).contains(&s) => (Robots::allow_all(), None),
        Err(PageError::NotHtml(_)) => (Robots::allow_all(), None),
        Err(_) => unreadable(),
    };
    let v = (Arc::new(r), note);
    cache.lock().await.insert(key, v.clone());
    v
}

fn excluded_path_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)/(?:blog|news|gallery|portfolio|careers?|jobs?|employment|hiring|team|staff|about|contact|privacy|terms|polic(?:y|ies)|cart|checkout|shop|store|products?|collections?|account|login|log-in|signin|sign-in|register|wp-admin|wp-login|wp-json|tags?|category|author|feed|events?|press|reviews?|testimonials?|faq|gift-?cards?|cdn-cgi)(?:/|$)").unwrap()
    })
}

const STRONG: &[&str] = &["price", "pricing", "menu", "service", "rate", "treatment", "cost"];
const MEDIUM: &[&str] = &[
    "hair", "color", "colour", "cut", "nail", "facial", "skin", "wax", "lash", "brow", "spa", "barber", "extension", "blowout", "balayage", "highlight", "bridal", "massage", "mani", "pedi", "keratin", "beauty", "style",
];

/// How much a link looks like it leads to a service menu (None: not worth a request).
pub fn link_score(u: &Url, text: &str) -> Option<i32> {
    let path = u.path().to_lowercase();
    let ext = path.rsplit('/').next().and_then(|f| f.rsplit_once('.')).map(|(_, e)| e.to_string());
    if ext.as_deref().is_some_and(|e| !matches!(e, "html" | "htm" | "php" | "aspx" | "asp")) || excluded_path_re().is_match(&path) {
        return None;
    }
    let text = text.to_lowercase();
    let next = text.contains("next page");
    if u.query().is_some() && !next {
        return None;
    }
    let hay = format!("{path} {text}");
    let mut score = if STRONG.iter().any(|k| hay.contains(k)) { 3 } else { 0 };
    score += MEDIUM.iter().filter(|k| hay.contains(*k)).count().min(3) as i32;
    if next {
        score += 2;
    }
    (score > 0).then_some(score)
}

fn is_pdf_menu(u: &Url, text: &str) -> bool {
    let hay = format!("{} {}", u.path().to_lowercase(), text.to_lowercase());
    u.path().to_lowercase().ends_with(".pdf") && (STRONG.iter().any(|k| hay.contains(k)) || MEDIUM.iter().any(|k| hay.contains(k)))
}

fn path_q(u: &Url) -> String {
    match u.query() {
        Some(q) => format!("{}?{q}", u.path()),
        None => u.path().to_string(),
    }
}

fn rec(url: &Url, status: &'static str, http_status: Option<u16>, note: impl Into<String>) -> PageRecord {
    PageRecord { url: url.to_string(), status, http_status, prices_found: 0, consultation: vec![], note: note.into(), checked_at: now_utc() }
}

fn loc_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)<loc>\s*([^<\s]+)\s*</loc>").unwrap())
}

/// Menu-like pages listed in the site's sitemap (one nested sitemap at most).
async fn sitemap_pages(web: &WebClient, robots: &Robots, site: &Url) -> Vec<Url> {
    let mut maps: Vec<Url> = robots.sitemaps.iter().filter_map(|s| Url::parse(s).ok()).filter(|u| same_site(u, site)).take(1).collect();
    if maps.is_empty() {
        maps.push(site.join("/sitemap.xml").expect("sitemap url"));
    }
    let mut out = Vec::new();
    for _ in 0..2 {
        let Some(m) = maps.pop() else { break };
        if !robots.allowed(&path_q(&m)) {
            continue;
        }
        let Ok(p) = web.get_text(&m).await else { continue };
        let locs: Vec<Url> = loc_re().captures_iter(&p.body).filter_map(|c| Url::parse(c[1].trim()).ok()).filter(|u| same_site(u, site)).take(2000).collect();
        if p.body.contains("<sitemapindex") {
            // Prefer the sitemap of ordinary pages.
            if let Some(child) = locs.iter().find(|u| u.path().contains("page")).or(locs.first()) {
                maps.push(child.clone());
            }
            continue;
        }
        out.extend(locs.into_iter().filter(|u| link_score(u, "").is_some_and(|s| s >= 3)).take(10));
    }
    out
}

/// A site's home page, if its terms and robots.txt allow reading it (for business details).
pub async fn homepage(web: &WebClient, robots: &RobotsCache, url: &Url) -> Option<super::http::Page> {
    if terms_restricted(url).is_some() || !robots_for(web, robots, url).await.0.allowed(&path_q(url)) {
        return None;
    }
    web.get(url).await.ok()
}

/// Read up to MAX_PAGES pages of one website.
pub async fn crawl_site(web: &WebClient, robots: &RobotsCache, start: &Url, cancel: &AtomicBool) -> SiteResult {
    let started = Instant::now();
    let mut res = SiteResult::default();
    let mut queue: Vec<(i32, Url)> = vec![(1000, start.clone())];
    let mut seen: HashSet<String> = HashSet::new();
    let mut site = start.clone();
    let (mut last_fetch, mut fetched, mut hops) = (None::<Instant>, 0usize, 0);
    let mut sitemap_tried = false;
    let mut pdfs: HashSet<String> = HashSet::new();
    while !queue.is_empty() {
        if fetched >= MAX_PAGES || cancel.load(Ordering::Relaxed) || started.elapsed() > SITE_BUDGET {
            break;
        }
        queue.sort_by_key(|(s, _)| *s);
        let (score, url) = queue.pop().unwrap();
        let is_start = score >= 1000;
        if !seen.insert(url.to_string()) {
            continue;
        }
        if let Some(name) = terms_restricted(&url) {
            res.pages.push(rec(&url, "terms_restricted", None, format!("{name} pages aren't read automatically because its terms don't allow it. Open the page to record prices yourself.")));
            continue;
        }
        let (rules, note) = robots_for(web, robots, &url).await;
        if !rules.allowed(&path_q(&url)) {
            res.pages.push(rec(&url, "robots_disallowed", None, note.unwrap_or_else(|| "The site's robots.txt asks automated visitors not to read this page.".into())));
            continue;
        }
        let delay = rules.crawl_delay.map(|d| Duration::from_secs_f64(d.min(MAX_CRAWL_DELAY_S))).unwrap_or_default().max(MIN_DELAY);
        if let Some(t) = last_fetch {
            let wait = delay.saturating_sub(t.elapsed());
            if !wait.is_zero() {
                tokio::time::sleep(wait).await;
            }
        }
        last_fetch = Some(Instant::now());
        fetched += 1;
        let checked_at = now_utc();
        match web.get(&url).await {
            Ok(page) => {
                let scan = extract::scan(&page.body, page.url.as_str());
                let mut r = rec(&page.url, if scan.items.is_empty() { "no_prices" } else { "prices_found" }, Some(200), "");
                r.checked_at = checked_at.clone();
                r.prices_found = scan.items.len();
                r.consultation = scan.consultation.clone();
                r.note = match (scan.retail_page, page.truncated) {
                    (true, _) => "Shop page: product prices aren't service prices.".into(),
                    (_, true) => "Very large page; only the first 3 MB was read.".into(),
                    _ => String::new(),
                };
                res.skipped_lines += scan.skipped;
                res.found.extend(scan.items.into_iter().map(|item| Found { page_url: page.url.to_string(), retrieved_at: checked_at.clone(), item }));
                res.pages.push(r);
                let mut strong = 0;
                for (u, text) in extract::links(&page.body, &page.url) {
                    if is_pdf_menu(&u, &text) && same_site(&u, &site) && pdfs.insert(u.to_string()) {
                        res.pages.push(rec(&u, "not_html", None, "PDF price list: not read automatically. Open it to record prices yourself."));
                    } else if same_site(&u, &site) && !seen.contains(u.as_str()) && !queue.iter().any(|(_, q)| *q == u) {
                        if let Some(s) = link_score(&u, &text) {
                            strong += (s >= 3) as i32;
                            queue.push((s, u));
                        }
                    }
                }
                if is_start && strong < 2 && !sitemap_tried {
                    sitemap_tried = true;
                    for u in sitemap_pages(web, &rules, &site).await {
                        if !seen.contains(u.as_str()) && !queue.iter().any(|(_, q)| *q == u) {
                            queue.push((3, u));
                        }
                    }
                }
            }
            Err(PageError::Moved(to)) if is_start && hops < 2 => {
                // The site lives elsewhere (another domain, a hosted page): continue there, after
                // checking that site's terms and robots.txt like any other.
                hops += 1;
                fetched -= 1;
                site = to.clone();
                queue.push((1000, to));
            }
            Err(PageError::Unreachable(m)) if is_start && url.scheme() == "https" && hops < 2 => {
                // Some small sites only work over plain http.
                hops += 1;
                let mut http = url.clone();
                let _ = http.set_scheme("http");
                site = http.clone();
                res.pages.push(rec(&url, "unreachable", None, format!("{} Trying http instead.", PageError::Unreachable(m))));
                queue.push((1000, http));
            }
            Err(e) => {
                let (status, code) = match &e {
                    PageError::RateLimited => ("rate_limited", Some(429)),
                    PageError::Blocked => ("blocked", None),
                    PageError::NotHtml(_) => ("not_html", None),
                    PageError::Status(s) => ("http_error", Some(*s)),
                    _ => ("unreachable", None),
                };
                let mut r = rec(&url, status, code, e.to_string());
                r.checked_at = checked_at;
                res.pages.push(r);
                let fatal = matches!(e, PageError::RateLimited | PageError::Blocked | PageError::Refused(_)) || (is_start && matches!(e, PageError::Unreachable(_)));
                if fatal {
                    break;
                }
                if is_start && matches!(e, PageError::Status(404 | 410)) && url.path() != "/" {
                    // A stale deep link: try the site's home page.
                    queue.push((1000, url.join("/").expect("root")));
                }
            }
        }
    }
    // A failed https attempt followed by a working http one isn't a problem worth showing.
    if res.pages.iter().any(|p| matches!(p.status, "prices_found" | "no_prices")) {
        res.pages.retain(|p| !(p.status == "unreachable" && p.note.ends_with("Trying http instead.")));
    }
    res
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// (status, content type, body, delay ms, extra headers)
    pub type Route = (u16, &'static str, String, u64, Vec<(String, String)>);

    /// A tiny HTTP/1.1 server on 127.0.0.1 for crawler tests.
    pub async fn serve(routes: Vec<(&'static str, Route)>) -> Url {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let routes: Arc<HashMap<&'static str, Route>> = Arc::new(routes.into_iter().collect());
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else { return };
                let routes = routes.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                    let (status, ctype, body, delay, headers) = routes.get(path.as_str()).cloned().unwrap_or((404, "text/html", "<h1>Not found</h1>".into(), 0, vec![]));
                    if delay > 0 {
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                    }
                    let extra: String = headers.iter().map(|(k, v)| format!("{k}: {v}\r\n")).collect();
                    let head = format!("HTTP/1.1 {status} X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n", body.len());
                    let _ = sock.write_all(head.as_bytes()).await;
                    let _ = sock.write_all(body.as_bytes()).await;
                    let _ = sock.shutdown().await;
                });
            }
        });
        Url::parse(&format!("http://{addr}/")).unwrap()
    }

    pub fn html(body: &str) -> Route {
        (200, "text/html; charset=utf-8", format!("<html><body>{body}</body></html>"), 0, vec![])
    }
    pub fn robots(txt: &str) -> Route {
        (200, "text/plain", txt.to_string(), 0, vec![])
    }

    fn client() -> WebClient {
        WebClient::for_tests(64 * 1024, Duration::from_millis(1500))
    }

    /// Live check against real salon websites found on OpenStreetMap (they may change):
    /// `cargo test live_crawl -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_crawl_real_salons() {
        let web = WebClient::new();
        let cache = RobotsCache::default();
        let mut with_prices = 0;
        for site in ["https://19thavenuesalon.com/", "https://www.salonvoda.com/", "https://industry-salon.com/", "https://reedalan.com/", "https://www.havegoodnail.com/"] {
            let t = Instant::now();
            let r = crawl_site(&web, &cache, &Url::parse(site).unwrap(), &AtomicBool::new(false)).await;
            with_prices += (!r.found.is_empty()) as i32;
            println!("{site}: {:?}, {} prices, {} pages, {:.0?}", r.status(), r.found.len(), r.pages.len(), t.elapsed());
            for p in &r.pages {
                println!("    {:<16} {:>3} {} {}", p.status, p.prices_found, p.url, p.note);
            }
            for f in r.found.iter().take(4) {
                println!("    ${:<8} {:<40} {:?} {}", f.item.price, f.item.label, f.item.canon.map(|c| c.key), f.item.stylist_level);
            }
        }
        assert!(with_prices >= 2, "expected prices from at least two of the sites");
    }

    #[test]
    fn scores_and_restrictions() {
        let u = |s: &str| Url::parse(s).unwrap();
        assert!(link_score(&u("https://s.example/services"), "Services").unwrap() >= 3);
        assert!(link_score(&u("https://s.example/hair-color"), "Color").is_some());
        assert_eq!(link_score(&u("https://s.example/blog/hair-trends"), "Blog"), None);
        assert_eq!(link_score(&u("https://s.example/shop/shampoo"), "Shampoo"), None);
        assert_eq!(link_score(&u("https://s.example/menu.jpg"), "Menu"), None);
        assert_eq!(link_score(&u("https://s.example/contact"), "Contact"), None);
        assert_eq!(terms_restricted(&u("https://www.vagaro.com/salon")), Some("Vagaro"));
        assert_eq!(terms_restricted(&u("https://salon.square.site/")), None);
        assert_eq!(site_status([("no_prices", 0), ("http_error", 0)].into_iter()), Some("no_prices"));
        assert_eq!(site_status([("no_prices", 0), ("prices_found", 3)].into_iter()), Some("prices_found"));
        assert_eq!(site_status([("robots_disallowed", 0)].into_iter()), Some("robots_disallowed"));
    }

    #[tokio::test]
    async fn crawls_menu_pages_politely_and_skips_the_rest() {
        let base = serve(vec![
            ("/robots.txt", robots("User-agent: *\nDisallow: /private\n")),
            ("/", html("<h1>Glow</h1><a href='/services'>Services &amp; Prices</a><a href='/blog/trends'>Blog</a><a href='/shop'>Shop</a><a href='/private/rates'>Rates</a><a href='/files/menu.pdf'>Price menu (PDF)</a><a href='https://elsewhere.example/'>Partner</a>")),
            ("/services", html("<h2>Hair</h2><p>Women's Haircut $65</p><p>Partial Balayage $180+</p><a href='/services?page=2' rel='next'>Next</a>")),
            ("/services?page=2", html("<p>Gel Manicure $40</p>")),
        ])
        .await;
        let cache = RobotsCache::default();
        let t = Instant::now();
        let r = crawl_site(&client(), &cache, &base, &AtomicBool::new(false)).await;
        let urls: Vec<_> = r.pages.iter().map(|p| (p.url.replace(base.as_str(), "/"), p.status)).collect();
        assert!(urls.contains(&("/services".into(), "prices_found")), "{urls:?}");
        assert!(urls.contains(&("/services?page=2".into(), "prices_found")), "{urls:?}");
        assert!(urls.contains(&("/private/rates".into(), "robots_disallowed")), "{urls:?}");
        assert!(urls.contains(&("/files/menu.pdf".into(), "not_html")), "{urls:?}");
        assert!(!urls.iter().any(|(u, _)| u.contains("blog") || u.contains("shop") || u.contains("elsewhere")), "{urls:?}");
        assert_eq!(r.found.len(), 3);
        assert!(r.found.iter().all(|f| f.page_url.starts_with(base.as_str()) && !f.retrieved_at.is_empty()));
        assert_eq!(r.status(), Some("prices_found"));
        // Three pages read with at least 1.5 s between requests.
        assert!(t.elapsed() >= Duration::from_millis(3000), "{:?}", t.elapsed());
    }

    #[tokio::test]
    async fn failures_are_reported_not_hidden() {
        let cache = RobotsCache::default();
        let w = client();
        let no = AtomicBool::new(false);
        // robots.txt answers 500: assume everything is disallowed.
        let a = serve(vec![("/robots.txt", (500, "text/plain", "oops".into(), 0, vec![])), ("/", html("<p>Haircut $40</p>"))]).await;
        let r = crawl_site(&w, &cache, &a, &no).await;
        assert_eq!(r.status(), Some("robots_disallowed"));
        assert!(r.found.is_empty() && r.pages[0].note.contains("couldn't be read"));
        // Rate limited.
        let b = serve(vec![("/", (429, "text/html", "slow down".into(), 0, vec![]))]).await;
        assert_eq!(crawl_site(&w, &cache, &b, &no).await.status(), Some("rate_limited"));
        // Anti-bot challenge: reported, not bypassed.
        let c = serve(vec![("/", (503, "text/html", "<title>Just a moment...</title>".into(), 0, vec![]))]).await;
        assert_eq!(crawl_site(&w, &cache, &c, &no).await.status(), Some("blocked"));
        // Too slow.
        let d = serve(vec![("/", (200, "text/html", "<p>Haircut $40</p>".into(), 3000, vec![]))]).await;
        let rd = crawl_site(&w, &cache, &d, &no).await;
        assert_eq!(rd.status(), Some("unreachable"));
        assert!(rd.pages[0].note.contains("timed out"), "{:?}", rd.pages);
        // Redirect loop.
        let e = serve(vec![("/", (302, "text/html", String::new(), 0, vec![("Location".into(), "/a".into())])), ("/a", (302, "text/html", String::new(), 0, vec![("Location".into(), "/".into())]))]).await;
        assert_eq!(crawl_site(&w, &cache, &e, &no).await.status(), Some("unreachable"));
        // A PDF instead of a page, and an oversized page (read up to the cap, still parsed).
        let f = serve(vec![("/", (200, "application/pdf", "%PDF-1.4".into(), 0, vec![]))]).await;
        assert_eq!(crawl_site(&w, &cache, &f, &no).await.status(), Some("not_html"));
        let big = format!("<p>Women's Haircut $70</p>{}", "<p>filler text</p>".repeat(10_000));
        let g = serve(vec![("/", (200, "text/html", big, 0, vec![]))]).await;
        let rg = crawl_site(&w, &cache, &g, &no).await;
        assert_eq!((rg.status(), rg.found.len()), (Some("prices_found"), 1));
        assert!(rg.pages[0].note.contains("first 3 MB"));
        // A stale deep link falls back to the home page; cancelling stops before any request.
        let h = serve(vec![("/", html("<p>Pedicure $50</p>"))]).await;
        assert_eq!(crawl_site(&w, &cache, &h.join("/old-menu").unwrap(), &no).await.found.len(), 1);
        assert!(crawl_site(&w, &cache, &h, &AtomicBool::new(true)).await.pages.is_empty());
    }
}
