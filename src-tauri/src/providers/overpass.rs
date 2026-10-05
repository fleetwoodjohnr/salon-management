//! OpenStreetMap Overpass: hair and beauty salons mapped near a point. Names and places only —
//! OpenStreetMap doesn't hold prices. Data © OpenStreetMap contributors (ODbL).

use super::fetch;
use crate::db::market::CompetitorInput;
use crate::error::{AppError, AppResult};
use crate::workspace::AppState;

pub fn parse(body: &str) -> AppResult<Vec<(String, CompetitorInput)>> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|_| AppError::Provider("Overpass sent an unreadable response.".into()))?;
    let mut out = Vec::new();
    for e in v["elements"].as_array().cloned().unwrap_or_default() {
        let tags = &e["tags"];
        let Some(name) = tags["name"].as_str() else { continue };
        let (lat, lon) = match (e["lat"].as_f64(), e["lon"].as_f64()) {
            (Some(a), Some(b)) => (a, b),
            _ => match (e["center"]["lat"].as_f64(), e["center"]["lon"].as_f64()) {
                (Some(a), Some(b)) => (a, b),
                _ => continue,
            },
        };
        let s = |k: &str| tags[k].as_str().unwrap_or("").to_string();
        let street = [s("addr:housenumber"), s("addr:street")].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" ");
        out.push((
            format!("{}/{}", e["type"].as_str().unwrap_or("node"), e["id"]),
            CompetitorInput {
                id: None,
                name: name.to_string(),
                address: street,
                city: s("addr:city"),
                state: s("addr:state"),
                postal_code: s("addr:postcode"),
                latitude: Some(lat.to_string()),
                longitude: Some(lon.to_string()),
                website: [s("website"), s("contact:website")].into_iter().find(|x| !x.is_empty()).unwrap_or_default(),
                phone: [s("phone"), s("contact:phone")].into_iter().find(|x| !x.is_empty()).unwrap_or_default(),
                notes: format!("OpenStreetMap category: {}", s("shop")),
            },
        ));
    }
    Ok(out)
}

pub async fn nearby(state: &AppState, lat: f64, lon: f64, radius_km: f64) -> AppResult<Vec<(String, CompetitorInput)>> {
    let r = (radius_km.clamp(0.5, 25.0) * 1000.0).round();
    let q = format!("[out:json][timeout:25];nwr[\"shop\"~\"^(hairdresser|beauty)$\"](around:{r},{lat:.5},{lon:.5});out center tags 300;");
    let url = reqwest::Url::parse_with_params("https://overpass-api.de/api/interpreter", &[("data", q.as_str())]).map_err(|e| AppError::Other(e.to_string()))?;
    // Cache a week: map data changes slowly and the public server asks for light use.
    let f = fetch(state, "overpass", &format!("{lat:.4},{lon:.4},{r}"), url.as_str(), &[], Some(24 * 7)).await?;
    parse(&f.body)
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_real_response() {
        let v = super::parse(include_str!("fixtures/overpass_tumwater.json")).unwrap();
        assert!(v.len() >= 10);
        assert!(v.iter().any(|(_, c)| c.name == "Great Clips"));
        assert!(v.iter().all(|(r, c)| r.contains('/') && c.latitude.is_some()));
    }
}
