import { ActionIcon, Alert, Badge, Button, Grid, Group, Menu, Paper, Select, SimpleGrid, Stack, Table, Tabs, Text, TextInput, Textarea, Title } from "@mantine/core";
import { useHotkeys } from "@mantine/hooks";
import { IconAdjustments, IconChevronDown, IconPlus, IconTrash } from "@tabler/icons-react";
import { useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { isAppError, type AppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { Business, CustomUnit, ProductDetail, ProductInput, ProductRow, StorageLocation, Supplier, UnitDef } from "../../api/types";
import { Breakdown } from "../../components/Breakdown";
import { DecimalInput } from "../../components/DecimalInput";
import { setDirty, useDirty } from "../../components/dirty";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { money, num, unitMoney } from "../../lib/format";
import { dimensionLabel, unitShort } from "../../lib/units";
import { categoryLabels } from "./labels";
import { LedgerTable } from "./LedgerTab";
import { MovementModal, type MovementKind } from "./MovementModal";

function blank(units: "us" | "metric"): ProductInput {
  return {
    id: null,
    name: "",
    brand: "",
    category: "professional",
    subcategory: "",
    sku: "",
    barcode: "",
    supplier_id: null,
    stock_unit: units === "metric" ? "ml" : "fl_oz",
    density_g_per_ml: null,
    default_storage_id: null,
    reorder_point: null,
    reorder_qty: null,
    retail_price: null,
    notes: "",
    custom_units: [],
  };
}

function toInput(p: ProductRow): ProductInput {
  const trim = (d: string | null) => (d == null ? null : d.includes(".") ? d.replace(/0+$/, "").replace(/\.$/, "") : d);
  return {
    id: p.id,
    name: p.name,
    brand: p.brand,
    category: p.category,
    subcategory: p.subcategory,
    sku: p.sku,
    barcode: p.barcode,
    supplier_id: p.supplier_id,
    stock_unit: p.stock_unit,
    density_g_per_ml: p.density_g_per_ml,
    default_storage_id: p.default_storage_id,
    reorder_point: trim(p.reorder_point),
    reorder_qty: trim(p.reorder_qty),
    retail_price: p.retail_price,
    notes: p.notes,
    custom_units: p.custom_units,
  };
}

export function ProductEditorPage() {
  const { id } = useParams();
  const isNew = id === "new";
  const q = useCmd<ProductDetail>("product_get", { id: Number(id) }, !isNew);
  const biz = useCmd<Business>("business_get");
  if (isNew) return biz.data ? <ProductEditor detail={null} units={biz.data.units} /> : null;
  return <QueryState loading={q.isLoading} error={q.error}>{() => <ProductEditor key={q.data!.product.id} detail={q.data!} units="us" />}</QueryState>;
}

function ProductEditor({ detail, units: pref }: { detail: ProductDetail | null; units: "us" | "metric" }) {
  const navigate = useNavigate();
  const p = detail?.product ?? null;
  const [form, setForm] = useState<ProductInput>(p ? toInput(p) : blank(pref));
  const baseline = useRef(JSON.stringify(form));
  const dirty = JSON.stringify(form) !== baseline.current;
  useDirty("product-editor", dirty);
  const [error, setError] = useState<AppError | null>(null);
  const [movement, setMovement] = useState<MovementKind | null>(null);
  const unitDefs = useCmd<UnitDef[]>("units_list");
  const suppliers = useCmd<Supplier[]>("suppliers_list");
  const storage = useCmd<StorageLocation[]>("storage_list");
  const set = <K extends keyof ProductInput>(k: K, v: ProductInput[K]) => setForm((f) => ({ ...f, [k]: v }));
  const err = (f: string) => (error?.field === f ? error.message : undefined);

  const save = useAction<{ product: ProductInput }, ProductRow>("product_save", {
    invalidate: ["products_list", "product_get", "reorder_list", "services_list", "service_estimate"],
    success: (r) => `Saved ${r.name}`,
    silentError: true,
  });
  function submit() {
    setError(null);
    save.mutate(
      { product: form },
      {
        onSuccess: (r) => {
          setDirty("product-editor", false); // before navigating, so the unsaved-changes guard doesn't fire
          baseline.current = JSON.stringify(form);
          if (!p) navigate(`/inventory/products/${r.id}`, { replace: true });
        },
        onError: (e) => setError(isAppError(e) ? e : { kind: "other", message: String(e), field: null }),
      },
    );
  }
  useHotkeys([["mod+S", submit]], []);

  const stockUnit = unitDefs.data?.find((u) => u.code === form.stock_unit);
  const unitSelect = useMemo(() => (unitDefs.data ?? []).map((u) => ({ value: u.code, label: u.label, group: dimensionLabel[u.dimension] })), [unitDefs.data]);
  const grouped = ["volume", "weight", "count"].map((g) => ({ group: `Measured by ${g}`, items: unitSelect.filter((u) => u.group === g).map(({ value, label }) => ({ value, label })) }));
  const setUnit = (i: number, patch: Partial<CustomUnit>) => set("custom_units", form.custom_units.map((c, j) => (j === i ? { ...c, ...patch } : c)));

  return (
    <>
      <PageHeader
        title={p ? p.name : "New product"}
        description={p ? [p.brand, categoryLabels[p.category]].filter(Boolean).join(", ") : "Describe the product and how you measure it. Stock and cost come from purchases."}
        actions={
          <>
            {p && (
              <Menu>
                <Menu.Target>
                  <Button variant="default" leftSection={<IconAdjustments size={16} />} rightSection={<IconChevronDown size={14} />}>
                    Stock
                  </Button>
                </Menu.Target>
                <Menu.Dropdown>
                  <Menu.Item onClick={() => navigate(`/inventory/receive?product=${p.id}`)}>Receive stock</Menu.Item>
                  <Menu.Item onClick={() => setMovement("adjustment")}>Count adjustment</Menu.Item>
                  <Menu.Item onClick={() => setMovement("waste")}>Record waste</Menu.Item>
                  <Menu.Item onClick={() => setMovement("transfer")}>Transfer</Menu.Item>
                  <Menu.Item onClick={() => setMovement("supplier_return")}>Return to supplier</Menu.Item>
                </Menu.Dropdown>
              </Menu>
            )}
            <Badge color={dirty ? "yellow" : "gray"} size="lg">
              {dirty ? "Unsaved changes" : p ? "Saved" : "Not saved yet"}
            </Badge>
            <Button onClick={submit} loading={save.isPending} disabled={!dirty && !!p}>
              Save product
            </Button>
          </>
        }
      />
      {error && !error.field && (
        <Alert color="red" mb="md">
          {error.message}
        </Alert>
      )}
      <Grid gap="xl">
        <Grid.Col span={{ base: 12, lg: 7.5 }}>
          <Stack gap="lg">
            <Paper p="lg">
              <SimpleGrid cols={2}>
                <TextInput label="Name" required value={form.name} onChange={(e) => set("name", e.currentTarget.value)} error={err("name")} data-autofocus={!p} />
                <TextInput label="Brand" value={form.brand} onChange={(e) => set("brand", e.currentTarget.value)} />
                <Select
                  label="Category"
                  data={Object.entries(categoryLabels).map(([value, label]) => ({ value, label }))}
                  value={form.category}
                  allowDeselect={false}
                  onChange={(v) => set("category", (v ?? "professional") as ProductInput["category"])}
                  error={err("category")}
                />
                <TextInput label="Subcategory" placeholder="e.g. Color, developer, treatment" value={form.subcategory} onChange={(e) => set("subcategory", e.currentTarget.value)} />
                <TextInput label="SKU" value={form.sku} onChange={(e) => set("sku", e.currentTarget.value)} error={err("sku")} />
                <TextInput label="Barcode" description="Optional" value={form.barcode} onChange={(e) => set("barcode", e.currentTarget.value)} error={err("barcode")} />
                <Select
                  label="Supplier"
                  placeholder="None"
                  clearable
                  data={(suppliers.data ?? []).filter((s) => !s.archived || s.id === form.supplier_id).map((s) => ({ value: String(s.id), label: s.name }))}
                  value={form.supplier_id != null ? String(form.supplier_id) : null}
                  onChange={(v) => set("supplier_id", v ? Number(v) : null)}
                />
                <Select
                  label="Default storage spot"
                  placeholder="None"
                  clearable
                  data={(storage.data ?? []).filter((s) => !s.archived || s.id === form.default_storage_id).map((s) => ({ value: String(s.id), label: s.name }))}
                  value={form.default_storage_id != null ? String(form.default_storage_id) : null}
                  onChange={(v) => set("default_storage_id", v ? Number(v) : null)}
                />
                {form.category === "retail" && (
                  <DecimalInput label="Retail price" unit="$" value={form.retail_price ?? ""} onChange={(v) => set("retail_price", v || null)} error={err("retail_price")} />
                )}
              </SimpleGrid>
            </Paper>
            <Paper p="lg">
              <Title order={4}>Measuring</Title>
              <Text size="sm" c="dimmed" mb="md">
                The stock unit is how on-hand quantity and cost are shown. Weight ounces and fluid ounces are different units. To convert between weight and volume,
                enter the product's density from its data sheet; it's never assumed to be water.
              </Text>
              <SimpleGrid cols={3}>
                <Select label="Stock unit" data={grouped} value={form.stock_unit} allowDeselect={false} onChange={(v) => set("stock_unit", v ?? form.stock_unit)} error={err("stock_unit")} />
                <DecimalInput
                  label="Density"
                  description="Grams per mL (optional)"
                  unit="g/mL"
                  value={form.density_g_per_ml ?? ""}
                  onChange={(v) => set("density_g_per_ml", v || null)}
                  onBlur={() => form.density_g_per_ml === "0" && set("density_g_per_ml", null)}
                  error={err("density_g_per_ml")}
                />
                <div />
                <DecimalInput
                  label="Reorder point"
                  description={`Alert at or below, in ${unitShort(form.stock_unit)}`}
                  value={form.reorder_point ?? ""}
                  onChange={(v) => set("reorder_point", v || null)}
                  error={err("reorder_point")}
                />
                <DecimalInput
                  label="Reorder quantity"
                  description={`Usual order, in ${unitShort(form.stock_unit)}`}
                  value={form.reorder_qty ?? ""}
                  onChange={(v) => set("reorder_qty", v || null)}
                  error={err("reorder_qty")}
                />
              </SimpleGrid>
              <Text size="sm" fw={600} mt="lg" mb={4}>
                Custom units
              </Text>
              <Text size="xs" c="dimmed" mb="sm">
                Define units like pump, scoop or application by how much of a standard unit they hold, then use them in recipes and usage.
              </Text>
              <Stack gap="xs">
                {form.custom_units.map((c, i) => (
                  <Group key={i} gap="xs" wrap="nowrap" align="flex-start">
                    <TextInput aria-label="Custom unit name" placeholder="pump" value={c.name} onChange={(e) => setUnit(i, { name: e.currentTarget.value })} w={160} />
                    <Text size="sm" mt={8}>
                      =
                    </Text>
                    <DecimalInput aria-label="Custom unit size" value={c.qty} onChange={(v) => setUnit(i, { qty: v })} w={110} />
                    <Select
                      aria-label="Custom unit base"
                      data={(unitDefs.data ?? []).filter((u) => u.dimension === stockUnit?.dimension || (form.density_g_per_ml && u.dimension !== "count" && stockUnit?.dimension !== "count")).map((u) => ({ value: u.code, label: u.label }))}
                      value={c.unit}
                      allowDeselect={false}
                      onChange={(v) => setUnit(i, { unit: v ?? c.unit })}
                      w={220}
                    />
                    <ActionIcon variant="subtle" color="gray" mt={4} aria-label="Remove custom unit" onClick={() => set("custom_units", form.custom_units.filter((_, j) => j !== i))}>
                      <IconTrash size={16} />
                    </ActionIcon>
                  </Group>
                ))}
                {err("custom_units") || err("qty") || err("unit") ? <Text c="red" size="sm">{err("custom_units") ?? err("qty") ?? err("unit")}</Text> : null}
                <Button
                  variant="subtle"
                  size="xs"
                  w="fit-content"
                  leftSection={<IconPlus size={14} />}
                  onClick={() => set("custom_units", [...form.custom_units, { name: "", qty: "1", unit: form.stock_unit }])}
                >
                  Add custom unit
                </Button>
              </Stack>
            </Paper>
            <Paper p="lg">
              <Textarea label="Notes" autosize minRows={2} value={form.notes} onChange={(e) => set("notes", e.currentTarget.value)} />
            </Paper>
          </Stack>
        </Grid.Col>
        <Grid.Col span={{ base: 12, lg: 4.5 }}>
          {p ? (
            <Paper p="lg" style={{ position: "sticky", top: "calc(var(--app-shell-header-height, 52px) + 16px)" }}>
              <Title order={4} mb="sm">
                Stock
              </Title>
              <Text className="srm-figure" fz={34}>
                {num(p.on_hand, 3)} {unitShort(p.stock_unit)}
              </Text>
              <Text size="sm" c="dimmed" mb="md">
                on hand {p.negative && "— negative: more was used than was recorded as received"}
              </Text>
              <Breakdown
                rows={[
                  { label: "Value at average cost", value: money(p.value) },
                  { label: `Average cost per ${unitShort(p.stock_unit)}`, value: p.avg_cost ? unitMoney(p.avg_cost) : "No stock", hint: "Moving weighted average of landed purchase costs." },
                  ...detail!.by_storage.map((s) => ({ label: s.storage_name ?? "Unassigned", value: `${num(s.qty, 3)} ${unitShort(p.stock_unit)}`, dim: true })),
                ]}
              />
              {p.low_stock && (
                <Alert color="yellow" mt="md">
                  At or below the reorder point.
                </Alert>
              )}
            </Paper>
          ) : (
            <Paper p="lg">
              <Text size="sm" c="dimmed">
                Save the product, then receive stock to give it a cost. Opening stock you already have can be recorded from Receive stock as “Opening stock”.
              </Text>
            </Paper>
          )}
        </Grid.Col>
      </Grid>
      {p && (
        <Tabs defaultValue="ledger" mt="xl">
          <Tabs.List mb="sm">
            <Tabs.Tab value="ledger">Stock history</Tabs.Tab>
            <Tabs.Tab value="lots">Lots and cost history</Tabs.Tab>
          </Tabs.List>
          <Tabs.Panel value="ledger">
            <LedgerTable rows={detail!.ledger} showProduct={false} />
          </Tabs.Panel>
          <Tabs.Panel value="lots">
            <Paper p={0}>
              <Table fz="sm">
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>Received</Table.Th>
                    <Table.Th>Supplier</Table.Th>
                    <Table.Th ta="right">Quantity</Table.Th>
                    <Table.Th ta="right">Landed cost</Table.Th>
                    <Table.Th ta="right">Per {unitShort(p.stock_unit)}</Table.Th>
                    <Table.Th>Lot</Table.Th>
                    <Table.Th>Expires</Table.Th>
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {detail!.purchases.flatMap((pu) =>
                    pu.lines
                      .filter((l) => l.product_id === p.id)
                      .map((l) => (
                        <Table.Tr key={l.id} style={{ opacity: pu.reversed_at ? 0.5 : 1 }}>
                          <Table.Td>
                            {pu.purchase_date} {pu.kind === "opening_balance" && <Badge color="gray">Opening</Badge>} {pu.reversed_at && <Badge color="red">Reversed</Badge>}
                          </Table.Td>
                          <Table.Td>{pu.supplier_name ?? "—"}</Table.Td>
                          <Table.Td className="srm-num">
                            {num(l.qty_stock, 3)} {unitShort(l.stock_unit)}
                          </Table.Td>
                          <Table.Td className="srm-num">{money(l.landed_cost)}</Table.Td>
                          <Table.Td className="srm-num">{unitMoney(l.unit_cost)}</Table.Td>
                          <Table.Td>{l.lot_code || "—"}</Table.Td>
                          <Table.Td>{l.expires_on ?? "—"}</Table.Td>
                        </Table.Tr>
                      )),
                  )}
                </Table.Tbody>
              </Table>
            </Paper>
          </Tabs.Panel>
        </Tabs>
      )}
      {p && <MovementModal product={p} kind={movement} onClose={() => setMovement(null)} />}
    </>
  );
}
