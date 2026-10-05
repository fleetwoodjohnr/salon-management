import { Alert, Anchor, Badge, Button, Group, Paper, PasswordInput, SimpleGrid, Stack, Text, Title } from "@mantine/core";
import { IconCloudOff, IconKey } from "@tabler/icons-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useState } from "react";
import { isAppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { ProviderStatus } from "../../api/types";
import { QueryState } from "../../components/QueryState";
import { isoToLocal } from "../../lib/format";

function KeyForm({ p }: { p: ProviderStatus }) {
  const [key, setKey] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const set = useAction<{ provider: string; key: string }>("provider_key_set", { invalidate: ["providers_status"], success: "Key saved in the system keyring", silentError: true });
  const clear = useAction<{ provider: string }>("provider_key_clear", { invalidate: ["providers_status"], success: "Key removed" });
  if (p.key_error) return <Alert color="yellow">{p.key_error}</Alert>;
  return p.key_configured ? (
    <Group gap="xs">
      <Badge color="teal" leftSection={<IconKey size={12} />}>
        Key saved in system keyring
      </Badge>
      <Button size="compact-xs" variant="subtle" color="gray" onClick={() => clear.mutate({ provider: p.info.id })}>
        Remove
      </Button>
    </Group>
  ) : (
    <Group gap="xs" align="flex-start">
      <PasswordInput
        size="xs"
        w={260}
        aria-label={`${p.info.name} API key`}
        placeholder={p.info.key!.required ? "Paste your free API key" : "Optional API key"}
        value={key}
        onChange={(e) => setKey(e.currentTarget.value)}
        error={err}
      />
      <Button size="xs" disabled={!key.trim()} loading={set.isPending} onClick={() => set.mutate({ provider: p.info.id, key }, { onSuccess: () => (setKey(""), setErr(null)), onError: (e) => setErr(isAppError(e) ? e.message : String(e)) })}>
        Save key
      </Button>
      <Anchor size="xs" mt={6} onClick={() => openUrl(p.info.key!.signup_url)}>
        Get a free key
      </Anchor>
    </Group>
  );
}

export function ProvidersTab() {
  const q = useCmd<ProviderStatus[]>("providers_status");
  return (
    <Stack maw={1100}>
      <Alert color="blue" icon={<IconCloudOff size={18} />}>
        Everything in this app works offline. These free services are only contacted when you press a lookup button, and only with the minimum needed (an address, ZIP or area
        code) — never client records. Keys are kept in your operating system's keyring, not in the workspace, backups or exports.
      </Alert>
      <QueryState loading={q.isLoading} error={q.error}>
        {() => (
          <SimpleGrid cols={{ base: 1, lg: 2 }} spacing="lg">
            {q.data!.map((p) => (
              <Paper key={p.info.id} p="lg">
                <Group justify="space-between" align="flex-start" mb={4}>
                  <Title order={5} maw="75%">
                    {p.info.name}
                  </Title>
                  <Badge color="gray">{p.info.purpose}</Badge>
                </Group>
                <Text size="sm" mb="sm">
                  {p.info.gives}
                </Text>
                <Stack gap={2} mb="sm">
                  <Text size="xs" c="dimmed">
                    Coverage: {p.info.coverage}
                  </Text>
                  <Text size="xs" c="dimmed">
                    Limits: {p.info.limits}
                  </Text>
                  <Text size="xs" c="dimmed">
                    Terms: {p.info.license}
                  </Text>
                  <Anchor size="xs" onClick={() => openUrl(p.info.docs_url)}>
                    Provider documentation
                  </Anchor>
                </Stack>
                {p.info.key && <KeyForm p={p} />}
                <Group gap="lg" mt="sm">
                  <Text size="xs">
                    Today: {p.requests_today} of {p.info.daily_cap} lookups{p.errors_today ? `, ${p.errors_today} failed` : ""}
                  </Text>
                  <Text size="xs" c="dimmed">
                    Cached results: {p.cached_responses}
                    {p.newest_cache ? `, newest ${isoToLocal(p.newest_cache)}` : ""}
                  </Text>
                </Group>
                <Text size="xs" c="dimmed">
                  Last success: {p.last_success ? isoToLocal(p.last_success) : "never"}
                </Text>
                {p.last_error && (
                  <Text size="xs" c="var(--srm-bad)">
                    Last error ({isoToLocal(p.last_error_at)}): {p.last_error}
                  </Text>
                )}
              </Paper>
            ))}
          </SimpleGrid>
        )}
      </QueryState>
      <Paper p="lg">
        <Title order={5} mb={4}>
          Not available as a free service
        </Title>
        <Text size="sm" c="dimmed">
          No free API offers actual salon menu prices: map and review platforms either don't include prices or require paid billing accounts. Competitor prices are therefore
          collected by you (typed in or imported from a spreadsheet) on the Market page, with their source and date. Sales tax lookups exist only where a state publishes a free
          official service (currently Washington and California in this app); elsewhere you enter the rate from your state revenue department.
        </Text>
      </Paper>
    </Stack>
  );
}
