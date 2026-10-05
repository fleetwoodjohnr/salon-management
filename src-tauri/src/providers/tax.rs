//! Official state sales-tax rate lookups (Washington DOR, California CDTFA).
//! A successful response is a rate for a jurisdiction; it says nothing about whether a given
//! service or product is taxable — that stays a separate, recorded decision.

use super::{fetch, Fetched};
use crate::db::core::Location;
use crate::db::tax::RateSetInput;
use crate::error::{AppError, AppResult};
use crate::workspace::AppState;
use rust_decimal::Decimal;
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct TaxLookup {
    pub provider: String,
    pub request_key: String,
    pub fetched_at: String,
    pub from_cache: bool,
    /// What we'd save; `location_id` filled by the caller.
    pub rate: RateSetInput,
    /// The address as the provider understood it — check it matches yours.
    pub matched_address: String,
    pub notes: Vec<String>,
}

pub fn provider_for(state_code: &str) -> Option<&'static str> {
    match state_code {
        "WA" => Some("wa_dor"),
        "CA" => Some("ca_cdtfa"),
        _ => None,
    }
}

fn pct(fraction: &str) -> AppResult<Decimal> {
    let d: Decimal = fraction.trim().parse().map_err(|_| AppError::Provider(format!("Unexpected rate value \"{fraction}\".")))?;
    Ok((d * Decimal::ONE_HUNDRED).normalize())
}

/// Value of `name="…"` inside the first `<tag …>` element. The WA response is a small fixed
/// shape, so a full XML parser isn't needed.
fn attr(xml: &str, tag: &str, name: &str) -> Option<String> {
    let start = xml.find(&format!("<{tag} "))?;
    let end = start + xml[start..].find('>')?;
    let el = &xml[start..end];
    let key = format!(" {name}=\"");
    let i = el.find(&key)? + key.len();
    let j = i + el[i..].find('"')?;
    Some(el[i..j].replace("&amp;", "&").replace("&quot;", "\""))
}

pub fn parse_wa(body: &str, fetched_at: &str) -> AppResult<(RateSetInput, String, Vec<String>)> {
    let code = attr(body, "response", "code").ok_or_else(|| AppError::Provider("Washington DOR sent an unreadable response.".into()))?;
    let (precision, status, mut notes): (&str, &str, Vec<String>) = match code.as_str() {
        "0" => ("address", "verified", vec![]),
        "2" | "4" => ("address", "verified", vec!["WA DOR adjusted the address to find it; check the matched address below.".into()]),
        "1" | "3" => ("zip9", "estimate", vec!["Address not found; the rate is for the ZIP+4 area, which can span more than one tax jurisdiction.".into()]),
        "5" => ("zip5", "estimate", vec!["Only the 5-digit ZIP was found. ZIP codes can cross tax boundaries, so this is an estimate for your address.".into()]),
        "6" => return Err(AppError::Provider("Washington DOR couldn't find the address, ZIP+4 or ZIP. Check the location's address.".into())),
        "7" => return Err(AppError::Provider("Washington DOR rejected the coordinates.".into())),
        _ => {
            let hint = attr(body, "response", "debughint").map(|h| format!(" ({h})")).unwrap_or_default();
            return Err(AppError::Provider(format!("Washington DOR couldn't look up this address{hint}.")));
        }
    };
    let total = pct(&attr(body, "response", "rate").unwrap_or_default())?;
    let state_rate = attr(body, "rate", "staterate").map(|r| pct(&r)).transpose()?;
    let local = attr(body, "rate", "localrate").or_else(|| attr(body, "response", "localrate")).map(|r| pct(&r)).transpose()?;
    let name = attr(body, "rate", "name").unwrap_or_default();
    let loccode = attr(body, "response", "loccode");
    let period = attr(body, "addressline", "period");
    if period.is_none() {
        notes.push("The response didn't state which tax period it covers.".into());
    }
    let matched = [
        attr(body, "addressline", "houselow").unwrap_or_default(),
        attr(body, "addressline", "street").unwrap_or_default(),
        attr(body, "addressline", "zip").unwrap_or_default(),
    ]
    .iter()
    .filter(|s| !s.is_empty())
    .cloned()
    .collect::<Vec<_>>()
    .join(" ");
    // If WA gave both parts and they don't add up, keep the total only (don't invent a split).
    let (state_rate, city_rate) = match (state_rate, local) {
        (Some(s), Some(l)) if s + l == total => (Some(s), Some(l)),
        _ => (None, None),
    };
    Ok((
        RateSetInput {
            location_id: 0,
            state_rate,
            county_rate: None,
            // WA reports one combined local rate (city/county/transit); it's stored as the local part.
            city_rate,
            district_rate: None,
            total_rate: Some(total),
            jurisdiction_label: format!("{} (WA location code {})", if name.is_empty() { "Washington".into() } else { name }, loccode.clone().unwrap_or_default()),
            jurisdiction_code: loccode,
            source: "wa_dor".into(),
            source_url: Some(super::info("wa_dor")?.docs_url.into()),
            precision: precision.into(),
            status: status.into(),
            effective_date: None,
            dataset_period: period,
            retrieved_at: Some(fetched_at.into()),
            note: "Local rate is WA's combined local component (city, county and transit).".into(),
        },
        matched,
        notes,
    ))
}

