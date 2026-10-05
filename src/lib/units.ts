import type { CustomUnit, Dimension, UnitDef } from "../api/types";

export const SHORT: Record<string, string> = {
  g: "g",
  kg: "kg",
  oz_wt: "oz (weight)",
  lb: "lb",
  ml: "mL",
  l: "L",
  fl_oz: "fl oz",
  gal: "gal",
  piece: "pcs",
};

export function unitShort(code: string): string {
  return SHORT[code] ?? code;
}

/** Units a product can be measured in: its dimension, the other of mass/volume if it has a density,
 *  and its custom units. The backend re-validates every conversion. */
export function unitOptions(units: UnitDef[], p: { dimension: Dimension; density_g_per_ml: string | null; custom_units: CustomUnit[] }) {
  const ok = (d: Dimension) => d === p.dimension || (p.density_g_per_ml != null && d !== "count" && p.dimension !== "count");
  return [
    ...units.filter((u) => ok(u.dimension)).map((u) => ({ value: u.code, label: unitShort(u.code) })),
    ...p.custom_units.map((c) => ({ value: c.name, label: `${c.name} (${c.qty} ${unitShort(c.unit)})` })),
  ];
}

export const dimensionLabel: Record<Dimension, string> = { mass: "weight", volume: "volume", count: "count" };
