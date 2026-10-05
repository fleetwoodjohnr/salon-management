import { Alert, Anchor, Badge, Group, NumberInput, Paper, Select, SimpleGrid, Stack, Switch, Table, Text, Title } from "@mantine/core";
import { IconAlertTriangle } from "@tabler/icons-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useState } from "react";
import { useNavigate } from "react-router";
import { useCmd } from "../../api/queries";
import type { EvidenceQuery, MarketEvidenceView, Position } from "../../api/types";
import { QueryState } from "../../components/QueryState";
import { useToday } from "../../lib/dates";
import { dateLabel, money } from "../../lib/format";
import { positionLabels } from "../profiles/labels";

export const qualityColor: Record<string, string> = { none: "gray", low: "orange", medium: "blue", high: "teal" };
export const qualityLabel: Record<string, string> = { none: "No evidence", low: "Low evidence", medium: "Medium evidence", high: "High evidence" };

/** Observed local prices for a service, with the selection rules and quality made explicit. */
export function MarketEvidence({ serviceId, position, targetPrice, detailed = false }: { serviceId: number; position: Position; targetPrice: string | null; detailed?: boolean }) {
  const today = useToday();
  const navigate = useNavigate();
  const [rules, setRules] = useState({ max_age_days: 540, radius_km: 25 as number | null, hair_length: "", stylist_level: "", include_starting_at: false });
  const query: EvidenceQuery = { service_id: serviceId, ...rules, position, target_price: targetPrice };
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
          return (
            <Stack gap="sm">
              {s ? (
                <SimpleGrid cols={{ base: 3, md: 6 }}>
                  {(
                    [
                      ["Lowest", s.min],
                      ["Lower quartile", s.q1],
                      ["Median", s.median],
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
              <Text size="sm">{e.suggestion_note}</Text>
              {e.suggested && (
                <Text size="sm">
                  Suggested for a <b>{positionLabels[position].label.toLowerCase()}</b> position: <b>{money(e.suggested)}</b> (observed prices)
                </Text>
              )}
              {e.conflict && (
                <Alert color="orange" icon={<IconAlertTriangle size={16} />} py={8}>
                  <Text size="sm">{e.conflict}</Text>
                </Alert>
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
              {detailed && (
                <>
                  <Group gap="sm" align="flex-end">
                    <NumberInput size="xs" label="Max age (days)" w={120} min={30} max={3650} value={rules.max_age_days} onChange={(v) => setRules({ ...rules, max_age_days: Number(v) || 540 })} />
                    <NumberInput size="xs" label="Within (km)" w={110} min={0} placeholder="Any" value={rules.radius_km ?? ""} onChange={(v) => setRules({ ...rules, radius_km: v === "" ? null : Number(v) })} />
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
                    <Switch size="xs" label='Count "starting at" prices' checked={rules.include_starting_at} onChange={(ev) => setRules({ ...rules, include_starting_at: ev.currentTarget.checked })} />
                  </Group>
                  {e.observations.length > 0 && (
                  <Table fz="xs">
                    <Table.Thead>
                      <Table.Tr>
                        <Table.Th>Business</Table.Th>
                        <Table.Th>As listed</Table.Th>
                        <Table.Th ta="right">Price</Table.Th>
                        <Table.Th>Observed</Table.Th>
                        <Table.Th>Used?</Table.Th>
                      </Table.Tr>
                    </Table.Thead>
                    <Table.Tbody>
                      {e.observations.map((o) => {
                        const d = e.evidence.decisions.find((x) => x.id === o.input.id);
                        return (
                          <Table.Tr key={o.input.id} style={{ opacity: d?.included ? 1 : 0.6 }}>
                            <Table.Td>
                              {o.competitor_name}
                              {o.distance_km != null && (
                                <Text span size="xs" c="dimmed">
                                  {" "}
                                  {o.distance_km.toFixed(1)} km
                                </Text>
                              )}
                            </Table.Td>
                            <Table.Td>
                              {o.input.source_url ? <Anchor size="xs" onClick={() => openUrl(o.input.source_url)}>{o.input.service_label}</Anchor> : o.input.service_label}
                            </Table.Td>
                            <Table.Td className="srm-num">
                              {o.input.price_type === "starting_at" ? "from " : ""}
                              {money(o.input.price)}
                              {o.input.price_type === "range" ? `–${money(o.input.price_max)}` : ""}
                            </Table.Td>
                            <Table.Td>{dateLabel(o.input.observed_on)}</Table.Td>
                            <Table.Td>{d?.included ? <Badge color="teal" size="xs">Used</Badge> : <Text size="xs" c="dimmed">{d?.reason}</Text>}</Table.Td>
                          </Table.Tr>
                        );
                      })}
                    </Table.Tbody>
                  </Table>
                  )}
                </>
              )}
              {!detailed && (
                <Anchor size="xs" onClick={() => navigate(`/market?service=${serviceId}`)}>
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
