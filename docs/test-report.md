# Test report

Date: 2026-10-04. Host: Fedora 44 Workstation (x86_64, Wayland). Builds and tests ran in rootless
toolbox containers: `srm-fedora` (Fedora 44, WebKitGTK 2.54) and `srm-ubuntu` (Ubuntu 22.04,
WebKitGTK 2.50, glibc 2.35). Install tests used separate clean containers (`srm-fedora-clean`,
`srm-ubuntu22-clean`, `srm-ubuntu24-clean`). Rust 1.99, Node 22.23, Tauri 2.12.

## Summary

| Check | Result |
|---|---|
| Rust unit and integration tests (`cargo test`) | **88 passed**, 0 failed, 2 ignored (large-dataset benchmark and OS keyring check, both run separately below) |
| OS keyring (`cargo test keyring_round_trip -- --ignored`) | Passed against GNOME Keyring (Secret Service) in the Fedora 44 desktop session: store, read back, remove, invalid key rejected |
| Frontend typecheck (`tsc --noEmit`) | Passed |
| Frontend unit tests (`vitest`) | 4 passed |
| Real desktop app end-to-end (WebDriver → built Linux binary, X11 virtual display) | 6 scripts, **all passed** in sequence on one data folder (`scripts/e2e-all.sh`, debug build) |
| Native print dialog | Opens ("Print" window owned by the app); PDF output not automated |
| Release RPM: build, install into a clean Fedora 44 container, launch | **Passed** |
| `.deb`: build on Ubuntu 22.04, install and launch in clean Ubuntu 22.04 and 24.04 containers | **Passed** |
| AppImage: launch on Fedora 44 and Ubuntu 24.04 | **Passed** |
| Windows installer (`…_x64-setup.exe`): cross-compiled on Linux | **Built**; the Rust suite compiled for Windows passed under Wine (88/88); silent install and uninstall worked under Wine. The app itself has **not** been run on Windows |
| macOS (`.dmg`) and Windows on real machines | **Not run yet**: needs the GitHub Actions workflow (see [Windows and macOS](#windows-and-macos)) |

## Automated tests (Rust)

Domain (pure calculation) tests:
- **Units**: exact factors (oz, lb, fl oz, gal; 128 fl oz = 1 gal, 16 oz = 1 lb), compatible
  conversions, weight-oz vs fluid-oz rejected without density, density conversions, count never
  converts, custom units, fractional quantities stay exact.
- **Inventory valuation**: the 32 fl oz / $24 acceptance case ($0.75/fl oz; 2 fl oz costs $1.50,
  leaves 30 fl oz worth $22.50), weighted average across purchases with earlier costs preserved,
  issuing all stock clears value exactly, negative stock and revaluation, missing cost basis,
  reversal residue clearing.
- **Money**: half-away-from-zero rounding (incl. $2.675 → $2.68), percent validation, cent allocation
  sums exactly.
- **Pricing**: margin vs markup ($60 → $100 at 40% margin, $84 at 40% markup), commission solved at
  the recommended price (margin exactly met), processing on the tax-inclusive total, tax-inclusive
  pricing, impossible targets rejected, rounding up/down/nearest with recalculated margin.
- **Costing**: processing time counted as overhead not labor; time validation.
- **Work profiles**: overhead per billable hour, zero billable hours rejected, commission vs hourly
  double-count prevention, burden, impossible margin.
- **Sale totals**: exclusive tax with discounts and tips (tax rounded once, split exactly),
  inclusive tax, unknown taxability and missing rate are unresolved (never 0%), validation, partial
  refunds never drift.
- **Market statistics**: quantiles, insufficient evidence and exclusion rules, duplicates, outliers,
  quality levels, distance, duration mismatch.
- **Offline place lookup**: bundled Census Gazetteer ZIP and city centres.

Database/integration tests (in-memory SQLite with all migrations):
- **Migrations**: empty → latest; upgrade from every earlier version with the backup hook called;
  newer data refused.
- **Backups**: round trip with attachments; tampered attachment detected by checksum; truncated
  archive rejected.
- **Inventory**: receive and consume (duplicate deduction rejected by the ledger), landed-cost
  extras, purchase reversal, movements and reversals, ledger reconciles with stock, transfers move
  quantity not value, duplicate SKU, stock-unit dimension lock.
- **Services**: estimates, recipe versioning, variants, below-target note required, incompatible
  recipe unit rejected, bundle price allocation.
- **Tax**: unknown taxability never becomes 0%; rate versions; component sums checked; staleness
  judged by the provider's data period, not fetch time.
- **Sales**: unresolved tax blocks finalize and drafts don't move stock; finalize deducts *actual*
  usage once and snapshots costs (second finalize rejected, nothing deducted twice); history unchanged
  after profile and tax changes; refunds with retail restock vs no restock of service product;
  over-refund rejected; void reverses stock and is refused after refunds.
- **Appointments**: duration from services, overlap detection and override, reschedule,
  opening-hours warning, cancelled bookings don't block; checkout from an appointment is idempotent.
- **Clients**: validation, formula versions.
- **Reports**: totals reconcile with groupings and the weekly trend; card fees allocated exactly;
  stock consumed equals materials + retail cost of goods; period result subtracts actual expenses
  only; voided sales excluded; prior period range.
- **Market**: evidence, position suggestion and conflict message, CPI-modeled median kept separate;
  CSV import validation and duplicate detection.
- **CSV**: product import preview/validation/dedupe with opening stock; export formula sanitising;
  portable export contains every table.
- **Export paths**: file writes require the expected extension and an existing folder outside the
  app's data folder.
- **What-if pricing**: each change explained (materials, labor via utilization, overhead, target).
- **Providers**: parsers tested against **real responses captured on 2026-10-04** (WA DOR address and
  error responses, CDTFA address and multi-jurisdiction responses, Census Geocoder, BLS CPI including
  a month published as "-", BLS Washington OEWS wage, Overpass). Secret redaction. Connection
  failure becomes an error, never a value. The Census Data API parser uses a hand-written sample
  (no key available).
- **Demo data**: two months generated through the normal code paths; report groupings reconcile and
  every product's ledger sums to its stored quantity and value.

## Large dataset (release build)

`cargo test --release large_dataset -- --ignored --nocapture` generates two years of history
(about 16 sales per open day):

| Measure | Result |
|---|---|
| Sales / sale lines generated | 9,888 sales, 18,601 lines (through the normal finalize path) |
| Generation time | 46 s |
| Two-year report (3 groupings, weekly trend, prior period) | 0.43 s |
| Product list + 2,000-row sales list | 8 ms |

## Real desktop app end-to-end

Scripts in `e2e/` drive the built Linux binary through `tauri-driver` and WebKitWebDriver on a
private Xvfb display, taking screenshots in light and dark themes. Each script relaunches the app on
the same data folder, so every run after the first also checks **persistence across restarts**.

| Script | Covers |
|---|---|
| `smoke.mjs` | Create business → onboarding (business, location, hours, work profile) → profile validation (zero utilization) → edit creates version 2 → manual backup → activity log → light theme |
| `slice2.mjs` | Restart → products (fl oz, grams + custom "tube" unit) → receive two lines → $0.75/fl oz shown → record waste (30 fl oz, $22.50) → service with recipe and variant → live breakdown (materials $10.50) → below-cost warning → use target price → save |
| `slice3.mjs` | **Live WA DOR lookup** (9.8%, Q4 2026, address match) → taxability decisions with sources → estimate shows tax → what-if utilization explained → **live Census geocode** (county 53067) → providers page |
| `slice4.mjs` | New appointment with new client → calendar → check out → actual usage 2.5 fl oz vs 2 planned → tip → card payment → finalize (S-00001, tax $8.33, materials $10.88 vs planned $10.50) → receipt → stock 27.5 fl oz (deducted once) → client formula → expense |
| `slice5.mjs` | Dashboard figures ($85 net revenue) → remove widget and save layout → **live OpenStreetMap discovery** (21 salons) → four observed prices → medium evidence → conflict shown on Pricing → reports statement and sales-tax tab |
| `slice6.mjs` | Export all data (zip of every table) → back up → **restore as a new workspace** and confirm the sale and stock level → create demo workspace → visual pass (calendar, inventory, services, clients, market, reports) in both themes |

Together these cover the requested workflow: create business → work profile → receive bulk
inventory → service recipe → calculate and override price → appointment → complete with actual usage
→ stock deducted → sale and tax → dashboard → export → restart → persistence → back up and restore.

Problems found and fixed through the real-app runs included: a false "unsaved changes" prompt after
creating records, a startup crash caused by Tauri's `freezePrototype` with the calendar's date library,
CommonJS interop for the drag-and-drop addon, cramped receive and taxability layouts, duplicate
time-zone options, "−$0.00" and plural wording.

**Print:** pressing "Print or save as PDF" on a receipt opened a native window titled "Print" owned by
the app (confirmed with `xwininfo`). Its contents didn't paint on the bare virtual display (no
printing services in the container), so saving a PDF through the dialog was not automated.

## Packaging

All packages were built from the final source. It differs from the last end-to-end run only by one
demo-data restock quantity, covered by the `demo_seed_is_consistent` test. "Launch" means: start
the installed app on a private X display with a fresh data folder, confirm a window titled "Salon
Resource Manager" exists (`xwininfo`), confirm the data folders were created, and inspect a
screenshot of the welcome screen.

| Package | Size | Built on | Install test | Launch test |
|---|---|---|---|---|
| `Salon Resource Manager-0.1.0-1.x86_64.rpm` (package `salon-resource-manager`) | 8.6 MB | Fedora 44 | Clean Fedora 44 container: `dnf install` pulled in WebKitGTK 4.1 and GTK 3; desktop entry installed | Passed |
| `Salon Resource Manager_0.1.0_amd64.deb` (depends `libwebkit2gtk-4.1-0`, `libgtk-3-0`) | 8.4 MB | Ubuntu 22.04 (glibc 2.35) | Clean Ubuntu 22.04.5 and 24.04.5 containers without WebKitGTK: `apt install` pulled in dependencies | Passed on both |
| `Salon Resource Manager_0.1.0_amd64.AppImage` | 83 MB | Ubuntu 22.04 | No install | Passed on Fedora 44 and Ubuntu 24.04 (`APPIMAGE_EXTRACT_AND_RUN=1`, since containers have no FUSE) |

Package sizes include the bundled offline Census Gazetteer files (about 2 MB).

## Windows and macOS

**Windows, cross-compiled here** (`scripts/build-windows.sh`: cargo-xwin, MSVC target, clang/lld,
NSIS 3.11 in the Fedora toolbox):

| Check | Result |
|---|---|
| Build `Salon Resource Manager_0.1.0_x64-setup.exe` | Built (5.8 MB) |
| Rust suite compiled for `x86_64-pc-windows-msvc`, run under Wine 11 | **88 passed**, 0 failed, 2 ignored. This covers Windows file paths and renames, the bundled SQLite, backups, CSV and money rules as Windows binaries, but Wine is not Windows. |
| Silent install (`/S`) under Wine | Installed per-user to `%LOCALAPPDATA%\Salon Resource Manager` (app + uninstaller), with a Start-menu shortcut and an *Apps* entry (name, version 0.1.0, uninstall command) |
| Silent uninstall under Wine | Removed the files and the *Apps* entry |
| WebView2 install step | **Not exercised.** Wine has no WebView2, so it was marked present in the Wine registry to let the installer continue |
| Launching the app | **Not done.** WebView2 doesn't run under Wine |

**GitHub Actions** (`.github/workflows/ci.yml`, run manually or by a `v*` tag) is set up to:
- run the Rust tests on real Windows and macOS runners;
- build the NSIS and MSI installers, and the universal (Apple Silicon + Intel) macOS app and `.dmg`;
- silently install the Windows build and launch it, launch the macOS app and check its ad-hoc
  signature and architectures, and confirm each one starts and creates its data folders.

**These CI checks have not run yet.** This section will be updated with their results.

macOS-specific changes made without being able to run them:
- `core:webview:allow-print`, granted on macOS only, because Tauri routes `window.print` through
  it there.
- Ad-hoc signing, so Apple Silicon will start the app.
- ⌘ instead of Ctrl in the shortcut hint.

## Not tested

- **Windows and macOS on real machines** until the GitHub Actions run above completes; even then,
  only the listed smoke checks (no end-to-end workflow, printing or keyring tests on those systems).
- A physical Wayland desktop session (tests used X11 via Xvfb on a Wayland host).
- Live Census Data API calls (needs a key) and the BLS v2 key path.
- Saving a PDF through the GTK print dialog.
- OS keyring on Ubuntu, KDE (KWallet) or headless sessions; the app reports a clear error when no
  Secret Service is available.
- Screen readers.
