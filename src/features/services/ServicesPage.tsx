import { Badge, Button, EmptyState, Group, Stack, Switch, Tabs, Text, TextInput, Tooltip } from "@mantine/core";
import { useDebouncedValue, useHotkeys } from "@mantine/hooks";
import { IconPlus, IconScissors, IconSearch } from "@tabler/icons-react";
import { DataTable } from "mantine-datatable";
import { useMemo, useState } from "react";
import { useNavigate } from "react-router";
import { useCmd } from "../../api/queries";
import type { ServiceStatus, ServiceSummary } from "../../api/types";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { minutesLabel, money, pct } from "../../lib/format";
import { BundlesTab } from "./BundlesTab";

export const statusBadge: Record<ServiceStatus, { label: string; color: string }> = {
  ok: { label: "On target", color: "teal" },
  below_target: { label: "Below target", color: "yellow" },
  below_cost: { label: "Below cost", color: "red" },
  no_price: { label: "No price", color: "gray" },
  error: { label: "Needs setup", color: "orange" },
};

export function ServicesPage() {
  const navigate = useNavigate();
  const [archived, setArchived] = useState(false);
  const q = useCmd<ServiceSummary[]>("services_list", { includeArchived: archived });
  const [search, setSearch] = useState("");
  const [debounced] = useDebouncedValue(search, 150);
  useHotkeys([["mod+N", () => navigate("/services/new")]]);
  const rows = useMemo(() => {
    const s = debounced.trim().toLowerCase();
    return (q.data ?? []).filter((r) => !s || r.name.toLowerCase().includes(s) || r.category.toLowerCase().includes(s));
  }, [q.data, debounced]);
  return (
    <>
      <PageHeader
        title="Services"
        description="Each service's real cost: products from its recipe, labor, overhead and fees. Prices are compared with your profit target."
        actions={
          <Button leftSection={<IconPlus size={16} />} onClick={() => navigate("/services/new")}>
            New service
          </Button>
        }
      />
      <Tabs defaultValue="services" keepMounted={false}>
        <Tabs.List mb="md">
          <Tabs.Tab value="services">Services</Tabs.Tab>
          <Tabs.Tab value="bundles">Bundles</Tabs.Tab>
        </Tabs.List>
        <Tabs.Panel value="bundles">
          <BundlesTab />
        </Tabs.Panel>
        <Tabs.Panel value="services">
      <QueryState loading={q.isLoading} error={q.error}>
        {() =>
          q.data!.length === 0 && !archived ? (
            <EmptyState icon={<IconScissors size={32} />} title="No services yet" description="Create a service, list the products it uses, and see what it really costs." mt="xl">
              <Button mt="md" onClick={() => navigate("/services/new")}>
                Create a service
              </Button>
            </EmptyState>
          ) : (
            <Stack gap="sm">
              <Group justify="space-between">
                <TextInput aria-label="Search services" placeholder="Search services" leftSection={<IconSearch size={16} />} value={search} onChange={(e) => setSearch(e.currentTarget.value)} w={300} />
                <Switch size="xs" label="Archived" checked={archived} onChange={(e) => setArchived(e.currentTarget.checked)} />
              </Group>
              <DataTable
                withTableBorder
                borderRadius="lg"
                highlightOnHover
                records={rows}
                idAccessor="id"
                minHeight={rows.length ? undefined : 140}
                noRecordsText="No services match"
                onRowClick={({ record }) => navigate(`/services/${record.id}`)}
                columns={[
                  {
                    accessor: "name",
                    title: "Service",
                    render: (s) => (
                      <div>
                        <Group gap={6}>
                          <Text size="sm" fw={600}>
                            {s.name}
                          </Text>
                          {s.is_addon && <Badge color="gray">Add-on</Badge>}
                          {s.archived && <Badge color="gray">Archived</Badge>}
                        </Group>
                        <Text size="xs" c="dimmed">
                          {[s.category, s.profile_name].filter(Boolean).join(", ") || " "}
                        </Text>
                      </div>
                    ),
                  },
                  { accessor: "duration_min", title: "Chair time", textAlign: "right", render: (s) => minutesLabel(s.duration_min) },
                  { accessor: "price", title: "Price", textAlign: "right", render: (s) => money(s.price) },
                  { accessor: "estimated_cost", title: "Est. cost", textAlign: "right", render: (s) => money(s.estimated_cost) },
                  { accessor: "margin_pct", title: "Margin", textAlign: "right", render: (s) => pct(s.margin_pct) },
                  { accessor: "target_price", title: "Target price", textAlign: "right", render: (s) => money(s.target_price) },
                  {
                    accessor: "status",
                    title: "",
                    render: (s) => (
                      <Tooltip label={s.status_detail} disabled={!s.status_detail} multiline w={260}>
                        <Badge color={statusBadge[s.status].color}>{statusBadge[s.status].label}</Badge>
                      </Tooltip>
                    ),
                  },
                ]}
              />
              <Text size="xs" c="dimmed">
                Estimated cost includes materials at current average cost, labor, overhead and fees at the listed price.
              </Text>
            </Stack>
          )
        }
      </QueryState>
        </Tabs.Panel>
      </Tabs>
    </>
  );
}
