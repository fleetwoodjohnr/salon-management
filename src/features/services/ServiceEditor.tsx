import {
  ActionIcon,
  Alert,
  Autocomplete,
  Badge,
  Button,
  Collapse,
  Drawer,
  Grid,
  Group,
  NumberInput,
  Paper,
  SegmentedControl,
  Select,
  SimpleGrid,
  Stack,
  Switch,
  Table,
  Text,
  Textarea,
  TextInput,
  Title,
} from "@mantine/core";
import { useDebouncedValue, useDisclosure, useHotkeys } from "@mantine/hooks";
import { IconArchive, IconHistory, IconPlus, IconTrash } from "@tabler/icons-react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { call, isAppError, type AppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { Estimate, ProductRow, ProfileView, ServiceInput, ServiceSummary, ServiceView, TaxCategory, TimeSpec, UnitDef } from "../../api/types";
import { DecimalInput } from "../../components/DecimalInput";
import { setDirty, useDirty } from "../../components/dirty";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { isoToLocal, minutesLabel, num } from "../../lib/format";
import { unitOptions, unitShort } from "../../lib/units";
import { positionLabels, roundLabels } from "../profiles/labels";
import { EstimatePanel } from "./EstimatePanel";

function blank(profileId: number | null): ServiceInput {
  return {
    id: null,
    name: "",
    category: "",
    description: "",
    profile_id: profileId,
    time: { hands_on_min: 45, processing_min: 0, setup_min: 5, cleanup_min: 10 },
    is_addon: false,
    waste_pct: "0",
    other_direct_cost: "0",
    other_direct_note: "",
    price: null,
    price_note: "",
    overrides: { target_kind: null, target_pct: null, position: null, rounding_increment: null, rounding_mode: null },
    tax_category: "service",
    variants: [],
    recipe: [],
    recipe_note: "",
  };
}

export function ServiceEditorPage() {
  const { id } = useParams();
  const isNew = id === "new";
  const q = useCmd<ServiceView>("service_get", { id: Number(id) }, !isNew);
  const profiles = useCmd<ProfileView[]>("profiles_list", { includeArchived: false });
  if (isNew) return profiles.data ? <ServiceEditor view={null} defaultProfile={profiles.data[0]?.id ?? null} /> : null;
  return <QueryState loading={q.isLoading} error={q.error}>{() => <ServiceEditor key={JSON.stringify(q.data!.input)} view={q.data!} defaultProfile={null} />}</QueryState>;
}

const TIME_FIELDS: { key: keyof TimeSpec; label: string; description: string }[] = [
  { key: "hands_on_min", label: "Hands-on", description: "Working on the client" },
  { key: "processing_min", label: "Processing", description: "Color or treatment develops; you're free" },
  { key: "setup_min", label: "Setup", description: "Mixing, preparing the station" },
  { key: "cleanup_min", label: "Cleanup", description: "After the client leaves" },
];

