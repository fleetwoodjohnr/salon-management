import { ActionIcon, Alert, Button, Group, Modal, NumberInput, Paper, Select, Stack, Table, Text, TextInput, Textarea } from "@mantine/core";
import { IconPlus, IconTrash } from "@tabler/icons-react";
import { useState } from "react";
import { isAppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { BundleInput, BundleView, ServiceSummary } from "../../api/types";
import { DecimalInput } from "../../components/DecimalInput";
import { QueryState } from "../../components/QueryState";
import { money, pct } from "../../lib/format";

const blank: BundleInput = { id: null, name: "", description: "", price: null, items: [] };

export function BundlesTab() {
  const q = useCmd<BundleView[]>("bundles_list", { includeArchived: false });
  const services = useCmd<ServiceSummary[]>("services_list", { includeArchived: false });
  const [edit, setEdit] = useState<BundleInput | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const save = useAction<{ bundle: BundleInput }, number>("bundle_save", { invalidate: ["bundles_list"], success: "Bundle saved", silentError: true });
  const archive = useAction<{ id: number; archived: boolean }>("bundle_archive", { invalidate: ["bundles_list"], success: "Bundle archived" });
  const opts = (services.data ?? []).map((s) => ({ value: String(s.id), label: s.name }));
  return (
    <QueryState loading={q.isLoading} error={q.error}>
      {() => (
        <Stack>
          <Group justify="space-between">
            <Text size="sm" c="dimmed" maw={640}>
              A bundle sells several services together at one price. Its price is split across the services in proportion to their own prices, and each share is costed with
              that service's materials, labor, overhead and fees.
            </Text>
            <Button leftSection={<IconPlus size={16} />} onClick={() => (setErr(null), setEdit({ ...blank }))}>
              New bundle
            </Button>
          </Group>
          {q.data!.length === 0 ? (
            <Text size="sm" c="dimmed">
              No bundles yet.
            </Text>
          ) : (
            q.data!.map((b) => (
              <Paper key={b.input.id} p="lg">
                <Group justify="space-between" mb="xs">
                  <div>
                    <Text fw={650}>{b.input.name}</Text>
                    <Text size="xs" c="dimmed">
                      {b.input.description}
                    </Text>
                  </div>
                  <Group gap="xs">
                    <Button size="xs" variant="default" onClick={() => (setErr(null), setEdit(b.input))}>
                      Edit
                    </Button>
                    <Button size="xs" variant="subtle" color="gray" onClick={() => archive.mutate({ id: b.input.id!, archived: true })}>
                      Archive
                    </Button>
                  </Group>
                </Group>
                <Table fz="sm">
                  <Table.Thead>
                    <Table.Tr>
                      <Table.Th>Service</Table.Th>
                      <Table.Th ta="right">Qty</Table.Th>
                      <Table.Th ta="right">Own price</Table.Th>
                      <Table.Th ta="right">Share of bundle</Table.Th>
                      <Table.Th ta="right">Est. cost</Table.Th>
                    </Table.Tr>
                  </Table.Thead>
                  <Table.Tbody>
                    {b.lines.map((l) => (
                      <Table.Tr key={l.service_id}>
                        <Table.Td>{l.name}</Table.Td>
                        <Table.Td className="srm-num">{l.qty}</Table.Td>
                        <Table.Td className="srm-num">{money(l.list_price)}</Table.Td>
                        <Table.Td className="srm-num">{money(l.allocated_price)}</Table.Td>
                        <Table.Td className="srm-num">{money(l.total_cost)}</Table.Td>
                      </Table.Tr>
                    ))}
                  </Table.Tbody>
                </Table>
                <Group gap="xl" mt="sm">
                  <Text size="sm">Bundle price {money(b.input.price)}</Text>
                  <Text size="sm" c="dimmed">
                    Separately {money(b.separate_total)}
                  </Text>
                  <Text size="sm">Est. cost {money(b.total_cost)}</Text>
                  <Text size="sm" fw={650} c={b.profit?.startsWith("-") ? "var(--srm-bad)" : undefined}>
                    Profit {money(b.profit)} ({pct(b.margin_pct)})
                  </Text>
                </Group>
                {b.warnings.map((w) => (
                  <Alert key={w} color="yellow" mt="xs" py={6}>
                    <Text size="xs">{w}</Text>
                  </Alert>
                ))}
              </Paper>
            ))
          )}
          <Modal opened={!!edit} onClose={() => setEdit(null)} title={edit?.id ? "Edit bundle" : "New bundle"} size="lg">
            {edit && (
              <Stack>
                <TextInput label="Name" required value={edit.name} onChange={(e) => setEdit({ ...edit, name: e.currentTarget.value })} data-autofocus />
                <Textarea label="Description" value={edit.description} onChange={(e) => setEdit({ ...edit, description: e.currentTarget.value })} />
                {edit.items.map((it, i) => (
                  <Group key={i} gap="xs" wrap="nowrap">
                    <Select
                      aria-label={`Bundle service ${i + 1}`}
                      data={opts}
                      searchable
                      value={it.service_id ? String(it.service_id) : null}
                      onChange={(v) => setEdit({ ...edit, items: edit.items.map((x, j) => (j === i ? { ...x, service_id: Number(v) } : x)) })}
                      style={{ flex: 1 }}
                    />
                    <NumberInput
                      aria-label={`Bundle service ${i + 1} quantity`}
                      min={1}
                      allowDecimal={false}
                      w={80}
                      value={it.qty}
                      onChange={(v) => setEdit({ ...edit, items: edit.items.map((x, j) => (j === i ? { ...x, qty: Number(v) || 1 } : x)) })}
                    />
                    <ActionIcon variant="subtle" color="gray" aria-label="Remove" onClick={() => setEdit({ ...edit, items: edit.items.filter((_, j) => j !== i) })}>
                      <IconTrash size={16} />
                    </ActionIcon>
                  </Group>
                ))}
                <Button variant="subtle" size="xs" w="fit-content" leftSection={<IconPlus size={14} />} onClick={() => setEdit({ ...edit, items: [...edit.items, { service_id: 0, qty: 1 }] })}>
                  Add service
                </Button>
                <DecimalInput label="Bundle price" unit="$" value={edit.price ?? ""} onChange={(v) => setEdit({ ...edit, price: v || null })} w={200} />
                {err && <Alert color="red">{err}</Alert>}
                <Group justify="flex-end">
                  <Button
                    loading={save.isPending}
                    onClick={() =>
                      save.mutate(
                        { bundle: { ...edit, items: edit.items.filter((x) => x.service_id) } },
                        { onSuccess: () => setEdit(null), onError: (e) => setErr(isAppError(e) ? e.message : String(e)) },
                      )
                    }
                  >
                    Save bundle
                  </Button>
                </Group>
              </Stack>
            )}
          </Modal>
        </Stack>
      )}
    </QueryState>
  );
}
