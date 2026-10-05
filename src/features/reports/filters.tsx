import { Badge, Group, Select, Switch, TextInput } from "@mantine/core";
import { useCmd } from "../../api/queries";
import type { Dashboard, Location, ProductRow, ProfileView, ReportFilter, ServiceSummary, Staff } from "../../api/types";
import { addDays } from "../../lib/dates";

export type GlobalFilters = Dashboard["filters"];

export const PRESETS = [
  { value: "this_month", label: "This month" },
  { value: "last_month", label: "Last month" },
  { value: "last_30", label: "Last 30 days" },
  { value: "this_quarter", label: "This quarter" },
  { value: "ytd", label: "Year to date" },
  { value: "last_year", label: "Last year" },
  { value: "custom", label: "Custom dates" },
];

/** Date range for a preset, in the business's local dates. */
export function presetRange(preset: string | undefined, today: string): { from: string; to: string } {
  const [y, m] = today.split("-").map(Number);
  const pad = (n: number) => String(n).padStart(2, "0");
  const monthStart = (yy: number, mm: number) => `${yy}-${pad(mm)}-01`;
  const monthEnd = (yy: number, mm: number) => addDays(mm === 12 ? `${yy + 1}-01-01` : `${yy}-${pad(mm + 1)}-01`, -1);
  switch (preset) {
    case "last_month": {
      const [yy, mm] = m === 1 ? [y - 1, 12] : [y, m - 1];
      return { from: monthStart(yy, mm), to: monthEnd(yy, mm) };
    }
    case "last_30":
      return { from: addDays(today, -29), to: today };
    case "this_quarter": {
      const q = Math.floor((m - 1) / 3) * 3 + 1;
      return { from: monthStart(y, q), to: today };
    }
    case "ytd":
      return { from: `${y}-01-01`, to: today };
    case "last_year":
      return { from: `${y - 1}-01-01`, to: `${y - 1}-12-31` };
    default:
      return { from: monthStart(y, m), to: monthEnd(y, m) };
  }
}

/** Widget-level values replace the dashboard's for the same field (shown as chips on the widget). */
export function effectiveFilter(global: GlobalFilters, widget: Partial<ReportFilter>, today: string): ReportFilter {
  const range = global.preset === "custom" && global.from && global.to ? { from: global.from, to: global.to } : presetRange(global.preset, today);
  const base: ReportFilter = {
    from: range.from,
    to: range.to,
    location_id: global.location_id ?? null,
    profile_id: global.profile_id ?? null,
    staff_id: global.staff_id ?? null,
    service_id: global.service_id ?? null,
    category: global.category ?? null,
    product_id: global.product_id ?? null,
    status: global.status ?? null,
  };
  const out = { ...base };
  for (const [k, v] of Object.entries(widget)) {
    if (v !== undefined && v !== null && v !== "") (out as Record<string, unknown>)[k] = v;
  }
  return out;
}

const PLACEHOLDERS: Record<string, string> = {
  location_id: "All locations",
  profile_id: "All work profiles",
  staff_id: "All staff",
  service_id: "All services",
  category: "All categories",
  product_id: "All products",
  status: "All sales",
};

export const FIELD_LABELS: Record<string, string> = {
  location_id: "Location",
  profile_id: "Work profile",
  staff_id: "Staff",
  service_id: "Service",
  category: "Category",
  product_id: "Product",
  status: "Refund status",
  from: "From",
  to: "To",
};

export function useFilterOptions() {
  const locations = useCmd<Location[]>("locations_list");
  const profiles = useCmd<ProfileView[]>("profiles_list", { includeArchived: true });
  const staff = useCmd<Staff[]>("staff_list");
  const services = useCmd<ServiceSummary[]>("services_list", { includeArchived: true });
  const products = useCmd<ProductRow[]>("products_list", { includeArchived: true });
  return {
    location_id: (locations.data ?? []).map((l) => ({ value: String(l.id), label: l.name })),
    profile_id: (profiles.data ?? []).map((p) => ({ value: String(p.id), label: p.name })),
    staff_id: (staff.data ?? []).map((s) => ({ value: String(s.id), label: s.name })),
    service_id: (services.data ?? []).map((s) => ({ value: String(s.id), label: s.name })),
    category: [...new Set((services.data ?? []).map((s) => s.category).filter(Boolean))].map((c) => ({ value: c, label: c })),
    product_id: (products.data ?? []).filter((p) => p.category === "retail").map((p) => ({ value: String(p.id), label: p.name })),
    status: [
      { value: "with_refunds", label: "Sales with refunds" },
      { value: "without_refunds", label: "Sales without refunds" },
    ],
  };
}

type Opts = ReturnType<typeof useFilterOptions>;

export function labelFor(opts: Opts, field: string, value: unknown): string {
  const list = (opts as Record<string, { value: string; label: string }[]>)[field];
  return list?.find((o) => o.value === String(value))?.label ?? String(value);
}

/** Select inputs for the dimension filters (used by the global bar and the widget editor). */
export function DimensionSelects({ value, onChange, size = "xs" }: { value: Partial<ReportFilter>; onChange: (v: Partial<ReportFilter>) => void; size?: "xs" | "sm" }) {
  const opts = useFilterOptions();
  const fields = ["location_id", "profile_id", "staff_id", "service_id", "category", "product_id", "status"] as const;
  return (
    <>
      {fields.map((f) => (
        <Select
          key={f}
          size={size}
          w={size === "xs" ? 150 : undefined}
          label={size === "sm" ? FIELD_LABELS[f] : undefined}
          aria-label={FIELD_LABELS[f]}
          placeholder={PLACEHOLDERS[f]}
          clearable
          searchable
          data={opts[f]}
          value={value[f] != null ? String(value[f]) : null}
          onChange={(v) => onChange({ ...value, [f]: v == null ? null : f === "category" || f === "status" ? v : Number(v) })}
        />
      ))}
    </>
  );
}

export function FilterBar({ value, onChange, today }: { value: GlobalFilters; onChange: (v: GlobalFilters) => void; today: string }) {
  const r = value.preset === "custom" && value.from && value.to ? { from: value.from, to: value.to } : presetRange(value.preset, today);
  return (
    <Group gap="xs" align="flex-end" wrap="wrap">
      <Select size="xs" w={140} aria-label="Date range" data={PRESETS} value={value.preset ?? "this_month"} allowDeselect={false} onChange={(v) => onChange({ ...value, preset: v ?? "this_month", from: r.from, to: r.to })} />
      {value.preset === "custom" ? (
        <>
          <TextInput size="xs" type="date" aria-label="From" value={value.from ?? r.from} onChange={(e) => onChange({ ...value, from: e.currentTarget.value })} />
          <TextInput size="xs" type="date" aria-label="To" value={value.to ?? r.to} onChange={(e) => onChange({ ...value, to: e.currentTarget.value })} />
        </>
      ) : (
        <Badge variant="outline" color="gray" size="lg">
          {r.from} to {r.to}
        </Badge>
      )}
      <DimensionSelects value={value} onChange={(v) => onChange({ ...value, ...v })} />
      <Switch size="xs" label="Compare with previous period" checked={!!value.compare} onChange={(e) => onChange({ ...value, compare: e.currentTarget.checked })} />
    </Group>
  );
}
