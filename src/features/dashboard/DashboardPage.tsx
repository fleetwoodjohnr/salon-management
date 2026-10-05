import "react-grid-layout/css/styles.css";
import { ActionIcon, Alert, Badge, Button, Drawer, Group, Menu, Modal, Paper, Select, SimpleGrid, Stack, Table, Text, TextInput, Tooltip } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconAdjustmentsHorizontal, IconDotsVertical, IconInfoCircle, IconLayoutGridAdd, IconPencil, IconPrinter, IconX } from "@tabler/icons-react";
import { useQueryClient } from "@tanstack/react-query";
import { save } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useState } from "react";
import { GridLayout, useContainerWidth, type Layout } from "react-grid-layout";
import { Navigate, useNavigate } from "react-router";
import { call, errorMessage } from "../../api/ipc";
import { useCmd } from "../../api/queries";
import type { Business, DashWidget, Dashboard, Drill, Report, ReportFilter, Snapshot } from "../../api/types";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { addDays, useToday } from "../../lib/dates";
import { dateLabel, money } from "../../lib/format";
import { toLocalStr } from "../../lib/time";
import { DimensionSelects, effectiveFilter, FIELD_LABELS, FilterBar, labelFor, useFilterOptions } from "../reports/filters";
import { metaFor, useReport, WIDGETS, WidgetBody } from "./widgets";

/** CSV rows for whatever a widget shows (strings straight from the backend). */
function widgetCsv(w: DashWidget, report: Report | undefined, snap: Snapshot | undefined): { headers: string[]; rows: string[][] } {
  const s = report?.summary;
  const groupRows = (k: string) => ({
    headers: ["group", "qty", "revenue", "refunds", "materials", "retail_cogs", "commission", "fees", "labor", "overhead", "other_direct", "contribution", "margin_pct"],
    rows: (report?.groups[k] ?? []).map((g) => [g.label, g.qty, g.revenue, g.refunds, g.materials, g.retail_cogs, g.commission, g.fees, g.labor, g.overhead, g.other_direct, g.contribution, g.margin_pct ?? ""]),
  });
  switch (w.kind) {
    case "by_service":
      return groupRows("service");
    case "by_profile":
      return groupRows("profile");
    case "by_staff":
      return groupRows("staff");
    case "trend":
      return { headers: ["week_starting", "net_revenue", "gross_profit", "contribution", "expenses", "period_result"], rows: (report?.trend ?? []).map((p) => [p.period, p.net_revenue, p.gross_profit, p.contribution, p.expenses, p.operating_result]) };
    case "low_stock":
      return { headers: ["product", "on_hand", "unit", "reorder_point", "suggested"], rows: (snap?.low_stock ?? []).map((r) => [r.name, r.on_hand, r.stock_unit, r.reorder_point, r.suggested_qty]) };
    case "upcoming":
      return { headers: ["starts", "ends", "client", "staff", "services"], rows: (snap?.upcoming ?? []).map((a) => [a.starts_at, a.ends_at, a.client_name ?? "", a.staff_name, a.service_names.join("; ")]) };
    case "pricing_alerts":
      return { headers: ["service", "price", "target_price", "status"], rows: (snap?.pricing_alerts ?? []).map((v) => [v.name, v.price ?? "", v.target_price ?? "", v.status]) };
    case "market":
      return { headers: ["service", "your_price", "median", "q1", "q3", "n", "quality"], rows: (snap?.market ?? []).map((m) => [m.service_name, m.your_price ?? "", m.median ?? "", m.q1 ?? "", m.q3 ?? "", String(m.n), m.quality]) };
    default:
      return { headers: ["metric", "value"], rows: s ? Object.entries(s).map(([k, v]) => [k, v == null ? "" : String(v)]) : [] };
  }
}

