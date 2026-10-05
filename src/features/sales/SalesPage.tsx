import { Badge, Button, Group, SegmentedControl, Stack, Tabs, Text, TextInput } from "@mantine/core";
import { useHotkeys } from "@mantine/hooks";
import { IconPlus } from "@tabler/icons-react";
import { DataTable } from "mantine-datatable";
import { useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { useCmd } from "../../api/queries";
import type { SaleSummary } from "../../api/types";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { dateLabel, money } from "../../lib/format";
import { EstimatesTab } from "./EstimatesTab";

const statusColor: Record<string, string> = { draft: "yellow", finalized: "teal", voided: "gray" };

function SalesTab() {
  const navigate = useNavigate();
  const [status, setStatus] = useState<string>("all");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const q = useCmd<SaleSummary[]>("sales_list", { filter: { from: from || null, to: to || null, status: status === "all" ? null : status, client_id: null } });
  return (
    <Stack gap="sm">
      <Group gap="sm" align="flex-end">
        <SegmentedControl
          size="xs"
          value={status}
          onChange={setStatus}
          data={[
            { value: "all", label: "All" },
            { value: "draft", label: "Drafts" },
            { value: "finalized", label: "Finalized" },
            { value: "voided", label: "Voided" },
          ]}
        />
        <TextInput size="xs" type="date" label="From" value={from} onChange={(e) => setFrom(e.currentTarget.value)} />
        <TextInput size="xs" type="date" label="To" value={to} onChange={(e) => setTo(e.currentTarget.value)} />
      </Group>
      <QueryState loading={q.isLoading} error={q.error}>
        {() => (
          <DataTable
            withTableBorder
            borderRadius="lg"
            highlightOnHover
            records={q.data!}
            idAccessor="id"
            minHeight={q.data!.length ? undefined : 160}
            noRecordsText="No sales yet. Complete an appointment from the calendar, or start a new sale."
            onRowClick={({ record }) => navigate(`/sales/${record.id}`)}
            columns={[
              { accessor: "sale_date", title: "Date", render: (s) => dateLabel(s.sale_date) },
              { accessor: "number", title: "Sale", render: (s) => s.number ?? "Draft" },
              { accessor: "client_name", title: "Client", render: (s) => s.client_name || "Walk-in" },
              { accessor: "staff_name", title: "Staff", render: (s) => s.staff_name ?? "—" },
              { accessor: "lines", title: "Items", textAlign: "right" },
              { accessor: "total", title: "Total", textAlign: "right", render: (s) => money(s.total) },
              {
                accessor: "status",
                title: "",
                render: (s) => (
                  <Group gap={4}>
                    <Badge color={statusColor[s.status]}>{s.status === "finalized" ? "Finalized" : s.status === "draft" ? "Draft" : "Voided"}</Badge>
                    {s.refunded && <Badge color="orange">Refunded</Badge>}
                  </Group>
                ),
              },
            ]}
          />
        )}
      </QueryState>
      <Text size="xs" c="dimmed">
        Draft totals appear once finalized; drafts can still change.
      </Text>
    </Stack>
  );
}

export function SalesPage() {
  const navigate = useNavigate();
  const [params, setParams] = useSearchParams();
  useHotkeys([["mod+N", () => navigate("/sales/new")]]);
  return (
    <>
      <PageHeader
        title="Sales"
        description="Each sale records services, retail items, discounts, tips, tax and payments. Payments are recorded here, not processed."
        actions={
          <Button leftSection={<IconPlus size={16} />} onClick={() => navigate("/sales/new")}>
            New sale
          </Button>
        }
      />
      <Tabs value={params.get("tab") ?? "sales"} onChange={(v) => setParams({ tab: v ?? "sales" }, { replace: true })} keepMounted={false}>
        <Tabs.List mb="md">
          <Tabs.Tab value="sales">Sales</Tabs.Tab>
          <Tabs.Tab value="estimates">Estimates</Tabs.Tab>
        </Tabs.List>
        <Tabs.Panel value="sales">
          <SalesTab />
        </Tabs.Panel>
        <Tabs.Panel value="estimates">
          <EstimatesTab />
        </Tabs.Panel>
      </Tabs>
    </>
  );
}
