//! Canonical salon services, so "WOMENS HAIRCUT", "Ladies' Cut & Style" and "Women's cut" from
//! different menus compare as one service.
//!
//! Classification: labels and patterns are normalised the same way (lowercase, "&"/"+" → "and",
//! punctuation dropped, simple plurals singular). Among the services whose specific patterns
//! appear as whole words, the winner is the one with the higher priority (main services over
//! add-ons like a blow-dry), then the longest pattern ("brazilian blowout" beats "brazilian").
//! Generic fallback words ("cut", "color", "treatment") count only when nothing specific matched.

use std::sync::OnceLock;

#[derive(Debug, PartialEq)]
pub struct Canon {
    pub key: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    /// 2 = main service, 1 = add-on or finishing service.
    priority: u8,
    patterns: &'static [&'static str],
    fallback: &'static [&'static str],
}

/// Service categories, in display order.
pub const CATEGORIES: &[(&str, &str)] = &[
    ("cut_style", "Haircuts and styling"),
    ("color", "Color and highlights"),
    ("specialty_color", "Balayage and specialty color"),
    ("treatment", "Hair treatments"),
    ("extensions", "Extensions"),
    ("barber", "Barbering"),
    ("nails", "Nails"),
    ("skin", "Facials and skin care"),
    ("brows_lashes", "Brows and lashes"),
    ("waxing", "Waxing"),
    ("makeup", "Makeup"),
    ("massage", "Massage"),
];

macro_rules! c {
    ($key:literal, $name:literal, $cat:literal, $prio:literal, [$($p:literal),*], [$($f:literal),*]) => {
        Canon { key: $key, name: $name, category: $cat, priority: $prio, patterns: &[$($p),*], fallback: &[$($f),*] }
    };
}

