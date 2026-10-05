import { Button, Divider, Group, Paper, Stack, Table, Text } from "@mantine/core";
import { IconArrowLeft, IconPrinter } from "@tabler/icons-react";
import { useNavigate, useParams } from "react-router";
import { useCmd } from "../../api/queries";
import type { Business, EstimateView, Location, SaleView } from "../../api/types";
import { QueryState } from "../../components/QueryState";
import { dateLabel, isZeroDec, money, num } from "../../lib/format";

function Letterhead() {
  const biz = useCmd<Business>("business_get");
  const locs = useCmd<Location[]>("locations_list");
  const loc = locs.data?.find((l) => l.id === biz.data?.primary_location_id);
  return (
    <Stack gap={0} mb="md">
      <Text className="srm-figure" fz={26}>
        {biz.data?.name}
      </Text>
      {loc && (
        <Text size="sm" c="dimmed">
          {[loc.address_line, loc.city, [loc.state, loc.postal_code].filter(Boolean).join(" ")].filter(Boolean).join(", ")}
        </Text>
      )}
    </Stack>
  );
}

function Row({ label, value, bold }: { label: string; value: string; bold?: boolean }) {
  return (
    <Group justify="space-between">
      <Text size="sm" fw={bold ? 700 : undefined}>
        {label}
      </Text>
      <Text size="sm" fw={bold ? 700 : undefined} className="srm-num">
        {value}
      </Text>
    </Group>
  );
}

function Receipt({ v }: { v: SaleView }) {
  return (
    <>
      <Text fw={650}>Receipt {v.number}</Text>
      <Text size="sm" c="dimmed" mb="md">
        {dateLabel(v.draft.sale_date)}
        {v.client_name ? `, ${v.client_name}` : ""}
        {v.staff_name ? `, with ${v.staff_name}` : ""}
      </Text>
      <Table fz="sm" mb="md">
        <Table.Tbody>
          {v.draft.lines.map((l, i) => (
            <Table.Tr key={i}>
              <Table.Td>
                {l.description}
                {l.kind !== "tip" && Number(l.qty) !== 1 ? ` × ${num(l.qty)}` : ""}
              </Table.Td>
              <Table.Td ta="right" className="srm-num">
                {money(v.totals.lines[i]?.gross)}
              </Table.Td>
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
      <Stack gap={2}>
        {!isZeroDec(v.totals.discount_total) && <Row label="Discounts included" value={`−${money(v.totals.discount_total)}`} />}
        <Row label={`Sales tax${v.tax_rate ? ` ${num(v.tax_rate, 3)}%` : ""}`} value={money(v.totals.tax_total)} />
        <Row label="Total" value={money(v.totals.total)} bold />
        {v.payments
          .filter((p) => !p.voided)
          .map((p) => (
            <Row key={p.id} label={p.refund_id ? `Refund ${p.reference}` : `Paid by ${p.method}`} value={money(p.amount)} />
          ))}
        <Row label="Balance" value={money(v.balance)} />
      </Stack>
      {v.status === "voided" && (
        <Text c="red" fw={700} mt="md">
          VOID — {v.void_reason}
        </Text>
      )}
    </>
  );
}

function EstimateDoc({ e }: { e: EstimateView }) {
  return (
    <>
      <Text fw={650}>Estimate</Text>
      <Text size="sm" c="dimmed" mb="md">
        For {e.client_name || "client"}, issued {dateLabel(e.input.issued_on)}
        {e.input.valid_until ? `, valid until ${dateLabel(e.input.valid_until)}` : ""}
      </Text>
      <Table fz="sm" mb="md">
        <Table.Tbody>
          {e.input.lines.map((l, i) => (
            <Table.Tr key={i}>
              <Table.Td>
                {l.description}
                {l.qty !== 1 ? ` × ${l.qty}` : ""}
              </Table.Td>
              <Table.Td ta="right" className="srm-num">
                {money(l.unit_price)}
              </Table.Td>
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
      <Row label="Estimated total before tax" value={money(e.subtotal)} bold />
      <Text size="xs" c="dimmed" mt="xs">
        Sales tax, if applicable, is added at checkout. Final price can change if the service changes.
      </Text>
      {e.input.notes && (
        <Text size="sm" mt="md">
          {e.input.notes}
        </Text>
      )}
    </>
  );
}

export function PrintPage() {
  const { kind, id } = useParams();
  const navigate = useNavigate();
  const sale = useCmd<SaleView>("sale_get", { id: Number(id) }, kind === "sale");
  const est = useCmd<EstimateView>("estimate_get", { id: Number(id) }, kind === "estimate");
  const q = kind === "sale" ? sale : est;
  return (
    <Stack maw={560} mx="auto">
      <Group justify="space-between" className="srm-no-print">
        <Button variant="subtle" leftSection={<IconArrowLeft size={16} />} onClick={() => navigate(-1)}>
          Back
        </Button>
        <Button leftSection={<IconPrinter size={16} />} onClick={() => window.print()}>
          Print or save as PDF
        </Button>
      </Group>
      <Paper p="xl" className="srm-print-sheet">
        <QueryState loading={q.isLoading} error={q.error}>
          {() => (
            <>
              <Letterhead />
              <Divider mb="md" />
              {kind === "sale" ? <Receipt v={sale.data!} /> : <EstimateDoc e={est.data!} />}
            </>
          )}
        </QueryState>
      </Paper>
      <Text size="xs" c="dimmed" className="srm-no-print">
        Choose "Print to File" in the print dialog to save a PDF.
      </Text>
    </Stack>
  );
}
