import { AreaChart, BarChart } from "@mantine/charts";
import { Anchor, Badge, Group, ScrollArea, Stack, Table, Text } from "@mantine/core";
import { useNavigate } from "react-router";
import { useCmd } from "../../api/queries";
import type { GroupRow, Report, ReportFilter, Snapshot, Summary, WidgetKind } from "../../api/types";
import { Breakdown } from "../../components/Breakdown";
import { dateLabel, minus, minutesLabel, money, num, pct } from "../../lib/format";
import { dateTimeLabel } from "../../lib/time";
import { unitShort } from "../../lib/units";
import { statusBadge } from "../services/ServicesPage";

export interface WidgetMeta {
  kind: WidgetKind;
  title: string;
  definition: string;
  w: number;
  h: number;
  /** Uses the date/dimension filters (false = a point-in-time widget). */
  filtered: boolean;
}

export const WIDGETS: WidgetMeta[] = [
  { kind: "kpi_revenue", title: "Net revenue", w: 3, h: 2, filtered: true, definition: "Service and retail sales after discounts, before tax, minus refunds issued in the period. Tips and sales tax are not revenue." },
  { kind: "kpi_profit", title: "Gross profit", w: 3, h: 2, filtered: true, definition: "Net revenue minus materials used, retail cost of goods, commissions and card fees. Margin is gross profit ÷ net revenue." },
  { kind: "money_in", title: "Money in, by kind", w: 6, h: 4, filtered: true, definition: "Each kind of money shown separately: services, retail, discounts given, refunds, tips (passed to staff) and sales tax collected (owed to the state)." },
  { kind: "trend", title: "Revenue and profit by week", w: 6, h: 5, filtered: true, definition: "Weekly net revenue, gross profit and operating result (gross profit minus recorded expenses)." },
  { kind: "operating", title: "Period result", w: 6, h: 3, filtered: true, definition: "Gross profit minus the expenses you recorded for the period. Allocated overhead isn't subtracted here, so overhead is never counted twice." },
  { kind: "by_service", title: "Profitability by service", w: 8, h: 5, filtered: true, definition: "Contribution = revenue − refunds − materials − commission − card fees − allocated labor and overhead (the work-profile estimates frozen on each sale)." },
  { kind: "by_profile", title: "Profitability by work profile", w: 6, h: 4, filtered: true, definition: "Same contribution measure, grouped by the work profile each sale was costed with." },
  { kind: "by_staff", title: "Profitability by staff", w: 6, h: 4, filtered: true, definition: "Same contribution measure, grouped by who performed or sold each line." },
  { kind: "costs", title: "Stock bought vs stock used", w: 4, h: 4, filtered: true, definition: "Cash spent on inventory purchases is not the same as the cost of stock actually used in services and sold. Waste and count adjustments are shown separately." },
  { kind: "inventory", title: "Inventory value", w: 4, h: 3, filtered: false, definition: "Current on-hand stock valued at moving average cost." },
  { kind: "low_stock", title: "Low stock", w: 4, h: 4, filtered: false, definition: "Products at or below their reorder point." },
  { kind: "estimate_vs_actual", title: "Estimated vs actual", w: 6, h: 4, filtered: true, definition: "Recipe-planned material cost vs the cost of what was actually used, and planned vs recorded chair time (only where actual time was entered)." },
  { kind: "upcoming", title: "Upcoming appointments", w: 4, h: 5, filtered: false, definition: "Booked and checked-in appointments in the next 7 days." },
  { kind: "pricing_alerts", title: "Pricing alerts", w: 4, h: 4, filtered: false, definition: "Services priced below cost or below your profit target at today's costs, or with no price." },
  { kind: "market", title: "Your prices vs the local market", w: 6, h: 4, filtered: false, definition: "Your price next to the median and middle half of comparable observed local prices. Only services with observations appear." },
];

export function metaFor(kind: WidgetKind): WidgetMeta {
  return WIDGETS.find((w) => w.kind === kind)!;
}

export const REPORT_GROUPS = ["service", "profile", "staff"];

export function useReport(filter: ReportFilter, compare: boolean) {
  return useCmd<Report>("report_run", { filter, groups: REPORT_GROUPS, trendBy: "week", compare });
}

function Big({ value, sub }: { value: string; sub?: string }) {
  return (
    <Stack gap={2}>
      <Text className="srm-figure" fz={34}>
        {value}
      </Text>
      {sub && (
        <Text size="xs" c="dimmed">
          {sub}
        </Text>
      )}
    </Stack>
  );
}

function prior(r: Report, pick: (s: Summary) => string | null) {
  return r.prior ? `Previous period (${dateLabel(r.prior_from)} – ${dateLabel(r.prior_to)}): ${money(pick(r.prior))}` : undefined;
}

