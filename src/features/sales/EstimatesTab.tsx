import { ActionIcon, Badge, Button, Group, Modal, NumberInput, Select, SimpleGrid, Stack, Text, TextInput, Textarea } from "@mantine/core";
import { IconPlus, IconTrash } from "@tabler/icons-react";
import { DataTable } from "mantine-datatable";
import { useState } from "react";
import { useNavigate } from "react-router";
import { call, isAppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { AppointmentInput, ClientRow, EstimateInput, EstimateView, ServiceSummary, Staff } from "../../api/types";
import { DecimalInput } from "../../components/DecimalInput";
import { QueryState } from "../../components/QueryState";
import { addDays, useToday } from "../../lib/dates";
import { dateLabel, money } from "../../lib/format";
import { AppointmentModal } from "../calendar/AppointmentModal";

export function EstimatesTab() {
  const navigate = useNavigate();
  const today = useToday();
  const q = useCmd<EstimateView[]>("estimates_list");
  const clients = useCmd<ClientRow[]>("clients_list", { includeArchived: false });
  const services = useCmd<ServiceSummary[]>("services_list", { includeArchived: false });
  const staff = useCmd<Staff[]>("staff_list");
  const [edit, setEdit] = useState<EstimateInput | null>(null);
  const [book, setBook] = useState<AppointmentInput | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const save = useAction<{ estimate: EstimateInput }, number>("estimate_save", { invalidate: ["estimates_list"], success: "Estimate saved", silentError: true });

  async function setService(i: number, id: number) {
    const price = await call<string>("estimate_default_price", { serviceId: id, variantIds: [] });
    const name = services.data?.find((s) => s.id === id)?.name ?? "";
    setEdit((e) => e && { ...e, lines: e.lines.map((l, j) => (j === i ? { ...l, service_id: id, unit_price: price, description: name } : l)) });
  }

  return (
    <QueryState loading={q.isLoading} error={q.error}>
      {() => (
        <Stack>
          <Group justify="space-between">
            <Text size="sm" c="dimmed">
              Quotes for clients, printable before booking. Convert one into an appointment when the client books.
            </Text>
            <Button
              leftSection={<IconPlus size={16} />}
              onClick={() => (setErr(null), setEdit({ id: null, client_id: null, staff_id: null, location_id: null, issued_on: today, valid_until: addDays(today, 30), notes: "", lines: [{ service_id: 0, variant_ids: [], qty: 1, unit_price: "0", description: "" }] }))}
            >
              New estimate
            </Button>
          </Group>
          <DataTable
            withTableBorder
            borderRadius="lg"
            records={q.data!}
            idAccessor={(e) => e.input.id!}
            minHeight={q.data!.length ? undefined : 140}
            noRecordsText="No estimates yet"
            columns={[
              { accessor: "issued", title: "Issued", render: (e) => dateLabel(e.input.issued_on) },
              { accessor: "client", title: "Client", render: (e) => e.client_name || "—" },
              { accessor: "services", title: "Services", render: (e) => e.input.lines.map((l) => l.description).join(", ") },
              { accessor: "subtotal", title: "Before tax", textAlign: "right", render: (e) => money(e.subtotal) },
              { accessor: "status", title: "", render: (e) => <Badge color={e.status === "converted" ? "teal" : e.status === "declined" ? "gray" : "blue"}>{e.status}</Badge> },
              {
                accessor: "actions",
                title: "",
                render: (e) => (
                  <Group gap={4} justify="flex-end">
                    <Button size="compact-xs" variant="subtle" onClick={() => navigate(`/print/estimate/${e.input.id}`)}>
                      Print
                    </Button>
                    {e.status !== "converted" && (
                      <>
                        <Button size="compact-xs" variant="subtle" onClick={() => (setErr(null), setEdit(e.input))}>
                          Edit
                        </Button>
                        <Button
                          size="compact-xs"
                          variant="light"
                          onClick={() =>
                            setBook({
                              id: null,
                              client_id: e.input.client_id,
                              staff_id: e.input.staff_id ?? staff.data?.find((s) => !s.archived)?.id ?? 0,
                              location_id: e.input.location_id,
                              starts_at: `${today}T10:00`,
                              ends_at: null,
                              notes: e.input.notes,
                              lines: e.input.lines.map((l) => ({ service_id: l.service_id, variant_ids: l.variant_ids, qty: l.qty })),
                              estimate_id: e.input.id,
                              allow_overlap: false,
                            })
                          }
                        >
                          Book it
                        </Button>
                      </>
                    )}
                  </Group>
                ),
              },
            ]}
          />
          <Modal opened={!!edit} onClose={() => setEdit(null)} title={edit?.id ? "Edit estimate" : "New estimate"} size="lg">
            {edit && (
              <Stack>
                <SimpleGrid cols={2}>
                  <Select
                    label="Client"
                    searchable
                    clearable
                    data={(clients.data ?? []).map((c) => ({ value: String(c.client.id), label: `${c.client.first_name} ${c.client.last_name}`.trim() }))}
                    value={edit.client_id != null ? String(edit.client_id) : null}
                    onChange={(v) => setEdit({ ...edit, client_id: v ? Number(v) : null })}
                  />
                  <Select
                    label="Staff"
                    clearable
                    data={(staff.data ?? []).filter((s) => !s.archived).map((s) => ({ value: String(s.id), label: s.name }))}
                    value={edit.staff_id != null ? String(edit.staff_id) : null}
                    onChange={(v) => setEdit({ ...edit, staff_id: v ? Number(v) : null })}
                  />
                  <TextInput type="date" label="Issued" value={edit.issued_on} onChange={(e) => setEdit({ ...edit, issued_on: e.currentTarget.value })} />
                  <TextInput type="date" label="Valid until" value={edit.valid_until ?? ""} onChange={(e) => setEdit({ ...edit, valid_until: e.currentTarget.value || null })} />
                </SimpleGrid>
                {edit.lines.map((l, i) => (
                  <Group key={i} gap="xs" wrap="nowrap">
                    <Select aria-label={`Estimate service ${i + 1}`} searchable style={{ flex: 1 }} data={(services.data ?? []).map((s) => ({ value: String(s.id), label: s.name }))} value={l.service_id ? String(l.service_id) : null} onChange={(v) => v && setService(i, Number(v))} />
                    <NumberInput aria-label="Quantity" w={70} min={1} allowDecimal={false} value={l.qty} onChange={(v) => setEdit({ ...edit, lines: edit.lines.map((x, j) => (j === i ? { ...x, qty: Number(v) || 1 } : x)) })} />
                    <DecimalInput aria-label="Price" unit="$" w={120} value={l.unit_price} onChange={(v) => setEdit({ ...edit, lines: edit.lines.map((x, j) => (j === i ? { ...x, unit_price: v } : x)) })} />
                    <ActionIcon variant="subtle" color="gray" aria-label="Remove" onClick={() => setEdit({ ...edit, lines: edit.lines.filter((_, j) => j !== i) })}>
                      <IconTrash size={16} />
                    </ActionIcon>
                  </Group>
                ))}
                <Button variant="subtle" size="xs" w="fit-content" leftSection={<IconPlus size={14} />} onClick={() => setEdit({ ...edit, lines: [...edit.lines, { service_id: 0, variant_ids: [], qty: 1, unit_price: "0", description: "" }] })}>
                  Add service
                </Button>
                <Textarea label="Notes for the client" value={edit.notes} onChange={(e) => setEdit({ ...edit, notes: e.currentTarget.value })} />
                {err && <Text c="red" size="sm">{err}</Text>}
                <Group justify="flex-end">
                  <Button loading={save.isPending} onClick={() => save.mutate({ estimate: { ...edit, lines: edit.lines.filter((l) => l.service_id) } }, { onSuccess: () => setEdit(null), onError: (e) => setErr(isAppError(e) ? e.message : String(e)) })}>
                    Save estimate
                  </Button>
                </Group>
              </Stack>
            )}
          </Modal>
          {book && <AppointmentModal initial={book} onClose={() => setBook(null)} onSaved={() => (setBook(null), q.refetch())} />}
        </Stack>
      )}
    </QueryState>
  );
}
