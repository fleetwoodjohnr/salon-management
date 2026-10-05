import { Badge, Button, Group, Switch, Text } from "@mantine/core";
import { useHotkeys } from "@mantine/hooks";
import { IconPlus, IconUserCircle } from "@tabler/icons-react";
import { DataTable } from "mantine-datatable";
import { useState } from "react";
import { useNavigate } from "react-router";
import { useCmd } from "../../api/queries";
import type { ProfileView } from "../../api/types";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { money, num, pct } from "../../lib/format";
import { EmptyState } from "@mantine/core";
import { compLabels, kindLabels, targetLabels } from "./labels";

export function ProfilesPage() {
  const navigate = useNavigate();
  const [archived, setArchived] = useState(false);
  const q = useCmd<ProfileView[]>("profiles_list", { includeArchived: archived });
  useHotkeys([["mod+N", () => navigate("/profiles/new")]]);

  return (
    <>
      <PageHeader
        title="Work profiles"
        description="A work profile describes how someone's time is paid for and how much overhead they carry. Service costs and price recommendations start here."
        actions={
          <Button leftSection={<IconPlus size={16} />} onClick={() => navigate("/profiles/new")}>
            New profile
          </Button>
        }
      />
      <QueryState loading={q.isLoading} error={q.error}>
        {() =>
          q.data!.length === 0 && !archived ? (
            <EmptyState
              icon={<IconUserCircle size={32} />}
              title="No work profiles yet"
              description="Create one for yourself first. Add more for employees or renters who are costed differently."
              mt="xl"
            >
              <Button mt="md" onClick={() => navigate("/profiles/new")}>
                Create a profile
              </Button>
            </EmptyState>
          ) : (
            <>
              <Group justify="flex-end" mb="xs">
                <Switch size="xs" label="Show archived" checked={archived} onChange={(e) => setArchived(e.currentTarget.checked)} />
              </Group>
              <DataTable
                withTableBorder
                borderRadius="lg"
                highlightOnHover
                records={q.data!}
                idAccessor="id"
                onRowClick={({ record }) => navigate(`/profiles/${record.id}`)}
                columns={[
                  {
                    accessor: "name",
                    render: (p) => (
                      <Group gap="xs">
                        <Text size="sm" fw={600}>
                          {p.name}
                        </Text>
                        {p.archived && <Badge color="gray">Archived</Badge>}
                      </Group>
                    ),
                  },
                  { accessor: "kind", title: "Type", render: (p) => kindLabels[p.data.kind] },
                  { accessor: "comp", title: "Pay model", render: (p) => compLabels[p.data.comp_model].label },
                  {
                    accessor: "billable",
                    title: "Billable h / month",
                    textAlign: "right",
                    render: (p) => num(p.rates?.monthly_billable_hours, 1),
                  },
                  {
                    accessor: "ovh",
                    title: "Overhead / billable h",
                    textAlign: "right",
                    render: (p) => money(p.rates?.overhead_per_billable_hour),
                  },
                  {
                    accessor: "labor",
                    title: "Pay / billable h",
                    textAlign: "right",
                    render: (p) => (p.rates && p.data.comp_model === "commission" ? `${pct(p.rates.commission_pct)} comm.` : money(p.rates?.labor_cost_per_billable_hour)),
                  },
                  {
                    accessor: "target",
                    title: "Target",
                    textAlign: "right",
                    render: (p) => `${pct(p.data.target_pct)} ${targetLabels[p.data.target_kind].toLowerCase()}`,
                  },
                  { accessor: "version", title: "Version", textAlign: "right", render: (p) => `v${p.version}` },
                ]}
              />
            </>
          )
        }
      </QueryState>
    </>
  );
}
