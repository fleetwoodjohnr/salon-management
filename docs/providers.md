# Data providers and APIs

All online features are optional and user-initiated. The app never contacts a service in the
background, sends only what the lookup needs, and keeps working when a service is unreachable.
Facts below were checked against each provider's documentation and with live requests on
**2026-10-04**; re-check them before relying on them later.

## Summary

| Provider | Used for | Key | Limits (provider / this app) | Terms | Freshness | When it fails |
|---|---|---|---|---|---|---|
| Washington DOR address rate lookup | Official sales-tax rate for a WA address | None | None published / 200 a day | Documented by WA DOR for checkout and accounting software | Response states its tax period (e.g. `Q42026`) | Error shown; rate stays unset (never 0%); enter manually |
| California CDTFA tax rate API | Official sales-tax rate for a CA address | None | None published / 200 a day | CDTFA open-data terms: no accuracy guarantee; user responsible | No data period in the response; treated as stale once a new quarter starts | Same as above |
| U.S. Census Geocoder | Coordinates and state/county/place codes for an address | None | None published / 200 a day | Public domain | Current address ranges | Falls back to the bundled Gazetteer (below) |
| Census Gazetteer 2025 files (**bundled, offline**) | ZIP → ZCTA centre; state + city → place centre | — | — | Public domain | 2025 vintage | Used when there's no street address or the geocoder is unreachable; labelled "ZIP area centre" / "City centre" |
| U.S. Census Data API (ACS 5-year 2020–2024; County Business Patterns 2023) | Median household income, population, beauty-salon establishment counts (NAICS 812112) | **Free key required** | 200 a day (app) | Public domain; required notice shown in the app | Dataset vintage shown on every figure | Message explains the key requirement; nothing else depends on it |
| BLS Public Data API | Hairdresser wages (OEWS 39-5012), CPI for haircuts and personal care (CUUR0000SEGD02) | None for v1; optional free key (v2) | v1: 25 series/10 years per request; v2: 500 queries/day / app allows 25 a day without a key | Public domain | Annual wages; monthly CPI | Context panel shows the error; modeled CPI adjustment is simply omitted |
| OpenStreetMap Overpass API | Names and locations of hair/beauty shops near you | None | Fair use: under ~100 queries/day for regular use; wait 30 s after HTTP 429 / app allows 50 a day, caches a week | ODbL; attribution "© OpenStreetMap contributors" shown | Live map data | Competitors can still be added by hand |

## Not available for free

- **Salon menu prices.** No free API provides them. Map/review platforms either have no prices or
  require a billing account (Google Places needs billing enabled; Yelp's API is paid). The app does
  not scrape websites. Prices are collected by you — typed in or imported from CSV — with source URL
  and date, and kept separate from statistics and modeled estimates. When there aren't enough, the
  app says **"Insufficient observed market prices"** and keeps pricing from your costs.
- **Sales-tax lookups outside WA and CA.** Other states don't offer a verified free address API that
  this app integrates. Enter the rate (and its components) from your state revenue department; it is
  labelled as entered by you, with the precision you choose.
- **Taxability rules.** The app doesn't ship tax rules. Each category starts as "not decided" until
  you record a decision and its source.

## How lookups work

- **Request de-duplication:** identical requests that overlap wait for the first one and reuse its
  result.
- **Caching:** responses are cached per workspace (geocodes for a year, Census for 30 days, BLS and
  Overpass for 7 days). Tax rates are always fetched fresh when you ask, then cached so the result can
  be reviewed offline. Applying an official rate re-reads the cached provider response in the core,
  so the interface can't alter what is recorded as official.
- **Retries:** up to two retries for timeouts, connection failures and server errors (not for
  refusals). HTTP 429 stops immediately with a "wait 30 seconds" message. Redirects are treated as
  errors (the Census API answers a missing key with a redirect).
- **Timeouts:** 10 s to connect, 20 s overall.
- **Quotas:** each workspace counts its own daily requests and stops at the caps above.
- **Status page:** Settings → Data providers shows coverage, limits, terms, today's usage and
  failures, last success, last error and cached results.
- **Secrets:** API keys are stored only in the operating system's credential store (Secret Service
  on Linux via the `keyring` crate, macOS Keychain, Windows Credential Manager). The interface only
  learns whether a key is configured. Keys are removed from any error text, never logged, and never
  written to the database, backups or exports. If no credential store is available, key entry is
  refused with an explanation.
- **What's sent:** an address/city/ZIP (tax and geocoding), ZIP/county/state codes (Census), series
  IDs (BLS), coordinates and a radius (Overpass). Never client data.
- **User agent:** `SalonResourceManager/<version> (offline-first desktop app; user-initiated lookups)`.

## Sales tax specifics

- **Washington DOR** result codes: 0 = address found (verified, address precision); 2 and 4 = address
  adjusted/corrected (verified, with a "check the matched address" note); 1 and 3 = ZIP+4 only
  (estimate); 5 = 5-digit ZIP only (estimate); 6, 7, 9 = errors. The local rate is WA's combined
  local component (city, county, transit).
- **California CDTFA**: one jurisdiction match with "Good" match codes and "High" confidence is
  recorded as verified; anything else is an estimate. If the address matches more than one
  jurisdiction, the lookup is refused and the candidates are listed.
- **Staleness** is judged by the provider's data period, not by when you fetched it: a rate fetched
  today for last quarter's period is shown as stale.

## Real-world data quirks found while testing

- BLS reports October 2025 CPI as "-" with the footnote "Data unavailable due to the 2025 lapse in
  appropriations". Missing months are skipped, never treated as zero.
- CDTFA geocoded a nonsense test address to a real address that spans two jurisdictions; this is why
  multi-jurisdiction matches are refused and the matched address is always shown.

## Test fixtures

`src-tauri/src/providers/fixtures/` holds real responses captured on 2026-10-04 (WA DOR, CDTFA,
Census Geocoder, BLS CPI and Washington OEWS wages, Overpass). The Census Data API test uses a
hand-written sample in the documented format, because no key was available; live Census Data API
calls are therefore **not** verified.
