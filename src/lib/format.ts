// Display formatting for decimal strings returned by the Rust core.
// Money is never computed here; these helpers only round for display, using the same rule as the
// backend (half away from zero) and string arithmetic so binary floats never creep in.

const DEC_RE = /^-?\d+(\.\d+)?$/;

export function isDecimal(s: string): boolean {
  return DEC_RE.test(s.trim());
}

/** Round a decimal string to `dp` places, half away from zero. */
export function roundDec(s: string, dp: number): string {
  const t = s.trim();
  if (!isDecimal(t)) return t;
  const neg = t.startsWith("-");
  const [intPart, frac = ""] = (neg ? t.slice(1) : t).split(".");
  if (frac.length <= dp) {
    const out = dp > 0 ? `${intPart}.${frac.padEnd(dp, "0")}` : intPart;
    return neg && /[1-9]/.test(out) ? `-${out}` : out;
  }
  const keep = intPart + frac.slice(0, dp);
  const roundUp = frac.charCodeAt(dp) - 48 >= 5;
  let digits = keep.split("").map(Number);
  if (roundUp) {
    let i = digits.length - 1;
    while (i >= 0) {
      if (digits[i] === 9) {
        digits[i] = 0;
        i--;
      } else {
        digits[i]++;
        break;
      }
    }
    if (i < 0) digits = [1, ...digits];
  }
  const str = digits.join("");
  const ip = str.slice(0, str.length - dp) || "0";
  const fp = str.slice(str.length - dp);
  const out = dp > 0 ? `${ip.replace(/^0+(?=\d)/, "")}.${fp}` : ip.replace(/^0+(?=\d)/, "");
  return neg && /[1-9]/.test(out) ? `-${out}` : out;
}

function group(intPart: string): string {
  return intPart.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

/** "$1,234.50" — `dp` defaults to cents; pass 4 for unit costs. */
export function money(s: string | null | undefined, dp = 2): string {
  if (s == null || s === "") return "—";
  if (!isDecimal(s)) return s;
  const r = roundDec(s, dp);
  const neg = r.startsWith("-");
  const [ip, fp] = (neg ? r.slice(1) : r).split(".");
  return `${neg ? "−" : ""}$${group(ip)}${fp ? `.${fp}` : ""}`;
}

/** Unit cost with enough precision to be meaningful (4 dp below $1, else 2–4). */
export function unitMoney(s: string | null | undefined): string {
  if (s == null || !isDecimal(s)) return money(s);
  const abs = s.replace("-", "");
  return money(s, Number(abs) < 1 ? 4 : 2).replace(/(\.\d{2}\d*?)0+$/, "$1");
}

/** Plain number with up to `maxDp` decimals, trailing zeros trimmed: "12.5". */
export function num(s: string | null | undefined, maxDp = 2): string {
  if (s == null || s === "") return "—";
  if (!isDecimal(s)) return s;
  const r = roundDec(s, maxDp);
  const neg = r.startsWith("-");
  const [ip, fp = ""] = (neg ? r.slice(1) : r).split(".");
  const f = fp.replace(/0+$/, "");
  return `${neg ? "−" : ""}${group(ip)}${f ? `.${f}` : ""}`;
}

export function pct(s: string | null | undefined, maxDp = 1): string {
  return s == null || s === "" ? "—" : `${num(s, maxDp)}%`;
}

export function hours(s: string | null | undefined): string {
  return s == null ? "—" : `${num(s, 1)} h`;
}

export function minutesLabel(min: number): string {
  if (min < 60) return `${min} min`;
  const h = Math.floor(min / 60);
  const m = min % 60;
  return m ? `${h} h ${m} min` : `${h} h`;
}

export function isoToLocal(iso: string | null | undefined): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return isNaN(d.getTime()) ? iso : d.toLocaleString();
}

export function dateLabel(ymd: string | null | undefined): string {
  if (!ymd) return "—";
  const [y, m, d] = ymd.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

export function isZeroDec(s: string | null | undefined): boolean {
  return s == null || /^-?0*(\.0*)?$/.test(s.trim());
}

export function isNegDec(s: string | null | undefined): boolean {
  return s != null && s.trim().startsWith("-") && !isZeroDec(s);
}

/** A deduction for display: "−$12.50", or "$0.00" when there's nothing to deduct. */
export function minus(s: string | null | undefined): string {
  return isZeroDec(s) ? money("0") : `−${money(s)}`;
}
