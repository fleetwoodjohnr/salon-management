-- 002: suppliers, products, purchases and lots, the stock ledger, services, variants and recipes.
-- Quantities are TEXT decimals in the product's base unit (g, mL or piece).

CREATE TABLE attachments (
  id          INTEGER PRIMARY KEY,
  sha256      TEXT NOT NULL,
  file_name   TEXT NOT NULL,
  stored_name TEXT NOT NULL,
  size        INTEGER NOT NULL,
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE INDEX attachments_sha ON attachments(sha256);

CREATE TABLE suppliers (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  contact     TEXT NOT NULL DEFAULT '',
  phone       TEXT NOT NULL DEFAULT '',
  email       TEXT NOT NULL DEFAULT '',
  website     TEXT NOT NULL DEFAULT '',
  notes       TEXT NOT NULL DEFAULT '',
  archived_at TEXT,
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

CREATE TABLE storage_locations (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  location_id INTEGER REFERENCES locations(id),
  archived_at TEXT,
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE INDEX storage_locations_location ON storage_locations(location_id);

CREATE TABLE products (
  id                 INTEGER PRIMARY KEY,
  name               TEXT NOT NULL,
  brand              TEXT NOT NULL DEFAULT '',
  category           TEXT NOT NULL CHECK (category IN ('professional','consumable','retail')),
  subcategory        TEXT NOT NULL DEFAULT '',
  sku                TEXT NOT NULL DEFAULT '',
  barcode            TEXT NOT NULL DEFAULT '',
  supplier_id        INTEGER REFERENCES suppliers(id),
  dimension          TEXT NOT NULL CHECK (dimension IN ('mass','volume','count')),
  stock_unit         TEXT NOT NULL,
  density_g_per_ml   TEXT,
  default_storage_id INTEGER REFERENCES storage_locations(id),
  reorder_point      TEXT,
  reorder_qty        TEXT,
  retail_price       TEXT,
  on_hand_qty        TEXT NOT NULL DEFAULT '0',
  on_hand_value      TEXT NOT NULL DEFAULT '0',
  last_unit_cost     TEXT,
  notes              TEXT NOT NULL DEFAULT '',
  archived_at        TEXT,
  created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE UNIQUE INDEX products_sku ON products(sku) WHERE sku <> '';
CREATE UNIQUE INDEX products_barcode ON products(barcode) WHERE barcode <> '';
CREATE INDEX products_name ON products(name COLLATE NOCASE);
CREATE INDEX products_supplier ON products(supplier_id);
CREATE INDEX products_storage ON products(default_storage_id);

CREATE TABLE product_units (
  id         INTEGER PRIMARY KEY,
  product_id INTEGER NOT NULL REFERENCES products(id),
  name       TEXT NOT NULL COLLATE NOCASE,
  qty        TEXT NOT NULL,
  unit       TEXT NOT NULL,
  UNIQUE (product_id, name)
) STRICT;

CREATE TABLE purchases (
  id                 INTEGER PRIMARY KEY,
  kind               TEXT NOT NULL DEFAULT 'purchase' CHECK (kind IN ('purchase','opening_balance')),
  supplier_id        INTEGER REFERENCES suppliers(id),
  purchase_date      TEXT NOT NULL,
  invoice_ref        TEXT NOT NULL DEFAULT '',
  subtotal           TEXT NOT NULL,
  discount           TEXT NOT NULL DEFAULT '0',
  shipping           TEXT NOT NULL DEFAULT '0',
  nonrecoverable_tax TEXT NOT NULL DEFAULT '0',
  total              TEXT NOT NULL,
  attachment_id      INTEGER REFERENCES attachments(id),
  notes              TEXT NOT NULL DEFAULT '',
  reversed_at        TEXT,
  reversal_note      TEXT,
  created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE INDEX purchases_date ON purchases(purchase_date);
CREATE INDEX purchases_supplier ON purchases(supplier_id);
CREATE INDEX purchases_attachment ON purchases(attachment_id);

-- Each purchase line is a lot: its landed cost, expiry and where it was put.
CREATE TABLE purchase_lines (
  id                   INTEGER PRIMARY KEY,
  purchase_id          INTEGER NOT NULL REFERENCES purchases(id),
  product_id           INTEGER NOT NULL REFERENCES products(id),
  package_count        TEXT NOT NULL,
  contents_per_package TEXT NOT NULL,
  unit                 TEXT NOT NULL,
  qty_base             TEXT NOT NULL,
  line_price           TEXT NOT NULL,
  landed_cost          TEXT NOT NULL,
  storage_id           INTEGER REFERENCES storage_locations(id),
  lot_code             TEXT NOT NULL DEFAULT '',
  expires_on           TEXT
) STRICT;
CREATE INDEX purchase_lines_purchase ON purchase_lines(purchase_id);
CREATE INDEX purchase_lines_product ON purchase_lines(product_id);
CREATE INDEX purchase_lines_storage ON purchase_lines(storage_id);
CREATE INDEX purchase_lines_expiry ON purchase_lines(expires_on) WHERE expires_on IS NOT NULL;

-- Append-only stock ledger. Corrections are new rows that point at the row they reverse.
-- qty_after/value_after are the product's running totals after the row (an audit trail).
CREATE TABLE stock_ledger (
  id          INTEGER PRIMARY KEY,
  product_id  INTEGER NOT NULL REFERENCES products(id),
  storage_id  INTEGER REFERENCES storage_locations(id),
  occurred_on TEXT NOT NULL,
  recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  kind        TEXT NOT NULL CHECK (kind IN ('purchase','opening','service_use','retail_sale','waste','adjustment',
                'transfer_out','transfer_in','supplier_return','customer_return','reversal','revaluation')),
  qty         TEXT NOT NULL,
  value       TEXT NOT NULL,
  qty_after   TEXT NOT NULL,
  value_after TEXT NOT NULL,
  source      TEXT,
  source_id   INTEGER,
  reversal_of INTEGER UNIQUE REFERENCES stock_ledger(id),
  note        TEXT NOT NULL DEFAULT ''
) STRICT;
CREATE INDEX stock_ledger_product ON stock_ledger(product_id, id);
CREATE INDEX stock_ledger_storage ON stock_ledger(storage_id);
CREATE INDEX stock_ledger_date ON stock_ledger(occurred_on);
-- One row per source document line and kind: completing a sale twice cannot deduct twice.
CREATE UNIQUE INDEX stock_ledger_source ON stock_ledger(source, source_id, kind) WHERE source IS NOT NULL;

CREATE TABLE services (
  id                 INTEGER PRIMARY KEY,
  name               TEXT NOT NULL,
  category           TEXT NOT NULL DEFAULT '',
  description        TEXT NOT NULL DEFAULT '',
  profile_id         INTEGER REFERENCES work_profiles(id),
  hands_on_min       INTEGER NOT NULL,
  processing_min     INTEGER NOT NULL DEFAULT 0,
  setup_min          INTEGER NOT NULL DEFAULT 0,
  cleanup_min        INTEGER NOT NULL DEFAULT 0,
  is_addon           INTEGER NOT NULL DEFAULT 0,
  waste_pct          TEXT NOT NULL DEFAULT '0',
  other_direct_cost  TEXT NOT NULL DEFAULT '0',
  other_direct_note  TEXT NOT NULL DEFAULT '',
  price              TEXT,
  price_note         TEXT NOT NULL DEFAULT '',
  price_set_at       TEXT,
  target_kind        TEXT CHECK (target_kind IN ('margin','markup')),
  target_pct         TEXT,
  position           TEXT CHECK (position IN ('budget','standard','premium','luxury')),
  rounding_increment TEXT,
  rounding_mode      TEXT CHECK (rounding_mode IN ('up','nearest','down')),
  tax_category       TEXT NOT NULL DEFAULT 'service',
  archived_at        TEXT,
  created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE INDEX services_profile ON services(profile_id);
CREATE INDEX services_category ON services(category);

-- Variants adjust a service by hair length, density, complexity or level. One choice per group.
CREATE TABLE service_variants (
  id               INTEGER PRIMARY KEY,
  service_id       INTEGER NOT NULL REFERENCES services(id),
  group_name       TEXT NOT NULL,
  name             TEXT NOT NULL,
  hands_on_delta   INTEGER NOT NULL DEFAULT 0,
  processing_delta INTEGER NOT NULL DEFAULT 0,
  material_factor  TEXT NOT NULL DEFAULT '1',
  price_delta      TEXT NOT NULL DEFAULT '0',
  sort             INTEGER NOT NULL DEFAULT 0,
  archived_at      TEXT
) STRICT;
CREATE INDEX service_variants_service ON service_variants(service_id);

CREATE TABLE recipe_versions (
  id         INTEGER PRIMARY KEY,
  service_id INTEGER NOT NULL REFERENCES services(id),
  version    INTEGER NOT NULL,
  note       TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE (service_id, version)
) STRICT;

CREATE TABLE recipe_lines (
  id                INTEGER PRIMARY KEY,
  recipe_version_id INTEGER NOT NULL REFERENCES recipe_versions(id),
  product_id        INTEGER NOT NULL REFERENCES products(id),
  qty               TEXT NOT NULL,
  unit              TEXT NOT NULL,
  note              TEXT NOT NULL DEFAULT '',
  sort              INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE INDEX recipe_lines_version ON recipe_lines(recipe_version_id);
CREATE INDEX recipe_lines_product ON recipe_lines(product_id);

CREATE TABLE bundles (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  price       TEXT,
  archived_at TEXT,
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

CREATE TABLE bundle_items (
  id         INTEGER PRIMARY KEY,
  bundle_id  INTEGER NOT NULL REFERENCES bundles(id),
  service_id INTEGER NOT NULL REFERENCES services(id),
  qty        INTEGER NOT NULL DEFAULT 1 CHECK (qty > 0)
) STRICT;
CREATE INDEX bundle_items_bundle ON bundle_items(bundle_id);
CREATE INDEX bundle_items_service ON bundle_items(service_id);
