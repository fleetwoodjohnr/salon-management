import { Alert, Badge, Button, EmptyState, Grid, Group, Modal, Paper, SimpleGrid, Stack, Switch, Table, Text, TextInput, Textarea, Title } from "@mantine/core";
import { useDebouncedValue, useHotkeys } from "@mantine/hooks";
import { IconPlus, IconSearch, IconUsers } from "@tabler/icons-react";
import { DataTable } from "mantine-datatable";
import { useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { call, isAppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { ClientDetail, ClientInput, ClientRow, FormulaInput } from "../../api/types";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { notifyUndo } from "../../components/undo";
import { dateLabel, isoToLocal, money } from "../../lib/format";
import { dateTimeLabel } from "../../lib/time";

const blank: ClientInput = { id: null, first_name: "", last_name: "", phone: "", email: "", sensitivities: "", notes: "" };

function ClientForm({ value, onClose, onSaved }: { value: ClientInput; onClose: () => void; onSaved: (id: number) => void }) {
  const [c, setC] = useState(value);
  const [err, setErr] = useState<{ field: string | null; message: string } | null>(null);
  const save = useAction<{ client: ClientInput }, number>("client_save", { invalidate: ["clients_list", "client_get"], success: "Client saved", silentError: true });
  const f = (k: string) => (err?.field === k ? err.message : undefined);
  return (
    <Modal opened onClose={onClose} title={c.id ? "Edit client" : "New client"}>
      <form onSubmit={(e) => (e.preventDefault(), save.mutate({ client: c }, { onSuccess: onSaved, onError: (x) => setErr(isAppError(x) ? x : { field: null, message: String(x) }) }))}>
        <Stack>
          <SimpleGrid cols={2}>
            <TextInput label="First name" required value={c.first_name} onChange={(e) => setC({ ...c, first_name: e.currentTarget.value })} error={f("first_name")} data-autofocus />
            <TextInput label="Last name" value={c.last_name} onChange={(e) => setC({ ...c, last_name: e.currentTarget.value })} />
            <TextInput label="Phone" value={c.phone} onChange={(e) => setC({ ...c, phone: e.currentTarget.value })} />
            <TextInput label="Email" value={c.email} onChange={(e) => setC({ ...c, email: e.currentTarget.value })} error={f("email")} />
          </SimpleGrid>
          <Textarea label="Allergies and sensitivities" description="Shown prominently on the client's page" value={c.sensitivities} onChange={(e) => setC({ ...c, sensitivities: e.currentTarget.value })} />
          <Textarea label="Notes" autosize minRows={2} value={c.notes} onChange={(e) => setC({ ...c, notes: e.currentTarget.value })} />
          {err && !err.field && <Alert color="red">{err.message}</Alert>}
          <Group justify="flex-end">
            <Button type="submit" loading={save.isPending}>
              Save client
            </Button>
          </Group>
        </Stack>
      </form>
    </Modal>
  );
}

export function ClientsPage() {
  const navigate = useNavigate();
  const [archived, setArchived] = useState(false);
  const q = useCmd<ClientRow[]>("clients_list", { includeArchived: archived });
  const [search, setSearch] = useState("");
  const [debounced] = useDebouncedValue(search, 150);
  const [creating, setCreating] = useState(false);
  useHotkeys([["mod+N", () => setCreating(true)]]);
  const rows = useMemo(() => {
    const s = debounced.trim().toLowerCase();
    return (q.data ?? []).filter((r) => !s || [r.client.first_name, r.client.last_name, r.client.phone, r.client.email].some((v) => v.toLowerCase().includes(s)));
  }, [q.data, debounced]);
  return (
    <>
      <PageHeader
        title="Clients"
        description="Contact details, sensitivities, visit history and formulas. Client records stay on this computer and are never sent to any online service."
        actions={
          <Button leftSection={<IconPlus size={16} />} onClick={() => setCreating(true)}>
            New client
          </Button>
        }
      />
      <QueryState loading={q.isLoading} error={q.error}>
        {() =>
          q.data!.length === 0 && !archived ? (
            <EmptyState icon={<IconUsers size={32} />} title="No clients yet" description="Add clients here or while booking an appointment." mt="xl">
              <Button mt="md" onClick={() => setCreating(true)}>
                Add a client
              </Button>
            </EmptyState>
          ) : (
            <Stack gap="sm">
              <Group justify="space-between">
                <TextInput aria-label="Search clients" placeholder="Search name, phone, email" leftSection={<IconSearch size={16} />} w={320} value={search} onChange={(e) => setSearch(e.currentTarget.value)} />
                <Switch size="xs" label="Archived" checked={archived} onChange={(e) => setArchived(e.currentTarget.checked)} />
              </Group>
              <DataTable
                withTableBorder
                borderRadius="lg"
                highlightOnHover
                records={rows}
                idAccessor={(r) => r.client.id!}
                onRowClick={({ record }) => navigate(`/clients/${record.client.id}`)}
                noRecordsText="No clients match"
                minHeight={rows.length ? undefined : 140}
                columns={[
                  {
                    accessor: "name",
                    title: "Client",
                    render: (r) => (
                      <Group gap={6}>
                        <Text size="sm" fw={600}>
                          {r.client.first_name} {r.client.last_name}
                        </Text>
                        {r.client.sensitivities && <Badge color="red">Sensitivities</Badge>}
                        {r.archived && <Badge color="gray">Archived</Badge>}
                      </Group>
                    ),
                  },
                  { accessor: "phone", title: "Phone", render: (r) => r.client.phone || "—" },
                  { accessor: "visits", title: "Visits", textAlign: "right" },
                  { accessor: "last_visit", title: "Last visit", render: (r) => dateLabel(r.last_visit) },
                  { accessor: "next", title: "Next appointment", render: (r) => (r.next_appointment ? dateTimeLabel(r.next_appointment) : "—") },
                ]}
              />
            </Stack>
          )
        }
      </QueryState>
      {creating && <ClientForm value={blank} onClose={() => setCreating(false)} onSaved={(id) => (setCreating(false), navigate(`/clients/${id}`))} />}
    </>
  );
}

function FormulaModal({ value, onClose }: { value: FormulaInput; onClose: () => void }) {
  const [f, setF] = useState(value);
  const [err, setErr] = useState<string | null>(null);
  const save = useAction<{ formula: FormulaInput }, number>("formula_save", { invalidate: ["client_get"], success: "Formula saved", silentError: true });
  return (
    <Modal opened onClose={onClose} title={f.formula_id ? "New version of formula" : "New formula"}>
      <Stack>
        {!f.formula_id && <TextInput label="Name" placeholder="e.g. Root color" value={f.title} onChange={(e) => setF({ ...f, title: e.currentTarget.value })} data-autofocus />}
        <Textarea label="Formula" placeholder="e.g. 7N 30 g + 6N 15 g, 20 vol 1:1, 35 min" autosize minRows={3} value={f.body} onChange={(e) => setF({ ...f, body: e.currentTarget.value })} />
        <TextInput label="Note" placeholder="What changed and why" value={f.note} onChange={(e) => setF({ ...f, note: e.currentTarget.value })} />
        {err && <Alert color="red">{err}</Alert>}
        <Group justify="flex-end">
          <Button loading={save.isPending} onClick={() => save.mutate({ formula: f }, { onSuccess: onClose, onError: (e) => setErr(isAppError(e) ? e.message : String(e)) })}>
            Save formula
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}

export function ClientDetailPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const q = useCmd<ClientDetail>("client_get", { id: Number(id) });
  const [editing, setEditing] = useState(false);
  const [formula, setFormula] = useState<FormulaInput | null>(null);
  const archive = useAction<{ id: number; archived: boolean }>("client_archive", { invalidate: ["clients_list", "client_get"] });
  return (
    <QueryState loading={q.isLoading} error={q.error}>
      {() => {
        const d = q.data!;
        const c = d.row.client;
        return (
          <>
            <PageHeader
              title={`${c.first_name} ${c.last_name}`.trim()}
              description={[c.phone, c.email].filter(Boolean).join(", ") || "No contact details"}
              actions={
                <>
                  <Button variant="default" onClick={() => setEditing(true)}>
                    Edit
                  </Button>
                  {!d.row.archived && (
                    <Button
                      variant="subtle"
                      color="gray"
                      onClick={() =>
                        archive.mutate(
                          { id: c.id!, archived: true },
                          { onSuccess: () => (notifyUndo("Client archived", () => call("client_archive", { id: c.id, archived: false }).then(() => q.refetch())), navigate("/clients")) },
                        )
                      }
                    >
                      Archive
                    </Button>
                  )}
                </>
              }
            />
            {c.sensitivities && (
              <Alert color="red" title="Allergies and sensitivities" mb="lg">
                {c.sensitivities}
              </Alert>
            )}
            <Grid gap="lg">
              <Grid.Col span={{ base: 12, lg: 7 }}>
                <Paper p="lg">
                  <Title order={4} mb="sm">
                    Visit history
                  </Title>
                  {d.history.length === 0 ? (
                    <Text size="sm" c="dimmed">
                      No visits yet.
                    </Text>
                  ) : (
                    <Table fz="sm" highlightOnHover>
                      <Table.Tbody>
                        {d.history.map((h, i) => (
                          <Table.Tr key={i} style={{ cursor: "pointer", opacity: h.status === "voided" ? 0.5 : 1 }} onClick={() => navigate(`/sales/${h.sale_id}`)}>
                            <Table.Td w={120}>{dateLabel(h.sale_date)}</Table.Td>
                            <Table.Td>{h.description}</Table.Td>
                            <Table.Td>{h.staff_name ?? ""}</Table.Td>
                            <Table.Td ta="right">{h.status === "draft" ? <Badge color="yellow">Draft</Badge> : money(h.net)}</Table.Td>
                          </Table.Tr>
                        ))}
                      </Table.Tbody>
                    </Table>
                  )}
                </Paper>
                {c.notes && (
                  <Paper p="lg" mt="lg">
                    <Title order={5} mb={4}>
                      Notes
                    </Title>
                    <Text size="sm" style={{ whiteSpace: "pre-wrap" }}>
                      {c.notes}
                    </Text>
                  </Paper>
                )}
              </Grid.Col>
              <Grid.Col span={{ base: 12, lg: 5 }}>
                <Paper p="lg">
                  <Group justify="space-between" mb="sm">
                    <Title order={4}>Formulas</Title>
                    <Button size="xs" leftSection={<IconPlus size={14} />} onClick={() => setFormula({ formula_id: null, client_id: c.id!, title: "", service_id: null, body: "", lines: [], note: "", sale_id: null })}>
                      New formula
                    </Button>
                  </Group>
                  {d.formulas.length === 0 && (
                    <Text size="sm" c="dimmed">
                      Record color and treatment formulas here. Each change keeps the previous versions.
                    </Text>
                  )}
                  <Stack>
                    {d.formulas.map((f) => (
                      <div key={f.id}>
                        <Group justify="space-between">
                          <Text fw={650}>{f.title}</Text>
                          <Button size="compact-xs" variant="subtle" onClick={() => setFormula({ formula_id: f.id, client_id: c.id!, title: f.title, service_id: f.service_id, body: f.versions[0]?.body ?? "", lines: [], note: "", sale_id: null })}>
                            Update
                          </Button>
                        </Group>
                        {f.versions.map((v, i) => (
                          <div key={v.id} style={{ opacity: i === 0 ? 1 : 0.6, marginTop: 6 }}>
                            <Text size="xs" c="dimmed">
                              Version {v.version}, {isoToLocal(v.created_at)}
                              {v.note ? `: ${v.note}` : ""}
                            </Text>
                            <Text size="sm" style={{ whiteSpace: "pre-wrap" }}>
                              {v.body}
                            </Text>
                          </div>
                        ))}
                      </div>
                    ))}
                  </Stack>
                </Paper>
              </Grid.Col>
            </Grid>
            {editing && <ClientForm value={c} onClose={() => setEditing(false)} onSaved={() => setEditing(false)} />}
            {formula && <FormulaModal value={formula} onClose={() => setFormula(null)} />}
          </>
        );
      }}
    </QueryState>
  );
}
