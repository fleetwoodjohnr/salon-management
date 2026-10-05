import { Alert, Anchor, Badge, Button, Grid, Group, Modal, NavLink, NumberInput, Paper, ScrollArea, Select, SimpleGrid, Stack, Table, Tabs, Text, TextInput, Textarea, Title } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconFileImport, IconMapSearch, IconPlus } from "@tabler/icons-react";
import { useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { DataTable } from "mantine-datatable";
import { useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { call, errorMessage, isAppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { Competitor, CompetitorInput, CsvTable, LocalContext, ObsImportRow, ObservationInput, ObservationMapping, ProfileView, ServiceSummary } from "../../api/types";
import { DecimalInput } from "../../components/DecimalInput";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { useToday } from "../../lib/dates";
import { money } from "../../lib/format";
import { MarketEvidence } from "./MarketEvidence";

function ObservationModal({ value, onClose }: { value: ObservationInput | null; onClose: () => void }) {
  const [o, setO] = useState<ObservationInput | null>(value);
  const [err, setErr] = useState<{ field: string | null; message: string } | null>(null);
  const comps = useCmd<Competitor[]>("competitors_list", { includeArchived: false });
  const services = useCmd<ServiceSummary[]>("services_list", { includeArchived: false });
  const save = useAction<{ observation: ObservationInput }, number>("observation_save", { invalidate: ["market_evidence", "observations_list", "competitors_list", "dashboard_snapshot"], success: "Price recorded", silentError: true });
  if (!o) return null;
  const f = (k: string) => (err?.field === k ? err.message : undefined);
  return (
    <Modal opened onClose={onClose} title="Record an observed price" size="lg">
      <Stack>
        <Text size="xs" c="dimmed">
          Record a price you saw on a business's own menu, website or price list, with where and when. Don't copy data from sites whose terms forbid it.
        </Text>
        <SimpleGrid cols={2}>
          <Select
            label="Business"
            searchable
            data={(comps.data ?? []).map((c) => ({ value: String(c.input.id), label: c.input.name }))}
            value={o.competitor_id ? String(o.competitor_id) : null}
            onChange={(v) => setO({ ...o, competitor_id: Number(v) })}
            nothingFoundMessage="Add the business on the Competitors tab first"
          />
          <Select label="Matches my service" clearable data={(services.data ?? []).map((s) => ({ value: String(s.id), label: s.name }))} value={o.service_id != null ? String(o.service_id) : null} onChange={(v) => setO({ ...o, service_id: v ? Number(v) : null })} />
          <TextInput label="Service as they list it" value={o.service_label} onChange={(e) => setO({ ...o, service_label: e.currentTarget.value })} error={f("service_label")} />
          <Select
            label="Price type"
            data={[
              { value: "exact", label: "Fixed price" },
              { value: "starting_at", label: '"Starting at" / from' },
              { value: "range", label: "Price range" },
            ]}
            value={o.price_type}
            allowDeselect={false}
            onChange={(v) => setO({ ...o, price_type: (v as ObservationInput["price_type"]) ?? "exact" })}
          />
          <DecimalInput label={o.price_type === "range" ? "Lowest price" : "Price"} unit="$" value={o.price} onChange={(v) => setO({ ...o, price: v })} error={f("price")} />
          {o.price_type === "range" && <DecimalInput label="Highest price" unit="$" value={o.price_max ?? ""} onChange={(v) => setO({ ...o, price_max: v || null })} error={f("price_max")} />}
          <TextInput type="date" label="Observed on" value={o.observed_on} onChange={(e) => setO({ ...o, observed_on: e.currentTarget.value })} error={f("observed_on")} />
          <TextInput label="Source (web page)" placeholder="https://…" value={o.source_url} onChange={(e) => setO({ ...o, source_url: e.currentTarget.value })} />
          <Select
            label="Hair length"
            data={[
              { value: "", label: "Not specified" },
              { value: "short", label: "Short" },
              { value: "medium", label: "Medium" },
              { value: "long", label: "Long" },
            ]}
            value={o.hair_length}
            onChange={(v) => setO({ ...o, hair_length: (v as ObservationInput["hair_length"]) ?? "" })}
          />
          <TextInput label="Stylist level" placeholder="e.g. Senior, Master" value={o.stylist_level} onChange={(e) => setO({ ...o, stylist_level: e.currentTarget.value })} />
          <NumberInput label="Duration (min)" min={0} allowDecimal={false} value={o.duration_min ?? ""} onChange={(v) => setO({ ...o, duration_min: v === "" ? null : Number(v) })} />
          <TextInput label="What's included" placeholder="e.g. includes blow-dry" value={o.inclusions} onChange={(e) => setO({ ...o, inclusions: e.currentTarget.value })} />
        </SimpleGrid>
        <Textarea label="Notes" value={o.notes} onChange={(e) => setO({ ...o, notes: e.currentTarget.value })} />
        {err && !err.field && <Alert color="red">{err.message}</Alert>}
        <Group justify="flex-end">
          <Button loading={save.isPending} disabled={!o.competitor_id} onClick={() => save.mutate({ observation: o }, { onSuccess: onClose, onError: (e) => setErr(isAppError(e) ? e : { field: null, message: String(e) }) })}>
            Save price
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}

function PricesTab() {
  const [params, setParams] = useSearchParams();
  const today = useToday();
  const services = useCmd<ServiceSummary[]>("services_list", { includeArchived: false });
  const profiles = useCmd<ProfileView[]>("profiles_list", { includeArchived: false });
  const sid = Number(params.get("service")) || services.data?.[0]?.id || 0;
  const svc = services.data?.find((s) => s.id === sid);
  const [obs, setObs] = useState<ObservationInput | null>(null);
  const position = profiles.data?.[0]?.data.position ?? "standard";
  return (
    <Grid gap="lg">
      <Grid.Col span={{ base: 12, lg: 3 }}>
        <Paper p="xs">
          <ScrollArea.Autosize mah="65vh">
            {(services.data ?? []).map((s) => (
              <NavLink key={s.id} active={s.id === sid} label={s.name} description={money(s.price)} onClick={() => setParams({ tab: "prices", service: String(s.id) }, { replace: true })} style={{ borderRadius: 8 }} />
            ))}
          </ScrollArea.Autosize>
        </Paper>
      </Grid.Col>
      <Grid.Col span={{ base: 12, lg: 9 }}>
        {sid ? (
          <Stack>
            <Group justify="space-between">
              <Text size="sm" c="dimmed">
                Your price: <b>{money(svc?.price)}</b>. Cost-based target: <b>{money(svc?.target_price)}</b>.
              </Text>
              <Button
                leftSection={<IconPlus size={16} />}
                onClick={() => setObs({ id: null, competitor_id: 0, service_id: sid, service_label: svc?.name ?? "", price: "", price_type: "exact", price_max: null, duration_min: null, hair_length: "", stylist_level: "", inclusions: "", source_url: "", observed_on: today, notes: "" })}
              >
                Record a price
              </Button>
            </Group>
            <MarketEvidence serviceId={sid} position={position} targetPrice={svc?.target_price ?? null} detailed />
          </Stack>
        ) : (
          <Text c="dimmed">Create a service first.</Text>
        )}
      </Grid.Col>
      {obs && <ObservationModal value={obs} onClose={() => setObs(null)} />}
    </Grid>
  );
}

const blankComp: CompetitorInput = { id: null, name: "", address: "", city: "", state: "", postal_code: "", latitude: null, longitude: null, website: "", phone: "", notes: "" };

function CompetitorsTab() {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const q = useCmd<Competitor[]>("competitors_list", { includeArchived: false });
  const [edit, setEdit] = useState<CompetitorInput | null>(null);
  const [radius, setRadius] = useState(5);
  const [busy, setBusy] = useState(false);
  const save = useAction<{ competitor: CompetitorInput }, number>("competitor_save", { invalidate: ["competitors_list"], success: "Business saved" });
  async function discover() {
    setBusy(true);
    try {
      const r = await call<{ found: number; added: number; updated: number; attribution: string }>("competitors_discover", { radiusKm: radius });
      notifications.show({ message: `Found ${r.found} salons on OpenStreetMap: ${r.added} added, ${r.updated} refreshed. ${r.attribution}`, color: "teal", autoClose: 10000 });
      qc.invalidateQueries({ queryKey: ["competitors_list"] });
    } catch (e) {
      notifications.show({ title: "Couldn't search nearby", message: errorMessage(e), color: "red", autoClose: 12000 });
    } finally {
      setBusy(false);
    }
  }
  return (
    <Stack>
      <Paper p="md">
        <Group justify="space-between" align="flex-end">
          <Group align="flex-end" gap="sm">
            <NumberInput label="Find hair and beauty salons within" suffix=" km" min={0.5} max={25} step={0.5} w={240} value={radius} onChange={(v) => setRadius(Number(v) || 5)} />
            <Button leftSection={<IconMapSearch size={16} />} loading={busy} onClick={discover}>
              Search OpenStreetMap
            </Button>
          </Group>
          <Button variant="default" leftSection={<IconPlus size={16} />} onClick={() => setEdit({ ...blankComp })}>
            Add a business
          </Button>
        </Group>
        <Text size="xs" c="dimmed" mt="xs">
          Sends only your location's coordinates to the public Overpass service. OpenStreetMap lists names and places, not prices; record prices yourself from each business's menu.
          Map data © OpenStreetMap contributors (ODbL).
        </Text>
      </Paper>
      <QueryState loading={q.isLoading} error={q.error}>
        {() => (
          <DataTable
            withTableBorder
            borderRadius="lg"
            records={q.data!}
            idAccessor={(c) => c.input.id!}
            minHeight={q.data!.length ? undefined : 140}
            noRecordsText="No competitors yet"
            columns={[
              {
                accessor: "name",
                title: "Business",
                render: (c) => (
                  <div>
                    <Text size="sm" fw={600}>
                      {c.input.name}
                    </Text>
                    <Text size="xs" c="dimmed">
                      {[c.input.address, c.input.city].filter(Boolean).join(", ")}
                    </Text>
                  </div>
                ),
              },
              { accessor: "distance", title: "Distance", textAlign: "right", render: (c) => (c.distance_km != null ? `${c.distance_km.toFixed(1)} km` : "—") },
              { accessor: "source", title: "Source", render: (c) => <Badge color={c.source === "osm" ? "green" : "gray"}>{c.source === "osm" ? "OpenStreetMap" : c.source === "csv" ? "Imported" : "Added by you"}</Badge> },
              { accessor: "observations", title: "Prices", textAlign: "right" },
              {
                accessor: "web",
                title: "",
                render: (c) => (
                  <Group gap={6} justify="flex-end">
                    {c.input.website && (
                      <Anchor size="xs" onClick={() => openUrl(c.input.website)}>
                        Website
                      </Anchor>
                    )}
                    <Button size="compact-xs" variant="subtle" onClick={() => setEdit(c.input)}>
                      Edit
                    </Button>
                  </Group>
                ),
              },
            ]}
          />
        )}
      </QueryState>
      <Modal opened={!!edit} onClose={() => setEdit(null)} title={edit?.id ? "Edit business" : "Add a business"}>
        {edit && (
          <Stack>
            <TextInput label="Name" required value={edit.name} onChange={(e) => setEdit({ ...edit, name: e.currentTarget.value })} data-autofocus />
            <TextInput label="Address" value={edit.address} onChange={(e) => setEdit({ ...edit, address: e.currentTarget.value })} />
            <SimpleGrid cols={3}>
              <TextInput label="City" value={edit.city} onChange={(e) => setEdit({ ...edit, city: e.currentTarget.value })} />
              <TextInput label="State" value={edit.state} onChange={(e) => setEdit({ ...edit, state: e.currentTarget.value })} />
              <TextInput label="ZIP" value={edit.postal_code} onChange={(e) => setEdit({ ...edit, postal_code: e.currentTarget.value })} />
            </SimpleGrid>
            <TextInput label="Website" value={edit.website} onChange={(e) => setEdit({ ...edit, website: e.currentTarget.value })} />
            <Textarea label="Notes" value={edit.notes} onChange={(e) => setEdit({ ...edit, notes: e.currentTarget.value })} />
            <Group justify="flex-end">
              <Button onClick={() => save.mutate({ competitor: edit }, { onSuccess: () => setEdit(null) })} loading={save.isPending}>
                Save business
              </Button>
            </Group>
          </Stack>
        )}
      </Modal>
      <Text size="xs" c="dimmed">
        Prices you record appear under each service on the{" "}
        <Anchor size="xs" onClick={() => navigate("/market?tab=prices")}>
          Prices
        </Anchor>{" "}
        tab.
      </Text>
    </Stack>
  );
}

const MAP_FIELDS: { key: keyof ObservationMapping; label: string; required?: boolean }[] = [
  { key: "business", label: "Business name", required: true },
  { key: "service_label", label: "Service as listed", required: true },
  { key: "price", label: "Price", required: true },
  { key: "observed_on", label: "Date observed (YYYY-MM-DD)", required: true },
  { key: "price_type", label: "Price type (exact, from, range)" },
  { key: "price_max", label: "Highest price (ranges)" },
  { key: "source_url", label: "Source URL" },
  { key: "matched_service", label: "Your matching service name" },
  { key: "hair_length", label: "Hair length" },
  { key: "stylist_level", label: "Stylist level" },
  { key: "duration_min", label: "Duration (minutes)" },
  { key: "inclusions", label: "Inclusions" },
  { key: "notes", label: "Notes" },
];

function ImportTab() {
  const qc = useQueryClient();
  const services = useCmd<ServiceSummary[]>("services_list", { includeArchived: false });
  const [path, setPath] = useState<string | null>(null);
  const [table, setTable] = useState<CsvTable | null>(null);
  const [mapping, setMapping] = useState<ObservationMapping | null>(null);
  const [defaultService, setDefaultService] = useState<string | null>(null);
  const [preview, setPreview] = useState<ObsImportRow[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  async function choose() {
    const p = await open({ title: "Choose a CSV of observed prices", filters: [{ name: "CSV", extensions: ["csv", "txt"] }] });
    if (typeof p !== "string") return;
    try {
      const t = await call<CsvTable>("csv_read", { path: p });
      const guess = (re: RegExp) => {
        const i = t.headers.findIndex((h) => re.test(h));
        return i >= 0 ? i : null;
      };
      setPath(p);
      setTable(t);
      setPreview(null);
      setMapping({
        business: guess(/salon|business|competitor|shop/i),
        service_label: guess(/service/i),
        price: guess(/^price$|amount|cost/i),
        price_type: guess(/type/i),
        price_max: guess(/max|high/i),
        observed_on: guess(/date|observed/i),
        source_url: guess(/url|source|link/i),
        hair_length: guess(/length/i),
        stylist_level: guess(/level/i),
        duration_min: guess(/duration|min/i),
        inclusions: guess(/includ/i),
        notes: guess(/note/i),
        matched_service: guess(/match/i),
      });
    } catch (e) {
      setErr(errorMessage(e));
    }
  }
  async function runPreview() {
    try {
      setPreview(await call<ObsImportRow[]>("observations_import_preview", { path, mapping, defaultService: defaultService ? Number(defaultService) : null }));
      setErr(null);
    } catch (e) {
      setErr(errorMessage(e));
    }
  }
  async function commit() {
    try {
      const n = await call<number>("observations_import_commit", { path, mapping, defaultService: defaultService ? Number(defaultService) : null });
      notifications.show({ message: `Imported ${n} prices`, color: "teal" });
      qc.invalidateQueries();
      setTable(null);
      setPreview(null);
    } catch (e) {
      setErr(errorMessage(e));
    }
  }
  return (
    <Stack maw={980}>
      <Text size="sm" c="dimmed">
        Import prices you've collected in a spreadsheet: one row per business and service, with the date you saw it. Each row keeps its source; rows already recorded are skipped.
      </Text>
      <Group>
        <Button leftSection={<IconFileImport size={16} />} onClick={choose}>
          Choose CSV file…
        </Button>
      </Group>
      {err && <Alert color="red">{err}</Alert>}
      {table && mapping && (
        <Paper p="lg">
          <SimpleGrid cols={3}>
            {MAP_FIELDS.map((f) => (
              <Select
                key={f.key}
                size="xs"
                label={f.label}
                required={f.required}
                clearable={!f.required}
                placeholder="Not in this file"
                data={table.headers.map((h, i) => ({ value: String(i), label: h || `Column ${i + 1}` }))}
                value={mapping[f.key] != null ? String(mapping[f.key]) : null}
                onChange={(v) => setMapping({ ...mapping, [f.key]: v == null ? null : Number(v) })}
              />
            ))}
            <Select size="xs" label="Match rows without a service column to" clearable data={(services.data ?? []).map((s) => ({ value: String(s.id), label: s.name }))} value={defaultService} onChange={setDefaultService} />
          </SimpleGrid>
          <Group justify="flex-end" mt="md">
            <Button variant="default" onClick={runPreview}>
              Preview
            </Button>
          </Group>
        </Paper>
      )}
      {preview && (
        <Paper p="lg">
          <Group gap="xs" mb="sm">
            <Badge color="teal">{preview.filter((r) => r.status === "new").length} new</Badge>
            <Badge color="gray">{preview.filter((r) => r.status === "duplicate").length} duplicates</Badge>
            <Badge color="red">{preview.filter((r) => r.status === "error").length} errors</Badge>
          </Group>
          <ScrollArea h={300}>
            <Table fz="xs">
              <Table.Tbody>
                {preview.map((r) => (
                  <Table.Tr key={r.line}>
                    <Table.Td>{r.line}</Table.Td>
                    <Table.Td>{r.business}</Table.Td>
                    <Table.Td>{r.service_label}</Table.Td>
                    <Table.Td>{r.price}</Table.Td>
                    <Table.Td>{r.observed_on}</Table.Td>
                    <Table.Td>{r.matched_service ?? "—"}</Table.Td>
                    <Table.Td c={r.status === "error" ? "red" : r.status === "duplicate" ? "dimmed" : "teal"}>{r.status === "new" ? "Will be added" : r.message}</Table.Td>
                  </Table.Tr>
                ))}
              </Table.Tbody>
            </Table>
          </ScrollArea>
          <Group justify="flex-end" mt="md">
            <Button onClick={commit} disabled={!preview.some((r) => r.status === "new")}>
              Import {preview.filter((r) => r.status === "new").length} prices
            </Button>
          </Group>
        </Paper>
      )}
    </Stack>
  );
}

function ContextTab() {
  const [load, setLoad] = useState(false);
  const q = useCmd<LocalContext>("market_context", {}, load);
  return (
    <Stack maw={980}>
      <Alert color="blue" title="Context, not prices">
        These official statistics describe your area and the trade. They are not salon prices and are never used as a "going rate". They can help you judge what local clients can afford
        and how prices are moving nationally.
      </Alert>
      {!load && (
        <Group>
          <Button onClick={() => setLoad(true)}>Load local context</Button>
          <Text size="xs" c="dimmed">
            Sends your ZIP code, county and state codes to the Census Bureau and BLS. Results are cached.
          </Text>
        </Group>
      )}
      {load && (
        <QueryState loading={q.isLoading} error={q.error}>
          {() => {
            const c = q.data!;
            return (
              <SimpleGrid cols={{ base: 1, lg: 2 }} spacing="lg">
                <Paper p="lg">
                  <Title order={5} mb="sm">
                    Census Bureau
                  </Title>
                  {c.census_error && <Alert color="yellow">{c.census_error}</Alert>}
                  {c.census.map((f, i) => (
                    <Stack key={i} gap={0} mb="sm">
                      <Group justify="space-between">
                        <Text size="sm">{f.label}</Text>
                        <Text size="sm" fw={650}>
                          {f.value ?? "Not available"}
                        </Text>
                      </Group>
                      <Text size="xs" c="dimmed">
                        {f.geography}. {f.dataset}. {f.note}.
                      </Text>
                    </Stack>
                  ))}
                  <Text size="xs" c="dimmed" mt="sm">
                    This product uses the Census Bureau Data API but is not endorsed or certified by the Census Bureau.
                  </Text>
                </Paper>
                <Paper p="lg">
                  <Title order={5} mb="sm">
                    Bureau of Labor Statistics
                  </Title>
                  {c.bls_error && <Alert color="yellow">{c.bls_error}</Alert>}
                  {c.wages.map(([area, pts]) => (
                    <Group key={area} justify="space-between">
                      <Text size="sm">Hairdresser and cosmetologist mean wage, {area}</Text>
                      <Text size="sm" fw={650}>
                        {pts[0] ? `${money(pts[0].value)}/h (${pts[0].year})` : "—"}
                      </Text>
                    </Group>
                  ))}
                  {c.cpi.length > 0 && (
                    <Text size="sm" mt="sm">
                      Consumer price index for haircuts and personal care services (U.S. city average): {c.cpi[0].value} in {c.cpi[0].year}-{c.cpi[0].period.slice(1)}
                      {c.cpi.find((p) => p.year === String(Number(c.cpi[0].year) - 1) && p.period === c.cpi[0].period)
                        ? `, up from ${c.cpi.find((p) => p.year === String(Number(c.cpi[0].year) - 1) && p.period === c.cpi[0].period)!.value} a year earlier`
                        : ""}
                      .
                    </Text>
                  )}
                  <Text size="xs" c="dimmed" mt="sm">
                    Wages come from Occupational Employment and Wage Statistics (OEWS), annual. The CPI is used for the modeled inflation adjustment of older observed prices.
                  </Text>
                </Paper>
              </SimpleGrid>
            );
          }}
        </QueryState>
      )}
    </Stack>
  );
}

export function MarketPage() {
  const [params, setParams] = useSearchParams();
  return (
    <>
      <PageHeader
        title="Market"
        description="Prices you observe at local businesses, kept separate from official statistics and from modeled estimates. No free service provides salon menu prices, so this evidence is collected by you."
      />
      <Tabs value={params.get("tab") ?? "prices"} onChange={(v) => setParams({ tab: v ?? "prices" }, { replace: true })} keepMounted={false}>
        <Tabs.List mb="md">
          <Tabs.Tab value="prices">Prices</Tabs.Tab>
          <Tabs.Tab value="competitors">Competitors</Tabs.Tab>
          <Tabs.Tab value="import">Import prices</Tabs.Tab>
          <Tabs.Tab value="context">Local context</Tabs.Tab>
        </Tabs.List>
        <Tabs.Panel value="prices">
          <PricesTab />
        </Tabs.Panel>
        <Tabs.Panel value="competitors">
          <CompetitorsTab />
        </Tabs.Panel>
        <Tabs.Panel value="import">
          <ImportTab />
        </Tabs.Panel>
        <Tabs.Panel value="context">
          <ContextTab />
        </Tabs.Panel>
      </Tabs>
    </>
  );
}
