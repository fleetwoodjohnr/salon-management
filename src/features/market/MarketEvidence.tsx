import { BarChart } from "@mantine/charts";
import { ActionIcon, Alert, Anchor, Badge, Button, Group, MultiSelect, NumberInput, Paper, Popover, Select, SimpleGrid, Stack, Switch, Table, Text, Title, Tooltip } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconAlertTriangle, IconArrowsExchange, IconExternalLink } from "@tabler/icons-react";
import { useQueryClient } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useState } from "react";
import { useNavigate } from "react-router";
import { call, errorMessage } from "../../api/ipc";
import { useCmd } from "../../api/queries";
import type { Competitor, EvidenceQuery, MarketEvidenceView, Observation, Position, ServiceSummary, SourceType } from "../../api/types";
import { DecimalInput } from "../../components/DecimalInput";
import { DistanceInput } from "../../components/DistanceInput";
import { QueryState } from "../../components/QueryState";
import { useToday } from "../../lib/dates";
import { fmtDistance, useDistanceUnit } from "../../lib/distance";
import { dateLabel, isoToLocal, money, ordinal } from "../../lib/format";
import { positionLabels } from "../profiles/labels";
import { sourceType } from "./labels";

export const qualityColor: Record<string, string> = { none: "gray", low: "orange", medium: "blue", high: "teal" };
export const qualityLabel: Record<string, string> = { none: "No evidence", low: "Low evidence", medium: "Medium evidence", high: "High evidence" };

/** Price bins for a histogram, with the bin holding your price marked. */
function bins(prices: number[], yours: number | null) {
  if (prices.length < 3) return [];
  const lo = Math.min(...prices);
  const hi = Math.max(...prices);
  const raw = (hi - lo) / Math.min(8, Math.ceil(Math.sqrt(prices.length)) + 1) || 1;
  const width = [5, 10, 20, 25, 50, 100, 250].find((w) => w >= raw) ?? 500;
  const start = Math.floor(lo / width) * width;
  const out: { range: string; prices: number; yours: number }[] = [];
  for (let b = start; b <= hi; b += width) {
    const count = prices.filter((p) => p >= b && p < b + width).length;
    const mine = yours != null && yours >= b && yours < b + width;
    out.push({ range: `$${b}–${b + width - 1}`, prices: mine ? 0 : count, yours: mine ? count : 0 });
  }
  return out;
}

function MatchControl({ o }: { o: Observation }) {
  const qc = useQueryClient();
  const services = useCmd<ServiceSummary[]>("services_list", { includeArchived: false });
  const [opened, setOpened] = useState(false);
  async function set(v: string | null) {
    try {
      await call("observation_match", { id: o.input.id, serviceId: v && v !== "none" ? Number(v) : null });
      ["market_evidence", "market_overview", "observations_list", "dashboard_snapshot"].forEach((k) => qc.invalidateQueries({ queryKey: [k] }));
      setOpened(false);
    } catch (e) {
      notifications.show({ title: "Not saved", message: errorMessage(e), color: "red" });
    }
  }
  return (
    <Popover opened={opened} onChange={setOpened} position="bottom-end" withArrow>
      <Popover.Target>
        <Tooltip label="Match to a different service, or mark as not comparable">
          <ActionIcon size="sm" variant="subtle" aria-label="Change match" onClick={() => setOpened((x) => !x)}>
            <IconArrowsExchange size={14} />
          </ActionIcon>
        </Tooltip>
      </Popover.Target>
      <Popover.Dropdown>
        <Select
          size="xs"
          w={240}
          label="Compare with my service"
          data={[{ value: "none", label: "Not comparable to any of mine" }, ...(services.data ?? []).map((s) => ({ value: String(s.id), label: s.name }))]}
          value={o.input.service_id != null ? String(o.input.service_id) : "none"}
          onChange={set}
          comboboxProps={{ withinPortal: false }}
        />
      </Popover.Dropdown>
    </Popover>
  );
}

