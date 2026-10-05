// Local business date-times are strings "YYYY-MM-DDTHH:MM" (no time zone): the calendar treats them
// as this computer's local time, which is assumed to match the business time zone.

const pad = (n: number) => String(n).padStart(2, "0");

export function toLocalStr(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function fromLocalStr(s: string): Date {
  const [date, time = "00:00"] = s.split("T");
  const [y, m, d] = date.split("-").map(Number);
  const [h, mi] = time.split(":").map(Number);
  return new Date(y, m - 1, d, h, mi);
}

export function timeLabel(s: string): string {
  return fromLocalStr(s).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
}

export function dateTimeLabel(s: string): string {
  return fromLocalStr(s).toLocaleString(undefined, { weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
}
