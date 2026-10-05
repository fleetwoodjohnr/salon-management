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

- **Competitors**: search OpenStreetMap within a radius of your location (locate your business in
  Settings → Locations first; a street address, ZIP code or city works), or add businesses yourself.
- **Prices**: record prices you see on a business's own menu, with source and date, or import a CSV.
  Choose which comparisons count (age, distance, hair length, "starting at" prices); the evidence
  rating explains itself.
- **Local context**: Census income, population and salon counts (needs a free Census key in
  Settings → Data providers) and BLS wages and price trends. These are context, not prices.

## Workspaces and backups (Settings → Workspaces and backups)

Back up now, save a backup to a file you choose, restore (as a new workspace, or replacing the open
one after a safety backup), export everything as CSV, rename or delete workspaces, and create or reset
the demo. Automatic backups run daily. The Activity log tab lists recorded changes.
