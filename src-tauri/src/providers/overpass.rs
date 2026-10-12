//! OpenStreetMap Overpass: hair, nail, beauty and barber businesses mapped near a point. Names,
//! places and websites only — OpenStreetMap doesn't hold prices. Data © OpenStreetMap
//! contributors (ODbL).
//!
//! Reliability: queries are POSTed (some public instances reject long GET queries) and tried on
//! several public servers in turn, because any one of them is often overloaded (504) or busy (429).
//! Results are never capped: a capped query silently drops businesses in dense areas. A 200 answer
//! carrying a `remark` runtime error (timeout, out of memory) is a failure, not a result.

use super::fetch_checked;
use crate::db::market::{normalize_website, CompetitorInput};
use crate::error::{AppError, AppResult};
use crate::workspace::AppState;

/// Public Overpass instances, tried in order. All are free and keyless.
pub const ENDPOINTS: &[&str] = &[
    "https://overpass-api.de/api/interpreter",
    "https://lz4.overpass-api.de/api/interpreter",
    "https://z.overpass-api.de/api/interpreter",
    "https://overpass.private.coffee/api/interpreter",
];

#[derive(Debug, Clone)]
pub struct Found {
    /// "node/123", "way/456"
    pub osm_ref: String,
    pub competitor: CompetitorInput,
    /// Business types (see `prefs::BUSINESS_TYPES`) this place belongs to.
    pub types: Vec<&'static str>,
}

#[derive(Debug, Clone)]
pub struct Discovery {
    pub found: Vec<Found>,
    /// Matching map features without a name (can't be identified, so not added).
    pub unnamed: usize,
    pub endpoint: String,
    pub from_cache: bool,
    pub fetched_at: String,
}

/// Overpass QL for the selected business types around a point (radius in metres).
pub fn query(lat: f64, lon: f64, radius_m: f64, types: &[String]) -> String {
    let a = format!("(around:{radius_m:.0},{lat:.5},{lon:.5})");
    let has = |t: &str| types.iter().any(|x| x == t);
    let mut q = String::new();
    if has("hair") || has("barber") {
        q += &format!("nwr[\"shop\"~\"^(hairdresser|barber)$\"]{a};nwr[\"craft\"=\"hairdresser\"]{a};");
    }
    if has("nails") || has("beauty") {
        q += &format!("nwr[\"shop\"~\"^(beauty|nails|nail_salon)$\"]{a};");
    }
    if has("beauty") {
        q += &format!("nwr[\"amenity\"=\"spa\"]{a};");
    }
    if has("massage") {
        q += &format!("nwr[\"shop\"=\"massage\"]{a};");
    }
    format!("[out:json][timeout:40];({q});out center tags;")
}

/// Which business types a mapped place belongs to, from its tags and name.
fn classify(tags: &serde_json::Value) -> Vec<&'static str> {
    let t = |k: &str| tags[k].as_str().unwrap_or("").to_lowercase();
    let (shop, name, beauty, hd) = (t("shop"), t("name"), t("beauty"), t("hairdresser"));
    let mut out = Vec::new();
    if shop == "hairdresser" || t("craft") == "hairdresser" || beauty.contains("hair") {
        out.push("hair");
    }
    if shop == "barber" || hd.contains("barber") || name.contains("barber") {
        out.push("barber");
    }
    if shop == "nails" || shop == "nail_salon" || beauty.contains("nail") || (shop == "beauty" && name.contains("nail")) {
        out.push("nails");
    }
    if (shop == "beauty" && !out.contains(&"nails")) || t("amenity") == "spa" {
        out.push("beauty");
    }
    if shop == "massage" {
        out.push("massage");
    }
    out
}

