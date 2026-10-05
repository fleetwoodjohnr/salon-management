import { ActionIcon, Alert, Badge, Button, Group, Paper, SegmentedControl, Select, SimpleGrid, Stack, Text, TextInput, Textarea, Title } from "@mantine/core";
import { useDebouncedValue, useHotkeys } from "@mantine/hooks";
import { IconPaperclip, IconPlus, IconTrash } from "@tabler/icons-react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { useMemo, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { call, errorMessage, isAppError, type AppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { Attachment, ProductRow, PurchaseInput, PurchaseLineInput, PurchasePreview, StorageLocation, Supplier, UnitDef } from "../../api/types";
import { Breakdown } from "../../components/Breakdown";
import { DecimalInput } from "../../components/DecimalInput";
import { useDirty } from "../../components/dirty";
import { PageHeader } from "../../components/PageHeader";
import { useToday } from "../../lib/dates";
import { minus, money, num, unitMoney } from "../../lib/format";
import { unitOptions, unitShort } from "../../lib/units";

function blankLine(p?: ProductRow): PurchaseLineInput {
  return {
    product_id: p?.id ?? 0,
    package_count: "1",
    contents_per_package: "",
    unit: p?.stock_unit ?? "",
    line_price: "",
    storage_id: p?.default_storage_id ?? null,
    lot_code: "",
    expires_on: null,
  };
}

