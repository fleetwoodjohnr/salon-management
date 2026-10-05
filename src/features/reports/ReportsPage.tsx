import { Alert, Button, Grid, Group, Paper, SegmentedControl, Stack, Table, Tabs, Text, Title } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconDownload, IconPrinter } from "@tabler/icons-react";
import { save } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { call, errorMessage } from "../../api/ipc";
import { useCmd } from "../../api/queries";
import type { Dashboard, Report, TaxRow } from "../../api/types";
import { Breakdown } from "../../components/Breakdown";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { useToday } from "../../lib/dates";
import { dateLabel, minus, money, num, pct } from "../../lib/format";
import { effectiveFilter, FilterBar } from "./filters";

async function exportRows(name: string, headers: string[], rows: string[][]) {
  const path = await save({ title: `Export ${name}`, defaultPath: `${name.toLowerCase().replace(/[^a-z0-9]+/g, "-")}.csv`, filters: [{ name: "CSV", extensions: ["csv"] }] });
  if (!path) return;
  try {
    await call("export_csv", { path, headers, rows });
    notifications.show({ message: `Exported ${rows.length} rows`, color: "teal" });
  } catch (e) {
    notifications.show({ title: "Export failed", message: errorMessage(e), color: "red" });
  }
}

function Statement({ r }: { r: Report }) {
  const s = r.summary;
  return (
    <Grid gap="lg">
      <Grid.Col span={{ base: 12, lg: 6 }}>
        <Paper p="lg">
          <Title order={4} mb="xs">
            Period result (actual)
          </Title>
          <Text size="xs" c="dimmed" mb="sm">
            What the business earned after the costs that actually happened. Allocated overhead and labor aren't used here, so nothing is counted twice.
          </Text>
          <Breakdown
            rows={[
              { label: "Service sales", value: money(s.service_sales) },
              { label: "Retail sales", value: money(s.retail_sales) },
              { label: "Refunds", value: minus(s.refunds) },
              { label: "Net revenue", value: money(s.net_revenue), total: true },
              { label: "Materials used in services", value: minus(s.materials_used) },
              { label: "Retail cost of goods", value: minus(s.retail_cogs) },
              { label: "Commissions", value: minus(s.commissions) },
              { label: "Card processing fees", value: minus(s.processing_fees) },
              { label: "Gross profit", value: money(s.gross_profit), total: true },
              { label: "Recorded expenses", value: minus(s.expenses) },
              { label: "Period result", value: money(s.operating_result), total: true, tone: s.operating_result.startsWith("-") ? "bad" : "good" },
            ]}
          />
        </Paper>
      </Grid.Col>
      <Grid.Col span={{ base: 12, lg: 6 }}>
        <Stack>
          <Paper p="lg">
            <Title order={4} mb="xs">
              Service contribution (allocated)
            </Title>
            <Text size="xs" c="dimmed" mb="sm">
              Uses the labor and overhead estimates frozen on each sale from its work profile. Good for comparing services; not a substitute for the period result.
            </Text>
            <Breakdown
              rows={[
                { label: "Gross profit", value: money(s.gross_profit) },
                { label: "Allocated labor", value: minus(s.labor_allocated) },
                { label: "Allocated overhead", value: minus(s.overhead_allocated) },
                { label: "Other direct costs", value: minus(s.other_direct) },
                { label: "Service contribution", value: money(s.service_contribution), total: true },
              ]}
            />
          </Paper>
          <Paper p="lg">
            <Title order={4} mb="sm">
              Pass-through and other figures
            </Title>
            <Breakdown
              rows={[
                { label: "Discounts given", value: money(s.discounts), dim: true },
                { label: "Tips", value: money(s.tips), dim: true },
                { label: "Sales tax collected (net of refunds)", value: money(s.tax_collected), dim: true },
                { label: "Inventory purchases (cash)", value: money(s.inventory_purchases), dim: true },
                { label: "Waste and count adjustments", value: money(s.waste_and_adjustments), dim: true },
                { label: "Sales", value: String(s.sales_count), dim: true },
                { label: "Average ticket", value: money(s.average_ticket), dim: true },
              ]}
            />
          </Paper>
        </Stack>
      </Grid.Col>
    </Grid>
  );
}

