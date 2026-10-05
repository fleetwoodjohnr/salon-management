-- 004: online data providers. Responses are cached per workspace so lookups work offline afterwards
-- and quotas are respected. API keys are never stored here (they live in the OS keyring).

CREATE TABLE provider_cache (
  provider    TEXT NOT NULL,
  request_key TEXT NOT NULL,
  response    TEXT NOT NULL,
  fetched_at  TEXT NOT NULL,
  PRIMARY KEY (provider, request_key)
) STRICT;

CREATE TABLE provider_usage (
  provider TEXT NOT NULL,
  day      TEXT NOT NULL,
  requests INTEGER NOT NULL DEFAULT 0,
  errors   INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (provider, day)
) STRICT;

CREATE TABLE provider_events (
  id       INTEGER PRIMARY KEY,
  provider TEXT NOT NULL,
  at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ok       INTEGER NOT NULL,
  message  TEXT NOT NULL
) STRICT;
CREATE INDEX provider_events_provider ON provider_events(provider, id);
