//! Optional online data providers. Nothing is sent unless the person asks for a lookup, and only
//! the minimum location/query data the provider needs (never client records).
//!
//! Shared plumbing: one HTTP client with timeouts, bounded retries, per-request de-duplication,
//! a per-workspace response cache, local daily quotas and an event log for the providers page.

pub mod bls;
pub mod census;
pub mod overpass;
pub mod tax;

use crate::db::now_utc;
use crate::error::{AppError, AppResult};
use crate::workspace::AppState;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

pub const USER_AGENT: &str = concat!("SalonResourceManager/", env!("CARGO_PKG_VERSION"), " (offline-first desktop app; user-initiated lookups)");
const KEYRING_SERVICE: &str = "salon-resource-manager";

#[derive(Serialize, Clone, Debug)]
pub struct ProviderInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub purpose: &'static str,
    pub gives: &'static str,
    pub coverage: &'static str,
    pub license: &'static str,
    pub limits: &'static str,
    pub docs_url: &'static str,
    /// Our own conservative daily cap per workspace.
    pub daily_cap: i64,
    pub key: Option<KeySpec>,
    pub attribution: Option<&'static str>,
}

#[derive(Serialize, Clone, Debug)]
pub struct KeySpec {
    pub required: bool,
    pub signup_url: &'static str,
    pub note: &'static str,
}

pub const PROVIDERS: &[ProviderInfo] = &[
    ProviderInfo {
        id: "wa_dor",
        name: "Washington Department of Revenue rate lookup",
        purpose: "Sales tax rate",
        gives: "Official combined state and local sales tax rate, location code and tax period for a Washington address",
        coverage: "Washington addresses only",
        license: "Public government service documented for use in checkout and accounting software",
        limits: "No published quota; this app allows 200 lookups a day",
        docs_url: "https://dor.wa.gov/taxes-rates/retail-sales-tax/destination-based-sales-tax-and-streamlined-sales-tax/wa-sales-tax-rate-lookup-url-interface",
        daily_cap: 200,
        key: None,
        attribution: None,
    },
    ProviderInfo {
        id: "ca_cdtfa",
        name: "California CDTFA tax rate API",
        purpose: "Sales tax rate",
        gives: "Official sales and use tax rate and jurisdiction for a California address",
        coverage: "California addresses only",
        license: "CDTFA open data terms: no accuracy guarantee; you remain responsible for the rates you charge",
        limits: "No published quota; this app allows 200 lookups a day",
        docs_url: "https://www.cdtfa.ca.gov/dataportal/policy.htm",
        daily_cap: 200,
        key: None,
        attribution: None,
    },
    ProviderInfo {
        id: "census_geocoder",
        name: "U.S. Census Geocoder",
        purpose: "Location",
        gives: "Coordinates, state, county and place for a street address",
        coverage: "United States street addresses",
        license: "Public domain (U.S. government work)",
        limits: "No key; this app allows 200 lookups a day",
        docs_url: "https://geocoding.geo.census.gov/geocoder/",
        daily_cap: 200,
        key: None,
        attribution: None,
    },
    ProviderInfo {
        id: "census_data",
        name: "U.S. Census Data API (ACS and County Business Patterns)",
        purpose: "Local market context",
        gives: "Median household income and population (ACS 5-year), count of beauty salon establishments (NAICS 812112). Context only — not salon prices.",
        coverage: "United States: ZIP Code Tabulation Areas, counties",
        license: "Public domain; required notice: \"This product uses the Census Bureau Data API but is not endorsed or certified by the Census Bureau.\"",
        limits: "Requires a free API key (requests without one are refused); this app allows 200 lookups a day",
        docs_url: "https://www.census.gov/data/developers.html",
        daily_cap: 200,
        key: Some(KeySpec { required: true, signup_url: "https://api.census.gov/data/key_signup.html", note: "Free; sent by email after a short form" }),
        attribution: Some("This product uses the Census Bureau Data API but is not endorsed or certified by the Census Bureau."),
    },
    ProviderInfo {
        id: "bls",
        name: "U.S. Bureau of Labor Statistics Public Data API",
        purpose: "Wage and price-trend context",
        gives: "Hourly wages for hairdressers and cosmetologists (OEWS 39-5012) and the consumer price index for haircuts and personal care services. Context only — not salon prices.",
        coverage: "United States and states",
        license: "Public domain (U.S. government work)",
        limits: "Version 1 needs no key (up to 25 series and 10 years per request); version 2 needs free registration and allows 500 queries a day. This app allows 25 a day without a key",
        docs_url: "https://www.bls.gov/bls/api_features.htm",
        daily_cap: 25,
        key: Some(KeySpec { required: false, signup_url: "https://data.bls.gov/registrationEngine/", note: "Optional; raises the limit" }),
        attribution: None,
    },
    ProviderInfo {
        id: "overpass",
        name: "OpenStreetMap Overpass API",
        purpose: "Competitor discovery",
        gives: "Names and locations of hair and beauty salons mapped in OpenStreetMap near you. No prices.",
        coverage: "Worldwide; completeness varies by area",
        license: "© OpenStreetMap contributors, Open Database License (ODbL); attribution required",
        limits: "Fair use: under 100 queries a day for regular use; this app allows 50 a day",
        docs_url: "https://wiki.openstreetmap.org/wiki/Overpass_API",
        daily_cap: 50,
        key: None,
        attribution: Some("Data © OpenStreetMap contributors, available under the Open Database License (ODbL)."),
    },
];

