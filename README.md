# Salon Resource Manager

A free, offline-first desktop app for independent salon professionals and small salons. It works
out what each service really costs, recommends prices you can defend, and keeps inventory, clients,
appointments, sales, sales tax and profitability in one place on your own computer.

- **No account, no subscription, no cloud.** Everything works offline. Your data stays in a local
  SQLite database.
- **Exact money.** All arithmetic uses exact decimals (no binary floating point), with documented
  rounding rules.
- **Honest numbers.** Missing tax information is never treated as 0%. Market statistics are never
  passed off as prices. Every dashboard figure can be traced to the records behind it.

MIT licensed. Linux (Fedora, Ubuntu) is the primary, tested platform. Windows and macOS installers
are built too; see [docs/test-report.md](docs/test-report.md) for exactly what has been verified on
each.

## What it does

| Area | Highlights |
|---|---|
| Work profiles | Owner pay, wages, commission (never double-counted), hours, realistic billable utilization, overhead per billable hour, card fees, profit target. Versioned. |
| Inventory | Grams, kilograms, weight ounces, pounds, mL, L, US fluid ounces, gallons, pieces and custom units (pump, scoop…). Density-gated weight↔volume conversion. Landed cost per unit, moving weighted average, append-only stock ledger with reversals, lots and expiry, low-stock and reorder lists, CSV import/export. |
| Services | Recipes (versioned), hands-on vs processing vs setup/cleanup time, waste allowance, variants (length, density, level), add-ons, bundles, live cost breakdown. |
| Pricing | Break-even and target price solved exactly even when commission and card fees depend on the price; margin vs markup; rounding rules; what-if panel that explains every change; market-position suggestion and conflict warnings; override notes. |
| Sales tax | Separates jurisdiction, rate and taxability. Official free address lookups for Washington (DOR) and California (CDTFA); manual multi-component rates elsewhere. Versioned settings; rates and decisions frozen on each sale; stale-data detection; tax-inclusive or exclusive pricing. |
| Clients and calendar | Client records with sensitivities, history and versioned formulas. Day/week calendar by staff, drag to reschedule, overlap detection, estimates that convert to bookings. |
| Sales | Checkout from an appointment with planned vs actual product usage, retail, discounts, tips, manual payments, partial/full refunds (explicit restock), void, printable receipts. Stock is deducted exactly once on finalize. |
| Dashboards and reports | Customizable dashboard (add, remove, move, resize, per-widget filters, saved views, drill-down, CSV/PDF), period statement, profitability by service/profile/staff/product, sales-tax report, estimated vs actual. |
| Market research | Competitor discovery from OpenStreetMap (names/places only), prices you observe or import (with source and date), quartiles, outliers, recency/distance rules and an evidence-quality rating. Census and BLS context kept separate from prices. |
| Safety | Separate workspaces (including a clearly labelled demo), automatic and manual backups with checksums, verified restore, migration backups, portable CSV export, audit log. |

## Download

