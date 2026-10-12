# Test report

## 0.2.0 (2026-10-11)

Same host and toolboxes as 0.1.0 below (Fedora 44 host; `srm-fedora`, `srm-ubuntu`). Tauri CLI
2.12.1, updater plugin 2.13.1.

| Check | Result |
|---|---|
| Rust unit and integration tests (`cargo test`) | **121 passed**, 0 failed, 5 ignored (benchmark, OS keyring, and three live-network checks run separately below) |
| Frontend typecheck and tests (`tsc --noEmit`, `vitest`) | Passed; **13 tests** (distance conversion, formatting, update error wording, release manifest) |
| Lint (`cargo clippy --all-targets`) | No warnings in new code; 6 older warnings in untouched files left as they were |
| Real desktop app end-to-end (`scripts/e2e-all.sh`, debug build, live network) | **7 scripts, all passed** (slice 5 now uses the Research tab; new slice 7: units, live research, overview, update check) |
| Live OpenStreetMap discovery (`cargo test live_overpass -- --ignored`) | Passed for Tumwater, Seattle, Austin and Indianapolis at 3 km (hair) and 8 km (all types). 4 of the 8 queries got 504s from the first one or two servers and succeeded on the next; Seattle 8 km returned **554** named businesses (the old query stopped at 300) |
| Live website crawl (`cargo test live_crawl -- --ignored`) | Passed: prices from **4 of 5** real Seattle salon sites (19, 28, 114 and 57 prices; the fifth publishes none as text), 2–8 s per site, robots.txt respected |
| In-app update, Linux AppImage (`e2e/update-appimage.mjs`) | **Passed**: two signed AppImages (0.2.0 → 0.2.1, test key, local update server). An interrupted download and a well-formed but wrong signature were each rejected with the installed AppImage byte-for-byte unchanged; the good update replaced it with exactly the 0.2.1 file, made a pre-update backup per attempt (3), and the restarted app reported 0.2.1 with the demo data intact |
| Windows: Rust suite compiled for `x86_64-pc-windows-msvc`, run under Wine | **121 passed**, 0 failed, 5 ignored (includes the crawler tests against a local server, migrations and the research runs). Wine is not Windows |
| Updater signatures per package format | With `createUpdaterArtifacts` and a test key: AppImage, `.deb`, `.rpm` (release build) and the cross-built Windows NSIS setup each got a `.sig`; without a key the build stops ("A public key has been found, but no private key"), which is why CI enables it only when the secret exists. MSI and macOS `.app.tar.gz` are built only by CI |
| Release manifest on real artifacts (`scripts/release-manifest.mjs`) | 11 platform entries with space-free asset URLs (GitHub would turn spaces into dots), signatures copied byte for byte, and a stale 0.1.0 package in the folder left out (MSI and macOS stand-ins) |
| Windows and macOS updater install | **Not run here** (see below) |

**Market research, root causes confirmed before fixing (2026-10-11):**
- `overpass-api.de` answered HTTP 504 to `node(1);out;` repeatedly; `overpass.private.coffee`
  answered 500 to GET and 200 to POST; `lz4.`/`z.overpass-api.de` worked. The app used GET on the
  first server only, so discovery failed.
- The query ended `out center tags 300`: Seattle within 6–8 km returned exactly 300 elements, while an
  uncapped query with the broader tags returned 586.
- Only `shop=hairdresser|beauty` were searched; barbers, `shop=nails`, spas and massage were missed
  (Tumwater 61 → 74 businesses with the broader tags).

**Price extraction against real menus** (saved locally from public sites, not committed; synthetic
fixtures in the unit tests reproduce each layout):

| Site (platform) | Layout | Result |
|---|---|---|
| industry-salon.com/color, /cut | Heading + "Stylist level \| $price" rows, 6 tiers | 96 prices, each with its tier; a row missing its "$" is skipped, not guessed |
| salonvoda.com/services-1 (Wix) | "BLOWOUT $55+" | 14 prices, all "starting at"; "PARTIAL/FULL FOIL $155/$200+" split into two services; 3 consultation-only services recorded |
| 19thavenuesalon.com/services | "Haircut - $110 - $115", add-on alternatives | 19 prices; ranges kept as ranges |
| square.site booking page (Square Online) | name / description / price / "· 1 hour" in separate elements | 17 of 17 prices with durations (before the fix: 0) |
| havegoodnail.com/services (Squarespace) | menu tabs; price printed before and after each title | 35 prices, each with its own item and section ("PEDICURE — Classic"); before the fix, prices attached to the previous item's title |

