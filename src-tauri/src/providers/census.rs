//! U.S. Census Geocoder: street address → coordinates and state/county/place codes.
//! (Census Data API context lookups are added with the market research features.)

use super::fetch;
use crate::db::core::Location;
use crate::error::{AppError, AppResult};
use crate::workspace::AppState;
use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Geocode {
    pub matched_address: String,
    pub latitude: String,
    pub longitude: String,
    pub state_fips: Option<String>,
    pub county_fips: Option<String>,
    pub county_name: Option<String>,
    pub place_fips: Option<String>,
    pub place_name: Option<String>,
}

pub fn parse_geocode(body: &str) -> AppResult<Geocode> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|_| AppError::Provider("The Census Geocoder sent an unreadable response.".into()))?;
    let matches = v["result"]["addressMatches"].as_array().cloned().unwrap_or_default();
    let m = match matches.len() {
        0 => return Err(AppError::Provider("The Census Geocoder couldn't match this address. Check the street, city, state and ZIP.".into())),
        1 => &matches[0],
        n => return Err(AppError::Provider(format!("The address matched {n} places. Add more detail (unit, ZIP+4) and try again."))),
    };
    let g = &m["geographies"];
    let first = |layer: &str, field: &str| g[layer][0][field].as_str().map(String::from);
    Ok(Geocode {
        matched_address: m["matchedAddress"].as_str().unwrap_or("").into(),
        latitude: m["coordinates"]["y"].to_string(),
        longitude: m["coordinates"]["x"].to_string(),
        state_fips: first("States", "STATE"),
        county_fips: first("Counties", "GEOID"),
        county_name: first("Counties", "NAME"),
        place_fips: first("Incorporated Places", "GEOID"),
        place_name: first("Incorporated Places", "NAME"),
    })
}