function Profitability({ r }: { r: Report }) {
  const [by, setBy] = useState("service");
  const rows = r.groups[by] ?? [];
  const label: Record<string, string> = { service: "Service", profile: "Work profile", staff: "Staff", category: "Category", product: "Retail product" };
  return (
    <Paper p="lg">
      <Group justify="space-between" mb="sm">
        <SegmentedControl size="xs" value={by} onChange={setBy} data={Object.entries(label).map(([value, l]) => ({ value, label: l }))} />
        <Button
          size="xs"
          variant="default"
          leftSection={<IconDownload size={14} />}
          onClick={() =>
            exportRows(`profitability-by-${by}`, [label[by], "qty", "revenue", "refunds", "materials", "retail_cogs", "commission", "fees", "gross_profit", "labor", "overhead", "other_direct", "contribution", "margin_pct"], rows.map((g) => [g.label, g.qty, g.revenue, g.refunds, g.materials, g.retail_cogs, g.commission, g.fees, g.gross_profit, g.labor, g.overhead, g.other_direct, g.contribution, g.margin_pct ?? ""]))
          }
        >
          Export CSV
        </Button>
      </Group>
      <Table fz="sm" highlightOnHover>
        <Table.Thead>
          <Table.Tr>
            <Table.Th>{label[by]}</Table.Th>
            <Table.Th ta="right">Qty</Table.Th>
            <Table.Th ta="right">Revenue</Table.Th>
            <Table.Th ta="right">Refunds</Table.Th>
            <Table.Th ta="right">Materials + goods</Table.Th>
            <Table.Th ta="right">Commission + fees</Table.Th>
            <Table.Th ta="right">Gross profit</Table.Th>
            <Table.Th ta="right">Allocated</Table.Th>
            <Table.Th ta="right">Contribution</Table.Th>
            <Table.Th ta="right">Margin</Table.Th>
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {rows.map((g) => (
            <Table.Tr key={g.key}>
              <Table.Td>{g.label}</Table.Td>
              <Table.Td className="srm-num">{num(g.qty)}</Table.Td>
              <Table.Td className="srm-num">{money(g.revenue)}</Table.Td>
              <Table.Td className="srm-num">{money(g.refunds)}</Table.Td>
              <Table.Td className="srm-num">{money(by === "product" ? g.retail_cogs : g.materials)}</Table.Td>
              <Table.Td className="srm-num">{money(g.commission)} + {money(g.fees)}</Table.Td>
              <Table.Td className="srm-num">{money(g.gross_profit)}</Table.Td>
              <Table.Td className="srm-num">{money(g.allocated)}</Table.Td>
              <Table.Td className="srm-num">{money(g.contribution)}</Table.Td>
              <Table.Td className="srm-num">{pct(g.margin_pct)}</Table.Td>
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
      {rows.length === 0 && (
        <Text size="sm" c="dimmed" mt="sm">
          No finalized sales match.
        </Text>
      )}
    </Paper>
  );
}

function TaxReport({ filter }: { filter: ReturnType<typeof effectiveFilter> }) {
  const q = useCmd<TaxRow[]>("report_tax", { filter });
  return (
    <QueryState loading={q.isLoading} error={q.error}>
      {() => (
        <Paper p="lg">
          <Group justify="space-between" mb="xs">
            <Title order={4}>Sales tax collected</Title>
            <Button
              size="xs"
              variant="default"
              leftSection={<IconDownload size={14} />}
              onClick={() => exportRows("sales-tax", ["month", "rate", "taxable_sales", "exempt_sales", "tax_collected", "tax_refunded", "net_tax"], q.data!.map((t) => [t.month, t.rate, t.taxable_sales, t.exempt_sales, t.tax_collected, t.tax_refunded, t.net_tax]))}
            >
              Export CSV
            </Button>
          </Group>
          <Text size="xs" c="dimmed" mb="sm">
            For reference when you file. This app calculates and reports sales tax; it doesn't file returns or work out income tax. Each sale used the rate and taxability recorded when
            it was finalized.
          </Text>
          <Table fz="sm">
            <Table.Thead>
              <Table.Tr>
                <Table.Th>Month</Table.Th>
                <Table.Th>Rate</Table.Th>
                <Table.Th ta="right">Taxable sales</Table.Th>
                <Table.Th ta="right">Exempt sales</Table.Th>
                <Table.Th ta="right">Tax collected</Table.Th>
                <Table.Th ta="right">Tax refunded</Table.Th>
                <Table.Th ta="right">Net tax</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {q.data!.map((t, i) => (
                <Table.Tr key={i}>
                  <Table.Td>{t.month}</Table.Td>
                  <Table.Td>{t.rate || "—"}</Table.Td>
                  <Table.Td className="srm-num">{money(t.taxable_sales)}</Table.Td>
                  <Table.Td className="srm-num">{money(t.exempt_sales)}</Table.Td>
                  <Table.Td className="srm-num">{money(t.tax_collected)}</Table.Td>
                  <Table.Td className="srm-num">{money(t.tax_refunded)}</Table.Td>
                  <Table.Td className="srm-num">{money(t.net_tax)}</Table.Td>
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
        </Paper>
      )}
    </QueryState>
  );
}

export function ReportsPage() {
  const today = useToday();
  const [filters, setFilters] = useState<Dashboard["filters"]>({ preset: "this_month" });
  const filter = effectiveFilter(filters, {}, today);
  const q = useCmd<Report>("report_run", { filter, groups: ["service", "profile", "staff", "category", "product"], trendBy: null, compare: false });
  return (
    <>
      <PageHeader
        title="Reports"
        description="Period statements, profitability and sales tax, all drawn from finalized records."
        actions={
          <Button variant="default" leftSection={<IconPrinter size={16} />} onClick={() => window.print()}>
            Print or PDF
          </Button>
        }
      />
      <Paper p="sm" mb="md" className="srm-no-print">
        <FilterBar value={filters} onChange={setFilters} today={today} />
      </Paper>
      <Text size="sm" mb="sm">
        {dateLabel(filter.from)} to {dateLabel(filter.to)}
      </Text>
      <QueryState loading={q.isLoading} error={q.error}>
        {() => (
          <Stack>
            {q.data!.notes.map((n) => (
              <Alert key={n} color="blue" py={6}>
                <Text size="xs">{n}</Text>
              </Alert>
            ))}
            <Tabs defaultValue="statement" keepMounted={false}>
              <Tabs.List mb="md" className="srm-no-print">
                <Tabs.Tab value="statement">Statement</Tabs.Tab>
                <Tabs.Tab value="profitability">Profitability</Tabs.Tab>
                <Tabs.Tab value="tax">Sales tax</Tabs.Tab>
              </Tabs.List>
              <Tabs.Panel value="statement">
                <Statement r={q.data!} />
              </Tabs.Panel>
              <Tabs.Panel value="profitability">
                <Profitability r={q.data!} />
              </Tabs.Panel>
              <Tabs.Panel value="tax">
                <TaxReport filter={filter} />
              </Tabs.Panel>
            </Tabs>
          </Stack>
        )}
      </QueryState>
    </>
  );
}
