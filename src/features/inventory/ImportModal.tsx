import { Alert, Badge, Button, Group, Modal, ScrollArea, Select, SimpleGrid, Stack, Table, Text, TextInput } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconFileSpreadsheet } from "@tabler/icons-react";
import { useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { call, errorMessage } from "../../api/ipc";
import type { CsvTable, ImportPreview, ProductMapping } from "../../api/types";
import { num } from "../../lib/format";
import { useToday } from "../../lib/dates";

const FIELDS: { key: keyof ProductMapping; label: string; required?: boolean; guess: RegExp }[] = [
  { key: "name", label: "Product name", required: true, guess: /^(product|item)?\s*name$|^product$|^item$/i },
  { key: "stock_unit", label: "Stock unit (g, ml, fl oz, piece…)", required: true, guess: /unit|uom/i },
  { key: "brand", label: "Brand", guess: /brand|manufacturer/i },
  { key: "category", label: "Category (professional, consumable, retail)", guess: /^category|^type$/i },
  { key: "subcategory", label: "Subcategory", guess: /sub.?category/i },
  { key: "sku", label: "SKU", guess: /sku|item\s*(no|number|#)|part/i },
  { key: "barcode", label: "Barcode", guess: /barcode|upc|ean|gtin/i },
  { key: "supplier", label: "Supplier", guess: /supplier|vendor|distributor/i },
  { key: "density_g_per_ml", label: "Density (g per mL)", guess: /density/i },
  { key: "reorder_point", label: "Reorder point", guess: /reorder|min/i },
  { key: "retail_price", label: "Retail price", guess: /retail|price/i },
  { key: "opening_qty", label: "Opening quantity (in stock unit)", guess: /qty|quantity|on.?hand|stock$/i },
  { key: "opening_unit_cost", label: "Opening cost per stock unit", guess: /cost/i },
];

function guessMapping(headers: string[]): ProductMapping {
  const used = new Set<number>();
  const m = Object.fromEntries(FIELDS.map((f) => [f.key, null])) as unknown as ProductMapping;
  for (const f of FIELDS) {
    const i = headers.findIndex((h, idx) => !used.has(idx) && f.guess.test(h.trim()));
    if (i >= 0) {
      (m as unknown as Record<string, number | null>)[f.key] = i;
      used.add(i);
    }
  }
  return m;
}

export function ImportModal({ opened, onClose }: { opened: boolean; onClose: () => void }) {
  const qc = useQueryClient();
  const today = useToday();
  const [path, setPath] = useState<string | null>(null);
  const [table, setTable] = useState<CsvTable | null>(null);
  const [mapping, setMapping] = useState<ProductMapping | null>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [date, setDate] = useState(today);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function reset() {
    setPath(null);
    setTable(null);
    setMapping(null);
    setPreview(null);
    setError(null);
  }

  async function choose() {
    const p = await open({ title: "Choose a CSV file", filters: [{ name: "CSV", extensions: ["csv", "txt"] }] });
    if (typeof p !== "string") return;
    setBusy(true);
    setError(null);
    try {
      const t = await call<CsvTable>("csv_read", { path: p });
      setPath(p);
      setTable(t);
      setMapping(guessMapping(t.headers));
      setPreview(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function runPreview() {
    setBusy(true);
    setError(null);
    try {
      setPreview(await call<ImportPreview>("products_import_preview", { path, mapping }));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function commit() {
    setBusy(true);
    try {
      const n = await call<number>("products_import_commit", { path, mapping, openingDate: date });
      notifications.show({ message: `Imported ${n} products`, color: "teal" });
      qc.invalidateQueries({ queryKey: ["products_list"] });
      qc.invalidateQueries({ queryKey: ["suppliers_list"] });
      reset();
      onClose();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const headerOptions = (table?.headers ?? []).map((h, i) => ({ value: String(i), label: h || `Column ${i + 1}` }));

  return (
    <Modal opened={opened} onClose={() => (reset(), onClose())} title="Import products from CSV" size="xl">
      <Stack>
        {!table && (
          <>
            <Text size="sm" c="dimmed">
              Use a spreadsheet saved as CSV with one product per row and a header row. You'll match its columns to fields and see a preview before anything is
              saved. Rows that match an existing SKU, barcode, or name and brand are skipped.
            </Text>
            <Group>
              <Button leftSection={<IconFileSpreadsheet size={16} />} onClick={choose} loading={busy}>
                Choose CSV file…
              </Button>
            </Group>
          </>
        )}
        {error && <Alert color="red">{error}</Alert>}
        {table && mapping && !preview && (
          <>
            <Text size="sm">
              {table.rows.length} rows found. Match each field to a column; leave optional fields unmatched if your file doesn't have them.
            </Text>
            <SimpleGrid cols={2}>
              {FIELDS.map((f) => (
                <Select
                  key={f.key}
                  label={f.label}
                  required={f.required}
                  clearable={!f.required}
                  placeholder="Not in this file"
                  data={headerOptions}
                  value={mapping[f.key] != null ? String(mapping[f.key]) : null}
                  onChange={(v) => setMapping({ ...mapping, [f.key]: v == null ? null : Number(v) })}
                />
              ))}
            </SimpleGrid>
            <Group justify="space-between">
              <Button variant="subtle" onClick={reset}>
                Choose another file
              </Button>
              <Button onClick={runPreview} loading={busy} disabled={mapping.name == null || mapping.stock_unit == null}>
                Preview import
              </Button>
            </Group>
          </>
        )}
        {preview && (
          <>
            <Group gap="xs">
              <Badge color="teal">{preview.new_count} new</Badge>
              <Badge color="gray">{preview.duplicate_count} duplicates skipped</Badge>
              <Badge color="red">{preview.error_count} with errors</Badge>
            </Group>
            <ScrollArea h={320}>
              <Table striped fz="sm">
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>Line</Table.Th>
                    <Table.Th>Product</Table.Th>
                    <Table.Th>Unit</Table.Th>
                    <Table.Th>Opening stock</Table.Th>
                    <Table.Th>Result</Table.Th>
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {preview.rows.map((r) => (
                    <Table.Tr key={r.line}>
                      <Table.Td>{r.line}</Table.Td>
                      <Table.Td>{[r.brand, r.name].filter(Boolean).join(" ")}</Table.Td>
                      <Table.Td>{r.stock_unit}</Table.Td>
                      <Table.Td>{r.opening_qty ? `${num(r.opening_qty)} at $${r.opening_unit_cost ?? "?"}` : "—"}</Table.Td>
                      <Table.Td>
                        <Text size="xs" c={r.status === "error" ? "red" : r.status === "duplicate" ? "dimmed" : "teal"}>
                          {r.status === "new" ? "Will be added" : r.message}
                        </Text>
                      </Table.Td>
                    </Table.Tr>
                  ))}
                </Table.Tbody>
              </Table>
            </ScrollArea>
            {preview.rows.some((r) => r.opening_qty) && (
              <TextInput
                type="date"
                label="Date for opening stock"
                description="Opening stock is recorded as stock you already had, not as a purchase."
                value={date}
                onChange={(e) => setDate(e.currentTarget.value)}
                w={220}
              />
            )}
            <Group justify="space-between">
              <Button variant="subtle" onClick={() => setPreview(null)}>
                Back to column matching
              </Button>
              <Button onClick={commit} loading={busy} disabled={preview.new_count === 0}>
                Import {preview.new_count} products
              </Button>
            </Group>
          </>
        )}
      </Stack>
    </Modal>
  );
}
