# Database

One SQLite database per workspace (`workspaces/<id>/salon.db`). Tables are `STRICT`, foreign keys
are enforced, writes are transactional, and the journal is WAL with `synchronous=FULL`.

## Conventions

- Money, rates and quantities: `TEXT` holding exact decimals (parsed with `rust_decimal`). Sums are
  done in Rust, never with SQL `SUM` over text.
- Product quantities are stored in the product's base unit (grams, millilitres or pieces).
- Timestamps: UTC ISO-8601 text. Business dates: local `YYYY-MM-DD`. Appointment times: local
  `YYYY-MM-DDTHH:MM`.
- Rows that history depends on are archived (`archived_at`), not deleted.
- `PRAGMA user_version` holds the schema version (currently 6).

## Migrations

| # | File | Adds |
|---|---|---|
| 1 | `001_core.sql` | settings, locations, work profiles + versions, staff, audit log |
| 2 | `002_inventory_services.sql` | attachments, suppliers, storage spots, products, custom units, purchases + lines (lots), stock ledger, services, variants, recipe versions + lines, bundles |
| 3 | `003_tax.sql` | tax categories, versioned rate sets, versioned taxability rules |
| 4 | `004_providers.sql` | provider response cache, daily usage, event log |
| 5 | `005_sales.sql` | clients, formulas + versions, estimates, appointments, sales, sale lines, product usage, refunds, payments, expenses |
| 6 | `006_market_dashboards.sql` | competitors, observed prices, saved dashboards |

Each migration runs in its own transaction after an automatic backup; a foreign-key check runs
afterwards. Tests apply every migration from an empty database and upgrade from each earlier version.

## Tables

**Setup**
- `settings(key, value JSON)` — business settings (name, time zone, units, schedule, tax-inclusive
  pricing, onboarding state).
- `locations` — address and optional geocode (coordinates, precision, source, FIPS codes).
- `work_profiles(id, name, archived_at)` and `work_profile_versions(profile_id, version, kind,
  location_id, data JSON)` — every save adds a version.
- `staff` — name, calendar color, default work profile, location.
- `audit_log(at, entity, entity_id, action, summary, detail JSON)` — financial, inventory and
  configuration changes.

**Inventory**
- `suppliers`, `storage_locations`, `attachments(sha256, file_name, stored_name, size)`.
- `products` — identity, category (professional / consumable / retail), dimension and stock unit,
  density, reorder settings, retail price, and the running `on_hand_qty`, `on_hand_value`,
  `last_unit_cost`.
- `product_units` — custom units per product.
- `purchases` (invoice, discount, shipping, non-recoverable tax, total, attachment, reversal) and
  `purchase_lines` (packages × contents, landed cost, storage, lot, expiry).
- `stock_ledger` — append-only: kind (`purchase`, `opening`, `service_use`, `retail_sale`, `waste`,
  `adjustment`, `transfer_out`/`transfer_in`, `supplier_return`, `customer_return`, `reversal`,
  `revaluation`), signed qty and value, running totals after the row, source document, `reversal_of`.
  A unique index on `(source, source_id, kind)` makes duplicate deductions impossible.

**Services**
- `services` — times, waste %, other direct cost, price and override note, pricing overrides, tax
  category.
- `service_variants` — group/name, time deltas, material factor, price delta (archived, not deleted).
- `recipe_versions` + `recipe_lines` — a new version whenever the recipe changes.
- `bundles` + `bundle_items`.

**Sales tax**
- `tax_categories` — service, retail, tips, plus your own.
- `tax_rate_sets` — per location and version: component and total rates, jurisdiction, source,
  precision, status (verified / estimate / manual), effective date, data period, retrieval time,
  `superseded_at`.
- `taxability_rules` — per location and category: taxable / exempt / unknown, with the basis you
  recorded, versioned by `superseded_at`.

**Clients, bookings, sales**
- `clients`, `client_formulas`, `client_formula_versions`.
- `estimates` + `estimate_lines`.
- `appointments` + `appointment_lines` — status, local start/end.
- `sales` — status (draft / finalized / voided), sale number, discount, and snapshots written at
  finalize (subtotal, discounts, tax, tips, total, rate set and rate, label, rate status).
- `sale_lines` — kind (service / retail / tip), price, discounts, tax category, planned/actual minutes,
  and finalize-time snapshots (net, tax, taxability + basis, profile version, recipe version, materials,
  planned materials, labor, overhead, other direct, commission, retail cost).
- `sale_usage` — planned and actual product amounts per service line, with the ledger row and cost.
- `payments` — method, amount, card fee, refund link, void flag. No card data is stored.
- `refunds` + `refund_lines` — amounts, tax returned, restock flag and cost, ledger row.
- `expenses` — date, category, vendor, amount, receipt, void flag.

**Market and dashboards**
- `competitors` — source (manual / OpenStreetMap / CSV) and OSM id for de-duplication.
- `market_observations` — business, matched service, price type and range, attributes (length,
  level, duration, inclusions), source URL, date, excluded flag.
- `dashboards` — saved views: layout and filters as JSON.

**Providers**
- `provider_cache`, `provider_usage`, `provider_events` — responses, daily counts, recent outcomes.
  No secrets.

## Integrity checks you can run

- Stock: for every product, Σ ledger qty = `on_hand_qty` and Σ ledger value = `on_hand_value` (tested
  on two years of generated data).
- Reports: totals = Σ groupings = Σ weekly trend; stock consumed = materials used + retail cost of
  goods (tested).
- Backups: `PRAGMA integrity_check` and `PRAGMA foreign_key_check` run before any restore.
