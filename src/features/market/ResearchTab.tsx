import { Alert, Anchor, Badge, Button, Chip, Group, List, Paper, Progress, ScrollArea, SegmentedControl, Select, SimpleGrid, Slider, Stack, Switch, Table, Text, TextInput, Title, Tooltip } from "@mantine/core";
import { IconAlertTriangle, IconCheck, IconPlayerStop, IconSearch, IconWorldWww, IconX } from "@tabler/icons-react";
import { useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useRef, useState } from "react";
import { call, errorMessage } from "../../api/ipc";
import { useCmd } from "../../api/queries";
import type { Location, ResearchCenter, ResearchProgress, ResearchRun, ResearchSummary, SiteLine } from "../../api/types";
import { DistanceInput } from "../../components/DistanceInput";
import { fmtDistance, fromDisplay, toDisplay, useDistanceUnit, usePrefs } from "../../lib/distance";
import { isoToLocal } from "../../lib/format";
import { businessTypes, siteStatus } from "./labels";

/** Query keys whose data a finished search changes. */
export const MARKET_KEYS = ["competitors_list", "market_evidence", "market_overview", "observations_list", "research_runs", "providers_status", "dashboard_snapshot", "competitor_pages"];

function SiteResults({ sites }: { sites: SiteLine[] }) {
  if (!sites.length) return null;
  return (
    <ScrollArea.Autosize mah={360}>
      <Table fz="xs" verticalSpacing={6} stickyHeader>
        <Table.Thead>
          <Table.Tr>
            <Table.Th>Business</Table.Th>
            <Table.Th>Website</Table.Th>
            <Table.Th ta="right">Prices</Table.Th>
            <Table.Th>Details</Table.Th>
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {[...sites].reverse().map((s) => (
            <Table.Tr key={`${s.competitor_id}-${s.url}`}>
              <Table.Td>
                <Anchor size="xs" onClick={() => openUrl(s.url)}>
                  {s.name}
                </Anchor>
              </Table.Td>
              <Table.Td>
                <Tooltip label={siteStatus[s.status].help} multiline w={280}>
                  <Badge size="xs" color={siteStatus[s.status].color}>
                    {siteStatus[s.status].label}
                  </Badge>
                </Tooltip>
              </Table.Td>
              <Table.Td ta="right" className="srm-num">
                {s.prices}
              </Table.Td>
              <Table.Td>
                <Text size="xs" c="dimmed" lineClamp={2}>
                  {s.note}
                </Text>
              </Table.Td>
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
    </ScrollArea.Autosize>
  );
}

function Stat({ label, value, note }: { label: string; value: number | string; note?: string }) {
  return (
    <div>
      <Text size="xs" c="dimmed">
        {label}
      </Text>
      <Text fw={650} size="lg" className="srm-num" ta="left">
        {value}
      </Text>
      {note && (
        <Text size="xs" c="dimmed">
          {note}
        </Text>
      )}
    </div>
  );
}

/** What a search covered and what it couldn't, stated plainly. */
export function Coverage({ s }: { s: ResearchSummary }) {
  const unit = useDistanceUnit();
  const problems = Object.entries(s.site_statuses).filter(([k]) => k !== "prices_found");
  return (
    <Stack gap="md">
      <SimpleGrid cols={{ base: 2, sm: 3, lg: 6 }}>
        <Stat label="Found on the map" value={s.discovered} note={[s.added ? `${s.added} new` : "", s.merged ? `${s.merged} merged with ones you had` : "", s.unnamed ? `${s.unnamed} unnamed skipped` : ""].filter(Boolean).join(", ") || undefined} />
        <Stat label={`Within ${fmtDistance(s.radius_km, unit)}`} value={s.in_radius} note={s.no_location ? `+${s.no_location} you added without a location` : undefined} />
        <Stat label="With a website" value={s.with_website} />
        <Stat label="Websites checked" value={s.websites_checked} note={[s.skipped_recent ? `${s.skipped_recent} checked this week, skipped` : "", s.not_checked_limit ? `${s.not_checked_limit} left for the next search` : ""].filter(Boolean).join("; ") || undefined} />
        <Stat label="With usable prices" value={s.with_prices} />
        <Stat label="Prices saved" value={s.prices_saved} note={s.unmatched_prices ? `${s.unmatched_prices} for services you don't offer` : undefined} />
      </SimpleGrid>
      {problems.length > 0 && (
        <Group gap={6}>
          <Text size="xs" c="dimmed">
            No prices from:
          </Text>
          {problems.map(([k, n]) => (
            <Tooltip key={k} label={siteStatus[k as keyof typeof siteStatus]?.help} multiline w={280}>
              <Badge size="sm" variant="outline" color={siteStatus[k as keyof typeof siteStatus]?.color ?? "gray"}>
                {siteStatus[k as keyof typeof siteStatus]?.label ?? k}: {n}
              </Badge>
            </Tooltip>
          ))}
        </Group>
      )}
      <Stack gap={4}>
        {s.sources.map((src) => (
          <Group key={src.name} gap={6} wrap="nowrap" align="flex-start">
            {src.ok ? <IconCheck size={16} color="var(--mantine-color-teal-6)" /> : <IconX size={16} color="var(--mantine-color-red-6)" />}
            <Text size="xs">
              <b>{src.name}:</b> {src.detail}
            </Text>
          </Group>
        ))}
        {(s.unrecognized > 0 || s.consultation_only > 0) && (
          <Text size="xs" c="dimmed">
            {s.consultation_only > 0 && `${s.consultation_only} services listed as "price on consultation" (no amount). `}
            {s.unrecognized > 0 && `${s.unrecognized} amounts weren't recognisable salon services and were left out.`}
          </Text>
        )}
      </Stack>
    </Stack>
  );
}

/** Add a competitor by its web address (reads its name and address when allowed), then check it. */
export function AddWebsite({ onDone }: { onDone?: () => void }) {
  const qc = useQueryClient();
  const [url, setUrl] = useState("");
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<SiteLine | null>(null);
  const [err, setErr] = useState<string | null>(null);
  async function add() {
    setBusy(true);
    setErr(null);
    setResult(null);
    try {
      const r = await call<SiteLine>("competitor_add_website", { url, name: name.trim() || null });
      setResult(r);
      setUrl("");
      setName("");
      MARKET_KEYS.forEach((k) => qc.invalidateQueries({ queryKey: [k] }));
      onDone?.();
    } catch (e) {
      setErr(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <Stack gap="xs">
      <Group align="flex-end" gap="sm">
        <TextInput label="Competitor's website" placeholder="https://example-salon.com" value={url} onChange={(e) => setUrl(e.currentTarget.value)} style={{ flex: 2, minWidth: 220 }} />
        <TextInput label="Name (optional)" placeholder="Read from the site" value={name} onChange={(e) => setName(e.currentTarget.value)} style={{ flex: 1, minWidth: 160 }} />
        <Button leftSection={<IconWorldWww size={16} />} loading={busy} disabled={!url.trim()} onClick={add}>
          Add and check prices
        </Button>
      </Group>
      {busy && (
        <Text size="xs" c="dimmed">
          Reading the site politely (robots.txt first, a pause between pages). This can take up to a minute.
        </Text>
      )}
      {err && <Alert color="red">{err}</Alert>}
      {result && (
        <Alert color={siteStatus[result.status].color} icon={result.prices > 0 ? <IconCheck size={16} /> : <IconAlertTriangle size={16} />}>
          <Text size="sm">
            <b>{result.name}</b>: {siteStatus[result.status].label}
            {result.prices > 0 ? `, ${result.prices} prices saved.` : "."} {result.note || siteStatus[result.status].help}
          </Text>
        </Alert>
      )}
    </Stack>
  );
}

const STAGES = ["discovering", "selecting", "checking_websites", "done"] as const;

export function ResearchTab() {
  const qc = useQueryClient();
  const unit = useDistanceUnit();
  const prefs = usePrefs();
  const locations = useCmd<Location[]>("locations_list");
  const runs = useCmd<ResearchRun[]>("research_runs");
  const located = (locations.data ?? []).filter((l) => !l.archived && l.latitude && l.longitude);
  const [kind, setKind] = useState<ResearchCenter["kind"]>("location");
  const [locId, setLocId] = useState<string | null>(null);
  const [zip, setZip] = useState("");
  const [city, setCity] = useState("");
  const [st, setSt] = useState("");
  const [radiusKm, setRadiusKm] = useState<number | null>(null);
  const [types, setTypes] = useState<string[] | null>(null);
  const [checkSites, setCheckSites] = useState(true);
  const [refresh, setRefresh] = useState(false);
  const [progress, setProgress] = useState<ResearchProgress | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const wasRunning = useRef(false);

  const km = radiusKm ?? prefs.data?.research_radius_km ?? 8.04672;
  const chosenTypes = types ?? prefs.data?.research_types ?? ["hair", "barber", "nails", "beauty"];
  const locValue = locId ?? (located[0]?.id != null ? String(located[0].id) : null);
  useEffect(() => {
    if (!located.length && kind === "location" && locations.data) setKind("zip");
  }, [locations.data]); // eslint-disable-line react-hooks/exhaustive-deps

  // Follow a running search, including one started before this page was opened.
  useEffect(() => {
    let alive = true;
    call<ResearchProgress | null>("research_progress").then((p) => alive && setProgress(p));
    const un = listen<ResearchProgress>("research-progress", (e) => setProgress(e.payload));
    return () => {
      alive = false;
      un.then((f) => f());
    };
  }, []);
  useEffect(() => {
    if (wasRunning.current && progress && !progress.running) MARKET_KEYS.forEach((k) => qc.invalidateQueries({ queryKey: [k] }));
    wasRunning.current = !!progress?.running;
  }, [progress, qc]);

  const center: ResearchCenter | null =
    kind === "location" ? (locValue ? { kind: "location", id: Number(locValue) } : null) : kind === "zip" ? (/^\d{5}$/.test(zip.trim()) ? { kind: "zip", zip: zip.trim() } : null) : city.trim() && st.trim().length === 2 ? { kind: "city", city: city.trim(), state: st.trim() } : null;

  async function start() {
    if (!center) return;
    setErr(null);
    try {
      await call<number>("research_start", { params: { center, radius_km: km, types: chosenTypes, check_websites: checkSites, refresh } });
      qc.invalidateQueries({ queryKey: ["prefs_get"] });
    } catch (e) {
      setErr(errorMessage(e));
    }
  }

  const running = !!progress?.running;
  const stageIndex = progress ? Math.max(0, STAGES.indexOf(progress.stage as (typeof STAGES)[number])) : 0;
  const maxKm = 50;
  return (
    <Stack maw={1180}>
      <Paper p="lg">
        <Title order={4} mb={4}>
          Find competitors and their advertised prices
        </Title>
        <Text size="sm" c="dimmed" mb="md">
          Searches OpenStreetMap for salons near you, then reads each competitor's own website for the prices it publishes. Every price keeps its source page and the time it was read.
        </Text>
        <Stack gap="md">
          <Group align="flex-end" gap="md" wrap="wrap">
            <div>
              <Text size="sm" fw={500} mb={4}>
                Search around
              </Text>
              <SegmentedControl
                value={kind}
                onChange={(v) => setKind(v as ResearchCenter["kind"])}
                data={[
                  { value: "location", label: "My location", disabled: !located.length },
                  { value: "zip", label: "A ZIP code" },
                  { value: "city", label: "A city" },
                ]}
              />
            </div>
            {kind === "location" && (
              <Select
                label="Location"
                w={260}
                data={located.map((l) => ({ value: String(l.id), label: l.name || l.address_line || `Location ${l.id}` }))}
                value={locValue}
                onChange={setLocId}
                allowDeselect={false}
              />
            )}
            {kind === "zip" && <TextInput label="ZIP code" w={140} maxLength={5} placeholder="98501" value={zip} onChange={(e) => setZip(e.currentTarget.value)} />}
            {kind === "city" && (
              <>
                <TextInput label="City" w={200} value={city} onChange={(e) => setCity(e.currentTarget.value)} />
                <TextInput label="State" w={80} maxLength={2} placeholder="WA" value={st} onChange={(e) => setSt(e.currentTarget.value.toUpperCase())} />
              </>
            )}
            <DistanceInput label="Within" w={130} km={km} minKm={0.5} maxKm={maxKm} onChange={(v) => v != null && setRadiusKm(Math.min(maxKm, Math.max(0.5, v)))} />
          </Group>
          <Slider
            aria-label="Search radius"
            maw={520}
            min={toDisplay(0.5, unit)}
            max={toDisplay(maxKm, unit)}
            step={unit === "mi" ? 0.5 : 1}
            value={toDisplay(km, unit)}
            onChange={(v) => setRadiusKm(Math.min(maxKm, Math.max(0.5, fromDisplay(v, unit))))}
            label={(v) => `${v.toFixed(1)} ${unit}`}
            marks={(unit === "mi" ? [1, 5, 10, 20, 30] : [2, 10, 25, 50]).map((v) => ({ value: v, label: `${v} ${unit}` }))}
            mb="lg"
          />
          {!located.length && kind === "location" && (
            <Text size="xs" c="dimmed">
              None of your locations has been located yet (Settings → Locations → Locate). You can search around a ZIP code or city instead.
            </Text>
          )}
          <Chip.Group multiple value={chosenTypes} onChange={setTypes}>
            <Group gap="xs">
              {businessTypes.map((t) => (
                <Chip key={t.value} value={t.value} size="sm">
                  {t.label}
                </Chip>
              ))}
            </Group>
          </Chip.Group>
          <Group gap="xl">
            <Switch label="Read competitors' websites for prices" checked={checkSites} onChange={(e) => setCheckSites(e.currentTarget.checked)} />
            <Switch label="Re-check websites checked in the last week" checked={refresh} onChange={(e) => setRefresh(e.currentTarget.checked)} />
          </Group>
          <Group>
            {!running ? (
              <Button leftSection={<IconSearch size={16} />} onClick={start} disabled={!center || !chosenTypes.length}>
                Start research
              </Button>
            ) : (
              <Button color="red" variant="light" leftSection={<IconPlayerStop size={16} />} onClick={() => call("research_cancel")}>
                Stop
              </Button>
            )}
            {!center && kind !== "location" && (
              <Text size="xs" c="dimmed">
                {kind === "zip" ? "Enter a 5-digit ZIP code." : "Enter a city and two-letter state."}
              </Text>
            )}
          </Group>
          {err && <Alert color="red">{err}</Alert>}
          <Text size="xs" c="dimmed">
            Sends only the search area to the public OpenStreetMap servers. Websites are read like a careful visitor: robots.txt is obeyed, one page at a time with pauses, at most 8 pages per site, and
            anti-bot checks are never bypassed. Social networks, booking platforms and directories aren't read because their terms don't allow it. Map data © OpenStreetMap contributors (ODbL).
          </Text>
        </Stack>
      </Paper>

      {progress && (
        <Paper p="lg">
          <Group justify="space-between" mb="xs">
            <Title order={4}>{running ? "Searching…" : progress.stage === "done" ? "Last search" : progress.stage === "cancelled" ? "Search stopped" : "Search failed"}</Title>
            <Text size="xs" c="dimmed">
              {progress.summary.center_label} · {fmtDistance(progress.summary.radius_km, unit)}
            </Text>
          </Group>
          <Group gap="xs" mb="xs">
            {["Map search", "Choose sites", "Read websites", "Done"].map((l, i) => (
              <Badge key={l} variant={i <= stageIndex ? "filled" : "outline"} color={i < stageIndex || progress.stage === "done" ? "teal" : running && i === stageIndex ? "blue" : "gray"}>
                {l}
              </Badge>
            ))}
          </Group>
          {running && progress.stage === "checking_websites" && progress.sites_total > 0 && (
            <Progress value={(progress.sites_done / progress.sites_total) * 100} animated mb="xs" aria-label="Websites checked" />
          )}
          <Text size="sm" mb="xs">
            {progress.message}
            {running && progress.sites_total > 0 && ` (${progress.sites_done} of ${progress.sites_total})`}
          </Text>
          {running && progress.current.length > 0 && (
            <Text size="xs" c="dimmed" mb="xs">
              Reading: {progress.current.join(", ")}
            </Text>
          )}
          {progress.error && (
            <Alert color="red" mb="sm">
              {progress.error}
            </Alert>
          )}
          <Coverage s={progress.summary} />
          <Alert color="blue" variant="light" mt="md" title="What this does and doesn't cover">
            <List size="xs" spacing={2}>
              <List.Item>OpenStreetMap only lists businesses someone has mapped, so some salons, especially new or home-based ones, may be missing. Add them by website below.</List.Item>
              <List.Item>Search engines, booking platforms and review sites aren't searched: their terms don't allow automated access, and the free APIs that once existed now need paid accounts.</List.Item>
              <List.Item>Prices shown only in pictures, PDFs or booking widgets can't be read automatically. Those sites are listed so you can check them yourself.</List.Item>
            </List>
          </Alert>
        </Paper>
      )}

      {progress && progress.sites.length > 0 && (
        <Paper p="lg">
          <Title order={5} mb="sm">
            Websites checked {running ? "so far" : ""}
          </Title>
          <SiteResults sites={progress.sites} />
        </Paper>
      )}

      <Paper p="lg">
        <Title order={5} mb={4}>
          Add a competitor by website
        </Title>
        <Text size="xs" c="dimmed" mb="sm">
          For a business the map doesn't know, or one whose website the map lacks.
        </Text>
        <AddWebsite />
      </Paper>

      {(runs.data?.length ?? 0) > 0 && (
        <Paper p="lg">
          <Title order={5} mb="sm">
            Past searches
          </Title>
          <Table fz="xs">
            <Table.Thead>
              <Table.Tr>
                <Table.Th>When</Table.Th>
                <Table.Th>Around</Table.Th>
                <Table.Th ta="right">Businesses</Table.Th>
                <Table.Th ta="right">Sites checked</Table.Th>
                <Table.Th ta="right">With prices</Table.Th>
                <Table.Th>Result</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {runs.data!.map((r) => (
                <Table.Tr key={r.id}>
                  <Table.Td>{isoToLocal(r.started_at)}</Table.Td>
                  <Table.Td>
                    {r.center_label} · {fmtDistance(r.radius_km, unit)}
                  </Table.Td>
                  <Table.Td ta="right">{r.summary?.in_radius ?? "—"}</Table.Td>
                  <Table.Td ta="right">{r.summary?.websites_checked ?? "—"}</Table.Td>
                  <Table.Td ta="right">{r.summary?.with_prices ?? "—"}</Table.Td>
                  <Table.Td>
                    <Badge size="xs" color={r.status === "done" ? "teal" : r.status === "running" ? "blue" : r.status === "cancelled" ? "gray" : "red"}>
                      {r.status}
                    </Badge>{" "}
                    {r.error && (
                      <Text span size="xs" c="dimmed">
                        {r.error}
                      </Text>
                    )}
                  </Table.Td>
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
        </Paper>
      )}
    </Stack>
  );
}
