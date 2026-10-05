import {
  Accordion,
  Alert,
  Badge,
  Box,
  Button,
  Drawer,
  Grid,
  Group,
  Paper,
  Radio,
  SegmentedControl,
  Select,
  SimpleGrid,
  Stack,
  Text,
  Textarea,
  TextInput,
  Title,
} from "@mantine/core";
import { useDebouncedValue, useDisclosure, useHotkeys } from "@mantine/hooks";
import { IconAlertTriangle, IconArchive, IconHistory, IconInfoCircle } from "@tabler/icons-react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { call, isAppError, type AppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { Business, Location, ProfileData, ProfileRates, ProfileVersionInfo, ProfileView } from "../../api/types";
import { Breakdown } from "../../components/Breakdown";
import { DecimalInput } from "../../components/DecimalInput";
import { setDirty, useDirty } from "../../components/dirty";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { hours, isoToLocal, money, num, pct } from "../../lib/format";
import { basisLabels, compLabels, experienceLevels, kindLabels, positionLabels, roundLabels } from "./labels";

export function defaultProfile(locationId: number | null): ProfileData {
  return {
    kind: "individual",
    specialty: "",
    experience_level: "experienced",
    location_id: locationId,
    comp_model: "owner_target_hourly",
    hourly_rate: "30",
    employer_burden_pct: "0",
    commission_pct: "0",
    retail_commission_pct: "0",
    weekly_hours: "40",
    weeks_per_year: "48",
    utilization_pct: "75",
    overhead: { rent: "0", utilities: "0", insurance: "0", software: "0", other: "0", other_label: "" },
    processing_pct: "0",
    processing_fixed: "0",
    card_share_pct: "100",
    target_kind: "margin",
    target_pct: "20",
    position: "standard",
    overhead_basis: "occupied",
    rounding_increment: "1",
    rounding_mode: "up",
    notes: "",
  };
}

export function ProfileEditorPage() {
  const { id } = useParams();
  const isNew = id === "new";
  const q = useCmd<ProfileView>("profile_get", { id: Number(id) }, !isNew);
  const biz = useCmd<Business>("business_get");
  if (isNew) {
    return biz.data ? <ProfileEditor initial={null} defaultLocation={biz.data.primary_location_id} /> : null;
  }
  return (
    <QueryState loading={q.isLoading} error={q.error}>
      {() => <ProfileEditor key={q.data!.version_id} initial={q.data!} defaultLocation={null} />}
    </QueryState>
  );
}

const definitions: { term: string; text: string }[] = [
  {
    term: "Labor cost",
    text: "What it costs to pay for someone's working time, including employer payroll costs for employees. Because pay covers every hour worked but only billable hours bring in money, it's spread over billable hours.",
  },
  {
    term: "Owner compensation",
    text: "What you want to earn per hour you work. Treating it as a cost means your prices pay you first; profit is what's left after that.",
  },
  {
    term: "Overhead",
    text: "Costs you pay whether or not a client is in the chair: rent or chair rental, utilities, insurance, software. It's divided by your realistic billable hours.",
  },
  {
    term: "Commission",
    text: "A share of the service price paid to whoever did the work. It grows with the price, so the pricing engine solves for it exactly instead of adding a fixed amount.",
  },
  {
    term: "Margin vs markup",
    text: "Margin is profit as a share of the price. Markup is profit as a share of cost. A $60 cost sold for $100 is a 40% margin but a 66.7% markup.",
  },
];

