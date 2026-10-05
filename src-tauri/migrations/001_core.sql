-- 001: business settings, locations, work profiles (versioned), staff, audit log.
-- Conventions: decimals are TEXT (exact, parsed by rust_decimal); timestamps are UTC ISO-8601 TEXT;
-- local business dates are 'YYYY-MM-DD'; archived rows keep their history (archived_at).

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL CHECK (json_valid(value))
) STRICT;

CREATE TABLE locations (
  id            INTEGER PRIMARY KEY,
  name          TEXT NOT NULL,
  address_line  TEXT NOT NULL DEFAULT '',
  city          TEXT NOT NULL DEFAULT '',
  state         TEXT NOT NULL DEFAULT '',
  postal_code   TEXT NOT NULL DEFAULT '',
  latitude      TEXT,
  longitude     TEXT,
  geo_precision TEXT CHECK (geo_precision IN ('address','zcta','place','manual')),
  geo_source    TEXT,
  state_fips    TEXT,
  county_fips   TEXT,
  place_fips    TEXT,
  archived_at   TEXT,
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

CREATE TABLE work_profiles (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  archived_at TEXT,
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- Every edit of a profile appends a version; estimates use the latest, finalized sales keep
-- their own snapshot of the figures that were applied.
CREATE TABLE work_profile_versions (
  id          INTEGER PRIMARY KEY,
  profile_id  INTEGER NOT NULL REFERENCES work_profiles(id),
  version     INTEGER NOT NULL,
  kind        TEXT NOT NULL CHECK (kind IN ('individual','chair_renter','independent','employee')),
  location_id INTEGER REFERENCES locations(id),
  data        TEXT NOT NULL CHECK (json_valid(data)),
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE (profile_id, version)
) STRICT;
CREATE INDEX work_profile_versions_location ON work_profile_versions(location_id);

CREATE TABLE staff (
  id                 INTEGER PRIMARY KEY,
  name               TEXT NOT NULL,
  color              TEXT NOT NULL DEFAULT 'grape',
  default_profile_id INTEGER REFERENCES work_profiles(id),
  location_id        INTEGER REFERENCES locations(id),
  archived_at        TEXT,
  created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE INDEX staff_profile ON staff(default_profile_id);
CREATE INDEX staff_location ON staff(location_id);

CREATE TABLE audit_log (
  id        INTEGER PRIMARY KEY,
  at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  entity    TEXT NOT NULL,
  entity_id INTEGER,
  action    TEXT NOT NULL,
  summary   TEXT NOT NULL,
  detail    TEXT CHECK (detail IS NULL OR json_valid(detail))
) STRICT;
CREATE INDEX audit_log_entity ON audit_log(entity, entity_id);
CREATE INDEX audit_log_at ON audit_log(at);
