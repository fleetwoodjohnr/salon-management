import type { SiteStatus, SourceType } from "../../api/types";

/** What happened when a competitor's website was checked, in plain words. */
export const siteStatus: Record<SiteStatus, { label: string; color: string; help: string }> = {
  prices_found: { label: "Prices found", color: "teal", help: "Advertised prices were read from the business's own website." },
  no_prices: { label: "No prices online", color: "gray", help: "The site was read but lists no prices as text (they may be in pictures, a booking widget, or not published)." },
  not_html: { label: "PDF or image menu", color: "yellow", help: "Prices seem to be in a PDF or picture, which isn't read automatically. Open it and record prices yourself." },
  robots_disallowed: { label: "Site asks not to be read", color: "orange", help: "The site's robots.txt asks automated visitors not to read these pages, so they weren't." },
  terms_restricted: { label: "Platform terms", color: "orange", help: "A social network, booking platform or directory whose terms don't allow automated reading. Open it and record prices yourself." },
  blocked: { label: "Blocked", color: "red", help: "The site shows an anti-bot check or denies automated access. It wasn't bypassed." },
  rate_limited: { label: "Busy", color: "red", help: "The site asked for fewer requests (HTTP 429), so it was left alone. Try again later." },
  unreachable: { label: "Couldn't reach", color: "red", help: "The site didn't answer, timed out or doesn't exist." },
  http_error: { label: "Site error", color: "red", help: "The site answered with an error page." },
};

export const sourceType: Record<SourceType, { label: string; color: string }> = {
  web_extracted: { label: "Advertised online", color: "teal" },
  user_observed: { label: "Recorded by you", color: "blue" },
  csv_import: { label: "Imported", color: "grape" },
};

/** Service categories (same keys as research::catalog::CATEGORIES in the core). */
export const categories: { value: string; label: string }[] = [
  { value: "cut_style", label: "Haircuts and styling" },
  { value: "color", label: "Color and highlights" },
  { value: "specialty_color", label: "Balayage and specialty color" },
  { value: "treatment", label: "Hair treatments" },
  { value: "extensions", label: "Extensions" },
  { value: "barber", label: "Barbering" },
  { value: "nails", label: "Nails" },
  { value: "skin", label: "Facials and skin care" },
  { value: "brows_lashes", label: "Brows and lashes" },
  { value: "waxing", label: "Waxing" },
  { value: "makeup", label: "Makeup" },
  { value: "massage", label: "Massage" },
  { value: "other", label: "Other" },
];
export const categoryLabel = (k: string) => categories.find((c) => c.value === k)?.label ?? "Other";

/** Kinds of business the map search looks for (prefs::BUSINESS_TYPES in the core). */
export const businessTypes: { value: string; label: string }[] = [
  { value: "hair", label: "Hair salons" },
  { value: "barber", label: "Barbers" },
  { value: "nails", label: "Nail salons" },
  { value: "beauty", label: "Beauty and skin" },
  { value: "massage", label: "Massage" },
];