function RatesPanel({ rates, error }: { rates: ProfileRates | undefined; error: AppError | null }) {
  return (
    <Paper p="lg" style={{ position: "sticky", top: "calc(var(--app-shell-header-height, 52px) + 16px)" }}>
      <Title order={4} mb={4}>
        What this profile costs
      </Title>
      <Text size="xs" c="dimmed" mb="md">
        Updates as you type. Services use these rates for future estimates.
      </Text>
      {error ? (
        <Alert color="yellow" icon={<IconAlertTriangle size={16} />}>
          {error.message}
        </Alert>
      ) : rates ? (
        <>
          <Breakdown
            rows={[
              { label: "Hours worked per month", value: hours(rates.monthly_hours) },
              { label: "Billable hours per month", value: hours(rates.monthly_billable_hours), hint: "Hours worked × realistic utilization." },
              { label: "Monthly overhead", value: money(rates.monthly_overhead) },
              {
                label: "Overhead per billable hour",
                value: money(rates.overhead_per_billable_hour),
                total: true,
                hint: "Monthly overhead ÷ billable hours.",
              },
              { label: "Pay per hour worked", value: money(rates.pay_per_hour_worked) },
              {
                label: "Pay per billable hour",
                value: money(rates.labor_cost_per_billable_hour),
                total: true,
                hint: "Pay per hour worked ÷ utilization: non-billable hours are paid for by billable ones.",
              },
              { label: "Service commission", value: pct(rates.commission_pct) },
              { label: "Retail commission", value: pct(rates.retail_commission_pct) },
            ]}
          />
          {rates.notes.length > 0 && (
            <Stack gap={6} mt="md">
              {rates.notes.map((n) => (
                <Alert key={n} color="blue" variant="light" icon={<IconInfoCircle size={16} />} py={6}>
                  <Text size="xs">{n}</Text>
                </Alert>
              ))}
            </Stack>
          )}
        </>
      ) : null}
      <Accordion variant="contained" mt="lg" radius="md">
        {definitions.map((d) => (
          <Accordion.Item key={d.term} value={d.term}>
            <Accordion.Control>
              <Text size="sm">{d.term}</Text>
            </Accordion.Control>
            <Accordion.Panel>
              <Text size="sm" c="var(--srm-ink-soft)">
                {d.text}
              </Text>
            </Accordion.Panel>
          </Accordion.Item>
        ))}
      </Accordion>
    </Paper>
  );
}

function Section({ title, description, children }: { title: string; description?: string; children: React.ReactNode }) {
  return (
    <Paper p="lg">
      <Title order={4}>{title}</Title>
      {description && (
        <Text size="sm" c="dimmed" mt={2} mb="md" maw={620}>
          {description}
        </Text>
      )}
      <Box mt={description ? 0 : "md"}>{children}</Box>
    </Paper>
  );
}

