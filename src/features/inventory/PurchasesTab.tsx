import { Anchor, Badge, Button, Group, Modal, Stack, Table, Text, TextInput, Textarea } from "@mantine/core";
import { IconPaperclip } from "@tabler/icons-react";
import { DataTable } from "mantine-datatable";
import { useState } from "react";
import { call } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { PurchaseRow } from "../../api/types";
import { QueryState } from "../../components/QueryState";
import { useToday } from "../../lib/dates";
import { dateLabel, money, num, unitMoney } from "../../lib/format";
import { unitShort } from "../../lib/units";

export function PurchaseLines({ p }: { p: PurchaseRow }) {
  return (
    <Table fz="sm" withRowBorders={false}>
      <Table.Thead>
        <Table.Tr>
          <Table.Th>Product</Table.Th>
          <Table.Th>Bought</Table.Th>
          <Table.Th ta="right">Line price</Table.Th>
          <Table.Th ta="right">Landed cost</Table.Th>
          <Table.Th ta="right">Per unit</Table.Th>
          <Table.Th>Lot / expiry</Table.Th>
        </Table.Tr>
      </Table.Thead>
      <Table.Tbody>
        {p.lines.map((l) => (
          <Table.Tr key={l.id}>
            <Table.Td>{l.product_name}</Table.Td>
            <Table.Td>
              {num(l.package_count)} × {num(l.contents_per_package)} {unitShort(l.unit)}
            </Table.Td>
            <Table.Td className="srm-num">{money(l.line_price)}</Table.Td>
            <Table.Td className="srm-num">{money(l.landed_cost)}</Table.Td>
            <Table.Td className="srm-num">
              {unitMoney(l.unit_cost)} / {unitShort(l.stock_unit)}
            </Table.Td>
            <Table.Td>{[l.lot_code, l.expires_on && `expires ${dateLabel(l.expires_on)}`].filter(Boolean).join(", ") || "—"}</Table.Td>
          </Table.Tr>
        ))}
      </Table.Tbody>
    </Table>
  );
}

export function PurchasesTab() {
  const today = useToday();
  const q = useCmd<PurchaseRow[]>("purchases_list", {});
  const [expanded, setExpanded] = useState<number[]>([]);
  const [reversing, setReversing] = useState<PurchaseRow | null>(null);
  const [note, setNote] = useState("");
  const [date, setDate] = useState(today);
  const rev = useAction<{ id: number; date: string; note: string }>("purchase_reverse", {
    invalidate: ["purchases_list", "products_list", "product_get", "ledger_list", "reorder_list"],
    success: "Purchase reversed",
  });
  return (
    <QueryState loading={q.isLoading} error={q.error}>
      {() => (
        <>
          <DataTable
            withTableBorder
            borderRadius="lg"
            records={q.data!}
            idAccessor="id"
            minHeight={q.data!.length ? undefined : 140}
            noRecordsText="No purchases yet. Use Receive stock to record one."
            rowExpansion={{
              allowMultiple: true,
              expanded: { recordIds: expanded, onRecordIdsChange: setExpanded },
              content: ({ record }) => (
                <Stack p="sm" gap="xs">
                  <PurchaseLines p={record} />
                  <Text size="xs" c="dimmed">
                    Subtotal {money(record.subtotal)}, discount {money(record.discount)}, shipping {money(record.shipping)}, non-recoverable tax{" "}
                    {money(record.nonrecoverable_tax)}. Extras are spread across lines in proportion to their price.
                  </Text>
                  {record.reversed_at && (
                    <Text size="xs" c="red">
                      Reversed: {record.reversal_note}
                    </Text>
                  )}
                </Stack>
              ),
            }}
            columns={[
              { accessor: "purchase_date", title: "Date", render: (p) => dateLabel(p.purchase_date) },
              {
                accessor: "supplier_name",
                title: "Supplier",
                render: (p) => (
                  <Group gap={6}>
                    <Text size="sm">{p.supplier_name ?? "—"}</Text>
                    {p.kind === "opening_balance" && <Badge color="gray">Opening stock</Badge>}
                    {p.reversed_at && <Badge color="red">Reversed</Badge>}
                  </Group>
                ),
              },
              { accessor: "invoice_ref", title: "Invoice" },
              { accessor: "lines", title: "Lines", textAlign: "right", render: (p) => p.lines.length },
              { accessor: "total", title: "Total", textAlign: "right", render: (p) => money(p.total) },
              {
                accessor: "attachment",
                title: "",
                render: (p) =>
                  p.attachment_id ? (
                    <Anchor
                      size="sm"
                      onClick={(e: React.MouseEvent) => {
                        e.stopPropagation();
                        call("attachment_open", { id: p.attachment_id });
                      }}
                    >
                      <Group gap={4}>
                        <IconPaperclip size={14} /> Receipt
                      </Group>
                    </Anchor>
                  ) : null,
              },
              {
                accessor: "actions",
                title: "",
                render: (p) =>
                  !p.reversed_at ? (
                    <Button
                      size="compact-xs"
                      variant="subtle"
                      onClick={(e) => {
                        e.stopPropagation();
                        setReversing(p);
                        setNote("");
                        setDate(today);
                      }}
                    >
                      Reverse
                    </Button>
                  ) : null,
              },
            ]}
          />
          <Modal opened={!!reversing} onClose={() => setReversing(null)} title="Reverse this purchase?">
            {reversing && (
              <Stack>
                <Text size="sm">
                  Every line's stock and value is taken back out with correcting ledger entries. Use this for a purchase entered by mistake. Material already used in
                  services keeps the cost it was assigned.
                </Text>
                <TextInput type="date" label="Date of correction" value={date} onChange={(e) => setDate(e.currentTarget.value)} w={200} />
                <Textarea label="Reason" required value={note} onChange={(e) => setNote(e.currentTarget.value)} />
                <Group justify="flex-end">
                  <Button variant="default" onClick={() => setReversing(null)}>
                    Cancel
                  </Button>
                  <Button color="red" disabled={!note.trim()} loading={rev.isPending} onClick={() => rev.mutate({ id: reversing.id, date, note }, { onSuccess: () => setReversing(null) })}>
                    Reverse purchase
                  </Button>
                </Group>
              </Stack>
            )}
          </Modal>
        </>
      )}
    </QueryState>
  );
}