**[Download the latest release](https://github.com/fleetwoodjohnr/salon-management/releases/latest)**,
then choose the file for your system:

| System | File |
|---|---|
| Windows 10 / 11 | `Salon.Resource.Manager_<version>_x64-setup.exe` (or the `.msi`) |
| macOS 11+ (Apple Silicon and Intel) | `Salon.Resource.Manager_<version>_universal.dmg` |
| Fedora | `…x86_64.rpm` |
| Ubuntu 22.04+ / Debian | `…amd64.deb` |
| Other Linux | `…amd64.AppImage` |

## Install

**Fedora (RPM)**

```sh
sudo dnf install ./Salon.Resource.Manager-0.1.0-1.x86_64.rpm
```

**Ubuntu 22.04 / 24.04 (DEB)**

```sh
sudo apt install ./Salon.Resource.Manager_0.1.0_amd64.deb
```

**Any recent Linux (AppImage)**

```sh
chmod +x Salon.Resource.Manager_0.1.0_amd64.AppImage
./Salon.Resource.Manager_0.1.0_amd64.AppImage
```

The packages depend on the system WebKitGTK 4.1 and GTK 3 (installed automatically by dnf/apt).

**Windows 10 / 11**

Run `Salon.Resource.Manager_0.1.0_x64-setup.exe`. It installs for your user only (no administrator
rights) and adds a Start-menu entry. The installer isn't code-signed, so SmartScreen may say
"Windows protected your PC": choose *More info → Run anyway*. The app uses Microsoft Edge WebView2,
which Windows 11 and up-to-date Windows 10 already have; if it's missing, the installer downloads it.
Uninstall from *Settings → Apps*.

**macOS 11 or later (Apple Silicon and Intel)**

Open `Salon.Resource.Manager_0.1.0_universal.dmg` and drag the app to Applications. The app isn't
notarized by Apple (that needs a paid developer account), so the first time macOS refuses to open it:
open *System Settings → Privacy & Security*, scroll to the message about Salon Resource Manager and
choose *Open Anyway*.

Start it from your applications menu ("Salon Resource Manager") or run `salon-resource-manager`.
On first start, create a business workspace or open the demo workspace to look around.

## Where your data lives

| System | Folder |
|---|---|
| Linux | `~/.local/share/app.salonresourcemanager.desktop/` |
| macOS | `~/Library/Application Support/app.salonresourcemanager.desktop/` |
| Windows | `%APPDATA%\app.salonresourcemanager.desktop\` |

Inside it: `workspaces.json`, `workspaces/<id>/salon.db` (+ `attachments/`) and `backups/<id>/`.
Set `SRM_DATA_DIR` to use another folder (for example a portable install).

Never put a workspace folder in a synced folder (Dropbox, Nextcloud…) used by two computers at
once: SQLite files are not safe to share that way. Use backups to move data.

## Back up, restore, update

- **Automatic:** a backup is made once a day when a workspace opens (newest 14 kept), before any
  data-format upgrade, before a restore replaces data, and before a workspace is deleted.
- **Manual:** Settings → Workspaces and backups → *Back up now* or *Save backup as…* (a `.zip`
  containing a consistent database snapshot, attachments and a checksummed manifest).
- **Restore:** *Restore from file…* checks the checksums, the database's integrity and references,
  and the data version before anything changes. By default it restores as a **new** workspace; you
  can choose to replace the open one (a safety backup is taken first and the old folder is kept).
- **Portable export:** *Export all data (CSV)…* writes every table to CSV files in one zip.
- **Update:** install the newer package over the old one (`dnf install`/`apt install` the new file,
  or replace the AppImage). On first open the app backs up each workspace, then upgrades it.
  Older app versions refuse to open data saved by newer ones instead of damaging it.

## Optional online lookups

Nothing is sent anywhere unless you press a lookup button. Settings → Data providers shows each
service, its limits, terms, usage today and last error. Only the minimum is sent (an address, ZIP,
or area code); client records never leave your computer. Free API keys (Census, optional BLS) are
stored in your operating system's keyring, never in the database, backups or exports.
Details: [docs/providers.md](docs/providers.md).

## Build from source

Requirements: Rust (stable), Node.js 20.19+ (22 LTS recommended), and Tauri's Linux prerequisites.

```sh
# Fedora
sudo dnf install webkit2gtk4.1-devel openssl-devel librsvg2-devel libxdo-devel libappindicator-gtk3-devel gcc gcc-c++
# Ubuntu
sudo apt install libwebkit2gtk-4.1-dev build-essential curl file libssl-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev

npm ci
npm run tauri dev                     # run with live reload
npx tauri build --bundles rpm         # Fedora package
npx tauri build --bundles deb,appimage  # build on Ubuntu 22.04 for the widest compatibility
```

Packages land in `src-tauri/target/release/bundle/`. Build `.deb`/AppImage on the oldest system you
want to support (glibc is forward- but not backward-compatible).

**Windows** installers can be built on Windows (`npx tauri build --bundles nsis,msi`, needs the
Visual Studio C++ build tools) or cross-compiled on Linux with `scripts/build-windows.sh` (NSIS only;
see the script for its one-time setup). **macOS** builds need a Mac:
`rustup target add aarch64-apple-darwin x86_64-apple-darwin` then
`npx tauri build --target universal-apple-darwin --bundles app,dmg`.

**GitHub Actions** (`.github/workflows/ci.yml`): every push runs the Rust tests on Linux, Windows
and macOS, builds unsigned installers for all three systems, launches each once as a smoke test, and
attaches the installers to the run (*Actions → CI → the run → Artifacts*). Pushing a version tag
(`git tag v0.2.0 && git push origin v0.2.0`, after bumping the version in `package.json`,
`src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`) also publishes a GitHub Release with all the
installers attached.

This repository's own development used rootless toolbox containers (no host changes):
`scripts/tb` (Fedora 44) and `scripts/tb-ubuntu` (Ubuntu 22.04).

## Tests

```sh
cargo test --manifest-path src-tauri/Cargo.toml          # 88 unit + integration tests (2 more with --ignored)
npx tsc --noEmit && npx vitest run                         # frontend typecheck + tests
scripts/e2e-all.sh                                         # real desktop app E2E (needs Xvfb, tauri-driver, WebKitWebDriver; build with `npx tauri build --debug --no-bundle` first)
```

See [docs/test-report.md](docs/test-report.md) for what was run and what wasn't.

## Documentation

- [User guide](docs/user-guide.md)
- [Calculations and definitions](docs/calculations.md)
- [Architecture](docs/architecture.md)
- [Database](docs/database.md)
- [Data providers and APIs](docs/providers.md)
- [Test report](docs/test-report.md)
- [Known limitations](docs/limitations.md)
- [Development log](PROGRESS.md)

## Signing

Linux packages need no signing to install. Windows (Authenticode) and macOS (Developer ID and
notarization) signing are optional and need paid certificates; unsigned builds show OS warnings.
None of this is required for Linux use.
# salon-management
