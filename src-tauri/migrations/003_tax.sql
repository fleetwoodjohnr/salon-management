-- 003: sales tax. Three separate questions, stored separately:
--   1. jurisdiction  (where: the business location, plus the provider's jurisdiction label/code)
--   2. rate          (versioned rate sets per location, with source, precision and dates)
--   3. taxability    (versioned per location and line category: taxable / exempt / unknown)
-- Rates are percents stored as TEXT decimals. Nothing defaults to 0%: a missing rate or an
-- unknown taxability is "unresolved" and blocks finalizing a sale.

CREATE TABLE tax_categories (
  code       TEXT PRIMARY KEY,
  label      TEXT NOT NULL,
  applies_to TEXT NOT NULL CHECK (applies_to IN ('service','retail','tips','other')),
  builtin    INTEGER NOT NULL DEFAULT 0
) STRICT;
INSERT INTO tax_categories (code, label, applies_to, builtin) VALUES
  ('service', 'Salon services', 'service', 1),
  ('retail', 'Retail products', 'retail', 1),
  ('tips', 'Tips and gratuities', 'tips', 1);

CREATE TABLE tax_rate_sets (
  id                 INTEGER PRIMARY KEY,
  location_id        INTEGER NOT NULL REFERENCES locations(id),
  version            INTEGER NOT NULL,
  state_rate         TEXT,
  county_rate        TEXT,
  city_rate          TEXT,
  district_rate      TEXT,
  total_rate         TEXT NOT NULL,
  jurisdiction_label TEXT NOT NULL DEFAULT '',
  jurisdiction_code  TEXT,
  source             TEXT NOT NULL CHECK (source IN ('manual','wa_dor','ca_cdtfa')),
  source_url         TEXT,
  precision          TEXT NOT NULL CHECK (precision IN ('address','zip9','zip5','city','county','state','unknown')),
  status             TEXT NOT NULL CHECK (status IN ('verified','estimate','manual')),
  effective_date     TEXT,
  dataset_period     TEXT,
  retrieved_at       TEXT,
  note               TEXT NOT NULL DEFAULT '',
  created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  superseded_at      TEXT,
  UNIQUE (location_id, version)
) STRICT;
CREATE INDEX tax_rate_sets_current ON tax_rate_sets(location_id) WHERE superseded_at IS NULL;

CREATE TABLE taxability_rules (
  id            INTEGER PRIMARY KEY,
  location_id   INTEGER NOT NULL REFERENCES locations(id),
  category_code TEXT NOT NULL REFERENCES tax_categories(code),
  status        TEXT NOT NULL CHECK (status IN ('taxable','exempt','unknown')),
  basis         TEXT NOT NULL DEFAULT '',
  decided_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  superseded_at TEXT
) STRICT;
CREATE INDEX taxability_rules_current ON taxability_rules(location_id, category_code) WHERE superseded_at IS NULL;
CREATE INDEX taxability_rules_category ON taxability_rules(category_code);
