# Changelog

Each version's section becomes its GitHub Release notes and the "What's new" text shown in
Settings → Application updates.

## [0.2.0] - 2026-10-11

- **Built-in updates.** Settings → Application updates checks GitHub Releases, shows what's new, and
  downloads and installs the right package for your system (AppImage, .deb, .rpm, Windows installer,
  macOS app) with a progress bar. Every update is verified against the project's signing key before
  it's installed, and the open workspace is backed up first. Optional daily check at startup.
- **Miles or kilometers.** Settings → General → Units. Search radius, distances and filters follow
  your choice; saved distances never change when you switch.
- **Market research that works.** The Market page's new Research tab finds salons, barbers, nail
  and beauty businesses on OpenStreetMap (several public servers, no result cap), merges duplicates,
  and reads competitors' own websites for the prices they advertise, respecting robots.txt and site
  terms. Every price keeps its source page, the time it was read and the exact text. Prices on
  consultation, PDF menus, blocked and unreachable sites are reported, never guessed.
- **Market overview and recommendations.** Average, median, range, spread chart and your percentile
  per service; a recommended price that combines the observed market price for your position with
  your cost-based target; filters by service type, distance, competitor, source and price.
- Fixed: discovery failed when the main OpenStreetMap server was overloaded; dense areas were cut off
  at 300 results; re-running discovery overwrote details you had corrected; website links without
  "https://" didn't open.

## [0.1.0] - 2026-10-04

- First release: work-profile costing, inventory, service recipes, pricing, sales tax, clients,
  appointments, sales, dashboards and market evidence; Windows, macOS and Linux installers.
