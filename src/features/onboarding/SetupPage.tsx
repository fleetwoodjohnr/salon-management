import { Alert, Button, Group, Paper, Select, SimpleGrid, Stack, Stepper, Text, TextInput } from "@mantine/core";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { call, isAppError, type AppError } from "../../api/ipc";
import { useCmd } from "../../api/queries";
import type { AppStatus, Business, Location, ProfileData, ProfileView, Staff } from "../../api/types";
import { STATUS_KEY } from "../../app/App";
import { DecimalInput } from "../../components/DecimalInput";
import { PageHeader } from "../../components/PageHeader";
import { defaultProfile } from "../profiles/ProfileEditor";
import { compLabels, kindLabels } from "../profiles/labels";
import { BusinessFields, emptyLocation, LocationFields, ScheduleFields } from "../settings/forms";


export function SetupPage() {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const biz = useCmd<Business>("business_get");
  const { data: status } = useQuery<AppStatus>({ queryKey: STATUS_KEY, enabled: false });
  const locs = useCmd<Location[]>("locations_list");
  const profiles = useCmd<ProfileView[]>("profiles_list", { includeArchived: false });
  const [active, setActive] = useState(0);
  const [business, setBusiness] = useState<Business | null>(null);
  const [location, setLocation] = useState<Location>(emptyLocation());
  const [profileName, setProfileName] = useState("Me");
  const [profile, setProfile] = useState<ProfileData>(defaultProfile(null));
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (biz.data && !business) {
      const fresh = !biz.data.name;
      setBusiness({
        ...biz.data,
        name: fresh ? (status?.current?.name ?? "") : biz.data.name,
        timezone: fresh ? Intl.DateTimeFormat().resolvedOptions().timeZone || biz.data.timezone : biz.data.timezone,
      });
    }
  }, [biz.data, business, status]);
  useEffect(() => {
    const primary = locs.data?.find((l) => l.id === biz.data?.primary_location_id);
    if (primary) setLocation(primary);
  }, [locs.data, biz.data]);

  if (!business) return null;
  const err = (f: string) => (error?.field === f ? error.message : undefined);

  async function step(fn: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await fn();
      setActive((a) => a + 1);
    } catch (e) {
      setError(isAppError(e) ? e : { kind: "other", message: String(e), field: null });
    } finally {
      setBusy(false);
    }
  }

  async function saveBusiness(b: Business) {
    const saved = await call<Business>("business_save", { business: b });
    setBusiness(saved);
    qc.invalidateQueries({ queryKey: ["business_get"] });
    return saved;
  }

  const skip = (name: string) => () => {
    setBusiness((b) => (b && !b.onboarding_skipped.includes(name) ? { ...b, onboarding_skipped: [...b.onboarding_skipped, name] } : b));
    setActive((a) => a + 1);
  };

  async function finish() {
    await saveBusiness({ ...business!, onboarding_completed: true });
    navigate("/");
  }

  const nameMissing = !business.name.trim();

  return (
    <>
      <PageHeader title="Set up your business" description="Four short steps. Skip anything you don't have to hand; you can finish it later in Settings and Work profiles." />
      <Paper p="xl" maw={1040}>
        <Stepper size="sm" active={active} onStepClick={(i) => !nameMissing && setActive(i)} allowNextStepsSelect={false}>
          <Stepper.Step label="Business" description="Name and preferences">
            <BusinessFields value={business} onChange={setBusiness} error={err} />
            <Group justify="flex-end" mt="xl">
              <Button loading={busy} disabled={nameMissing} onClick={() => step(async () => void (await saveBusiness(business)))}>
                Save and continue
              </Button>
            </Group>
          </Stepper.Step>
          <Stepper.Step label="Location" description="Where you work">
            <LocationFields value={location} onChange={setLocation} error={err} />
            <Group justify="space-between" mt="xl">
              <Button variant="subtle" onClick={skip("location")}>
                Skip for now
              </Button>
              <Button
                loading={busy}
                onClick={() =>
                  step(async () => {
                    const saved = await call<Location>("location_save", { location });
                    setLocation(saved);
                    setProfile((p) => ({ ...p, location_id: saved.id }));
                    await saveBusiness({ ...business, primary_location_id: saved.id });
                    qc.invalidateQueries({ queryKey: ["locations_list"] });
                  })
                }
              >
                Save and continue
              </Button>
            </Group>
          </Stepper.Step>
          <Stepper.Step label="Hours" description="Your working week">
            <Text size="sm" c="dimmed" mb="md">
              Opening hours set the default calendar view and help catch bookings outside working time.
            </Text>
            <ScheduleFields value={business.schedule} onChange={(schedule) => setBusiness({ ...business, schedule })} error={err} />
            <Group justify="space-between" mt="xl">
              <Button variant="subtle" onClick={skip("hours")}>
                Skip for now
              </Button>
              <Button loading={busy} onClick={() => step(async () => void (await saveBusiness(business)))}>
                Save and continue
              </Button>
            </Group>
          </Stepper.Step>
          <Stepper.Step label="Work profile" description="How your time is costed">
            {(profiles.data?.length ?? 0) > 0 ? (
              <Alert color="teal" mb="md">
                You already have {profiles.data!.length} work profile{profiles.data!.length > 1 ? "s" : ""}. You can add more any time.
              </Alert>
            ) : null}
            <Stack>
              <SimpleGrid cols={2}>
                <TextInput label="Profile name" value={profileName} onChange={(e) => setProfileName(e.currentTarget.value)} error={err("name")} />
                <Select
                  label="Type"
                  data={Object.entries(kindLabels).map(([value, label]) => ({ value, label }))}
                  value={profile.kind}
                  allowDeselect={false}
                  onChange={(v) => setProfile({ ...profile, kind: v as ProfileData["kind"], comp_model: v === "employee" ? "hourly_wage" : "owner_target_hourly" })}
                />
                <Select
                  label="Paid by"
                  data={Object.entries(compLabels).map(([value, c]) => ({ value, label: c.label }))}
                  value={profile.comp_model}
                  allowDeselect={false}
                  onChange={(v) => setProfile({ ...profile, comp_model: v as ProfileData["comp_model"] })}
                />
                {profile.comp_model === "commission" ? (
                  <DecimalInput label="Service commission" unit="%" value={profile.commission_pct} onChange={(v) => setProfile({ ...profile, commission_pct: v })} error={err("commission_pct")} />
                ) : (
                  <DecimalInput
                    label={profile.comp_model === "owner_target_hourly" ? "Desired pay per hour worked" : "Hourly wage"}
                    unit="$"
                    value={profile.hourly_rate}
                    onChange={(v) => setProfile({ ...profile, hourly_rate: v })}
                    error={err("hourly_rate")}
                  />
                )}
                <DecimalInput label="Hours worked per week" unit="h" value={profile.weekly_hours} onChange={(v) => setProfile({ ...profile, weekly_hours: v })} error={err("weekly_hours")} />
                <DecimalInput
                  label="Realistic billable utilization"
                  description="Share of working time spent with paying clients"
                  unit="%"
                  value={profile.utilization_pct}
                  onChange={(v) => setProfile({ ...profile, utilization_pct: v })}
                  error={err("utilization_pct")}
                />
                <DecimalInput
                  label={profile.kind === "chair_renter" ? "Monthly chair or booth rent" : "Monthly rent share"}
                  unit="$"
                  value={profile.overhead.rent}
                  onChange={(v) => setProfile({ ...profile, overhead: { ...profile.overhead, rent: v } })}
                  error={err("overhead.rent")}
                />
                <DecimalInput
                  label="Other monthly overhead"
                  description="Utilities, insurance, software…"
                  unit="$"
                  value={profile.overhead.other}
                  onChange={(v) => setProfile({ ...profile, overhead: { ...profile.overhead, other: v } })}
                  error={err("overhead.other")}
                />
              </SimpleGrid>
              <Text size="xs" c="dimmed">
                Fees, pricing targets and a full overhead breakdown are in Work profiles.
              </Text>
            </Stack>
            {error && !error.field && (
              <Alert color="red" mt="md">
                {error.message}
              </Alert>
            )}
            <Group justify="space-between" mt="xl">
              <Button variant="subtle" onClick={skip("profile")}>
                Skip for now
              </Button>
              <Button
                loading={busy}
                onClick={() =>
                  step(async () => {
                    const saved = await call<ProfileView>("profile_save", { id: null, name: profileName, data: profile });
                    // The calendar books staff, so start with one person using this profile.
                    const staff = await call<Staff[]>("staff_list");
                    if (staff.length === 0) {
                      await call("staff_save", { staff: { id: null, name: profileName, color: "grape", default_profile_id: saved.id, location_id: profile.location_id, archived: false } });
                    }
                    qc.invalidateQueries({ queryKey: ["profiles_list"] });
                    qc.invalidateQueries({ queryKey: ["staff_list"] });
                  })
                }
              >
                Create profile
              </Button>
            </Group>
          </Stepper.Step>
          <Stepper.Completed>
            <Stack align="flex-start">
              <Text fw={600}>You're set up.</Text>
              <Text size="sm" c="dimmed" maw={560}>
                Next, receive your first products in Inventory, then build a service with its recipe to see its real cost.
                {business.onboarding_skipped.length > 0 && ` You skipped: ${business.onboarding_skipped.join(", ")}. Finish these in Settings when you're ready.`}
              </Text>
              <Button onClick={finish} loading={busy}>
                Go to dashboard
              </Button>
            </Stack>
          </Stepper.Completed>
        </Stepper>
      </Paper>
    </>
  );
}
