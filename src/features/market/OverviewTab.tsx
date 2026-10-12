import { Alert, Badge, Group, MultiSelect, Paper, Select, Stack, Switch, Table, Text, Tooltip } from "@mantine/core";
import { useState } from "react";
import { useNavigate } from "react-router";
import { useCmd } from "../../api/queries";
import type { Competitor, OverviewQuery, OverviewRow, SourceType } from "../../api/types";
import { DecimalInput } from "../../components/DecimalInput";
import { DistanceInput } from "../../components/DistanceInput";
import { QueryState } from "../../components/QueryState";
import { useToday } from "../../lib/dates";
import { money, ordinal } from "../../lib/format";
import { categories, categoryLabel, sourceType } from "./labels";
import { qualityColor, qualityLabel } from "./MarketEvidence";

const n = (d: string | null | undefined) => (d == null ? null : Number(d));

/** Lowest–highest line, middle half box, median tick and your price, on a scale shared by all rows. */
export function RangeStrip({ row, lo, hi }: { row: OverviewRow; lo: number; hi: number }) {
  const s = row.stats;
  if (!s || hi <= lo) return null;
  const x = (v: number) => `${Math.min(100, Math.max(0, ((v - lo) / (hi - lo)) * 100))}%`;
  const [min, q1, med, q3, max] = [s.min, s.q1, s.median, s.q3, s.max].map(Number);
  const yours = n(row.your_price);
  const label = `Lowest ${money(s.min)}, middle half ${money(s.q1)} to ${money(s.q3)}, median ${money(s.median)}, highest ${money(s.max)}${yours != null ? `; yours ${money(row.your_price)}` : ""}`;
  return (
    <Tooltip label={label} multiline w={260}>
      <div role="img" aria-label={label} style={{ position: "relative", height: 18, minWidth: 140 }}>
        <div style={{ position: "absolute", top: 8, height: 2, left: x(min), width: `calc(${x(max)} - ${x(min)})`, background: "var(--mantine-color-gray-4)" }} />
        <div style={{ position: "absolute", top: 3, height: 12, left: x(q1), width: `calc(${x(q3)} - ${x(q1)})`, minWidth: 2, background: "var(--mantine-primary-color-light)", border: "1px solid var(--mantine-primary-color-filled)", borderRadius: 3 }} />
        <div style={{ position: "absolute", top: 1, height: 16, width: 2, left: x(med), background: "var(--mantine-primary-color-filled)" }} />
        {yours != null && <div style={{ position: "absolute", top: 4, width: 10, height: 10, marginLeft: -5, left: x(yours), borderRadius: 5, background: "var(--mantine-color-orange-6)", border: "2px solid var(--mantine-color-body)" }} />}
      </div>
    </Tooltip>
  );
}