pub const CATALOG: &[Canon] = &[
    // Haircuts and styling
    c!("womens_cut", "Women's haircut", "cut_style", 2, ["womens haircut", "womens cut", "womens hair cut", "ladies cut", "ladies haircut", "women haircut", "female haircut"], []),
    c!("mens_cut", "Men's haircut", "cut_style", 2, ["mens haircut", "mens cut", "mens hair cut", "gents cut", "gentlemens cut", "male haircut"], []),
    c!("kids_cut", "Kids' haircut", "cut_style", 2, ["kids cut", "kids haircut", "kid cut", "childrens cut", "childrens haircut", "child cut", "child haircut", "boys cut", "girls cut", "youth cut", "toddler cut"], ["kids", "children"]),
    c!("haircut", "Haircut", "cut_style", 2, ["haircut", "hair cut", "dry cut", "precision cut", "pixie cut", "aircut", "specialty cut"], ["cut", "trim", "restyle", "cut and style", "cut and blow dry", "wash and cut"]),
    c!("bang_trim", "Bang trim", "cut_style", 1, ["bang trim", "fringe trim", "bang cut", "fringe cut", "neck trim", "bang and neck trim"], ["bang", "fringe"]),
    c!("blowout", "Blowout / style", "cut_style", 1, ["blowout", "blow out", "blow dry", "blowdry", "silk press", "wash and set", "shampoo and style", "shampoo and blow dry", "wash and style", "curling iron", "flat iron"], ["style", "styling", "press"]),
    c!("updo", "Updo / formal style", "cut_style", 2, ["updo", "up do", "formal style", "special occasion style", "special occasion hair", "bridal hair", "event hair"], []),
    c!("braids", "Braids / locs", "cut_style", 2, ["braid", "cornrow", "loc retwist", "retwist", "locs", "twist", "box braid", "knotless"], []),
    c!("perm", "Perm / body wave", "cut_style", 2, ["perm", "permanent wave", "body wave", "spiral perm"], []),
    // Color
    c!("cut_and_color", "Cut and color", "color", 2, ["cut and color", "color and cut", "haircut and color", "color and haircut", "cut and colour"], []),
    c!("root_touchup", "Root touch-up", "color", 2, ["root touch", "root touch up", "root retouch", "root color", "root colour", "retouch", "regrowth", "touch up color"], ["root"]),
    c!("full_highlights", "Full highlights", "color", 2, ["full highlight", "full foil", "full head highlight", "full head foil", "full babylight", "full dimensional"], []),
    c!("partial_highlights", "Partial highlights", "color", 2, ["partial highlight", "partial foil", "half head highlight", "half head foil", "half highlight", "mini highlight", "face frame highlight", "face framing highlight", "partial babylight", "half foil", "partial dimensional"], []),
    c!("highlights", "Highlights", "color", 2, ["highlight", "lowlight", "babylight", "dimensional color", "dimensional blonde"], ["foil"]),
    c!("single_color", "All-over color", "color", 2, ["all over color", "allover color", "single process", "one process", "full color", "permanent color", "demi permanent", "semi permanent", "base color", "single color", "gray coverage", "grey coverage", "color retouch full"], ["color", "colour", "tint"]),
    c!("gloss", "Gloss / toner", "color", 1, ["gloss", "glaze", "toner", "toning"], []),
    // Specialty color
    c!("balayage", "Balayage / ombré", "specialty_color", 2, ["balayage", "ombre", "sombre", "foilayage", "lived in color", "color melt", "babylayage"], []),
    c!("color_correction", "Color correction", "specialty_color", 2, ["color correction", "colour correction", "corrective color"], []),
    c!("vivid", "Vivid / fashion color", "specialty_color", 2, ["vivid", "fashion color", "creative color", "pastel color", "fantasy color"], []),
    c!("double_process", "Double process / blonding", "specialty_color", 2, ["double process", "platinum", "bleach and tone", "lightening", "blonding", "global blonde"], []),
    // Treatments
    c!("keratin", "Keratin / smoothing", "treatment", 2, ["keratin", "brazilian blowout", "smoothing treatment", "smoothing", "japanese straight", "straightening", "relaxer", "hair botox"], []),
    c!("treatment", "Conditioning treatment", "treatment", 1, ["deep conditioning", "deep condition", "conditioning treatment", "olaplex", "k18", "bond builder", "bond repair", "hair mask", "scalp treatment", "hair treatment", "hydration treatment", "malibu"], ["treatment"]),
    // Extensions
    c!("extensions", "Hair extensions", "extensions", 2, ["hair extension", "extension", "tape in", "k tip", "i tip", "keratin bond", "hand tied", "weft", "fusion extension", "extension move up"], []),
    // Barbering
    c!("barber_cut", "Barber cut / fade", "barber", 2, ["fade", "skin fade", "taper", "buzz cut", "clipper cut", "flat top", "barber cut", "high and tight"], []),
    c!("beard", "Beard trim / shave", "barber", 1, ["beard", "shave", "hot towel", "straight razor", "line up", "lineup", "edge up"], []),
    // Nails
    c!("mani_pedi", "Manicure and pedicure", "nails", 2, ["mani pedi", "manicure and pedicure", "mani and pedi"], []),
    c!("gel_manicure", "Gel manicure", "nails", 2, ["gel manicure", "gel mani", "shellac manicure", "shellac", "gel polish", "gel color", "gel colour"], []),
    c!("manicure", "Manicure", "nails", 2, ["manicure", "mani"], []),
    c!("gel_pedicure", "Gel pedicure", "nails", 2, ["gel pedicure", "gel pedi", "shellac pedicure"], []),
    c!("pedicure", "Pedicure", "nails", 2, ["pedicure", "pedi"], []),
    c!("nail_extensions", "Nail extensions (acrylic, Gel-X)", "nails", 2, ["acrylic full set", "full set acrylic", "acrylic set", "acrylic nail", "gel x", "gelx", "nail extension", "hard gel", "sculpted"], ["acrylic", "full set"]),
    c!("nail_fill", "Nail fill", "nails", 2, ["acrylic fill", "gel fill", "nail fill", "fill in"], ["fill"]),
    c!("dip_powder", "Dip powder", "nails", 2, ["dip powder", "dipping powder", "sns"], ["dip"]),
    c!("nail_art", "Nail art", "nails", 1, ["nail art", "nail design", "french tip", "chrome"], []),
    c!("polish_change", "Polish change / removal", "nails", 1, ["polish change", "polish only", "polish removal", "gel removal", "gel polish removal", "acrylic removal", "soak off", "removal"], []),
    // Skin
    c!("facial", "Facial", "skin", 2, ["facial", "hydrafacial", "hydra facial", "skin treatment"], []),
    c!("peel", "Chemical peel", "skin", 2, ["chemical peel", "peel"], []),
    c!("dermaplaning", "Dermaplaning / microdermabrasion", "skin", 2, ["dermaplaning", "dermaplane", "microdermabrasion", "microderm", "microneedling"], []),
    // Brows and lashes
    c!("lash_extensions", "Lash extensions (full set)", "brows_lashes", 2, ["lash extension", "eyelash extension", "classic full set", "volume full set", "hybrid full set", "lash full set", "classic lash", "volume lash", "hybrid lash"], []),
    c!("lash_fill", "Lash fill", "brows_lashes", 2, ["lash fill", "lash refill", "classic fill", "volume fill", "hybrid fill"], []),
    c!("lash_lift", "Lash lift / tint", "brows_lashes", 2, ["lash lift", "lash perm", "lash tint", "lift and tint"], []),
    c!("brow_lamination", "Brow lamination", "brows_lashes", 2, ["brow lamination", "lamination"], []),
    c!("brow_tint", "Brow tint", "brows_lashes", 1, ["brow tint", "eyebrow tint"], []),
    c!("brows", "Brow shaping (wax or threading)", "brows_lashes", 2, ["brow wax", "eyebrow wax", "brow shaping", "eyebrow shaping", "brow threading", "eyebrow threading", "threading", "eyebrow", "brow"], []),
    // Waxing
    c!("brazilian_wax", "Brazilian wax", "waxing", 2, ["brazilian"], []),
    c!("bikini_wax", "Bikini wax", "waxing", 2, ["bikini"], []),
    c!("leg_wax", "Leg wax", "waxing", 2, ["full leg", "half leg", "leg wax", "lower leg", "upper leg"], []),
    c!("underarm_wax", "Underarm wax", "waxing", 2, ["underarm", "under arm"], []),
    c!("face_wax", "Lip / chin / face wax", "waxing", 2, ["lip wax", "upper lip", "chin wax", "full face wax", "lip and chin"], []),
    c!("wax", "Body waxing", "waxing", 1, ["sugaring", "body wax", "back wax", "arm wax", "chest wax"], ["wax", "waxing"]),
    // Makeup, massage
    c!("makeup", "Makeup", "makeup", 2, ["makeup", "make up", "airbrush makeup", "bridal makeup"], []),
    c!("massage", "Massage", "massage", 2, ["massage", "deep tissue", "swedish", "hot stone", "reflexology"], []),
];

