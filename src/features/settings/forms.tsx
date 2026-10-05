// Business, location and schedule forms shared by onboarding and Settings.
import { Checkbox, Group, Select, SimpleGrid, Stack, Switch, Text, TextInput } from "@mantine/core";
import type { Business, DaySchedule, Location } from "../../api/types";

export const DAY_NAMES = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

const timezones: string[] = (() => {
  try {
    return (Intl as unknown as { supportedValuesOf(k: string): string[] }).supportedValuesOf("timeZone");
  } catch {
    return ["America/New_York", "America/Chicago", "America/Denver", "America/Los_Angeles", "America/Anchorage", "Pacific/Honolulu"];
  }
})();
const usFirst = [...new Set([...timezones.filter((t) => t.startsWith("America/") || t === "Pacific/Honolulu"), ...timezones])];

export const US_STATES = "AL AK AZ AR CA CO CT DE DC FL GA HI ID IL IN IA KS KY LA ME MD MA MI MN MS MO MT NE NV NH NJ NM NY NC ND OH OK OR PA RI SC SD TN TX UT VT VA WA WV WI WY PR".split(" ");

export function emptyLocation(): Location {
  return {
    id: null,
    name: "Main location",
    address_line: "",
    city: "",
    state: "",
    postal_code: "",
    latitude: null,
    longitude: null,
    geo_precision: null,
    geo_source: null,
    state_fips: null,
    county_fips: null,
    place_fips: null,
    archived: false,
  };
}

type Err = (field: string) => string | undefined;

export function BusinessFields({ value, onChange, error }: { value: Business; onChange: (b: Business) => void; error: Err }) {
  return (
    <Stack>
      <TextInput label="Business name" required value={value.name} onChange={(e) => onChange({ ...value, name: e.currentTarget.value })} error={error("name")} />
      <SimpleGrid cols={2}>
        <Select
          label="Time zone"
          searchable
          data={usFirst}
          value={value.timezone}
          allowDeselect={false}
          onChange={(v) => onChange({ ...value, timezone: v ?? value.timezone })}
        />
        <Select
          label="Preferred units"
          description="Defaults for new products; you can always enter any unit"
          data={[
            { value: "us", label: "US (oz, fl oz, lb, gal)" },
            { value: "metric", label: "Metric (g, ml, kg, L)" },
          ]}
          value={value.units}
          allowDeselect={false}
          onChange={(v) => onChange({ ...value, units: (v as Business["units"]) ?? "us" })}
          error={error("units")}
        />
      </SimpleGrid>
      <Switch
        label="My listed prices already include sales tax"
        description="Leave off for the usual US practice of adding tax at checkout."
        checked={value.prices_include_tax}
        onChange={(e) => onChange({ ...value, prices_include_tax: e.currentTarget.checked })}
      />
    </Stack>
  );
}

export function LocationFields({ value, onChange, error }: { value: Location; onChange: (l: Location) => void; error: Err }) {
  const set = (k: keyof Location, v: string) => onChange({ ...value, [k]: v });
  return (
    <Stack>
      <TextInput label="Location name" required value={value.name} onChange={(e) => set("name", e.currentTarget.value)} error={error("name")} />
      <TextInput label="Street address" placeholder="Optional, but needed for address-level tax lookups" value={value.address_line} onChange={(e) => set("address_line", e.currentTarget.value)} />
      <SimpleGrid cols={3}>
        <TextInput label="City" value={value.city} onChange={(e) => set("city", e.currentTarget.value)} />
        <Select label="State" searchable data={US_STATES} value={value.state || null} onChange={(v) => set("state", v ?? "")} error={error("state")} />
        <TextInput label="ZIP code" placeholder="12345" value={value.postal_code} onChange={(e) => set("postal_code", e.currentTarget.value)} error={error("postal_code")} />
      </SimpleGrid>
      <Text size="xs" c="dimmed">
        Your address stays on this computer. It's sent to a government lookup service only when you ask for a tax rate or location lookup.
      </Text>
    </Stack>
  );
}

export function ScheduleFields({ value, onChange, error }: { value: DaySchedule[]; onChange: (s: DaySchedule[]) => void; error: Err }) {
  const set = (i: number, patch: Partial<DaySchedule>) => onChange(value.map((d, j) => (j === i ? { ...d, ...patch } : d)));
  return (
    <Stack gap="xs">
      {value.map((d, i) => (
        <Group key={d.day} gap="md" wrap="nowrap">
          <Checkbox w={140} label={DAY_NAMES[d.day]} checked={d.open} onChange={(e) => set(i, { open: e.currentTarget.checked })} />
          <TextInput type="time" aria-label={`${DAY_NAMES[d.day]} opens`} w={130} disabled={!d.open} value={d.start} onChange={(e) => set(i, { start: e.currentTarget.value })} />
          <Text size="sm" c="dimmed">
            to
          </Text>
          <TextInput type="time" aria-label={`${DAY_NAMES[d.day]} closes`} w={130} disabled={!d.open} value={d.end} onChange={(e) => set(i, { end: e.currentTarget.value })} />
          {!d.open && (
            <Text size="sm" c="dimmed">
              Closed
            </Text>
          )}
        </Group>
      ))}
      {error("schedule") && (
        <Text size="sm" c="red">
          {error("schedule")}
        </Text>
      )}
    </Stack>
  );
}