/** Observed local prices for a service, with the selection rules and quality made explicit. */
export function MarketEvidence({ serviceId, position, targetPrice, detailed = false }: { serviceId: number; position: Position; targetPrice: string | null; detailed?: boolean }) {
  const today = useToday();
  const navigate = useNavigate();
  const unit = useDistanceUnit();
  const comps = useCmd<Competitor[]>("competitors_list", { includeArchived: false }, detailed);
  const [rules, setRules] = useState({ max_age_days: 540, radius_km: 25 as number | null, hair_length: "", stylist_level: "", include_starting_at: false });
  const [filters, setFilters] = useState<{ source_types: SourceType[] | null; competitor_ids: number[] | null }>({ source_types: null, competitor_ids: null });
  const [adjust, setAdjust] = useState<string | null>(null);
  const query: EvidenceQuery = { service_id: serviceId, ...rules, ...filters, position, target_price: targetPrice };
  const q = useCmd<MarketEvidenceView>("market_evidence", { query, today });
  return (
    <Paper p="lg">
      <Group justify="space-between" mb="xs">
        <Title order={4}>Market evidence</Title>
        {q.data && <Badge color={qualityColor[q.data.evidence.quality]}>{qualityLabel[q.data.evidence.quality]}</Badge>}
      </Group>
      <QueryState loading={q.isLoading} error={q.error}>
        {() => {
          const e = q.data!;
          const s = e.evidence.stats;
          const included = e.evidence.decisions.filter((d) => d.included).map((d) => e.observations.find((o) => o.input.id === d.id)!);
          const prices = included.map((o) => (o.input.price_type === "range" && o.input.price_max ? (Number(o.input.price) + Number(o.input.price_max)) / 2 : Number(o.input.price)));
          const hist = detailed ? bins(prices, e.your_price ? Number(e.your_price) : null) : [];
          const rec = e.recommendation;
          const adjusted = adjust ?? rec.recommended ?? "";
          return (
            <Stack gap="sm">
              {s ? (
                <SimpleGrid cols={{ base: 3, md: 7 }}>
                  {(
                    [
                      ["Lowest", s.min],
                      ["Lower quartile", s.q1],
                      ["Median", s.median],
                      ["Average", s.mean],
                      ["Upper quartile", s.q3],
                      ["90th pct.", s.p90],
                      ["Highest", s.max],
                    ] as const
                  ).map(([l, v]) => (
                    <div key={l}>
                      <Text size="xs" c="dimmed">
                        {l}
                      </Text>
                      <Text fw={650} className="srm-num" ta="left">
                        {money(v)}
                      </Text>
                    </div>
                  ))}
                </SimpleGrid>
              ) : (
                <Text fw={600}>Insufficient observed market prices.</Text>
              )}
              {s && e.your_price && e.your_percentile != null && (
                <Text size="sm">
                  Your price {money(e.your_price)} is at the <b>{ordinal(e.your_percentile)} percentile</b> of {s.n} comparable prices from {s.businesses} {s.businesses === 1 ? "business" : "businesses"}.
                </Text>
              )}
              <Text size="sm">{e.suggestion_note}</Text>
              {e.suggested && !detailed && (
                <Text size="sm">
                  Suggested for a <b>{positionLabels[position].label.toLowerCase()}</b> position: <b>{money(e.suggested)}</b> (observed prices)
                </Text>
              )}
              {e.conflict && (
                <Alert color="orange" icon={<IconAlertTriangle size={16} />} py={8}>
                  <Text size="sm">{e.conflict}</Text>
                </Alert>
              )}
              {detailed && (
                <Paper withBorder p="md" radius="md" bg="var(--mantine-color-default-hover)">
                  <Title order={5} mb="xs">
                    Recommended price
                  </Title>
                  <SimpleGrid cols={{ base: 1, sm: 3 }} mb="xs">
                    <div>
                      <Text size="xs" c="dimmed">
                        Market price, {positionLabels[position].label.toLowerCase()} position (observed)
                      </Text>
                      <Text fw={650} className="srm-num" ta="left">
                        {money(rec.market_price)}
                      </Text>
                    </div>
                    <div>
                      <Text size="xs" c="dimmed">
                        Needed for your costs and target margin (calculated)
                      </Text>
                      <Text fw={650} className="srm-num" ta="left">
                        {money(rec.cost_target)}
                      </Text>
                    </div>
                    <div>
                      <Text size="xs" c="dimmed">
                        Recommended ({rec.basis === "market" ? "market" : rec.basis === "cost" ? "costs" : "—"})
                      </Text>
                      <Text fw={700} size="lg" className="srm-num" ta="left">
                        {money(rec.recommended)}
                      </Text>
                    </div>
                  </SimpleGrid>
                  <Text size="xs" c="dimmed" mb="sm">
                    {rec.explanation} Costs include materials at inventory cost, labor time, overhead, card fees and commission from this service's work profile.
                  </Text>
                  <Group align="flex-end" gap="sm">
                    <DecimalInput label="Try a different price" unit="$" w={150} value={adjusted} onChange={setAdjust} />
                    <Button variant="light" disabled={!adjusted} onClick={() => navigate(`/pricing?service=${serviceId}&price=${encodeURIComponent(adjusted)}`)}>
                      See margin in Pricing
                    </Button>
                  </Group>
                </Paper>
              )}
              {e.cpi_adjusted_median && (
                <Text size="xs" c="dimmed">
                  Modeled, not observed: {money(e.cpi_adjusted_median)} median after inflation adjustment. {e.cpi_note}
                </Text>
              )}
              <Stack gap={0}>
                {e.evidence.reasons.map((r) => (
                  <Text key={r} size="xs" c="dimmed">
                    {r}
                  </Text>
                ))}
              </Stack>
              {detailed && hist.length > 0 && (
                <div>
                  <Text size="xs" c="dimmed" mb={4}>
                    Distribution of comparable prices
                  </Text>
                  <BarChart
                    h={160}
                    data={hist}
                    dataKey="range"
                    type="stacked"
                    series={[
                      { name: "prices", label: "Prices", color: "var(--mantine-primary-color-filled)" },
                      { name: "yours", label: "Includes your price", color: "orange.6" },
                    ]}
                    withLegend
                    legendProps={{ verticalAlign: "bottom", height: 30 }}
                    gridAxis="y"
                    yAxisProps={{ allowDecimals: false }}
                  />
                </div>
              )}
              {detailed && (
                <>
                  <Group gap="sm" align="flex-end" wrap="wrap">
                    <NumberInput size="xs" label="Max age (days)" w={120} min={30} max={3650} value={rules.max_age_days} onChange={(v) => setRules({ ...rules, max_age_days: Number(v) || 540 })} />
                    <DistanceInput size="xs" label="Within" w={110} minKm={0} placeholder="Any" km={rules.radius_km} onChange={(km) => setRules({ ...rules, radius_km: km })} />
                    <Select
                      size="xs"
                      label="Hair length"
                      w={120}
                      data={[
                        { value: "", label: "Any" },
                        { value: "short", label: "Short" },
                        { value: "medium", label: "Medium" },
                        { value: "long", label: "Long" },
                      ]}
                      value={rules.hair_length}
                      onChange={(v) => setRules({ ...rules, hair_length: v ?? "" })}
                    />
                    <MultiSelect
                      size="xs"
                      label="Source"
                      w={220}
                      clearable
                      placeholder="All"
                      data={(Object.keys(sourceType) as SourceType[]).map((k) => ({ value: k, label: sourceType[k].label }))}
                      value={filters.source_types ?? []}
                      onChange={(v) => setFilters({ ...filters, source_types: v.length ? (v as SourceType[]) : null })}
                    />
                    <MultiSelect
                      size="xs"
                      label="Competitors"
                      w={240}
                      searchable
                      clearable
                      placeholder="All"
                      data={(comps.data ?? []).filter((c) => c.observations > 0).map((c) => ({ value: String(c.input.id), label: c.input.name }))}
                      value={(filters.competitor_ids ?? []).map(String)}
                      onChange={(v) => setFilters({ ...filters, competitor_ids: v.length ? v.map(Number) : null })}
                    />
                    <Switch size="xs" label='Count "starting at" prices' checked={rules.include_starting_at} onChange={(ev) => setRules({ ...rules, include_starting_at: ev.currentTarget.checked })} />
                  </Group>
                  {e.observations.length > 0 && (
                    <Table.ScrollContainer minWidth={820}>
                      <Table fz="xs">
                        <Table.Thead>
                          <Table.Tr>
                            <Table.Th>Business</Table.Th>
                            <Table.Th>As listed</Table.Th>
                            <Table.Th ta="right">Price</Table.Th>
                            <Table.Th>Source</Table.Th>
                            <Table.Th>Match</Table.Th>
                            <Table.Th>Used?</Table.Th>
                          </Table.Tr>
                        </Table.Thead>
                        <Table.Tbody>
                          {e.observations.map((o) => {
                            const d = e.evidence.decisions.find((x) => x.id === o.input.id);
                            const src = sourceType[o.source_type];
                            return (
                              <Table.Tr key={o.input.id} style={{ opacity: d?.included ? 1 : 0.6 }}>
                                <Table.Td>
                                  {o.competitor_name}
                                  {o.distance_km != null && (
                                    <Text span size="xs" c="dimmed">
                                      {" "}
                                      {fmtDistance(o.distance_km, unit)}
                                    </Text>
                                  )}
                                </Table.Td>
                                <Table.Td>
                                  <Tooltip label={o.raw_text || o.input.service_label} disabled={!o.raw_text} multiline w={320}>
                                    <span>{o.input.service_label}</span>
                                  </Tooltip>
                                  {(o.input.stylist_level || o.input.hair_length) && (
                                    <Text span size="xs" c="dimmed">
                                      {" "}
                                      · {[o.input.stylist_level, o.input.hair_length && `${o.input.hair_length} hair`].filter(Boolean).join(", ")}
                                    </Text>
                                  )}
                                </Table.Td>
                                <Table.Td className="srm-num">
                                  {o.input.price_type === "starting_at" ? "from " : ""}
                                  {money(o.input.price)}
                                  {o.input.price_type === "range" ? `–${money(o.input.price_max)}` : ""}
                                </Table.Td>
                                <Table.Td>
                                  <Group gap={4} wrap="nowrap">
                                    <Badge size="xs" color={src.color}>
                                      {src.label}
                                    </Badge>
                                    {o.input.source_url && (
                                      <Tooltip label={`Open ${o.input.source_url}`}>
                                        <ActionIcon size="xs" variant="subtle" aria-label="Open source page" onClick={() => openUrl(o.input.source_url)}>
                                          <IconExternalLink size={12} />
                                        </ActionIcon>
                                      </Tooltip>
                                    )}
                                  </Group>
                                  <Text size="xs" c="dimmed">
                                    {o.retrieved_at ? `Read ${isoToLocal(o.retrieved_at)}` : dateLabel(o.input.observed_on)}
                                  </Text>
                                </Table.Td>
                                <Table.Td>
                                  <Group gap={4} wrap="nowrap">
                                    {o.match_method === "auto" ? (
                                      <Tooltip label="Matched automatically by service name. Check it's comparable.">
                                        <Badge size="xs" variant="outline" color="yellow">
                                          Inferred
                                        </Badge>
                                      </Tooltip>
                                    ) : (
                                      <Badge size="xs" variant="outline" color="gray">
                                        By you
                                      </Badge>
                                    )}
                                    <MatchControl o={o} />
                                  </Group>
                                </Table.Td>
                                <Table.Td>
                                  {d?.included ? (
                                    <Badge color="teal" size="xs">
                                      Used
                                    </Badge>
                                  ) : (
                                    <Text size="xs" c="dimmed">
                                      {d?.reason}
                                    </Text>
                                  )}
                                </Table.Td>
                              </Table.Tr>
                            );
                          })}
                        </Table.Tbody>
                      </Table>
                    </Table.ScrollContainer>
                  )}
                </>
              )}
              {!detailed && (
                <Anchor size="xs" onClick={() => navigate(`/market?tab=prices&service=${serviceId}`)}>
                  {e.observations.length ? `See all ${e.observations.length} observations and the selection rules` : "Collect local prices on the Market page"}
                </Anchor>
              )}
            </Stack>
          );
        }}
      </QueryState>
    </Paper>
  );
}
