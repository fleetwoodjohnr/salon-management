import { Alert, Box, Button, Divider, Group, Paper, SimpleGrid, Stack, Text, TextInput, Title } from "@mantine/core";
import { IconArchive, IconBuildingStore, IconFlask } from "@tabler/icons-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { call, errorMessage } from "../../api/ipc";
import type { AppStatus } from "../../api/types";
import { useApplyStatus } from "../../app/App";

export function Welcome({ status }: { status: AppStatus }) {
  const apply = useApplyStatus();
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(status.open_error);

  async function run(fn: () => Promise<AppStatus>) {
    setBusy(true);
    setError(null);
    try {
      apply(await fn());
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function restore() {
    const path = await open({ title: "Choose a backup to restore", filters: [{ name: "Backup", extensions: ["zip"] }] });
    if (typeof path === "string") run(() => call<AppStatus>("backup_restore", { path, replaceCurrent: false }));
  }

  const business = status.workspaces.filter((w) => w.kind === "business");
  const demo = status.workspaces.find((w) => w.kind === "demo");

  return (
    <Box mih="100%" p={48} style={{ background: "var(--srm-canvas)" }}>
      <SimpleGrid cols={{ base: 1, md: 2 }} spacing={64} maw={1080} mx="auto" mt="6vh">
        <Stack gap="lg">
          <h1 className="srm-page-title" style={{ fontSize: "2.75rem", margin: 0 }}>
            Salon Resource Manager
          </h1>
          <Text size="lg" c="var(--srm-ink-soft)" maw={460}>
            Know what every service really costs, price it with confidence, and keep stock, sales and tax in order. Everything
            stays on this computer.
          </Text>
          {error && (
            <Alert color="red" title="Something went wrong">
              {error}
            </Alert>
          )}
          <Paper p="xl" component="form" onSubmit={(e) => (e.preventDefault(), run(() => call<AppStatus>("workspace_create", { name })))}>
            <Stack>
              <Title order={3}>Set up your business</Title>
              <TextInput
                label="Business name"
                placeholder="e.g. Rowan & Ash Hair Studio"
                value={name}
                onChange={(e) => setName(e.currentTarget.value)}
                required
                data-autofocus
              />
              <Button type="submit" loading={busy} disabled={!name.trim()} leftSection={<IconBuildingStore size={18} />}>
                Create workspace
              </Button>
              <Text size="xs" c="dimmed">
                No account or internet connection needed. You can add more businesses later; each workspace is kept separately.
              </Text>
            </Stack>
          </Paper>
        </Stack>
        <Stack gap="md" pt={8}>
          {business.length > 0 && (
            <Paper p="lg">
              <Title order={4} mb="sm">
                Open a workspace
              </Title>
              <Stack gap={4}>
                {business.map((w) => (
                  <Group key={w.id} justify="space-between">
                    <Text>{w.name}</Text>
                    <Button variant="light" size="xs" onClick={() => run(() => call<AppStatus>("workspace_open", { id: w.id }))}>
                      Open
                    </Button>
                  </Group>
                ))}
              </Stack>
            </Paper>
          )}
          <Paper p="lg">
            <Group gap="sm" align="flex-start" wrap="nowrap">
              <IconFlask size={22} style={{ flexShrink: 0, marginTop: 2 }} />
              <Stack gap={6}>
                <Title order={4}>Look around with demo data</Title>
                <Text size="sm" c="dimmed">
                  A separate, clearly labelled workspace filled with a sample salon. It never mixes with your own records.
                </Text>
                <Group>
                  <Button
                    variant="default"
                    size="xs"
                    loading={busy}
                    onClick={() => run(() => (demo ? call<AppStatus>("workspace_open", { id: demo.id }) : call<AppStatus>("demo_create")))}
                  >
                    {demo ? "Open demo workspace" : "Create demo workspace"}
                  </Button>
                </Group>
              </Stack>
            </Group>
          </Paper>
          <Paper p="lg">
            <Group gap="sm" align="flex-start" wrap="nowrap">
              <IconArchive size={22} style={{ flexShrink: 0, marginTop: 2 }} />
              <Stack gap={6}>
                <Title order={4}>Restore from a backup</Title>
                <Text size="sm" c="dimmed">
                  The backup is checked before anything is restored, and it opens as a new workspace.
                </Text>
                <Group>
                  <Button variant="default" size="xs" onClick={restore} loading={busy}>
                    Choose backup file…
                  </Button>
                </Group>
              </Stack>
            </Group>
          </Paper>
          <Divider />
          <Stack gap={2}>
            <Text size="xs" c="dimmed">
              Data folder: {status.data_dir}
            </Text>
            <Text size="xs" c="dimmed">
              Version {status.app_version}
            </Text>
          </Stack>
        </Stack>
      </SimpleGrid>
    </Box>
  );
}