export function OverviewTab() {
  const today = useToday();
  const navigate = useNavigate();
  const comps = useCmd<Competitor[]>("competitors_list", { includeArchived: false });
  const [f, setF] = useState<OverviewQuery>({ max_age_days: 540, radius_km: null, include_starting_at: false, category: null, competitor_ids: null, source_types: null, price_min: null, price_max: null });
  const [offeredOnly, setOfferedOnly] = useState(false);
  const q = useCmd<OverviewRow[]>("market_overview", { query: f, today });
  return (
    <Stack>
      <Paper p="md">
        <Group gap="sm" align="flex-end" wrap="wrap">
          <Select size="xs" label="Service type" w={210} clearable placeholder="All" data={categories} value={f.category} onChange={(v) => setF({ ...f, category: v })} />
          <DistanceInput size="xs" label="Within" w={110} minKm={0} placeholder="Any" km={f.radius_km} onChange={(km) => setF({ ...f, radius_km: km })} />
          <MultiSelect
            size="xs"
            label="Competitors"
            w={260}
            searchable
            clearable
            placeholder="All"
            data={(comps.data ?? []).filter((c) => c.observations > 0).map((c) => ({ value: String(c.input.id), label: c.input.name }))}
            value={(f.competitor_ids ?? []).map(String)}
            onChange={(v) => setF({ ...f, competitor_ids: v.length ? v.map(Number) : null })}
          />
          <MultiSelect
            size="xs"
            label="Source"
            w={230}
            clearable
            placeholder="All"
            data={(Object.keys(sourceType) as SourceType[]).map((k) => ({ value: k, label: sourceType[k].label }))}
            value={f.source_types ?? []}
            onChange={(v) => setF({ ...f, source_types: v.length ? (v as SourceType[]) : null })}
          />
          <DecimalInput size="xs" label="Price from" unit="$" w={100} value={f.price_min ?? ""} onChange={(v) => setF({ ...f, price_min: v || null })} />
          <DecimalInput size="xs" label="to" unit="$" w={100} value={f.price_max ?? ""} onChange={(v) => setF({ ...f, price_max: v || null })} />
          <Switch size="xs" label='Count "starting at" prices' checked={f.include_starting_at} onChange={(e) => setF({ ...f, include_starting_at: e.currentTarget.checked })} />
          <Switch size="xs" label="Only services I offer" checked={offeredOnly} onChange={(e) => setOfferedOnly(e.currentTarget.checked)} />
        </Group>
      </Paper>
      <QueryState loading={q.isLoading} error={q.error}>
        {() => {
          const rows = q.data!.filter((r) => !offeredOnly || r.service_id != null);
          const withStats = rows.filter((r) => r.stats);
          if (!withStats.length)
            return (
              <Alert color="gray" title="No comparable prices yet">
                Run a search on the Research tab, add competitors' websites, or record and import prices you've collected. Services appear here once prices match them.
              </Alert>
            );
          const vals = withStats.flatMap((r) => [Number(r.stats!.min), Number(r.stats!.max), ...(r.your_price ? [Number(r.your_price)] : [])]);
          const lo = Math.min(...vals) * 0.95;
          const hi = Math.max(...vals) * 1.05;
          return (
            <Paper p="md">
              <Table.ScrollContainer minWidth={980}>
                <Table fz="sm" verticalSpacing={8} highlightOnHover>
                  <Table.Thead>
                    <Table.Tr>
                      <Table.Th>Service</Table.Th>
                      <Table.Th ta="right">Yours</Table.Th>
                      <Table.Th ta="right">Compared</Table.Th>
                      <Table.Th ta="right">Low</Table.Th>
                      <Table.Th ta="right">Median</Table.Th>
                      <Table.Th ta="right">Average</Table.Th>
                      <Table.Th ta="right">High</Table.Th>
                      <Table.Th w={240}>Spread</Table.Th>
                      <Table.Th>Your position</Table.Th>
                      <Table.Th ta="right">Recommended</Table.Th>
                    </Table.Tr>
                  </Table.Thead>
                  <Table.Tbody>
                    {rows.map((r) => {
                      const s = r.stats;
                      const rec = r.recommendation;
                      return (
                        <Table.Tr key={r.service_id ?? r.canonical} style={{ cursor: r.service_id ? "pointer" : undefined }} onClick={() => r.service_id && navigate(`/market?tab=prices&service=${r.service_id}`)}>
                          <Table.Td>
                            <Text size="sm" fw={600}>
                              {r.name}
                            </Text>
                            <Group gap={4}>
                              <Text size="xs" c="dimmed">
                                {categoryLabel(r.category)}
                              </Text>
                              {r.service_id == null && (
                                <Badge size="xs" variant="outline" color="gray">
                                  Not on your menu
                                </Badge>
                              )}
                              <Badge size="xs" color={qualityColor[r.quality]}>
                                {qualityLabel[r.quality]}
                              </Badge>
                            </Group>
                          </Table.Td>
                          <Table.Td ta="right" className="srm-num">
                            {money(r.your_price)}
                          </Table.Td>
                          <Table.Td ta="right">
                            {s ? (
                              <Text size="xs">
                                {s.n} from {s.businesses} {s.businesses === 1 ? "business" : "businesses"}
                              </Text>
                            ) : (
                              <Text size="xs" c="dimmed">
                                {r.observations ? `${r.observations} recorded, none comparable` : "None yet"}
                              </Text>
                            )}
                          </Table.Td>
                          <Table.Td ta="right" className="srm-num">
                            {s ? money(s.min) : "—"}
                          </Table.Td>
                          <Table.Td ta="right" className="srm-num">
                            {s ? money(s.median) : "—"}
                          </Table.Td>
                          <Table.Td ta="right" className="srm-num">
                            {s ? money(s.mean) : "—"}
                          </Table.Td>
                          <Table.Td ta="right" className="srm-num">
                            {s ? money(s.max) : "—"}
                          </Table.Td>
                          <Table.Td>
                            <RangeStrip row={r} lo={lo} hi={hi} />
                          </Table.Td>
                          <Table.Td>
                            {r.your_percentile != null ? (
                              <Text size="xs">{r.your_percentile <= 25 ? "Lower end" : r.your_percentile >= 75 ? "Upper end" : "Middle"} ({ordinal(r.your_percentile)} percentile)</Text>
                            ) : (
                              <Text size="xs" c="dimmed">
                                —
                              </Text>
                            )}
                          </Table.Td>
                          <Table.Td ta="right">
                            {rec?.recommended ? (
                              <Tooltip label={rec.explanation} multiline w={300}>
                                <div>
                                  <Text size="sm" fw={650} className="srm-num">
                                    {money(rec.recommended)}
                                  </Text>
                                  <Text size="xs" c="dimmed">
                                    {rec.basis === "market" ? "from market prices" : "from your costs"}
                                  </Text>
                                </div>
                              </Tooltip>
                            ) : (
                              <Text size="xs" c="dimmed">
                                —
                              </Text>
                            )}
                          </Table.Td>
                        </Table.Tr>
                      );
                    })}
                  </Table.Tbody>
                </Table>
              </Table.ScrollContainer>
              <Group gap="lg" mt="sm">
                <Text size="xs" c="dimmed">
                  Spread: grey line lowest to highest, box the middle half, bar the median, <span style={{ color: "var(--mantine-color-orange-6)" }}>●</span> your price. Observed prices use one price per
                  business (its middle tier). Recommendations combine the observed price at your market position with your cost-based target; open a service to adjust.
                </Text>
              </Group>
            </Paper>
          );
        }}
      </QueryState>
    </Stack>
  );
}
