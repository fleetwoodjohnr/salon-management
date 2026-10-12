//! Read advertised prices from a salon's own web page.
//!
//! Two passes: schema.org JSON-LD offers (structured), then the visible text, read as one line per
//! block element (table rows keep their cells, headings are remembered as context). On each line:
//! - "$45" fixed; "$45+", "from $45", "$45 & up" starting at; "$45 - $60", "$45–60" a range;
//!   "by consultation" / "price upon request" (no amount) is recorded as consultation-only.
//! - Several prices on one line are never merged: stylist tiers ("Associate $45 Senior $55"),
//!   hair-length columns and table headers become separate prices, each with its tier.
//! - The service name is the text before the price, else the nearest earlier line without a
//!   price (menus often put the name and price in separate elements).
//! - Fees, deposits, gift cards, discounts, memberships, retail products and shop pages are
//!   skipped, as are amounts outside $5–$2,000.
//!
//! Nothing is inferred: a line without a readable dollar amount never produces a price.

use super::catalog::{self, Canon};
use regex::Regex;
use rust_decimal::Decimal;
use scraper::{ElementRef, Html, Node, Selector};
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// The service as the business lists it (with its section heading when the line alone was
    /// too vague, e.g. "Highlights — Partial").
    pub label: String,
    pub description: String,
    pub price: Decimal,
    pub price_max: Option<Decimal>,
    /// "exact" | "starting_at" | "range"
    pub price_type: &'static str,
    pub duration_min: Option<i64>,
    pub stylist_level: String,
    /// "" | "short" | "medium" | "long"
    pub hair_length: String,
    /// The text the price was read from.
    pub raw: String,
    pub canon: Option<&'static Canon>,
}

#[derive(Debug, Default)]
pub struct PageScan {
    pub items: Vec<Item>,
    /// Services listed with "price on consultation" (no amount).
    pub consultation: Vec<String>,
    /// A shop/product page: prices there are retail, not services.
    pub retail_page: bool,
    /// Lines with an amount that were skipped (fees, retail, memberships, out of range, prose).
    pub skipped: usize,
}

const MIN_PRICE: i64 = 5;
const MAX_PRICE: i64 = 2000;

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("valid regex"))
}

macro_rules! regex {
    ($name:ident, $pat:expr) => {
        fn $name() -> &'static Regex {
            static R: OnceLock<Regex> = OnceLock::new();
            re(&R, $pat)
        }
    };
}

regex!(range_re, r"\$\s?(\d{1,4}(?:,\d{3})?(?:\.\d{1,2})?)\s*(?:-|–|—|to)\s*\$?\s?(\d{1,4}(?:,\d{3})?(?:\.\d{1,2})?)");
regex!(money_re, r"\$\s?(\d{1,4}(?:,\d{3})?(?:\.\d{1,2})?)");
regex!(plus_after_re, r"(?i)^\s*(?:\+|&\s*up\b|and\s+up\b|or\s+more\b)");
regex!(from_before_re, r"(?i)(?:\bfrom|\bstarting(?:\s+at)?|\bstarts\s+at|\bbeginning\s+at|\bas\s+low\s+as|\bminimum)\s*:?\s*$");
regex!(duration_after_re, r"(?i)^\s*(?:min|mins|minutes|hr|hrs|hour|hours)\b");
regex!(upto_before_re, r"(?i)(?:\bup\s+to|\bmax(?:imum)?\.?)\s*:?\s*$");
regex!(minutes_re, r"(?i)\b(\d{1,3})\s*(?:min|mins|minutes)\b\.?");
regex!(hours_re, r"(?i)\b(\d(?:\.\d+)?)\s*(?:h|hr|hrs|hour|hours)\b\.?(?:\s*(\d{1,2})\s*(?:m|min|mins|minutes)\b\.?)?");
regex!(
    noise_re,
    r"(?i)gift\s*(?:card|certificate)|\bdeposits?\s*(?::|required|is|of|due|needed|to\s+book|will|must|non)|(?:booking|non-?refundable|a)\s+deposit|cancel|no[\s-]show|late\s+(?:fee|arrival)|\bfees?\b|gratuit|discount|%\s*off|\boff\s+(?:your|any|first|all)|\bsave\b|coupon|promo|\bmembers?\b|membership|/\s*mo\b|/\s*month|per\s+month|a\s+month\b|monthly|annual|yearly|subscription|shipping|add\s+to\s+cart|sold\s+out|in\s+stock|\bqty\b|quantity|\bper\s+(?:foil|row|strand|inch|minute|hour|additional|\d)|each\s+additional|package\s+of|series\s+of|pack\s+of|\bbottle\b|\b\d+\s?(?:oz|ml|fl\.?\s?oz)\b|\bretail\b|\bproducts?\b|\bsubtotal\b|\btotal\b"
);
regex!(
    consult_re,
    r"(?i)(?:by|upon|on|with|after|requires?|needs?)\s+(?:a\s+)?consult(?:ation)?|consult(?:ation)?\s+(?:required|needed|only|for\s+pric)|pric(?:e|es|ed|ing)\s+(?:upon|on|by)\s+(?:request|consult)|call\s+(?:for|to)\s+(?:a\s+)?(?:pric|quote)|pric(?:e|ing)\s+varies|custom\s+quote"
);
regex!(
    tier_re,
    r"(?i)^(?:(?:new\s+talent|apprentice|associate|junior|jr\.?|senior|sr\.?|lead|master|director|creative\s+director|art\s+director|level\s*\d|lvl\.?\s*\d|tier\s*\d|artist|educator|expert|elite|principal|designer|emerging|advanced|stylist|owner|all\s+levels|any\s+level|all\s+stylists)\s*)+(?:stylist|artist|designer|level|colou?rist|barber)?s?$"
);
regex!(
    tier_prefix_re,
    r"(?i)^(?:new\s+talent|apprentice|associate|junior|jr\.?|senior|sr\.?|lead|master|director|level\s*\d|stylist|designer|artist|all\s+levels)(?:\s+(?:stylist|artist|designer))?\s*[|:–—-]"
);
regex!(length_re, r"(?i)^(short|medium|med\.?|long|extra[\s-]long|x-?long|shoulder(?:\s+length)?|chin(?:\s+length)?)(?:\s+hair)?$");
regex!(
    tier_suffix_re,
    r"(?i)^(.*?\S)[\s:|–—-]+((?:new\s+talent|apprentice|associate|junior|senior|lead|master|director|level\s*\d)(?:\s+(?:stylist|artist|designer))?|stylist|short|medium|long)$"
);
regex!(qualifier_re, r"^\s*\(([^()$]{1,24})\)");
regex!(retail_path_re, r"(?i)/(?:shop|store|product|products|cart|checkout|collections?|gift-?cards?)(?:/|$|\?|\.)");

