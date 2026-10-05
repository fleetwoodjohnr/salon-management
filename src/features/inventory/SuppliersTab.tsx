import { Badge, Button, Group, Modal, Paper, Select, SimpleGrid, Stack, Table, Text, TextInput, Textarea, Title } from "@mantine/core";
import { IconPlus } from "@tabler/icons-react";
import { useState } from "react";
import { isAppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { Location, StorageLocation, Supplier } from "../../api/types";

const blankSupplier: Supplier = { id: null, name: "", contact: "", phone: "", email: "", website: "", notes: "", archived: false };

export function SuppliersTab() {
  const suppliers = useCmd<Supplier[]>("suppliers_list");
  const storage = useCmd<StorageLocation[]>("storage_list");
  const locations = useCmd<Location[]>("locations_list");
  const [sup, setSup] = useState<Supplier | null>(null);
  const [st, setSt] = useState<StorageLocation | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const saveSup = useAction<{ supplier: Supplier }>("supplier_save", { invalidate: ["suppliers_list", "products_list"], success: "Supplier saved", silentError: true });
  const archSup = useAction<{ id: number; archived: boolean }>("supplier_archive", { invalidate: ["suppliers_list"] });
  const saveSt = useAction<{ storage: StorageLocation }>("storage_save", { invalidate: ["storage_list", "products_list"], success: "Storage spot saved", silentError: true });
  const archSt = useAction<{ id: number; archived: boolean }>("storage_archive", { invalidate: ["storage_list"] });
  const onErr = (e: unknown) => setErr(isAppError(e) ? e.message : String(e));

  return (
    <SimpleGrid cols={{ base: 1, xl: 2 }} spacing="lg">
      <Paper p="lg">
        <Group justify="space-between" mb="sm">
          <Title order={4}>Suppliers</Title>
          <Button size="xs" leftSection={<IconPlus size={14} />} onClick={() => (setErr(null), setSup(blankSupplier))}>
            Add supplier
          </Button>
        </Group>
        <Table fz="sm">
          <Table.Tbody>
            {(suppliers.data ?? []).length === 0 && (
              <Table.Tr>
                <Table.Td>
                  <Text size="sm" c="dimmed">
                    No suppliers yet.
                  </Text>
                </Table.Td>
              </Table.Tr>
            )}
            {(suppliers.data ?? []).map((s) => (
              <Table.Tr key={s.id} style={{ opacity: s.archived ? 0.55 : 1 }}>
                <Table.Td>
                  <Text size="sm" fw={600}>
                    {s.name} {s.archived && <Badge color="gray">Archived</Badge>}
                  </Text>
                  <Text size="xs" c="dimmed">
                    {[s.contact, s.phone, s.email].filter(Boolean).join(", ")}
                  </Text>
                </Table.Td>
                <Table.Td>
                  <Group gap="xs" justify="flex-end">
                    <Button size="compact-xs" variant="default" onClick={() => (setErr(null), setSup(s))}>
                      Edit
                    </Button>
                    <Button size="compact-xs" variant="subtle" color="gray" onClick={() => archSup.mutate({ id: s.id!, archived: !s.archived })}>
                      {s.archived ? "Restore" : "Archive"}
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
          <Title order={4}>Storage spots</Title>
          <Button size="xs" leftSection={<IconPlus size={14} />} onClick={() => (setErr(null), setSt({ id: null, name: "", location_id: null, archived: false }))}>
            Add storage spot
          </Button>
        </Group>
        <Text size="xs" c="dimmed" mb="sm">
          Shelves, rooms or stations where stock is kept, e.g. Back room or Color bar. Quantities are tracked per spot; cost is tracked per product.
        </Text>
        <Table fz="sm">
          <Table.Tbody>
            {(storage.data ?? []).map((s) => (
              <Table.Tr key={s.id} style={{ opacity: s.archived ? 0.55 : 1 }}>
                <Table.Td>
                  <Text size="sm" fw={600}>
                    {s.name}
                  </Text>
                  <Text size="xs" c="dimmed">
                    {locations.data?.find((l) => l.id === s.location_id)?.name ?? ""}
                  </Text>
                </Table.Td>
                <Table.Td>
                  <Group gap="xs" justify="flex-end">
                    <Button size="compact-xs" variant="default" onClick={() => (setErr(null), setSt(s))}>
                      Edit
                    </Button>
                    <Button size="compact-xs" variant="subtle" color="gray" onClick={() => archSt.mutate({ id: s.id!, archived: !s.archived })}>
                      {s.archived ? "Restore" : "Archive"}
                    </Button>
                  </Group>
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      </Paper>
      <Modal opened={!!sup} onClose={() => setSup(null)} title={sup?.id ? "Edit supplier" : "Add supplier"}>
        {sup && (
          <form onSubmit={(e) => (e.preventDefault(), saveSup.mutate({ supplier: sup }, { onSuccess: () => setSup(null), onError: onErr }))}>
            <Stack>
              <TextInput label="Name" required value={sup.name} onChange={(e) => setSup({ ...sup, name: e.currentTarget.value })} data-autofocus />
              <SimpleGrid cols={2}>
                <TextInput label="Contact person" value={sup.contact} onChange={(e) => setSup({ ...sup, contact: e.currentTarget.value })} />
                <TextInput label="Phone" value={sup.phone} onChange={(e) => setSup({ ...sup, phone: e.currentTarget.value })} />
                <TextInput label="Email" value={sup.email} onChange={(e) => setSup({ ...sup, email: e.currentTarget.value })} />
                <TextInput label="Website" value={sup.website} onChange={(e) => setSup({ ...sup, website: e.currentTarget.value })} />
              </SimpleGrid>
              <Textarea label="Notes" value={sup.notes} onChange={(e) => setSup({ ...sup, notes: e.currentTarget.value })} />
              {err && <Text c="red" size="sm">{err}</Text>}
              <Group justify="flex-end">
                <Button type="submit" loading={saveSup.isPending}>
                  Save supplier
                </Button>
              </Group>
            </Stack>
          </form>
        )}
      </Modal>
      <Modal opened={!!st} onClose={() => setSt(null)} title={st?.id ? "Edit storage spot" : "Add storage spot"}>
        {st && (
          <form onSubmit={(e) => (e.preventDefault(), saveSt.mutate({ storage: st }, { onSuccess: () => setSt(null), onError: onErr }))}>
            <Stack>
              <TextInput label="Name" required value={st.name} onChange={(e) => setSt({ ...st, name: e.currentTarget.value })} data-autofocus />
              <Select
                label="Business location"
                clearable
                data={(locations.data ?? []).filter((l) => !l.archived).map((l) => ({ value: String(l.id), label: l.name }))}
                value={st.location_id != null ? String(st.location_id) : null}
                onChange={(v) => setSt({ ...st, location_id: v ? Number(v) : null })}
              />
              {err && <Text c="red" size="sm">{err}</Text>}
              <Group justify="flex-end">
                <Button type="submit" loading={saveSt.isPending}>
                  Save
                </Button>
              </Group>
            </Stack>
          </form>
        )}
      </Modal>
    </SimpleGrid>
  );
}