pub fn parse_cdtfa(body: &str, fetched_at: &str) -> AppResult<(RateSetInput, String, Vec<String>)> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|_| AppError::Provider("CDTFA sent an unreadable response.".into()))?;
    let infos = v["taxRateInfo"].as_array().cloned().unwrap_or_default();
    let matched = v["geocodeInfo"]["formattedAddress"].as_str().unwrap_or("").to_string();
    if infos.is_empty() {
        return Err(AppError::Provider("CDTFA found no tax rate for this address.".into()));
    }
    if infos.len() > 1 {
        let list: Vec<String> = infos.iter().map(|i| format!("{} {}%", i["jurisdiction"].as_str().unwrap_or("?"), i["rate"])).collect();
        return Err(AppError::Provider(format!(
            "CDTFA matched \"{matched}\" to more than one jurisdiction ({}). Check the address, or enter the rate manually from CDTFA's website.",
            list.join("; ")
        )));
    }
    let i = &infos[0];
    let rate = i["rate"].as_f64().map(|_| i["rate"].to_string()).ok_or_else(|| AppError::Provider("CDTFA response had no rate.".into()))?;
    let total = pct(&rate)?;
    let confidence = v["geocodeInfo"]["confidence"].as_str().unwrap_or("");
    let good = v["geocodeInfo"]["matchCodes"].as_array().is_some_and(|m| m.iter().any(|x| x == "Good"));
    let (precision, status, mut notes) = if confidence == "High" && good {
        ("address", "verified", vec![])
    } else {
        ("unknown", "estimate", vec![format!("CDTFA's address match confidence was \"{confidence}\"; treat this rate as an estimate.")])
    };
    notes.push("CDTFA doesn't state which period its rate data covers; it is re-checked each new quarter.".into());
    Ok((
        RateSetInput {
            location_id: 0,
            state_rate: None,
            county_rate: None,
            city_rate: None,
            district_rate: None,
            total_rate: Some(total),
            jurisdiction_label: format!("{} ({} County)", i["jurisdiction"].as_str().unwrap_or(""), i["county"].as_str().unwrap_or("").to_lowercase().split(' ').map(|w| { let mut c = w.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }).collect::<Vec<_>>().join(" ")),
            jurisdiction_code: i["tac"].as_str().map(String::from),
            source: "ca_cdtfa".into(),
            source_url: Some("https://www.cdtfa.ca.gov/taxes-and-fees/rates.aspx".into()),
            precision: precision.into(),
            status: status.into(),
            effective_date: None,
            dataset_period: None,
            retrieved_at: Some(fetched_at.into()),
            note: "Total rate from CDTFA (statewide base plus district taxes). CDTFA doesn't guarantee accuracy; confirm on its site.".into(),
        },
        matched,
        notes,
    ))
}

