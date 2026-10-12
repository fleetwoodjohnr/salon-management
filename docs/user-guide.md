# User guide

## First start

1. **Create a workspace** with your business name (or open the demo workspace to look around first;
   it's separate and labelled, and never mixes with your data).
2. **Set up** (each step can be skipped and finished later in Settings):
   - Business: name, time zone, preferred units, whether your prices include tax.
   - Location: address (needed for tax lookups and local market features).
   - Hours: your working week.
   - Work profile: how you're paid, hours, realistic utilization, monthly overhead. A staff member
     using this profile is created for the calendar.

Keyboard: `Ctrl+K` go to any page · `Ctrl+1…9` jump to pages · `Ctrl+N` new item · `Ctrl+S` save. On a Mac use `⌘`
instead of `Ctrl`.

## Work profiles (Setup → Work profiles)

A profile says how someone's time is paid for and how much overhead they carry.

- **Owner pay per hour**: what you want to earn per hour worked. **Hourly wage** (+ employer
  payroll costs), **commission only**, or **hourly plus commission** for employees.
- **Utilization**: the share of working hours spent with paying clients (most people: 60–85%).
- **Overhead**: the monthly share this person carries (chair renters: chair or booth rent).
- **Fees and targets**: card processing, profit target (margin or markup), market position and price
  rounding.

The panel on the right shows overhead and pay per billable hour as you type. Saving creates a new
version; finished sales keep the figures they were costed with.

## Inventory

1. **Add products** (Inventory → New product), or **Import** a CSV (map columns, preview, then
   import; duplicates by SKU, barcode or name+brand are skipped; optional opening stock).
   - Pick the stock unit you think in (fl oz, g, pieces…). Weight ounces and fluid ounces are
     different units.
   - Add a density to convert between weight and volume. Add custom units such as "pump = 2 mL".
2. **Receive stock** (Inventory → Receive stock): one line per product with packages × contents and
   the line price. Invoice discount, shipping and non-recoverable purchase tax are spread across lines
   to give each product its landed cost. Attach the receipt. Use "Opening stock" for what you already
   had.
3. **Day to day**: count adjustments, waste, transfers between storage spots, returns to the supplier
   (product page → Stock). Mistakes are corrected with a reversal; the original entry stays in the
   ledger.
4. **Reorder and expiry**: set reorder points; the Reorder tab lists what's low (copy or print it)
   and lots expiring in the next 60 days.

## Services and recipes (Catalog → Services)

Enter hands-on, processing, setup and cleanup minutes; the products one service uses (the recipe);
a waste allowance; other per-service costs; variants such as hair length (extra minutes, a material
multiplier, a price change); and your price. The right panel shows the full cost, profit, margin,
tax, what the client pays, break-even and the price your target needs ("Use" copies it). A price
below target needs a short note. Bundles (Services → Bundles) price several services together.

## Pricing (Catalog → Pricing)

Pick a service and try different assumptions: target, market position, overhead, utilization,
minutes, material use, price. "Why it changes" explains each difference. Market evidence shows
observed local prices and any conflict with your cost-based target. Save a price when you've decided.

## Sales tax (Setup → Sales tax)

1. **Rate**: in Washington or California, press *Look up official rate*, check the matched address and
   use it. Elsewhere, enter the rate (and parts, if you know them) from your state revenue department.
2. **What's taxable**: mark salon services, retail products and tips as taxable or not, and record
   where that came from. Add categories (e.g. nail services) if your state treats them differently.

Until both are done, sales can be drafted (with an estimated tax) but not finalized. Rates are
flagged stale when a new quarter starts.

## Calendar, clients and checkout

- **Book**: click an empty slot (or *New appointment*), pick or create the client, services and
  variants; the end time follows the services' chair time. Overlaps for the same person and bookings
  outside opening hours are flagged before saving. Drag to move or resize.
- **Estimates** (Sales → Estimates): print a quote, then *Book it* to turn it into an appointment.
- **Check out**: click the appointment → *Complete and check out*. Adjust what product was actually
  used and the actual chair time, add retail items, discounts and a tip, record payments, then
  **Finalize sale**. Finalizing deducts stock once and locks the sale. Print the receipt (choose
  "Print to File" for a PDF).
- **Corrections**: *Refund* (all or part; choose whether retail items go back to stock) or *Void* for
  a sale entered by mistake.
- **Clients**: contact details, allergies/sensitivities (shown prominently), visit history and
  versioned formulas.

## Money (Expenses, Reports, Dashboard)

- **Expenses**: rent, utilities, software, wages and other spending (stock purchases belong in
  Inventory).
- **Dashboard**: choose dates and filters at the top. *Edit layout* to add, remove, move or resize
  widgets and *Save as new view*. Each widget's menu offers its own filters (shown as a blue tag),
  *Show underlying sales*, and CSV export. *PDF* prints the dashboard.
- **Reports**: the period statement (actual result after recorded expenses), allocated service
  contribution, profitability by service/profile/staff/category/product, and the sales tax report.
  Definitions are in [calculations.md](calculations.md).

## Market (Money → Market)

- **Research**: find competitors and the prices they advertise. Choose where to search (one of your
  located addresses, a ZIP code or a city), how far (the slider and box follow your distance unit)
  and which kinds of business (hair salons, barbers, nail salons, beauty and skin, massage), then
  *Start research*. The app searches OpenStreetMap, merges businesses it finds more than once (and
  ones you already had), then reads each competitor's own website for the prices it publishes. You
  can keep working while it runs; progress and every website's outcome appear as they happen, and
  *Stop* keeps what was found so far. Sites read in the last week are skipped unless you tick
  *Re-check*. The coverage summary says how many businesses were found, how many have websites, how
  many were read, how many had usable prices, and why the others didn't (no prices online, PDF or
  picture menu, the site asks not to be read, platform terms, blocked, couldn't reach).
  *Add a competitor by website* adds a business the map doesn't know: its name and address are read
  from its home page when allowed, then it's checked for prices.
- **Overview**: one row per service: your price, how many prices from how many businesses, lowest,
  median, average and highest, a spread chart (with your price marked), where your price sits
  (percentile), and the recommended price. Services competitors offer that you don't are listed too.
  Filter by service type, distance, competitor, source (advertised online, recorded by you,
  imported) and price range. Click a row to open that service's evidence.
- **Prices**: the evidence for one service. Every price shows its business, distance, what the
  business called it, the price type (fixed, from, range), its source (with a link to the page and
  when it was read) and whether the match to your service was *Inferred* automatically or made *By
  you*. Use the ⇄ button to match a price to a different service or mark it not comparable; your
  choice is kept when the site is checked again. Choose which comparisons count (age, distance,
  hair length, source, competitors, "starting at" prices); each excluded price says why. A business
  listing several prices for one service (stylist tiers) counts once, at its middle price.
  The **Recommended price** card shows the observed market price for your market position, the price
  your costs and target margin need (calculated from materials at inventory cost, labor time,
  overhead, fees and commission), and the recommendation: the market price when it covers your costs,
  otherwise the cost-based price. Type a different price and press *See margin in Pricing* to try it
  in the what-if panel, where you can save it to the service.
- **Competitors**: everyone found or added, with distance, source, the outcome of the last website
  check (click it for every page looked at, including services listed "price on consultation"),
  and *Check prices* to read one website again now. Filter by name, source, website outcome and
  distance; add businesses by hand or by website.
- **Import prices**: a CSV of prices you collected, one row per business and service, with the date.
- **Local context**: Census income, population and salon counts (needs a free Census key in
  Settings → Data providers) and BLS wages and price trends. These are context, not prices.

## Units and updates (Settings → General, Settings → Application updates)

- **Units**: show distances in miles or kilometres. Distances are stored exactly and only
  converted for display, so switching never changes a search radius or filter.
- **Application updates**: shows your version and how it was installed. *Check for updates* asks
  GitHub for the newest release and shows its version, date and changes; *Download & install update*
  backs up the open workspace, downloads the right package for your computer (with progress and
  speed), verifies it against the project's signing key and installs it. On Linux and macOS press
  *Restart now* afterwards; on Windows the installer closes and reopens the app. A .deb or .rpm
  install asks for your password. Your workspaces, backups and settings are never touched. Turn
  *Check for updates automatically* off to stop the once-a-day check at startup.

## Workspaces and backups (Settings → Workspaces and backups)

Back up now, save a backup to a file you choose, restore (as a new workspace, or replacing the open
one after a safety backup), export everything as CSV, rename or delete workspaces, and create or reset
the demo. Automatic backups run daily. The Activity log tab lists recorded changes.
