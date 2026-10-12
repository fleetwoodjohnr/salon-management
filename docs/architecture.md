# Architecture

## Stack

| Layer | Choice | Why |
|---|---|---|
| Desktop shell | Tauri 2 (Rust) | Small native packages for Linux, Windows, macOS using the system webview; privileged work stays in Rust |
| Interface | React 19, TypeScript, Vite | Mature, typed, fast builds |
| Components | Mantine 9 (+ dates, charts, notifications, spotlight, modals), mantine-datatable | One consistent accessible component set: theming, focus handling, light/dark |
| Charts | Recharts via `@mantine/charts` | Free, themeable |
| Calendar | react-big-calendar (+ drag-and-drop addon) with dayjs | Resource (staff) day view, drag to reschedule |
| Dashboard grid | react-grid-layout 2 | Drag/resize/save widget layouts |
| Data | SQLite (bundled via `rusqlite`), STRICT tables, foreign keys, WAL | No database server; one file per workspace |
| Money | `rust_decimal` | Exact base-10 arithmetic |
| HTTP | `reqwest` (rustls) | Provider calls and website reading happen only in Rust, never from the webview |
| HTML | `scraper` (html5ever) + `regex` | Parse competitors' menu pages without running their scripts |
| Updates | `tauri-plugin-updater` + `tauri-plugin-process` | Official Tauri updater: GitHub Releases manifest, Ed25519 signature check, per-format installers (AppImage, deb, rpm, NSIS, MSI, macOS app) |
| Secrets | `keyring` 4 (Secret Service / Keychain / Credential Manager) | API keys never touch the database or frontend |
| Fonts | Figtree and Bodoni Moda (OFL), bundled | Offline; no font CDN |

All dependencies are MIT, Apache-2.0, BSD-style, ISC or OFL licensed.

## Layout

```
src-tauri/src/
  lib.rs          plugin setup, command registration
  workspace.rs    workspace registry, open/close/restore, transaction helpers
  backup.rs       zip backups with checksummed manifest, verified restore
  attachments.rs  receipts stored by content hash
  csvio.rs        CSV import (mapping → preview → commit), exports, portable export
  demo.rs         fictional demo data (also the large-dataset test)
  gazetteer.rs    offline ZIP/city centres from the bundled Census Gazetteer (src-tauri/data/)
  prefs.rs        app-wide preferences (distance unit, update checks, last search) in preferences.json
  research/       market research: catalog (normalised services), extract (prices from HTML),
                  robots (robots.txt), http (safe web client), crawl (one site), mod (background runs)
  domain/         pure calculations, no IO: money, units, profile, costing, pricing,
                  inventory (moving average), sale (totals/tax/refunds), market (statistics)
  db/             SQL per area: core, profiles, inventory, services, pricing (what-if), tax,
                  appointments, clients, estimates, sales, expenses, market, research, reports, dashboards
  commands/       thin Tauri command handlers (deserialize → validate → db/domain)
  providers/      HTTP plumbing (cache, quotas, retries, de-duplication, keyring) and
                  WA DOR, CDTFA, Census, BLS, Overpass clients
src-tauri/migrations/   001…007 SQL migrations (embedded in the binary)
src/
  api/            invoke wrapper, query helpers, TypeScript mirrors of the Rust types
  app/            shell, navigation, routes, theme
  components/     shared pieces (decimal input, breakdown, page header, unsaved-changes tracking, undo)
  features/       one folder per area (inventory, services, pricing, tax, calendar, sales, …)
  lib/            display formatting (string-based decimal rounding), units, dates
e2e/              real-app end-to-end scripts (WebDriver against the built binary)
```

## Principles

- **The Rust core is the single source of truth.** The interface sends inputs and displays decimal
  strings. Live previews (profile rates, service estimates, landed cost, what-if pricing, sale
  totals) are computed by preview commands, debounced as you type.
- **Domain code is pure** (`domain/`), so financial rules are unit-tested without a database.
- **Every write runs in a transaction** (`AppState::tx`); a failed step rolls back everything.
- **History is append-only where money is concerned**: profile versions, recipe versions, tax rate
  and taxability versions, the stock ledger, refunds. Finalized sales snapshot all figures used.
