import { useCmd } from "../api/queries";
import type { Business } from "../api/types";

/** YYYY-MM-DD for `d` in the given IANA time zone. */
export function ymd(d: Date, timeZone?: string): string {
  return new Intl.DateTimeFormat("en-CA", { timeZone, year: "numeric", month: "2-digit", day: "2-digit" }).format(d);
}

export function addDays(ymdStr: string, days: number): string {
  const [y, m, d] = ymdStr.split("-").map(Number);
  const dt = new Date(Date.UTC(y, m - 1, d + days));
  return dt.toISOString().slice(0, 10);
}

/** Today's date in the business's time zone. */
export function useToday(): string {
  const biz = useCmd<Business>("business_get");
  return ymd(new Date(), biz.data?.timezone);
}
