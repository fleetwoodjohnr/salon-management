-- 005: clients and formulas, appointments, estimates, sales (with usage, payments, refunds), expenses.
-- Finalized sales snapshot every figure they were calculated with (prices, discounts, tax rate and
-- taxability, material cost from the ledger, labor/overhead/commission from the profile version).

CREATE TABLE clients (
  id          INTEGER PRIMARY KEY,
  first_name  TEXT NOT NULL,
  last_name   TEXT NOT NULL DEFAULT '',
  phone       TEXT NOT NULL DEFAULT '',
  email       TEXT NOT NULL DEFAULT '',
  sensitivities TEXT NOT NULL DEFAULT '',
  notes       TEXT NOT NULL DEFAULT '',
  archived_at TEXT,
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE INDEX clients_name ON clients(last_name COLLATE NOCASE, first_name COLLATE NOCASE);

CREATE TABLE client_formulas (
  id          INTEGER PRIMARY KEY,
  client_id   INTEGER NOT NULL REFERENCES clients(id),
  title       TEXT NOT NULL,
  service_id  INTEGER REFERENCES services(id),
  archived_at TEXT,
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE INDEX client_formulas_client ON client_formulas(client_id);
CREATE INDEX client_formulas_service ON client_formulas(service_id);

CREATE TABLE client_formula_versions (
  id         INTEGER PRIMARY KEY,
  formula_id INTEGER NOT NULL REFERENCES client_formulas(id),
  version    INTEGER NOT NULL,
  body       TEXT NOT NULL,
  lines      TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(lines)),
  note       TEXT NOT NULL DEFAULT '',
  sale_id    INTEGER,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE (formula_id, version)
) STRICT;

CREATE TABLE estimates (
  id          INTEGER PRIMARY KEY,
  client_id   INTEGER REFERENCES clients(id),
  staff_id    INTEGER REFERENCES staff(id),
  location_id INTEGER REFERENCES locations(id),
  issued_on   TEXT NOT NULL,
  valid_until TEXT,
  status      TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','accepted','declined','converted')),
  notes       TEXT NOT NULL DEFAULT '',
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE INDEX estimates_client ON estimates(client_id);
CREATE INDEX estimates_staff ON estimates(staff_id);
CREATE INDEX estimates_location ON estimates(location_id);

CREATE TABLE estimate_lines (
  id          INTEGER PRIMARY KEY,
  estimate_id INTEGER NOT NULL REFERENCES estimates(id),
  service_id  INTEGER NOT NULL REFERENCES services(id),
  variant_ids TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(variant_ids)),
  qty         INTEGER NOT NULL DEFAULT 1 CHECK (qty > 0),
  unit_price  TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  sort        INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE INDEX estimate_lines_estimate ON estimate_lines(estimate_id);
CREATE INDEX estimate_lines_service ON estimate_lines(service_id);

CREATE TABLE appointments (
  id          INTEGER PRIMARY KEY,
  client_id   INTEGER REFERENCES clients(id),
  staff_id    INTEGER NOT NULL REFERENCES staff(id),
  location_id INTEGER REFERENCES locations(id),
  starts_at   TEXT NOT NULL,
  ends_at     TEXT NOT NULL,
  status      TEXT NOT NULL DEFAULT 'scheduled' CHECK (status IN ('scheduled','checked_in','completed','cancelled','no_show')),
  notes       TEXT NOT NULL DEFAULT '',
  estimate_id INTEGER REFERENCES estimates(id),
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  CHECK (ends_at > starts_at)
) STRICT;
CREATE INDEX appointments_time ON appointments(starts_at);
CREATE INDEX appointments_staff ON appointments(staff_id, starts_at);
CREATE INDEX appointments_client ON appointments(client_id);
CREATE INDEX appointments_location ON appointments(location_id);
CREATE INDEX appointments_estimate ON appointments(estimate_id);

CREATE TABLE appointment_lines (
  id             INTEGER PRIMARY KEY,
  appointment_id INTEGER NOT NULL REFERENCES appointments(id),
  service_id     INTEGER NOT NULL REFERENCES services(id),
  variant_ids    TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(variant_ids)),
  qty            INTEGER NOT NULL DEFAULT 1 CHECK (qty > 0),
  sort           INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE INDEX appointment_lines_appointment ON appointment_lines(appointment_id);
CREATE INDEX appointment_lines_service ON appointment_lines(service_id);

CREATE TABLE sales (
  id                 INTEGER PRIMARY KEY,
  number             TEXT UNIQUE,
  status             TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','finalized','voided')),
  client_id          INTEGER REFERENCES clients(id),
  staff_id           INTEGER REFERENCES staff(id),
  location_id        INTEGER REFERENCES locations(id),
  appointment_id     INTEGER UNIQUE REFERENCES appointments(id),
  sale_date          TEXT NOT NULL,
  prices_include_tax INTEGER NOT NULL DEFAULT 0,
  sale_discount      TEXT NOT NULL DEFAULT '0',
  notes              TEXT NOT NULL DEFAULT '',
  -- snapshots written at finalize
  subtotal           TEXT,
  discount_total     TEXT,
  tax_total          TEXT,
  tip_total          TEXT,
  total              TEXT,
  tax_rate_set_id    INTEGER REFERENCES tax_rate_sets(id),
  tax_rate           TEXT,
  tax_label          TEXT,
  tax_rate_status    TEXT,
  created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  finalized_at       TEXT,
  voided_at          TEXT,
  void_reason        TEXT
) STRICT;
CREATE INDEX sales_date ON sales(sale_date);
CREATE INDEX sales_status ON sales(status);
CREATE INDEX sales_client ON sales(client_id);
CREATE INDEX sales_staff ON sales(staff_id);
CREATE INDEX sales_location ON sales(location_id);
CREATE INDEX sales_rate ON sales(tax_rate_set_id);

CREATE TABLE sale_lines (
  id                   INTEGER PRIMARY KEY,
  sale_id              INTEGER NOT NULL REFERENCES sales(id),
  kind                 TEXT NOT NULL CHECK (kind IN ('service','retail','tip')),
  service_id           INTEGER REFERENCES services(id),
  product_id           INTEGER REFERENCES products(id),
  staff_id             INTEGER REFERENCES staff(id),
  description          TEXT NOT NULL,
  variant_ids          TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(variant_ids)),
  qty                  TEXT NOT NULL,
  unit_price           TEXT NOT NULL,
  line_discount        TEXT NOT NULL DEFAULT '0',
  tax_category         TEXT NOT NULL,
  planned_minutes      INTEGER,
  actual_minutes       INTEGER,
  sort                 INTEGER NOT NULL DEFAULT 0,
  -- snapshots written at finalize
  gross                TEXT,
  sale_discount_share  TEXT,
  net                  TEXT,
  taxability           TEXT,
  taxability_basis     TEXT,
  tax                  TEXT,
  profile_version_id   INTEGER REFERENCES work_profile_versions(id),
  recipe_version_id    INTEGER REFERENCES recipe_versions(id),
  materials_cost       TEXT,
  planned_materials_cost TEXT,
  labor_cost           TEXT,
  overhead_cost        TEXT,
  other_direct_cost    TEXT,
  commission           TEXT,
  retail_cost          TEXT
) STRICT;
CREATE INDEX sale_lines_sale ON sale_lines(sale_id);
CREATE INDEX sale_lines_service ON sale_lines(service_id);
CREATE INDEX sale_lines_product ON sale_lines(product_id);
CREATE INDEX sale_lines_staff ON sale_lines(staff_id);
CREATE INDEX sale_lines_profile ON sale_lines(profile_version_id);
CREATE INDEX sale_lines_recipe ON sale_lines(recipe_version_id);

-- Products used by a service line: planned (from the recipe) and actual (what was really used).
CREATE TABLE sale_usage (
  id            INTEGER PRIMARY KEY,
  sale_line_id  INTEGER NOT NULL REFERENCES sale_lines(id),
  product_id    INTEGER NOT NULL REFERENCES products(id),
  planned_qty   TEXT NOT NULL DEFAULT '0',
  qty           TEXT NOT NULL,
  unit          TEXT NOT NULL,
  qty_base      TEXT,
  cost          TEXT,
  ledger_id     INTEGER REFERENCES stock_ledger(id)
) STRICT;
CREATE INDEX sale_usage_line ON sale_usage(sale_line_id);
CREATE INDEX sale_usage_product ON sale_usage(product_id);
CREATE INDEX sale_usage_ledger ON sale_usage(ledger_id);

CREATE TABLE refunds (
  id           INTEGER PRIMARY KEY,
  sale_id      INTEGER NOT NULL REFERENCES sales(id),
  number       TEXT NOT NULL UNIQUE,
  refund_date  TEXT NOT NULL,
  reason       TEXT NOT NULL,
  net_total    TEXT NOT NULL,
  tax_total    TEXT NOT NULL,
  tip_total    TEXT NOT NULL,
  total        TEXT NOT NULL,
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE INDEX refunds_sale ON refunds(sale_id);
CREATE INDEX refunds_date ON refunds(refund_date);

CREATE TABLE refund_lines (
  id           INTEGER PRIMARY KEY,
  refund_id    INTEGER NOT NULL REFERENCES refunds(id),
  sale_line_id INTEGER NOT NULL REFERENCES sale_lines(id),
  qty          TEXT NOT NULL,
  net          TEXT NOT NULL,
  tax          TEXT NOT NULL,
  restock      INTEGER NOT NULL DEFAULT 0,
  restock_cost TEXT,
  ledger_id    INTEGER REFERENCES stock_ledger(id)
) STRICT;
CREATE INDEX refund_lines_refund ON refund_lines(refund_id);
CREATE INDEX refund_lines_line ON refund_lines(sale_line_id);
CREATE INDEX refund_lines_ledger ON refund_lines(ledger_id);

-- Payments are recorded, not processed. Refund payments are negative and point at the refund.
CREATE TABLE payments (
  id          INTEGER PRIMARY KEY,
  sale_id     INTEGER NOT NULL REFERENCES sales(id),
  refund_id   INTEGER REFERENCES refunds(id),
  method      TEXT NOT NULL CHECK (method IN ('cash','card','check','transfer','other')),
  amount      TEXT NOT NULL,
  fee         TEXT NOT NULL DEFAULT '0',
  reference   TEXT NOT NULL DEFAULT '',
  paid_on     TEXT NOT NULL,
  voided_at   TEXT,
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE INDEX payments_sale ON payments(sale_id);
CREATE INDEX payments_refund ON payments(refund_id);
CREATE INDEX payments_date ON payments(paid_on);

CREATE TABLE expenses (
  id             INTEGER PRIMARY KEY,
  expense_date   TEXT NOT NULL,
  category       TEXT NOT NULL,
  vendor         TEXT NOT NULL DEFAULT '',
  description    TEXT NOT NULL DEFAULT '',
  amount         TEXT NOT NULL,
  payment_method TEXT NOT NULL DEFAULT '',
  location_id    INTEGER REFERENCES locations(id),
  attachment_id  INTEGER REFERENCES attachments(id),
  voided_at      TEXT,
  void_reason    TEXT,
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE INDEX expenses_date ON expenses(expense_date);
CREATE INDEX expenses_category ON expenses(category);
CREATE INDEX expenses_location ON expenses(location_id);
CREATE INDEX expenses_attachment ON expenses(attachment_id);