/// Lowercase, "&"/"+" → "and", accents folded, punctuation dropped, simple plurals singular.
pub fn normalize(s: &str) -> String {
    let folded: String = s
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'á' | 'à' | 'â' | 'ä' => 'a',
            'í' | 'ï' => 'i',
            'ó' | 'ô' | 'ö' => 'o',
            'ú' | 'ü' => 'u',
            'ñ' => 'n',
            '&' | '+' => '&',
            '\'' | '’' | '‘' | '`' => '\0',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .filter(|c| *c != '\0')
        .collect();
    folded.replace('&', " and ").split_whitespace().map(singular).collect::<Vec<_>>().join(" ")
}

fn singular(w: &str) -> String {
    let n = w.len();
    if n > 4 && w.ends_with("ies") {
        format!("{}y", &w[..n - 3])
    } else if n > 4 && (w.ends_with("shes") || w.ends_with("ches") || w.ends_with("xes")) {
        w[..n - 2].to_string()
    } else if n > 3 && w.ends_with('s') && !w.ends_with("ss") && !w.ends_with("us") {
        w[..n - 1].to_string()
    } else {
        w.to_string()
    }
}

struct Compiled {
    canon: &'static Canon,
    patterns: Vec<String>,
    fallback: Vec<String>,
}

fn table() -> &'static Vec<Compiled> {
    static T: OnceLock<Vec<Compiled>> = OnceLock::new();
    T.get_or_init(|| {
        CATALOG
            .iter()
            .map(|c| Compiled { canon: c, patterns: c.patterns.iter().map(|p| format!(" {} ", normalize(p))).collect(), fallback: c.fallback.iter().map(|p| format!(" {} ", normalize(p))).collect() })
            .collect()
    })
}