/// One line of visible text.
#[derive(Debug, Clone, Default)]
pub struct Line {
    pub text: String,
    pub heading: bool,
    /// Table cells when the line is a table row.
    pub cells: Vec<String>,
    /// The table's header row, when the line is a body row.
    pub header: Option<Vec<String>>,
    /// Ids of the enclosing elements, outermost first (to find a price's own container).
    pub path: Vec<usize>,
}

const SKIP: &[&str] = &["script", "style", "noscript", "svg", "template", "iframe", "select", "option", "nav", "footer", "button", "del", "s", "strike", "head", "textarea", "form", "canvas"];
const BLOCK: &[&str] = &[
    "p", "div", "li", "ul", "ol", "dl", "dd", "section", "article", "header", "main", "aside", "blockquote", "figure", "figcaption", "address", "pre", "center", "details", "summary", "body", "html", "hr",
];

#[derive(Default)]
struct Builder {
    out: Vec<Line>,
    cur: String,
    cell: Option<String>,
    row: Option<(Vec<String>, bool)>,
    headers: Vec<Option<Vec<String>>>,
    stack: Vec<usize>,
    next_id: usize,
}

/// Section titles that aren't h1–h6 (menu builders use classes; tab panels use aria-label).
fn heading_like(el: &ElementRef) -> bool {
    let v = el.value();
    v.attr("role") == Some("heading") || v.attr("class").is_some_and(|c| ["section-title", "category-title", "menu-section-header", "menu-section-title", "service-category", "category-name", "section-heading"].iter().any(|k| c.contains(k)))
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Builder {
    fn text(&mut self, t: &str) {
        let target = self.cell.as_mut().unwrap_or(&mut self.cur);
        target.push(' ');
        target.push_str(t);
    }
    fn flush(&mut self, heading: bool) {
        let t = collapse(&std::mem::take(&mut self.cur));
        if !t.is_empty() && t.len() <= 600 {
            self.out.push(Line { text: t, heading, path: self.stack.clone(), ..Default::default() });
        }
    }
    fn walk(&mut self, el: ElementRef) {
        let name = el.value().name();
        if SKIP.contains(&name) {
            return;
        }
        self.next_id += 1;
        self.stack.push(self.next_id);
        self.walk_inner(el, name);
        self.stack.pop();
    }
    fn walk_inner(&mut self, el: ElementRef, name: &str) {
        // A tab panel's label names the section inside it ("MANICURE", "PEDICURE").
        if el.value().attr("role") == Some("tabpanel") {
            if let Some(l) = el.value().attr("aria-label").map(collapse).filter(|l| !l.is_empty() && l.len() <= 60) {
                self.flush(false);
                self.out.push(Line { text: l, heading: true, path: self.stack.clone(), ..Default::default() });
            }
        }
        let children = |b: &mut Builder| {
            for child in el.children() {
                if let Some(ce) = ElementRef::wrap(child) {
                    b.walk(ce);
                } else if let Node::Text(t) = child.value() {
                    b.text(t);
                }
            }
        };
        match name {
            "br" => self.flush(false),
            "table" => {
                self.flush(false);
                // A table nested in a cell (layout tables) gets its own rows; the outer row resumes after.
                let saved = (self.row.take(), self.cell.take());
                self.headers.push(None);
                children(self);
                self.headers.pop();
                (self.row, self.cell) = saved;
            }
            "tr" => {
                self.flush(false);
                self.row = Some((Vec::new(), false));
                children(self);
                let Some((cells, all_th)) = self.row.take() else { return };
                let cells: Vec<String> = cells.into_iter().map(|c| collapse(&c)).collect();
                if cells.iter().all(|c| c.is_empty()) {
                    return;
                }
                let has_money = cells.iter().any(|c| money_re().is_match(c));
                let header = self.headers.last().cloned().flatten();
                if (all_th || header.is_none()) && !has_money && cells.len() > 1 {
                    if let Some(h) = self.headers.last_mut() {
                        *h = Some(cells.clone());
                    }
                    self.out.push(Line { text: cells.join(" | "), heading: true, path: self.stack.clone(), ..Default::default() });
                } else {
                    self.out.push(Line { text: cells.join(" | "), heading: false, cells, header, path: self.stack.clone() });
                }
            }
            "td" | "th" => {
                if self.row.is_none() {
                    return children(self);
                }
                self.cell = Some(String::new());
                children(self);
                let c = self.cell.take().unwrap_or_default();
                if let Some((cells, all_th)) = self.row.as_mut() {
                    cells.push(c);
                    if name == "td" {
                        *all_th = false;
                    } else if cells.len() == 1 {
                        *all_th = true;
                    }
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "dt" => {
                self.flush(false);
                children(self);
                self.flush(true);
            }
            _ if heading_like(&el) => {
                self.flush(false);
                children(self);
                self.flush(true);
            }
            n if BLOCK.contains(&n) => {
                self.flush(false);
                children(self);
                self.flush(false);
            }
            _ => children(self),
        }
    }
}

/// Visible text as lines, skipping scripts, navigation, footers and struck-through prices.
pub fn lines(doc: &Html) -> Vec<Line> {
    let mut b = Builder::default();
    b.walk(doc.root_element());
    b.flush(false);
    b.out
}

fn dec(s: &str) -> Option<Decimal> {
    s.replace(',', "").parse().ok()
}

fn in_range(d: Decimal) -> bool {
    d >= Decimal::from(MIN_PRICE) && d <= Decimal::from(MAX_PRICE)
}

/// Trim separators, dot leaders, bullets and price-type words from the ends of a label.
fn clean_label(s: &str) -> String {
    let t = s.trim_matches(|c: char| c.is_whitespace() || "-–—:|.·•…/\\*()[],~_+=".contains(c));
    let t = from_before_re().replace(t, "");
    let t = t.trim_matches(|c: char| c.is_whitespace() || "-–—:|.·•…/\\*()[],~_+=".contains(c));
    let mut out: String = collapse(t).chars().take(120).collect();
    // Trimming can eat the ")" of "Gloss (add on)".
    if out.matches('(').count() > out.matches(')').count() {
        out.push(')');
    }
    out
}

fn duration(s: &str) -> Option<i64> {
    if let Some(c) = hours_re().captures(s) {
        let h: f64 = c[1].parse().ok()?;
        let m: i64 = c.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        return Some((h * 60.0).round() as i64 + m).filter(|m| (5..=600).contains(m));
    }
    minutes_re().captures(s).and_then(|c| c[1].parse().ok()).filter(|m| (5..=600).contains(m))
}

fn strip_duration(s: &str) -> String {
    let t = hours_re().replace_all(s, "");
    collapse(&minutes_re().replace_all(&t, ""))
}

fn length_of(s: &str) -> Option<&'static str> {
    let c = length_re().captures(s.trim())?;
    let w = c[1].to_lowercase();
    Some(if w.starts_with("short") || w.starts_with("chin") {
        "short"
    } else if w.starts_with("med") || w.starts_with("shoulder") {
        "medium"
    } else {
        "long"
    })
}

fn is_tier(s: &str) -> bool {
    tier_re().is_match(s.trim()) || length_of(s).is_some()
}

struct Token {
    start: usize,
    end: usize,
    low: Decimal,
    high: Option<Decimal>,
    kind: &'static str,
}

fn tokens(t: &str) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::new();
    for c in range_re().captures_iter(t) {
        let m = c.get(0).unwrap();
        // "$45 - 60 min" is a price and a duration, not a range.
        if duration_after_re().is_match(&t[m.end()..]) {
            continue;
        }
        if let (Some(lo), Some(hi)) = (dec(&c[1]), dec(&c[2])) {
            if hi > lo {
                out.push(Token { start: m.start(), end: m.end(), low: lo, high: Some(hi), kind: "range" });
            }
        }
    }
    for c in money_re().captures_iter(t) {
        let m = c.get(0).unwrap();
        if out.iter().any(|r| m.start() >= r.start && m.start() < r.end) {
            continue;
        }
        let Some(v) = dec(&c[1]) else { continue };
        let before = &t[..m.start()];
        if upto_before_re().is_match(before) {
            continue;
        }
        let after = &t[m.end()..];
        let plus = plus_after_re().find(after);
        let kind = if plus.is_some() || from_before_re().is_match(before) { "starting_at" } else { "exact" };
        out.push(Token { start: m.start(), end: m.end() + plus.map_or(0, |p| p.end()), low: v, high: None, kind });
    }
    out.sort_by_key(|t| t.start);
    out
}

/// A sentence rather than a service name.
fn is_prose(t: &str) -> bool {
    let words = t.split_whitespace().count();
    words > 10 || t.len() > 80 || (words >= 4 && (t.ends_with('.') || t.ends_with('!'))) || (words >= 5 && t.contains(", "))
}

fn shared_depth(a: &[usize], b: &[usize]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

/// The service name for a price on a line of its own, and its index: a short line without a
/// price, preferring the one in the price's own container (menu builders often print the price
/// before the name, inside the same item), else the nearest earlier one. Skips descriptions, tier
/// rows ("Senior Stylist | 274" with a missing "$") and noise.
fn name_before(lines: &[Line], i: usize) -> Option<(usize, &str)> {
    let ok = |j: usize| {
        let t = lines[j].text.as_str();
        !money_re().is_match(t) && !is_prose(t) && !noise_re().is_match(t) && !is_tier(t) && !tier_prefix_re().is_match(t) && t.chars().any(char::is_alphabetic)
    };
    let here = &lines[i].path;
    let before = (i.saturating_sub(8)..i).rev().filter(|j| ok(*j)).max_by_key(|j| (shared_depth(here, &lines[*j].path), *j));
    let after = (i + 1..(i + 4).min(lines.len())).take_while(|j| !money_re().is_match(&lines[*j].text)).filter(|j| ok(*j) && !lines[*j].heading).max_by_key(|j| (shared_depth(here, &lines[*j].path), usize::MAX - *j));
    let pick = match (before, after) {
        (Some(b), Some(a)) if shared_depth(here, &lines[a].path) > shared_depth(here, &lines[b].path) => Some(a),
        (Some(b), _) => Some(b),
        (None, a) => a,
    };
    pick.map(|j| (j, lines[j].text.as_str()))
}

/// A description: a sentence-like line (not just a duration) without a price.
fn describes(l: &Line) -> bool {
    !l.heading && !money_re().is_match(&l.text) && strip_duration(&l.text).split_whitespace().count() >= 4 && !consult_re().is_match(&l.text)
}

/// Is a text segment between two prices a continuation of the same service ("(style not
/// included) or", "add on with a style") rather than a new service name?
fn continues(raw: &str) -> bool {
    let r = raw.trim_start_matches(|c: char| c.is_whitespace() || "-–—|,:".contains(c));
    let lower = r.to_lowercase();
    r.starts_with('(') || r.chars().next().is_some_and(char::is_lowercase) || ["or ", "and ", "with ", "plus ", "add"].iter().any(|w| lower.starts_with(w))
}

/// "PARTIAL/FULL FOIL" with two prices → ["PARTIAL FOIL", "FULL FOIL"].
fn split_alternatives(label: &str, n: usize) -> Option<Vec<String>> {
    let parts: Vec<&str> = label.split('/').map(str::trim).collect();
    if parts.len() != n || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let last: Vec<&str> = parts[n - 1].split_whitespace().collect();
    let singles = parts[..n - 1].iter().all(|p| !p.contains(' '));
    Some(if singles && last.len() > 1 {
        let tail = last[1..].join(" ");
        parts[..n - 1].iter().map(|p| format!("{p} {tail}")).chain([parts[n - 1].to_string()]).collect()
    } else {
        parts.iter().map(|p| p.to_string()).collect()
    })
}

/// Scan one page. `url` is used only to recognise shop pages.
pub fn scan(html: &str, url: &str) -> PageScan {
    let mut scan = PageScan::default();
    if retail_path_re().is_match(reqwest::Url::parse(url).map(|u| u.path().to_string()).unwrap_or_default().as_str()) {
        scan.retail_page = true;
        return scan;
    }
    let doc = Html::parse_document(html);
    if json_ld(&doc, &mut scan.items) || doc.select(&sel(r#"meta[property="og:type"][content="product"]"#)).next().is_some() {
        scan.retail_page = true;
        scan.items.clear();
        return scan;
    }
    let lines = lines(&doc);
    let mut heading: Option<&str> = None;
    for (i, line) in lines.iter().enumerate() {
        let t = line.text.as_str();
        let toks = tokens(t);
        if line.heading && toks.is_empty() {
            heading = Some(t);
            continue;
        }
        if toks.is_empty() {
            if t.len() > 120 {
                continue;
            }
            if let Some(m) = consult_re().find(t) {
                let own = clean_label(&t[..m.start()]);
                let label = if own.chars().any(char::is_alphabetic) { own } else { name_before(&lines, i).map(|(_, n)| clean_label(n)).unwrap_or_default() };
                if !label.is_empty() && label.split_whitespace().count() <= 8 && classify_in(&label, heading).is_some() && !scan.consultation.contains(&label) {
                    scan.consultation.push(label);
                }
            }
            continue;
        }
        if noise_re().is_match(t) || t.len() > 220 {
            scan.skipped += 1;
            continue;
        }
        let mut raw_segs: Vec<&str> = toks.iter().enumerate().map(|(k, tok)| &t[if k == 0 { 0 } else { toks[k - 1].end }..tok.start]).collect();
        // "Arms $35 (Half) / $50 (Full)": a bracketed word right after a price qualifies it.
        let mut qualifiers: Vec<String> = Vec::new();
        for k in 0..toks.len() {
            let after = if k + 1 < toks.len() { raw_segs[k + 1] } else { &t[toks[k].end..] };
            match qualifier_re().captures(after) {
                Some(c) => {
                    qualifiers.push(collapse(&c[1]));
                    if k + 1 < toks.len() {
                        raw_segs[k + 1] = &raw_segs[k + 1][c.get(0).unwrap().end()..];
                    }
                }
                None => qualifiers.push(String::new()),
            }
        }
        let qualified = toks.len() > 1 && qualifiers.iter().all(|q| !q.is_empty());
        let segs: Vec<String> = raw_segs.iter().map(|r| clean_label(&strip_duration(r))).collect();
        let trailing = clean_label(&strip_duration(&t[toks.last().unwrap().end..]));
        // (label, tier) per price
        let mut parts: Vec<(String, String)> = Vec::new();
        let n = toks.len();
        if n == 1 {
            let own = if segs[0].chars().any(char::is_alphabetic) { segs[0].clone() } else { trailing.clone() };
            parts.push((own, String::new()));
        } else if let (Some(h), true) = (&line.header, line.cells.len() > 1) {
            // Table: the first cell names the service, the header names each price column.
            let name = line.cells[0].clone();
            for tok in &toks {
                let mut pos = 0;
                let col = line.cells.iter().position(|c| {
                    let r = (pos..pos + c.len() + 3).contains(&tok.start);
                    pos += c.len() + 3;
                    r
                });
                // A tier or hair-length column ("Senior", "Long") is a tier; anything else
                // ("Cut & Color") is part of the service name.
                match col.and_then(|j| h.get(j)).map(|c| clean_label(c)).filter(|c| !c.is_empty()) {
                    Some(c) if is_tier(&c) => parts.push((name.clone(), c)),
                    Some(c) => parts.push((format!("{name} {c}"), String::new())),
                    None => parts.push((name.clone(), String::new())),
                }
            }
        } else if segs[1..].iter().all(|s| is_tier(s)) && segs[1..].iter().any(|s| !s.is_empty()) {
            // "Haircut: Junior $45 Senior $55 Master $65"
            let (name, first_tier) = match tier_suffix_re().captures(&segs[0]) {
                Some(c) => (c[1].to_string(), c[2].to_string()),
                None if is_tier(&segs[0]) => (String::new(), segs[0].clone()),
                None => (segs[0].clone(), String::new()),
            };
            parts.push((name.clone(), first_tier));
            for s in &segs[1..] {
                parts.push((name.clone(), s.clone()));
            }
        } else if raw_segs[1..].iter().zip(&segs[1..]).all(|(r, s)| s.is_empty() || continues(r)) {
            match split_alternatives(&segs[0], n) {
                _ if qualified => parts.extend(qualifiers.iter().map(|q| (format!("{} ({q})", segs[0]), String::new()))),
                // "PARTIAL/FULL FOIL $155/$200+"
                Some(labels) => parts.extend(labels.into_iter().map(|l| (l, String::new()))),
                // "Men's cut $35 / $45": several prices with nothing saying which is which.
                None => parts.extend((0..n).map(|k| (segs[0].clone(), format!("Price {} of {n} listed", k + 1)))),
            }
        } else {
            // "Haircut $45 Blowout $35": separate services on one line.
            parts.extend(segs.iter().map(|s| (s.clone(), String::new())));
        }
        for (tok, (label, tier)) in toks.iter().zip(parts) {
            if !in_range(tok.low) || tok.high.is_some_and(|h| h > Decimal::from(3000)) {
                scan.skipped += 1;
                continue;
            }
            let (mut label, mut tier, mut raw) = (label, tier, t.chars().take(300).collect::<String>());
            if is_tier(&label) && tier.is_empty() {
                tier = std::mem::take(&mut label);
            }
            let mut description = String::new();
            if !label.chars().any(char::is_alphabetic) {
                let Some((j, name)) = name_before(&lines, i) else {
                    scan.skipped += 1;
                    continue;
                };
                raw = format!("{name} … {raw}").chars().take(300).collect();
                label = clean_label(name);
                // Menus often put the description between the name and the price.
                let between = if j < i { &lines[j + 1..i] } else { &lines[i + 1..=j] };
                if let Some(d) = between.iter().chain(lines.get(j + 1)).find(|l| describes(l)) {
                    description = d.text.chars().take(300).collect();
                }
            }
            if description.is_empty() && n == 1 {
                if let Some(l) = lines.get(i + 1).filter(|l| describes(l)) {
                    description = l.text.chars().take(300).collect();
                }
            }
            let duration_min = duration(&label).or_else(|| duration(t)).or_else(|| lines.get(i + 1).and_then(|l| duration(&l.text).filter(|_| strip_duration(&l.text).split_whitespace().count() <= 1))).or_else(|| duration(&description));
            let label = clean_label(&strip_duration(&label));
            if label.len() < 2 {
                scan.skipped += 1;
                continue;
            }
            let ambiguous = tier.starts_with("Price ");
            let hair_length = length_of(&tier).unwrap_or("").to_string();
            let stylist_level = if hair_length.is_empty() && !ambiguous { tier.clone() } else { String::new() };
            if ambiguous {
                description = tier.clone();
            }
            let canon = classify_in(&label, heading);
            // Name the section when it, not the label, says what the service is.
            let label = match (catalog::classify(&label), canon, heading) {
                (own, Some(c), Some(h)) if own.is_none_or(|o| o.key != c.key) => format!("{} — {label}", clean_label(h)),
                _ => label,
            };
            scan.items.push(Item { label, description, price: tok.low, price_max: tok.high, price_type: tok.kind, duration_min, stylist_level, hair_length, raw, canon });
        }
    }
    dedupe(&mut scan.items);
    scan
}

/// Classify a label in its menu section ("Partial" under "Highlights").
fn classify_in(label: &str, heading: Option<&str>) -> Option<&'static Canon> {
    catalog::classify_in_section(label, heading)
}

fn dedupe(items: &mut Vec<Item>) {
    let mut seen = std::collections::HashSet::new();
    items.retain(|i| seen.insert((catalog::normalize(&i.label), i.price, i.price_max, i.stylist_level.clone(), i.hair_length.clone())));
}

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("valid selector")
}

fn jstr(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.trim().to_string()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn has_type(v: &serde_json::Value, t: &str) -> bool {
    match &v["@type"] {
        serde_json::Value::String(s) => s == t,
        serde_json::Value::Array(a) => a.iter().any(|x| x.as_str() == Some(t)),
        _ => false,
    }
}

/// schema.org offers in JSON-LD. Returns true when the page describes retail products.
fn json_ld(doc: &Html, out: &mut Vec<Item>) -> bool {
    let mut product = false;
    for s in doc.select(&sel(r#"script[type="application/ld+json"]"#)) {
        let text: String = s.text().collect();
        if text.len() > 500_000 {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            walk_ld(&v, None, out, &mut product, 0);
        }
    }
    product && out.is_empty()
}

fn walk_ld(v: &serde_json::Value, ctx: Option<&str>, out: &mut Vec<Item>, product: &mut bool, depth: usize) {
    if depth > 12 {
        return;
    }
    match v {
        serde_json::Value::Array(a) => a.iter().for_each(|x| walk_ld(x, ctx, out, product, depth + 1)),
        serde_json::Value::Object(o) => {
            if has_type(v, "Product") {
                *product = true;
                return;
            }
            let name = o.get("name").and_then(|n| n.as_str()).map(str::trim).filter(|n| !n.is_empty());
            if has_type(v, "Offer") || has_type(v, "AggregateOffer") {
                let usd = o.get("priceCurrency").and_then(|c| c.as_str()).is_none_or(|c| c.eq_ignore_ascii_case("USD"));
                let label = v["itemOffered"]["name"].as_str().or(name).or(ctx).map(clean_label).unwrap_or_default();
                let spec = &v["priceSpecification"];
                let low = jstr(&v["price"]).or_else(|| jstr(&v["lowPrice"])).or_else(|| jstr(&spec["price"])).or_else(|| jstr(&spec["minPrice"])).and_then(|s| dec(&s));
                let high = jstr(&v["highPrice"]).or_else(|| jstr(&spec["maxPrice"])).and_then(|s| dec(&s));
                if let (true, Some(lo), false) = (usd, low, label.is_empty()) {
                    let high = high.filter(|h| *h > lo && *h <= Decimal::from(3000));
                    let starting = high.is_none() && jstr(&spec["minPrice"]).is_some() && jstr(&spec["price"]).is_none() && jstr(&v["price"]).is_none();
                    if in_range(lo) {
                        out.push(Item {
                            canon: catalog::classify(&label),
                            raw: format!("Structured data: {label} ${lo}{}", high.map(|h| format!("–${h}")).unwrap_or_default()),
                            label,
                            description: v["itemOffered"]["description"].as_str().or(o.get("description").and_then(|d| d.as_str())).unwrap_or("").chars().take(300).collect(),
                            price: lo,
                            price_max: high,
                            price_type: if high.is_some() { "range" } else if starting { "starting_at" } else { "exact" },
                            duration_min: None,
                            stylist_level: String::new(),
                            hair_length: String::new(),
                        });
                    }
                }
            }
            for (k, child) in o {
                if k != "@context" {
                    walk_ld(child, name.or(ctx), out, product, depth + 1);
                }
            }
        }
        _ => {}
    }
}

/// Links on a page: (absolute URL, link text).
pub fn links(html: &str, base: &reqwest::Url) -> Vec<(reqwest::Url, String)> {
    let doc = Html::parse_document(html);
    let mut out: Vec<(reqwest::Url, String)> = Vec::new();
    for a in doc.select(&sel("a[href], link[rel=next][href]")) {
        let Some(href) = a.value().attr("href") else { continue };
        let Ok(mut u) = base.join(href.trim()) else { continue };
        if !matches!(u.scheme(), "http" | "https") {
            continue;
        }
        u.set_fragment(None);
        let text = collapse(&a.text().collect::<String>()).chars().take(100).collect::<String>();
        let text = if a.value().attr("rel").is_some_and(|r| r.contains("next")) { format!("{text} next page") } else { text };
        if !out.iter().any(|(x, _)| *x == u) {
            out.push((u, text));
        }
    }
    out
}

/// What a business says about itself on its homepage (schema.org LocalBusiness and kin, else the
/// page title).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SiteMeta {
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub phone: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

pub fn site_meta(html: &str) -> SiteMeta {
    let doc = Html::parse_document(html);
    let mut m = SiteMeta::default();
    let business = ["LocalBusiness", "HairSalon", "BeautySalon", "NailSalon", "DaySpa", "HealthAndBeautyBusiness", "Organization", "BarberShop"];
    fn find<'a>(v: &'a serde_json::Value, types: &[&str], depth: usize) -> Option<&'a serde_json::Value> {
        if depth > 8 {
            return None;
        }
        match v {
            serde_json::Value::Array(a) => a.iter().find_map(|x| find(x, types, depth + 1)),
            serde_json::Value::Object(o) => {
                if types.iter().any(|t| has_type(v, t)) {
                    return Some(v);
                }
                o.values().find_map(|x| find(x, types, depth + 1))
            }
            _ => None,
        }
    }
    for s in doc.select(&sel(r#"script[type="application/ld+json"]"#)) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&s.text().collect::<String>()) else { continue };
        // Prefer the most specific business type present.
        let Some(b) = business.iter().find_map(|t| find(&v, &[t], 0)) else { continue };
        let a = &b["address"];
        let g = |x: &serde_json::Value| x.as_str().unwrap_or("").trim().to_string();
        m.name = g(&b["name"]);
        m.address = g(&a["streetAddress"]);
        m.city = g(&a["addressLocality"]);
        m.state = g(&a["addressRegion"]);
        m.postal_code = g(&a["postalCode"]);
        m.phone = g(&b["telephone"]);
        m.latitude = jstr(&b["geo"]["latitude"]).and_then(|s| s.parse().ok());
        m.longitude = jstr(&b["geo"]["longitude"]).and_then(|s| s.parse().ok());
        if !m.name.is_empty() {
            break;
        }
    }
    if m.name.is_empty() {
        m.name = doc
            .select(&sel(r#"meta[property="og:site_name"]"#))
            .next()
            .and_then(|e| e.value().attr("content"))
            .map(String::from)
            .or_else(|| doc.select(&sel("title")).next().map(|t| t.text().collect::<String>()))
            .map(|t| collapse(t.split(['|', '–', '—']).next().unwrap_or("")))
            .unwrap_or_default();
    }
    m.name = m.name.chars().take(120).collect();
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn page(body: &str) -> String {
        format!("<!doctype html><html><head><title>T</title><script>var p = '$99';</script></head><body><nav><a href='/'>Home $1</a></nav>{body}<footer>Gift cards $25 · © 2026</footer></body></html>")
    }
    fn one(s: &PageScan, label: &str) -> Item {
        s.items.iter().find(|i| i.label == label).unwrap_or_else(|| panic!("no {label:?} in {:#?}", s.items)).clone()
    }

    /// Wix-style menu (seen on a Seattle salon's site): name and "$55+" on one line, upper case.
    #[test]
    fn starting_at_prices_on_one_line() {
        let s = scan(&page("<div><p><span>BLOWOUT $55+</span></p><p><span>WOMENS HAIRCUT $75+</span></p><p><span>PARTIAL BALAYAGE $155+</span></p><p>Hair & scalp facial from $40</p></div>"), "https://x.example/services");
        assert_eq!(s.items.len(), 4, "{:#?}", s.items);
        let b = one(&s, "PARTIAL BALAYAGE");
        assert_eq!((b.price, b.price_type, b.canon.unwrap().key), (dec!(155), "starting_at", "balayage"));
        assert_eq!(one(&s, "WOMENS HAIRCUT").canon.unwrap().key, "womens_cut");
        assert_eq!(one(&s, "Hair & scalp facial").price_type, "starting_at");
    }

    /// "Name - $110 - $115" style list (seen on a Seattle salon's site).
    #[test]
    fn ranges_and_dash_separators() {
        let s = scan(&page("<ul><li>Signature Blowout - $80</li><li>Haircut - $110 - $115</li><li>Short / Clipper haircut - $80</li><li>Root touch up $65–85</li></ul>"), "https://x.example/");
        let h = one(&s, "Haircut");
        assert_eq!((h.price, h.price_max, h.price_type), (dec!(110), Some(dec!(115)), "range"));
        assert_eq!(one(&s, "Root touch up").price_max, Some(dec!(85)));
        assert_eq!(one(&s, "Signature Blowout").price_type, "exact");
        assert_eq!(s.items.len(), 4);
    }

    /// Stylist tiers on separate lines under a heading (seen on a Seattle salon's site): each tier
    /// is its own price, named after the heading, never merged into a range.
    #[test]
    fn stylist_tiers_under_a_heading() {
        let s = scan(
            &page("<h3>Full Highlight</h3><p>Associate Stylist | $155</p><p>Stylist | $173</p><p>Senior Stylist | $206</p><h3>Women's Haircut</h3><p>Associate Stylist | $65</p><p>Master Stylist | $95</p>"),
            "https://x.example/pricing",
        );
        let hl: Vec<_> = s.items.iter().filter(|i| i.label == "Full Highlight").collect();
        assert_eq!(hl.iter().map(|i| (i.stylist_level.as_str(), i.price)).collect::<Vec<_>>(), [("Associate Stylist", dec!(155)), ("Stylist", dec!(173)), ("Senior Stylist", dec!(206))]);
        assert!(hl.iter().all(|i| i.price_type == "exact" && i.canon.unwrap().key == "full_highlights"));
        assert_eq!(s.items.iter().filter(|i| i.label == "Women's Haircut").count(), 2);
    }

    #[test]
    fn tiers_on_one_line_and_in_table_headers() {
        let s = scan(
            &page(
                "<p>Haircut: Junior $45 Senior $55 Master $65</p>
                 <table><tr><th>Service</th><th>Short</th><th>Long</th></tr><tr><td>Single process color</td><td>$90</td><td>$120</td></tr></table>
                 <p>Men's cut $35 / $45</p>",
            ),
            "https://x.example/",
        );
        let cuts: Vec<_> = s.items.iter().filter(|i| i.label == "Haircut").map(|i| (i.stylist_level.as_str(), i.price)).collect();
        assert_eq!(cuts, [("Junior", dec!(45)), ("Senior", dec!(55)), ("Master", dec!(65))]);
        let color: Vec<_> = s.items.iter().filter(|i| i.label == "Single process color").map(|i| (i.hair_length.as_str(), i.price)).collect();
        assert_eq!(color, [("short", dec!(90)), ("long", dec!(120))]);
        let men: Vec<_> = s.items.iter().filter(|i| i.label == "Men's cut").collect();
        assert_eq!(men.len(), 2);
        assert!(men.iter().all(|i| i.price_type == "exact" && i.stylist_level.is_empty() && i.description.starts_with("Price ")));
    }

    /// Square Online / Squarespace menus: name, price and description in separate elements.
    #[test]
    fn name_and_price_in_separate_elements() {
        let s = scan(
            &page("<div class='item'><div class='title'>Gel Manicure</div><div class='price'>$45.00</div><div class='desc'>Includes nail shaping, cuticle care and long-lasting gel polish. 45 min</div></div><div class='item'><div class='title'>Classic Pedicure</div><div class='price'>$55.00</div></div>"),
            "https://x.square.site/s/services",
        );
        let g = one(&s, "Gel Manicure");
        assert_eq!((g.price, g.duration_min, g.canon.unwrap().key), (dec!(45.00), Some(45), "gel_manicure"));
        assert!(g.description.starts_with("Includes nail shaping"));
        assert!(g.raw.contains("Gel Manicure") && g.raw.contains("$45.00"));
        assert_eq!(one(&s, "Classic Pedicure").price, dec!(55));
    }

    #[test]
    fn durations_are_not_prices_or_minimums() {
        let s = scan(&page("<p>Gel Manicure (45 min) $40</p><p>Haircut 30 min $40</p><p>Facial $85 - 60 min</p><table><tr><th>Service</th><th>Cut</th><th>Cut &amp; Color</th></tr><tr><td>Women</td><td>$50</td><td>$140</td></tr></table>"), "https://x.example/");
        let g = one(&s, "Gel Manicure");
        assert_eq!((g.price_type, g.duration_min), ("exact", Some(45)));
        assert_eq!(one(&s, "Haircut").price_type, "exact");
        let f = one(&s, "Facial");
        assert_eq!((f.price, f.price_max, f.duration_min), (dec!(85), None, Some(60)));
        assert_eq!(one(&s, "Women Cut & Color").canon.unwrap().key, "cut_and_color");
        assert_eq!(one(&s, "Women Cut").canon.unwrap().key, "womens_cut");
    }

    /// Layouts from real pages: Square booking (name / description / price / "· 1 hour" lines),
    /// "PARTIAL/FULL FOIL $155/$200+", a conditioning price with an add-on alternative, a tier row
    /// whose "$" is missing, and a "Deposit Color" service (a technique, not a deposit).
    #[test]
    fn real_world_layouts() {
        let s = scan(
            &page(
                "<div><div>Dermaplaning Facial</div><div>True dermaplaning is not a shave at all! It uses a scalpel to gently scrape off dead skin.</div><div>$150</div><div>· 1 hour 15 minutes</div><div>Book now</div></div>
                 <p>PARTIAL/FULL FOIL $155/$200+</p><p>GLOSS (add on) $55+</p>
                 <p>Conditioning Treatment - $65 (style not included) or $45 add on with a style</p>
                 <h3>Partial Balayage</h3><p>Associate Stylist | $206</p><p>Senior Stylist | 274</p><p>Master Stylist | $296</p>
                 <p>Deposit Color/ Retouch</p><p>Associate Stylist | $155</p><p>Bang Trim</p><p>All Levels | $12</p>",
            ),
            "https://square.site/book/X/salon",
        );
        let d = one(&s, "Dermaplaning Facial");
        assert_eq!((d.price, d.duration_min), (dec!(150), Some(75)));
        assert!(d.description.starts_with("True dermaplaning"));
        assert_eq!(one(&s, "PARTIAL FOIL").canon.unwrap().key, "partial_highlights");
        assert_eq!(one(&s, "FULL FOIL").price, dec!(200));
        assert!(s.items.iter().any(|i| i.label == "GLOSS (add on)"));
        let ct: Vec<_> = s.items.iter().filter(|i| i.label == "Conditioning Treatment").map(|i| i.price).collect();
        assert_eq!(ct, [dec!(65), dec!(45)]);
        let bal: Vec<_> = s.items.iter().filter(|i| i.label == "Partial Balayage").map(|i| (i.stylist_level.as_str(), i.price)).collect();
        assert_eq!(bal, [("Associate Stylist", dec!(206)), ("Master Stylist", dec!(296))]); // the "$"-less 274 is not guessed
        assert_eq!(one(&s, "Deposit Color/ Retouch").canon.unwrap().key, "root_touchup");
        assert_eq!(one(&s, "Bang Trim").stylist_level, "All Levels");
        let q = scan(&page("<h3>Waxing</h3><p>Legs $40 (Half) / $70 (Full)</p>"), "https://x.example/");
        assert_eq!(q.items.iter().map(|i| (i.label.as_str(), i.price)).collect::<Vec<_>>(), [("Waxing — Legs (Half)", dec!(40)), ("Waxing — Legs (Full)", dec!(70))]);
    }

    /// Squarespace menu block (seen on a Seattle nail salon's site): tab panels name the sections,
    /// and each item prints its price before and after its title. Prices must stay with their own
    /// item, never the previous one's title.
    #[test]
    fn menu_blocks_keep_prices_with_their_own_item() {
        let item = |price: u32, title: &str, desc: &str| {
            format!("<div class='menu-item'><span class='menu-item-price-top'><span>$</span>{price}</span><div class='menu-item-title'>{title}</div><div class='menu-item-description'>{desc}</div><div class='menu-item-price-bottom'><span>$</span>{price}</div></div>")
        };
        let html = format!(
            "<div class='menu-selector'><label role='tab'>MANICURE</label> <label role='tab'>PEDICURE</label></div>
             <div role='tabpanel' aria-label='MANICURE'><div class='menu-items'>{}{}</div></div>
             <div role='tabpanel' aria-label='PEDICURE'><div class='menu-items'>{}</div></div>",
            item(28, "Classic", "We will shape your nails and care for your cuticles, then polish."),
            item(45, "Gel (Shellac)", ""),
            item(45, "Classic", "Soak, scrub, shape and polish for tired feet.")
        );
        let s = scan(&page(&html), "https://nails.example/services");
        let got: Vec<_> = s.items.iter().map(|i| (i.label.as_str(), i.price, i.canon.map(|c| c.key))).collect();
        assert_eq!(got, [("MANICURE — Classic", dec!(28), Some("manicure")), ("Gel (Shellac)", dec!(45), Some("gel_manicure")), ("PEDICURE — Classic", dec!(45), Some("pedicure"))]);
        assert!(s.items[0].description.starts_with("We will shape"));
    }

    #[test]
    fn json_ld_offers() {
        let ld = r#"{"@context":"https://schema.org","@type":"HairSalon","name":"Glow","hasOfferCatalog":{"@type":"OfferCatalog","itemListElement":[
            {"@type":"Offer","itemOffered":{"@type":"Service","name":"Balayage"},"priceSpecification":{"@type":"PriceSpecification","minPrice":180,"maxPrice":260,"priceCurrency":"USD"}},
            {"@type":"Offer","itemOffered":{"@type":"Service","name":"Gloss"},"price":"45.00","priceCurrency":"USD"},
            {"@type":"Offer","itemOffered":{"@type":"Service","name":"Cut"},"price":"40","priceCurrency":"CAD"}]}}"#;
        let s = scan(&page(&format!("<script type='application/ld+json'>{ld}</script><p>Gloss $45.00</p>")), "https://x.example/");
        let b = one(&s, "Balayage");
        assert_eq!((b.price, b.price_max, b.price_type), (dec!(180), Some(dec!(260)), "range"));
        assert_eq!(s.items.iter().filter(|i| i.label == "Gloss").count(), 1); // JSON-LD and text agree: once
        assert!(!s.items.iter().any(|i| i.label == "Cut")); // not US dollars
    }

    /// Noise seen on real salon sites: shipping thresholds, memberships, $0.00 placeholders, fees.
    #[test]
    fn noise_is_skipped_never_priced() {
        let s = scan(
            &page(
                "<p>Luxury Haircare Free Shipping over $74+</p><p>$0.00 USD</p><p>Unlimited cuts membership $23.99/month</p>
                 <p>Cancellation fee $25</p><p>Deposit required: $50</p><p>20% off your first visit! Save $10</p>
                 <p>Olaplex No.3 8 oz $30</p><p>Facial <s>$120</s> $95</p><p>Color correction — priced upon consultation</p>
                 <p>Extensions</p><p>Consultation required</p><p>Up to $300</p><p>Parking $6</p>",
            ),
            "https://x.example/",
        );
        assert_eq!(s.items.iter().map(|i| (i.label.as_str(), i.price)).collect::<Vec<_>>(), [("Facial", dec!(95)), ("Parking", dec!(6))]);
        assert_eq!(s.consultation, ["Color correction", "Extensions"]);
        assert!(s.skipped >= 6, "{}", s.skipped);
        // "Parking" is kept by the page scan but has no service match; the caller drops it.
        assert!(one(&s, "Parking").canon.is_none());
    }

    #[test]
    fn shop_pages_and_products_are_retail() {
        assert!(scan(&page("<p>Shampoo $28</p>"), "https://x.example/shop/shampoo").retail_page);
        let ld = r#"<script type="application/ld+json">{"@type":"Product","name":"Hair oil","offers":{"@type":"Offer","price":"30"}}</script>"#;
        let s = scan(&page(&format!("{ld}<p>Haircut $50</p>")), "https://x.example/");
        assert!(s.retail_page && s.items.is_empty());
    }

    #[test]
    fn headings_give_context_to_vague_labels() {
        let s = scan(&page("<h2>Highlights</h2><p>Partial $120</p><p>Full $160</p><h2>Nails</h2><p>Polish change $15</p>"), "https://x.example/");
        let p = one(&s, "Highlights — Partial");
        assert_eq!(p.canon.unwrap().key, "partial_highlights");
        assert_eq!(one(&s, "Highlights — Full").canon.unwrap().key, "full_highlights");
        assert_eq!(one(&s, "Polish change").canon.unwrap().key, "polish_change");
    }

    #[test]
    fn links_and_site_meta() {
        let html = r#"<html><head><title>Glow Salon | Olympia hair</title><script type="application/ld+json">{"@context":"https://schema.org","@type":"HairSalon","name":"Glow Salon","telephone":"(360) 555-0100","address":{"@type":"PostalAddress","streetAddress":"1 Main St","addressLocality":"Olympia","addressRegion":"WA","postalCode":"98501"},"geo":{"@type":"GeoCoordinates","latitude":47.04,"longitude":"-122.9"}}</script></head>
            <body><a href="/services#top">Services &amp; Prices</a><a href="mailto:x@y.z">Mail</a><a href="https://other.example/">Out</a><link rel="next" href="/services?page=2"></body></html>"#;
        let base = reqwest::Url::parse("https://glow.example/").unwrap();
        let l = links(html, &base);
        assert_eq!(l[0].0.as_str(), "https://glow.example/services");
        assert_eq!(l[0].1, "Services & Prices");
        assert!(l.iter().any(|(u, t)| u.as_str().ends_with("?page=2") && t.contains("next")));
        assert!(!l.iter().any(|(u, _)| u.scheme() == "mailto"));
        let m = site_meta(html);
        assert_eq!((m.name.as_str(), m.city.as_str(), m.postal_code.as_str(), m.latitude, m.longitude), ("Glow Salon", "Olympia", "98501", Some(47.04), Some(-122.9)));
        assert_eq!(site_meta("<title>Bliss Nails | Lacey WA</title>").name, "Bliss Nails");
    }
}

#[cfg(test)]
mod dev_pages {
    /// Developer aid: print what each saved page yields.
    /// `SRM_HTML_DIR=/path/with/pN.html+pN.url cargo test dev_scan_saved_pages -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn dev_scan_saved_pages() {
        let dir = std::env::var("SRM_HTML_DIR").expect("SRM_HTML_DIR");
        let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "html")).collect();
        files.sort();
        for f in files {
            let url = std::fs::read_to_string(f.with_extension("url")).unwrap_or_default();
            let s = super::scan(&std::fs::read_to_string(&f).unwrap(), url.trim());
            println!("\n=== {} ({} items, {} skipped, consult {:?}, retail {})", url.trim(), s.items.len(), s.skipped, s.consultation, s.retail_page);
            for i in &s.items {
                println!("  {:<45} {:>8} {:<11} {:<14} lvl={:<18} len={:<6} dur={:?} | {}", i.label, format!("${}{}", i.price, i.price_max.map(|m| format!("-{m}")).unwrap_or_default()), i.price_type, i.canon.map(|c| c.key).unwrap_or("-"), i.stylist_level, i.hair_length, i.duration_min, i.raw.chars().take(70).collect::<String>());
            }
        }
    }
}