pub fn info(id: &str) -> AppResult<&'static ProviderInfo> {
    PROVIDERS.iter().find(|p| p.id == id).ok_or_else(|| AppError::msg(format!("Unknown provider {id}")))
}

// ---------------------------------------------------------------- secrets

fn keyring_entry(provider: &str) -> AppResult<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, &format!("{provider}_api_key")).map_err(|e| {
        AppError::Provider(format!(
            "The system keyring isn't available ({e}). API keys are only stored in the OS keyring (Secret Service on Linux, Keychain on macOS, Credential Manager on Windows), so this provider can't be given a key on this computer."
        ))
    })
}

pub fn key_get(provider: &str) -> AppResult<Option<String>> {
    match keyring_entry(provider)?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::Provider(format!("Couldn't read the API key from the keyring: {e}"))),
    }
}

pub fn key_set(provider: &str, key: &str) -> AppResult<()> {
    let key = key.trim();
    if key.is_empty() || key.len() > 200 || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(AppError::invalid("key", "That doesn't look like an API key (letters, digits, - and _ only)."));
    }
    keyring_entry(provider)?.set_password(key).map_err(|e| AppError::Provider(format!("Couldn't save the key in the keyring: {e}")))
}

pub fn key_clear(provider: &str) -> AppResult<()> {
    match keyring_entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Provider(format!("Couldn't remove the key: {e}"))),
    }
}

/// Remove secret values from any text before it is shown or logged.
pub fn redact(text: &str, secrets: &[&str]) -> String {
    let mut out = text.to_string();
    for s in secrets.iter().filter(|s| s.len() >= 4) {
        out = out.replace(s, "[redacted]");
    }
    out
}

// ---------------------------------------------------------------- HTTP

pub struct Http {
    client: reqwest::Client,
    inflight: tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl Default for Http {
    fn default() -> Self {
        Self::new()
    }
}

impl Http {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            // Census answers a missing key with a redirect to an HTML page; treat redirects as errors.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("HTTP client");
        Http { client, inflight: tokio::sync::Mutex::new(HashMap::new()) }
    }

