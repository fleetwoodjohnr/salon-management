import { Alert, Button, Chip, Divider, Group, Paper, Stack, Text, Title } from "@mantine/core";
import { IconAlertTriangle, IconInfoCircle } from "@tabler/icons-react";
import type { Estimate, VariantInput } from "../../api/types";
import { Breakdown } from "../../components/Breakdown";
import { hours, money, num, pct, unitMoney } from "../../lib/format";
import { unitShort } from "../../lib/units";

/** Live cost breakdown and price check for a service (used by the service editor and pricing). */
export function EstimatePanel({
  estimate,
  variants,
  selected,
  onSelect,
  onUsePrice,
}: {
  estimate: Estimate | undefined;
  variants: VariantInput[];
  selected: number[];
  onSelect: (ids: number[]) => void;
  onUsePrice?: (price: string) => void;
}) {
  const groups = [...new Set(variants.filter((v) => v.id != null).map((v) => v.group_name))];
  const c = estimate?.cost;
  const p = estimate?.pricing;
  const a = p?.at_price;
  const t = p?.targets;
  return (
    <Paper p="lg" style={{ position: "sticky", top: "calc(var(--app-shell-header-height, 52px) + 16px)" }}>
      <Title order={4}>Cost and price</Title>
      <Text size="xs" c="dimmed" mb="md">
        {estimate?.profile_name ? `Costed with the “${estimate.profile_name}” work profile. ` : ""}Updates as you edit.
      </Text>
      {groups.length > 0 && (
        <Stack gap={6} mb="md">
          {groups.map((g) => (
            <Group key={g} gap={6}>
              <Text size="xs" c="dimmed" w={90}>
                {g}
              </Text>
              <Chip.Group
                value={String(selected.find((id) => variants.find((v) => v.id === id)?.group_name === g) ?? "")}
                onChange={(v) => {
                  const others = selected.filter((id) => variants.find((x) => x.id === id)?.group_name !== g);
                  onSelect(v ? [...others, Number(v)] : others);
                }}
              >
                <Group gap={4}>
                  <Chip size="xs" value="">
                    Base
                  </Chip>
                  {variants
                    .filter((v) => v.group_name === g && v.id != null)
                    .map((v) => (
                      <Chip key={v.id} size="xs" value={String(v.id)}>
                        {v.name}
                      </Chip>
                    ))}
                </Group>
              </Chip.Group>
            </Group>
          ))}
          <Text size="xs" c="dimmed">
            Save new variants to preview them here.
          </Text>
        </Stack>
      )}
      {estimate?.cost_error && (
        <Alert color="yellow" icon={<IconAlertTriangle size={16} />}>
          {estimate.cost_error}
        </Alert>
      )}
      {c && (
        <>
          <Breakdown
            rows={[
              {
                label: "Materials",
                value: money(c.materials),
                hint: c.lines.map((l) => `${l.name}: ${num(l.qty, 3)} ${unitShort(l.unit)} = ${money(l.cost)}${l.unit_cost ? ` (${unitMoney(l.unit_cost)}/${l.base_unit})` : ""}`).join("\n") || undefined,
              },
              { label: "Estimated waste", value: money(c.waste), dim: true, hint: "Extra material lost to mixing and leftovers, shown separately from the recipe." },
              { label: "Other direct costs", value: money(c.other_direct), dim: true },
              { label: `Labor (${hours(c.working_hours)} at ${money(c.labor_rate)}/h)`, value: money(c.labor), hint: "Hands-on, setup and cleanup time × pay per billable hour. Processing time isn't labor." },
              { label: `Overhead (${hours(c.overhead_hours)} at ${money(c.overhead_rate)}/h)`, value: money(c.overhead) },
              { label: "Cost before fees", value: money(c.fixed_total), total: true },
            ]}
          />
          {c.warnings.map((w) => (
            <Alert key={w} color="yellow" variant="light" mt="xs" py={6} icon={<IconInfoCircle size={16} />}>
              <Text size="xs">{w}</Text>
            </Alert>
          ))}
        </>
      )}
      {p && (
        <>
          <Divider my="md" />
          {a ? (
            <Breakdown
              rows={[
                { label: "Price before tax", value: money(a.revenue) },
                { label: "Commission", value: money(a.commission), dim: true },
                { label: "Card processing", value: money(a.processing), dim: true, hint: "Charged on what the client pays, including tax." },
                { label: "Total estimated cost", value: money(a.total_cost) },
                { label: "Estimated profit", value: money(a.profit), total: true, tone: a.profit.startsWith("-") ? "bad" : "good" },
                { label: "Margin", value: pct(a.margin_pct), hint: "Profit ÷ price before tax." },
                { label: "Markup", value: pct(a.markup_pct), dim: true, hint: "Profit ÷ total cost." },
                { label: p.tax.resolved ? `Sales tax (${p.tax.label})` : "Sales tax", value: p.tax.resolved ? money(a.tax) : "Not resolved", dim: true },
                { label: "Client pays", value: p.tax.resolved ? money(a.customer_total) : `${money(a.customer_total)} + tax`, total: true },
              ]}
            />
          ) : (
            <Text size="sm" c="dimmed">
              Enter a price to see profit and margin.
            </Text>
          )}
          {(p.earnings_per_working_hour || p.earnings_per_occupied_hour) && (
            <Text size="xs" c="dimmed" mt="xs">
              You earn {money(p.earnings_per_working_hour)} per hands-on hour and {money(p.earnings_per_occupied_hour)} per hour of chair time (profit plus labor).
            </Text>
          )}
          <Divider my="md" />
          {t ? (
            <Stack gap={6}>
              <Group justify="space-between">
                <Text size="sm">Break-even price</Text>
                <Text size="sm" className="srm-num">
                  {money(t.break_even)}
                </Text>
              </Group>
              <Group justify="space-between">
                <Text size="sm">
                  Price for {pct(p.params.target_pct, 2)} {p.params.target_kind}
                </Text>
                <Text size="sm" className="srm-num" fw={650}>
                  {money(t.target_price)}
                </Text>
              </Group>
              <Group justify="space-between">
                <Text size="sm" c="dimmed">
                  Rounded ({p.params.rounding_mode} to ${p.params.rounding_increment})
                </Text>
                <Group gap="xs">
                  <Text size="sm" className="srm-num">
                    {money(t.target_rounded)}
                  </Text>
                  {onUsePrice && (
                    <Button size="compact-xs" variant="light" onClick={() => onUsePrice(t.target_rounded)}>
                      Use
                    </Button>
                  )}
                </Group>
              </Group>
              {t.target_rounded_misses && (
                <Text size="xs" c="var(--srm-warn)">
                  Rounding down lands below the target.
                </Text>
              )}
            </Stack>
          ) : (
            p.targets_error && <Alert color="red">{p.targets_error}</Alert>
          )}
          {p.warnings.map((w) => (
            <Alert key={w} color={w.includes("below cost") ? "red" : "yellow"} variant="light" mt="xs" py={6} icon={<IconAlertTriangle size={16} />}>
              <Text size="xs">{w}</Text>
            </Alert>
          ))}
        </>
      )}
    </Paper>
  );
}
