import { Badge, Button, Group, Modal, Select, Stack, Text, TextInput, Textarea } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconDownload } from "@tabler/icons-react";
import { save } from "@tauri-apps/plugin-dialog";
import { DataTable } from "mantine-datatable";
import { useState } from "react";
import { call, errorMessage } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { LedgerFilter, LedgerRow, ProductRow } from "../../api/types";
import { QueryState } from "../../components/QueryState";
import { useToday } from "../../lib/dates";
import { dateLabel, money, num } from "../../lib/format";
import { unitShort } from "../../lib/units";
import { ledgerKindLabels, reversibleKinds } from "./labels";

export function LedgerTable({ rows, showProduct = true }: { rows: LedgerRow[]; showProduct?: boolean }) {
  const today = useToday();
  const [reversing, setReversing] = useState<LedgerRow | null>(null);
  const [note, setNote] = useState("");
  const [date, setDate] = useState(today);
  const rev = useAction<{ id: number; date: string; note: string }>("ledger_reverse", {
    invalidate: ["ledger_list", "product_get", "products_list", "reorder_list"],
    success: "Entry reversed",
  });
  return (
    <>
      <DataTable
        withTableBorder
        borderRadius="lg"
        records={rows}
        idAccessor="id"
        minHeight={rows.length ? undefined : 140}
        noRecordsText="No stock movements yet"
        columns={[
          { accessor: "occurred_on", title: "Date", render: (r) => dateLabel(r.occurred_on) },
          ...(showProduct ? [{ accessor: "product_name", title: "Product" }] : []),
          {
            accessor: "kind",
            title: "Movement",
            render: (r) => (
              <Group gap={6}>
                <Text size="sm">{ledgerKindLabels[r.kind] ?? r.kind}</Text>
                {r.reversed_by && <Badge color="gray">Reversed</Badge>}
              </Group>
            ),
          },
          { accessor: "storage_name", title: "Storage", render: (r) => r.storage_name ?? "—" },
          {
            accessor: "qty",
            title: "Quantity",
            textAlign: "right",
            render: (r) => (r.qty_base === "0" ? "—" : `${r.qty.startsWith("-") ? "" : "+"}${num(r.qty, 3)} ${unitShort(r.stock_unit)}`),
          },
          { accessor: "value", title: "Value", textAlign: "right", render: (r) => (r.value === "0" ? "—" : money(r.value)) },
          { accessor: "qty_after", title: "On hand after", textAlign: "right", render: (r) => `${num(r.qty_after, 3)} ${unitShort(r.stock_unit)}` },
          { accessor: "note", title: "Note", render: (r) => <Text size="xs" c="dimmed" lineClamp={2}>{r.note}</Text> },
          {
            accessor: "actions",
            title: "",
            render: (r) =>
              reversibleKinds.has(r.kind) && !r.reversed_by ? (
                <Button size="compact-xs" variant="subtle" onClick={() => (setReversing(r), setNote(""), setDate(today))}>
                  Reverse
                </Button>
              ) : null,
          },
        ]}
      />
      <Modal opened={!!reversing} onClose={() => setReversing(null)} title="Reverse this stock movement?">
        {reversing && (
          <Stack>
            <Text size="sm">
              A correcting entry is added; the original stays in the ledger. {ledgerKindLabels[reversing.kind]} of {num(reversing.qty, 3)} {unitShort(reversing.stock_unit)}{" "}
              on {dateLabel(reversing.occurred_on)}.
            </Text>
            <TextInput type="date" label="Date of correction" value={date} onChange={(e) => setDate(e.currentTarget.value)} w={200} />
            <Textarea label="Reason" required value={note} onChange={(e) => setNote(e.currentTarget.value)} />
            <Group justify="flex-end">
              <Button variant="default" onClick={() => setReversing(null)}>
                Cancel
              </Button>
              <Button
                color="red"
                disabled={!note.trim()}
                loading={rev.isPending}
                onClick={() => rev.mutate({ id: reversing.id, date, note }, { onSuccess: () => setReversing(null) })}
              >
                Reverse entry
              </Button>
            </Group>
          </Stack>
        )}
      </Modal>
    </>
  );
}

export function LedgerTab() {
  const products = useCmd<ProductRow[]>("products_list", { includeArchived: true });
  const [filter, setFilter] = useState<LedgerFilter>({ product_id: null, kind: null, from: null, to: null, limit: 1000 });
  const q = useCmd<LedgerRow[]>("ledger_list", { filter });
  async function exportCsv() {
    const path = await save({ title: "Export stock ledger", defaultPath: "stock-ledger.csv", filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path) return;
    try {
      const n = await call<number>("ledger_export", { path, filter: { ...filter, limit: null } });
      notifications.show({ message: `Exported ${n} entries`, color: "teal" });
    } catch (e) {
      notifications.show({ title: "Export failed", message: errorMessage(e), color: "red" });
    }
  }
  return (
    <Stack gap="sm">
      <Group justify="space-between" align="flex-end">
        <Group gap="sm" align="flex-end">
          <Select
            label="Product"
            placeholder="All products"
            searchable
            clearable
            w={260}
            data={(products.data ?? []).map((p) => ({ value: String(p.id), label: [p.brand, p.name].filter(Boolean).join(" ") }))}
            value={filter.product_id ? String(filter.product_id) : null}
            onChange={(v) => setFilter({ ...filter, product_id: v ? Number(v) : null })}
          />
          <Select
            label="Movement"
            placeholder="All movements"
            clearable
            w={200}
            data={Object.entries(ledgerKindLabels).map(([value, label]) => ({ value, label }))}
            value={filter.kind}
            onChange={(v) => setFilter({ ...filter, kind: v })}
          />
          <TextInput type="date" label="From" value={filter.from ?? ""} onChange={(e) => setFilter({ ...filter, from: e.currentTarget.value || null })} />
          <TextInput type="date" label="To" value={filter.to ?? ""} onChange={(e) => setFilter({ ...filter, to: e.currentTarget.value || null })} />
        </Group>
        <Button size="xs" variant="default" leftSection={<IconDownload size={14} />} onClick={exportCsv}>
          Export CSV
        </Button>
      </Group>
      <QueryState loading={q.isLoading} error={q.error}>
        {() => <LedgerTable rows={q.data!} />}
      </QueryState>
      <Text size="xs" c="dimmed">
        The ledger is append-only: mistakes are fixed with correcting entries, so every past figure stays traceable. Showing up to 1,000 most recent entries;
        export includes all.
      </Text>
    </Stack>
  );
}
