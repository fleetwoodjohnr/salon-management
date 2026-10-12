# Progress and decisions

Working log so development can resume across sessions. Newest entries at the bottom of each section.

## Environment
- Host: Fedora 44 (Wayland). Builds run in toolbox `srm-fedora` (Fedora 44 + webkit2gtk4.1-devel etc.).
  `scripts/tb <cmd>` runs a command there with `~/.cargo/bin` on PATH.
- Rust via rustup in `~/.rustup` / `~/.cargo` (not added to shell profile).
- Ubuntu builds: toolbox `srm-ubuntu` (22.04) — see slice 6.
- No git (user's choice). This file is the history.

## Commands
- Rust tests: `scripts/tb cargo test --manifest-path src-tauri/Cargo.toml`
- Frontend: `npx tsc --noEmit`, `npx vitest run`
- Debug desktop build: `scripts/tb npx tauri build --debug --no-bundle`
- Real-app E2E (Xvfb + tauri-driver + WebKitWebDriver): `scripts/tb node e2e/<script>.mjs <empty-data-dir>`
  screenshots land in `e2e/screenshots/`.

## Slices
| # | Slice | Status |
|---|-------|--------|
| 1 | Shell, storage, onboarding, work profiles | done |
| 2 | Inventory and service costing | done |
| 3 | Pricing and tax | done |
| 4 | Clients, appointments, sales | done |
| 5 | Dashboards and market research | done |
| 6 | Polish, recovery, testing, packaging | done |

## Decisions
- Business logic in Rust (single source of truth); UI renders decimal strings. TS types are hand-written
  in `src/api/types.ts` (ts-rs has no rust_decimal support); backend rejects unknown fields.
- Decimals as TEXT (rust_decimal). Money rounded half away from zero to cents only at totals.
- Versioned config (work profiles) stored as JSON `data` + indexed columns; each save appends a version.
- Labor cost per billable hour = pay per hour worked ÷ utilization (non-billable time is paid by billable time).
- Workspace = folder with `salon.db` + `attachments/`; registry `workspaces.json`; `SRM_DATA_DIR` overrides root.
- Backups: zip with `VACUUM INTO` snapshot, attachments, manifest with sha256; verified + migrated in scratch before restore.
- Visual system: mulberry primary (#7a2e5c), porcelain canvas, aubergine dark neutrals; Figtree for UI,
  Bodoni Moda only for page titles and headline figures (fonts bundled, offline).
- Hash router (custom protocol, no server fallback needed).

- Inventory: moving weighted average on (Q, V) per product, multiply-before-divide; issuing all stock costs exactly V;
  negative stock allowed (warned) and re-valued on next receipt via a `revaluation` ledger row. Ledger is append-only,
  unique (source, source_id, kind) blocks double deductions. Quantity per storage spot, value per product.
- Purchases: discount/shipping/non-recoverable tax allocated to lines by line price in cents (largest remainder).
- Recipes versioned (new version only when lines change); variants archived not deleted.
- Bundles: bundle price allocated to services by their own list prices; each share costed with that service's profile.
- Tax model pulled into slice 2 (services need TaxContext): migration 003, `db/tax.rs`. Unknown taxability or missing
  rate => unresolved (never 0%). Staleness judged by provider dataset period, not fetch time.
- E2E harness quirks: WebKitWebDriver getText skips button text (use innerText); type with select-all + addValue.

- Providers (`src-tauri/src/providers`): user-initiated only; per-workspace cache + daily caps + event log; redirects are
  errors (Census missing-key = 302); keys only in OS keyring via keyring 4 (`v1` API, zbus Secret Service on Linux).
  WA DOR result codes mapped to precision (0/2/4 address, 1/3 ZIP+4, 5 ZIP). CDTFA multi-jurisdiction = error.
  Applying a lookup re-parses the cached provider response server-side (UI can't alter "official" data).
- What-if pricing re-runs `estimate_with_profile` on an in-memory modified profile/service; explanations generated
  from component deltas.

- Tauri `freezePrototype` is OFF: dayjs (calendar) assigns `toString` on its prototype, which throws when
  Object.prototype is frozen (JS "override mistake"). Strict CSP + validated commands remain the boundary.

- Sales: drafts autosave (no stock effect); finalize = status guard + ledger unique source => deduct once.
  Tax rounded once per sale on the taxable base, allocated to lines (largest remainder) for refunds.
  Finalized lines snapshot net, tax, taxability+basis, profile version, labor/overhead (actual minutes scale
  the planned split), commission, materials (ledger cost) and planned materials. Void = mistake (reverses stock,
  voids payments; refused once refunds exist). Refund = proportional, last refund takes the remainder; restock
  only retail, at the cost it left with. Card fee per payment from the sale staff's profile.
- Appointments: local "YYYY-MM-DDTHH:MM" strings (computer TZ assumed = business TZ). Overlap needs
  `allow_overlap`; opening-hours warnings. Onboarding creates the first staff member.

## Temporary items to remove before release
- None remaining (placeholders and stubs are gone).

## Windows and macOS (2026-10-04)
- User approved git **only** to run GitHub Actions; they create the GitHub repo and push (no `gh`).
- Windows NSIS installer is cross-compiled locally with `scripts/build-windows.sh` (cargo-xwin, needs
  both `mingw32-nsis` and `mingw64-nsis` in the toolbox). The Rust suite passes as Windows binaries
  under Wine (88/88), and silent install/uninstall works under Wine with WebView2 marked present. The app
  has not been launched on Windows.
- macOS: ad-hoc signing (`bundle.macOS.signingIdentity "-"`, min 11.0); `capabilities/macos.json` grants
  `core:webview:allow-print` (Tauri's macOS `window.print` shim needs it); ⌘ shortcut label.
- CI (`.github/workflows/ci.yml`): Linux checks on push; manual run or `v*` tag runs Rust tests on
  Windows/macOS, builds all installers (universal macOS dmg; nsis+msi) and smoke-launches each.

## 0.2.0: updates, distance units, market research (2026-10-11)
- **Root causes of the broken market research** (verified live 2026-10-11): one hard-coded Overpass
  server via GET (overpass-api.de answered 504 to a one-node query; private.coffee rejects GET); `out
  … 300` silently truncated dense areas (Seattle 300 vs 586); narrow tags; re-discovery overwrote user
  edits; scheme-less OSM websites didn't open; no duplicate merging; only the primary location; and
  no price retrieval at all (prices were manual by design).
- Discovery: POST + 4 endpoints in turn, uncapped, `remark` errors treated as failures and never
  cached (`providers::fetch_checked`), business-type tag sets, `upsert_discovered` (ref → website host
  → phone → name within 150 m; fills empty fields only). Centre = located address, ZIP or city
  (offline Gazetteer).
- Prices: `research/` reads competitors' own websites. robots.txt (own RFC 9309 parser), terms
  deny-list (social, booking platforms, directories), public-IP-only resolver + IP-literal/port
  checks per redirect hop, same-site redirects only, 3 MB cap, anti-bot = stop. Extraction:
  JSON-LD offers + DOM lines with ancestry (a price takes its name from its own menu item first —
  Squarespace prints price before title), tiers/hair-length columns/qualifiers kept separate,
  consultation-only recorded without amount, noise filters. Catalog: priority + longest pattern,
  covering phrase wins, section headings refine vague labels only within the label.
- Search engines/directories/booking APIs: none usable free or permitted in 2026 (Brave needs a
  card, Foursquare ~500 calls + billing, Google CSE closed, result pages disallow bots). Domain
  guessing verified 1/25 → dropped. User chose "free sources only".
- Migration 007 rebuilds `market_observations` (new source type + provenance) and adds
  `research_runs`, `research_pages`. Stats: one price per business (middle tier), mean,
  percentile; `recommend()` = market position price if ≥ cost target, else cost target.
- Runs are background tasks (`research-progress` events), one at a time, workspace-guarded, partial
  failures reported; sites re-read after 7 days; 60 sites per run; `web` provider caps 1,500
  pages/day.
- Units: `prefs.rs` (`preferences.json` in the data root, tolerant of unknown/missing fields);
  km stored everywhere, `DistanceInput` converts at the edge. Default miles (user's choice).
- Updates: official `tauri-plugin-updater` 2.13.1 + process 2.4.0 (2.13 installs AppImage, deb via
  pkexec dpkg, rpm via pkexec rpm, NSIS/MSI passive, macOS app; picks `<os>-<arch>-<bundle>`).
  Key generated at `~/.tauri/salon-resource-manager.key` (no password); public key in
  tauri.conf.json; user adds the GitHub secrets. CI signs only when the secret exists
  (`--config createUpdaterArtifacts`), so local builds need no key. `scripts/release-manifest.mjs`
  writes latest.json (11 platform keys) and release notes from CHANGELOG.md.
- E2E harness: `clickLabel` for Mantine switch/segmented labels (WebKitWebDriver says "not
  clickable"). slice5 now runs the Research tab map-only; slice7 covers units, a live research
  run, overview and the update check; `e2e/update-appimage.mjs` tests the AppImage update path.

## Next steps (resume here)
- All six slices done. Final packages built and install/launch-tested (see docs/test-report.md):
  RPM in `src-tauri/target/release/bundle/rpm/`, .deb and AppImage in
  `src-tauri/target-ubuntu/release/bundle/{deb,appimage}/` (build: `scripts/tb-ubuntu npx tauri build
  --bundles deb,appimage` after a host/Fedora frontend build).
- Decisions added in slice 6: inventory cost assignments rounded to 10 dp (demo ledger check found
  1e-25 drift); export paths validated in Rust (`AppState::output_path`); offline locating via bundled
  Census Gazetteer 2025 (`src-tauri/data/`, `gazetteer.rs`); market observations whose listed duration
  differs >50% from the service are excluded.
- First full GitHub Actions run (#2, `aee9250`, 2026-10-05) passed on Linux, Windows and macOS: tests,
  installers, smoke launches. CI now runs the full matrix on every push (public repo, free minutes).
- Open items (not blockers): live Census Data API needs a key; PDF via the GTK print dialog not automated; no screen-reader audit.
- **0.2.0 (2026-10-11), waiting on the user:** add GitHub secrets `TAURI_SIGNING_PRIVATE_KEY` (contents
  of `~/.tauri/salon-resource-manager.key`) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (empty), back up
  the key, push, tag `v0.2.0`. Then check the CI release has latest.json and that an installed 0.2.0
  sees "You have the latest version". First real cross-platform update test: release 0.2.1 and update
  a Windows/macOS install from 0.2.0 (only the Linux AppImage path is tested so far).
- Ideas not done: PDF menu text extraction (PDF menus are listed for manual entry); a free source
  for websites of salons OSM lacks (none found that's free and permitted); state cosmetology license
  open data for small-salon discovery (state-specific).
