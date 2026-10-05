import {
  Alert,
  Badge,
  Button,
  Chip,
  Grid,
  Group,
  Modal,
  NavLink,
  NumberInput,
  Paper,
  ScrollArea,
  SegmentedControl,
  Select,
  SimpleGrid,
  Slider,
  Stack,
  Table,
  Text,
  Textarea,
  TextInput,
  Title,
} from "@mantine/core";
import { useDebouncedValue } from "@mantine/hooks";
import { IconArrowRight, IconRefresh, IconSearch } from "@tabler/icons-react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router";
import { call, isAppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { Estimate, ProfileView, ServiceSummary, ServiceView, WhatIf, WhatIfResult } from "../../api/types";
import { DecimalInput } from "../../components/DecimalInput";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { money, pct } from "../../lib/format";
import { positionLabels } from "../profiles/labels";
import { MarketEvidence } from "../market/MarketEvidence";
import { statusBadge } from "../services/ServicesPage";

type Row = { label: string; get: (e: Estimate) => string | null | undefined; fmt?: (v: string | null | undefined) => string };

const ROWS: Row[] = [
  { label: "Materials", get: (e) => e.cost?.materials },
  { label: "Estimated waste", get: (e) => e.cost?.waste },
  { label: "Labor", get: (e) => e.cost?.labor },
  { label: "Overhead", get: (e) => e.cost?.overhead },
  { label: "Cost before fees", get: (e) => e.cost?.fixed_total },
  { label: "Break-even price", get: (e) => e.pricing?.targets?.break_even },
  { label: "Price for target", get: (e) => e.pricing?.targets?.target_price },
  { label: "Rounded target price", get: (e) => e.pricing?.targets?.target_rounded },
  { label: "Your price", get: (e) => e.pricing?.chosen_price },
  { label: "Estimated profit", get: (e) => e.pricing?.at_price?.profit },
  { label: "Margin", get: (e) => e.pricing?.at_price?.margin_pct, fmt: (v) => pct(v) },
  { label: "Earnings per hands-on hour", get: (e) => e.pricing?.earnings_per_working_hour },
  { label: "Earnings per chair hour", get: (e) => e.pricing?.earnings_per_occupied_hour },
];

function emptyWhatIf(id: number): WhatIf {
  return { service_id: id, variant_ids: [], target_kind: null, target_pct: null, position: null, monthly_overhead: null, utilization_pct: null, time: null, material_factor: null, price: null };
}

function WhatIfPanel({ service }: { service: ServiceView }) {
  const sid = service.input.id!;
  const profile = useCmd<ProfileView>("profile_get", { id: service.input.profile_id ?? 0 }, !!service.input.profile_id);
  const [w, setW] = useState<WhatIf>(emptyWhatIf(sid));
  useEffect(() => setW(emptyWhatIf(sid)), [sid]);
  const [debounced] = useDebouncedValue(w, 200);
  const q = useQuery({
    queryKey: ["pricing_what_if", debounced],
    queryFn: () => call<WhatIfResult>("pricing_what_if", { whatIf: debounced }),
    placeholderData: keepPreviousData,
    enabled: debounced.service_id === sid,
  });
  const err = q.error && isAppError(q.error) ? q.error : null;
  const [saving, setSaving] = useState(false);
  const [note, setNote] = useState("");
  const save = useAction<{ service: ServiceView["input"] }, ServiceView>("service_save", { invalidate: ["services_list", "service_get", "pricing_what_if"], success: "Price saved to the service" });

  const p = profile.data?.data;
  const time = w.time ?? service.input.time;
  const groups = [...new Set(service.input.variants.map((v) => v.group_name))];
  const sc = q.data?.scenario.pricing;
  const below = sc && (sc.status === "below_cost" || sc.status === "below_target");
  const changed = JSON.stringify(w) !== JSON.stringify(emptyWhatIf(sid));

  return (
    <Stack gap="lg">
      <Paper p="lg">
        <Group justify="space-between" mb="sm">
          <Title order={4}>What if…</Title>
          <Button size="xs" variant="subtle" leftSection={<IconRefresh size={14} />} disabled={!changed} onClick={() => setW(emptyWhatIf(sid))}>
            Back to current settings
          </Button>
        </Group>
        <Text size="sm" c="dimmed" mb="md">
          Try different assumptions. Nothing is saved unless you choose to save the price.
        </Text>
        {groups.length > 0 && (
          <Group gap="lg" mb="md">
            {groups.map((g) => (
              <Group key={g} gap={6}>
                <Text size="xs" c="dimmed">
                  {g}
                </Text>
                <Chip.Group
                  value={String(w.variant_ids.find((id) => service.input.variants.find((v) => v.id === id)?.group_name === g) ?? "")}
                  onChange={(v) => {
                    const others = w.variant_ids.filter((id) => service.input.variants.find((x) => x.id === id)?.group_name !== g);
                    setW({ ...w, variant_ids: v ? [...others, Number(v)] : others });
                  }}
                >
                  <Group gap={4}>
                    <Chip size="xs" value="">
                      Base
                    </Chip>
                    {service.input.variants
                      .filter((v) => v.group_name === g)
                      .map((v) => (
                        <Chip key={v.id} size="xs" value={String(v.id)}>
                          {v.name}
                        </Chip>
                      ))}
                  </Group>
                </Chip.Group>
              </Group>
            ))}
          </Group>
        )}
        <SimpleGrid cols={{ base: 2, xl: 4 }}>
          <div>
            <Text size="sm" fw={500} mb={4}>
              Target type
            </Text>
            <SegmentedControl
              fullWidth
              size="xs"
              value={w.target_kind ?? q.data?.base.pricing?.params.target_kind ?? "margin"}
              onChange={(v) => setW({ ...w, target_kind: v as WhatIf["target_kind"] })}
              data={[
                { value: "margin", label: "Margin" },
                { value: "markup", label: "Markup" },
              ]}
            />
          </div>
          <DecimalInput label="Target" unit="%" placeholder={q.data?.base.pricing?.params.target_pct ?? ""} value={w.target_pct ?? ""} onChange={(v) => setW({ ...w, target_pct: v || null })} error={err?.field === "target_pct" ? err.message : undefined} />
          <Select
            label="Market position"
            data={Object.entries(positionLabels).map(([value, x]) => ({ value, label: x.label }))}
            value={w.position ?? q.data?.base.pricing?.position ?? "standard"}
            allowDeselect={false}
            onChange={(v) => setW({ ...w, position: v as WhatIf["position"] })}
          />
          <DecimalInput label="Your price" unit="$" placeholder={service.input.price ?? "none"} value={w.price ?? ""} onChange={(v) => setW({ ...w, price: v || null })} />
          <DecimalInput
            label="Monthly overhead"
            unit="$"
            placeholder={profile.data?.rates?.monthly_overhead ?? ""}
            value={w.monthly_overhead ?? ""}
            onChange={(v) => setW({ ...w, monthly_overhead: v || null })}
          />
          <DecimalInput label="Billable utilization" unit="%" placeholder={p?.utilization_pct ?? ""} value={w.utilization_pct ?? ""} onChange={(v) => setW({ ...w, utilization_pct: v || null })} error={err?.field === "utilization_pct" ? err.message : undefined} />
          <NumberInput label="Hands-on" suffix=" min" min={0} allowDecimal={false} value={time.hands_on_min} onChange={(v) => setW({ ...w, time: { ...time, hands_on_min: Number(v) || 0 } })} />
          <NumberInput label="Processing" suffix=" min" min={0} allowDecimal={false} value={time.processing_min} onChange={(v) => setW({ ...w, time: { ...time, processing_min: Number(v) || 0 } })} />
        </SimpleGrid>
        <Text size="sm" fw={500} mt="md">
          Material used: {Number(w.material_factor ?? "1").toFixed(2)}× the recipe
        </Text>
        <Slider
          aria-label="Material multiplier"
          min={0.5}
          max={2}
          step={0.05}
          value={Number(w.material_factor ?? "1")}
          onChange={(v) => setW({ ...w, material_factor: v === 1 ? null : v.toFixed(2) })}
          marks={[
            { value: 0.5, label: "½×" },
            { value: 1, label: "1×" },
            { value: 1.5, label: "1½×" },
            { value: 2, label: "2×" },
          ]}
          mb="lg"
        />
        {err && !err.field && <Alert color="red">{err.message}</Alert>}
      </Paper>

      {q.data && (
        <Grid gap="lg">
          <Grid.Col span={{ base: 12, xl: 7 }}>
            <Paper p="lg">
              <Title order={4} mb="sm">
                Current and what-if
              </Title>
              <Table fz="sm">
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th />
                    <Table.Th ta="right">Current</Table.Th>
                    <Table.Th ta="right">What-if</Table.Th>
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {ROWS.map((r) => {
                    const a = r.get(q.data.base);
                    const b = r.get(q.data.scenario);
                    const fmt = r.fmt ?? ((v) => money(v));
                    return (
                      <Table.Tr key={r.label}>
                        <Table.Td>{r.label}</Table.Td>
                        <Table.Td className="srm-num">{fmt(a)}</Table.Td>
                        <Table.Td className="srm-num" fw={a !== b ? 650 : undefined}>
                          {fmt(b)}
                        </Table.Td>
                      </Table.Tr>
                    );
                  })}
                </Table.Tbody>
              </Table>
              {sc && (
                <Group justify="space-between" mt="md">
                  <Badge color={statusBadge[sc.status].color} size="lg">
                    What-if: {statusBadge[sc.status].label}
                  </Badge>
                  <Button
                    disabled={!sc.chosen_price && !sc.targets}
                    rightSection={<IconArrowRight size={16} />}
                    onClick={() => {
                      setNote(service.input.price_note);
                      setSaving(true);
                    }}
                  >
                    Save a price…
                  </Button>
                </Group>
              )}
            </Paper>
          </Grid.Col>
          <Grid.Col span={{ base: 12, xl: 5 }}>
            <Paper p="lg">
              <Title order={4} mb="sm">
                Why it changes
              </Title>
              {q.data.changes.length === 0 ? (
                <Text size="sm" c="dimmed">
                  Nothing differs from the current settings yet.
                </Text>
              ) : (
                <Stack gap="sm">
                  {q.data.changes.map((c) => (
                    <div key={c.item}>
                      <Group justify="space-between">
                        <Text size="sm" fw={600}>
                          {c.item}
                        </Text>
                        {(c.before || c.after) && (
                          <Text size="sm" className="srm-num">
                            {c.item.startsWith("Margin") ? `${pct(c.before)} → ${pct(c.after)}` : `${money(c.before)} → ${money(c.after)}`}
                          </Text>
                        )}
                      </Group>
                      <Text size="sm" c="var(--srm-ink-soft)">
                        {c.reason}
                      </Text>
                    </div>
                  ))}
                </Stack>
              )}
              {q.data.scenario.pricing?.warnings.map((wn) => (
                <Alert key={wn} color={wn.includes("below cost") ? "red" : "yellow"} mt="sm" py={6}>
                  <Text size="xs">{wn}</Text>
                </Alert>
              ))}
            </Paper>
          </Grid.Col>
        </Grid>
      )}
      <MarketEvidence serviceId={sid} position={w.position ?? q.data?.base.pricing?.position ?? "standard"} targetPrice={q.data?.scenario.pricing?.targets?.target_price ?? null} />

      <Modal opened={saving} onClose={() => setSaving(false)} title={`Save a price for ${service.input.name}`}>
        {sc && (
          <SavePrice
            sc={sc}
            note={note}
            setNote={setNote}
            below={!!below}
            loading={save.isPending}
            onSave={(price) =>
              save.mutate({ service: { ...service.input, price, price_note: note } }, { onSuccess: () => (setSaving(false), setW(emptyWhatIf(sid))) })
            }
          />
        )}
      </Modal>
    </Stack>
  );
}

function SavePrice({
  sc,
  note,
  setNote,
  below,
  loading,
  onSave,
}: {
  sc: NonNullable<Estimate["pricing"]>;
  note: string;
  setNote: (n: string) => void;
  below: boolean;
  loading: boolean;
  onSave: (price: string) => void;
}) {
  const [price, setPrice] = useState(sc.chosen_price ?? sc.targets?.target_rounded ?? "");
  return (
    <Stack>
      <DecimalInput label="Price before tax" unit="$" value={price} onChange={setPrice} w={200} />
      {sc.targets && (
        <Group gap="xs">
          <Button size="compact-xs" variant="light" onClick={() => setPrice(sc.targets!.target_rounded)}>
            Use rounded target ({money(sc.targets.target_rounded)})
          </Button>
        </Group>
      )}
      {below && price === (sc.chosen_price ?? "") && (
        <Alert color="yellow">This price misses your target. Saving it is an explicit override, so say why.</Alert>
      )}
      <Textarea label="Price note" description={below ? "Required for a below-target price" : "Optional"} value={note} onChange={(e) => setNote(e.currentTarget.value)} />
      <Group justify="flex-end">
        <Button loading={loading} disabled={!price || (below && price === (sc.chosen_price ?? "") && !note.trim())} onClick={() => onSave(price)}>
          Save price
        </Button>
      </Group>
    </Stack>
  );
}

export function PricingPage() {
  const [params, setParams] = useSearchParams();
  const list = useCmd<ServiceSummary[]>("services_list", { includeArchived: false });
  const [search, setSearch] = useState("");
  const selected = Number(params.get("service")) || list.data?.[0]?.id || 0;
  const service = useCmd<ServiceView>("service_get", { id: selected }, !!selected);
  const rows = useMemo(() => (list.data ?? []).filter((s) => s.name.toLowerCase().includes(search.toLowerCase())), [list.data, search]);
  return (
    <>
      <PageHeader
        title="Pricing"
        description="Break-even, the price your target needs, and what changes if your assumptions do. Commission and card fees are solved at each price, not estimated from a guess."
      />
      <QueryState loading={list.isLoading} error={list.error}>
        {() =>
          list.data!.length === 0 ? (
            <Text c="dimmed">Create a service first to price it.</Text>
          ) : (
            <Grid gap="lg">
              <Grid.Col span={{ base: 12, lg: 3 }}>
                <Paper p="xs">
                  <TextInput aria-label="Search services" placeholder="Search" leftSection={<IconSearch size={14} />} size="xs" mb="xs" value={search} onChange={(e) => setSearch(e.currentTarget.value)} />
                  <ScrollArea.Autosize mah="70vh">
                    {rows.map((s) => (
                      <NavLink
                        key={s.id}
                        active={s.id === selected}
                        label={s.name}
                        description={`${money(s.price)} — ${statusBadge[s.status].label}`}
                        onClick={() => setParams({ service: String(s.id) }, { replace: true })}
                        style={{ borderRadius: 8 }}
                      />
                    ))}
                  </ScrollArea.Autosize>
                </Paper>
              </Grid.Col>
              <Grid.Col span={{ base: 12, lg: 9 }}>
                <QueryState loading={service.isLoading} error={service.error}>
                  {() => <WhatIfPanel service={service.data!} />}
                </QueryState>
              </Grid.Col>
            </Grid>
          )
        }
      </QueryState>
    </>
  );
}
