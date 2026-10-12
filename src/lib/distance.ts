// Distances are stored and computed in kilometres everywhere; the chosen unit only affects what
// is shown and typed, so switching units never moves a search radius.
import { useCmd } from "../api/queries";
import type { DistanceUnit, Prefs } from "../api/types";

/** Exact by definition (international mile). */
export const KM_PER_MI = 1.609344;

export const toDisplay = (km: number, unit: DistanceUnit) => (unit === "mi" ? km / KM_PER_MI : km);
export const fromDisplay = (value: number, unit: DistanceUnit) => (unit === "mi" ? value * KM_PER_MI : value);

export function fmtDistance(km: number | null | undefined, unit: DistanceUnit, dp = 1): string {
  return km == null ? "—" : `${toDisplay(km, unit).toFixed(dp)} ${unit}`;
}

export const usePrefs = () => useCmd<Prefs>("prefs_get");

export function useDistanceUnit(): DistanceUnit {
  return usePrefs().data?.distance_unit ?? "mi";
}
