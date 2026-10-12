//! robots.txt (RFC 9309): the group for our user-agent token (else `*`), longest matching
//! Allow/Disallow rule wins with Allow winning ties, `*` and `$` wildcards, Crawl-delay and
//! Sitemap lines. Fetch outcomes follow the RFC: a 4xx means no restrictions; a 5xx, 429 or an
//! unreachable server means "assume everything is disallowed".

/// (user agents, (allow?, pattern) rules, crawl delay) of one group.
type Group = (Vec<String>, Vec<(bool, String)>, Option<f64>);

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Robots {
    rules: Vec<(bool, String)>,
    pub crawl_delay: Option<f64>,
    pub sitemaps: Vec<String>,
    disallow_all: bool,
}

impl Robots {
    pub fn allow_all() -> Self {
        Robots::default()
    }
    pub fn disallow_all() -> Self {
        Robots { disallow_all: true, ..Default::default() }
    }

    pub fn parse(text: &str, agent: &str) -> Self {
        let agent = agent.to_lowercase();
        let mut groups: Vec<Group> = Vec::new();
        let mut sitemaps = Vec::new();
        let mut in_agents = false;
        for line in text.lines().take(5000) {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((k, v)) = line.split_once(':') else { continue };
            let (k, v) = (k.trim().to_lowercase(), v.trim());
            match k.as_str() {
                "user-agent" => {
                    if !in_agents {
                        groups.push((Vec::new(), Vec::new(), None));
                    }
                    in_agents = true;
                    groups.last_mut().unwrap().0.push(v.to_lowercase());
                }
                "allow" | "disallow" => {
                    in_agents = false;
                    if let Some(g) = groups.last_mut() {
                        if !v.is_empty() {
                            g.1.push((k == "allow", v.to_string()));
                        }
                    }
                }
                "crawl-delay" => {
                    in_agents = false;
                    if let Some(g) = groups.last_mut() {
                        g.2 = v.parse::<f64>().ok().filter(|d| d.is_finite() && *d >= 0.0);
                    }
                }
                "sitemap" => sitemaps.push(v.to_string()),
                _ => {}
            }
        }
        // Our own group (all groups naming our token are combined), else the `*` groups.
        let pick = |want: &dyn Fn(&str) -> bool| {
            let gs: Vec<_> = groups.iter().filter(|g| g.0.iter().any(|a| want(a))).collect();
            (!gs.is_empty()).then(|| (gs.iter().flat_map(|g| g.1.clone()).collect::<Vec<_>>(), gs.iter().find_map(|g| g.2)))
        };
        let (rules, crawl_delay) = pick(&|a: &str| a != "*" && agent.starts_with(a)).or_else(|| pick(&|a: &str| a == "*")).unwrap_or_default();
        Robots { rules, crawl_delay, sitemaps, disallow_all: false }
    }

    /// May this path (with its query) be fetched?
    pub fn allowed(&self, path_and_query: &str) -> bool {
        if path_and_query == "/robots.txt" {
            return true;
        }
        if self.disallow_all {
            return false;
        }
        let best = self.rules.iter().filter(|(_, p)| matches(p, path_and_query)).max_by_key(|(allow, p)| (p.len(), *allow));
        best.is_none_or(|(allow, _)| *allow)
    }
}

/// Robots pattern match: anchored at the start, `*` matches any run, a trailing `$` anchors the end.
fn matches(pattern: &str, path: &str) -> bool {
    let (pat, anchored) = match pattern.strip_suffix('$') {
        Some(p) => (p, true),
        None => (pattern, false),
    };
    fn go(p: &[u8], s: &[u8], anchored: bool) -> bool {
        match p.split_first() {
            None => !anchored || s.is_empty(),
            Some((b'*', rest)) => (0..=s.len()).any(|i| go(rest, &s[i..], anchored)),
            Some((c, rest)) => s.first() == Some(c) && go(rest, &s[1..], anchored),
        }
    }
    go(pat.as_bytes(), path.as_bytes(), anchored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_rules_and_wildcards() {
        let txt = "# example\nUser-agent: *\nDisallow: /private\nAllow: /private/menu\nDisallow: /*.pdf$\nDisallow: /search?\nCrawl-delay: 4\n\nUser-agent: BadBot\nUser-agent: OtherBot\nDisallow: /\n\nSitemap: https://x.example/sitemap.xml\n";
        let r = Robots::parse(txt, "SalonResourceManager");
        assert!(r.allowed("/services"));
        assert!(!r.allowed("/private/staff"));
        assert!(r.allowed("/private/menu")); // longer Allow beats shorter Disallow
        assert!(!r.allowed("/files/menu.pdf"));
        assert!(r.allowed("/files/menu.pdf?v=2")); // "$" anchors the end
        assert!(!r.allowed("/search?q=hair"));
        assert_eq!(r.crawl_delay, Some(4.0));
        assert_eq!(r.sitemaps, ["https://x.example/sitemap.xml"]);
        let bad = Robots::parse(txt, "OtherBot/1.0");
        assert!(!bad.allowed("/services"));
        assert!(bad.allowed("/robots.txt"));
    }

    #[test]
    fn our_group_overrides_star_and_edge_cases() {
        let txt = "User-agent: *\nDisallow: /\n\nUser-agent: salonresourcemanager\nAllow: /\nDisallow: /cart\n";
        let r = Robots::parse(txt, "SalonResourceManager");
        assert!(r.allowed("/services") && !r.allowed("/cart/1"));
        assert!(Robots::parse("User-agent: *\nDisallow:\n", "SalonResourceManager").allowed("/anything"));
        assert!(Robots::parse("garbage\n<html>", "SalonResourceManager").allowed("/"));
        assert!(!Robots::disallow_all().allowed("/"));
        let tie = Robots::parse("User-agent: *\nDisallow: /a\nAllow: /a\n", "x");
        assert!(tie.allowed("/a")); // Allow wins a tie
    }
}
