//! BLS Public Data API: OEWS hourly wages for hairdressers/cosmetologists (39-5012) and the CPI for
//! haircuts and other personal care services. Context only — these are not salon prices.

use super::{fetch, key_get};
use crate::error::{AppError, AppResult};
use crate::workspace::AppState;
use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::HashMap;

pub const CPI_SERIES: &str = "CUUR0000SEGD02";

#[derive(Serialize, Clone, Debug)]
pub struct SeriesPoint {
    pub year: String,
    pub period: String,
    pub value: Decimal,
}

pub fn parse(body: &str) -> AppResult<Vec<SeriesPoint>> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|_| AppError::Provider("BLS sent an unreadable response.".into()))?;
    if v["status"] != "REQUEST_SUCCEEDED" {
        let msg = v["message"].as_array().map(|m| m.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" ")).unwrap_or_default();
        return Err(AppError::Provider(format!("BLS refused the request. {msg}")));
    }
    let data = v["Results"]["series"][0]["data"].as_array().cloned().unwrap_or_default();
    // Missing months come back as "-" with a footnote (e.g. October 2025, when data collection
    // lapsed); they are skipped rather than treated as zero.
    Ok(data
        .iter()
        .filter_map(|d| {
            Some(SeriesPoint {
                year: d["year"].as_str()?.into(),
                period: d["period"].as_str()?.into(),
                value: d["value"].as_str()?.parse().ok()?,
            })
        })
        .collect())
}

/// Monthly CPI as "YYYY-MM" → index value.
pub fn cpi_map(points: &[SeriesPoint]) -> HashMap<String, Decimal> {
    points
        .iter()
        .filter(|p| p.period.starts_with('M') && p.period != "M13")
        .map(|p| (format!("{}-{}", p.year, &p.period[1..]), p.value))
        .collect()
}

pub fn oews_state_series(state_fips: &str) -> String {
    format!("OEUS{state_fips}0000000000039501203")
}

async fn series(state: &AppState, id: &str) -> AppResult<Vec<SeriesPoint>> {
    let key = key_get("bls").ok().flatten();
    let (url, secrets) = match &key {
        Some(k) => (format!("https://api.bls.gov/publicAPI/v2/timeseries/data/{id}?registrationkey={k}"), vec![k.as_str()]),
        None => (format!("https://api.bls.gov/publicAPI/v1/timeseries/data/{id}"), vec![]),
    };
    let f = fetch(state, "bls", id, &url, &secrets, Some(24 * 7)).await?;
    parse(&f.body)
}

pub async fn cpi(state: &AppState) -> AppResult<Vec<SeriesPoint>> {
    series(state, CPI_SERIES).await
}

pub async fn wages(state: &AppState, state_fips: Option<&str>) -> AppResult<Vec<(String, Vec<SeriesPoint>)>> {
    let mut out = vec![("United States".to_string(), series(state, "OEUN000000000000039501203").await?)];
    if let Some(f) = state_fips {
        out.push((format!("State FIPS {f}"), series(state, &oews_state_series(f)).await?));
    }
    Ok(out)
}

/// Cached CPI (no network) for modeled adjustments.
pub fn cached_cpi(state: &AppState) -> AppResult<Option<HashMap<String, Decimal>>> {
    Ok(match super::cached(state, "bls", CPI_SERIES)? {
        Some(f) => parse(&f.body).ok().map(|p| cpi_map(&p)),
        None => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn parses_real_series() {
        let cpi = parse(include_str!("fixtures/bls_cpi.json")).unwrap();
        let m = cpi_map(&cpi);
        assert_eq!(m.get("2026-08"), Some(&dec!(417.820)));
        assert!(!m.contains_key("2025-10"), "a missing month must not become a value");
        let wa = parse(include_str!("fixtures/bls_oews_wa.json")).unwrap();
        assert_eq!(wa[0].value, dec!(30.67));
        assert_eq!(oews_state_series("53"), "OEUS530000000000039501203");
        assert!(parse(r#"{"status":"REQUEST_NOT_PROCESSED","message":["Daily threshold"]}"#).is_err());
    }
}
