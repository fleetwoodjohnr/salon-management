# Calculations and definitions

Every calculation lives in the Rust core (`src-tauri/src/domain/` and `src-tauri/src/db/`); the
interface only displays results. This page is the reference for what each number means.

## Numbers and rounding

- All amounts, rates and quantities are exact decimals (`rust_decimal`, 28 significant digits,
  base 10). Nothing uses binary floating point. They are stored as text in SQLite.
- **Currency rounding:** to the cent, **half away from zero** ($0.125 → $0.13; −$0.125 → −$0.13),
  only when a figure becomes something a person sees or pays: a line total, a tax total, a
  document total, a price recommendation.
- **Inventory cost assignments** are rounded to 10 decimal places (a ten-billionth of a dollar) so
  the stock ledger always sums exactly to each product's stored value.
- Percentages are entered as percents (35 means 35%).
- Charts convert values to ordinary numbers for drawing only.

## Units

Base units: grams (mass), millilitres (volume), pieces (count). Exact conversion factors:

| Unit | Base | Factor |
|---|---|---|
| kilogram | g | 1000 |
| ounce (weight, avoirdupois) | g | 28.349523125 |
| pound | g | 453.59237 |
| litre | mL | 1000 |
| US fluid ounce | mL | 29.5735295625 |
| US gallon | mL | 3785.411784 (= 128 fl oz) |

- Weight ounces and fluid ounces are different units and never interchangeable.
- Weight ↔ volume conversion happens only for a product with a recorded density (g/mL). Water's
  density is never assumed.
- Count never converts to weight or volume. Custom units (pump, scoop, application, tube) are
  defined per product as an amount of a standard unit, e.g. "1 pump = 2 mL".

## Work profiles

- Monthly hours worked = weekly hours × weeks worked per year ÷ 12.
- Billable hours per month = monthly hours × billable utilization.
- **Overhead per billable hour** = monthly overhead (rent or chair rental, utilities, insurance,
  software, other) ÷ billable hours.
- Pay per hour worked: owner target pay; or wage × (1 + employer burden); or 0 for commission-only.
- **Labor cost per billable hour** = pay per hour worked ÷ utilization (non-billable time is paid
  for by billable time).
- Commission applies only under the "commission" and "hourly plus commission" models; other
  models ignore any commission figure (and say so), so labor is never counted twice.
- Zero billable hours is rejected.

## Service cost (before price-dependent fees)

- Working time = hands-on + setup + cleanup. Chair time = working time + processing time.
- Materials = Σ recipe quantity × the product's current moving-average cost (estimates) or the
  cost frozen in the ledger when it was actually used (completed sales).
- Estimated waste = materials × waste allowance %, shown separately.
- Labor = working hours × labor cost per billable hour (processing time is not labor).
- Overhead = hours × overhead per billable hour, where hours are chair time (default) or working
  time (profile option, for people who serve others while color processes).
- Variants add minutes, multiply recipe quantities and adjust the price.
- **Cost before fees (F)** = materials + waste + other direct costs + labor + overhead.

## Pricing

With list price L and tax rate t (fraction; 0 when exempt):

| | Tax added at checkout | Prices include tax |
|---|---|---|
| Revenue R | L | L ÷ (1 + t) |
| Customer pays T | L × (1 + t) | L |

Commission c is charged on R (the pre-tax service price). Card processing is p × T + p₀ per card
payment, scaled by the share s of payments made by card.

- Cost(L) = F + s·p₀ + c·R + s·p·T
- Profit = R − Cost; **margin** = profit ÷ R; **markup** = profit ÷ total cost.
- **Target margin m:** L = (F + s·p₀) ÷ (α(1 − c − m) − s·p·β), where R = αL and T = βL.
- **Target markup k:** L = (1 + k)(F + s·p₀) ÷ (α − (1 + k)(c·α + s·p·β)).
- **Break-even** = the margin formula with m = 0.
- Results are rounded up to the cent so the price really reaches the target. A denominator of zero
  or less means the percentages consume the whole price; the app names them instead of producing a
  price.
- Simple check: cost $60 at a 40% margin → 60 ÷ 0.6 = $100; at a 40% markup → 60 × 1.4 = $84.
- **Rounding rule:** to an increment ($0.01, $0.25, $0.50, $1, $5, $10), up / nearest / down. Margin
  is recalculated after rounding; rounding down below the target is flagged.
- A price below break-even or below target is allowed only with an explanatory note.
- **Earnings per hour** = (profit + labor) ÷ hands-on hours, and ÷ chair hours.
- When tax for a service is unresolved, processing on tax is left out and the estimate says so.

### Market position

When observed local prices have medium or high evidence quality, the suggestion for each position
is a statistic of the comparable prices: budget = lower quartile, standard = median, premium = upper
quartile, luxury = 90th percentile. If that is below the cost-based target price, the app shows the
conflict ("you can't meet both") rather than claiming both are satisfied. With low or no evidence
there is no suggestion and pricing continues from costs.

## Inventory (moving weighted average)

Each product keeps on-hand quantity Q (base units) and value V.

- Receive q at landed cost C: Q += q, V += C.
- Issue q (service use, retail sale, waste, supplier return, negative count): cost = V × q ÷ Q
  (multiplying first, so terminating results are exact). Issuing everything on hand costs exactly V.