/// A classification and how it was made.
#[derive(Debug, Clone, Copy)]
pub struct Match {
    pub canon: &'static Canon,
    /// A specific pattern (not a generic fallback word) matched.
    pub specific: bool,
    /// Byte offset of the winning pattern in the padded, normalised text.
    pub start: usize,
    pub len: usize,
}

/// The canonical service a menu label describes, with how it matched.
pub fn classify_match(label: &str) -> Option<Match> {
    let hay = format!(" {} ", normalize(label));
    let best = |fallback: bool| {
        let mut all: Vec<Match> = Vec::new();
        for c in table() {
            let pats = if fallback { &c.fallback } else { &c.patterns };
            for p in pats {
                if let Some(at) = hay.find(p.as_str()) {
                    all.push(Match { canon: c.canon, specific: !fallback, start: at, len: p.len() - 2 });
                }
            }
        }
        // Higher priority, then longer pattern; equal scores keep catalog order.
        let mut win = *all.iter().rev().max_by_key(|m| (m.canon.priority, m.len))?;
        // ...unless a longer phrase covers the winner ("gel polish removal" over "gel polish").
        if let Some(cover) = all.iter().filter(|m| m.len > win.len && m.start <= win.start && m.start + m.len >= win.start + win.len).max_by_key(|m| m.len) {
            win = *cover;
        }
        Some(win)
    };
    best(false).or_else(|| best(true))
}

/// The canonical service a menu label describes, if any.
pub fn classify(label: &str) -> Option<&'static Canon> {
    classify_match(label).map(|m| m.canon)
}

/// Classify a label inside a menu section ("Classic" under "PEDICURE", "Gel (Shellac)" under
/// "PEDICURE"). The section refines the label only when the label alone is vague, or when the
/// combined phrase still starts within the label — so a "Cut & Color" heading can't turn "Gloss"
/// into a cut and color.
pub fn classify_in_section(label: &str, heading: Option<&str>) -> Option<&'static Canon> {
    let own = classify_match(label);
    let Some(h) = heading else { return own.map(|m| m.canon) };
    let label_len = normalize(label).len();
    let combined = classify_match(&format!("{label} {h}")).filter(|m| m.specific);
    match (own, combined) {
        (Some(o), Some(c)) if o.specific => Some(if c.start < label_len && c.len > o.len { c.canon } else { o.canon }),
        (_, Some(c)) => Some(c.canon),
        (Some(o), None) => Some(o.canon),
        // Last resort: generic words across label and section ("Legs (Half)" under "Waxing").
        (None, None) => classify(&format!("{label} {h}")).or_else(|| classify(&format!("{h} {label}"))),
    }
}

pub fn by_key(key: &str) -> Option<&'static Canon> {
    CATALOG.iter().find(|c| c.key == key)
}

pub fn category_label(key: &str) -> &'static str {
    CATEGORIES.iter().find(|(k, _)| *k == key).map(|(_, l)| *l).unwrap_or("Other")
}

fn words(s: &str) -> std::collections::HashSet<String> {
    normalize(s).split(' ').filter(|w| w.len() > 1).map(String::from).collect()
}

/// Share of words two labels have in common (Jaccard index).
pub fn similarity(a: &str, b: &str) -> f64 {
    let (a, b) = (words(a), words(b));
    let union = a.union(&b).count();
    if union == 0 {
        0.0
    } else {
        a.intersection(&b).count() as f64 / union as f64
    }
}

