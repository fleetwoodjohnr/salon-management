import { Alert, Anchor, Badge, Button, Group, Modal, Select, SimpleGrid, Stack, Text, TextInput, Textarea } from "@mantine/core";
import { useHotkeys } from "@mantine/hooks";
import { IconPaperclip, IconPlus } from "@tabler/icons-react";
import { open } from "@tauri-apps/plugin-dialog";
import { DataTable } from "mantine-datatable";
import { useState } from "react";
import { call, isAppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { Attachment, Expense, ExpenseInput, Location } from "../../api/types";
import { DecimalInput } from "../../components/DecimalInput";
import { PageHeader } from "../../components/PageHeader";
import { QueryState } from "../../components/QueryState";
import { useToday } from "../../lib/dates";
import { dateLabel, money } from "../../lib/format";

export function ExpensesPage() {
  const today = useToday();
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const q = useCmd<Expense[]>("expenses_list", { from: from || null, to: to || null });
  const cats = useCmd<[string, string][]>("expense_categories");
  const locs = useCmd<Location[]>("locations_list");
  const catLabel = (c: string) => cats.data?.find((x) => x[0] === c)?.[1] ?? c;
  const [edit, setEdit] = useState<ExpenseInput | null>(null);
  const [att, setAtt] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [voiding, setVoiding] = useState<Expense | null>(null);
  const [reason, setReason] = useState("");
  const save = useAction<{ expense: ExpenseInput }, number>("expense_save", { invalidate: ["expenses_list"], success: "Expense saved", silentError: true });
  const voidE = useAction<{ id: number; reason: string }>("expense_void", { invalidate: ["expenses_list"], success: "Expense voided" });
  const blank = (): ExpenseInput => ({ id: null, expense_date: today, category: "rent", vendor: "", description: "", amount: "", payment_method: "card", location_id: null, attachment_id: null });
  useHotkeys([["mod+N", () => setEdit(blank())]]);
  async function attach() {
    const path = await open({ title: "Attach receipt", filters: [{ name: "Receipt", extensions: ["pdf", "png", "jpg", "jpeg", "webp", "heic", "gif", "txt"] }] });
    if (typeof path !== "string" || !edit) return;
    try {
      const a = await call<Attachment>("attachment_add", { path });
      setEdit({ ...edit, attachment_id: a.id });
      setAtt(a.file_name);
    } catch (e) {
      setErr(isAppError(e) ? e.message : String(e));
    }
  }
  return (
    <>
      <PageHeader
        title="Expenses"
        description="Rent, utilities, software and other spending. Product purchases belong in Inventory, so reports can tell cash spent on stock from stock actually used."
        actions={
          <Button leftSection={<IconPlus size={16} />} onClick={() => (setErr(null), setAtt(null), setEdit(blank()))}>
            Add expense
          </Button>
        }
      />
      <Group gap="sm" mb="sm">
        <TextInput size="xs" type="date" label="From" value={from} onChange={(e) => setFrom(e.currentTarget.value)} />
        <TextInput size="xs" type="date" label="To" value={to} onChange={(e) => setTo(e.currentTarget.value)} />
      </Group>
      <QueryState loading={q.isLoading} error={q.error}>
        {() => (
          <DataTable
            withTableBorder
            borderRadius="lg"
            records={q.data!}
            idAccessor={(e) => e.input.id!}
            minHeight={q.data!.length ? undefined : 140}
            noRecordsText="No expenses recorded"
            columns={[
              { accessor: "date", title: "Date", render: (e) => dateLabel(e.input.expense_date) },
              { accessor: "category", title: "Category", render: (e) => catLabel(e.input.category) },
              { accessor: "vendor", title: "Vendor", render: (e) => e.input.vendor || "—" },
              { accessor: "description", title: "Description", render: (e) => e.input.description },
              { accessor: "amount", title: "Amount", textAlign: "right", render: (e) => <Text size="sm" td={e.voided ? "line-through" : undefined}>{money(e.input.amount)}</Text> },
              {
                accessor: "x",
                title: "",
                render: (e) => (
                  <Group gap={4} justify="flex-end">
                    {e.input.attachment_id && (
                      <Anchor size="xs" onClick={() => call("attachment_open", { id: e.input.attachment_id })}>
                        <IconPaperclip size={14} />
                      </Anchor>
                    )}
                    {e.voided ? (
                      <Badge color="gray">Voided</Badge>
                    ) : (
                      <>
                        <Button size="compact-xs" variant="subtle" onClick={() => (setErr(null), setAtt(e.attachment_name), setEdit(e.input))}>
                          Edit
                        </Button>
                        <Button size="compact-xs" variant="subtle" color="gray" onClick={() => (setReason(""), setVoiding(e))}>
                          Void
                        </Button>
                      </>
                    )}
                  </Group>
                ),
              },
            ]}
          />
        )}
      </QueryState>
      <Modal opened={!!edit} onClose={() => setEdit(null)} title={edit?.id ? "Edit expense" : "Add expense"}>
        {edit && (
          <Stack>
            <SimpleGrid cols={2}>
              <TextInput type="date" label="Date" value={edit.expense_date} onChange={(e) => setEdit({ ...edit, expense_date: e.currentTarget.value })} />
              <DecimalInput label="Amount" unit="$" value={edit.amount} onChange={(v) => setEdit({ ...edit, amount: v })} data-autofocus />
              <Select label="Category" data={(cats.data ?? []).map(([value, label]) => ({ value, label }))} value={edit.category} allowDeselect={false} onChange={(v) => setEdit({ ...edit, category: v ?? "other" })} />
              <TextInput label="Vendor" value={edit.vendor} onChange={(e) => setEdit({ ...edit, vendor: e.currentTarget.value })} />
              <Select label="Paid by" data={["card", "cash", "check", "transfer", "other"]} value={edit.payment_method} onChange={(v) => setEdit({ ...edit, payment_method: v ?? "" })} />
              <Select label="Location" clearable data={(locs.data ?? []).map((l) => ({ value: String(l.id), label: l.name }))} value={edit.location_id != null ? String(edit.location_id) : null} onChange={(v) => setEdit({ ...edit, location_id: v ? Number(v) : null })} />
            </SimpleGrid>
            <Textarea label="Description" value={edit.description} onChange={(e) => setEdit({ ...edit, description: e.currentTarget.value })} />
            <Group gap="sm">
              <Button size="xs" variant="default" leftSection={<IconPaperclip size={14} />} onClick={attach}>
                {att ? "Replace receipt" : "Attach receipt"}
              </Button>
              {att && <Text size="xs" c="dimmed">{att}</Text>}
            </Group>
            {err && <Alert color="red">{err}</Alert>}
            <Group justify="flex-end">
              <Button loading={save.isPending} onClick={() => save.mutate({ expense: edit }, { onSuccess: () => setEdit(null), onError: (e) => setErr(isAppError(e) ? e.message : String(e)) })}>
                Save expense
              </Button>
            </Group>
          </Stack>
        )}
      </Modal>
      <Modal opened={!!voiding} onClose={() => setVoiding(null)} title="Void this expense?">
        <Stack>
          <Text size="sm">The expense stays in the list, marked void, and is left out of reports.</Text>
          <Textarea label="Reason" value={reason} onChange={(e) => setReason(e.currentTarget.value)} />
          <Group justify="flex-end">
            <Button color="red" disabled={!reason.trim()} onClick={() => voidE.mutate({ id: voiding!.input.id!, reason }, { onSuccess: () => setVoiding(null) })}>
              Void expense
            </Button>
          </Group>
        </Stack>
      </Modal>
    </>
  );
}