pub async fn geocode(state: &AppState, loc: &Location) -> AppResult<Geocode> {
    if loc.address_line.trim().is_empty() || (loc.postal_code.trim().is_empty() && (loc.city.trim().is_empty() || loc.state.is_empty())) {
        return Err(AppError::Provider("Enter a street address plus a ZIP code (or city and state) to look it up.".into()));
    }
    let one_line = format!("{}, {}, {} {}", loc.address_line.trim(), loc.city.trim(), loc.state.trim(), loc.postal_code.trim());
    let url = reqwest::Url::parse_with_params(
        "https://geocoding.geo.census.gov/geocoder/geographies/onelineaddress",
        &[("address", one_line.as_str()), ("benchmark", "Public_AR_Current"), ("vintage", "Current_Current"), ("format", "json"), ("layers", "States,Counties,Incorporated Places")],
    )
    .map_err(|e| AppError::Other(e.to_string()))?;
    let f = fetch(state, "census_geocoder", &one_line.to_lowercase(), url.as_str(), &[], Some(24 * 365)).await?;
    parse_geocode(&f.body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_match() {
        let g = parse_geocode(include_str!("fixtures/census_geocoder.json")).unwrap();
        assert_eq!(g.state_fips.as_deref(), Some("53"));
        assert_eq!(g.county_fips.as_deref(), Some("53067"));
        assert_eq!(g.place_name.as_deref(), Some("Tumwater city"));
        assert!(g.latitude.starts_with("46.99"));
        assert!(parse_geocode(r#"{"result":{"addressMatches":[]}}"#).is_err());
    }
}

// ---------------------------------------------------------------- Census Data API (key required)

/// ACS 5-year vintage and County Business Patterns year used for context. Update when newer data
/// is published (the API lists datasets at https://api.census.gov/data.json).
pub const ACS_YEAR: &str = "2024";
pub const CBP_YEAR: &str = "2023";

#[derive(Serialize, Clone, Debug)]
pub struct ContextFigure {
    pub label: String,
    pub value: Option<String>,
    pub geography: String,
    pub dataset: String,
    pub note: String,
}

/// Parse the Census API's array-of-arrays JSON into header → value maps.
pub fn parse_table(body: &str) -> AppResult<Vec<std::collections::HashMap<String, String>>> {
    let v: Vec<Vec<serde_json::Value>> = serde_json::from_str(body).map_err(|_| AppError::Provider("The Census Data API sent an unreadable response.".into()))?;
    let Some((head, rows)) = v.split_first() else { return Ok(vec![]) };
    let head: Vec<String> = head.iter().map(|h| h.as_str().unwrap_or("").to_string()).collect();
    Ok(rows
        .iter()
        .map(|r| head.iter().cloned().zip(r.iter().map(|c| c.as_str().map(String::from).unwrap_or_else(|| c.to_string()))).collect())
        .collect())
}

/// Census uses large negative sentinels (e.g. -666666666) for "not available".
fn clean(v: Option<&String>) -> Option<String> {
    v.filter(|s| !s.starts_with('-') && !s.is_empty() && s.as_str() != "null").cloned()
}

pub async fn context(state: &AppState, loc: &Location) -> AppResult<Vec<ContextFigure>> {
    let key = super::key_get("census_data")?.ok_or_else(|| {
        AppError::Provider("The Census Data API now requires a free key. Add one in Settings → Data providers to load local income, population and salon counts.".into())
    })?;
    let zip = loc.postal_code.get(..5).unwrap_or("").to_string();
    let mut out = Vec::new();
    let get = |dataset: &str, vars: &str, geo: String| {
        let url = format!("https://api.census.gov/data/{dataset}?get={vars}&{geo}&key={key}");
        (url, format!("{dataset}?{vars}&{geo}"))
    };
    let acs = format!("{ACS_YEAR}/acs/acs5");
    let acs_label = format!("ACS {}–{} 5-year estimates", ACS_YEAR.parse::<i32>().unwrap_or(2024) - 4, ACS_YEAR);
    if zip.len() == 5 {
        let (url, rk) = get(&acs, "NAME,B19013_001E,B01003_001E", format!("for=zip%20code%20tabulation%20area:{zip}"));
        let f = fetch(state, "census_data", &rk, &url, &[&key], Some(24 * 30)).await?;
        if let Some(r) = parse_table(&f.body)?.first() {
            let geo = format!("ZCTA {zip} (a Census area that approximates the ZIP code)");
            out.push(ContextFigure { label: "Median household income".into(), value: clean(r.get("B19013_001E")).map(|v| format!("${v}")), geography: geo.clone(), dataset: acs_label.clone(), note: "Context only".into() });
            out.push(ContextFigure { label: "Population".into(), value: clean(r.get("B01003_001E")), geography: geo, dataset: acs_label.clone(), note: "Context only".into() });
        }
        let (url, rk) = get(&format!("{CBP_YEAR}/cbp"), "ESTAB,NAICS2017_LABEL", format!("for=zip%20code:{zip}&NAICS2017=812112"));
        if let Ok(f) = fetch(state, "census_data", &rk, &url, &[&key], Some(24 * 30)).await {
            if let Some(r) = parse_table(&f.body)?.first() {
                out.push(ContextFigure {
                    label: "Beauty salon establishments (NAICS 812112)".into(),
                    value: clean(r.get("ESTAB")),
                    geography: format!("ZIP code {zip} (postal ZIP as used by County Business Patterns)"),
                    dataset: format!("County Business Patterns {CBP_YEAR}"),
                    note: "Employer establishments only; self-employed booth renters aren't counted".into(),
                });
            }
        }
    }
    if let (Some(st), Some(county)) = (&loc.state_fips, &loc.county_fips) {
        let cty = &county[county.len().saturating_sub(3)..];
        let (url, rk) = get(&acs, "NAME,B19013_001E", format!("for=county:{cty}&in=state:{st}"));
        if let Ok(f) = fetch(state, "census_data", &rk, &url, &[&key], Some(24 * 30)).await {
            if let Some(r) = parse_table(&f.body)?.first() {
                out.push(ContextFigure { label: "Median household income".into(), value: clean(r.get("B19013_001E")).map(|v| format!("${v}")), geography: r.get("NAME").cloned().unwrap_or_default(), dataset: acs_label.clone(), note: "Context only".into() });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod data_tests {
    use super::*;

    #[test]
    fn parses_census_table_shape() {
        // Shape documented by the Census Data API (header row, then rows). Values are made up for the test.
        let body = r#"[["NAME","B19013_001E","zip code tabulation area"],["ZCTA5 98501","81234","98501"]]"#;
        let t = parse_table(body).unwrap();
        assert_eq!(t[0]["B19013_001E"], "81234");
        assert_eq!(clean(Some(&"-666666666".to_string())), None);
    }
}