export function ReceivePage() {
  const navigate = useNavigate();
  const today = useToday();
  const [params] = useSearchParams();
  const products = useCmd<ProductRow[]>("products_list", { includeArchived: false });
  const suppliers = useCmd<Supplier[]>("suppliers_list");
  const storage = useCmd<StorageLocation[]>("storage_list");
  const unitDefs = useCmd<UnitDef[]>("units_list");
  const initialProduct = Number(params.get("product")) || null;

  const fresh = (): PurchaseInput => ({
    kind: "purchase",
    supplier_id: null,
    purchase_date: today,
    invoice_ref: "",
    discount: "0",
    shipping: "0",
    nonrecoverable_tax: "0",
    attachment_id: null,
    notes: "",
    lines: [blankLine()],
  });
  const [form, setForm] = useState<PurchaseInput>(fresh);
  const [seeded, setSeeded] = useState(false);
  if (!seeded && products.data) {
    setSeeded(true);
    const p = products.data.find((x) => x.id === initialProduct);
    if (p) setForm((f) => ({ ...f, supplier_id: p.supplier_id, lines: [blankLine(p)] }));
  }
  const baseline = useRef(JSON.stringify(fresh()));
  const dirty = JSON.stringify({ ...form, purchase_date: today }) !== baseline.current;
  useDirty("receive", dirty);
  const [attachment, setAttachment] = useState<Attachment | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const byId = useMemo(() => new Map((products.data ?? []).map((p) => [p.id, p])), [products.data]);

  const complete = form.lines.every((l) => l.product_id && l.contents_per_package && l.line_price !== "" && l.unit);
  const [debounced] = useDebouncedValue(form, 200);
  const preview = useQuery({
    queryKey: ["purchase_preview", debounced],
    queryFn: () => call<PurchasePreview>("purchase_preview", { purchase: debounced }),
    enabled: complete,
    placeholderData: keepPreviousData,
  });
  const previewErr = preview.error && isAppError(preview.error) ? preview.error : null;
  const err = (f: string) => [error, previewErr].find((e) => e?.field === f)?.message;

  const setLine = (i: number, patch: Partial<PurchaseLineInput>) => setForm((f) => ({ ...f, lines: f.lines.map((l, j) => (j === i ? { ...l, ...patch } : l)) }));
  const receive = useAction<{ purchase: PurchaseInput }, number>("purchase_receive", {
    invalidate: ["products_list", "product_get", "purchases_list", "ledger_list", "reorder_list", "expiring_lots", "services_list", "service_estimate"],
    success: () => `Received ${form.lines.length} line${form.lines.length === 1 ? "" : "s"}`,
    silentError: true,
  });
  function submit() {
    setError(null);
    receive.mutate(
      { purchase: form },
      {
        onSuccess: () => {
          setForm(fresh());
          setAttachment(null);
          navigate("/inventory?tab=purchases");
        },
        onError: (e) => setError(isAppError(e) ? e : { kind: "other", message: errorMessage(e), field: null }),
      },
    );
  }
  useHotkeys([["mod+S", submit]], []);

  async function attach() {
    const path = await open({ title: "Attach receipt or invoice", filters: [{ name: "Receipt", extensions: ["pdf", "png", "jpg", "jpeg", "webp", "heic", "gif", "txt"] }] });
    if (typeof path !== "string") return;
    try {
      const a = await call<Attachment>("attachment_add", { path });
      setAttachment(a);
      setForm((f) => ({ ...f, attachment_id: a.id }));
    } catch (e) {
      setError({ kind: "io", message: errorMessage(e), field: null });
    }
  }

  const productOpts = (products.data ?? []).map((p) => ({ value: String(p.id), label: [p.brand, p.name].filter(Boolean).join(" ") }));
  const storageOpts = (storage.data ?? []).filter((s) => !s.archived).map((s) => ({ value: String(s.id), label: s.name }));
  const opening = form.kind === "opening_balance";

  return (
    <>
      <PageHeader
        title="Receive stock"
        description="Record a delivery or invoice. Discounts, shipping and non-recoverable tax are spread across lines to give each product its landed cost."
        actions={
          <>
            <Badge color={dirty ? "yellow" : "gray"} size="lg">
              {dirty ? "Not saved" : "Empty"}
            </Badge>
            <Button onClick={submit} loading={receive.isPending} disabled={!complete}>
              {opening ? "Record opening stock" : "Receive stock"}
            </Button>
          </>
        }
      />
      {products.data?.length === 0 && (
        <Alert color="blue" mb="md">
          Add your products first, then come back to receive them.{" "}
          <Button size="compact-sm" variant="subtle" onClick={() => navigate("/inventory/products/new")}>
            New product
          </Button>
        </Alert>
      )}
      <Stack gap="lg">
        <Paper p="lg">
          <SimpleGrid cols={{ base: 2, lg: 4 }}>
            <div>
              <Text size="sm" fw={500} mb={4}>
                Type
              </Text>
              <SegmentedControl
                fullWidth
                value={form.kind}
                onChange={(v) => setForm({ ...form, kind: v as PurchaseInput["kind"] })}
                data={[
                  { value: "purchase", label: "Purchase" },
                  { value: "opening_balance", label: "Opening stock" },
                ]}
              />
            </div>
            <Select
              label="Supplier"
              placeholder={opening ? "Not needed" : "Choose supplier"}
              clearable
              searchable
              data={(suppliers.data ?? []).filter((s) => !s.archived).map((s) => ({ value: String(s.id), label: s.name }))}
              value={form.supplier_id != null ? String(form.supplier_id) : null}
              onChange={(v) => setForm({ ...form, supplier_id: v ? Number(v) : null })}
            />
            <TextInput type="date" label="Date" value={form.purchase_date} onChange={(e) => setForm({ ...form, purchase_date: e.currentTarget.value })} error={err("purchase_date")} />
            <TextInput label="Invoice or order reference" value={form.invoice_ref} onChange={(e) => setForm({ ...form, invoice_ref: e.currentTarget.value })} />
          </SimpleGrid>
          {opening && (
            <Text size="xs" c="dimmed" mt="sm">
              Opening stock records what you already had, at what it cost you. It sets stock and value but isn't counted as spending in this period.
            </Text>
          )}
        </Paper>

        <Paper p="lg">
          <Title order={4} mb="sm">
            Products
          </Title>
          <Stack gap="sm">
            {form.lines.map((l, i) => {
              const p = byId.get(l.product_id);
              const pv = preview.data?.lines[i];
              return (
                <div key={i} className="srm-receive-line">
                  <div className="srm-receive-main">
                    <Select
                      label={i === 0 ? "Product" : undefined}
                      aria-label={`Line ${i + 1} product`}
                      placeholder="Choose product"
                      searchable
                      data={productOpts}
                      value={l.product_id ? String(l.product_id) : null}
                      onChange={(v) => {
                        const np = byId.get(Number(v));
                        setLine(i, { product_id: Number(v) || 0, unit: np?.stock_unit ?? "", storage_id: np?.default_storage_id ?? l.storage_id });
                      }}
                    />
                    <DecimalInput label={i === 0 ? "Packages" : undefined} aria-label={`Line ${i + 1} packages`} value={l.package_count} onChange={(v) => setLine(i, { package_count: v })} error={err(`lines.${i}.package_count`)} />
                    <DecimalInput label={i === 0 ? "Each holds" : undefined} aria-label={`Line ${i + 1} contents per package`} value={l.contents_per_package} onChange={(v) => setLine(i, { contents_per_package: v })} error={err(`lines.${i}.contents_per_package`)} />
                    <Select
                      label={i === 0 ? "Unit" : undefined}
                      aria-label={`Line ${i + 1} unit`}
                      data={p ? unitOptions(unitDefs.data ?? [], p) : []}
                      value={l.unit || null}
                      allowDeselect={false}
                      onChange={(v) => setLine(i, { unit: v ?? "" })}
                      error={err(`lines.${i}.unit`)}
                      comboboxProps={{ width: 240, position: "bottom-start" }}
                    />
                    <DecimalInput label={i === 0 ? "Line price" : undefined} aria-label={`Line ${i + 1} price`} unit="$" value={l.line_price} onChange={(v) => setLine(i, { line_price: v })} error={err(`lines.${i}.line_price`)} />
                    <div className="srm-receive-landed">
                      {i === 0 && (
                        <Text size="sm" fw={500} mb={4}>
                          Landed cost
                        </Text>
                      )}
                      {pv ? (
                        <>
                          <Text size="sm" className="srm-num">
                            {money(pv.landed_cost)}
                          </Text>
                          <Text size="xs" c="dimmed" className="srm-num">
                            {num(pv.qty_stock, 3)} {unitShort(pv.stock_unit)} at {unitMoney(pv.unit_cost)}
                          </Text>
                        </>
                      ) : (
                        <Text size="sm" c="dimmed" className="srm-num">
                          —
                        </Text>
                      )}
                    </div>
                    <ActionIcon
                      variant="subtle"
                      color="gray"
                      aria-label={`Remove line ${i + 1}`}
                      disabled={form.lines.length === 1}
                      onClick={() => setForm({ ...form, lines: form.lines.filter((_, j) => j !== i) })}
                      mt={i === 0 ? 26 : 4}
                    >
                      <IconTrash size={16} />
                    </ActionIcon>
                  </div>
                  <Group gap="xs" mt={6} wrap="nowrap">
                    <Select
                      size="xs"
                      aria-label={`Line ${i + 1} storage`}
                      placeholder="Storage spot"
                      clearable
                      data={storageOpts}
                      value={l.storage_id != null ? String(l.storage_id) : null}
                      onChange={(v) => setLine(i, { storage_id: v ? Number(v) : null })}
                      w={180}
                    />
                    <TextInput size="xs" aria-label={`Line ${i + 1} lot`} placeholder="Lot code" value={l.lot_code} onChange={(e) => setLine(i, { lot_code: e.currentTarget.value })} w={140} />
                    <Text size="xs" c="dimmed">
                      Expires
                    </Text>
                    <TextInput
                      size="xs"
                      aria-label={`Line ${i + 1} expiry`}
                      type="date"
                      value={l.expires_on ?? ""}
                      onChange={(e) => setLine(i, { expires_on: e.currentTarget.value || null })}
                      error={err(`lines.${i}.expires_on`)}
                      w={150}
                    />
                  </Group>
                </div>
              );
            })}
          </Stack>
          <Button variant="subtle" size="xs" mt="xs" leftSection={<IconPlus size={14} />} onClick={() => setForm({ ...form, lines: [...form.lines, blankLine()] })}>
            Add line
          </Button>
        </Paper>

        <SimpleGrid cols={{ base: 1, lg: 2 }} spacing="lg">
          <Paper p="lg">
            <Title order={4} mb="sm">
              Invoice extras
            </Title>
            <SimpleGrid cols={3}>
              <DecimalInput label="Discount" unit="$" value={form.discount} onChange={(v) => setForm({ ...form, discount: v })} error={err("discount")} />
              <DecimalInput label="Shipping" unit="$" value={form.shipping} onChange={(v) => setForm({ ...form, shipping: v })} error={err("shipping")} />
              <DecimalInput
                label="Non-recoverable tax"
                description="Sales tax you paid and can't reclaim"
                unit="$"
                value={form.nonrecoverable_tax}
                onChange={(v) => setForm({ ...form, nonrecoverable_tax: v })}
                error={err("nonrecoverable_tax")}
              />
            </SimpleGrid>
            <Textarea mt="md" label="Notes" value={form.notes} onChange={(e) => setForm({ ...form, notes: e.currentTarget.value })} />
            <Group mt="md" gap="sm">
              <Button variant="default" size="xs" leftSection={<IconPaperclip size={14} />} onClick={attach}>
                {attachment ? "Replace receipt" : "Attach receipt"}
              </Button>
              {attachment && (
                <Text size="sm" c="dimmed">
                  {attachment.file_name}
                </Text>
              )}
            </Group>
          </Paper>
          <Paper p="lg">
            <Title order={4} mb="sm">
              Totals
            </Title>
            {preview.data && complete ? (
              <Breakdown
                rows={[
                  { label: "Lines subtotal", value: money(preview.data.subtotal) },
                  { label: "Discount", value: minus(form.discount), dim: true },
                  { label: "Shipping", value: money(form.shipping), dim: true },
                  { label: "Non-recoverable tax", value: money(form.nonrecoverable_tax), dim: true },
                  { label: "Landed total", value: money(preview.data.total), total: true },
                ]}
              />
            ) : (
              <Text size="sm" c="dimmed">
                Fill in each line's product, quantity and price to see landed costs.
              </Text>
            )}
            {previewErr && !previewErr.field?.startsWith("lines.") && previewErr.field !== "discount" && (
              <Alert color="yellow" mt="sm">
                {previewErr.message}
              </Alert>
            )}
            {error && !error.field && (
              <Alert color="red" mt="sm">
                {error.message}
              </Alert>
            )}
          </Paper>
        </SimpleGrid>
      </Stack>
    </>
  );
}