function GroupTable({ rows, onRow }: { rows: GroupRow[]; onRow?: (g: GroupRow) => void }) {
  if (!rows.length) return <Text size="sm" c="dimmed">No finalized sales in this period.</Text>;
  return (
    <ScrollArea h="100%" type="auto">
      <Table fz="xs" highlightOnHover stickyHeader>
        <Table.Thead>
          <Table.Tr>
            <Table.Th />
            <Table.Th ta="right">Qty</Table.Th>
            <Table.Th ta="right">Revenue</Table.Th>
            <Table.Th ta="right">Materials</Table.Th>
            <Table.Th ta="right">Labor + overhead</Table.Th>
            <Table.Th ta="right">Contribution</Table.Th>
            <Table.Th ta="right">Margin</Table.Th>
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {rows.map((g) => (
            <Table.Tr key={g.key} style={{ cursor: onRow ? "pointer" : undefined }} onClick={() => onRow?.(g)}>
              <Table.Td>{g.label}</Table.Td>
              <Table.Td className="srm-num">{num(g.qty)}</Table.Td>
              <Table.Td className="srm-num">{money(g.revenue)}</Table.Td>
              <Table.Td className="srm-num">{money(g.materials)}</Table.Td>
              <Table.Td className="srm-num">{money(g.allocated)}</Table.Td>
              <Table.Td className="srm-num" c={g.contribution.startsWith("-") ? "var(--srm-bad)" : undefined}>
                {money(g.contribution)}
              </Table.Td>
              <Table.Td className="srm-num">{pct(g.margin_pct)}</Table.Td>
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
    </ScrollArea>
  );
}

export function WidgetBody({
  kind,
  report,
  snapshot,
  onDrill,
}: {
  kind: WidgetKind;
  report: Report | undefined;
  snapshot: Snapshot | undefined;
  onDrill: (extra: Partial<ReportFilter>, title: string) => void;
}) {
  const navigate = useNavigate();
  const s = report?.summary;
  switch (kind) {
    case "kpi_revenue":
      return s ? <Big value={money(s.net_revenue)} sub={prior(report!, (x) => x.net_revenue) ?? `${s.sales_count} sale${s.sales_count === 1 ? "" : "s"}, average ticket ${money(s.average_ticket)}`} /> : null;
    case "kpi_profit":
      return s ? <Big value={money(s.gross_profit)} sub={`Margin ${pct(s.gross_margin_pct)}${report!.prior ? `. ${prior(report!, (x) => x.gross_profit)}` : ""}`} /> : null;
    case "money_in":
      return s ? (
        <Breakdown
          rows={[
            { label: "Services", value: money(s.service_sales) },
            { label: "Retail", value: money(s.retail_sales) },
            { label: "Refunds", value: minus(s.refunds) },
            { label: "Net revenue", value: money(s.net_revenue), total: true },
            { label: "Discounts given (already deducted)", value: money(s.discounts), dim: true },
            { label: "Tips (for staff, not revenue)", value: money(s.tips), dim: true },
            { label: "Sales tax collected (owed to the state)", value: money(s.tax_collected), dim: true },
          ]}
        />
      ) : null;
    case "operating":
      return s ? (
        <Breakdown
          rows={[
            { label: "Gross profit", value: money(s.gross_profit) },
            { label: "Recorded expenses", value: minus(s.expenses) },
            { label: "Period result", value: money(s.operating_result), total: true, tone: s.operating_result.startsWith("-") ? "bad" : "good" },
          ]}
        />
      ) : null;
    case "trend":
      return report ? (
        report.trend.length === 0 ? (
          <Text size="sm" c="dimmed">
            Nothing recorded in this period yet.
          </Text>
        ) : (
          (() => {
            const data = report.trend.map((p) => ({ period: p.period, "Net revenue": Number(p.net_revenue), "Gross profit": Number(p.gross_profit), "Period result": Number(p.operating_result) }));
            const series = [
              { name: "Net revenue", color: "mulberry.6" },
              { name: "Gross profit", color: "teal.6" },
              { name: "Period result", color: "yellow.7" },
            ];
            const fmt = (v: number) => money(v.toFixed(2), 0);
            // Bars read better than a line until there are a few weeks to connect.
            return data.length < 4 ? (
              <BarChart h="100%" data={data} dataKey="period" series={series} withLegend legendProps={{ verticalAlign: "bottom" }} valueFormatter={fmt} gridAxis="y" />
            ) : (
              <AreaChart h="100%" data={data} dataKey="period" series={series} curveType="monotone" withLegend legendProps={{ verticalAlign: "bottom" }} valueFormatter={fmt} gridAxis="y" />
            );
          })()
        )
      ) : null;
    case "by_service":
      return report ? <GroupTable rows={report.groups.service ?? []} onRow={(g) => g.key.startsWith("s") && onDrill({ service_id: Number(g.key.slice(1)) }, g.label)} /> : null;
    case "by_profile":
      return report ? <GroupTable rows={report.groups.profile ?? []} onRow={(g) => g.key !== "none" && onDrill({ profile_id: Number(g.key) }, g.label)} /> : null;
    case "by_staff":
      return report ? <GroupTable rows={report.groups.staff ?? []} onRow={(g) => g.key !== "none" && onDrill({ staff_id: Number(g.key) }, g.label)} /> : null;
    case "costs":
      return s ? (
        <Breakdown
          rows={[
            { label: "Inventory purchases (cash spent)", value: money(s.inventory_purchases) },
            { label: "Materials used in services", value: money(s.materials_used) },
            { label: "Retail cost of goods sold", value: money(s.retail_cogs) },
            { label: "Waste and count adjustments", value: money(s.waste_and_adjustments), tone: s.waste_and_adjustments.startsWith("-") || s.waste_and_adjustments === "0" ? undefined : "warn" },
          ]}
        />
      ) : null;
    case "estimate_vs_actual":
      return s ? (
        <Breakdown
          rows={[
            { label: "Planned material cost (recipes)", value: money(s.planned_materials) },
            { label: "Actual material cost (used)", value: money(s.materials_used), tone: Number(s.materials_used) > Number(s.planned_materials) * 1.1 ? "warn" : undefined },
            { label: "Lines with actual time recorded", value: String(s.lines_with_actual_time), dim: true },
            { label: "Planned chair time (those lines)", value: minutesLabel(s.planned_minutes) },
            { label: "Actual chair time (those lines)", value: minutesLabel(s.actual_minutes) },
          ]}
        />
      ) : null;
    case "inventory":
      return snapshot ? <Big value={money(snapshot.inventory_value)} sub={`${snapshot.products} products, ${snapshot.low_stock.length} at or below reorder point`} /> : null;
    case "low_stock":
      return snapshot ? (
        snapshot.low_stock.length === 0 ? (
          <Text size="sm" c="dimmed">
            Nothing is low.
          </Text>
        ) : (
          <Stack gap={4}>
            {snapshot.low_stock.map((r) => (
              <Group key={r.product_id} justify="space-between">
                <Anchor size="sm" onClick={() => navigate(`/inventory/products/${r.product_id}`)}>
                  {r.name}
                </Anchor>
                <Text size="sm" className="srm-num">
                  {num(r.on_hand)} {unitShort(r.stock_unit)}
                </Text>
              </Group>
            ))}
          </Stack>
        )
      ) : null;
    case "upcoming":
      return snapshot ? (
        snapshot.upcoming.length === 0 ? (
          <Text size="sm" c="dimmed">
            No appointments in the next 7 days.
          </Text>
        ) : (
          <ScrollArea h="100%">
            <Stack gap={6}>
              {snapshot.upcoming.map((a) => (
                <div key={a.id} style={{ borderLeft: `3px solid var(--mantine-color-${a.staff_color}-6)`, paddingLeft: 8 }}>
                  <Text size="sm" fw={600}>
                    {dateTimeLabel(a.starts_at)}
                  </Text>
                  <Text size="xs" c="dimmed">
                    {a.client_name || "Walk-in"}, {a.service_names.join(", ")} with {a.staff_name}
                  </Text>
                </div>
              ))}
            </Stack>
          </ScrollArea>
        )
      ) : null;
    case "pricing_alerts":
      return snapshot ? (
        snapshot.pricing_alerts.length === 0 ? (
          <Text size="sm" c="dimmed">
            Every priced service meets its target.
          </Text>
        ) : (
          <Stack gap={4}>
            {snapshot.pricing_alerts.map((sv) => (
              <Group key={sv.id} justify="space-between" wrap="nowrap">
                <Anchor size="sm" onClick={() => navigate(`/pricing?service=${sv.id}`)} truncate>
                  {sv.name}
                </Anchor>
                <Badge color={statusBadge[sv.status].color}>{statusBadge[sv.status].label}</Badge>
              </Group>
            ))}
          </Stack>
        )
      ) : null;
    case "market":
      return snapshot ? (
        snapshot.market.length === 0 ? (
          <Text size="sm" c="dimmed">
            No observed competitor prices yet. Add them on the Market page.
          </Text>
        ) : (
          <Table fz="xs">
            <Table.Thead>
              <Table.Tr>
                <Table.Th>Service</Table.Th>
                <Table.Th ta="right">Yours</Table.Th>
                <Table.Th ta="right">Market median</Table.Th>
                <Table.Th ta="right">Middle half</Table.Th>
                <Table.Th>Evidence</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {snapshot.market.map((m) => (
                <Table.Tr key={m.service_id}>
                  <Table.Td>{m.service_name}</Table.Td>
                  <Table.Td className="srm-num">{money(m.your_price)}</Table.Td>
                  <Table.Td className="srm-num">{money(m.median)}</Table.Td>
                  <Table.Td className="srm-num">
                    {money(m.q1, 0)}–{money(m.q3, 0)}
                  </Table.Td>
                  <Table.Td>
                    <Badge color={m.quality === "high" ? "teal" : m.quality === "medium" ? "blue" : "gray"}>
                      {m.quality} ({m.n})
                    </Badge>
                  </Table.Td>
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
        )
      ) : null;
  }
}
