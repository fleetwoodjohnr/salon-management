import { Alert, Badge, Button, ColorSwatch, Group, Modal, Paper, Select, Stack, Table, Tabs, Text, TextInput, Title } from "@mantine/core";
import { modals } from "@mantine/modals";
import { notifications } from "@mantine/notifications";
import { IconDownload, IconPlus, IconRestore } from "@tabler/icons-react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { useSearchParams } from "react-router";
import { call, errorMessage, isAppError, type AppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { AppStatus, AuditEntry, BackupInfo, Business, Location, ProfileView, Staff } from "../../api/types";
import { useApplyStatus } from "../../app/App";
import { useDirty } from "../../components/dirty";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { isoToLocal } from "../../lib/format";
import { BusinessFields, emptyLocation, LocationFields, ScheduleFields } from "./forms";
import { ProvidersTab } from "./ProvidersTab";

function useFieldErrors() {
  const [error, setError] = useState<AppError | null>(null);
  return {
    error,
    setError: (e: unknown) => setError(e == null ? null : isAppError(e) ? e : { kind: "other", message: String(e), field: null }),
    err: (f: string) => (error?.field === f ? error.message : undefined),
  };
}

function BusinessTab() {
  const q = useCmd<Business>("business_get");
  const [b, setB] = useState<Business | null>(null);
  const base = useRef("");
  const fe = useFieldErrors();
  useEffect(() => {
    if (q.data) {
      setB(q.data);
      base.current = JSON.stringify(q.data);
    }
  }, [q.data]);
  const dirty = !!b && JSON.stringify(b) !== base.current;
  useDirty("settings-business", dirty);
  const saveB = useAction<{ business: Business }, Business>("business_save", { invalidate: ["business_get"], success: "Business settings saved", silentError: true });
  if (!b) return null;
  return (
    <Stack maw={760}>
      <Paper p="lg">
        <BusinessFields value={b} onChange={setB} error={fe.err} />
      </Paper>
      <Paper p="lg">
        <Title order={4} mb="md">
          Opening hours
        </Title>
        <ScheduleFields value={b.schedule} onChange={(schedule) => setB({ ...b, schedule })} error={fe.err} />
      </Paper>
      {fe.error && !fe.error.field && <Alert color="red">{fe.error.message}</Alert>}
      <Group justify="flex-end">
        <Badge color={dirty ? "yellow" : "gray"} size="lg">
          {dirty ? "Unsaved changes" : "Saved"}
        </Badge>
        <Button disabled={!dirty} loading={saveB.isPending} onClick={() => saveB.mutate({ business: b }, { onError: fe.setError, onSuccess: () => fe.setError(null) })}>
          Save settings
        </Button>
      </Group>
    </Stack>
  );
}

function LocationsTab() {
  const q = useCmd<Location[]>("locations_list");
  const biz = useCmd<Business>("business_get");
  const [editing, setEditing] = useState<Location | null>(null);
  const fe = useFieldErrors();
  const saveL = useAction<{ location: Location }, Location>("location_save", { invalidate: ["locations_list"], success: "Location saved", silentError: true });
  const archive = useAction<{ id: number; archived: boolean }>("location_archive", { invalidate: ["locations_list"] });
  const geocode = useAction<{ locationId: number }, Location>("location_geocode", {
    invalidate: ["locations_list"],
    success: (l) => `Located: ${l.geo_source ?? l.name}`,
  });
  return (
    <QueryState loading={q.isLoading} error={q.error}>
      {() => (
        <Stack maw={900}>
          <Group justify="flex-end">
            <Button leftSection={<IconPlus size={16} />} onClick={() => (fe.setError(null), setEditing({ ...emptyLocation(), name: "" }))}>
              Add location
            </Button>
          </Group>
          <Paper p={0}>
            <Table>
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>Name</Table.Th>
                  <Table.Th>Address</Table.Th>
                  <Table.Th>Geography</Table.Th>
                  <Table.Th />
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {q.data!.length === 0 && (
                  <Table.Tr>
                    <Table.Td colSpan={4}>
                      <Text c="dimmed" size="sm" py="md" ta="center">
                        No locations yet. Add where you work to enable tax and local market lookups.
                      </Text>
                    </Table.Td>
                  </Table.Tr>
                )}
                {q.data!.map((l) => (
                  <Table.Tr key={l.id} style={{ opacity: l.archived ? 0.55 : 1 }}>
                    <Table.Td>
                      <Group gap="xs">
                        <Text size="sm" fw={600}>
                          {l.name}
                        </Text>
                        {biz.data?.primary_location_id === l.id && <Badge>Primary</Badge>}
                        {l.archived && <Badge color="gray">Archived</Badge>}
                      </Group>
                    </Table.Td>
                    <Table.Td>
                      <Text size="sm">{[l.address_line, l.city, [l.state, l.postal_code].filter(Boolean).join(" ")].filter(Boolean).join(", ") || "—"}</Text>
                    </Table.Td>
                    <Table.Td>
                      <Text size="sm" c="dimmed">
                        {l.geo_precision
                          ? `${{ address: "Address match", zcta: "ZIP area centre", place: "City centre", manual: "Entered" }[l.geo_precision]}${l.county_fips ? `, County ${l.county_fips}` : ""} (${l.latitude?.slice(0, 7)}, ${l.longitude?.slice(0, 8)})`
                          : "Not located"}
                      </Text>
                    </Table.Td>
                    <Table.Td>
                      <Group gap="xs" justify="flex-end">
                        <Button size="xs" variant="default" onClick={() => (fe.setError(null), setEditing(l))}>
                          Edit
                        </Button>
                        <Button size="xs" variant="subtle" loading={geocode.isPending && geocode.variables?.locationId === l.id} onClick={() => geocode.mutate({ locationId: l.id! })}>
                          Locate
                        </Button>
                        <Button size="xs" variant="subtle" color="gray" onClick={() => archive.mutate({ id: l.id!, archived: !l.archived })}>
                          {l.archived ? "Restore" : "Archive"}
                        </Button>
                      </Group>
                    </Table.Td>
                  </Table.Tr>
                ))}
              </Table.Tbody>
            </Table>
          </Paper>
          <Modal opened={!!editing} onClose={() => setEditing(null)} title={editing?.id ? "Edit location" : "Add location"} size="lg">
            {editing && (
              <form
                onSubmit={(e) => {
                  e.preventDefault();
                  saveL.mutate({ location: editing }, { onSuccess: () => setEditing(null), onError: fe.setError });
                }}
              >
                <LocationFields value={editing} onChange={setEditing} error={fe.err} />
                {fe.error && !fe.error.field && (
                  <Alert color="red" mt="md">
                    {fe.error.message}
                  </Alert>
                )}
                <Group justify="flex-end" mt="lg">
                  <Button variant="default" onClick={() => setEditing(null)}>
                    Cancel
                  </Button>
                  <Button type="submit" loading={saveL.isPending}>
                    Save location
                  </Button>
                </Group>
              </form>
            )}
          </Modal>
        </Stack>
      )}
    </QueryState>
  );
}

const STAFF_COLORS = ["grape", "teal", "orange", "indigo", "pink", "lime", "cyan", "red", "yellow", "blue"];

function StaffTab() {
  const q = useCmd<Staff[]>("staff_list");
  const profiles = useCmd<ProfileView[]>("profiles_list", { includeArchived: false });
  const locs = useCmd<Location[]>("locations_list");
  const [editing, setEditing] = useState<Staff | null>(null);
  const fe = useFieldErrors();
  const saveS = useAction<{ staff: Staff }, number>("staff_save", { invalidate: ["staff_list"], success: "Staff saved", silentError: true });
  const archive = useAction<{ id: number; archived: boolean }>("staff_archive", { invalidate: ["staff_list"] });
  const profName = (id: number | null) => profiles.data?.find((p) => p.id === id)?.name ?? "—";
  return (
    <QueryState loading={q.isLoading} error={q.error}>
      {() => (
        <Stack maw={900}>
          <Text size="sm" c="dimmed">
            Staff appear as columns in the calendar. Each person's default work profile decides how their services are costed.
          </Text>
          <Group justify="flex-end">
            <Button
              leftSection={<IconPlus size={16} />}
              onClick={() => (fe.setError(null), setEditing({ id: null, name: "", color: STAFF_COLORS[(q.data?.length ?? 0) % STAFF_COLORS.length], default_profile_id: profiles.data?.[0]?.id ?? null, location_id: null, archived: false }))}
            >
              Add staff member
            </Button>
          </Group>
          <Paper p={0}>
            <Table>
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>Name</Table.Th>
                  <Table.Th>Work profile</Table.Th>
                  <Table.Th />
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {q.data!.length === 0 && (
                  <Table.Tr>
                    <Table.Td colSpan={3}>
                      <Text c="dimmed" size="sm" py="md" ta="center">
                        No staff yet. Add yourself to start booking appointments.
                      </Text>
                    </Table.Td>
                  </Table.Tr>
                )}
                {q.data!.map((s) => (
                  <Table.Tr key={s.id} style={{ opacity: s.archived ? 0.55 : 1 }}>
                    <Table.Td>
                      <Group gap="xs">
                        <ColorSwatch color={`var(--mantine-color-${s.color}-6)`} size={14} />
                        <Text size="sm" fw={600}>
                          {s.name}
                        </Text>
                        {s.archived && <Badge color="gray">Archived</Badge>}
                      </Group>
                    </Table.Td>
                    <Table.Td>
                      <Text size="sm">{profName(s.default_profile_id)}</Text>
                    </Table.Td>
                    <Table.Td>
                      <Group gap="xs" justify="flex-end">
                        <Button size="xs" variant="default" onClick={() => (fe.setError(null), setEditing(s))}>
                          Edit
                        </Button>
                        <Button size="xs" variant="subtle" color="gray" onClick={() => archive.mutate({ id: s.id!, archived: !s.archived })}>
                          {s.archived ? "Restore" : "Archive"}
                        </Button>
                      </Group>
                    </Table.Td>
                  </Table.Tr>
                ))}
              </Table.Tbody>
            </Table>
          </Paper>
          <Modal opened={!!editing} onClose={() => setEditing(null)} title={editing?.id ? "Edit staff member" : "Add staff member"}>
            {editing && (
              <form onSubmit={(e) => (e.preventDefault(), saveS.mutate({ staff: editing }, { onSuccess: () => setEditing(null), onError: fe.setError }))}>
                <Stack>
                  <TextInput label="Name" required value={editing.name} onChange={(e) => setEditing({ ...editing, name: e.currentTarget.value })} error={fe.err("name")} data-autofocus />
                  <Select
                    label="Default work profile"
                    data={(profiles.data ?? []).map((p) => ({ value: String(p.id), label: p.name }))}
                    value={editing.default_profile_id != null ? String(editing.default_profile_id) : null}
                    onChange={(v) => setEditing({ ...editing, default_profile_id: v ? Number(v) : null })}
                    clearable
                  />
                  <Select
                    label="Location"
                    data={(locs.data ?? []).filter((l) => !l.archived).map((l) => ({ value: String(l.id), label: l.name }))}
                    value={editing.location_id != null ? String(editing.location_id) : null}
                    onChange={(v) => setEditing({ ...editing, location_id: v ? Number(v) : null })}
                    clearable
                  />
                  <div>
                    <Text size="sm" fw={500} mb={6}>
                      Calendar color
                    </Text>
                    <Group gap={8}>
                      {STAFF_COLORS.map((c) => (
                        <ColorSwatch
                          key={c}
                          component="button"
                          type="button"
                          aria-label={c}
                          aria-pressed={editing.color === c}
                          color={`var(--mantine-color-${c}-6)`}
                          onClick={() => setEditing({ ...editing, color: c })}
                          style={{ outline: editing.color === c ? "2px solid var(--mantine-color-text)" : undefined, outlineOffset: 2 }}
                        />
                      ))}
                    </Group>
                  </div>
                  <Group justify="flex-end">
                    <Button variant="default" onClick={() => setEditing(null)}>
                      Cancel
                    </Button>
                    <Button type="submit" loading={saveS.isPending}>
                      Save
                    </Button>
                  </Group>
                </Stack>
              </form>
            )}
          </Modal>
        </Stack>
      )}
    </QueryState>
  );
}

function fmtSize(n: number) {
  return n > 1024 * 1024 ? `${(n / 1024 / 1024).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`;
}

function WorkspacesTab() {
  const status = useCmd<AppStatus>("app_status");
  const backups = useCmd<BackupInfo[]>("backup_list");
  const apply = useApplyStatus();
  const [renaming, setRenaming] = useState<{ id: string; name: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const cur = status.data?.current;

  async function run<T>(fn: () => Promise<T>, ok?: string) {
    setBusy(true);
    try {
      const out = await fn();
      if (ok) notifications.show({ message: ok, color: "teal" });
      return out;
    } catch (e) {
      notifications.show({ title: "That didn't work", message: errorMessage(e), color: "red", autoClose: 10000 });
    } finally {
      setBusy(false);
      backups.refetch();
    }
  }

  async function exportBackup() {
    const stamp = new Date().toISOString().slice(0, 10);
    const path = await save({ title: "Save backup", defaultPath: `${cur?.name ?? "salon"}-backup-${stamp}.zip`, filters: [{ name: "Backup", extensions: ["zip"] }] });
    if (path) run(() => call<string>("backup_create", { path }), `Backup saved to ${path}`);
  }

  function confirmRestore(path: string) {
    modals.open({
      title: "Restore this backup?",
      children: (
        <Stack>
          <Text size="sm">The backup is checked first. Choose how to restore it:</Text>
          <Button
            onClick={async () => {
              modals.closeAll();
              const s = await run(() => call<AppStatus>("backup_restore", { path, replaceCurrent: false }), "Backup restored as a new workspace");
              if (s) apply(s);
            }}
          >
            Restore as a new workspace (keeps current data)
          </Button>
          <Button
            color="red"
            variant="light"
            onClick={async () => {
              modals.closeAll();
              const s = await run(() => call<AppStatus>("backup_restore", { path, replaceCurrent: true }), "Workspace replaced from backup. The previous data was backed up first.");
              if (s) apply(s);
            }}
          >
            Replace “{cur?.name}” (a safety backup is taken first)
          </Button>
        </Stack>
      ),
    });
  }

  async function exportAll() {
    const stamp = new Date().toISOString().slice(0, 10);
    const path = await save({ title: "Export all data", defaultPath: `${cur?.name ?? "salon"}-export-${stamp}.zip`, filters: [{ name: "Zip of CSV files", extensions: ["zip"] }] });
    if (path) run(() => call<number>("export_all", { path }), `Exported every table to ${path}`);
  }

  async function chooseRestore() {
    const path = await open({ title: "Choose a backup to restore", filters: [{ name: "Backup", extensions: ["zip"] }] });
    if (typeof path === "string") confirmRestore(path);
  }

  function confirmDelete(id: string, name: string) {
    let typed = "";
    modals.openConfirmModal({
      title: `Delete “${name}”?`,
      children: (
        <Stack>
          <Text size="sm">
            This removes the workspace from this computer. A final backup is kept in the backups folder. Type the workspace name to confirm.
          </Text>
          <TextInput aria-label="Workspace name" onChange={(e) => (typed = e.currentTarget.value)} />
        </Stack>
      ),
      labels: { confirm: "Delete workspace", cancel: "Cancel" },
      confirmProps: { color: "red" },
      onConfirm: async () => {
        const s = await run(() => call<AppStatus>("workspace_delete", { id, confirmName: typed }), "Workspace deleted");
        if (s) apply(s);
      },
    });
  }

  return (
    <Stack maw={960}>
      <Paper p="lg">
        <Group justify="space-between" mb="sm">
          <Title order={4}>Workspaces</Title>
          <Button
            size="xs"
            variant="default"
            loading={busy}
            onClick={async () => {
              const s = await run(() => call<AppStatus>("demo_create"), "Demo workspace ready");
              if (s) apply(s);
            }}
          >
            {status.data?.workspaces.some((w) => w.kind === "demo") ? "Reset demo workspace" : "Create demo workspace"}
          </Button>
        </Group>
        <Text size="sm" c="dimmed" mb="md">
          Each workspace is a separate database on this computer. Sharing one workspace file between computers is not supported and can corrupt data; use backups to move data.
        </Text>
        <Table>
          <Table.Tbody>
            {status.data?.workspaces.map((w) => (
              <Table.Tr key={w.id}>
                <Table.Td>
                  <Group gap="xs">
                    <Text size="sm" fw={600}>
                      {w.name}
                    </Text>
                    {w.id === cur?.id && <Badge>Open</Badge>}
                    {w.kind === "demo" && <Badge color="yellow">Demo</Badge>}
                  </Group>
                </Table.Td>
                <Table.Td>
                  <Text size="xs" c="dimmed">
                    Created {isoToLocal(w.created_at)}
                  </Text>
                </Table.Td>
                <Table.Td>
                  <Group gap="xs" justify="flex-end">
                    {w.id !== cur?.id && (
                      <Button size="xs" variant="default" onClick={async () => apply(await call<AppStatus>("workspace_open", { id: w.id }))}>
                        Open
                      </Button>
                    )}
                    <Button size="xs" variant="subtle" onClick={() => setRenaming({ id: w.id, name: w.name })}>
                      Rename
                    </Button>
                    <Button size="xs" variant="subtle" color="red" onClick={() => confirmDelete(w.id, w.name)}>
                      Delete
                    </Button>
                  </Group>
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      </Paper>

      <Paper p="lg">
        <Group justify="space-between" mb="sm">
          <Title order={4}>Backups of “{cur?.name}”</Title>
          <Group gap="xs">
            <Button size="xs" variant="default" leftSection={<IconRestore size={14} />} onClick={chooseRestore} loading={busy}>
              Restore from file…
            </Button>
            <Button size="xs" variant="default" onClick={() => run(() => call<string>("backup_create", {}), "Backup created")} loading={busy}>
              Back up now
            </Button>
            <Button size="xs" leftSection={<IconDownload size={14} />} onClick={exportBackup} loading={busy}>
              Save backup as…
            </Button>
            <Button size="xs" variant="default" onClick={exportAll} loading={busy}>
              Export all data (CSV)…
            </Button>
          </Group>
        </Group>
        <Text size="sm" c="dimmed" mb="md">
          An automatic backup is made once a day when the workspace opens (the newest 14 are kept) and before every update to the data format. Each backup includes
          attachments and is verified with checksums before it can be restored.
        </Text>
        <QueryState loading={backups.isLoading} error={backups.error}>
          {() =>
            backups.data!.length === 0 ? (
              <Text size="sm" c="dimmed">
                No backups yet.
              </Text>
            ) : (
              <Table>
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>Made</Table.Th>
                    <Table.Th>Reason</Table.Th>
                    <Table.Th>Size</Table.Th>
                    <Table.Th />
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {backups.data!.map((b) => (
                    <Table.Tr key={b.path}>
                      <Table.Td>{b.manifest ? isoToLocal(b.manifest.created_at) : b.file_name}</Table.Td>
                      <Table.Td>{b.error ? <Text c="red" size="sm">{b.error}</Text> : b.manifest?.reason}</Table.Td>
                      <Table.Td>{fmtSize(b.size)}</Table.Td>
                      <Table.Td>
                        <Group justify="flex-end">
                          <Button size="xs" variant="subtle" disabled={!b.manifest} onClick={() => confirmRestore(b.path)}>
                            Restore…
                          </Button>
                        </Group>
                      </Table.Td>
                    </Table.Tr>
                  ))}
                </Table.Tbody>
              </Table>
            )
          }
        </QueryState>
        <Text size="xs" c="dimmed" mt="md">
          Data folder: {status.data?.data_dir}
        </Text>
      </Paper>

      <Modal opened={!!renaming} onClose={() => setRenaming(null)} title="Rename workspace">
        {renaming && (
          <form
            onSubmit={async (e) => {
              e.preventDefault();
              const s = await run(() => call<AppStatus>("workspace_rename", { id: renaming.id, name: renaming.name }), "Workspace renamed");
              if (s) {
                status.refetch();
                setRenaming(null);
              }
            }}
          >
            <TextInput label="Name" value={renaming.name} onChange={(e) => setRenaming({ ...renaming, name: e.currentTarget.value })} data-autofocus />
            <Group justify="flex-end" mt="md">
              <Button type="submit">Rename</Button>
            </Group>
          </form>
        )}
      </Modal>
    </Stack>
  );
}

function ActivityTab() {
  const q = useCmd<AuditEntry[]>("audit_list", { limit: 200 });
  return (
    <QueryState loading={q.isLoading} error={q.error}>
      {() => (
        <Paper p={0} maw={1000}>
          <Table>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>When</Table.Th>
                <Table.Th>What</Table.Th>
                <Table.Th>Record</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {q.data!.map((a) => (
                <Table.Tr key={a.id}>
                  <Table.Td w={200}>
                    <Text size="sm">{isoToLocal(a.at)}</Text>
                  </Table.Td>
                  <Table.Td>
                    <Text size="sm">{a.summary}</Text>
                  </Table.Td>
                  <Table.Td>
                    <Text size="xs" c="dimmed">
                      {a.entity.replace(/_/g, " ")} {a.entity_id ?? ""}
                    </Text>
                  </Table.Td>
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
        </Paper>
      )}
    </QueryState>
  );
}

export function SettingsPage() {
  const [params, setParams] = useSearchParams();
  const tab = params.get("tab") ?? "business";
  return (
    <>
      <PageHeader title="Settings" description="Business details, locations, staff, workspaces and backups." />
      <Tabs value={tab} onChange={(v) => setParams({ tab: v ?? "business" }, { replace: true })} keepMounted={false}>
        <Tabs.List mb="lg">
          <Tabs.Tab value="business">Business</Tabs.Tab>
          <Tabs.Tab value="locations">Locations</Tabs.Tab>
          <Tabs.Tab value="staff">Staff</Tabs.Tab>
          <Tabs.Tab value="workspaces">Workspaces and backups</Tabs.Tab>
          <Tabs.Tab value="providers">Data providers</Tabs.Tab>
          <Tabs.Tab value="activity">Activity log</Tabs.Tab>
        </Tabs.List>
        <Tabs.Panel value="business">
          <BusinessTab />
        </Tabs.Panel>
        <Tabs.Panel value="locations">
          <LocationsTab />
        </Tabs.Panel>
        <Tabs.Panel value="staff">
          <StaffTab />
        </Tabs.Panel>
        <Tabs.Panel value="workspaces">
          <WorkspacesTab />
        </Tabs.Panel>
        <Tabs.Panel value="providers">
          <ProvidersTab />
        </Tabs.Panel>
        <Tabs.Panel value="activity">
          <ActivityTab />
        </Tabs.Panel>
      </Tabs>
    </>
  );
}
