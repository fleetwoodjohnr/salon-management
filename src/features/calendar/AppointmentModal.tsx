import { ActionIcon, Alert, Button, Group, Modal, NumberInput, Select, SimpleGrid, Stack, Text, TextInput, Textarea } from "@mantine/core";
import { IconPlus, IconTrash, IconUserPlus } from "@tabler/icons-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { call, isAppError } from "../../api/ipc";
import { useCmd } from "../../api/queries";
import type { AppointmentInput, AppointmentSaveResult, ClientInput, ClientRow, ServiceSummary, ServiceView, Staff } from "../../api/types";
import { minutesLabel } from "../../lib/format";
import { fromLocalStr, toLocalStr } from "../../lib/time";

function VariantPickers({ serviceId, value, onChange }: { serviceId: number; value: number[]; onChange: (v: number[]) => void }) {
  const s = useCmd<ServiceView>("service_get", { id: serviceId }, !!serviceId);
  const groups = [...new Set((s.data?.input.variants ?? []).map((v) => v.group_name))];
  return (
    <>
      {groups.map((g) => {
        const opts = s.data!.input.variants.filter((v) => v.group_name === g);
        const cur = value.find((id) => opts.some((o) => o.id === id));
        return (
          <Select
            key={g}
            size="xs"
            aria-label={g}
            placeholder={`${g}: base`}
            clearable
            w={150}
            data={opts.map((o) => ({ value: String(o.id), label: `${g}: ${o.name}` }))}
            value={cur != null ? String(cur) : null}
            onChange={(v) => onChange([...value.filter((id) => !opts.some((o) => o.id === id)), ...(v ? [Number(v)] : [])])}
          />
        );
      })}
    </>
  );
}

