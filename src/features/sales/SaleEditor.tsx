import {
  ActionIcon,
  Alert,
  Anchor,
  Badge,
  Button,
  Checkbox,
  Divider,
  Grid,
  Group,
  Menu,
  Modal,
  NumberInput,
  Paper,
  SegmentedControl,
  Select,
  SimpleGrid,
  Stack,
  Table,
  Text,
  Textarea,
  TextInput,
  Title,
} from "@mantine/core";
import { useDebouncedValue, useHotkeys } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import { IconAlertTriangle, IconChevronDown, IconPlus, IconPrinter, IconTrash } from "@tabler/icons-react";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { call, errorMessage, isAppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type {
  ClientRow,
  PaymentInput,
  ProductRow,
  RefundInput,
  SaleDraft,
  SaleLineInput,
  SaleView,
  ServiceSummary,
  ServiceView,
  Staff,
  UnitDef,
} from "../../api/types";
import { Breakdown } from "../../components/Breakdown";
import { DecimalInput } from "../../components/DecimalInput";
import { setDirty, useDirty } from "../../components/dirty";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { useToday } from "../../lib/dates";
import { dateLabel, isNegDec, isoToLocal, isZeroDec, minutesLabel, money, num } from "../../lib/format";
import { unitOptions, unitShort } from "../../lib/units";

const METHODS = [
  { value: "card", label: "Card" },
  { value: "cash", label: "Cash" },
  { value: "check", label: "Check" },
  { value: "transfer", label: "Transfer" },
  { value: "other", label: "Other" },
];

function blankDraft(today: string): SaleDraft {
  return { id: null, client_id: null, staff_id: null, location_id: null, appointment_id: null, sale_date: today, sale_discount: "0", notes: "", lines: [] };
}

export function SaleEditorPage() {
  const { id } = useParams();
  const today = useToday();
  const isNew = id === "new";
  const q = useCmd<SaleView>("sale_get", { id: Number(id) }, !isNew);
  if (isNew) return <SaleEditor initial={null} today={today} />;
  return <QueryState loading={q.isLoading} error={q.error}>{() => <SaleEditor key={q.data!.draft.id} initial={q.data!} today={today} />}</QueryState>;
}

function ServiceLine({ line, onChange, units, products }: { line: SaleLineInput; onChange: (l: SaleLineInput) => void; units: UnitDef[]; products: ProductRow[] }) {
  const svc = useCmd<ServiceView>("service_get", { id: line.service_id ?? 0 }, !!line.service_id);
  const byId = new Map(products.map((p) => [p.id, p]));
  const setU = (i: number, patch: Partial<SaleLineInput["usage"][number]>) => onChange({ ...line, usage: line.usage.map((u, j) => (j === i ? { ...u, ...patch } : u)) });
  return (
    <Stack gap={6} mt="xs">
      <Text size="xs" fw={600} c="dimmed">
        Products used (planned from the recipe; enter what was actually used)
      </Text>
      {line.usage.length > 0 && (
        <Table fz="sm" withRowBorders={false} verticalSpacing={2}>
          <Table.Thead>
            <Table.Tr>
              <Table.Th>Product</Table.Th>
              <Table.Th ta="right">Planned</Table.Th>
              <Table.Th>Actually used</Table.Th>
              <Table.Th>Unit</Table.Th>
              <Table.Th />
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {line.usage.map((u, i) => {
              const p = byId.get(u.product_id);
              return (
                <Table.Tr key={i}>
                  <Table.Td>
                    {!isZeroDec(u.planned_qty) ? (
                      <Text size="sm">{p ? [p.brand, p.name].filter(Boolean).join(" ") : "…"}</Text>
                    ) : (
                      <Select
                        size="xs"
                        aria-label="Product used"
                        searchable
                        data={products.filter((x) => x.category !== "retail").map((x) => ({ value: String(x.id), label: [x.brand, x.name].filter(Boolean).join(" ") }))}
                        value={u.product_id ? String(u.product_id) : null}
                        onChange={(v) => setU(i, { product_id: Number(v), unit: byId.get(Number(v))?.stock_unit ?? u.unit })}
                      />
                    )}
                  </Table.Td>
                  <Table.Td className="srm-num">
                    <Text size="xs" c="dimmed">
                      {isZeroDec(u.planned_qty) ? "—" : `${num(u.planned_qty, 3)} ${unitShort(u.unit)}`}
                    </Text>
                  </Table.Td>
                  <Table.Td>
                    <DecimalInput size="xs" aria-label={`Actual amount of ${p?.name ?? "product"}`} w={90} value={u.qty} onChange={(v) => setU(i, { qty: v })} />
                  </Table.Td>
                  <Table.Td>
                    <Select size="xs" aria-label="Usage unit" w={120} data={p ? unitOptions(units, p) : []} value={u.unit} allowDeselect={false} onChange={(v) => setU(i, { unit: v ?? u.unit })} />
                  </Table.Td>
                  <Table.Td>
                    <ActionIcon size="sm" variant="subtle" color="gray" aria-label="Remove product" onClick={() => onChange({ ...line, usage: line.usage.filter((_, j) => j !== i) })}>
                      <IconTrash size={14} />
                    </ActionIcon>
                  </Table.Td>
                </Table.Tr>
              );
            })}
          </Table.Tbody>
        </Table>
      )}
      <Group gap="sm">
        <Button size="compact-xs" variant="subtle" leftSection={<IconPlus size={12} />} onClick={() => onChange({ ...line, usage: [...line.usage, { product_id: 0, planned_qty: "0", qty: "", unit: "" }] })}>
          Add product used
        </Button>
        <NumberInput
          size="xs"
          w={150}
          aria-label="Actual chair minutes"
          placeholder={line.planned_minutes ? `Planned ${line.planned_minutes} min` : "Actual minutes"}
          suffix=" min"
          min={0}
          allowDecimal={false}
          value={line.actual_minutes ?? ""}
          onChange={(v) => onChange({ ...line, actual_minutes: v === "" ? null : Number(v) })}
        />
        {svc.data && line.planned_minutes != null && (
          <Text size="xs" c="dimmed">
            Planned chair time {minutesLabel(line.planned_minutes)}
          </Text>
        )}
      </Group>
    </Stack>
  );
}

function SaleEditor({ initial, today }: { initial: SaleView | null; today: string }) {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const [view, setView] = useState<SaleView | null>(initial);
  const [draft, setDraft] = useState<SaleDraft>(initial?.draft ?? blankDraft(today));
  const saved = useRef(JSON.stringify(initial?.draft ?? blankDraft(today)));
  const [saveState, setSaveState] = useState<"saved" | "saving" | "error" | "idle">(initial ? "saved" : "idle");
  const [saveError, setSaveError] = useState<string | null>(null);
  const finalized = view?.status === "finalized";
  const voided = view?.status === "voided";
  const editable = !finalized && !voided;
  const dirty = editable && JSON.stringify(draft) !== saved.current;
  useDirty("sale-editor", dirty);

  const clients = useCmd<ClientRow[]>("clients_list", { includeArchived: false });
  const staff = useCmd<Staff[]>("staff_list");
  const services = useCmd<ServiceSummary[]>("services_list", { includeArchived: false });
  const products = useCmd<ProductRow[]>("products_list", { includeArchived: false });
  const units = useCmd<UnitDef[]>("units_list");
  const retail = useMemo(() => (products.data ?? []).filter((p) => p.category === "retail"), [products.data]);

  // Autosave drafts: they never touch stock or money, so saving as you go is safe.
  const [debounced] = useDebouncedValue(draft, 700);
  useEffect(() => {
    if (!editable || JSON.stringify(debounced) === saved.current) return;
    if (debounced.lines.length === 0 && !debounced.id) return;
    let cancelled = false;
    setSaveState("saving");
    call<SaleView>("sale_save_draft", { sale: debounced })
      .then((v) => {
        if (cancelled) return;
        saved.current = JSON.stringify(debounced.id ? debounced : { ...debounced, id: v.draft.id });
        setView(v);
        setSaveState("saved");
        setSaveError(null);
        if (!debounced.id) {
          setDraft((d) => ({ ...d, id: v.draft.id }));
          setDirty("sale-editor", false);
          navigate(`/sales/${v.draft.id}`, { replace: true });
        }
        qc.invalidateQueries({ queryKey: ["sales_list"] });
      })
      .catch((e) => {
        if (cancelled) return;
        setSaveState("error");
        setSaveError(errorMessage(e));
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [debounced]);

  const setLine = (i: number, l: SaleLineInput) => setDraft((d) => ({ ...d, lines: d.lines.map((x, j) => (j === i ? l : x)) }));
  const removeLine = (i: number) => setDraft((d) => ({ ...d, lines: d.lines.filter((_, j) => j !== i) }));

  async function addService(serviceId: number) {
    const s = await call<ServiceView>("service_get", { id: serviceId });
    const est = await call<{ cost: { lines: { product_id: number; qty: string; unit: string }[]; time: { hands_on_min: number; processing_min: number; setup_min: number; cleanup_min: number } } | null }>(
      "service_estimate",
      { service: s.input, variantIds: [] },
    );
    const t = s.input.time;
    setDraft((d) => ({
      ...d,
      lines: [
        ...d.lines,
        {
          kind: "service",
          service_id: serviceId,
          product_id: null,
          staff_id: null,
          description: s.input.name,
          variant_ids: [],
          qty: "1",
          unit_price: s.input.price ?? "0",
          line_discount: "0",
          planned_minutes: t.hands_on_min + t.processing_min + t.setup_min + t.cleanup_min,
          actual_minutes: null,
          usage: (est.cost?.lines ?? []).map((m) => ({ product_id: m.product_id, planned_qty: m.qty, qty: m.qty, unit: m.unit })),
        },
      ],
    }));
  }

  function addRetail(productId: number) {
    const p = retail.find((x) => x.id === productId)!;
    setDraft((d) => ({
      ...d,
      lines: [...d.lines, { kind: "retail", service_id: null, product_id: p.id, staff_id: null, description: [p.brand, p.name].filter(Boolean).join(" "), variant_ids: [], qty: "1", unit_price: p.retail_price ?? "0", line_discount: "0", planned_minutes: null, actual_minutes: null, usage: [] }],
    }));
  }

  function addTip() {
    setDraft((d) => ({
      ...d,
      lines: [...d.lines, { kind: "tip", service_id: null, product_id: null, staff_id: null, description: "Tip", variant_ids: [], qty: "1", unit_price: "", line_discount: "0", planned_minutes: null, actual_minutes: null, usage: [] }],
    }));
  }

  // payments
  const [pay, setPay] = useState<PaymentInput>({ method: "card", amount: "", reference: "", paid_on: today });
  const [payErr, setPayErr] = useState<string | null>(null);
  async function refresh() {
    if (!draft.id) return;
    const v = await call<SaleView>("sale_get", { id: draft.id });
    setView(v);
    qc.invalidateQueries({ queryKey: ["sales_list"] });
    qc.invalidateQueries({ queryKey: ["appointments_list"] });
  }
  async function addPayment() {
    setPayErr(null);
    try {
      await call("payment_add", { saleId: draft.id, payment: { ...pay, amount: pay.amount || view?.balance } });
      setPay({ ...pay, amount: "", reference: "" });
      await refresh();
    } catch (e) {
      setPayErr(errorMessage(e));
    }
  }

  const [finalizing, setFinalizing] = useState(false);
  async function finalize() {
    setFinalizing(true);
    try {
      const v = await call<SaleView>("sale_finalize", { id: draft.id });
      setView(v);
      saved.current = JSON.stringify(draft);
      qc.invalidateQueries();
      notifications.show({ message: `Sale ${v.number} finalized`, color: "teal" });
    } catch (e) {
      notifications.show({ title: "Not finalized", message: errorMessage(e), color: "red", autoClose: 10000 });
    } finally {
      setFinalizing(false);
    }
  }
  useHotkeys([["mod+S", () => setDraft((d) => ({ ...d }))]], []);

  const [refundOpen, setRefundOpen] = useState(false);
  const [voidOpen, setVoidOpen] = useState(false);
  const del = useAction<{ id: number }>("sale_delete_draft", { invalidate: ["sales_list", "appointments_list"], success: "Draft deleted" });

  const t = view?.totals;
  const staffOpts = (staff.data ?? []).filter((s) => !s.archived).map((s) => ({ value: String(s.id), label: s.name }));
  const blockers = view?.blockers ?? [];

  return (
    <>
      <PageHeader
        title={view?.number ? `Sale ${view.number}` : draft.id ? "Draft sale" : "New sale"}
        description={
          finalized
            ? `Finalized ${isoToLocal(view!.finalized_at)}. Figures are locked; use a refund to correct amounts.`
            : voided
              ? `Voided: ${view!.void_reason}`
              : "Drafts save automatically and don't touch stock until you finalize."
        }
        actions={
          <>
            {editable && (
              <Badge size="lg" color={saveState === "error" ? "red" : dirty || saveState === "saving" ? "yellow" : "gray"}>
                {saveState === "error" ? "Not saved" : dirty || saveState === "saving" ? "Saving…" : draft.id ? "Draft saved" : "Empty"}
              </Badge>
            )}
            {finalized && (
              <>
                <Button variant="default" leftSection={<IconPrinter size={16} />} onClick={() => navigate(`/print/sale/${draft.id}`)}>
                  Receipt
                </Button>
                <Menu>
                  <Menu.Target>
                    <Button variant="default" rightSection={<IconChevronDown size={14} />}>
                      Correct
                    </Button>
                  </Menu.Target>
                  <Menu.Dropdown>
                    <Menu.Item onClick={() => setRefundOpen(true)}>Refund…</Menu.Item>
                    <Menu.Item color="red" onClick={() => setVoidOpen(true)} disabled={(view?.refunds.length ?? 0) > 0}>
                      Void (entered by mistake)…
                    </Menu.Item>
                  </Menu.Dropdown>
                </Menu>
              </>
            )}
            {editable && draft.id && (
              <Button variant="subtle" color="gray" onClick={() => del.mutate({ id: draft.id! }, { onSuccess: () => (setDirty("sale-editor", false), navigate("/sales")) })}>
                Delete draft
              </Button>
            )}
          </>
        }
      />
      {saveError && (
        <Alert color="red" mb="md">
          {saveError}
        </Alert>
      )}
      <Grid gap="xl">
        <Grid.Col span={{ base: 12, lg: 8 }}>
          <Stack gap="lg">
            <Paper p="lg">
              <SimpleGrid cols={3}>
                <Select
                  label="Client"
                  placeholder="Walk-in"
                  searchable
                  clearable
                  disabled={!editable}
                  data={(clients.data ?? []).map((c) => ({ value: String(c.client.id), label: `${c.client.first_name} ${c.client.last_name}`.trim() }))}
                  value={draft.client_id != null ? String(draft.client_id) : null}
                  onChange={(v) => setDraft({ ...draft, client_id: v ? Number(v) : null })}
                />
                <Select label="Staff" placeholder="Choose" disabled={!editable} data={staffOpts} value={draft.staff_id != null ? String(draft.staff_id) : null} onChange={(v) => setDraft({ ...draft, staff_id: v ? Number(v) : null })} />
                <TextInput type="date" label="Date" disabled={!editable} value={draft.sale_date} onChange={(e) => setDraft({ ...draft, sale_date: e.currentTarget.value })} />
              </SimpleGrid>
            </Paper>
            {draft.lines.map((l, i) => (
              <Paper key={i} p="lg">
                <Group justify="space-between" align="flex-start" wrap="nowrap">
                  <div style={{ flex: 1 }}>
                    <Group gap="xs">
                      <Badge color={l.kind === "service" ? "mulberry" : l.kind === "retail" ? "teal" : "yellow"}>{l.kind === "service" ? "Service" : l.kind === "retail" ? "Retail" : "Tip"}</Badge>
                      <Text fw={600}>{l.description}</Text>
                    </Group>
                    <Group gap="sm" mt="sm" align="flex-end">
                      {l.kind !== "tip" && (
                        <DecimalInput label="Qty" w={80} disabled={!editable} value={l.qty} onChange={(v) => setLine(i, { ...l, qty: v })} />
                      )}
                      <DecimalInput label={l.kind === "tip" ? "Tip amount" : "Price each"} unit="$" w={130} disabled={!editable} value={l.unit_price} onChange={(v) => setLine(i, { ...l, unit_price: v })} />
                      {l.kind !== "tip" && <DecimalInput label="Line discount" unit="$" w={130} disabled={!editable} value={l.line_discount} onChange={(v) => setLine(i, { ...l, line_discount: v })} />}
                      <Select label={l.kind === "tip" ? "For" : "By"} placeholder="Sale staff" clearable w={160} disabled={!editable} data={staffOpts} value={l.staff_id != null ? String(l.staff_id) : null} onChange={(v) => setLine(i, { ...l, staff_id: v ? Number(v) : null })} />
                      {view?.totals.lines[i] && (
                        <Text size="sm" ml="auto" className="srm-num">
                          {money(view.totals.lines[i].net)}
                          {!isZeroDec(view.totals.lines[i].tax) && (
                            <Text span size="xs" c="dimmed">
                              {" "}
                              + {money(view.totals.lines[i].tax)} tax
                            </Text>
                          )}
                        </Text>
                      )}
                    </Group>
                    {l.kind === "service" && editable && <ServiceLine line={l} onChange={(x) => setLine(i, x)} units={units.data ?? []} products={products.data ?? []} />}
                    {finalized && view?.lines[i] && l.kind !== "tip" && (
                      <Text size="xs" c="dimmed" mt="xs">
                        {l.kind === "service"
                          ? `Materials ${money(view.lines[i].materials_cost)} (planned ${money(view.lines[i].planned_materials_cost)}), labor ${money(view.lines[i].labor_cost)}, overhead ${money(view.lines[i].overhead_cost)}, commission ${money(view.lines[i].commission)}`
                          : `Cost of goods ${money(view.lines[i].retail_cost)}, commission ${money(view.lines[i].commission)}`}
                        {view.lines[i].taxability && `. Tax: ${view.lines[i].taxability}`}
                        {!isZeroDec(view.lines[i].refunded_qty) && `. Refunded ${num(view.lines[i].refunded_qty)}`}
                      </Text>
                    )}
                  </div>
                  {editable && (
                    <ActionIcon variant="subtle" color="gray" aria-label="Remove line" onClick={() => removeLine(i)}>
                      <IconTrash size={16} />
                    </ActionIcon>
                  )}
                </Group>
              </Paper>
            ))}
            {editable && (
              <Paper p="md">
                <Group gap="sm" wrap="wrap">
                  <Select
                    aria-label="Add service"
                    placeholder="Add a service…"
                    searchable
                    style={{ flex: "1 1 200px" }}
                    data={(services.data ?? []).map((s) => ({ value: String(s.id), label: s.name }))}
                    value={null}
                    onChange={(v) => v && addService(Number(v))}
                  />
                  <Select
                    aria-label="Add retail item"
                    placeholder={retail.length ? "Add a retail item…" : "No retail products yet"}
                    disabled={!retail.length}
                    searchable
                    style={{ flex: "1 1 200px" }}
                    data={retail.map((p) => ({ value: String(p.id), label: [p.brand, p.name].filter(Boolean).join(" ") }))}
                    value={null}
                    onChange={(v) => v && addRetail(Number(v))}
                  />
                  <Button variant="default" leftSection={<IconPlus size={14} />} onClick={addTip}>
                    Add tip
                  </Button>
                </Group>
              </Paper>
            )}
            {editable && (
              <Paper p="lg">
                <SimpleGrid cols={2}>
                  <DecimalInput label="Discount on the whole sale" description="Split across services and retail by price; tips aren't discounted" unit="$" value={draft.sale_discount} onChange={(v) => setDraft({ ...draft, sale_discount: v })} />
                  <Textarea label="Notes" value={draft.notes} onChange={(e) => setDraft({ ...draft, notes: e.currentTarget.value })} />
                </SimpleGrid>
              </Paper>
            )}
          </Stack>
        </Grid.Col>
        <Grid.Col span={{ base: 12, lg: 4 }}>
          <Paper p="lg" style={{ position: "sticky", top: "calc(var(--app-shell-header-height, 52px) + 16px)" }}>
            <Title order={4} mb="sm">
              Totals
            </Title>
            {t ? (
              <Breakdown
                rows={[
                  { label: "Services and retail", value: money(t.subtotal) },
                  { label: "Discounts", value: isZeroDec(t.discount_total) ? money("0") : `−${money(t.discount_total)}`, dim: true },
                  {
                    label: t.tax_resolved ? `Sales tax${view?.tax_label ? ` (${view.tax_label})` : ""}` : "Sales tax (estimate)",
                    value: money(t.tax_total),
                    tone: t.tax_resolved ? undefined : "warn",
                  },
                  { label: "Tips", value: money(t.tip_total), dim: true },
                  { label: "Total", value: money(t.total), total: true },
                  ...(view && !isZeroDec(view.refunded) ? [{ label: "Refunded", value: `−${money(view.refunded)}`, dim: true }] : []),
                  { label: "Paid", value: money(view?.paid ?? "0") },
                  { label: "Balance due", value: money(view?.balance ?? t.total), total: true, tone: view && !isZeroDec(view.balance) ? "warn" : undefined },
                ]}
              />
            ) : (
              <Text size="sm" c="dimmed">
                Add a service, retail item or tip.
              </Text>
            )}
            {blockers.length > 0 && editable && (
              <Alert color="yellow" mt="md" icon={<IconAlertTriangle size={16} />} title="Before finalizing">
                <Stack gap={2}>
                  {blockers.map((b) => (
                    <Text key={b} size="xs">
                      {b}
                    </Text>
                  ))}
                </Stack>
                <Anchor size="xs" onClick={() => navigate("/tax")}>
                  Open Sales tax
                </Anchor>
              </Alert>
            )}
            {view?.warnings.map((w) => (
              <Alert key={w} color="blue" mt="xs" py={6}>
                <Text size="xs">{w}</Text>
              </Alert>
            ))}
            {draft.id && !voided && (
              <>
                <Divider my="md" label="Payments" labelPosition="left" />
                {view?.payments.map((p) => (
                  <Group key={p.id} justify="space-between" opacity={p.voided ? 0.5 : 1}>
                    <Text size="sm" td={p.voided ? "line-through" : undefined}>
                      {p.refund_id ? "Refund" : METHODS.find((m) => m.value === p.method)?.label} {dateLabel(p.paid_on)}
                    </Text>
                    <Group gap={4}>
                      <Text size="sm" className="srm-num">
                        {money(p.amount)}
                      </Text>
                      {!p.voided && !p.refund_id && (
                        <ActionIcon
                          size="sm"
                          variant="subtle"
                          color="gray"
                          aria-label="Remove payment"
                          onClick={async () => {
                            await call("payment_void", { id: p.id });
                            refresh();
                          }}
                        >
                          <IconTrash size={12} />
                        </ActionIcon>
                      )}
                    </Group>
                  </Group>
                ))}
                {view && !isZeroDec(view.balance) && !isNegDec(view.balance) && (
                  <Stack gap="xs" mt="sm">
                    <SegmentedControl size="xs" fullWidth value={pay.method} onChange={(m) => setPay({ ...pay, method: m })} data={METHODS} />
                    <Group gap="xs" align="flex-end">
                      <DecimalInput aria-label="Payment amount" unit="$" placeholder={view.balance} value={pay.amount} onChange={(v) => setPay({ ...pay, amount: v })} style={{ flex: 1 }} />
                      <Button onClick={addPayment} disabled={dirty}>
                        Record payment
                      </Button>
                    </Group>
                    <Text size="xs" c="dimmed">
                      Payments are recorded here, not processed. Card details are never stored.
                    </Text>
                    {payErr && (
                      <Text size="xs" c="red">
                        {payErr}
                      </Text>
                    )}
                  </Stack>
                )}
              </>
            )}
            {editable && (
              <Button fullWidth mt="lg" size="md" onClick={finalize} loading={finalizing} disabled={!draft.id || dirty || saveState === "saving" || blockers.length > 0}>
                Finalize sale
              </Button>
            )}
            {editable && (
              <Text size="xs" c="dimmed" mt={6}>
                Finalizing deducts the products used from stock and locks this sale's prices, costs and tax.
              </Text>
            )}
            {view && view.refunds.length > 0 && (
              <>
                <Divider my="md" label="Refunds" labelPosition="left" />
                {view.refunds.map((r) => (
                  <Text key={r.id} size="sm">
                    {r.number} on {dateLabel(r.refund_date)}: {money(r.total)} ({r.reason})
                  </Text>
                ))}
              </>
            )}
          </Paper>
        </Grid.Col>
      </Grid>
      {view && finalized && <RefundModal opened={refundOpen} onClose={() => setRefundOpen(false)} view={view} today={today} onDone={refresh} />}
      <VoidModal opened={voidOpen} onClose={() => setVoidOpen(false)} saleId={draft.id} onDone={refresh} />
    </>
  );
}

function RefundModal({ opened, onClose, view, today, onDone }: { opened: boolean; onClose: () => void; view: SaleView; today: string; onDone: () => void }) {
  const [sel, setSel] = useState<Record<number, { on: boolean; qty: string; restock: boolean }>>({});
  const [reason, setReason] = useState("");
  const [method, setMethod] = useState("card");
  const [date, setDate] = useState(today);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (opened) {
      setSel({});
      setReason("");
      setErr(null);
    }
  }, [opened]);
  async function submit() {
    const lines = view.lines
      .map((l, i) => ({ l, i, s: sel[l.id] }))
      .filter((x) => x.s?.on)
      .map((x) => ({ sale_line_id: x.l.id, qty: x.s.qty || remaining(x.i), restock: x.s.restock && view.draft.lines[x.i].kind === "retail" }));
    const refund: RefundInput = { refund_date: date, reason, method, lines };
    setBusy(true);
    try {
      await call("sale_refund", { id: view.draft.id, refund });
      notifications.show({ message: "Refund recorded", color: "teal" });
      onDone();
      onClose();
    } catch (e) {
      setErr(isAppError(e) ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }
  const remaining = (i: number) => {
    const q = view.draft.lines[i].qty;
    const r = view.lines[i].refunded_qty;
    // Display/default only: quantities are small decimals; the backend validates.
    return String(Number(q) - Number(r));
  };
  return (
    <Modal opened={opened} onClose={onClose} title={`Refund on sale ${view.number}`} size="lg">
      <Stack>
        <Text size="sm" c="dimmed">
          Tax is returned in proportion to what's refunded. Retail items can go back into stock at the cost they left with; product used in a service is never put back
          automatically.
        </Text>
        {view.draft.lines.map((l, i) => {
          const s = sel[view.lines[i].id] ?? { on: false, qty: "", restock: l.kind === "retail" };
          const left = remaining(i);
          if (Number(left) <= 0) return null;
          return (
            <Group key={i} gap="sm" wrap="nowrap">
              <Checkbox checked={s.on} onChange={(e) => setSel({ ...sel, [view.lines[i].id]: { ...s, on: e.currentTarget.checked } })} label={`${l.description} (${money(view.lines[i].net)})`} style={{ flex: 1 }} />
              {l.kind !== "tip" && <DecimalInput size="xs" w={90} aria-label="Refund quantity" placeholder={left} value={s.qty} onChange={(v) => setSel({ ...sel, [view.lines[i].id]: { ...s, qty: v } })} disabled={!s.on} />}
              {l.kind === "retail" && <Checkbox size="xs" label="Back to stock" checked={s.restock} onChange={(e) => setSel({ ...sel, [view.lines[i].id]: { ...s, restock: e.currentTarget.checked } })} disabled={!s.on} />}
            </Group>
          );
        })}
        <SimpleGrid cols={2}>
          <Select label="Money returned by" data={METHODS} value={method} allowDeselect={false} onChange={(v) => setMethod(v ?? "card")} />
          <TextInput type="date" label="Refund date" value={date} onChange={(e) => setDate(e.currentTarget.value)} />
        </SimpleGrid>
        <Textarea label="Reason" required value={reason} onChange={(e) => setReason(e.currentTarget.value)} />
        {err && <Alert color="red">{err}</Alert>}
        <Group justify="flex-end">
          <Button onClick={submit} loading={busy} disabled={!reason.trim() || !Object.values(sel).some((s) => s.on)}>
            Record refund
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}

function VoidModal({ opened, onClose, saleId, onDone }: { opened: boolean; onClose: () => void; saleId: number | null; onDone: () => void }) {
  const [reason, setReason] = useState("");
  const [err, setErr] = useState<string | null>(null);
  return (
    <Modal opened={opened} onClose={onClose} title="Void this sale?">
      <Stack>
        <Text size="sm">
          Use this only for a sale recorded by mistake. Its stock movements are reversed, its payments are cancelled, and it drops out of reports. For a real sale that's being
          returned, use a refund instead.
        </Text>
        <Textarea label="Reason" required value={reason} onChange={(e) => setReason(e.currentTarget.value)} />
        {err && <Alert color="red">{err}</Alert>}
        <Group justify="flex-end">
          <Button
            color="red"
            disabled={!reason.trim()}
            onClick={async () => {
              try {
                await call("sale_void", { id: saleId, reason });
                onDone();
                onClose();
              } catch (e) {
                setErr(errorMessage(e));
              }
            }}
          >
            Void sale
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
