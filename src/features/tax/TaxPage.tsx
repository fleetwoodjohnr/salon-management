import { Alert, Anchor, Badge, Button, Group, Modal, Paper, SegmentedControl, Select, SimpleGrid, Stack, Table, Text, TextInput, Textarea, Title } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconAlertTriangle, IconCheck, IconWorldSearch } from "@tabler/icons-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { call, errorMessage, isAppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { Location, RateSet, RateSetInput, TaxabilityRule, TaxLookup, TaxOverview } from "../../api/types";
import { DecimalInput } from "../../components/DecimalInput";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { isoToLocal, pct } from "../../lib/format";

const statusColor: Record<string, string> = { verified: "teal", estimate: "yellow", manual: "blue", stale: "orange" };
const statusLabel: Record<string, string> = { verified: "Official lookup", estimate: "Estimate", manual: "Entered by you" };
const precisionLabel: Record<string, string> = {
  address: "exact address",
  zip9: "ZIP+4 area",
  zip5: "5-digit ZIP (may cross boundaries)",
  city: "city",
  county: "county",
  state: "state only (not a local rate)",
  unknown: "unknown",
};
const LOOKUP_STATES = new Set(["WA", "CA"]);

function RateCard({ rate }: { rate: RateSet }) {
  return (
    <Stack gap={6}>
      <Group gap="xs">
        <Text className="srm-figure" fz={40}>
          {pct(rate.total_rate, 3)}
        </Text>
        <Badge color={statusColor[rate.status]}>{statusLabel[rate.status]}</Badge>
        {rate.freshness === "stale" && <Badge color="orange">Stale</Badge>}
      </Group>
      <Text size="sm">{rate.jurisdiction_label || "Jurisdiction not named"}</Text>
      <Table fz="sm" withRowBorders={false} verticalSpacing={2}>
        <Table.Tbody>
          {(
            [
              ["State", rate.state_rate],
              ["County", rate.county_rate],
              ["City / local", rate.city_rate],
              ["District", rate.district_rate],
            ] as const
          )
            .filter(([, v]) => v != null)
            .map(([k, v]) => (
              <Table.Tr key={k}>
                <Table.Td c="dimmed" w={160}>
                  {k}
                </Table.Td>
                <Table.Td>{pct(v, 3)}</Table.Td>
              </Table.Tr>
            ))}
          <Table.Tr>
            <Table.Td c="dimmed">Precision</Table.Td>
            <Table.Td>{precisionLabel[rate.precision]}</Table.Td>
          </Table.Tr>
          {rate.dataset_period && (
            <Table.Tr>
              <Table.Td c="dimmed">Data period</Table.Td>
              <Table.Td>{rate.dataset_period}</Table.Td>
            </Table.Tr>
          )}
          {rate.effective_date && (
            <Table.Tr>
              <Table.Td c="dimmed">Effective</Table.Td>
              <Table.Td>{rate.effective_date}</Table.Td>
            </Table.Tr>
          )}
          {rate.retrieved_at && (
            <Table.Tr>
              <Table.Td c="dimmed">Retrieved</Table.Td>
              <Table.Td>{isoToLocal(rate.retrieved_at)}</Table.Td>
            </Table.Tr>
          )}
          <Table.Tr>
            <Table.Td c="dimmed">Saved</Table.Td>
            <Table.Td>
              {isoToLocal(rate.created_at)} (version {rate.version})
            </Table.Td>
          </Table.Tr>
        </Table.Tbody>
      </Table>
      <Text size="xs" c={rate.freshness === "stale" ? "var(--srm-warn)" : "dimmed"}>
        {rate.freshness_note}
      </Text>
      {rate.note && (
        <Text size="xs" c="dimmed">
          {rate.note}
        </Text>
      )}
      {rate.source_url && (
        <Anchor size="xs" onClick={() => openUrl(rate.source_url!)}>
          Source
        </Anchor>
      )}
    </Stack>
  );
}

