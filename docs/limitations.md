# Known limitations

Things the app deliberately doesn't do, can't do with free data, or hasn't been verified.

## Data availability

- **Automatic prices only from competitors' own websites, and only some of them.** No free API has
  salon prices. Measured on 2026-10-11: OpenStreetMap lists a website for about 6% (Indianapolis), 13%
  (Tumwater WA) and 50% (Seattle) of mapped salons; of 30 real salon websites tried, about 6 showed
  prices as readable text and 3 asked robots not to read them. The rest put prices in pictures,
  PDFs or booking widgets, or don't publish them. Expect usable prices from a minority of nearby
  businesses, and more in cities than small towns. Add websites the map lacks, and type or import
  prices you collect; without enough prices the app says "Insufficient observed market prices" and
  recommends prices from costs alone.
- **Not every source can be searched.** Search engines, review sites, directories and booking
  platforms aren't used: their free APIs are gone or need billing, and their pages forbid automated
  access (see providers.md). The app never claims to have searched everything; each search reports
  what it checked and what it couldn't.
- **Reading menus is heuristic.** Prices are only taken when a dollar amount sits with a recognisable
  service name. Unusual layouts can still mislabel a price, so every price links to its page and the
  exact text, automatic matches to your services are marked "Inferred", and you can exclude or
  re-match any of them.
- **OpenStreetMap coverage varies.** Discovery finds only businesses someone has mapped (new and
  home-based salons are often missing), and public mirrors can lag the live map by months.
- **Census Data API requires a free key** (requests without one are refused). The app's Census
  context features were tested against a sample response only; live calls weren't run.
- **BLS v1 without a key** allows small requests; the app caps itself at 25 a day. Wages are annual
  and statewide or national, not local salon pay rates.
- **Offline locating** uses the centre of the ZIP area (ZCTA) or city from the bundled 2025 Census
  Gazetteer, so distances to competitors are approximate unless you locate by street address.
- **ZIP vs ZCTA:** Census figures are for ZIP Code Tabulation Areas, which approximate but don't equal
  postal ZIP codes. County Business Patterns uses postal ZIP codes and counts only employer
  establishments (booth renters aren't included).

## Sales tax

- Official free address lookups are integrated for **Washington and California only**. Everywhere
  else rates are entered manually.
- A successful lookup gives a rate for a location. It is not a ruling on what's taxable and doesn't
  guarantee compliance; taxability is your recorded decision.
- CDTFA doesn't state the period its rates cover; the app treats such rates as stale once a new
  quarter begins.
- The app calculates and reports sales tax. It doesn't file returns or handle income tax, payroll tax
  or use tax on purchases (non-recoverable purchase tax is simply part of landed cost).
- Tax is computed per sale at one location's rate; destination-based delivery sales aren't modelled.

## Business model limits

- **USD only** and US sales-tax concepts. Distances can be shown in miles or kilometres; product
  units are US and metric.
- **Single location per sale**; multi-location salons work, but stock is valued per product across all
  storage spots (quantities are tracked per spot).
- **Moving average costing** doesn't track which lot was used, so expiry alerts say a lot *may* still
  be on the shelf.
- **Commission isn't reversed on refunds**, and processing fees on refunded card payments aren't
  modelled (processors differ).
- Card fees are estimated from the staff member's work-profile processing settings; they aren't read
  from a processor.
- **No payment processing**, gift cards, memberships, loyalty, online booking, SMS/email reminders or
  payroll.
- **Appointments use local times without time-zone conversion.** The computer's time zone is assumed
  to match the business's.
- Recurring expenses must be entered each time.

## Sharing and sync

- One workspace = one SQLite file for **one computer**. Shared real-time use across computers is out
  of scope; putting the file in a synced folder used by two machines can corrupt it. Move data with
  backups.

## Platforms and packaging

- Built, installed and run on **Fedora 44** (RPM) and **Ubuntu** (deb/AppImage) — see the test report
  for exactly what was checked.
- **No code signing by a certificate authority.** The Windows installer is unsigned (SmartScreen
  warning on first run) and the macOS app is only ad-hoc signed and not notarized (macOS blocks the
  first open until you choose *Open Anyway*). Both need paid certificates. In-app updates are
  verified with the project's own signing key, which is free.
- **In-app updates start with 0.2.0.** Copies of 0.1.0 have no updater; install 0.2.0 by hand once.
  Updates need the release to be built by CI with the signing key configured. A .deb or .rpm update
  asks for an administrator password (pkexec). An AppImage in a folder you can't write to can't
  replace itself. The Windows and macOS install paths were not exercised hands-on (no earlier signed
  release existed to update from); only the Linux AppImage path was tested end to end.
- The Windows installer built on Linux is NSIS only; the `.msi` comes from a Windows machine or CI.
- Windows and macOS were verified only by the automated CI checks in the test report (build, unit
  tests, install and launch on GitHub runners), not by hands-on use.
- The GTK print dialog opens from the app, but saving a PDF through it wasn't automated in testing.
- The real-app end-to-end tests run on an X11 virtual display; they weren't run on a physical
  Wayland session.

## Accessibility

- Keyboard navigation, visible focus, labelled controls and contrast follow the component library's
  defaults plus app-specific labels. No formal screen-reader audit has been done.
