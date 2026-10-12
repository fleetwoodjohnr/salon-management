-- 007: automatic market research. Prices read from competitors' own websites are stored as
-- observations with source_type 'web_extracted', the exact snippet they were read from, the page
-- URL and the time retrieved. Research runs and per-page results record what was checked and why
-- a page gave no prices, so coverage can be reported honestly.

-- Rebuild market_observations to allow the new source type (CHECK constraints can't be altered).
-- Nothing references this table, so copy → drop → rename is safe.
CREATE TABLE market_observations_new (
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
  source_type   TEXT NOT NULL CHECK (source_type IN ('user_observed','csv_import','web_extracted')),
  source_url    TEXT NOT NULL DEFAULT '',
  observed_on   TEXT NOT NULL,
  notes         TEXT NOT NULL DEFAULT '',
  excluded      INTEGER NOT NULL DEFAULT 0,
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  -- UTC timestamp the page was read (web_extracted only)
  retrieved_at  TEXT,
  -- service category and normalised service name (research::catalog)
  category      TEXT NOT NULL DEFAULT '',
  canonical     TEXT NOT NULL DEFAULT '',
  description   TEXT NOT NULL DEFAULT '',
  -- the text the price was read from, for checking against the source
  raw_text      TEXT NOT NULL DEFAULT '',
  -- how service_id was set: by the user, matched automatically, or not matched
  match_method  TEXT NOT NULL DEFAULT 'user' CHECK (match_method IN ('user','auto','none'))
) STRICT;

INSERT INTO market_observations_new (id, competitor_id, service_id, service_label, price, price_type, price_max, duration_min, hair_length,
  stylist_level, inclusions, source_type, source_url, observed_on, notes, excluded, created_at, match_method)
SELECT id, competitor_id, service_id, service_label, price, price_type, price_max, duration_min, hair_length,
  stylist_level, inclusions, source_type, source_url, observed_on, notes, excluded, created_at,
  CASE WHEN service_id IS NULL THEN 'none' ELSE 'user' END
FROM market_observations;

DROP TABLE market_observations;
ALTER TABLE market_observations_new RENAME TO market_observations;
CREATE INDEX market_observations_competitor ON market_observations(competitor_id);
CREATE INDEX market_observations_service ON market_observations(service_id);
CREATE INDEX market_observations_date ON market_observations(observed_on);
CREATE INDEX market_observations_canonical ON market_observations(canonical);

CREATE TABLE research_runs (
  id           INTEGER PRIMARY KEY,
  started_at   TEXT NOT NULL,
  finished_at  TEXT,
  status       TEXT NOT NULL CHECK (status IN ('running','done','cancelled','failed')),
  center_label TEXT NOT NULL,
  center_lat   REAL NOT NULL,
  center_lon   REAL NOT NULL,
  radius_km    REAL NOT NULL,
  types        TEXT NOT NULL CHECK (json_valid(types)),
  summary      TEXT CHECK (summary IS NULL OR json_valid(summary)),
  error        TEXT
) STRICT;

-- Latest check of each competitor's website, one row per page looked at.
CREATE TABLE research_pages (
  id            INTEGER PRIMARY KEY,
  competitor_id INTEGER NOT NULL REFERENCES competitors(id),
  run_id        INTEGER REFERENCES research_runs(id),
  url           TEXT NOT NULL,
  status        TEXT NOT NULL CHECK (status IN ('prices_found','no_prices','robots_disallowed','terms_restricted','blocked',
                  'rate_limited','unreachable','http_error','not_html')),
  http_status   INTEGER,
  prices_found  INTEGER NOT NULL DEFAULT 0,
  -- JSON list of services listed as "price on consultation"
  consultation  TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(consultation)),
  note          TEXT NOT NULL DEFAULT '',
  checked_at    TEXT NOT NULL
) STRICT;
CREATE INDEX research_pages_competitor ON research_pages(competitor_id);
