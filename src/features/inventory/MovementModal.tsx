import { Alert, Button, Group, Modal, Select, SimpleGrid, Stack, Text, TextInput, Textarea } from "@mantine/core";
import { useEffect, useState } from "react";
import { isAppError, type AppError } from "../../api/ipc";
import { useAction, useCmd } from "../../api/queries";
import type { MovementInput, Posted, ProductRow, StorageLocation, UnitDef } from "../../api/types";
import { DecimalInput } from "../../components/DecimalInput";
import { useToday } from "../../lib/dates";
import { money, num } from "../../lib/format";
import { unitOptions, unitShort } from "../../lib/units";

export type MovementKind = MovementInput["kind"];

const titles: Record<MovementKind, string> = {
  waste: "Record waste",
  adjustment: "Count adjustment",
  transfer: "Transfer between storage spots",
  supplier_return: "Return to supplier",
};
const help: Record<MovementKind, string> = {
  waste: "Product thrown away, spilled or expired. It's removed at the current average cost and reported as waste.",
  adjustment: "Enter what you actually counted. The difference from the recorded quantity is posted as an adjustment.",
  transfer: "Move stock between storage spots. Total quantity and value don't change.",
  supplier_return: "Stock sent back to the supplier. It's removed at the current average cost; record any refund as a credit in your notes.",
};

export function MovementModal({ product, kind, onClose }: { product: ProductRow; kind: MovementKind | null; onClose: () => void }) {
  const today = useToday();
  const units = useCmd<UnitDef[]>("units_list");
  const storage = useCmd<StorageLocation[]>("storage_list");
  const [m, setM] = useState<MovementInput | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  useEffect(() => {
    if (kind) {
      setError(null);
      setM({
        product_id: product.id,
        kind,
        qty: kind === "adjustment" ? product.on_hand.replace(/(\.\d*?)0+$/, "$1").replace(/\.$/, "") : "",
        unit: product.stock_unit,
        storage_id: product.default_storage_id,
        to_storage_id: null,
        unit_cost: null,
        occurred_on: today,
        note: "",
      });
    }
  }, [kind, product, today]);
  const rec = useAction<{ movement: MovementInput }, Posted | null>("movement_record", {
    invalidate: ["product_get", "products_list", "ledger_list", "reorder_list"],
    success: (p) => (p ? `Recorded. ${p.went_negative ? "Stock is now negative — check your counts." : ""}` : "Transfer recorded"),
    silentError: true,
  });
  const storageOpts = (storage.data ?? []).filter((s) => !s.archived).map((s) => ({ value: String(s.id), label: s.name }));
  const field = (f: string) => (error?.field === f ? error.message : undefined);
  return (
    <Modal opened={!!kind && !!m} onClose={onClose} title={kind ? titles[kind] : ""} size="lg">
      {m && kind && (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            rec.mutate({ movement: m }, { onSuccess: onClose, onError: (er) => setError(isAppError(er) ? er : { kind: "other", message: String(er), field: null }) });
          }}
        >
          <Stack>
            <Text size="sm" c="dimmed">
              {help[kind]}
            </Text>
            <Text size="sm">
              {product.name}: {num(product.on_hand, 3)} {unitShort(product.stock_unit)} recorded on hand, worth {money(product.value)}.
            </Text>
            <SimpleGrid cols={2}>
              <DecimalInput
                label={kind === "adjustment" ? "Counted quantity" : "Quantity"}
                value={m.qty}
                onChange={(qty) => setM({ ...m, qty })}
                error={field("qty")}
                required
                data-autofocus
              />
              <Select
                label="Unit"
                data={unitOptions(units.data ?? [], product)}
                value={m.unit}
                allowDeselect={false}
                onChange={(v) => setM({ ...m, unit: v ?? m.unit })}
                error={field("unit")}
              />
              <Select
                label={kind === "transfer" ? "From" : "Storage spot"}
                placeholder="Unassigned"
                clearable
                data={storageOpts}
                value={m.storage_id != null ? String(m.storage_id) : null}
                onChange={(v) => setM({ ...m, storage_id: v ? Number(v) : null })}
              />
              {kind === "transfer" && (
                <Select
                  label="To"
                  placeholder="Unassigned"
                  clearable
                  data={storageOpts}
                  value={m.to_storage_id != null ? String(m.to_storage_id) : null}
                  onChange={(v) => setM({ ...m, to_storage_id: v ? Number(v) : null })}
                  error={field("to_storage_id")}
                />
              )}
              {kind === "adjustment" && !product.avg_cost && (
                <DecimalInput
                  label={`Cost per ${unitShort(m.unit)}`}
                  description="Needed because this product has no purchase cost yet"
                  unit="$"
                  value={m.unit_cost ?? ""}
                  onChange={(v) => setM({ ...m, unit_cost: v || null })}
                  error={field("unit_cost")}
                />
              )}
              <TextInput type="date" label="Date" value={m.occurred_on} onChange={(e) => setM({ ...m, occurred_on: e.currentTarget.value })} error={field("occurred_on")} />
            </SimpleGrid>
            <Textarea label="Note" value={m.note} onChange={(e) => setM({ ...m, note: e.currentTarget.value })} />
            {error && !error.field && <Alert color="red">{error.message}</Alert>}
            <Group justify="flex-end">
              <Button variant="default" onClick={onClose}>
                Cancel
              </Button>
              <Button type="submit" loading={rec.isPending}>
                {titles[kind]}
              </Button>
            </Group>
          </Stack>
        </form>
      )}
    </Modal>
  );
}