function ManualRateModal({ location, opened, onClose }: { location: Location; opened: boolean; onClose: () => void }) {
  const blank = (): RateSetInput => ({
    location_id: location.id!,
    state_rate: null,
    county_rate: null,
    city_rate: null,
    district_rate: null,
    total_rate: null,
    jurisdiction_label: [location.city, location.state].filter(Boolean).join(", "),
    jurisdiction_code: null,
    source: "manual",
    source_url: null,
    precision: "address",
    status: "manual",
    effective_date: null,
    dataset_period: null,
    retrieved_at: null,
    note: "",
  });
  const [r, setR] = useState<RateSetInput>(blank);
  const [err, setErr] = useState<{ field: string | null; message: string } | null>(null);
  useEffect(() => {
    if (opened) {
      setR(blank());
      setErr(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [opened]);
  const save = useAction<{ rate: RateSetInput }, RateSet>("tax_rate_save", { invalidate: ["tax_overview", "tax_rate_history", "services_list", "service_estimate"], success: "Tax rate saved", silentError: true });
  const f = (k: string) => (err?.field === k ? err.message : undefined);
  const dec = (k: keyof RateSetInput, label: string) => (
    <DecimalInput label={label} unit="%" value={(r[k] as string | null) ?? ""} onChange={(v) => setR({ ...r, [k]: v || null })} error={f(k)} />
  );
  return (
    <Modal opened={opened} onClose={onClose} title="Enter a sales tax rate" size="lg">
      <Stack>
        <Text size="sm" c="dimmed">
          Use the rate from your state revenue department for this exact address. Enter the parts if you know them, or just the total. Leave a part blank if it isn't supplied — it won't be
          treated as zero.
        </Text>
        <SimpleGrid cols={3}>
          {dec("state_rate", "State")}
          {dec("county_rate", "County")}
          {dec("city_rate", "City / local")}
          {dec("district_rate", "Special district")}
          {dec("total_rate", "Total")}
        </SimpleGrid>
        <SimpleGrid cols={2}>
          <TextInput label="Jurisdiction" value={r.jurisdiction_label} onChange={(e) => setR({ ...r, jurisdiction_label: e.currentTarget.value })} />
          <Select
            label="How precise is it?"
            data={Object.entries(precisionLabel).map(([value, label]) => ({ value, label }))}
            value={r.precision}
            allowDeselect={false}
            onChange={(v) => setR({ ...r, precision: v as RateSetInput["precision"], status: v === "address" ? "manual" : "estimate" })}
          />
          <TextInput type="date" label="Effective date" value={r.effective_date ?? ""} onChange={(e) => setR({ ...r, effective_date: e.currentTarget.value || null })} error={f("effective_date")} />
          <TextInput label="Source (web page)" placeholder="https://…" value={r.source_url ?? ""} onChange={(e) => setR({ ...r, source_url: e.currentTarget.value || null })} />
        </SimpleGrid>
        <Textarea label="Note" value={r.note} onChange={(e) => setR({ ...r, note: e.currentTarget.value })} />
        {r.precision !== "address" && (
          <Alert color="yellow" icon={<IconAlertTriangle size={16} />}>
            A rate that isn't for your exact address is saved as an estimate. It can still be used, but it's labelled that way on sales.
          </Alert>
        )}
        {err && !err.field && <Alert color="red">{err.message}</Alert>}
        <Group justify="flex-end">
          <Button
            loading={save.isPending}
            onClick={() => save.mutate({ rate: r }, { onSuccess: onClose, onError: (e) => setErr(isAppError(e) ? { field: e.field, message: e.message } : { field: null, message: String(e) }) })}
          >
            Save rate
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}

function RuleRow({ rule, locationId }: { rule: TaxabilityRule; locationId: number }) {
  const [status, setStatus] = useState(rule.status);
  const [basis, setBasis] = useState(rule.basis);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    setStatus(rule.status);
    setBasis(rule.basis);
  }, [rule]);
  const save = useAction<Record<string, unknown>>("tax_rule_set", { invalidate: ["tax_overview", "services_list", "service_estimate"], success: `${rule.category_label} updated`, silentError: true });
  const changed = status !== rule.status || basis !== rule.basis;
  return (
    <div style={{ padding: "10px 0", borderBottom: "1px solid var(--srm-rule)" }} role="group" aria-label={rule.category_label}>
      <Group justify="space-between" wrap="nowrap" mb={6}>
        <div>
          <Text size="sm" fw={600}>
            {rule.category_label}
          </Text>
          <Text size="xs" c="dimmed">
            {rule.decided_at ? `Decided ${isoToLocal(rule.decided_at)}` : "Not decided yet"}
          </Text>
        </div>
        <SegmentedControl
          size="xs"
          value={status}
          onChange={(v) => setStatus(v as TaxabilityRule["status"])}
          data={[
            { value: "taxable", label: "Taxable" },
            { value: "exempt", label: "Not taxable" },
            { value: "unknown", label: "Not decided" },
          ]}
        />
      </Group>
      <Group gap="xs" wrap="nowrap" align="flex-start">
        <TextInput
          size="xs"
          style={{ flex: 1 }}
          aria-label={`${rule.category_label} basis`}
          placeholder="Where this comes from, e.g. your state revenue department's page or your accountant"
          value={basis}
          onChange={(e) => setBasis(e.currentTarget.value)}
          error={err}
        />
        <Button
          size="xs"
          variant={changed ? "filled" : "default"}
          disabled={!changed}
          loading={save.isPending}
          onClick={() =>
            save.mutate({ locationId, categoryCode: rule.category_code, status, basis }, { onSuccess: () => setErr(null), onError: (e) => setErr(isAppError(e) ? e.message : String(e)) })
          }
        >
          Save
        </Button>
      </Group>
    </div>
  );
}

export function TaxPage() {
  const navigate = useNavigate();
  const locations = useCmd<Location[]>("locations_list");
  const [locationId, setLocationId] = useState<number | null>(null);
  const q = useCmd<TaxOverview>("tax_overview", { locationId });
  const history = useCmd<RateSet[]>("tax_rate_history", { locationId: q.data?.location?.id ?? 0 }, !!q.data?.location?.id);
  const [manual, setManual] = useState(false);
  const [lookup, setLookup] = useState<TaxLookup | null>(null);
  const [busy, setBusy] = useState(false);
  const apply = useAction<Record<string, unknown>, RateSet>("tax_rate_apply_lookup", { invalidate: ["tax_overview", "tax_rate_history", "services_list", "service_estimate"], success: "Official rate saved" });
  const clear = useAction<Record<string, unknown>>("tax_rate_clear", { invalidate: ["tax_overview", "tax_rate_history", "services_list", "service_estimate"], success: "Rate cleared" });
  const [newCat, setNewCat] = useState<string | null>(null);
  const addCat = useAction<Record<string, unknown>>("tax_category_add", { invalidate: ["tax_overview", "tax_categories"], success: "Category added" });

  async function runLookup(id: number) {
    setBusy(true);
    try {
      setLookup(await call<TaxLookup>("tax_lookup", { locationId: id }));
    } catch (e) {
      notifications.show({ title: "Lookup didn't work", message: errorMessage(e), color: "red", autoClose: 12000 });
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <PageHeader
        title="Sales tax"
        description="Three separate questions: where you are, what the rate is there, and whether each kind of sale is taxable. Anything unresolved stays unresolved — it never quietly becomes 0%."
      />
      <QueryState loading={q.isLoading} error={q.error}>
        {() => {
          const o = q.data!;
          if (!o.location) {
            return (
              <Alert color="blue" title="Add a location first">
                Sales tax depends on where you work.{" "}
                <Button size="compact-sm" variant="subtle" onClick={() => navigate("/settings?tab=locations")}>
                  Add a location
                </Button>
              </Alert>
            );
          }
          const loc = o.location;
          return (
            <Stack gap="lg">
              <Group justify="space-between">
                <Select
                  label="Location"
                  w={320}
                  data={(locations.data ?? []).filter((l) => !l.archived).map((l) => ({ value: String(l.id), label: l.name }))}
                  value={String(loc.id)}
                  allowDeselect={false}
                  onChange={(v) => setLocationId(Number(v))}
                />
                <Text size="sm" c="dimmed">
                  Prices are entered {o.prices_include_tax ? "including" : "before"} tax.{" "}
                  <Anchor size="sm" onClick={() => navigate("/settings?tab=business")}>
                    Change
                  </Anchor>
                </Text>
              </Group>
              {o.unresolved.length > 0 ? (
                <Alert color="yellow" icon={<IconAlertTriangle size={18} />} title="Sales can't be finalized until these are resolved">
                  <Stack gap={2}>
                    {o.unresolved.map((u) => (
                      <Text key={u} size="sm">
                        {u}
                      </Text>
                    ))}
                  </Stack>
                  <Text size="xs" mt="xs">
                    You can still record draft sales; their tax shows as an estimate until this is done.
                  </Text>
                </Alert>
              ) : (
                <Alert color="teal" icon={<IconCheck size={18} />}>
                  Tax is set up for {loc.name}. Completed sales keep the rate and decision they used, even if you change these later.
                </Alert>
              )}
              <SimpleGrid cols={{ base: 1, lg: 2 }} spacing="lg">
                <Paper p="lg">
                  <Group justify="space-between" mb="sm">
                    <Title order={4}>Rate at {loc.name}</Title>
                    <Group gap="xs">
                      {LOOKUP_STATES.has(loc.state) && (
                        <Button size="xs" leftSection={<IconWorldSearch size={14} />} loading={busy} onClick={() => runLookup(loc.id!)}>
                          Look up official rate
                        </Button>
                      )}
                      <Button size="xs" variant="default" onClick={() => setManual(true)}>
                        Enter manually
                      </Button>
                    </Group>
                  </Group>
                  {o.rate ? (
                    <>
                      <RateCard rate={o.rate} />
                      <Button size="compact-xs" variant="subtle" color="gray" mt="sm" onClick={() => clear.mutate({ locationId: loc.id, note: "Cleared from Sales tax page" })}>
                        Clear this rate
                      </Button>
                    </>
                  ) : (
                    <Text size="sm" c="dimmed">
                      No rate set.{" "}
                      {LOOKUP_STATES.has(loc.state)
                        ? `${loc.state === "WA" ? "Washington's Department of Revenue" : "California's CDTFA"} offers a free official lookup by address.`
                        : `There's no verified free official lookup for ${loc.state || "this state"} in this app; enter the rate from your state revenue department.`}
                    </Text>
                  )}
                  <Text size="xs" c="dimmed" mt="md">
                    Looking up sends only this location's street address, city and ZIP to the state's service.
                  </Text>
                </Paper>
                <Paper p="lg">
                  <Title order={4} mb={4}>
                    What's taxable
                  </Title>
                  <Text size="sm" c="dimmed" mb="sm">
                    Rules differ by state and by kind of service. This app doesn't guess: record each decision and where it came from.
                  </Text>
                  {o.rules.map((r) => (
                    <RuleRow key={r.category_code} rule={r} locationId={loc.id!} />
                  ))}
                  {newCat === null ? (
                    <Button size="compact-xs" variant="subtle" mt="xs" onClick={() => setNewCat("")}>
                      Add a category (e.g. nail services)
                    </Button>
                  ) : (
                    <Group mt="xs" gap="xs">
                      <TextInput size="xs" placeholder="Category name" value={newCat} onChange={(e) => setNewCat(e.currentTarget.value)} />
                      <Button size="xs" onClick={() => addCat.mutate({ label: newCat, appliesTo: "service" }, { onSuccess: () => setNewCat(null) })}>
                        Add
                      </Button>
                    </Group>
                  )}
                </Paper>
              </SimpleGrid>
              {(history.data?.length ?? 0) > 1 && (
                <Paper p="lg">
                  <Title order={4} mb="sm">
                    Rate history
                  </Title>
                  <Table fz="sm">
                    <Table.Thead>
                      <Table.Tr>
                        <Table.Th>Version</Table.Th>
                        <Table.Th>Rate</Table.Th>
                        <Table.Th>Source</Table.Th>
                        <Table.Th>Saved</Table.Th>
                        <Table.Th>Replaced</Table.Th>
                      </Table.Tr>
                    </Table.Thead>
                    <Table.Tbody>
                      {history.data!.map((h) => (
                        <Table.Tr key={h.id}>
                          <Table.Td>{h.version}</Table.Td>
                          <Table.Td>{pct(h.total_rate, 3)}</Table.Td>
                          <Table.Td>{statusLabel[h.status]}</Table.Td>
                          <Table.Td>{isoToLocal(h.created_at)}</Table.Td>
                          <Table.Td>{h.superseded_at ? isoToLocal(h.superseded_at) : "Current"}</Table.Td>
                        </Table.Tr>
                      ))}
                    </Table.Tbody>
                  </Table>
                </Paper>
              )}
              <ManualRateModal location={loc} opened={manual} onClose={() => setManual(false)} />
              <Modal opened={!!lookup} onClose={() => setLookup(null)} title="Official rate lookup" size="lg">
                {lookup && (
                  <Stack>
                    <Group gap="xs">
                      <Text className="srm-figure" fz={36}>
                        {pct(lookup.rate.total_rate, 3)}
                      </Text>
                      <Badge color={statusColor[lookup.rate.status]}>{lookup.rate.status === "verified" ? "Address match" : "Estimate"}</Badge>
                    </Group>
                    <Text size="sm">{lookup.rate.jurisdiction_label}</Text>
                    <Text size="sm">
                      Matched address: <b>{lookup.matched_address || "not returned"}</b>. Check it's yours before saving.
                    </Text>
                    <Text size="xs" c="dimmed">
                      Precision: {precisionLabel[lookup.rate.precision]}. {lookup.rate.dataset_period ? `Data period ${lookup.rate.dataset_period}. ` : ""}Retrieved {isoToLocal(lookup.fetched_at)}.
                    </Text>
                    {lookup.notes.map((n) => (
                      <Alert key={n} color="yellow" py={6}>
                        <Text size="xs">{n}</Text>
                      </Alert>
                    ))}
                    <Text size="xs" c="dimmed">
                      This is the rate for the location. Whether a service or product is taxable is decided separately on this page.
                    </Text>
                    <Group justify="flex-end">
                      <Button variant="default" onClick={() => setLookup(null)}>
                        Don't use
                      </Button>
                      <Button
                        loading={apply.isPending}
                        onClick={() => apply.mutate({ locationId: loc.id, provider: lookup.provider, requestKey: lookup.request_key }, { onSuccess: () => setLookup(null) })}
                      >
                        Use this rate
                      </Button>
                    </Group>
                  </Stack>
                )}
              </Modal>
            </Stack>
          );
        }}
      </QueryState>
    </>
  );
}