    /// GET with up to two retries for timeouts, connection errors and 5xx (not 4xx).
    async fn get_text(&self, url: &str, secrets: &[&str]) -> AppResult<String> {
        let mut last = String::new();
        for attempt in 0..3u64 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(800 * attempt * attempt)).await;
            }
            match self.client.get(url).send().await {
                Ok(r) if r.status().is_success() => return r.text().await.map_err(|e| AppError::Provider(redact(&format!("Reading the response failed: {e}"), secrets))),
                Ok(r) if r.status().as_u16() == 429 => {
                    return Err(AppError::Provider("The provider is rate-limiting requests. Wait at least 30 seconds before trying again.".into()))
                }
                Ok(r) if r.status().is_server_error() => last = format!("The provider returned an error ({}).", r.status()),
                Ok(r) if r.status().is_redirection() => {
                    let to = r.headers().get("location").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
                    return Err(AppError::Provider(redact(&format!("The provider redirected the request ({}){}", r.status(), if to.contains("missing_key") || to.contains("invalid_key") { ": the API key is missing or invalid." } else { "." }), secrets)));
                }
                Ok(r) => return Err(AppError::Provider(format!("The provider refused the request ({}).", r.status()))),
                Err(e) if e.is_timeout() => last = "The provider didn't answer in time.".into(),
                Err(e) if e.is_connect() => last = "Couldn't connect. Check your internet connection.".into(),
                Err(e) => return Err(AppError::Provider(redact(&format!("Request failed: {e}"), secrets))),
            }
        }
        Err(AppError::Provider(last))
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Fetched {
    pub provider: String,
    pub request_key: String,
    pub body: String,
    pub fetched_at: String,
    pub from_cache: bool,
}

pub fn cached(state: &AppState, provider: &str, request_key: &str) -> AppResult<Option<Fetched>> {
    state.read(|c| {
        Ok(c.query_row(
            "SELECT response, fetched_at FROM provider_cache WHERE provider = ?1 AND request_key = ?2",
            params![provider, request_key],
            |r| Ok(Fetched { provider: provider.into(), request_key: request_key.into(), body: r.get(0)?, fetched_at: r.get(1)?, from_cache: true }),
        )
        .optional()?)
    })
}

fn record(state: &AppState, provider: &str, ok: bool, message: &str) -> AppResult<()> {
    let day = crate::db::tax::today().to_string();
    state.tx(|c| {
        c.execute(
            "INSERT INTO provider_usage (provider, day, requests, errors) VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(provider, day) DO UPDATE SET requests = requests + 1, errors = errors + excluded.errors",
            params![provider, day, (!ok) as i64],
        )?;
        c.execute("INSERT INTO provider_events (provider, ok, message) VALUES (?1, ?2, ?3)", params![provider, ok as i64, message])?;
        c.execute("DELETE FROM provider_events WHERE provider = ?1 AND id NOT IN (SELECT id FROM provider_events WHERE provider = ?1 ORDER BY id DESC LIMIT 50)", [provider])?;
        Ok(())
    })
}

fn used_today(state: &AppState, provider: &str) -> AppResult<i64> {
    let day = crate::db::tax::today().to_string();
    state.read(|c| Ok(c.query_row("SELECT COALESCE(SUM(requests), 0) FROM provider_usage WHERE provider = ?1 AND day = ?2", params![provider, day], |r| r.get(0))?))
}

