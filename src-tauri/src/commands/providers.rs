//! Online providers: status page, API keys (OS keyring), tax-rate and address lookups.

use crate::db::core::{self as core, Location};
use crate::db::tax::{self, RateSet};
use crate::error::{AppError, AppResult};
use crate::providers::{self, census, tax as taxp, ProviderStatus};
use crate::workspace::AppState;
use tauri::State;

#[tauri::command]
pub async fn providers_status(state: State<'_, AppState>) -> AppResult<Vec<ProviderStatus>> {
    providers::status(&state)
}

/// Store a key in the OS keyring. The key is never returned to the UI.
#[tauri::command]
pub async fn provider_key_set(provider: String, key: String) -> AppResult<()> {
    if providers::info(&provider)?.key.is_none() {
        return Err(AppError::msg("This provider doesn't use a key."));
    }
    providers::key_set(&provider, &key)
}

#[tauri::command]
pub async fn provider_key_clear(provider: String) -> AppResult<()> {
    providers::key_clear(&provider)
}

#[tauri::command]
pub async fn tax_lookup(state: State<'_, AppState>, location_id: i64) -> AppResult<taxp::TaxLookup> {
    let loc = state.read(|c| core::location(c, location_id))?;
    taxp::lookup(&state, &loc).await
}

/// Save the cached result of a lookup as the location's current rate. The rate is rebuilt from
/// the cached provider response here, so the UI can't alter what gets recorded as "official".
#[tauri::command]
pub async fn tax_rate_apply_lookup(state: State<'_, AppState>, location_id: i64, provider: String, request_key: String) -> AppResult<RateSet> {
    let f = providers::cached(&state, &provider, &request_key)?.ok_or_else(|| AppError::msg("That lookup is no longer available; look the rate up again."))?;
    let (mut rate, matched, _) = taxp::parse(&f)?;
    rate.location_id = location_id;
    if !matched.is_empty() {
        rate.note = format!("{} Matched address: {matched}.", rate.note);
    }
    state.tx(|c| {
        let id = tax::save_rate(c, &rate)?;
        tax::rate_by_id(c, id)
    })
}

/// Locate a business location: the Census Geocoder for a street address (online), otherwise — or if
/// that fails — the bundled Gazetteer for the ZIP code or city (offline, area centre).
#[tauri::command]
pub async fn location_geocode(state: State<'_, AppState>, location_id: i64) -> AppResult<Location> {
    let loc = state.read(|c| core::location(c, location_id))?;
    let online = if loc.address_line.trim().is_empty() { None } else { Some(census::geocode(&state, &loc).await) };
    let update = |l: &mut Location| -> AppResult<()> {
        match &online {
            Some(Ok(g)) => {
                l.latitude = Some(g.latitude.clone());
                l.longitude = Some(g.longitude.clone());
                l.geo_precision = Some("address".into());
                l.geo_source = Some(format!("Census Geocoder: {}", g.matched_address));
                l.state_fips = g.state_fips.clone();
                l.county_fips = g.county_fips.clone();
                l.place_fips = g.place_fips.clone();
            }
            other => {
                let p = crate::gazetteer::zcta(&loc.postal_code).or_else(|| crate::gazetteer::place(&loc.state, &loc.city)).ok_or_else(|| match other {
                    Some(Err(e)) => AppError::Provider(format!("{e} No ZIP or city match in the offline list either.")),
                    _ => AppError::msg("Enter a street address, a 5-digit ZIP code, or a city and state to locate this place."),
                })?;
                l.latitude = Some(p.latitude);
                l.longitude = Some(p.longitude);
                l.geo_precision = Some(p.precision.into());
                l.geo_source = Some(match other {
                    Some(Err(_)) => format!("{} (online address lookup failed)", p.label),
                    _ => p.label,
                });
            }
        }
        Ok(())
    };
    state.tx(|c| {
        let mut l = core::location(c, location_id)?;
        update(&mut l)?;
        core::save_location(c, &l)?;
        core::location(c, location_id)
    })
}
