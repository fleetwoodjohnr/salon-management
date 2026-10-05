-- 006: competitors and observed market prices; saved dashboards.
-- Observed prices are evidence collected by the user (typed in or imported) with source and date.
-- Census/BLS context is cached in provider_cache and is never stored as a price.

CREATE TABLE competitors (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  address     TEXT NOT NULL DEFAULT '',
  city        TEXT NOT NULL DEFAULT '',
  state       TEXT NOT NULL DEFAULT '',
  postal_code TEXT NOT NULL DEFAULT '',
  latitude    TEXT,
  longitude   TEXT,
  website     TEXT NOT NULL DEFAULT '',
  phone       TEXT NOT NULL DEFAULT '',
  source      TEXT NOT NULL CHECK (source IN ('manual','osm','csv')),
  source_ref  TEXT,
  attribution TEXT NOT NULL DEFAULT '',
  notes       TEXT NOT NULL DEFAULT '',
  archived_at TEXT,
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE UNIQUE INDEX competitors_source_ref ON competitors(source, source_ref) WHERE source_ref IS NOT NULL;
CREATE INDEX competitors_name ON competitors(name COLLATE NOCASE);

CREATE TABLE market_observations (
  id            INTEGER PRIMARY KEY,
  competitor_id INTEGER NOT NULL REFERENCES competitors(id),
  service_id    INTEGER REFERENCES services(id),
  service_label TEXT NOT NULL,
  price         TEXT NOT NULL,
  price_type    TEXT NOT NULL CHECK (price_type IN ('exact','starting_at','range')),
  price_max     TEXT,
  duration_min  INTEGER,
  hair_length   TEXT NOT NULL DEFAULT '',
  stylist_level TEXT NOT NULL DEFAULT '',
  inclusions    TEXT NOT NULL DEFAULT '',
  source_type   TEXT NOT NULL CHECK (source_type IN ('user_observed','csv_import')),
  source_url    TEXT NOT NULL DEFAULT '',
  observed_on   TEXT NOT NULL,
  notes         TEXT NOT NULL DEFAULT '',
  excluded      INTEGER NOT NULL DEFAULT 0,
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE INDEX market_observations_competitor ON market_observations(competitor_id);
CREATE INDEX market_observations_service ON market_observations(service_id);
CREATE INDEX market_observations_date ON market_observations(observed_on);

CREATE TABLE dashboards (
  id         INTEGER PRIMARY KEY,
  name       TEXT NOT NULL,
  layout     TEXT NOT NULL CHECK (json_valid(layout)),
  filters    TEXT NOT NULL CHECK (json_valid(filters)),
  is_default INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