/// Match a competitor's listed service to one of yours: same canonical service (closest wording
/// wins when you have several), or — for your services the catalog doesn't know — wording at
/// least 60% the same. Returns None rather than guess.
pub fn match_service(canon: Option<&Canon>, label: &str, services: &[(i64, String)]) -> Option<i64> {
    let score = |name: &str| similarity(name, label);
    let pick = |cands: Vec<&(i64, String)>| cands.into_iter().map(|(id, n)| (score(n), *id)).fold(None::<(f64, i64)>, |m, x| if m.is_none_or(|m| x.0 > m.0) { Some(x) } else { m }).map(|x| x.1);
    if let Some(c) = canon {
        let same: Vec<_> = services.iter().filter(|(_, n)| classify(n).is_some_and(|k| k.key == c.key)).collect();
        if !same.is_empty() {
            return pick(same);
        }
    }
    let unknown: Vec<_> = services.iter().filter(|(_, n)| classify(n).is_none() && score(n) >= 0.6).collect();
    pick(unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(l: &str) -> &'static str {
        classify(l).map(|c| c.key).unwrap_or("-")
    }

    #[test]
    fn normalises_equivalent_names() {
        for l in ["WOMENS HAIRCUT", "Women's Cut & Style", "Ladies' cut", "Women’s Haircut + Blowdry"] {
            assert_eq!(key(l), "womens_cut", "{l}");
        }
        assert_eq!(key("Men's Cut"), "mens_cut");
        assert_eq!(key("Kids Cut (under 12)"), "kids_cut");
        assert_eq!(key("Haircut & Blowdry"), "haircut"); // a cut with a finish, not a blow-dry
        assert_eq!(key("Signature Blowout"), "blowout");
        assert_eq!(key("Brazilian Blowout"), "keratin"); // not a wax, not a blow-dry
        assert_eq!(key("Brazilian"), "brazilian_wax");
        assert_eq!(key("PARTIAL BALAYAGE"), "balayage");
        assert_eq!(key("Ombré"), "balayage");
        assert_eq!(key("Full Highlights"), "full_highlights");
        assert_eq!(key("Partial Foils"), "partial_highlights");
        assert_eq!(key("Root Touch-Up"), "root_touchup");
        assert_eq!(key("Cut & Color"), "cut_and_color"); // a combination stays a combination
        assert_eq!(key("Color"), "single_color");
        assert_eq!(key("Gel Color Manicure"), "gel_manicure");
        assert_eq!(key("Gel Manicure & Pedicure"), "mani_pedi");
        assert_eq!(key("Lash Extensions - Full Set"), "lash_extensions");
        assert_eq!(key("Tape-In Extensions"), "extensions");
        assert_eq!(key("Acne Facial Treatment"), "facial"); // specific beats the generic word
        assert_eq!(key("Deep Conditioning Treatment"), "treatment");
        assert_eq!(key("Skin Fade"), "barber_cut");
        assert_eq!(key("Eyebrow Wax"), "brows");
        assert_eq!(key("Full Leg Wax"), "leg_wax");
        assert_eq!(key("Shampoo, 8 oz"), "-");
        assert_eq!(key("Gift card"), "-");
        assert_eq!(key("Gel Polish Removal"), "polish_change");
        let sec = |l: &str, h: &str| classify_in_section(l, Some(h)).map(|c| c.key).unwrap_or("-");
        assert_eq!(sec("Classic", "PEDICURE"), "pedicure");
        assert_eq!(sec("Gel (Shellac)", "PEDICURE"), "gel_pedicure");
        assert_eq!(sec("Kids (9 and under)", "MANICURE"), "manicure");
        assert_eq!(sec("Gloss", "Cut & Color"), "gloss");
        assert_eq!(sec("Partial", "Highlights"), "partial_highlights");
        assert_eq!(sec("Women's Haircut", "Color"), "womens_cut");
    }

    #[test]
    fn matches_your_services() {
        let mine = vec![(1, "Women's Cut & Finish".to_string()), (2, "Men's Cut".to_string()), (3, "Root Color".to_string()), (4, "Lived-in Blonde Package".to_string())];
        assert_eq!(match_service(classify("WOMENS HAIRCUT"), "WOMENS HAIRCUT", &mine), Some(1));
        assert_eq!(match_service(classify("Root touch-up"), "Root touch-up", &mine), Some(3));
        assert_eq!(match_service(classify("Balayage"), "Balayage", &mine), None); // no guess
        // A service the catalog doesn't know matches only on close wording.
        assert_eq!(match_service(None, "Lived-in Blonde Package", &mine), Some(4));
        assert_eq!(match_service(None, "Blonde", &mine), None);
    }
}
