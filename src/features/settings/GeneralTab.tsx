import { Paper, SegmentedControl, Stack, Text, Title } from "@mantine/core";
import { useAction } from "../../api/queries";
import type { DistanceUnit, Prefs } from "../../api/types";
import { QueryState } from "../../components/QueryState";
import { usePrefs } from "../../lib/distance";

/** App-wide preferences shared by every workspace. */
export function GeneralTab() {
  const q = usePrefs();
  const save = useAction<{ prefs: Prefs }, Prefs>("prefs_set", { invalidate: ["prefs_get", "market_evidence", "market_overview"], success: "Preference saved" });
  return (
    <QueryState loading={q.isLoading} error={q.error}>
      {() => (
        <Paper p="lg" maw={720}>
          <Stack gap="xs">
            <Title order={4}>Units</Title>
            <Text size="sm" c="dimmed">
              Distance unit for search radius, competitor distances and market filters.
            </Text>
            <SegmentedControl
              w={320}
              aria-label="Distance unit"
              data={[
                { value: "mi", label: "Miles (mi)" },
                { value: "km", label: "Kilometers (km)" },
              ]}
              value={q.data!.distance_unit}
              disabled={save.isPending}
              onChange={(v) => save.mutate({ prefs: { ...q.data!, distance_unit: v as DistanceUnit } })}
            />
            <Text size="xs" c="dimmed">
              Distances are stored exactly (in kilometres) and only converted for display, so switching units never changes a saved search area. 1 mi = 1.609344 km.
            </Text>
          </Stack>
        </Paper>
      )}
    </QueryState>
  );
}