function WidgetCard({
  w,
  globalFilter,
  compare,
  today,
  snapshot,
  editing,
  onRemove,
  onEditFilters,
  onDrill,
}: {
  w: DashWidget;
  globalFilter: Dashboard["filters"];
  compare: boolean;
  today: string;
  snapshot: Snapshot | undefined;
  editing: boolean;
  onRemove: () => void;
  onEditFilters: () => void;
  onDrill: (f: ReportFilter, title: string) => void;
}) {
  const meta = metaFor(w.kind);
  const filter = effectiveFilter(globalFilter, w.filters, today);
  const report = useReport(filter, compare);
  const opts = useFilterOptions();
  const overrides = Object.entries(w.filters).filter(([, v]) => v != null && v !== "");
  async function exportCsv() {
    const { headers, rows } = widgetCsv(w, report.data, snapshot);
    const path = await save({ title: `Export ${meta.title}`, defaultPath: `${meta.title.toLowerCase().replace(/[^a-z0-9]+/g, "-")}.csv`, filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path) return;
    try {
      await call("export_csv", { path, headers, rows });
      notifications.show({ message: `Exported ${rows.length} rows`, color: "teal" });
    } catch (e) {
      notifications.show({ title: "Export failed", message: errorMessage(e), color: "red" });
    }
  }
  return (
    <Paper p="md" h="100%" style={{ display: "flex", flexDirection: "column", overflow: "hidden" }} className="srm-widget">
      <Group justify="space-between" wrap="nowrap" mb="xs" className={editing ? "srm-drag-handle" : undefined} style={{ cursor: editing ? "move" : undefined }}>
        <Group gap={6} wrap="nowrap" style={{ minWidth: 0 }}>
          <Text fw={650} size="sm" truncate>
            {meta.title}
          </Text>
          <Tooltip label={meta.definition} multiline w={300}>
            <IconInfoCircle size={14} style={{ flexShrink: 0, color: "var(--mantine-color-dimmed)" }} aria-label={`About ${meta.title}`} />
          </Tooltip>
          {overrides.map(([k, v]) => (
            <Badge key={k} size="xs" color="blue" variant="light" title="This widget's own filter replaces the dashboard filter for this field">
              {FIELD_LABELS[k]}: {labelFor(opts, k, v)}
            </Badge>
          ))}
        </Group>
        <Group gap={2} wrap="nowrap" className="srm-no-print">
          {editing ? (
            <ActionIcon size="sm" variant="subtle" color="gray" aria-label={`Remove ${meta.title}`} onClick={onRemove}>
              <IconX size={14} />
            </ActionIcon>
          ) : (
            <Menu position="bottom-end">
              <Menu.Target>
                <ActionIcon size="sm" variant="subtle" color="gray" aria-label={`${meta.title} options`}>
                  <IconDotsVertical size={14} />
                </ActionIcon>
              </Menu.Target>
              <Menu.Dropdown>
                {meta.filtered && <Menu.Item onClick={() => onDrill(filter, meta.title)}>Show underlying sales</Menu.Item>}
                {meta.filtered && <Menu.Item onClick={onEditFilters}>Widget filters…</Menu.Item>}
                <Menu.Item onClick={exportCsv}>Export CSV</Menu.Item>
              </Menu.Dropdown>
            </Menu>
          )}
        </Group>
      </Group>
      <div style={{ flex: 1, minHeight: 0, overflow: "auto" }}>
        {meta.filtered && report.error ? (
          <Alert color="red">{errorMessage(report.error)}</Alert>
        ) : (
          <WidgetBody kind={w.kind} report={report.data} snapshot={snapshot} onDrill={(extra, title) => onDrill({ ...filter, ...extra }, title)} />
        )}
      </div>
    </Paper>
  );
}

function DrillDrawer({ filter, title, onClose }: { filter: ReportFilter | null; title: string; onClose: () => void }) {
  const navigate = useNavigate();
  const q = useCmd<Drill>("report_drill", { filter }, !!filter);
  return (
    <Drawer opened={!!filter} onClose={onClose} position="right" size="xl" title={`Underlying records: ${title}`}>
      {filter && (
        <QueryState loading={q.isLoading} error={q.error}>
          {() => (
            <Stack>
              <Text size="xs" c="dimmed">
                {dateLabel(filter.from)} to {dateLabel(filter.to)}. Click a line to open its sale.
              </Text>
              <Table fz="xs" highlightOnHover>
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>Date</Table.Th>
                    <Table.Th>Sale</Table.Th>
                    <Table.Th>Line</Table.Th>
                    <Table.Th ta="right">Revenue</Table.Th>
                    <Table.Th ta="right">Materials</Table.Th>
                    <Table.Th ta="right">Tax</Table.Th>
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {q.data!.lines.map((l, i) => (
                    <Table.Tr key={i} style={{ cursor: "pointer" }} onClick={() => navigate(`/sales/${l.sale_id}`)}>
                      <Table.Td>{dateLabel(l.sale_date)}</Table.Td>
                      <Table.Td>{l.number}</Table.Td>
                      <Table.Td>{l.description}</Table.Td>
                      <Table.Td className="srm-num">{money(l.net)}</Table.Td>
                      <Table.Td className="srm-num">{money(l.kind === "retail" ? l.retail_cost : l.materials)}</Table.Td>
                      <Table.Td className="srm-num">{money(l.tax)}</Table.Td>
                    </Table.Tr>
                  ))}
                  {q.data!.refunds.map((r) => (
                    <Table.Tr key={`r${r.refund_id}${r.description}`} style={{ cursor: "pointer" }} onClick={() => navigate(`/sales/${r.sale_id}`)}>
                      <Table.Td>{dateLabel(r.refund_date)}</Table.Td>
                      <Table.Td>{r.number}</Table.Td>
                      <Table.Td>Refund: {r.description}</Table.Td>
                      <Table.Td className="srm-num">−{money(r.net)}</Table.Td>
                      <Table.Td />
                      <Table.Td className="srm-num">−{money(r.tax)}</Table.Td>
                    </Table.Tr>
                  ))}
                </Table.Tbody>
              </Table>
              {q.data!.lines.length === 0 && q.data!.refunds.length === 0 && (
                <Text size="sm" c="dimmed">
                  No records match.
                </Text>
              )}
            </Stack>
          )}
        </QueryState>
      )}
    </Drawer>
  );
}

function Builder({ initial, views }: { initial: Dashboard; views: Dashboard[] }) {
  const qc = useQueryClient();
  const today = useToday();
  const [dash, setDash] = useState<Dashboard>(initial);
  useEffect(() => setDash(initial), [initial]);
  const [editing, setEditing] = useState(false);
  const [filterFor, setFilterFor] = useState<DashWidget | null>(null);
  const [widgetFilters, setWidgetFilters] = useState<Partial<ReportFilter>>({});
  const [drill, setDrill] = useState<{ filter: ReportFilter; title: string } | null>(null);
  const [saveAs, setSaveAs] = useState<string | null>(null);
  const { width, containerRef, mounted } = useContainerWidth();
  const now = new Date();
  const snapshot = useCmd<Snapshot>("dashboard_snapshot", { now: toLocalStr(now).slice(0, 13) + ":00", until: `${addDays(today, 7)}T23:59` });
  const dirty = JSON.stringify(dash) !== JSON.stringify(initial);

  async function persist(d: Dashboard) {
    try {
      const id = await call<number>("dashboard_save", { dashboard: d });
      await qc.invalidateQueries({ queryKey: ["dashboards_list"] });
      notifications.show({ message: `Saved view "${d.name}"`, color: "teal" });
      return id;
    } catch (e) {
      notifications.show({ title: "Not saved", message: errorMessage(e), color: "red" });
    }
  }

  const layout: Layout = useMemo(() => dash.layout.map((w) => ({ i: w.i, x: w.x, y: w.y, w: w.w, h: w.h, minW: 2, minH: 2 })), [dash.layout]);
  const missing = WIDGETS.filter((m) => !dash.layout.some((w) => w.kind === m.kind));

  return (
    <>
      <PageHeader
        title="Dashboard"
        description="All figures come from finalized sales, refunds, expenses and the stock ledger. Hover the info icon on a widget for its definition."
        actions={
          <>
            <Select
              aria-label="Saved view"
              w={180}
              data={views.map((v) => ({ value: String(v.id), label: v.name }))}
              value={String(dash.id)}
              allowDeselect={false}
              onChange={(v) => {
                const next = views.find((x) => String(x.id) === v);
                if (next) setDash(next);
              }}
            />
            {editing ? (
              <>
                <Menu>
                  <Menu.Target>
                    <Button variant="default" leftSection={<IconLayoutGridAdd size={16} />} disabled={!missing.length}>
                      Add widget
                    </Button>
                  </Menu.Target>
                  <Menu.Dropdown>
                    {missing.map((m) => (
                      <Menu.Item key={m.kind} onClick={() => setDash({ ...dash, layout: [...dash.layout, { i: `${m.kind}-${Date.now()}`, kind: m.kind, x: 0, y: 1000, w: m.w, h: m.h, filters: {} }] })}>
                        {m.title}
                      </Menu.Item>
                    ))}
                  </Menu.Dropdown>
                </Menu>
                <Button variant="default" onClick={() => (setDash(initial), setEditing(false))}>
                  Cancel
                </Button>
                <Button variant="default" onClick={() => setSaveAs("")}>
                  Save as new view
                </Button>
                <Button onClick={async () => (await persist(dash), setEditing(false))}>Save layout</Button>
              </>
            ) : (
              <>
                <Button variant="default" leftSection={<IconPencil size={16} />} onClick={() => setEditing(true)}>
                  Edit layout
                </Button>
                <Button variant="default" leftSection={<IconPrinter size={16} />} onClick={() => window.print()}>
                  PDF
                </Button>
                {dirty && <Button onClick={() => persist(dash)}>Save filters to view</Button>}
              </>
            )}
          </>
        }
      />
      <Paper p="sm" mb="md" className="srm-no-print">
        <Group gap="xs" align="center">
          <IconAdjustmentsHorizontal size={16} />
          <FilterBar value={dash.filters} onChange={(filters) => setDash({ ...dash, filters })} today={today} />
        </Group>
        <Text size="xs" c="dimmed" mt={6}>
          These filters apply to every widget. A widget's own filter, shown as a blue tag on it, replaces the dashboard filter for that field only. Point-in-time widgets (inventory value, low stock, upcoming, pricing alerts, market) ignore dates.
        </Text>
      </Paper>
      <div ref={containerRef as React.RefObject<HTMLDivElement>} className={editing ? "srm-grid srm-grid-editing" : "srm-grid"}>
        {mounted && (
          <GridLayout
            width={width}
            layout={layout}
            gridConfig={{ cols: 12, rowHeight: 64, margin: [16, 16], containerPadding: [0, 0] }}
            dragConfig={{ enabled: editing, handle: ".srm-drag-handle" }}
            resizeConfig={{ enabled: editing, handles: ["se"] }}
            onLayoutChange={(l) => editing && setDash((d) => ({ ...d, layout: d.layout.map((w) => ({ ...w, ...(({ x, y, w: ww, h }) => ({ x, y, w: ww, h }))(l.find((x) => x.i === w.i) ?? w) })) }))}
          >
            {dash.layout.map((w) => (
              <div key={w.i}>
                <WidgetCard
                  w={w}
                  globalFilter={dash.filters}
                  compare={!!dash.filters.compare}
                  today={today}
                  snapshot={snapshot.data}
                  editing={editing}
                  onRemove={() => setDash({ ...dash, layout: dash.layout.filter((x) => x.i !== w.i) })}
                  onEditFilters={() => (setWidgetFilters(w.filters), setFilterFor(w))}
                  onDrill={(filter, title) => setDrill({ filter, title })}
                />
              </div>
            ))}
          </GridLayout>
        )}
      </div>
      <Modal opened={!!filterFor} onClose={() => setFilterFor(null)} title={filterFor ? `Filters for ${metaFor(filterFor.kind).title}` : ""} size="lg">
        <Stack>
          <Text size="sm" c="dimmed">
            Set only the fields this widget should see differently. Empty fields follow the dashboard's filters.
          </Text>
          <SimpleGrid cols={2}>
            <DimensionSelects size="sm" value={widgetFilters} onChange={setWidgetFilters} />
          </SimpleGrid>
          <Group justify="flex-end">
            <Button variant="default" onClick={() => setWidgetFilters({})}>
              Clear widget filters
            </Button>
            <Button
              onClick={() => {
                const next = { ...dash, layout: dash.layout.map((x) => (x.i === filterFor!.i ? { ...x, filters: Object.fromEntries(Object.entries(widgetFilters).filter(([, v]) => v != null && v !== "")) } : x)) };
                setDash(next);
                persist(next);
                setFilterFor(null);
              }}
            >
              Apply and save
            </Button>
          </Group>
        </Stack>
      </Modal>
      <Modal opened={saveAs !== null} onClose={() => setSaveAs(null)} title="Save as a new view">
        <Stack>
          <TextInput label="View name" value={saveAs ?? ""} onChange={(e) => setSaveAs(e.currentTarget.value)} data-autofocus />
          <Group justify="flex-end">
            <Button
              disabled={!saveAs?.trim()}
              onClick={async () => {
                const id = await persist({ ...dash, id: null, name: saveAs!, is_default: false });
                if (id) setDash({ ...dash, id, name: saveAs!, is_default: false });
                setSaveAs(null);
                setEditing(false);
              }}
            >
              Save view
            </Button>
          </Group>
        </Stack>
      </Modal>
      <DrillDrawer filter={drill?.filter ?? null} title={drill?.title ?? ""} onClose={() => setDrill(null)} />
    </>
  );
}

export function DashboardPage() {
  const biz = useCmd<Business>("business_get");
  const views = useCmd<Dashboard[]>("dashboards_list");
  if (biz.data && !biz.data.onboarding_completed && !biz.data.name) return <Navigate to="/setup" replace />;
  return (
    <QueryState loading={views.isLoading} error={views.error}>
      {() => {
        const v = views.data!;
        const d = v.find((x) => x.is_default) ?? v[0];
        return (
          <>
            {biz.data && !biz.data.onboarding_completed && (
              <Alert color="blue" mb="md" title="Finish setting up">
                <Group justify="space-between">
                  <Text size="sm">A few setup steps were skipped.</Text>
                  <Button size="xs" component="a" href="#/setup">
                    Continue setup
                  </Button>
                </Group>
              </Alert>
            )}
            <Builder key={d.id} initial={d} views={v} />
          </>
        );
      }}
    </QueryState>
  );
}
