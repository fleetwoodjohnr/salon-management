import { Button, Group, Paper, SimpleGrid, Stack, Table, Text, Title } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconCopy, IconPrinter } from "@tabler/icons-react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useCmd } from "../../api/queries";
import type { ExpiringLot, ReorderItem } from "../../api/types";
import { QueryState } from "../../components/QueryState";
import { addDays, useToday } from "../../lib/dates";
import { dateLabel, num, unitMoney } from "../../lib/format";
import { unitShort } from "../../lib/units";

export function ReorderTab() {
  const today = useToday();
  const reorder = useCmd<ReorderItem[]>("reorder_list");
  const expiring = useCmd<ExpiringLot[]>("expiring_lots", { until: addDays(today, 60) });

  async function copy() {
    const lines = (reorder.data ?? []).map((r) => `${[r.brand, r.name].filter(Boolean).join(" ")}: order ${num(r.suggested_qty)} ${unitShort(r.stock_unit)}${r.supplier_name ? ` (${r.supplier_name})` : ""}`);
    await writeText(`Reorder list ${today}\n${lines.join("\n")}`);
    notifications.show({ message: "Reorder list copied", color: "teal" });
  }

  return (
    <SimpleGrid cols={{ base: 1, xl: 2 }} spacing="lg">
      <Paper p="lg">
        <Group justify="space-between" mb="sm">
          <Title order={4}>Reorder list</Title>
          <Group gap="xs" className="srm-no-print">
            <Button size="xs" variant="default" leftSection={<IconCopy size={14} />} onClick={copy} disabled={!reorder.data?.length}>
              Copy
            </Button>
            <Button size="xs" variant="default" leftSection={<IconPrinter size={14} />} onClick={() => window.print()} disabled={!reorder.data?.length}>
              Print
            </Button>
          </Group>
        </Group>
        <QueryState loading={reorder.isLoading} error={reorder.error}>
          {() =>
            reorder.data!.length === 0 ? (
              <Text size="sm" c="dimmed">
                Nothing is at or below its reorder point. Set reorder points on products to get alerts here.
              </Text>
            ) : (
              <Table fz="sm">
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>Product</Table.Th>
                    <Table.Th ta="right">On hand</Table.Th>
                    <Table.Th ta="right">Reorder at</Table.Th>
                    <Table.Th ta="right">Suggested</Table.Th>
                    <Table.Th>Supplier</Table.Th>
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {reorder.data!.map((r) => (
                    <Table.Tr key={r.product_id}>
                      <Table.Td>{[r.brand, r.name].filter(Boolean).join(" ")}</Table.Td>
                      <Table.Td className="srm-num">
                        {num(r.on_hand)} {unitShort(r.stock_unit)}
                      </Table.Td>
                      <Table.Td className="srm-num">{num(r.reorder_point)}</Table.Td>
                      <Table.Td className="srm-num">
                        {num(r.suggested_qty)} {unitShort(r.stock_unit)}
                        {r.last_unit_cost && (
                          <Text size="xs" c="dimmed">
                            ~{unitMoney(r.last_unit_cost)} each
                          </Text>
                        )}
                      </Table.Td>
                      <Table.Td>{r.supplier_name ?? "—"}</Table.Td>
                    </Table.Tr>
                  ))}
                </Table.Tbody>
              </Table>
            )
          }
        </QueryState>
        <Text size="xs" c="dimmed" mt="sm">
          Suggested quantity is the product's reorder quantity, or enough to get back to twice the reorder point.
        </Text>
      </Paper>
      <Paper p="lg">
        <Title order={4} mb="sm">
          Expiring in the next 60 days
        </Title>
        <QueryState loading={expiring.isLoading} error={expiring.error}>
          {() =>
            expiring.data!.length === 0 ? (
              <Text size="sm" c="dimmed">
                No lots with an expiry date in the next 60 days. Add expiry dates when you receive stock to track this.
              </Text>
            ) : (
              <Stack gap={0}>
                <Table fz="sm">
                  <Table.Thead>
                    <Table.Tr>
                      <Table.Th>Product</Table.Th>
                      <Table.Th>Lot</Table.Th>
                      <Table.Th>Expires</Table.Th>
                      <Table.Th ta="right">Received</Table.Th>
                    </Table.Tr>
                  </Table.Thead>
                  <Table.Tbody>
                    {expiring.data!.map((l) => (
                      <Table.Tr key={l.purchase_line_id}>
                        <Table.Td>{l.product_name}</Table.Td>
                        <Table.Td>{l.lot_code || "—"}</Table.Td>
                        <Table.Td c={l.expires_on < today ? "var(--srm-bad)" : undefined}>{dateLabel(l.expires_on)}</Table.Td>
                        <Table.Td className="srm-num">
                          {num(l.qty_received)} {unitShort(l.stock_unit)} on {dateLabel(l.received_on)}
                        </Table.Td>
                      </Table.Tr>
                    ))}
                  </Table.Tbody>
                </Table>
                <Text size="xs" c="dimmed" mt="sm">
                  Average costing doesn't track which lot you used, so these lots may already be used up. Check the shelf.
                </Text>
              </Stack>
            )
          }
        </QueryState>
      </Paper>
    </SimpleGrid>
  );
}