fn request(loc: &Location) -> AppResult<(&'static str, String, String)> {
    let provider = provider_for(&loc.state).ok_or_else(|| {
        AppError::Provider(format!("There's no verified free official rate lookup for {} yet. Enter the rate from your state revenue department's website.", if loc.state.is_empty() { "this location".into() } else { loc.state.clone() }))
    })?;
    if loc.address_line.trim().is_empty() || loc.postal_code.trim().is_empty() {
        return Err(AppError::Provider("Add the location's street address and ZIP code first; tax rates are set by exact address.".into()));
    }
    let key = format!("{}|{}|{}", loc.address_line.trim().to_lowercase(), loc.city.trim().to_lowercase(), loc.postal_code.trim());
    let url = match provider {
        "wa_dor" => reqwest::Url::parse_with_params(
            "https://webgis.dor.wa.gov/webapi/AddressRates.aspx",
            &[("output", "xml"), ("addr", loc.address_line.trim()), ("city", loc.city.trim()), ("zip", loc.postal_code.trim())],
        ),
        _ => reqwest::Url::parse_with_params(
            "https://services.maps.cdtfa.ca.gov/api/taxrate/GetRateByAddress",
            &[("address", loc.address_line.trim()), ("city", loc.city.trim()), ("zip", loc.postal_code.trim())],
        ),
    }
    .map_err(|e| AppError::Other(e.to_string()))?;
    Ok((provider, key, url.to_string()))
}

pub fn parse(f: &Fetched) -> AppResult<(RateSetInput, String, Vec<String>)> {
    match f.provider.as_str() {
        "wa_dor" => parse_wa(&f.body, &f.fetched_at),
        "ca_cdtfa" => parse_cdtfa(&f.body, &f.fetched_at),
        p => Err(AppError::msg(format!("Unknown tax provider {p}"))),
    }
}

/// Always asks the provider (rates change quarterly); the response is cached for offline review.
pub async fn lookup(state: &AppState, loc: &Location) -> AppResult<TaxLookup> {
    let (provider, key, url) = request(loc)?;
    let f = fetch(state, provider, &key, &url, &[], None).await?;
    let (mut rate, matched_address, notes) = parse(&f)?;
    rate.location_id = loc.id.unwrap_or_default();
    Ok(TaxLookup { provider: f.provider, request_key: f.request_key, fetched_at: f.fetched_at, from_cache: f.from_cache, rate, matched_address, notes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn washington_address_response() {
        let (r, matched, notes) = parse_wa(include_str!("fixtures/wa_dor_address.xml"), "2026-10-04T18:00:00Z").unwrap();
        assert_eq!(r.total_rate, Some(dec!(9.8)));
        assert_eq!(r.state_rate, Some(dec!(6.5)));
        assert_eq!(r.city_rate, Some(dec!(3.3)));
        assert_eq!(r.dataset_period.as_deref(), Some("Q42026"));
        assert_eq!(r.precision, "address");
        assert_eq!(r.jurisdiction_code.as_deref(), Some("3406"));
        assert!(matched.contains("LINDERSON"));
        assert!(notes.iter().any(|n| n.contains("adjusted the address")), "code 2 must ask the user to check the address");
    }

    #[test]
    fn washington_failure_is_not_zero() {
        let e = parse_wa(include_str!("fixtures/wa_dor_notfound.xml"), "x").unwrap_err().to_string();
        assert!(e.contains("Zipcode is invalid"), "{e}");
        let zip_only = r#"<response loccode="3406" localrate=".033" rate=".098" code="5"></response>"#;
        let (r, _, _) = parse_wa(zip_only, "x").unwrap();
        assert_eq!(r.precision, "zip5");
        assert_eq!(r.status, "estimate");
    }

    #[test]
    fn california_address_and_ambiguity() {
        let (r, matched, _) = parse_cdtfa(include_str!("fixtures/cdtfa_address.json"), "2026-10-04T18:00:00Z").unwrap();
        assert_eq!(r.total_rate, Some(dec!(8.75)));
        assert_eq!(r.precision, "address");
        assert_eq!(matched, "450 N St, Sacramento, CA 95814");
        let e = parse_cdtfa(include_str!("fixtures/cdtfa_ambiguous.json"), "x").unwrap_err().to_string();
        assert!(e.contains("more than one jurisdiction"), "{e}");
        assert!(parse_cdtfa("not json", "x").is_err());
    }

    #[test]
    fn unsupported_state_has_no_fake_lookup() {
        let mut loc = crate::db::core::Location {
            id: Some(1),
            name: "x".into(),
            address_line: "1 Main".into(),
            city: "Austin".into(),
            state: "TX".into(),
            postal_code: "78701".into(),
            latitude: None,
            longitude: None,
            geo_precision: None,
            geo_source: None,
            state_fips: None,
            county_fips: None,
            place_fips: None,
            archived: false,
        };
        assert!(request(&loc).is_err());
        loc.state = "WA".into();
        loc.address_line = String::new();
        assert!(request(&loc).is_err());
    }
}
