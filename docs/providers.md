# Data providers and APIs

All online features are optional and user-initiated, with one exception you can turn off: the daily
update check (below). The app sends only what a lookup needs and keeps working when a service is
unreachable. Facts below were checked against each provider's documentation and with live requests
on **2026-10-04** (tax, Census, BLS) and **2026-10-11** (OpenStreetMap, competitor websites, GitHub
Releases, search and directory services); re-check them before relying on them later.

## Summary

| Provider | Used for | Key | Limits (provider / this app) | Terms | Freshness | When it fails |
|---|---|---|---|---|---|---|
| Washington DOR address rate lookup | Official sales-tax rate for a WA address | None | None published / 200 a day | Documented by WA DOR for checkout and accounting software | Response states its tax period (e.g. `Q42026`) | Error shown; rate stays unset (never 0%); enter manually |
| California CDTFA tax rate API | Official sales-tax rate for a CA address | None | None published / 200 a day | CDTFA open-data terms: no accuracy guarantee; user responsible | No data period in the response; treated as stale once a new quarter starts | Same as above |
| U.S. Census Geocoder | Coordinates and state/county/place codes for an address | None | None published / 200 a day | Public domain | Current address ranges | Falls back to the bundled Gazetteer (below) |
| Census Gazetteer 2025 files (**bundled, offline**) | ZIP → ZCTA centre; state + city → place centre | — | — | Public domain | 2025 vintage | Used when there's no street address or the geocoder is unreachable; labelled "ZIP area centre" / "City centre" |
| U.S. Census Data API (ACS 5-year 2020–2024; County Business Patterns 2023) | Median household income, population, beauty-salon establishment counts (NAICS 812112) | **Free key required** | 200 a day (app) | Public domain; required notice shown in the app | Dataset vintage shown on every figure | Message explains the key requirement; nothing else depends on it |
| BLS Public Data API | Hairdresser wages (OEWS 39-5012), CPI for haircuts and personal care (CUUR0000SEGD02) | None for v1; optional free key (v2) | v1: 25 series/10 years per request; v2: 500 queries/day / app allows 25 a day without a key | Public domain | Annual wages; monthly CPI | Context panel shows the error; modeled CPI adjustment is simply omitted |
| OpenStreetMap Overpass API | Names, places, websites and phone numbers of hair, barber, nail, beauty (and optionally massage) businesses near a point | None | Fair use: under ~100 queries/day for regular use / app allows 50 a day, caches a week, tries 4 public servers in turn | ODbL; attribution "© OpenStreetMap contributors" shown | Live map data (mirrors may lag a few months) | Known competitors' websites are still checked; competitors can be added by hand or by website |
| Competitors' own websites | Prices they advertise, with page URL, time read and the exact text | None | robots.txt obeyed; ≥1.5 s between requests to a site (longer if it asks); ≤8 pages a site; 3 sites at a time / app allows 1,500 pages a day; a site is re-read after 7 days unless you refresh | The site's own terms; sites whose terms forbid automated reading are skipped (list below) | As of the time read | Reported per site (blocked, unreachable, no prices, PDF menu…); earlier prices kept |
| GitHub Releases (updates) | Whether a newer version exists, its notes, and the signed package | None | One check a day at startup (optional) and when you press *Check for updates* | Project's own releases | Each release | "No update information" / "couldn't check"; the app keeps working |

## Not available for free (checked 2026-10-11)

- **Salon menu prices from an API.** No free API provides them. Instead the app reads the prices
  each competitor publishes on **its own website** (see *Reading competitors' websites*), and you can
  still type prices in or import a CSV. When there aren't enough, the app says **"Insufficient
  observed market prices"** and keeps pricing from your costs.
- **Search engines.** Bing's search API was retired in 2025; Google's Custom Search JSON API is
  closed to new customers; Brave's API now needs a credit card (its free tier became a monthly credit
  in 2026). Search result pages themselves disallow automated visitors in robots.txt
  (`duckduckgo.com/html`, `search.yahoo.com/search`, `mojeek.com/search`). So the app doesn't use
  search engines.
- **Directories and booking platforms.** Yelp's API is paid; Foursquare's Places API gives about 500
  free calls a month and needs a billing account; Google Places needs billing. Booking platforms
  (Vagaro, Booksy, Fresha, StyleSeat, GlossGenius, Square Appointments…) and social networks forbid
  automated access in their terms or need a login. They're never read automatically; a competitor
  whose only website is one of them is listed as "Platform terms" so you can look yourself.
- **Guessing websites.** Trying likely domain names and checking the page found a matching site for
  1 of 25 salons without a website in OpenStreetMap, so it isn't done.
