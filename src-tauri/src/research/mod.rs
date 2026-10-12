//! Automatic market research: find competitors on OpenStreetMap, read the prices they advertise
//! on their own websites, and keep a record of what was checked and why something gave no prices.
//!
//! A run happens in the background (the UI follows `research-progress` events) and survives
//! partial failure: if the map servers are down, known competitors' websites are still checked;
//! if a website fails, the others continue. Every write first checks that the workspace the run
//! started in is still the open one.

pub mod catalog;
pub mod crawl;
pub mod extract;
pub mod http;
pub mod robots;

use crate::db::{self, market, research as store};
use crate::domain::market::haversine_km;
use crate::error::{AppError, AppResult};
use crate::providers::overpass;
use crate::workspace::AppState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Websites checked per run, nearest first. Recently checked sites are skipped next time, so
/// running again continues with the rest.
pub const MAX_SITES: usize = 60;
pub const RECHECK_AFTER_DAYS: i64 = 7;
const PARALLEL_SITES: usize = 3;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Center {
    /// One of your saved locations (must have been located).
    Location { id: i64 },
    /// A ZIP code, placed at its area centre from the bundled Census Gazetteer (offline).
    Zip { zip: String },
    City { city: String, state: String },
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ResearchParams {
    pub center: Center,
    pub radius_km: f64,
    pub types: Vec<String>,
    /// Read competitors' websites for prices (otherwise discovery only).
    pub check_websites: bool,
    /// Re-check websites checked in the last week.
    pub refresh: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Resolved {
    pub lat: f64,
    pub lon: f64,
    pub label: String,
    /// An area centre rather than an address.
    pub approximate: bool,
}

pub fn resolve_center(conn: &rusqlite::Connection, c: &Center) -> AppResult<Resolved> {
    let num = |s: &str| s.parse::<f64>().map_err(|_| AppError::msg("That place has unreadable coordinates."));
    match c {
        Center::Location { id } => {
            let l = db::core::location(conn, *id)?;
            match (l.latitude.as_deref(), l.longitude.as_deref()) {
                (Some(a), Some(b)) => Ok(Resolved {
                    lat: num(a)?,
                    lon: num(b)?,
                    label: [l.name.as_str(), l.address_line.as_str(), l.city.as_str()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(", "),
                    approximate: l.geo_precision.as_deref().is_some_and(|p| p != "address" && p != "rooftop"),
                }),
                _ => Err(AppError::msg("Locate this location first (Settings → Locations → Locate), or search around a ZIP code or city instead.")),
            }
        }
        Center::Zip { zip } => {
            let p = crate::gazetteer::zcta(zip.trim()).ok_or_else(|| AppError::invalid("zip", "That ZIP code isn't in the bundled Census list. Check it, or search around a city."))?;
            Ok(Resolved { lat: num(&p.latitude)?, lon: num(&p.longitude)?, label: format!("ZIP {} (area centre)", zip.trim()), approximate: true })
        }
        Center::City { city, state } => {
            let p = crate::gazetteer::place(state, city).ok_or_else(|| AppError::invalid("city", "That city isn't in the bundled Census list. Check the spelling and the two-letter state."))?;
            Ok(Resolved { lat: num(&p.latitude)?, lon: num(&p.longitude)?, label: format!("{}, {} (city centre)", city.trim(), state.trim().to_uppercase()), approximate: true })
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SiteLine {
    pub competitor_id: i64,
    pub name: String,
    pub url: String,
    /// research_pages status for the site as a whole
    pub status: String,
    pub prices: usize,
    pub note: String,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct SourceCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub center_label: String,
    pub radius_km: f64,
    pub types: Vec<String>,
    pub sources: Vec<SourceCheck>,
    /// Businesses the map search returned (named ones).
    pub discovered: usize,
    /// Map features without a name, which can't be identified.
    pub unnamed: usize,
    pub added: usize,
    pub merged: usize,
    /// Your active competitors within the radius (map search plus ones you added).
    pub in_radius: usize,
    /// Competitors you added without a location (checked, distance unknown).
    pub no_location: usize,
    pub with_website: usize,
    pub websites_checked: usize,
    pub skipped_recent: usize,
    /// Left for the next run because of the per-run limit.
    pub not_checked_limit: usize,
    pub with_prices: usize,
    pub prices_saved: usize,
    /// Saved prices for services you don't offer (or that couldn't be matched to yours).
    pub unmatched_prices: usize,
    /// Priced lines that weren't recognisable salon services (dropped).
    pub unrecognized: usize,
    pub consultation_only: usize,
    /// Site outcome → number of sites.
    pub site_statuses: BTreeMap<String, usize>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Progress {
    pub run_id: i64,
    pub running: bool,
    /// locating | discovering | selecting | checking_websites | done | cancelled | failed
    pub stage: String,
    pub message: String,
    pub sites_total: usize,
    pub sites_done: usize,
    /// Businesses whose websites are being read right now.
    pub current: Vec<String>,
    pub sites: Vec<SiteLine>,
    pub summary: Summary,
    pub error: Option<String>,
    /// The workspace the search ran in (its results belong there).
    #[serde(skip)]
    pub workspace_id: String,
}

/// The one research run allowed at a time, and the latest progress (kept after it ends).
#[derive(Default)]
pub struct Runner {
    progress: Mutex<Option<Progress>>,
    cancel: Arc<AtomicBool>,
}

impl Runner {
    pub fn snapshot(&self) -> Option<Progress> {
        self.progress.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    /// The latest search for this workspace (a running one always shows, wherever it started).
    pub fn snapshot_for(&self, workspace_id: Option<&str>) -> Option<Progress> {
        self.snapshot().filter(|p| p.running || Some(p.workspace_id.as_str()) == workspace_id)
    }
    pub fn is_running(&self) -> bool {
        self.snapshot().is_some_and(|p| p.running)
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    fn update(&self, f: impl FnOnce(&mut Progress)) -> Option<Progress> {
        let mut g = self.progress.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(p) = g.as_mut() {
            f(p);
        }
        g.clone()
    }
}

fn guard(state: &AppState, ws_id: &str) -> AppResult<()> {
    match state.current_info() {
        Some(w) if w.id == ws_id => Ok(()),
        _ => Err(AppError::Conflict("The workspace was closed or switched during the search, so it stopped. Results gathered so far were saved.".into())),
    }
}

pub fn validate(p: &ResearchParams) -> AppResult<()> {
    if !(0.5..=50.0).contains(&p.radius_km) {
        return Err(AppError::invalid("radius_km", "Choose a search radius between 0.3 and 31 miles (0.5 to 50 km)."));
    }
    if p.types.is_empty() || p.types.iter().any(|t| !crate::prefs::BUSINESS_TYPES.contains(&t.as_str())) {
        return Err(AppError::invalid("types", "Choose at least one kind of business."));
    }
    Ok(())
}

/// Validate, resolve the centre, record the run and mark it running. The caller spawns `run`.
pub fn start(state: &AppState, p: &ResearchParams) -> AppResult<(i64, String, Resolved)> {
    validate(p)?;
    if state.research.is_running() {
        return Err(AppError::Conflict("A market search is already running. Wait for it to finish or stop it first.".into()));
    }
    let ws = state.current_info().ok_or_else(|| AppError::NoWorkspace("Open a workspace first.".into()))?;
    let center = state.read(|c| resolve_center(c, &p.center))?;
    let run_id = state.tx(|c| store::create_run(c, &center.label, center.lat, center.lon, p.radius_km, &p.types))?;
    // Remember the search settings for next time (best effort).
    let mut prefs = state.prefs();
    prefs.research_radius_km = p.radius_km;
    prefs.research_types = p.types.clone();
    let _ = crate::prefs::save(&state.root, &prefs);
    state.research.cancel.store(false, Ordering::Relaxed);
    *state.research.progress.lock().unwrap_or_else(|e| e.into_inner()) = Some(Progress {
        run_id,
        running: true,
        stage: "locating".into(),
        message: format!("Searching around {}", center.label),
        sites_total: 0,
        sites_done: 0,
        current: vec![],
        sites: vec![],
        summary: Summary { center_label: center.label.clone(), radius_km: p.radius_km, types: p.types.clone(), ..Default::default() },
        error: None,
        workspace_id: ws.id.clone(),
    });
    Ok((run_id, ws.id, center))
}

/// Run a started search to the end, publishing progress through `emit`.
pub async fn run(state: &AppState, run_id: i64, ws_id: &str, p: ResearchParams, center: Resolved, emit: &(dyn Fn(&Progress) + Sync)) {
    let result = run_inner(state, run_id, ws_id, &p, &center, emit).await;
    let cancelled = state.research.cancel.load(Ordering::Relaxed);
    let (status, error) = match &result {
        Ok(()) if cancelled => ("cancelled", None),
        Ok(()) => ("done", None),
        Err(e) => ("failed", Some(e.to_string())),
    };
    let unit = state.prefs().distance_unit;
    let snap = state.research.update(|pr| {
        pr.running = false;
        pr.stage = status.into();
        pr.current.clear();
        pr.error = error.clone();
        let s = &pr.summary;
        pr.message = match status {
            "done" => format!("Finished: {} businesses within {}, prices from {} of {} websites checked.", s.in_radius, unit.fmt(s.radius_km), s.with_prices, s.websites_checked),
            "cancelled" => "Stopped. Results gathered so far were saved.".into(),
            _ => error.clone().unwrap_or_default(),
        };
    });
    if let Some(snap) = snap {
        if guard(state, ws_id).is_ok() {
            let summary = serde_json::to_value(&snap.summary).ok();
            let _ = state.tx(|c| store::finish_run(c, run_id, status, summary.as_ref(), error.as_deref()));
        }
        emit(&snap);
    }
}

fn publish(state: &AppState, emit: &(dyn Fn(&Progress) + Sync), f: impl FnOnce(&mut Progress)) {
    if let Some(p) = state.research.update(f) {
        emit(&p);
    }
}

async fn run_inner(state: &AppState, run_id: i64, ws_id: &str, p: &ResearchParams, center: &Resolved, emit: &(dyn Fn(&Progress) + Sync)) -> AppResult<()> {
    let unit = state.prefs().distance_unit;
    let cancel = state.research.cancel.clone();
    publish(state, emit, |pr| {
        pr.stage = "discovering".into();
        pr.message = format!("Searching OpenStreetMap within {} of {}…", unit.fmt(p.radius_km), center.label);
    });
    // 1. Discover on OpenStreetMap. A failure here is reported, and known competitors are still checked.
    let attribution = crate::providers::info("overpass")?.attribution.unwrap_or_default().to_string();
    match overpass::nearby(state, center.lat, center.lon, p.radius_km, &p.types).await {
        Ok(d) => {
            let found: Vec<_> = d.found.iter().map(|f| (f.osm_ref.clone(), f.competitor.clone())).collect();
            guard(state, ws_id)?;
            let u = state.tx(|c| market::upsert_discovered(c, &found, "osm", &attribution))?;
            publish(state, emit, |pr| {
                let s = &mut pr.summary;
                s.sources.push(SourceCheck {
                    name: "OpenStreetMap".into(),
                    ok: true,
                    detail: if d.from_cache { format!("Saved search results from {} (map searches are reused for a week).", &d.fetched_at[..10.min(d.fetched_at.len())]) } else { format!("Answered by {}.", d.endpoint) },
                });
                (s.discovered, s.unnamed, s.added, s.merged) = (found.len(), d.unnamed, u.added, u.merged);
            });
        }
        Err(e) => publish(state, emit, |pr| pr.summary.sources.push(SourceCheck { name: "OpenStreetMap".into(), ok: false, detail: format!("{e} Competitors you already have were still checked.") })),
    }
    if cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    // 2. Choose websites to check: active competitors within the radius (nearest first) plus
    // ones you added without a location.
    publish(state, emit, |pr| pr.stage = "selecting".into());
    let comps = state.read(|c| market::competitors(c, false))?;
    let since = (chrono::Utc::now() - chrono::Duration::days(RECHECK_AFTER_DAYS)).format("%Y-%m-%dT%H:%M:%S").to_string();
    let mut targets: Vec<(f64, Target)> = Vec::new();
    let (mut in_radius, mut no_location, mut with_website, mut skipped_recent) = (0, 0, 0, 0);
    for c in &comps {
        let d = match (c.input.latitude.as_deref().and_then(|v| v.parse().ok()), c.input.longitude.as_deref().and_then(|v| v.parse().ok())) {
            (Some(a), Some(b)) => Some(haversine_km(center.lat, center.lon, a, b)),
            _ => None,
        };
        match d {
            Some(d) if d <= p.radius_km => in_radius += 1,
            None if c.source != "osm" => no_location += 1,
            _ => continue,
        }
        if c.input.website.is_empty() {
            continue;
        }
        with_website += 1;
        if !p.refresh && c.research.as_ref().is_some_and(|r| r.checked_at.as_str() >= since.as_str()) {
            skipped_recent += 1;
            continue;
        }
        targets.push((d.unwrap_or(f64::MAX), Target { competitor_id: c.input.id.unwrap(), name: c.input.name.clone(), website: c.input.website.clone() }));
    }
    targets.sort_by(|a, b| a.0.total_cmp(&b.0));
    let not_checked_limit = targets.len().saturating_sub(MAX_SITES);
    targets.truncate(MAX_SITES);
    if !p.check_websites {
        targets.clear();
    }
    let total = targets.len();
    publish(state, emit, |pr| {
        let s = &mut pr.summary;
        (s.in_radius, s.no_location, s.with_website, s.skipped_recent, s.not_checked_limit) = (in_radius, no_location, with_website, skipped_recent, not_checked_limit);
        pr.stage = "checking_websites".into();
        pr.sites_total = total;
        pr.message = if total == 0 { "No websites to check.".into() } else { format!("Reading {total} competitor websites for prices (robots.txt respected, one page at a time per site)…") };
    });
    // 3. Read the websites.
    let services = state.read(your_services)?;
    process_sites(
        state,
        ws_id,
        Some(run_id),
        targets.into_iter().map(|(_, t)| t).collect(),
        &services,
        &cancel,
        |t| publish(state, emit, |pr| pr.current.push(t.name.clone())),
        |o| {
            publish(state, emit, |pr| {
                pr.current.retain(|n| *n != o.line.name);
                pr.sites_done += 1;
                let s = &mut pr.summary;
                s.websites_checked += 1;
                s.with_prices += (o.line.prices > 0) as usize;
                s.prices_saved += o.saved;
                s.unmatched_prices += o.unmatched;
                s.unrecognized += o.unrecognized;
                s.consultation_only += o.consultation;
                *s.site_statuses.entry(o.line.status.clone()).or_default() += 1;
                pr.sites.push(o.line);
            })
        },
    )
    .await?;
    let sources_ok = state.research.snapshot().map(|p| p.summary.websites_checked).unwrap_or(0);
    publish(state, emit, |pr| {
        if sources_ok > 0 {
            pr.summary.sources.push(SourceCheck { name: "Competitor websites".into(), ok: true, detail: format!("{} websites read with robots.txt respected.", pr.summary.websites_checked) });
        }
    });
    Ok(())
}

/// Your active services as (id, name), for matching competitors' services.
pub fn your_services(conn: &rusqlite::Connection) -> AppResult<Vec<(i64, String)>> {
    let mut st = conn.prepare("SELECT id, name FROM services WHERE archived_at IS NULL")?;
    let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[derive(Debug, Clone)]
pub struct Target {
    pub competitor_id: i64,
    pub name: String,
    pub website: String,
}

#[derive(Debug, Clone)]
pub struct SiteOutcome {
    pub line: SiteLine,
    pub saved: usize,
    pub unmatched: usize,
    pub unrecognized: usize,
    pub consultation: usize,
}

/// Read each target's website (a few at a time) and save what was found as it arrives.
#[allow(clippy::too_many_arguments)]
pub async fn process_sites(
    state: &AppState,
    ws_id: &str,
    run_id: Option<i64>,
    targets: Vec<Target>,
    services: &[(i64, String)],
    cancel: &Arc<AtomicBool>,
    mut on_start: impl FnMut(&Target),
    mut on_done: impl FnMut(SiteOutcome),
) -> AppResult<()> {
    let robots = crawl::RobotsCache::default();
    let mut set = tokio::task::JoinSet::new();
    let mut queue = targets.into_iter();
    loop {
        while set.len() < PARALLEL_SITES && !cancel.load(Ordering::Relaxed) {
            if crate::providers::remaining_today(state, "web")? < crawl::MAX_PAGES as i64 {
                return Err(AppError::Provider("Today's limit of 1,500 competitor web pages is used up. Results so far were saved; continue tomorrow.".into()));
            }
            let Some(t) = queue.next() else { break };
            on_start(&t);
            let (web, robots, cancel) = (state.web.clone(), robots.clone(), cancel.clone());
            set.spawn(async move {
                let res = match reqwest::Url::parse(&t.website) {
                    Ok(u) => crawl::crawl_site(&web, &robots, &u, &cancel).await,
                    Err(_) => crawl::SiteResult {
                        pages: vec![crawl::PageRecord { url: t.website.clone(), status: "unreachable", http_status: None, prices_found: 0, consultation: vec![], note: "Not a valid web address.".into(), checked_at: db::now_utc() }],
                        ..Default::default()
                    },
                };
                (t, res)
            });
        }
        let Some(joined) = set.join_next().await else { break };
        let (t, res) = joined.map_err(|e| AppError::Other(format!("A website check stopped unexpectedly: {e}")))?;
        guard(state, ws_id)?;
        let outcome = save_result(state, run_id, &t, res, services)?;
        on_done(outcome);
    }
    Ok(())
}

fn save_result(state: &AppState, run_id: Option<i64>, t: &Target, res: crawl::SiteResult, services: &[(i64, String)]) -> AppResult<SiteOutcome> {
    let today = db::tax::today().to_string();
    let (mut prices, mut unrecognized, mut unmatched) = (Vec::new(), 0, 0);
    for f in &res.found {
        let sid = catalog::match_service(f.item.canon, &f.item.label, services);
        if f.item.canon.is_none() && sid.is_none() {
            unrecognized += 1;
            continue;
        }
        unmatched += sid.is_none() as usize;
        prices.push(store::WebPrice {
            service_id: sid,
            label: f.item.label.clone(),
            price: f.item.price,
            price_type: f.item.price_type.into(),
            price_max: f.item.price_max,
            duration_min: f.item.duration_min,
            hair_length: f.item.hair_length.clone(),
            stylist_level: f.item.stylist_level.clone(),
            source_url: f.page_url.clone(),
            observed_on: today.clone(),
            retrieved_at: f.retrieved_at.clone(),
            category: f.item.canon.map(|c| c.category).unwrap_or("").into(),
            canonical: f.item.canon.map(|c| c.key).unwrap_or("").into(),
            description: f.item.description.clone(),
            raw_text: f.item.raw.clone(),
        });
    }
    // Page counts reflect what was kept, not every amount seen.
    let mut pages = res.pages.clone();
    for pg in &mut pages {
        if pg.status == "prices_found" {
            pg.prices_found = prices.iter().filter(|x| x.source_url == pg.url).count();
            if pg.prices_found == 0 {
                pg.status = "no_prices";
                pg.note = "Amounts here weren't recognisable salon services (fees, products and the like).".into();
            }
        }
    }
    let status = crawl::site_status(pages.iter().map(|p| (p.status, p.prices_found))).unwrap_or("no_prices");
    let reached = matches!(status, "prices_found" | "no_prices" | "not_html");
    let saved = state.tx(|c| store::save_site(c, t.competitor_id, run_id, &pages, reached.then_some(&prices[..])))?;
    let note = pages.iter().find(|p| p.status == status && !p.note.is_empty()).map(|p| p.note.clone()).unwrap_or_default();
    // Requests made: each page fetched plus robots.txt (pages refused by terms or robots weren't requested).
    let requested = pages.iter().filter(|p| !matches!(p.status, "terms_restricted" | "robots_disallowed") && !(p.status == "not_html" && p.http_status.is_none())).count() as i64;
    let failed = pages.iter().filter(|p| matches!(p.status, "unreachable" | "http_error" | "blocked" | "rate_limited")).count() as i64;
    let _ = crate::providers::record_many(state, "web", requested + 1, failed, reached, &format!("{}: {}", t.name, status.replace('_', " ")));
    Ok(SiteOutcome {
        line: SiteLine { competitor_id: t.competitor_id, name: t.name.clone(), url: t.website.clone(), status: status.into(), prices: saved, note },
        saved,
        unmatched,
        unrecognized,
        consultation: pages.iter().map(|p| p.consultation.len()).sum(),
    })
}

/// Check one competitor's website now (the "Check prices" button).
pub async fn check_competitor(state: &AppState, competitor_id: i64) -> AppResult<SiteLine> {
    if state.research.is_running() {
        return Err(AppError::Conflict("A market search is running; it will check websites itself. Wait for it to finish.".into()));
    }
    let ws = state.current_info().ok_or_else(|| AppError::NoWorkspace("Open a workspace first.".into()))?;
    let c = state.read(|conn| market::competitors(conn, true))?.into_iter().find(|c| c.input.id == Some(competitor_id)).ok_or_else(|| AppError::NotFound("Competitor not found.".into()))?;
    if c.input.website.is_empty() {
        return Err(AppError::msg("Add this business's website first (Edit)."));
    }
    let services = state.read(your_services)?;
    let mut line = None;
    let t = Target { competitor_id, name: c.input.name, website: c.input.website };
    process_sites(state, &ws.id, None, vec![t], &services, &Arc::new(AtomicBool::new(false)), |_| {}, |o| line = Some(o.line)).await?;
    line.ok_or_else(|| AppError::Other("The website check didn't finish.".into()))
}

/// Add a competitor from its website address, reading its name and address from the home page
/// when its robots.txt allows, then check it for prices.
pub async fn add_website(state: &AppState, url: &str, name: Option<String>) -> AppResult<SiteLine> {
    if state.research.is_running() {
        return Err(AppError::Conflict("A market search is running. Add the website when it finishes.".into()));
    }
    let website = market::normalize_website(url)?;
    if website.is_empty() {
        return Err(AppError::invalid("website", "Enter the competitor's web address."));
    }
    let u = reqwest::Url::parse(&website).map_err(|_| AppError::invalid("website", "Enter a web address like https://example.com."))?;
    let robots = crawl::RobotsCache::default();
    let meta = match crawl::homepage(&state.web, &robots, &u).await {
        Some(page) => extract::site_meta(&page.body),
        None => extract::SiteMeta::default(),
    };
    let host = u.host_str().unwrap_or("").trim_start_matches("www.").to_string();
    let name = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).or((!meta.name.is_empty()).then(|| meta.name.clone())).unwrap_or_else(|| host.clone());
    let zip_point = (meta.latitude.is_none()).then(|| crate::gazetteer::zcta(&meta.postal_code)).flatten();
    let (lat, lon) = match (meta.latitude, meta.longitude, &zip_point) {
        (Some(a), Some(b), _) => (Some(a.to_string()), Some(b.to_string())),
        (_, _, Some(p)) => (Some(p.latitude.clone()), Some(p.longitude.clone())),
        _ => (None, None),
    };
    let c = market::CompetitorInput {
        id: None,
        name: name.clone(),
        address: meta.address.clone(),
        city: meta.city.clone(),
        state: meta.state.clone(),
        postal_code: meta.postal_code.clone(),
        latitude: lat,
        longitude: lon,
        website: website.clone(),
        phone: meta.phone.clone(),
        notes: if zip_point.is_some() { "Location is approximate (centre of its ZIP area).".into() } else { String::new() },
    };
    let ws = state.current_info().ok_or_else(|| AppError::NoWorkspace("Open a workspace first.".into()))?;
    let id = state.tx(|conn| market::upsert_discovered(conn, &[(format!("web:{host}"), c)], "manual", ""))?.ids[0];
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await; // politeness after the home page read
    let services = state.read(your_services)?;
    let mut line = None;
    process_sites(state, &ws.id, None, vec![Target { competitor_id: id, name, website }], &services, &Arc::new(AtomicBool::new(false)), |_| {}, |o| line = Some(o.line)).await?;
    line.ok_or_else(|| AppError::Other("The website check didn't finish.".into()))
}

#[cfg(test)]
mod tests {
    use super::crawl::tests::{html, robots, serve};
    use super::*;
    use crate::workspace::{AppState, WorkspaceKind};

    fn state() -> (AppState, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("srm-research-{}-{}", std::process::id(), chrono::Utc::now().timestamp_nanos_opt().unwrap()));
        let mut st = AppState::new(root.clone()).unwrap();
        st.web = Arc::new(http::WebClient::for_tests(256 * 1024, std::time::Duration::from_millis(1500)));
        let ws = st.create("Test salon", WorkspaceKind::Business).unwrap();
        st.open(&ws.id).unwrap();
        (st, root)
    }

    #[tokio::test]
    async fn checks_sites_saves_prices_and_reports_failures() {
        let (st, root) = state();
        st.tx(|c| {
            c.execute("INSERT INTO services (name, hands_on_min) VALUES ('Women''s Cut', 45), ('Root Color', 60)", [])?;
            Ok(())
        })
        .unwrap();
        let good = serve(vec![
            ("/robots.txt", robots("User-agent: *\nAllow: /\n")),
            ("/", html("<a href='/menu'>Menu</a>")),
            ("/menu", html("<h2>Hair</h2><p>Women's Haircut $65</p><p>Root touch-up $80+</p><p>Balayage $200 - $260</p><p>Mystery item $30</p><p>Color correction: by consultation</p>")),
        ])
        .await;
        let blocked = serve(vec![("/", (503, "text/html", "<title>Just a moment...</title>".into(), 0, vec![]))]).await;
        let noprice = serve(vec![("/", html("<p>Call us to book!</p>"))]).await;
        let mut ids = vec![];
        for (n, u) in [("Good", &good), ("Blocked", &blocked), ("Quiet", &noprice)] {
            let c = market::CompetitorInput { id: None, name: n.into(), address: String::new(), city: String::new(), state: String::new(), postal_code: String::new(), latitude: None, longitude: None, website: u.to_string(), phone: String::new(), notes: String::new() };
            ids.push(st.tx(|conn| market::save_competitor(conn, &c, "manual", None, "")).unwrap());
        }
        let services = st.read(your_services).unwrap();
        let ws = st.current_info().unwrap().id;
        let targets: Vec<Target> = st.read(|c| market::competitors(c, false)).unwrap().into_iter().map(|c| Target { competitor_id: c.input.id.unwrap(), name: c.input.name, website: c.input.website }).collect();
        let mut outcomes = vec![];
        let (mut started, cancel) = (0, Arc::new(AtomicBool::new(false)));
        process_sites(&st, &ws, None, targets, &services, &cancel, |_| started += 1, |o| outcomes.push(o)).await.unwrap();
        assert_eq!(started, 3);
        let by = |n: &str| outcomes.iter().find(|o| o.line.name == n).unwrap().clone();
        let g = by("Good");
        assert_eq!((g.line.status.as_str(), g.saved, g.unmatched, g.unrecognized, g.consultation), ("prices_found", 3, 1, 1, 1));
        assert_eq!(by("Blocked").line.status, "blocked");
        assert_eq!(by("Quiet").line.status, "no_prices");
        // Saved with source, time, and inferred matches to your services.
        let obs = st.read(|c| market::observations(c, None)).unwrap();
        let cut = obs.iter().find(|o| o.input.service_label == "Women's Haircut").unwrap();
        assert_eq!((cut.match_method.as_str(), cut.service_name.as_deref()), ("auto", Some("Women's Cut")));
        assert!(cut.input.source_url.ends_with("/menu") && cut.retrieved_at.is_some() && cut.raw_text.contains("$65"));
        let bal = obs.iter().find(|o| o.input.service_label == "Balayage").unwrap();
        assert_eq!((bal.input.price_type.as_str(), bal.input.service_id, bal.canonical.as_str()), ("range", None, "balayage"));
        // Competitor list shows each site's outcome.
        let comps = st.read(|c| market::competitors(c, false)).unwrap();
        assert_eq!(comps.iter().find(|c| c.input.name == "Blocked").unwrap().research.as_ref().unwrap().status, "blocked");
        // Checking one competitor again works through the same path.
        let line = check_competitor(&st, ids[0]).await.unwrap();
        assert_eq!(line.prices, 3);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn adds_a_competitor_from_its_website() {
        let (st, root) = state();
        let site = serve(vec![(
            "/",
            html(r#"<script type="application/ld+json">{"@type":"NailSalon","name":"Polish Bar","address":{"streetAddress":"5 Main St","addressLocality":"Olympia","addressRegion":"WA","postalCode":"98501"}}</script><p>Gel Manicure $45</p>"#),
        )])
        .await;
        let line = add_website(&st, site.as_str(), None).await.unwrap();
        assert_eq!((line.name.as_str(), line.status.as_str(), line.prices), ("Polish Bar", "prices_found", 1));
        let c = st.read(|c| market::competitors(c, false)).unwrap().pop().unwrap();
        assert_eq!((c.input.city.as_str(), c.source.as_str()), ("Olympia", "manual"));
        assert!(c.input.latitude.is_some() && c.input.notes.contains("approximate")); // ZIP centre
        // Adding the same site again doesn't duplicate the business.
        add_website(&st, site.as_str(), Some("Polish Bar Olympia".into())).await.unwrap();
        assert_eq!(st.read(|c| market::competitors(c, false)).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn centres_and_validation() {
        let (st, root) = state();
        let z = st.read(|c| resolve_center(c, &Center::Zip { zip: "98501".into() })).unwrap();
        assert!(z.approximate && (z.lat - 47.0).abs() < 0.2);
        assert!(st.read(|c| resolve_center(c, &Center::Zip { zip: "00000".into() })).is_err());
        assert!(st.read(|c| resolve_center(c, &Center::City { city: "Olympia".into(), state: "wa".into() })).is_ok());
        let p = |r: f64, t: &[&str]| ResearchParams { center: Center::Zip { zip: "98501".into() }, radius_km: r, types: t.iter().map(|s| s.to_string()).collect(), check_websites: true, refresh: false };
        assert!(validate(&p(8.0, &["hair"])).is_ok());
        assert!(validate(&p(80.0, &["hair"])).is_err());
        assert!(validate(&p(8.0, &[])).is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