export function AppointmentModal({ initial, onClose, onSaved }: { initial: AppointmentInput | null; onClose: () => void; onSaved: (r: AppointmentSaveResult) => void }) {
  const qc = useQueryClient();
  const [a, setA] = useState<AppointmentInput | null>(initial);
  const [result, setResult] = useState<AppointmentSaveResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [newClient, setNewClient] = useState<ClientInput | null>(null);
  useEffect(() => {
    setA(initial);
    setResult(null);
    setError(null);
  }, [initial]);
  const staff = useCmd<Staff[]>("staff_list");
  const clients = useCmd<ClientRow[]>("clients_list", { includeArchived: false });
  const services = useCmd<ServiceSummary[]>("services_list", { includeArchived: false });
  const lines = a?.lines.filter((l) => l.service_id) ?? [];
  const minutes = useQuery({ queryKey: ["appointment_minutes", lines], queryFn: () => call<number>("appointment_minutes", { lines }), enabled: lines.length > 0 });
  if (!a) return null;
  const start = a.starts_at ? fromLocalStr(a.starts_at) : new Date();
  const end = minutes.data != null ? toLocalStr(new Date(start.getTime() + minutes.data * 60000)) : null;

  async function save(allowOverlap: boolean) {
    setBusy(true);
    setError(null);
    try {
      const r = await call<AppointmentSaveResult>("appointment_save", { appointment: { ...a!, lines, allow_overlap: allowOverlap } });
      setResult(r);
      if (r.id) {
        qc.invalidateQueries({ queryKey: ["appointments_list"] });
        onSaved(r);
      }
    } catch (e) {
      setError(isAppError(e) ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function createClient() {
    try {
      const id = await call<number>("client_save", { client: newClient });
      await qc.invalidateQueries({ queryKey: ["clients_list"] });
      setA({ ...a!, client_id: id });
      setNewClient(null);
    } catch (e) {
      setError(isAppError(e) ? e.message : String(e));
    }
  }

  return (
    <Modal opened onClose={onClose} title={a.id ? "Edit appointment" : "New appointment"} size="lg">
      <Stack>
        {newClient ? (
          <Group align="flex-end" gap="xs">
            <TextInput label="First name" value={newClient.first_name} onChange={(e) => setNewClient({ ...newClient, first_name: e.currentTarget.value })} data-autofocus />
            <TextInput label="Last name" value={newClient.last_name} onChange={(e) => setNewClient({ ...newClient, last_name: e.currentTarget.value })} />
            <TextInput label="Phone" value={newClient.phone} onChange={(e) => setNewClient({ ...newClient, phone: e.currentTarget.value })} w={140} />
            <Button size="sm" onClick={createClient} disabled={!newClient.first_name.trim()}>
              Add
            </Button>
            <Button size="sm" variant="subtle" onClick={() => setNewClient(null)}>
              Cancel
            </Button>
          </Group>
        ) : (
          <Group align="flex-end" gap="xs">
            <Select
              label="Client"
              placeholder="Walk-in"
              searchable
              clearable
              style={{ flex: 1 }}
              data={(clients.data ?? []).map((c) => ({ value: String(c.client.id), label: `${c.client.first_name} ${c.client.last_name}`.trim() + (c.client.phone ? ` (${c.client.phone})` : "") }))}
              value={a.client_id != null ? String(a.client_id) : null}
              onChange={(v) => setA({ ...a, client_id: v ? Number(v) : null })}
            />
            <Button variant="default" leftSection={<IconUserPlus size={16} />} onClick={() => setNewClient({ id: null, first_name: "", last_name: "", phone: "", email: "", sensitivities: "", notes: "" })}>
              New client
            </Button>
          </Group>
        )}
        <SimpleGrid cols={3}>
          <Select
            label="With"
            data={(staff.data ?? []).filter((s) => !s.archived).map((s) => ({ value: String(s.id), label: s.name }))}
            value={a.staff_id ? String(a.staff_id) : null}
            allowDeselect={false}
            onChange={(v) => setA({ ...a, staff_id: Number(v) })}
          />
          <TextInput type="date" label="Date" value={a.starts_at.slice(0, 10)} onChange={(e) => setA({ ...a, starts_at: `${e.currentTarget.value}T${a.starts_at.slice(11, 16)}` })} />
          <TextInput type="time" label="Start" step={300} value={a.starts_at.slice(11, 16)} onChange={(e) => setA({ ...a, starts_at: `${a.starts_at.slice(0, 10)}T${e.currentTarget.value}` })} />
        </SimpleGrid>
        <div>
          <Text size="sm" fw={500} mb={4}>
            Services
          </Text>
          <Stack gap={6}>
            {a.lines.map((l, i) => (
              <Group key={i} gap="xs" wrap="nowrap">
                <Select
                  aria-label={`Appointment service ${i + 1}`}
                  searchable
                  style={{ flex: 1 }}
                  data={(services.data ?? []).map((s) => ({ value: String(s.id), label: s.is_addon ? `${s.name} (add-on)` : s.name }))}
                  value={l.service_id ? String(l.service_id) : null}
                  onChange={(v) => setA({ ...a, lines: a.lines.map((x, j) => (j === i ? { ...x, service_id: Number(v), variant_ids: [] } : x)) })}
                />
                {l.service_id ? <VariantPickers serviceId={l.service_id} value={l.variant_ids} onChange={(v) => setA({ ...a, lines: a.lines.map((x, j) => (j === i ? { ...x, variant_ids: v } : x)) })} /> : null}
                <NumberInput aria-label="Quantity" w={64} min={1} allowDecimal={false} value={l.qty} onChange={(v) => setA({ ...a, lines: a.lines.map((x, j) => (j === i ? { ...x, qty: Number(v) || 1 } : x)) })} />
                <ActionIcon variant="subtle" color="gray" aria-label="Remove service" onClick={() => setA({ ...a, lines: a.lines.filter((_, j) => j !== i) })}>
                  <IconTrash size={16} />
                </ActionIcon>
              </Group>
            ))}
            <Button variant="subtle" size="xs" w="fit-content" leftSection={<IconPlus size={14} />} onClick={() => setA({ ...a, lines: [...a.lines, { service_id: 0, variant_ids: [], qty: 1 }] })}>
              Add service or add-on
            </Button>
          </Stack>
        </div>
        {minutes.data != null && end && (
          <Text size="sm" c="dimmed">
            Chair time {minutesLabel(minutes.data)}, until {fromLocalStr(end).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })}
          </Text>
        )}
        <Textarea label="Notes" value={a.notes} onChange={(e) => setA({ ...a, notes: e.currentTarget.value })} />
        {result && result.conflicts.length > 0 && !result.id && (
          <Alert color="yellow" title="This overlaps another booking">
            {result.conflicts.map((c) => (
              <Text key={c} size="sm">
                {c}
              </Text>
            ))}
            <Button size="xs" mt="xs" color="yellow" onClick={() => save(true)} loading={busy}>
              Book anyway
            </Button>
          </Alert>
        )}
        {result?.warnings.map((w) => (
          <Alert key={w} color="blue" py={6}>
            <Text size="sm">{w}</Text>
          </Alert>
        ))}
        {error && <Alert color="red">{error}</Alert>}
        <Group justify="flex-end">
          <Button variant="default" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={() => save(false)} loading={busy} disabled={!a.staff_id || lines.length === 0}>
            Save appointment
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