- **Sales-tax lookups outside WA and CA.** Other states don't offer a verified free address API that
  this app integrates. Enter the rate (and its components) from your state revenue department; it is
  labelled as entered by you, with the precision you choose.
- **Taxability rules.** The app doesn't ship tax rules. Each category starts as "not decided" until
  you record a decision and its source.

## Reading competitors' websites

- **What's read:** the competitor's website from OpenStreetMap, or one you add. The home page first,
  then up to 7 more pages on the same site chosen because their link text or address looks like a
  service menu (services, menu, prices, rates, treatments, hair, nails…), pages listed in its sitemap
  when the home page doesn't link to a menu, and "next page" links. Shop, blog, gallery, careers,
  contact and policy pages are skipped.
- **Politeness:** robots.txt is fetched first for every site and obeyed (the group for
  `SalonResourceManager`, else `*`; longest matching rule wins). If robots.txt can't be read because
  of a server error, the site is left alone. One request at a time per site with at least 1.5 seconds
  between requests (or the site's Crawl-delay, up to 30 seconds); three sites in parallel; 20 seconds
  per page and 90 seconds per site at most. User agent:
  `SalonResourceManager/<version> (+https://github.com/fleetwoodjohnr/salon-management; user-initiated price research)`.
- **Never bypassed:** a CAPTCHA, anti-bot challenge, "access denied" (HTTP 403) or "slow down"
  (HTTP 429) stops that site immediately and is reported. No JavaScript is run and no cookies kept.
- **Safety:** only http(s) on the standard ports; addresses must resolve to the public internet
  (never this computer or the local network, also checked on every redirect); redirects are followed
  within a site, and a move to another site is checked against that site's robots.txt and terms;
  pages are capped at 3 MB and only HTML is read (PDF menus are listed for you to open).
- **What counts as a price:** a dollar amount on the same line as a recognisable salon service (or
  the service named just before it in the same menu item). Fixed, "from"/"+" (starting at), ranges,
  stylist tiers and hair-length columns are kept apart; "by consultation" is recorded without an
  amount. Fees, deposits, gift cards, discounts, memberships, retail products and amounts outside
  $5–$2,000 are ignored. Nothing is ever filled in or guessed.
- **What's stored:** each price with the page URL, the time it was read, the exact text it came from,
  its normalised service (e.g. "Women's haircut") and whether it was matched to one of your services
  automatically. A later check replaces that business's website prices but keeps the ones you
  excluded or matched by hand. If a check fails (site down, blocked), earlier prices stay.

## Updates (GitHub Releases)

- The app asks `https://github.com/fleetwoodjohnr/salon-management/releases/latest/download/latest.json`
  whether a newer version exists: at most once a day at startup if *Check for updates automatically*
  is on (Settings → Application updates), and whenever you press *Check for updates*. Only a normal
  web request is made; nothing about you or your data is sent.
- Packages are verified against the project's public signing key (minisign/Ed25519, built into the
  app) before installation. Only published, non-prerelease releases count.

## How lookups work

- **Request de-duplication:** identical requests that overlap wait for the first one and reuse its
  result.
- **Caching:** responses are cached per workspace (geocodes for a year, Census for 30 days, BLS and
  Overpass for 7 days). An Overpass answer that reports an error inside a successful response is
  never cached. Website prices are stored as observations; a site is re-read after 7 days unless you
  ask for a refresh. Tax rates are always fetched fresh when you ask, then cached so the result can
  be reviewed offline. Applying an official rate re-reads the cached provider response in the core,
  so the interface can't alter what is recorded as official.
- **Retries:** up to two retries for timeouts, connection failures and server errors (not for
  refusals). HTTP 429 stops immediately with a "wait 30 seconds" message. Redirects are treated as
  errors (the Census API answers a missing key with a redirect). OpenStreetMap searches are POSTed
  and tried on `overpass-api.de`, `lz4.overpass-api.de`, `z.overpass-api.de` and
  `overpass.private.coffee` in turn: on 2026-10-11 the first answered 504 even to a one-node query and
  the last rejected GET requests, while the others worked.
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
  IDs (BLS), coordinates and a radius (Overpass), ordinary page requests to competitors' websites,
  and an update check to GitHub. Never client data.
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
Census Geocoder, BLS CPI and Washington OEWS wages, Overpass). Price-extraction tests use HTML
written to reproduce the layouts of real salon menus seen on 2026-10-11 (third-party pages aren't
copied into the repository); `cargo test live_ -- --ignored` runs the same code against live
OpenStreetMap servers and real salon websites. The Census Data API test uses a
hand-written sample in the documented format, because no key was available; live Census Data API
calls are therefore **not** verified.