/// Fetch through cache → quota → network. `request_key` must not contain secrets; `url` may.
/// `max_age_hours`: reuse a cached response younger than this (None = always refetch).
pub async fn fetch(state: &AppState, provider: &str, request_key: &str, url: &str, secrets: &[&str], max_age_hours: Option<i64>) -> AppResult<Fetched> {
    let p = info(provider)?;
    let lock = {
        let mut m = state.http.inflight.lock().await;
        m.entry(format!("{provider}|{request_key}")).or_default().clone()
    };
    let _guard = lock.lock().await; // identical requests wait here and then hit the cache
    if let (Some(max), Some(hit)) = (max_age_hours, cached(state, provider, request_key)?) {
        let fresh = chrono::DateTime::parse_from_rfc3339(&hit.fetched_at).map(|t| (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_hours() < max).unwrap_or(false);
        if fresh {
            return Ok(hit);
        }
    }
    if used_today(state, provider)? >= p.daily_cap {
        return Err(AppError::Provider(format!("Today's limit of {} lookups for {} is used up. Cached results still work; try again tomorrow.", p.daily_cap, p.name)));
    }
    match state.http.get_text(url, secrets).await {
        Ok(body) => {
            let fetched_at = now_utc();
            state.tx(|c| {
                c.execute(
                    "INSERT INTO provider_cache (provider, request_key, response, fetched_at) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(provider, request_key) DO UPDATE SET response = excluded.response, fetched_at = excluded.fetched_at",
                    params![provider, request_key, body, fetched_at],
                )?;
                Ok(())
            })?;
            record(state, provider, true, "Lookup succeeded")?;
            Ok(Fetched { provider: provider.into(), request_key: request_key.into(), body, fetched_at, from_cache: false })
        }
        Err(e) => {
            let msg = redact(&e.to_string(), secrets);
            record(state, provider, false, &msg)?;
            Err(AppError::Provider(msg))
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ProviderStatus {
    pub info: ProviderInfo,
    pub key_configured: Option<bool>,
    pub key_error: Option<String>,
    pub requests_today: i64,
    pub errors_today: i64,
    pub last_success: Option<String>,
    pub last_error: Option<String>,
    pub last_error_at: Option<String>,
    pub cached_responses: i64,
    pub newest_cache: Option<String>,
}

pub fn status(state: &AppState) -> AppResult<Vec<ProviderStatus>> {
    let day = crate::db::tax::today().to_string();
    PROVIDERS
        .iter()
        .map(|p| {
            let (key_configured, key_error) = match &p.key {
                None => (None, None),
                Some(_) => match key_get(p.id) {
                    Ok(k) => (Some(k.is_some()), None),
                    Err(e) => (Some(false), Some(e.to_string())),
                },
            };
            state.read(|c| {
                let (req, err): (i64, i64) = c
                    .query_row("SELECT requests, errors FROM provider_usage WHERE provider = ?1 AND day = ?2", params![p.id, day], |r| Ok((r.get(0)?, r.get(1)?)))
                    .optional()?
                    .unwrap_or((0, 0));
                let last_success: Option<String> = c.query_row("SELECT MAX(at) FROM provider_events WHERE provider = ?1 AND ok = 1", [p.id], |r| r.get(0))?;
                let last_err: Option<(String, String)> = c
                    .query_row("SELECT message, at FROM provider_events WHERE provider = ?1 AND ok = 0 ORDER BY id DESC LIMIT 1", [p.id], |r| Ok((r.get(0)?, r.get(1)?)))
                    .optional()?;
                let (n, newest): (i64, Option<String>) = c.query_row("SELECT COUNT(*), MAX(fetched_at) FROM provider_cache WHERE provider = ?1", [p.id], |r| Ok((r.get(0)?, r.get(1)?)))?;
                Ok(ProviderStatus {
                    info: p.clone(),
                    key_configured,
                    key_error: key_error.clone(),
                    requests_today: req,
                    errors_today: err,
                    last_success,
                    last_error: last_err.as_ref().map(|x| x.0.clone()),
                    last_error_at: last_err.map(|x| x.1),
                    cached_responses: n,
                    newest_cache: newest,
                })
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction() {
        assert_eq!(redact("https://x?key=abcd1234&q=1 failed", &["abcd1234"]), "https://x?key=[redacted]&q=1 failed");
        assert_eq!(redact("short", &["ab"]), "short");
    }

    /// Needs a desktop session with a Secret Service (GNOME Keyring / KWallet):
    /// `cargo test keyring_round_trip -- --ignored`
    #[test]
    #[ignore]
    fn keyring_round_trip() {
        key_set("srm_selftest", "abc123-test_KEY").unwrap();
        assert_eq!(key_get("srm_selftest").unwrap().as_deref(), Some("abc123-test_KEY"));
        key_clear("srm_selftest").unwrap();
        assert_eq!(key_get("srm_selftest").unwrap(), None);
        assert!(key_set("srm_selftest", "bad key with spaces").is_err());
    }

    #[tokio::test]
    async fn connection_failure_is_an_error_not_a_value() {
        let h = Http::new();
        // Port 9 (discard) on localhost is closed in test environments.
        let e = h.get_text("http://127.0.0.1:9/", &[]).await.unwrap_err().to_string();
        assert!(e.contains("connect") || e.contains("Request failed"), "{e}");
    }
}
