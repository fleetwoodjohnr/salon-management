import { Badge, Button, Group, Menu, Modal, Paper, SegmentedControl, Select, Stack, Switch, Tabs, Text, TextInput } from "@mantine/core";
import { useDebouncedValue, useHotkeys } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import { IconBox, IconChevronDown, IconDownload, IconFileImport, IconPackageImport, IconPlus, IconSearch } from "@tabler/icons-react";
import { save } from "@tauri-apps/plugin-dialog";
import { DataTable } from "mantine-datatable";
import { useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { call, errorMessage } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { ProductCategory, ProductRow, StorageLocation, Supplier } from "../../api/types";
import { DecimalInput } from "../../components/DecimalInput";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { money, num, unitMoney } from "../../lib/format";
import { unitShort } from "../../lib/units";
import { EmptyState } from "@mantine/core";
import { ImportModal } from "./ImportModal";
import { LedgerTab } from "./LedgerTab";
import { PurchasesTab } from "./PurchasesTab";
import { ReorderTab } from "./ReorderTab";
import { SuppliersTab } from "./SuppliersTab";
import { categoryLabels } from "./labels";

type BulkField = "category" | "supplier" | "storage" | "reorder" | null;

function BulkEdit({ ids, onDone }: { ids: number[]; onDone: () => void }) {
  const [field, setField] = useState<BulkField>(null);
  const [value, setValue] = useState<string | null>(null);
  const suppliers = useCmd<Supplier[]>("suppliers_list");
  const storage = useCmd<StorageLocation[]>("storage_list");
  const bulk = useAction<{ ids: number[]; patch: Record<string, unknown> }, number>("products_bulk_update", {
    invalidate: ["products_list", "product_get", "reorder_list"],
    success: (n) => `Updated ${n} product${n === 1 ? "" : "s"}`,
  });
  const apply = (patch: Record<string, unknown>) => bulk.mutate({ ids, patch }, { onSuccess: () => (setField(null), setValue(null), onDone()) });
  return (
    <>
      <Menu>
        <Menu.Target>
          <Button variant="light" size="xs" rightSection={<IconChevronDown size={14} />}>
            Edit {ids.length} selected
          </Button>
        </Menu.Target>
        <Menu.Dropdown>
          <Menu.Item onClick={() => setField("category")}>Set category</Menu.Item>
          <Menu.Item onClick={() => setField("supplier")}>Set supplier</Menu.Item>
          <Menu.Item onClick={() => setField("storage")}>Set storage spot</Menu.Item>
          <Menu.Item onClick={() => setField("reorder")}>Set reorder point</Menu.Item>
          <Menu.Divider />
          <Menu.Item onClick={() => apply({ archived: true })}>Archive</Menu.Item>
          <Menu.Item onClick={() => apply({ archived: false })}>Restore from archive</Menu.Item>
        </Menu.Dropdown>
      </Menu>
      <Modal opened={field !== null} onClose={() => setField(null)} title={`Edit ${ids.length} products`}>
        <Stack>
          {field === "category" && (
            <Select label="Category" data={Object.entries(categoryLabels).map(([v, l]) => ({ value: v, label: l }))} value={value} onChange={setValue} />
          )}
          {field === "supplier" && (
            <Select
              label="Supplier"
              placeholder="No supplier"
              clearable
              data={(suppliers.data ?? []).filter((s) => !s.archived).map((s) => ({ value: String(s.id), label: s.name }))}
              value={value}
              onChange={setValue}
            />
          )}
          {field === "storage" && (
            <Select
              label="Default storage spot"
              placeholder="None"
              clearable
              data={(storage.data ?? []).filter((s) => !s.archived).map((s) => ({ value: String(s.id), label: s.name }))}
              value={value}
              onChange={setValue}
            />
          )}
          {field === "reorder" && (
            <DecimalInput label="Reorder point" description="In each product's own stock unit. Leave empty to remove." value={value ?? ""} onChange={setValue} />
          )}
          <Group justify="flex-end">
            <Button
              loading={bulk.isPending}
              onClick={() =>
                apply(
                  field === "category"
                    ? { category: value }
                    : field === "supplier"
                      ? { supplier_id: value ? Number(value) : null }
                      : field === "storage"
                        ? { default_storage_id: value ? Number(value) : null }
                        : { reorder_point: value ? value : null },
                )
              }
              disabled={field === "category" && !value}
            >
              Apply to {ids.length}
            </Button>
          </Group>
        </Stack>
      </Modal>
    </>
  );
}

function ProductsTab() {
  const navigate = useNavigate();
  const [archived, setArchived] = useState(false);
  const q = useCmd<ProductRow[]>("products_list", { includeArchived: archived });
  const [search, setSearch] = useState("");
  const [debounced] = useDebouncedValue(search, 150);
  const [cat, setCat] = useState<"all" | ProductCategory>("all");
  const [lowOnly, setLowOnly] = useState(false);
  const [selected, setSelected] = useState<ProductRow[]>([]);
  const [importing, setImporting] = useState(false);

  const rows = useMemo(() => {
    const s = debounced.trim().toLowerCase();
    return (q.data ?? []).filter(
      (p) =>
        (cat === "all" || p.category === cat) &&
        (!lowOnly || p.low_stock || p.negative) &&
        (!s || [p.name, p.brand, p.sku, p.barcode, p.subcategory, p.supplier_name ?? ""].some((v) => v.toLowerCase().includes(s))),
    );
  }, [q.data, debounced, cat, lowOnly]);

  async function exportCsv() {
    const path = await save({ title: "Export products", defaultPath: "products.csv", filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path) return;
    try {
      const n = await call<number>("products_export", { path });
      notifications.show({ message: `Exported ${n} products`, color: "teal" });
    } catch (e) {
      notifications.show({ title: "Export failed", message: errorMessage(e), color: "red" });
    }
  }

  return (
    <QueryState loading={q.isLoading} error={q.error}>
      {() =>
        q.data!.length === 0 && !archived ? (
          <EmptyState icon={<IconBox size={32} />} title="No products yet" description="Add the color, developer, treatments and disposables you use, or import a list from a spreadsheet." mt="xl">
            <Group mt="md" justify="center">
              <Button onClick={() => navigate("/inventory/products/new")}>Add a product</Button>
              <Button variant="default" leftSection={<IconFileImport size={16} />} onClick={() => setImporting(true)}>
                Import CSV
              </Button>
            </Group>
            <ImportModal opened={importing} onClose={() => setImporting(false)} />
          </EmptyState>
        ) : (
          <Stack gap="sm">
            <Group justify="space-between" wrap="nowrap">
              <Group gap="sm" wrap="nowrap">
                <TextInput
                  aria-label="Search products"
                  placeholder="Search name, brand, SKU, barcode…"
                  leftSection={<IconSearch size={16} />}
                  value={search}
                  onChange={(e) => setSearch(e.currentTarget.value)}
                  w={300}
                />
                <SegmentedControl
                  size="xs"
                  value={cat}
                  onChange={(v) => setCat(v as typeof cat)}
                  data={[
                    { value: "all", label: "All" },
                    { value: "professional", label: "Professional" },
                    { value: "consumable", label: "Consumables" },
                    { value: "retail", label: "Retail" },
                  ]}
                />
                <Switch size="xs" label="Low stock only" checked={lowOnly} onChange={(e) => setLowOnly(e.currentTarget.checked)} />
                <Switch size="xs" label="Archived" checked={archived} onChange={(e) => setArchived(e.currentTarget.checked)} />
              </Group>
              <Group gap="xs" wrap="nowrap">
                {selected.length > 0 && <BulkEdit ids={selected.map((s) => s.id)} onDone={() => setSelected([])} />}
                <Button size="xs" variant="default" leftSection={<IconFileImport size={14} />} onClick={() => setImporting(true)}>
                  Import
                </Button>
                <Button size="xs" variant="default" leftSection={<IconDownload size={14} />} onClick={exportCsv}>
                  Export
                </Button>
              </Group>
            </Group>
            <DataTable
              withTableBorder
              borderRadius="lg"
              highlightOnHover
              minHeight={rows.length ? undefined : 160}
              noRecordsText="No products match"
              records={rows}
              idAccessor="id"
              selectedRecords={selected}
              onSelectedRecordsChange={setSelected}
              onRowClick={({ record }) => navigate(`/inventory/products/${record.id}`)}
              columns={[
                {
                  accessor: "name",
                  title: "Product",
                  render: (p) => (
                    <div>
                      <Text size="sm" fw={600}>
                        {p.name}
                      </Text>
                      <Text size="xs" c="dimmed">
                        {[p.brand, p.sku && `SKU ${p.sku}`].filter(Boolean).join(", ") || " "}
                      </Text>
                    </div>
                  ),
                },
                { accessor: "category", render: (p) => <Text size="sm">{categoryLabels[p.category]}</Text> },
                {
                  accessor: "on_hand",
                  title: "On hand",
                  textAlign: "right",
                  render: (p) => (
                    <Text size="sm" c={p.negative ? "var(--srm-bad)" : undefined}>
                      {num(p.on_hand, 2)} {unitShort(p.stock_unit)}
                    </Text>
                  ),
                },
                {
                  accessor: "avg_cost",
                  title: "Avg cost",
                  textAlign: "right",
                  render: (p) => (p.avg_cost ? `${unitMoney(p.avg_cost)} / ${unitShort(p.stock_unit)}` : "—"),
                },
                { accessor: "value", title: "Value", textAlign: "right", render: (p) => money(p.value) },
                {
                  accessor: "status",
                  title: "",
                  render: (p) =>
                    p.negative ? (
                      <Badge color="red">Negative</Badge>
                    ) : p.low_stock ? (
                      <Badge color="yellow">Low</Badge>
                    ) : p.archived ? (
                      <Badge color="gray">Archived</Badge>
                    ) : null,
                },
              ]}
            />
            <Text size="xs" c="dimmed" ta="right">
              {rows.length} product{rows.length === 1 ? "" : "s"} shown. Values use moving average cost; see the dashboard for total inventory value.
            </Text>
            <ImportModal opened={importing} onClose={() => setImporting(false)} />
          </Stack>
        )
      }
    </QueryState>
  );
}

export function InventoryPage() {
  const navigate = useNavigate();
  const [params, setParams] = useSearchParams();
  const tab = params.get("tab") ?? "products";
  useHotkeys([["mod+N", () => navigate("/inventory/products/new")]]);
  return (
    <>
      <PageHeader
        title="Inventory"
        description="Bulk supplies, consumables and retail stock, valued at moving average cost. Every change is kept in the stock ledger."
        actions={
          <>
            <Button variant="default" leftSection={<IconPlus size={16} />} onClick={() => navigate("/inventory/products/new")}>
              New product
            </Button>
            <Button leftSection={<IconPackageImport size={16} />} onClick={() => navigate("/inventory/receive")}>
              Receive stock
            </Button>
          </>
        }
      />
      <Tabs value={tab} onChange={(v) => setParams({ tab: v ?? "products" }, { replace: true })} keepMounted={false}>
        <Tabs.List mb="md">
          <Tabs.Tab value="products">Products</Tabs.Tab>
          <Tabs.Tab value="purchases">Purchases</Tabs.Tab>
          <Tabs.Tab value="ledger">Stock ledger</Tabs.Tab>
          <Tabs.Tab value="reorder">Reorder and expiry</Tabs.Tab>
          <Tabs.Tab value="suppliers">Suppliers and storage</Tabs.Tab>
        </Tabs.List>
        <Tabs.Panel value="products">
          <ProductsTab />
        </Tabs.Panel>
        <Tabs.Panel value="purchases">
          <PurchasesTab />
        </Tabs.Panel>
        <Tabs.Panel value="ledger">
          <LedgerTab />
        </Tabs.Panel>
        <Tabs.Panel value="reorder">
          <ReorderTab />
        </Tabs.Panel>
        <Tabs.Panel value="suppliers">
          <Paper p={0} bd={0} bg="transparent">
            <SuppliersTab />
          </Paper>
        </Tabs.Panel>
      </Tabs>
    </>
  );
}