/// Parse an Overpass JSON answer, keeping only places of the selected types.
pub fn parse(body: &str, types: &[String]) -> AppResult<(Vec<Found>, usize)> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|_| AppError::Provider("OpenStreetMap sent an unreadable response.".into()))?;
    if let Some(r) = v["remark"].as_str().filter(|r| r.contains("error")) {
        return Err(AppError::Provider(format!("The OpenStreetMap server couldn't finish the search ({}).", r.chars().take(120).collect::<String>())));
    }
    let elements = v["elements"].as_array().ok_or_else(|| AppError::Provider("OpenStreetMap sent an unexpected response.".into()))?;
    let (mut out, mut unnamed) = (Vec::new(), 0);
    for e in elements {
        let tags = &e["tags"];
        let kinds: Vec<&'static str> = classify(tags).into_iter().filter(|k| types.iter().any(|t| t == k)).collect();
        if kinds.is_empty() {
            continue;
        }
        let Some(name) = tags["name"].as_str().map(str::trim).filter(|n| !n.is_empty()) else {
            unnamed += 1;
            continue;
        };
        let (lat, lon) = match (e["lat"].as_f64(), e["lon"].as_f64()) {
            (Some(a), Some(b)) => (a, b),
            _ => match (e["center"]["lat"].as_f64(), e["center"]["lon"].as_f64()) {
                (Some(a), Some(b)) => (a, b),
                _ => continue,
            },
        };
        let s = |k: &str| tags[k].as_str().unwrap_or("").trim().to_string();
        let street = [s("addr:housenumber"), s("addr:street")].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" ");
        let website = [s("website"), s("contact:website"), s("url")].into_iter().find_map(|w| normalize_website(&w).ok().filter(|w| !w.is_empty())).unwrap_or_default();
        out.push(Found {
            osm_ref: format!("{}/{}", e["type"].as_str().unwrap_or("node"), e["id"]),
            competitor: CompetitorInput {
                id: None,
                name: name.to_string(),
                address: street,
                city: s("addr:city"),
                state: s("addr:state"),
                postal_code: s("addr:postcode"),
                latitude: Some(lat.to_string()),
                longitude: Some(lon.to_string()),
                website,
                phone: [s("phone"), s("contact:phone")].into_iter().find(|x| !x.is_empty()).unwrap_or_default(),
                notes: String::new(),
            },
            types: kinds,
        });
    }
    Ok((out, unnamed))
}