- **Validated boundary**: commands deserialize into typed structs that reject unknown fields, then
  validate business rules. Errors return `{kind, message, field}`, so forms show messages next to the
  right field.
- **Least privilege**: the webview gets only `core:default`, window close, file dialogs, clipboard
  write, opening `http(s)://` links (competitor websites), the updater (check/download/install from
  the configured GitHub endpoint only, signature-verified in Rust) and restart. No shell,
  filesystem or HTTP plugin access from JavaScript.
  Strict content security policy (`default-src 'self'`, IPC only). File reads/writes happen in Rust
  on paths the person picked in a native dialog. Tauri's `freezePrototype` is off because the calendar's
  date library assigns to its own prototype (see PROGRESS.md).
- **Responsiveness**: commands are `async` and run on the async runtime, off the UI thread. Network
  calls release the database lock while waiting. A market search runs as a background task that
  reports progress through `research-progress` events; the page can be left and reopened mid-run.

## Market research

`research::start` validates the request, resolves the search centre (a located address, or a ZIP or
city centre from the offline Gazetteer), records a run and marks it running; the command then spawns
`research::run`:

1. **Discover**: `providers::overpass::nearby` POSTs one query (tags chosen from the business types)
   to each public Overpass server in turn until one answers properly; results are uncapped and an
   in-band `remark` error counts as a failure. `db::market::upsert_discovered` merges duplicates
   (same OSM id, website host, phone, or name within 150 m) and only fills empty fields.
2. **Select**: active competitors within the radius (plus ones you added without a location) that
   have a website and weren't checked in the last 7 days, nearest first, at most 60 a run.
3. **Read**: `research::crawl::crawl_site`, three sites at a time, each with its own politeness
   (robots.txt, ≥1.5 s between requests, ≤8 pages, 90 s budget). `research::extract::scan` turns each
   page into priced items; `research::catalog` normalises service names and matches them to yours.
4. **Save** per site in one transaction (`db::research::save_site`): page outcomes, and when the site
   was actually read, its website prices replaced (your exclusions and manual matches carried over).

A failure in one step is reported and the rest continues (if the map servers are down, known
competitors' websites are still read). Before every write the run checks that the workspace it
started in is still open. Statistics (`domain::market::evaluate`) count one price per business and
`domain::market::recommend` combines the observed position price with the cost-based target.

## Updates

`tauri.conf.json` names the public key and the endpoint
`releases/latest/download/latest.json`. The Settings page calls the updater plugin's `check()` and
`downloadAndInstall()`; the plugin picks the platform entry `<os>-<arch>-<bundle type>` (the bundle
type is stamped into each package at build time), downloads into memory, verifies the minisign
signature and installs: AppImage replaced in place (restored on failure), `.deb`/`.rpm` via
`pkexec dpkg -i` / `pkexec rpm -U`, Windows NSIS/MSI in passive mode (the app exits and is
restarted by the installer), macOS `.app` replaced (restored on failure). `update_prepare` backs up
the open workspace first. CI signs the packages with `TAURI_SIGNING_PRIVATE_KEY` and
`scripts/release-manifest.mjs` writes `latest.json` for each tagged release.

## Workspaces

Each workspace is a folder with its own `salon.db` and `attachments/`. The registry
(`workspaces.json`) lists them and remembers the last one opened. The demo workspace is a separate
workspace of kind `demo`, with a permanent banner; demo and real data can't mix. Workspaces are not
designed for simultaneous use from several computers.

## Startup and migrations

Opening a workspace: SQLite pragmas (foreign keys, WAL, full sync, busy timeout) → if the data
version is older, back up first, then apply each pending migration in its own transaction and run a
foreign-key check → daily automatic backup if due. Data from a newer app version is refused.

## Interface patterns

- Persistent left navigation grouped by task; `Ctrl+K` command palette; `Ctrl+1…9` jump to pages;
  `Ctrl+N` new item on list pages; `Ctrl+S` save in editors.
- Unsaved-changes protection: in-app navigation and window close both ask first.
- Undo toasts for reversible actions (appointment status, archiving a client); corrections to money and
  stock use explicit reversals instead.
- Sale drafts autosave; they have no effect on stock until finalized.
- Printing (receipts, estimates, reports, dashboard) uses the system print dialog via `window.print()`
  with print styles; "Print to File" produces a PDF.