function ServiceEditor({ view, defaultProfile }: { view: ServiceView | null; defaultProfile: number | null }) {
  const navigate = useNavigate();
  const [form, setForm] = useState<ServiceInput>(view?.input ?? blank(defaultProfile));
  const baseline = useRef(JSON.stringify(form));
  const dirty = JSON.stringify(form) !== baseline.current;
  useDirty("service-editor", dirty);
  const [error, setError] = useState<AppError | null>(null);
  const [variantSel, setVariantSel] = useState<number[]>([]);
  const [showOverrides, overrides] = useDisclosure(Object.values(form.overrides).some((v) => v != null));
  const [historyOpen, history] = useDisclosure(false);

  const profiles = useCmd<ProfileView[]>("profiles_list", { includeArchived: false });
  const products = useCmd<ProductRow[]>("products_list", { includeArchived: false });
  const units = useCmd<UnitDef[]>("units_list");
  const taxCats = useCmd<TaxCategory[]>("tax_categories");
  const services = useCmd<ServiceSummary[]>("services_list", { includeArchived: false });
  const categories = useMemo(() => [...new Set((services.data ?? []).map((s) => s.category).filter(Boolean))], [services.data]);
  const byId = useMemo(() => new Map((products.data ?? []).map((p) => [p.id, p])), [products.data]);

  const [debounced] = useDebouncedValue(form, 200);
  const validVariants = variantSel.filter((id) => debounced.variants.some((v) => v.id === id));
  const estimate = useQuery({
    queryKey: ["service_estimate", debounced, validVariants],
    queryFn: () => call<Estimate>("service_estimate", { service: debounced, variantIds: validVariants }),
    placeholderData: keepPreviousData,
  });
  const estErr = estimate.error && isAppError(estimate.error) ? estimate.error : null;
  const err = (f: string) => [error, estErr].find((e) => e?.field === f)?.message;

  const set = <K extends keyof ServiceInput>(k: K, v: ServiceInput[K]) => setForm((f) => ({ ...f, [k]: v }));
  const setTime = (k: keyof TimeSpec, v: number) => setForm((f) => ({ ...f, time: { ...f.time, [k]: v } }));
  const setRecipe = (i: number, patch: Partial<ServiceInput["recipe"][number]>) => set("recipe", form.recipe.map((l, j) => (j === i ? { ...l, ...patch } : l)));
  const setVariant = (i: number, patch: Partial<ServiceInput["variants"][number]>) => set("variants", form.variants.map((v, j) => (j === i ? { ...v, ...patch } : v)));
  const setOv = <K extends keyof ServiceInput["overrides"]>(k: K, v: ServiceInput["overrides"][K]) => setForm((f) => ({ ...f, overrides: { ...f.overrides, [k]: v } }));

  const save = useAction<{ service: ServiceInput }, ServiceView>("service_save", {
    invalidate: ["services_list", "service_get"],
    success: (s) => `Saved ${s.input.name}${s.recipe_version ? ` (recipe version ${s.recipe_version})` : ""}`,
    silentError: true,
  });
  const archive = useAction<{ id: number; archived: boolean }>("service_archive", { invalidate: ["services_list", "service_get"], success: "Service archived" });
  function submit() {
    setError(null);
    save.mutate(
      { service: form },
      {
        onSuccess: (s) => {
          setDirty("service-editor", false); // before navigating, so the unsaved-changes guard doesn't fire
          baseline.current = JSON.stringify(s.input);
          setForm(s.input);
          if (!view) navigate(`/services/${s.input.id}`, { replace: true });
        },
        onError: (e) => setError(isAppError(e) ? e : { kind: "other", message: String(e), field: null }),
      },
    );
  }
  useHotkeys([["mod+S", submit]], []);

  const productOpts = (products.data ?? []).map((p) => ({ value: String(p.id), label: [p.brand, p.name].filter(Boolean).join(" ") }));
  const p = estimate.data?.pricing;
  const needsNote = p && (p.status === "below_target" || p.status === "below_cost");

  return (
    <>
      <PageHeader
        title={view ? view.input.name : "New service"}
        description={view ? `${form.category || "Service"}${view.recipe_version ? `, recipe version ${view.recipe_version}` : ""}` : "Describe the service, its time and the products it uses."}
        actions={
          <>
            {view && (
              <Button variant="default" leftSection={<IconHistory size={16} />} onClick={history.open}>
                Recipe history
              </Button>
            )}
            {view && !view.archived && (
              <Button variant="default" leftSection={<IconArchive size={16} />} onClick={() => archive.mutate({ id: view.input.id!, archived: true })}>
                Archive
              </Button>
            )}
            <Badge color={dirty ? "yellow" : "gray"} size="lg">
              {dirty ? "Unsaved changes" : view ? "Saved" : "Not saved yet"}
            </Badge>
            <Button onClick={submit} loading={save.isPending} disabled={!dirty && !!view}>
              Save service
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
                <TextInput label="Service name" required value={form.name} onChange={(e) => set("name", e.currentTarget.value)} error={err("name")} data-autofocus={!view} />
                <Autocomplete label="Category" placeholder="e.g. Color, Cuts, Nails" data={categories} value={form.category} onChange={(v) => set("category", v)} />
                <Select
                  label="Work profile"
                  description="Whose pay and overhead this service is costed with"
                  data={(profiles.data ?? []).map((pr) => ({ value: String(pr.id), label: pr.name }))}
                  value={form.profile_id != null ? String(form.profile_id) : null}
                  onChange={(v) => set("profile_id", v ? Number(v) : null)}
                  error={err("profile_id")}
                />
                <Select
                  label="Sales tax category"
                  description="Taxability is decided in Sales tax"
                  data={(taxCats.data ?? []).filter((c) => c.applies_to !== "tips" && c.applies_to !== "retail").map((c) => ({ value: c.code, label: c.label }))}
                  value={form.tax_category}
                  allowDeselect={false}
                  onChange={(v) => set("tax_category", v ?? "service")}
                />
              </SimpleGrid>
              <Textarea mt="md" label="Description" autosize minRows={2} value={form.description} onChange={(e) => set("description", e.currentTarget.value)} />
              <Switch mt="md" label="This is an add-on (booked with another service)" checked={form.is_addon} onChange={(e) => set("is_addon", e.currentTarget.checked)} />
            </Paper>

            <Paper p="lg">
              <Title order={4}>Time</Title>
              <Text size="sm" c="dimmed" mb="md">
                In minutes. Hands-on, setup and cleanup are paid working time. Processing time occupies the chair but frees you for other work.
              </Text>
              <SimpleGrid cols={4}>
                {TIME_FIELDS.map((f) => (
                  <NumberInput
                    key={f.key}
                    label={f.label}
                    description={f.description}
                    min={0}
                    max={1440}
                    allowDecimal={false}
                    allowNegative={false}
                    suffix=" min"
                    value={form.time[f.key]}
                    onChange={(v) => setTime(f.key, typeof v === "number" ? v : 0)}
                    error={err(f.key)}
                  />
                ))}
              </SimpleGrid>
              <Text size="xs" c="dimmed" mt="sm">
                Chair time: {minutesLabel(form.time.hands_on_min + form.time.processing_min + form.time.setup_min + form.time.cleanup_min)}
              </Text>
            </Paper>

            <Paper p="lg">
              <Title order={4}>Recipe</Title>
              <Text size="sm" c="dimmed" mb="md">
                The products one service normally uses. Actual usage is recorded when the appointment is completed; this is the plan used for estimates.
              </Text>
              {form.recipe.length > 0 && (
                <Table withRowBorders={false} verticalSpacing={4}>
                  <Table.Thead>
                    <Table.Tr>
                      <Table.Th w="40%">Product</Table.Th>
                      <Table.Th>Amount</Table.Th>
                      <Table.Th>Unit</Table.Th>
                      <Table.Th>Note</Table.Th>
                      <Table.Th />
                    </Table.Tr>
                  </Table.Thead>
                  <Table.Tbody>
                    {form.recipe.map((l, i) => {
                      const pr = byId.get(l.product_id);
                      return (
                        <Table.Tr key={i}>
                          <Table.Td>
                            <Select
                              aria-label={`Recipe line ${i + 1} product`}
                              searchable
                              data={productOpts}
                              value={l.product_id ? String(l.product_id) : null}
                              onChange={(v) => {
                                const np = byId.get(Number(v));
                                setRecipe(i, { product_id: Number(v) || 0, unit: np?.stock_unit ?? l.unit });
                              }}
                            />
                          </Table.Td>
                          <Table.Td>
                            <DecimalInput aria-label={`Recipe line ${i + 1} amount`} value={l.qty} onChange={(v) => setRecipe(i, { qty: v })} error={err(`recipe.${i}.qty`)} w={100} />
                          </Table.Td>
                          <Table.Td>
                            <Select
                              aria-label={`Recipe line ${i + 1} unit`}
                              data={pr ? unitOptions(units.data ?? [], pr) : []}
                              value={l.unit || null}
                              allowDeselect={false}
                              onChange={(v) => setRecipe(i, { unit: v ?? l.unit })}
                              error={err(`recipe.${i}.unit`)}
                              w={130}
                            />
                          </Table.Td>
                          <Table.Td>
                            <TextInput miw={110} aria-label={`Recipe line ${i + 1} note`} value={l.note} onChange={(e) => setRecipe(i, { note: e.currentTarget.value })} />
                          </Table.Td>
                          <Table.Td>
                            <ActionIcon variant="subtle" color="gray" aria-label={`Remove recipe line ${i + 1}`} onClick={() => set("recipe", form.recipe.filter((_, j) => j !== i))}>
                              <IconTrash size={16} />
                            </ActionIcon>
                          </Table.Td>
                        </Table.Tr>
                      );
                    })}
                  </Table.Tbody>
                </Table>
              )}
              {(products.data?.length ?? 0) === 0 ? (
                <Text size="sm" c="dimmed">
                  Add products in Inventory to build recipes.
                </Text>
              ) : (
                <Button variant="subtle" size="xs" mt="xs" leftSection={<IconPlus size={14} />} onClick={() => set("recipe", [...form.recipe, { product_id: 0, qty: "", unit: "", note: "" }])}>
                  Add product
                </Button>
              )}
              {view && JSON.stringify(form.recipe) !== JSON.stringify(view.input.recipe) && (
                <TextInput mt="sm" label="What changed in the recipe?" description="Saved with the new recipe version" value={form.recipe_note} onChange={(e) => set("recipe_note", e.currentTarget.value)} />
              )}
              <SimpleGrid cols={3} mt="lg">
                <DecimalInput
                  label="Waste allowance"
                  description="Extra material lost, % of recipe"
                  unit="%"
                  value={form.waste_pct}
                  onChange={(v) => set("waste_pct", v)}
                  error={err("waste_pct")}
                />
                <DecimalInput
                  label="Other direct costs"
                  description="Per service, not in inventory"
                  unit="$"
                  value={form.other_direct_cost}
                  onChange={(v) => set("other_direct_cost", v)}
                  error={err("other_direct_cost")}
                />
                <TextInput label="What they are" placeholder="e.g. Laundry, disposable cape" value={form.other_direct_note} onChange={(e) => set("other_direct_note", e.currentTarget.value)} />
              </SimpleGrid>
            </Paper>

            <Paper p="lg">
              <Title order={4}>Variants</Title>
              <Text size="sm" c="dimmed" mb="md">
                Options such as hair length, density, complexity or stylist level. Each changes time, material (as a multiple of the recipe) and price. Clients get one
                choice per group.
              </Text>
              {form.variants.length > 0 && (
                <Table withRowBorders={false} verticalSpacing={4}>
                  <Table.Thead>
                    <Table.Tr>
                      <Table.Th>Group</Table.Th>
                      <Table.Th>Option</Table.Th>
                      <Table.Th>Hands-on</Table.Th>
                      <Table.Th>Processing</Table.Th>
                      <Table.Th>Material ×</Table.Th>
                      <Table.Th>Price +</Table.Th>
                      <Table.Th />
                    </Table.Tr>
                  </Table.Thead>
                  <Table.Tbody>
                    {form.variants.map((v, i) => (
                      <Table.Tr key={v.id ?? `new-${i}`}>
                        <Table.Td>
                          <TextInput miw={130} aria-label={`Variant ${i + 1} group`} placeholder="Hair length" value={v.group_name} onChange={(e) => setVariant(i, { group_name: e.currentTarget.value })} error={err(`variants.${i}.name`)} />
                        </Table.Td>
                        <Table.Td>
                          <TextInput miw={110} aria-label={`Variant ${i + 1} option`} placeholder="Long" value={v.name} onChange={(e) => setVariant(i, { name: e.currentTarget.value })} />
                        </Table.Td>
                        <Table.Td>
                          <NumberInput aria-label={`Variant ${i + 1} extra hands-on minutes`} suffix=" min" allowDecimal={false} value={v.hands_on_delta} onChange={(x) => setVariant(i, { hands_on_delta: Number(x) || 0 })} w={90} />
                        </Table.Td>
                        <Table.Td>
                          <NumberInput aria-label={`Variant ${i + 1} extra processing minutes`} suffix=" min" allowDecimal={false} value={v.processing_delta} onChange={(x) => setVariant(i, { processing_delta: Number(x) || 0 })} w={90} />
                        </Table.Td>
                        <Table.Td>
                          <DecimalInput aria-label={`Variant ${i + 1} material factor`} value={v.material_factor} onChange={(x) => setVariant(i, { material_factor: x })} w={80} />
                        </Table.Td>
                        <Table.Td>
                          <DecimalInput aria-label={`Variant ${i + 1} price adjustment`} unit="$" allowNegative value={v.price_delta} onChange={(x) => setVariant(i, { price_delta: x })} w={100} />
                        </Table.Td>
                        <Table.Td>
                          <ActionIcon variant="subtle" color="gray" aria-label={`Remove variant ${i + 1}`} onClick={() => set("variants", form.variants.filter((_, j) => j !== i))}>
                            <IconTrash size={16} />
                          </ActionIcon>
                        </Table.Td>
                      </Table.Tr>
                    ))}
                  </Table.Tbody>
                </Table>
              )}
              <Button
                variant="subtle"
                size="xs"
                mt="xs"
                leftSection={<IconPlus size={14} />}
                onClick={() => set("variants", [...form.variants, { id: null, group_name: form.variants.at(-1)?.group_name ?? "Hair length", name: "", hands_on_delta: 0, processing_delta: 0, material_factor: "1", price_delta: "0" }])}
              >
                Add variant
              </Button>
            </Paper>

            <Paper p="lg">
              <Title order={4} mb="md">
                Price
              </Title>
              <SimpleGrid cols={2}>
                <DecimalInput label="Your price" description="Before tax, for the base service" unit="$" value={form.price ?? ""} onChange={(v) => set("price", v || null)} error={err("price")} />
                <Textarea
                  label="Price note"
                  description={needsNote ? "Required: explain why this price is below your target" : "Why you chose this price (optional)"}
                  autosize
                  minRows={2}
                  value={form.price_note}
                  onChange={(e) => set("price_note", e.currentTarget.value)}
                  error={needsNote && !form.price_note.trim() ? "Explain this override" : undefined}
                />
              </SimpleGrid>
              <Button variant="subtle" size="xs" mt="sm" onClick={overrides.toggle}>
                {showOverrides ? "Hide" : "Show"} pricing settings for this service
              </Button>
              <Collapse expanded={showOverrides}>
                <Text size="xs" c="dimmed" my="sm">
                  Leave blank to use the work profile's defaults.
                </Text>
                <SimpleGrid cols={3}>
                  <div>
                    <Text size="sm" fw={500} mb={4}>
                      Target type
                    </Text>
                    <SegmentedControl
                      fullWidth
                      size="xs"
                      value={form.overrides.target_kind ?? "default"}
                      onChange={(v) => setOv("target_kind", v === "default" ? null : (v as "margin" | "markup"))}
                      data={[
                        { value: "default", label: "Profile" },
                        { value: "margin", label: "Margin" },
                        { value: "markup", label: "Markup" },
                      ]}
                    />
                  </div>
                  <DecimalInput label="Target %" unit="%" placeholder="Profile default" value={form.overrides.target_pct ?? ""} onChange={(v) => setOv("target_pct", v || null)} error={err("overrides.target_pct")} />
                  <Select
                    label="Market position"
                    placeholder="Profile default"
                    clearable
                    data={Object.entries(positionLabels).map(([value, x]) => ({ value, label: x.label }))}
                    value={form.overrides.position}
                    onChange={(v) => setOv("position", v as ServiceInput["overrides"]["position"])}
                  />
                  <Select
                    label="Round to"
                    placeholder="Profile default"
                    clearable
                    data={["0.01", "0.25", "0.5", "1", "5", "10"].map((v) => ({ value: v, label: v === "0.01" ? "Cents" : `$${v}` }))}
                    value={form.overrides.rounding_increment}
                    onChange={(v) => setOv("rounding_increment", v)}
                  />
                  <Select
                    label="Rounding direction"
                    placeholder="Profile default"
                    clearable
                    data={Object.entries(roundLabels).map(([value, label]) => ({ value, label }))}
                    value={form.overrides.rounding_mode}
                    onChange={(v) => setOv("rounding_mode", v as ServiceInput["overrides"]["rounding_mode"])}
                  />
                </SimpleGrid>
              </Collapse>
            </Paper>
          </Stack>
        </Grid.Col>
        <Grid.Col span={{ base: 12, lg: 4.5 }}>
          <EstimatePanel estimate={estimate.data} variants={form.variants} selected={validVariants} onSelect={setVariantSel} onUsePrice={(price) => set("price", price)} />
        </Grid.Col>
      </Grid>
      <Drawer opened={historyOpen} onClose={history.close} title="Recipe history" position="right" size="md">
        <Text size="sm" c="dimmed" mb="md">
          Each change to the recipe is kept as a version. Completed sales record which version they used and what was actually consumed.
        </Text>
        <Stack>
          {(view?.recipe_versions ?? []).map((v) => (
            <Paper key={v.id} p="sm">
              <Group justify="space-between">
                <Text fw={600} size="sm">
                  Version {v.version}
                </Text>
                <Text size="xs" c="dimmed">
                  {isoToLocal(v.created_at)}
                </Text>
              </Group>
              {v.note && (
                <Text size="xs" c="dimmed">
                  {v.note}
                </Text>
              )}
              {v.lines.map((l, i) => (
                <Text key={i} size="sm">
                  {l.product_name}: {num(l.qty, 3)} {unitShort(l.unit)}
                </Text>
              ))}
            </Paper>
          ))}
        </Stack>
      </Drawer>
    </>
  );
}