/// Search each public server in turn until one answers properly.
pub async fn nearby(state: &AppState, lat: f64, lon: f64, radius_km: f64, types: &[String]) -> AppResult<Discovery> {
    let r = (radius_km.clamp(0.5, 50.0) * 1000.0).round();
    let q = query(lat, lon, r, types);
    let mut sorted = types.to_vec();
    sorted.sort();
    let key = format!("v2|{lat:.4},{lon:.4},{r}|{}", sorted.join(","));
    let check = |body: &str| parse(body, types).map(|_| ());
    let mut errors = Vec::new();
    for ep in ENDPOINTS {
        // Cache a week: map data changes slowly and the public servers ask for light use.
        match fetch_checked(state, "overpass", &key, ep, Some(&[("data", q.as_str())]), &[], Some(24 * 7), &check).await {
            Ok(f) => {
                let (found, unnamed) = parse(&f.body, types)?;
                let host = reqwest::Url::parse(ep).ok().and_then(|u| u.host_str().map(String::from)).unwrap_or_default();
                return Ok(Discovery { found, unnamed, endpoint: if f.from_cache { "cache".into() } else { host }, from_cache: f.from_cache, fetched_at: f.fetched_at });
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.starts_with("Today's limit") {
                    return Err(e);
                }
                errors.push(msg);
            }
        }
    }
    Err(AppError::Provider(format!(
        "All {} public OpenStreetMap servers failed or were busy. Try again in a few minutes. ({})",
        ENDPOINTS.len(),
        errors.last().cloned().unwrap_or_default()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<String> {
        crate::prefs::BUSINESS_TYPES.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_real_response() {
        let (v, _) = parse(include_str!("fixtures/overpass_tumwater.json"), &all()).unwrap();
        assert!(v.len() >= 10);
        assert!(v.iter().any(|f| f.competitor.name == "Great Clips" && f.types.contains(&"hair")));
        assert!(v.iter().all(|f| f.osm_ref.contains('/') && f.competitor.latitude.is_some()));
        // Websites always come out absolute (OSM often has "www.example.com").
        assert!(v.iter().all(|f| f.competitor.website.is_empty() || f.competitor.website.starts_with("http")));
    }

    #[test]
    fn types_filter_and_unnamed() {
        let body = r#"{"elements":[
            {"type":"node","id":1,"lat":1.0,"lon":2.0,"tags":{"shop":"hairdresser","name":"Cuts","website":"www.cuts.example"}},
            {"type":"node","id":2,"lat":1.0,"lon":2.0,"tags":{"shop":"beauty","name":"Star Nail & Spa"}},
            {"type":"way","id":3,"center":{"lat":1.0,"lon":2.0},"tags":{"shop":"barber","name":"Joe's"}},
            {"type":"node","id":4,"lat":1.0,"lon":2.0,"tags":{"shop":"beauty"}},
            {"type":"node","id":5,"lat":1.0,"lon":2.0,"tags":{"shop":"massage","name":"Kneads"}}
        ]}"#;
        let (v, unnamed) = parse(body, &["nails".into()]).unwrap();
        assert_eq!(v.iter().map(|f| f.competitor.name.as_str()).collect::<Vec<_>>(), ["Star Nail & Spa"]);
        assert_eq!(unnamed, 0);
        let (v, unnamed) = parse(body, &all()).unwrap();
        assert_eq!(v.len(), 4);
        assert_eq!(unnamed, 1);
        assert_eq!(v[0].competitor.website, "https://www.cuts.example/");
        assert_eq!(v[2].osm_ref, "way/3");
        assert!(!parse(body, &["hair".into()]).unwrap().0.iter().any(|f| f.competitor.name == "Kneads"));
    }

    #[test]
    fn remark_errors_are_failures_and_query_is_uncapped() {
        let partial = r#"{"elements":[],"remark":"runtime error: Query timed out in \"query\" at line 1 after 41 seconds."}"#;
        assert!(parse(partial, &all()).unwrap_err().to_string().contains("timed out"));
        let q = query(47.0, -122.9, 8046.72, &["hair".into(), "massage".into()]);
        assert!(q.ends_with("out center tags;"), "{q}");
        assert!(q.contains("around:8047,47.00000,-122.90000") && q.contains("massage") && !q.contains("amenity"));
    }

    /// Live check against the public servers: `cargo test live_overpass -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_overpass_cities() {
        let client = reqwest::Client::builder().user_agent(crate::providers::USER_AGENT).timeout(std::time::Duration::from_secs(60)).build().unwrap();
        for (city, lat, lon) in [("Tumwater", 46.992, -122.9168), ("Seattle", 47.6097, -122.3331), ("Austin", 30.2672, -97.7431), ("Indianapolis", 39.7684, -86.1581)] {
            for (r, types) in [(3000.0, vec!["hair".to_string()]), (8000.0, all())] {
                let q = query(lat, lon, r, &types);
                let mut result = None;
                for ep in ENDPOINTS {
                    let Ok(resp) = client.post(*ep).form(&[("data", q.as_str())]).send().await else { continue };
                    if !resp.status().is_success() {
                        println!("  {city} {ep}: {}", resp.status());
                        continue;
                    }
                    match parse(&resp.text().await.unwrap(), &types) {
                        Ok(v) => {
                            result = Some((ep, v));
                            break;
                        }
                        Err(e) => println!("  {city} {ep}: {e}"),
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
                let (ep, (found, unnamed)) = result.unwrap_or_else(|| panic!("{city}: every endpoint failed"));
                let web = found.iter().filter(|f| !f.competitor.website.is_empty()).count();
                println!("{city} r={r}m types={types:?}: {} named ({unnamed} unnamed), {web} with website, via {ep}", found.len());
                assert!(!found.is_empty());
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        }
    }
}