- The cost is stored on the ledger row and never recalculated.
- Issuing more than is on hand: the excess is costed at the last average and stock goes negative
  (with a warning). The next receipt values the remaining stock at the new purchase cost and posts
  the difference as a `revaluation` row.
- Corrections are new rows that exactly negate the row they reverse.
- Transfers move quantity between storage spots; value is per product and doesn't change.
- **Landed cost**: line price − discount share + shipping share + non-recoverable tax share, where
  invoice-level amounts are split across lines by line price to the cent (largest remainder).
- Acceptance example: one 32 fl oz bottle at $24 landed → $0.75 per fl oz; using 2 fl oz costs
  $1.50 and leaves 30 fl oz worth $22.50.

## Sales

- Line gross = round(qty × unit price) − line discount.
- A sale-level discount is split across service and retail lines by gross (to the cent, largest
  remainder). Tips are never discounted.
- **Tax** is computed once per sale on the total taxable amount and rounded once; it is then split
  across taxable lines by amount so refunds can return the right share. Tax-inclusive: tax = base ×
  t ÷ (1 + t).
- A line whose taxability is undecided, or a taxable sale with no rate, is **unresolved**: drafts show
  an estimate and finalizing is blocked. Missing data never becomes 0%.
- Finalizing (one database transaction) deducts actual product usage and retail items from stock,
  and freezes on each line: net revenue, tax and the taxability decision and its basis, the rate set
  used, materials cost from the ledger, planned materials cost, labor and overhead from the work
  profile version (actual chair time, when recorded, scales the planned split), commission, and
  retail cost of goods.
- Card fee per card payment = amount × processing % + fixed fee (from the staff member's profile).
- **Refund** of q of a line = round(net × q ÷ qty) and round(tax × q ÷ qty); the last refund of a
  line takes exactly what remains. Retail items go back to stock only when chosen, at the cost they
  left with. Product used in a service is never returned to stock by a refund. Commission already
  paid isn't reversed.
- **Void** is for a sale entered by mistake: it reverses the sale's stock movements and cancels its
  payments. A sale with refunds can't be voided.

## Reports and dashboards

Sales count on their sale date; refunds on their refund date; card fees with their sale.

| Measure | Definition |
|---|---|
| Service / retail sales | Net (after discounts, before tax) of finalized lines |
| Refunds | Net amount refunded in the period |
| **Net revenue** | Service + retail sales − refunds. Tips and tax are not revenue |
| Discounts | Line and sale discounts given (already deducted from sales) |
| Tips | Tip lines less tip refunds (passed to staff) |
| Tax collected | Tax on sales less tax refunded (owed to the state) |
| Materials used | Ledger cost of products used in services |
| Retail cost of goods | Ledger cost of retail items sold, less items restocked from refunds |
| Direct costs | Materials used + retail cost of goods + commissions + card fees |
| **Gross profit** | Net revenue − direct costs; margin = gross profit ÷ net revenue |
| Service contribution (allocated) | Gross profit − allocated labor − allocated overhead − other direct costs, using estimates frozen on each sale |
| **Period result (actual)** | Gross profit − recorded expenses |
| Inventory purchases | Cash spent on stock purchases (not opening stock) |
| Stock consumed | Ledger value of stock used or sold (equals materials used + retail cost of goods) |
| Waste and adjustments | Value lost to waste, count adjustments, supplier returns and revaluations |

- **No double counting:** the period result subtracts the expenses you actually recorded, not the
  allocated overhead. Service contribution subtracts allocated overhead and is for comparing services;
  the two are shown separately and never combined.
- Breakdowns by service, profile, staff, category and product, and the weekly trend, are summed from
  the same records as the totals and reconcile exactly (tested).
- Expenses, purchases and stock adjustments are business-wide: they're left out (with a note) while
  a staff, profile, service, category or product filter is active.
- Filter precedence: a widget's own filter replaces the dashboard filter for that field only (shown
  as a blue tag on the widget). Point-in-time widgets ignore the date range.
- Prior-period comparison uses the same number of days immediately before the selected range.

## Market evidence

Comparable observed prices for a service are selected by these rules, in order (each exclusion is
shown with its reason): excluded by you; older than the age limit (default 540 days); farther than the
radius (when both places have coordinates); different hair length or stylist level (when both are
specified); "starting at" prices (lower bounds; excluded unless you choose to count them); duplicates
(same business and price within 30 days; newest kept); outliers (with 5+ prices, outside 1.5 × the
interquartile range). A price range counts at its midpoint.

Statistics: count, businesses, min, quartiles (linear interpolation between order statistics),
median, 90th percentile, max, typical age, distance coverage.

| Evidence quality | Rule |
|---|---|
| None | No comparable prices: "Insufficient observed market prices" |
| Low | Fewer than 4 prices, fewer than 3 businesses, or typically over a year old |
| Medium | 4+ prices from 3+ businesses, typically within a year |
| High | 8+ prices from 5+ businesses, typically within 6 months |

The **modeled** inflation-adjusted median (shown separately and labelled) adjusts each comparable
price by the national CPI for haircuts and personal care services (BLS series CUUR0000SEGD02) from
its observation month to the latest month. It is never presented as an observed price.