**slice 7 live research run** (ZIP 98501, 4 mi, demo workspace): 31 businesses found on
OpenStreetMap (all new), 36 within the radius, 5 with websites, all 5 read: 2 with prices
(79 saved, 48 for services the demo doesn't offer, 8 unrecognised amounts dropped), 2 with no prices
online, 1 asks robots not to read it. The update check reported "No update information is published
yet" (no 0.2.0 release with latest.json exists yet), as designed.

**In-app update test details.** The AppImages were debug builds made in the Fedora toolbox with
`--config` setting the version, `createUpdaterArtifacts`, the test public key and
`http://127.0.0.1:8765/latest.json` (with `dangerousInsecureTransportProtocol`, test only); the
script serves `latest.json` with a `linux-x86_64-appimage` entry. Found while testing: an AppImage
carries its own WebKitGTK, so it must be driven by a matching WebKitWebDriver (the Ubuntu-built one
couldn't be automated from Fedora); and `app_status` reports the Cargo version while the updater
uses `tauri.conf.json`'s, so CI now refuses a tag unless `package.json`, `Cargo.toml` and
`tauri.conf.json` all match it.

**Not validated here:** installing an update on Windows (NSIS/MSI) and macOS (the plugin's
`.app` replacement), and `.deb`/`.rpm` installs through `pkexec` (they need an interactive password).
CI proves those packages build and are signed into `latest.json`; the first real check happens when
a 0.2.x release is published and an installed 0.2.0 updates from it.

## 0.1.0 (2026-10-04)

Date: 2026-10-04. Host: Fedora 44 Workstation (x86_64, Wayland). Builds and tests ran in rootless
toolbox containers: `srm-fedora` (Fedora 44, WebKitGTK 2.54) and `srm-ubuntu` (Ubuntu 22.04,
WebKitGTK 2.50, glibc 2.35). Install tests used separate clean containers (`srm-fedora-clean`,
`srm-ubuntu22-clean`, `srm-ubuntu24-clean`). Rust 1.99, Node 22.23, Tauri 2.12.

### Summary

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
| Windows and macOS on GitHub Actions runners ([CI run #2](https://github.com/fleetwoodjohnr/salon-management/actions/runs/37251648909), commit `aee9250`) | **Passed**: Rust tests on Windows and macOS; NSIS, MSI and universal `.dmg` built; Windows silent install plus launch, and macOS launch with signature and architecture checks |

### Automated tests (Rust)

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

### Large dataset (release build)

`cargo test --release large_dataset -- --ignored --nocapture` generates two years of history
(about 16 sales per open day):

| Measure | Result |
|---|---|
| Sales / sale lines generated | 9,888 sales, 18,601 lines (through the normal finalize path) |
| Generation time | 46 s |
| Two-year report (3 groupings, weekly trend, prior period) | 0.43 s |
| Product list + 2,000-row sales list | 8 ms |

### Real desktop app end-to-end

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

### Packaging

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

### Windows and macOS

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

**GitHub Actions** (`.github/workflows/ci.yml`, on every push) is set up to:
- run the Rust tests on real Windows and macOS runners;
- build the NSIS and MSI installers, and the universal (Apple Silicon + Intel) macOS app and `.dmg`;
- silently install the Windows build and launch it, launch the macOS app and check its ad-hoc
  signature and architectures, and confirm each one starts and creates its data folders.

**Result, 2026-10-05:** [CI run #2](https://github.com/fleetwoodjohnr/salon-management/actions/runs/37251648909)
on commit `aee9250` passed all 9 jobs in about 20 minutes:

| Job | Runner | Result |
|---|---|---|
| Rust tests | `ubuntu-22.04`, `windows-latest`, `macos-latest` | Passed on all three (`cargo test` exited 0, so every non-ignored test passed) |
| Build Windows `nsis,msi` | `windows-latest` | Built. Smoke test passed: `setup.exe /S` installed to `%LOCALAPPDATA%\Salon Resource Manager`; the installed app was still running after 15 s and had created its data folders |
| Build macOS `universal-apple-darwin` `app,dmg` | `macos-latest` | Built. Smoke test passed: `codesign --verify --deep --strict` accepted the ad-hoc signature; `lipo` listed the architectures; the app was still running after 15 s and had created its data folders |
| Build Linux `deb,appimage,rpm` | `ubuntu-22.04` | Built. Smoke test under Xvfb passed |

The installers are attached to the run as artifacts (Windows 13 MB, macOS 15 MB, Linux 104 MB),
kept until 2027-01-03.

What this does **not** cover:
- The Windows runner is Windows Server, not a Windows 10/11 desktop.
- Neither runner ran the end-to-end workflow, printing or the OS keyring.
- No person has used the app on either system.

macOS-specific changes made without being able to run them:
- `core:webview:allow-print`, granted on macOS only, because Tauri routes `window.print` through
  it there.
- Ad-hoc signing, so Apple Silicon will start the app.
- ⌘ instead of Ctrl in the shortcut hint.

### Not tested

- **Windows and macOS beyond the CI smoke checks above**: no end-to-end workflow, printing, keyring
  or hands-on use on those systems; no Windows 10/11 desktop (the CI runner is Windows Server).
- A physical Wayland desktop session (tests used X11 via Xvfb on a Wayland host).
- Live Census Data API calls (needs a key) and the BLS v2 key path.
- Saving a PDF through the GTK print dialog.
- OS keyring on Ubuntu, KDE (KWallet) or headless sessions; the app reports a clear error when no
  Secret Service is available.
- Screen readers.