function ProfileEditor({ initial, defaultLocation }: { initial: ProfileView | null; defaultLocation: number | null }) {
  const navigate = useNavigate();
  const [name, setName] = useState(initial?.name ?? "");
  const [data, setData] = useState<ProfileData>(initial?.data ?? defaultProfile(defaultLocation));
  const baseline = useRef(JSON.stringify({ name, data }));
  const dirty = JSON.stringify({ name, data }) !== baseline.current;
  useDirty("profile-editor", dirty);
  const [saveError, setSaveError] = useState<AppError | null>(null);
  const [historyOpen, history] = useDisclosure(false);

  const locations = useCmd<Location[]>("locations_list");
  const versions = useCmd<ProfileVersionInfo[]>("profile_versions", { id: initial?.id ?? 0 }, !!initial);

  const [debounced] = useDebouncedValue(data, 200);
  const preview = useQuery({
    queryKey: ["profile_preview", debounced],
    queryFn: () => call<ProfileRates>("profile_preview", { data: debounced }),
    placeholderData: keepPreviousData,
  });
  const previewError = preview.error && isAppError(preview.error) ? preview.error : null;
  const fieldError = (f: string) => {
    for (const e of [saveError, previewError]) if (e?.field === f) return e.message;
    return undefined;
  };

  const set = <K extends keyof ProfileData>(k: K, v: ProfileData[K]) => setData((d) => ({ ...d, [k]: v }));
  const setOh = (k: keyof ProfileData["overhead"], v: string) => setData((d) => ({ ...d, overhead: { ...d.overhead, [k]: v } }));

  const save = useAction<{ id: number | null; name: string; data: ProfileData }, ProfileView>("profile_save", {
    invalidate: ["profiles_list", "profile_get", "profile_versions"],
    success: (p) => `Saved ${p.name} (version ${p.version})`,
    silentError: true,
  });
  const archive = useAction<{ id: number; archived: boolean }>("profile_archive", {
    invalidate: ["profiles_list", "profile_get"],
    success: "Profile archived",
  });

  function submit() {
    setSaveError(null);
    save.mutate(
      { id: initial?.id ?? null, name, data },
      {
        onSuccess: (p) => {
          setDirty("profile-editor", false); // before navigating, so the unsaved-changes guard doesn't fire
          baseline.current = JSON.stringify({ name, data });
          if (!initial) navigate(`/profiles/${p.id}`, { replace: true });
        },
        onError: (e) => setSaveError(isAppError(e) ? e : { kind: "other", message: String(e), field: null }),
      },
    );
  }
  useHotkeys([["mod+S", submit]], []);

  const locOptions = useMemo(
    () => (locations.data ?? []).filter((l) => !l.archived || l.id === data.location_id).map((l) => ({ value: String(l.id), label: l.name })),
    [locations.data, data.location_id],
  );
  const showWage = data.comp_model !== "commission";
  const showCommission = data.comp_model === "commission" || data.comp_model === "hourly_plus_commission";
  const showBurden = data.comp_model === "hourly_wage" || data.comp_model === "hourly_plus_commission";

  return (
    <>
      <PageHeader
        title={initial ? initial.name : "New work profile"}
        description={
          initial
            ? `Version ${initial.version}, saved ${isoToLocal(initial.version_created_at)}. Saving creates a new version; completed sales keep the figures they were costed with.`
            : "Describe how this person is paid, how much they work, and the overhead they carry."
        }
        actions={
          <>
            {initial && (
              <Button variant="default" leftSection={<IconHistory size={16} />} onClick={history.open}>
                History
              </Button>
            )}
            {initial && !initial.archived && (
              <Button variant="default" leftSection={<IconArchive size={16} />} onClick={() => archive.mutate({ id: initial.id, archived: true })}>
                Archive
              </Button>
            )}
            <Badge color={dirty ? "yellow" : "gray"} variant="light" size="lg">
              {dirty ? "Unsaved changes" : initial ? "Saved" : "Not saved yet"}
            </Badge>
            <Button onClick={submit} loading={save.isPending} disabled={!dirty && !!initial}>
              Save profile
            </Button>
          </>
        }
      />
      {saveError && !saveError.field && (
        <Alert color="red" mb="md" title="Not saved">
          {saveError.message}
        </Alert>
      )}
      <Grid gap="xl">
        <Grid.Col span={{ base: 12, lg: 7.5 }}>
          <Stack gap="lg">
            <Section title="Who this is">
              <SimpleGrid cols={2}>
                <TextInput
                  label="Profile name"
                  placeholder="e.g. Me – color specialist"
                  value={name}
                  onChange={(e) => setName(e.currentTarget.value)}
                  error={fieldError("name")}
                  required
                />
                <Select
                  label="Type"
                  data={Object.entries(kindLabels).map(([value, label]) => ({ value, label }))}
                  value={data.kind}
                  allowDeselect={false}
                  onChange={(v) => {
                    const kind = v as ProfileData["kind"];
                    setData((d) => ({
                      ...d,
                      kind,
                      comp_model: kind === "employee" ? (d.comp_model === "owner_target_hourly" ? "hourly_wage" : d.comp_model) : d.comp_model,
                    }));
                  }}
                />
                <TextInput label="Specialty" placeholder="e.g. Blonding, cuts, nails" value={data.specialty} onChange={(e) => set("specialty", e.currentTarget.value)} />
                <Select label="Experience" data={experienceLevels} value={data.experience_level} onChange={(v) => set("experience_level", v ?? "")} />
                <Select
                  label="Location"
                  placeholder={locOptions.length ? "Choose a location" : "Add locations in Settings"}
                  data={locOptions}
                  value={data.location_id != null ? String(data.location_id) : null}
                  onChange={(v) => set("location_id", v ? Number(v) : null)}
                  clearable
                />
              </SimpleGrid>
            </Section>

            <Section title="Pay" description="Choose how this person is paid. Only the parts that apply are counted, so labor is never added twice.">
              <Radio.Group value={data.comp_model} onChange={(v) => set("comp_model", v as ProfileData["comp_model"])}>
                <SimpleGrid cols={2} spacing="sm">
                  {Object.entries(compLabels).map(([value, c]) => (
                    <Radio.Card key={value} value={value} p="md" radius="md">
                      <Group wrap="nowrap" align="flex-start">
                        <Radio.Indicator />
                        <div>
                          <Text fw={600} size="sm">
                            {c.label}
                          </Text>
                          <Text size="xs" c="dimmed">
                            {c.description}
                          </Text>
                        </div>
                      </Group>
                    </Radio.Card>
                  ))}
                </SimpleGrid>
              </Radio.Group>
              <SimpleGrid cols={3} mt="md">
                {showWage && (
                  <DecimalInput
                    label={data.comp_model === "owner_target_hourly" ? "Desired pay per hour worked" : "Hourly wage"}
                    unit="$"
                    value={data.hourly_rate}
                    onChange={(v) => set("hourly_rate", v)}
                    error={fieldError("hourly_rate")}
                  />
                )}
                {showBurden && (
                  <DecimalInput
                    label="Employer burden"
                    description="Your payroll taxes and benefits"
                    unit="%"
                    value={data.employer_burden_pct}
                    onChange={(v) => set("employer_burden_pct", v)}
                    error={fieldError("employer_burden_pct")}
                  />
                )}
                {showCommission && (
                  <DecimalInput
                    label="Service commission"
                    description="Of the service price before tax"
                    unit="%"
                    value={data.commission_pct}
                    onChange={(v) => set("commission_pct", v)}
                    error={fieldError("commission_pct")}
                  />
                )}
                <DecimalInput
                  label="Retail commission"
                  description="Of retail product sales"
                  unit="%"
                  value={data.retail_commission_pct}
                  onChange={(v) => set("retail_commission_pct", v)}
                  error={fieldError("retail_commission_pct")}
                />
              </SimpleGrid>
            </Section>

            <Section
              title="Time"
              description="Utilization is the share of working hours actually spent with paying clients. Breaks, cleanup between clients, gaps and admin all lower it."
            >
              <SimpleGrid cols={3}>
                <DecimalInput label="Hours worked per week" unit="h" value={data.weekly_hours} onChange={(v) => set("weekly_hours", v)} error={fieldError("weekly_hours")} />
                <DecimalInput
                  label="Weeks worked per year"
                  description="After vacation and holidays"
                  unit="wk"
                  value={data.weeks_per_year}
                  onChange={(v) => set("weeks_per_year", v)}
                  error={fieldError("weeks_per_year")}
                />
                <DecimalInput
                  label="Realistic billable utilization"
                  unit="%"
                  value={data.utilization_pct}
                  onChange={(v) => set("utilization_pct", v)}
                  error={fieldError("utilization_pct")}
                />
              </SimpleGrid>
              <Radio.Group
                mt="md"
                label="Overhead is charged for"
                value={data.overhead_basis}
                onChange={(v) => set("overhead_basis", v as ProfileData["overhead_basis"])}
              >
                <Stack gap={6} mt={6}>
                  {Object.entries(basisLabels).map(([value, label]) => (
                    <Radio key={value} value={value} label={label} />
                  ))}
                </Stack>
              </Radio.Group>
            </Section>

            <Section
              title="Monthly overhead"
              description="Enter the share of monthly costs this profile should carry. For a chair renter, chair or booth rent goes in rent."
            >
              <SimpleGrid cols={3}>
                <DecimalInput label={data.kind === "chair_renter" ? "Chair / booth rent" : "Rent"} unit="$" value={data.overhead.rent} onChange={(v) => setOh("rent", v)} error={fieldError("overhead.rent")} />
                <DecimalInput label="Utilities" unit="$" value={data.overhead.utilities} onChange={(v) => setOh("utilities", v)} error={fieldError("overhead.utilities")} />
                <DecimalInput label="Insurance" unit="$" value={data.overhead.insurance} onChange={(v) => setOh("insurance", v)} error={fieldError("overhead.insurance")} />
                <DecimalInput label="Software" unit="$" value={data.overhead.software} onChange={(v) => setOh("software", v)} error={fieldError("overhead.software")} />
                <DecimalInput label="Other" unit="$" value={data.overhead.other} onChange={(v) => setOh("other", v)} error={fieldError("overhead.other")} />
                <TextInput label="What “other” covers" placeholder="e.g. Laundry, education" value={data.overhead.other_label} onChange={(e) => setOh("other_label", e.currentTarget.value)} />
              </SimpleGrid>
            </Section>

            <Section
              title="Payment fees"
              description="Card processing fees are charged on what the client pays, including tax. Leave at 0 if you don't take cards."
            >
              <SimpleGrid cols={3}>
                <DecimalInput label="Processing rate" unit="%" value={data.processing_pct} onChange={(v) => set("processing_pct", v)} error={fieldError("processing_pct")} />
                <DecimalInput label="Per-payment fee" unit="$" value={data.processing_fixed} onChange={(v) => set("processing_fixed", v)} error={fieldError("processing_fixed")} />
                <DecimalInput
                  label="Share paid by card"
                  unit="%"
                  value={data.card_share_pct}
                  onChange={(v) => set("card_share_pct", v)}
                  error={fieldError("card_share_pct")}
                />
              </SimpleGrid>
            </Section>

            <Section title="Pricing defaults" description="Services start from these settings. You can override them for individual services.">
              <SimpleGrid cols={2}>
                <div>
                  <Text size="sm" fw={500} mb={4}>
                    Profit target
                  </Text>
                  <SegmentedControl
                    fullWidth
                    value={data.target_kind}
                    onChange={(v) => set("target_kind", v as ProfileData["target_kind"])}
                    data={[
                      { value: "margin", label: "Margin (share of price)" },
                      { value: "markup", label: "Markup (share of cost)" },
                    ]}
                  />
                </div>
                <DecimalInput
                  label={data.target_kind === "margin" ? "Target margin" : "Target markup"}
                  unit="%"
                  value={data.target_pct}
                  onChange={(v) => set("target_pct", v)}
                  error={fieldError("target_pct")}
                />
                <Select
                  label="Market position"
                  description="Used when you have observed local prices"
                  data={Object.entries(positionLabels).map(([value, p]) => ({ value, label: `${p.label} — ${p.description}` }))}
                  value={data.position}
                  allowDeselect={false}
                  onChange={(v) => set("position", v as ProfileData["position"])}
                />
                <Group grow align="flex-end">
                  <Select
                    label="Round prices to"
                    data={["0.01", "0.25", "0.5", "1", "5", "10"].map((v) => ({ value: v, label: v === "0.01" ? "Cents" : `$${v}` }))}
                    value={data.rounding_increment}
                    allowDeselect={false}
                    onChange={(v) => set("rounding_increment", v ?? "1")}
                    error={fieldError("rounding_increment")}
                  />
                  <Select
                    aria-label="Rounding direction"
                    data={Object.entries(roundLabels).map(([value, label]) => ({ value, label }))}
                    value={data.rounding_mode}
                    allowDeselect={false}
                    onChange={(v) => set("rounding_mode", v as ProfileData["rounding_mode"])}
                  />
                </Group>
              </SimpleGrid>
              <Textarea mt="md" label="Notes" autosize minRows={2} value={data.notes} onChange={(e) => set("notes", e.currentTarget.value)} />
            </Section>
          </Stack>
        </Grid.Col>
        <Grid.Col span={{ base: 12, lg: 4.5 }}>
          <RatesPanel rates={previewError ? undefined : preview.data} error={previewError} />
        </Grid.Col>
      </Grid>

      <Drawer opened={historyOpen} onClose={history.close} title="Version history" position="right">
        <Text size="sm" c="dimmed" mb="md">
          Each save adds a version. Completed sales keep the figures they were costed with, so editing this profile never changes past results.
        </Text>
        <Stack gap={6}>
          {(versions.data ?? []).map((v) => (
            <Group key={v.version_id} justify="space-between">
              <Text size="sm" fw={600}>
                Version {v.version}
              </Text>
              <Text size="sm" c="dimmed">
                {isoToLocal(v.created_at)}
              </Text>
            </Group>
          ))}
        </Stack>
        <Text size="xs" c="dimmed" mt="lg">
          {num(String(versions.data?.length ?? 0), 0)} versions
        </Text>
      </Drawer>
    </>
  );
}
