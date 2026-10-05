// TypeScript mirrors of the Rust command payloads. Decimals are strings (exact).
// Keep in sync with src-tauri/src (serde structs); the backend rejects unknown fields.

export type Dec = string;

// ---------- app / workspaces ----------
export type WorkspaceKind = "business" | "demo";
export interface WorkspaceInfo {
  id: string;
  name: string;
  kind: WorkspaceKind;
  created_at: string;
}
export interface AppStatus {
  app_version: string;
  schema_version: number;
  data_dir: string;
  workspaces: WorkspaceInfo[];
  current: WorkspaceInfo | null;
  open_error: string | null;
}
export interface ManifestFile {
  path: string;
  sha256: string;
  size: number;
}
export interface BackupManifest {
  format: string;
  format_version: number;
  app_version: string;
  schema_version: number;
  created_at: string;
  reason: string;
  workspace: WorkspaceInfo;
  files: ManifestFile[];
}
export interface BackupInfo {
  path: string;
  file_name: string;
  size: number;
  manifest: BackupManifest | null;
  error: string | null;
}

// ---------- business / locations / staff ----------
export interface DaySchedule {
  day: number;
  open: boolean;
  start: string;
  end: string;
}
export interface Business {
  name: string;
  timezone: string;
  units: "us" | "metric";
  currency: "USD";
  schedule: DaySchedule[];
  primary_location_id: number | null;
  prices_include_tax: boolean;
  onboarding_completed: boolean;
  onboarding_skipped: string[];
}
export interface Location {
  id: number | null;
  name: string;
  address_line: string;
  city: string;
  state: string;
  postal_code: string;
  latitude: string | null;
  longitude: string | null;
  geo_precision: "address" | "zcta" | "place" | "manual" | null;
  geo_source: string | null;
  state_fips: string | null;
  county_fips: string | null;
  place_fips: string | null;
  archived: boolean;
}
export interface Staff {
  id: number | null;
  name: string;
  color: string;
  default_profile_id: number | null;
  location_id: number | null;
  archived: boolean;
}
export interface AuditEntry {
  id: number;
  at: string;
  entity: string;
  entity_id: number | null;
  action: string;
  summary: string;
  detail: string | null;
}

// ---------- work profiles ----------
export type ProfileKind = "individual" | "chair_renter" | "independent" | "employee";
export type CompModel = "owner_target_hourly" | "hourly_wage" | "commission" | "hourly_plus_commission";
export type TargetKind = "margin" | "markup";
export type Position = "budget" | "standard" | "premium" | "luxury";
export type OverheadBasis = "occupied" | "hands_on";
export type RoundMode = "up" | "nearest" | "down";

export interface Overhead {
  rent: Dec;
  utilities: Dec;
  insurance: Dec;
  software: Dec;
  other: Dec;
  other_label: string;
}
export interface ProfileData {
  kind: ProfileKind;
  specialty: string;
  experience_level: string;
  location_id: number | null;
  comp_model: CompModel;
  hourly_rate: Dec;
  employer_burden_pct: Dec;
  commission_pct: Dec;
  retail_commission_pct: Dec;
  weekly_hours: Dec;
  weeks_per_year: Dec;
  utilization_pct: Dec;
  overhead: Overhead;
  processing_pct: Dec;
  processing_fixed: Dec;
  card_share_pct: Dec;
  target_kind: TargetKind;
  target_pct: Dec;
  position: Position;
  overhead_basis: OverheadBasis;
  rounding_increment: Dec;
  rounding_mode: RoundMode;
  notes: string;
}
export interface ProfileRates {
  monthly_hours: Dec;
  monthly_billable_hours: Dec;
  monthly_overhead: Dec;
  overhead_per_billable_hour: Dec;
  pay_per_hour_worked: Dec;
  labor_cost_per_billable_hour: Dec;
  commission_pct: Dec;
  retail_commission_pct: Dec;
  notes: string[];
}
export interface ProfileView {
  id: number;
  name: string;
  archived: boolean;
  version: number;
  version_id: number;
  version_created_at: string;
  data: ProfileData;
  rates: ProfileRates | null;
  rates_error: string | null;
}
export interface ProfileVersionInfo {
  version: number;
  version_id: number;
  created_at: string;
}

// ---------- units / inventory ----------
export type Dimension = "mass" | "volume" | "count";
export interface UnitDef {
  code: string;
  label: string;
  dimension: Dimension;
  to_base: Dec;
}
export interface CustomUnit {
  name: string;
  qty: Dec;
  unit: string;
}
export type ProductCategory = "professional" | "consumable" | "retail";
export interface Supplier {
  id: number | null;
  name: string;
  contact: string;
  phone: string;
  email: string;
  website: string;
  notes: string;
  archived: boolean;
}
export interface StorageLocation {
  id: number | null;
  name: string;
  location_id: number | null;
  archived: boolean;
}
export interface ProductInput {
  id: number | null;
  name: string;
  brand: string;
  category: ProductCategory;
  subcategory: string;
  sku: string;
  barcode: string;
  supplier_id: number | null;
  stock_unit: string;
  density_g_per_ml: Dec | null;
  default_storage_id: number | null;
  reorder_point: Dec | null;
  reorder_qty: Dec | null;
  retail_price: Dec | null;
  notes: string;
  custom_units: CustomUnit[];
}
export interface ProductRow {
  id: number;
  name: string;
  brand: string;
  category: ProductCategory;
  subcategory: string;
  sku: string;
  barcode: string;
  supplier_id: number | null;
  supplier_name: string | null;
  dimension: Dimension;
  stock_unit: string;
  base_unit: string;
  density_g_per_ml: Dec | null;
  default_storage_id: number | null;
  storage_name: string | null;
  reorder_point: Dec | null;
  reorder_qty: Dec | null;
  retail_price: Dec | null;
  on_hand: Dec;
  on_hand_base: Dec;
  value: Dec;
  avg_cost: Dec | null;
  low_stock: boolean;
  negative: boolean;
  custom_units: CustomUnit[];
  notes: string;
  archived: boolean;
}
export interface StorageQty {
  storage_id: number | null;
  storage_name: string | null;
  qty: Dec;
}
export interface PurchaseLineRow {
  id: number;
  product_id: number;
  product_name: string;
  package_count: Dec;
  contents_per_package: Dec;
  unit: string;
  qty_stock: Dec;
  stock_unit: string;
  line_price: Dec;
  landed_cost: Dec;
  unit_cost: Dec;
  storage_name: string | null;
  lot_code: string;
  expires_on: string | null;
}
export interface PurchaseRow {
  id: number;
  kind: "purchase" | "opening_balance";
  supplier_id: number | null;
  supplier_name: string | null;
  purchase_date: string;
  invoice_ref: string;
  subtotal: Dec;
  discount: Dec;
  shipping: Dec;
  nonrecoverable_tax: Dec;
  total: Dec;
  attachment_id: number | null;
  attachment_name: string | null;
  notes: string;
  reversed_at: string | null;
  reversal_note: string | null;
  lines: PurchaseLineRow[];
}
export interface LedgerRow {
  id: number;
  product_id: number;
  product_name: string;
  storage_id: number | null;
  storage_name: string | null;
  occurred_on: string;
  recorded_at: string;
  kind: string;
  qty_base: Dec;
  qty: Dec;
  stock_unit: string;
  value: Dec;
  qty_after: Dec;
  value_after: Dec;
  source: string | null;
  source_id: number | null;
  reversal_of: number | null;
  reversed_by: number | null;
  note: string;
}
export interface ProductDetail {
  product: ProductRow;
  by_storage: StorageQty[];
  purchases: PurchaseRow[];
  ledger: LedgerRow[];
}
export interface PurchaseLineInput {
  product_id: number;
  package_count: Dec;
  contents_per_package: Dec;
  unit: string;
  line_price: Dec;
  storage_id: number | null;
  lot_code: string;
  expires_on: string | null;
}
export interface PurchaseInput {
  kind: "purchase" | "opening_balance";
  supplier_id: number | null;
  purchase_date: string;
  invoice_ref: string;
  discount: Dec;
  shipping: Dec;
  nonrecoverable_tax: Dec;
  attachment_id: number | null;
  notes: string;
  lines: PurchaseLineInput[];
}
export interface LinePreview {
  qty_base: Dec;
  qty_stock: Dec;
  stock_unit: string;
  landed_cost: Dec;
  unit_cost: Dec;
}
export interface PurchasePreview {
  subtotal: Dec;
  total: Dec;
  lines: LinePreview[];
}
export interface MovementInput {
  product_id: number;
  kind: "waste" | "adjustment" | "transfer" | "supplier_return";
  qty: Dec;
  unit: string;
  storage_id: number | null;
  to_storage_id: number | null;
  unit_cost: Dec | null;
  occurred_on: string;
  note: string;
}
export interface Posted {
  ledger_id: number;
  qty: Dec;
  value: Dec;
  went_negative: boolean;
}
export interface LedgerFilter {
  product_id: number | null;
  kind: string | null;
  from: string | null;
  to: string | null;
  limit: number | null;
}
export interface ReorderItem {
  product_id: number;
  name: string;
  brand: string;
  supplier_name: string | null;
  on_hand: Dec;
  reorder_point: Dec;
  suggested_qty: Dec;
  stock_unit: string;
  last_unit_cost: Dec | null;
}
export interface ExpiringLot {
  purchase_line_id: number;
  product_id: number;
  product_name: string;
  lot_code: string;
  expires_on: string;
  received_on: string;
  qty_received: Dec;
  stock_unit: string;
  on_hand: Dec;
}
export interface Attachment {
  id: number;
  file_name: string;
  size: number;
  created_at: string;
}
export interface CsvTable {
  headers: string[];
  rows: string[][];
}
export interface ProductMapping {
  name: number | null;
  brand: number | null;
  category: number | null;
  subcategory: number | null;
  sku: number | null;
  barcode: number | null;
  supplier: number | null;
  stock_unit: number | null;
  density_g_per_ml: number | null;
  reorder_point: number | null;
  retail_price: number | null;
  opening_qty: number | null;
  opening_unit_cost: number | null;
}
export interface ImportRow {
  line: number;
  status: "new" | "duplicate" | "error";
  message: string | null;
  name: string;
  brand: string;
  category: string;
  sku: string;
  stock_unit: string;
  opening_qty: Dec | null;
  opening_unit_cost: Dec | null;
}
export interface ImportPreview {
  rows: ImportRow[];
  new_count: number;
  duplicate_count: number;
  error_count: number;
}

// ---------- services / estimates ----------
export interface TimeSpec {
  hands_on_min: number;
  processing_min: number;
  setup_min: number;
  cleanup_min: number;
}
export interface PricingOverrides {
  target_kind: TargetKind | null;
  target_pct: Dec | null;
  position: Position | null;
  rounding_increment: Dec | null;
  rounding_mode: RoundMode | null;
}
export interface VariantInput {
  id: number | null;
  group_name: string;
  name: string;
  hands_on_delta: number;
  processing_delta: number;
  material_factor: Dec;
  price_delta: Dec;
}
export interface RecipeLineInput {
  product_id: number;
  qty: Dec;
  unit: string;
  note: string;
}
export interface ServiceInput {
  id: number | null;
  name: string;
  category: string;
  description: string;
  profile_id: number | null;
  time: TimeSpec;
  is_addon: boolean;
  waste_pct: Dec;
  other_direct_cost: Dec;
  other_direct_note: string;
  price: Dec | null;
  price_note: string;
  overrides: PricingOverrides;
  tax_category: string;
  variants: VariantInput[];
  recipe: RecipeLineInput[];
  recipe_note: string;
}
export interface RecipeLineView {
  product_id: number;
  product_name: string;
  qty: Dec;
  unit: string;
  note: string;
}
export interface RecipeVersionInfo {
  id: number;
  version: number;
  note: string;
  created_at: string;
  lines: RecipeLineView[];
}
export interface ServiceView {
  input: ServiceInput;
  archived: boolean;
  price_set_at: string | null;
  recipe_version_id: number | null;
  recipe_version: number | null;
  recipe_versions: RecipeVersionInfo[];
}
export type ServiceStatus = "ok" | "below_target" | "below_cost" | "no_price" | "error";
export interface ServiceSummary {
  id: number;
  name: string;
  category: string;
  is_addon: boolean;
  profile_name: string | null;
  duration_min: number;
  price: Dec | null;
  estimated_cost: Dec | null;
  margin_pct: Dec | null;
  target_price: Dec | null;
  status: ServiceStatus;
  status_detail: string | null;
  archived: boolean;
}
export interface MaterialCost {
  product_id: number;
  name: string;
  qty: Dec;
  unit: string;
  qty_base: Dec;
  base_unit: string;
  unit_cost: Dec | null;
  cost: Dec;
  warning: string | null;
}
export interface CostBreakdown {
  materials: Dec;
  waste: Dec;
  other_direct: Dec;
  labor: Dec;
  overhead: Dec;
  fixed_total: Dec;
  time: TimeSpec;
  working_hours: Dec;
  occupied_hours: Dec;
  overhead_hours: Dec;
  labor_rate: Dec;
  overhead_rate: Dec;
  lines: MaterialCost[];
  warnings: string[];
}
export interface PriceParams {
  fixed_cost: Dec;
  commission_pct: Dec;
  processing_pct: Dec;
  processing_fixed: Dec;
  card_share_pct: Dec;
  tax_rate_pct: Dec | null;
  prices_include_tax: boolean;
  target_kind: TargetKind;
  target_pct: Dec;
  rounding_increment: Dec;
  rounding_mode: RoundMode;
}
export interface PriceAnalysis {
  list_price: Dec;
  revenue: Dec;
  tax: Dec;
  customer_total: Dec;
  commission: Dec;
  processing: Dec;
  fixed_cost: Dec;
  total_cost: Dec;
  profit: Dec;
  margin_pct: Dec | null;
  markup_pct: Dec | null;
}
export interface Targets {
  break_even: Dec;
  target_price: Dec;
  target_rounded: Dec;
  target_rounded_misses: boolean;
}
export interface TaxContext {
  taxable: boolean | null;
  rate_pct: Dec | null;
  prices_include_tax: boolean;
  label: string;
  resolved: boolean;
}
export interface PricingView {
  params: PriceParams;
  targets: Targets | null;
  targets_error: string | null;
  at_price: PriceAnalysis | null;
  chosen_price: Dec | null;
  earnings_per_working_hour: Dec | null;
  earnings_per_occupied_hour: Dec | null;
  tax: TaxContext;
  position: Position;
  warnings: string[];
  status: ServiceStatus;
}
export interface Estimate {
  profile_id: number | null;
  profile_name: string | null;
  profile_version_id: number | null;
  cost: CostBreakdown | null;
  cost_error: string | null;
  pricing: PricingView | null;
}

// ---------- sales tax ----------
export interface TaxCategory {
  code: string;
  label: string;
  applies_to: "service" | "retail" | "tips" | "other";
  builtin: boolean;
}
export interface RateSetInput {
  location_id: number;
  state_rate: Dec | null;
  county_rate: Dec | null;
  city_rate: Dec | null;
  district_rate: Dec | null;
  total_rate: Dec | null;
  jurisdiction_label: string;
  jurisdiction_code: string | null;
  source: "manual" | "wa_dor" | "ca_cdtfa";
  source_url: string | null;
  precision: "address" | "zip9" | "zip5" | "city" | "county" | "state" | "unknown";
  status: "verified" | "estimate" | "manual";
  effective_date: string | null;
  dataset_period: string | null;
  retrieved_at: string | null;
  note: string;
}
export interface RateSet extends Omit<RateSetInput, "total_rate"> {
  id: number;
  version: number;
  total_rate: Dec;
  created_at: string;
  superseded_at: string | null;
  freshness: "current" | "stale" | "manual";
  freshness_note: string;
}
export interface TaxabilityRule {
  id: number | null;
  category_code: string;
  category_label: string;
  applies_to: string;
  status: "taxable" | "exempt" | "unknown";
  basis: string;
  decided_at: string | null;
}
export interface TaxOverview {
  location: Location | null;
  rate: RateSet | null;
  rules: TaxabilityRule[];
  prices_include_tax: boolean;
  unresolved: string[];
}
export interface BundleItem {
  service_id: number;
  qty: number;
}
export interface BundleInput {
  id: number | null;
  name: string;
  description: string;
  price: Dec | null;
  items: BundleItem[];
}
export interface BundleLine {
  service_id: number;
  name: string;
  qty: number;
  list_price: Dec | null;
  allocated_price: Dec | null;
  total_cost: Dec | null;
}
export interface BundleView {
  input: BundleInput;
  archived: boolean;
  lines: BundleLine[];
  separate_total: Dec | null;
  total_cost: Dec | null;
  profit: Dec | null;
  margin_pct: Dec | null;
  warnings: string[];
}

// ---------- providers / lookups ----------
export interface KeySpec {
  required: boolean;
  signup_url: string;
  note: string;
}
export interface ProviderInfo {
  id: string;
  name: string;
  purpose: string;
  gives: string;
  coverage: string;
  license: string;
  limits: string;
  docs_url: string;
  daily_cap: number;
  key: KeySpec | null;
  attribution: string | null;
}
export interface ProviderStatus {
  info: ProviderInfo;
  key_configured: boolean | null;
  key_error: string | null;
  requests_today: number;
  errors_today: number;
  last_success: string | null;
  last_error: string | null;
  last_error_at: string | null;
  cached_responses: number;
  newest_cache: string | null;
}
export interface TaxLookup {
  provider: string;
  request_key: string;
  fetched_at: string;
  from_cache: boolean;
  rate: RateSetInput;
  matched_address: string;
  notes: string[];
}

// ---------- pricing what-if ----------
export interface WhatIf {
  service_id: number;
  variant_ids: number[];
  target_kind: TargetKind | null;
  target_pct: Dec | null;
  position: Position | null;
  monthly_overhead: Dec | null;
  utilization_pct: Dec | null;
  time: TimeSpec | null;
  material_factor: Dec | null;
  price: Dec | null;
}
export interface Change {
  item: string;
  before: Dec | null;
  after: Dec | null;
  reason: string;
}
export interface WhatIfResult {
  base: Estimate;
  scenario: Estimate;
  changes: Change[];
}

// ---------- clients ----------
export interface ClientInput {
  id: number | null;
  first_name: string;
  last_name: string;
  phone: string;
  email: string;
  sensitivities: string;
  notes: string;
}
export interface ClientRow {
  client: ClientInput;
  archived: boolean;
  visits: number;
  last_visit: string | null;
  next_appointment: string | null;
}
export interface FormulaLine {
  product_id: number | null;
  product_name: string;
  qty: Dec;
  unit: string;
}
export interface FormulaVersion {
  id: number;
  version: number;
  body: string;
  lines: FormulaLine[];
  note: string;
  sale_id: number | null;
  created_at: string;
}
export interface Formula {
  id: number;
  title: string;
  service_id: number | null;
  service_name: string | null;
  versions: FormulaVersion[];
}
export interface HistoryItem {
  sale_id: number;
  number: string | null;
  sale_date: string;
  status: string;
  description: string;
  staff_name: string | null;
  net: Dec | null;
}
export interface ClientDetail {
  row: ClientRow;
  history: HistoryItem[];
  formulas: Formula[];
}
export interface FormulaInput {
  formula_id: number | null;
  client_id: number;
  title: string;
  service_id: number | null;
  body: string;
  lines: FormulaLine[];
  note: string;
  sale_id: number | null;
}

// ---------- appointments / estimates ----------
export interface AppointmentLine {
  service_id: number;
  variant_ids: number[];
  qty: number;
}
export interface AppointmentInput {
  id: number | null;
  client_id: number | null;
  staff_id: number;
  location_id: number | null;
  starts_at: string;
  ends_at: string | null;
  notes: string;
  lines: AppointmentLine[];
  estimate_id: number | null;
  allow_overlap: boolean;
}
export type AppointmentStatus = "scheduled" | "checked_in" | "completed" | "cancelled" | "no_show";
export interface AppointmentView {
  id: number;
  client_id: number | null;
  client_name: string | null;
  staff_id: number;
  staff_name: string;
  staff_color: string;
  location_id: number | null;
  starts_at: string;
  ends_at: string;
  status: AppointmentStatus;
  notes: string;
  lines: AppointmentLine[];
  service_names: string[];
  sale_id: number | null;
  sale_status: string | null;
}
export interface AppointmentSaveResult {
  id: number | null;
  conflicts: string[];
  warnings: string[];
}
export interface EstimateLine {
  service_id: number;
  variant_ids: number[];
  qty: number;
  unit_price: Dec;
  description: string;
}
export interface EstimateInput {
  id: number | null;
  client_id: number | null;
  staff_id: number | null;
  location_id: number | null;
  issued_on: string;
  valid_until: string | null;
  notes: string;
  lines: EstimateLine[];
}
export interface EstimateView {
  input: EstimateInput;
  status: "open" | "accepted" | "declined" | "converted";
  client_name: string | null;
  subtotal: Dec;
}

// ---------- sales ----------
export type LineKind = "service" | "retail" | "tip";
export interface UsageInput {
  product_id: number;
  planned_qty: Dec;
  qty: Dec;
  unit: string;
}
export interface SaleLineInput {
  kind: LineKind;
  service_id: number | null;
  product_id: number | null;
  staff_id: number | null;
  description: string;
  variant_ids: number[];
  qty: Dec;
  unit_price: Dec;
  line_discount: Dec;
  planned_minutes: number | null;
  actual_minutes: number | null;
  usage: UsageInput[];
}
export interface SaleDraft {
  id: number | null;
  client_id: number | null;
  staff_id: number | null;
  location_id: number | null;
  appointment_id: number | null;
  sale_date: string;
  sale_discount: Dec;
  notes: string;
  lines: SaleLineInput[];
}
export interface LineOut {
  gross: Dec;
  sale_discount_share: Dec;
  net: Dec;
  tax: Dec;
}
export interface SaleTotals {
  lines: LineOut[];
  subtotal: Dec;
  discount_total: Dec;
  tax_total: Dec;
  tip_total: Dec;
  total: Dec;
  tax_resolved: boolean;
  unresolved: string[];
}
export interface LineSnapshot {
  id: number;
  gross: Dec | null;
  sale_discount_share: Dec | null;
  net: Dec | null;
  tax: Dec | null;
  taxability: string | null;
  taxability_basis: string | null;
  tax_category: string;
  materials_cost: Dec | null;
  planned_materials_cost: Dec | null;
  labor_cost: Dec | null;
  overhead_cost: Dec | null;
  other_direct_cost: Dec | null;
  commission: Dec | null;
  retail_cost: Dec | null;
  refunded_qty: Dec;
  refunded_net: Dec;
  refunded_tax: Dec;
  usage_costs: (Dec | null)[];
}
export interface Payment {
  id: number;
  refund_id: number | null;
  method: string;
  amount: Dec;
  fee: Dec;
  reference: string;
  paid_on: string;
  voided: boolean;
}
export interface RefundView {
  id: number;
  number: string;
  refund_date: string;
  reason: string;
  net_total: Dec;
  tax_total: Dec;
  tip_total: Dec;
  total: Dec;
}
export interface SaleView {
  draft: SaleDraft;
  number: string | null;
  status: "draft" | "finalized" | "voided";
  client_name: string | null;
  staff_name: string | null;
  location_name: string | null;
  created_at: string;
  finalized_at: string | null;
  voided_at: string | null;
  void_reason: string | null;
  totals: SaleTotals;
  tax_label: string | null;
  tax_rate: Dec | null;
  tax_rate_status: string | null;
  lines: LineSnapshot[];
  payments: Payment[];
  refunds: RefundView[];
  paid: Dec;
  refunded: Dec;
  balance: Dec;
  blockers: string[];
  warnings: string[];
}
export interface SaleSummary {
  id: number;
  number: string | null;
  status: string;
  sale_date: string;
  client_name: string | null;
  staff_name: string | null;
  total: Dec | null;
  lines: number;
  refunded: boolean;
}
export interface PaymentInput {
  method: string;
  amount: Dec;
  reference: string;
  paid_on: string;
}
export interface RefundLineInput {
  sale_line_id: number;
  qty: Dec;
  restock: boolean;
}
export interface RefundInput {
  refund_date: string;
  reason: string;
  method: string;
  lines: RefundLineInput[];
}

// ---------- expenses ----------
export interface ExpenseInput {
  id: number | null;
  expense_date: string;
  category: string;
  vendor: string;
  description: string;
  amount: Dec;
  payment_method: string;
  location_id: number | null;
  attachment_id: number | null;
}
export interface Expense {
  input: ExpenseInput;
  attachment_name: string | null;
  voided: boolean;
  void_reason: string | null;
}

// ---------- reports / dashboards ----------
export interface ReportFilter {
  from: string;
  to: string;
  location_id: number | null;
  profile_id: number | null;
  staff_id: number | null;
  service_id: number | null;
  category: string | null;
  product_id: number | null;
  status: "all" | "with_refunds" | "without_refunds" | null;
}
export interface Summary {
  service_sales: Dec;
  retail_sales: Dec;
  discounts: Dec;
  refunds: Dec;
  net_revenue: Dec;
  tips: Dec;
  tax_collected: Dec;
  materials_used: Dec;
  retail_cogs: Dec;
  commissions: Dec;
  processing_fees: Dec;
  direct_costs: Dec;
  gross_profit: Dec;
  gross_margin_pct: Dec | null;
  labor_allocated: Dec;
  overhead_allocated: Dec;
  other_direct: Dec;
  service_contribution: Dec;
  expenses: Dec;
  operating_result: Dec;
  inventory_purchases: Dec;
  stock_consumed: Dec;
  waste_and_adjustments: Dec;
  sales_count: number;
  services_count: Dec;
  average_ticket: Dec | null;
  planned_materials: Dec;
  planned_minutes: number;
  actual_minutes: number;
  lines_with_actual_time: number;
}
export interface GroupRow {
  key: string;
  label: string;
  qty: Dec;
  revenue: Dec;
  refunds: Dec;
  materials: Dec;
  retail_cogs: Dec;
  commission: Dec;
  fees: Dec;
  gross_profit: Dec;
  labor: Dec;
  overhead: Dec;
  other_direct: Dec;
  allocated: Dec;
  contribution: Dec;
  margin_pct: Dec | null;
  planned_materials: Dec;
}
export interface PeriodPoint {
  period: string;
  net_revenue: Dec;
  gross_profit: Dec;
  contribution: Dec;
  expenses: Dec;
  operating_result: Dec;
}
export interface Report {
  filter: ReportFilter;
  summary: Summary;
  prior: Summary | null;
  prior_from: string | null;
  prior_to: string | null;
  groups: Record<string, GroupRow[]>;
  trend: PeriodPoint[];
  notes: string[];
}
export interface LineFact {
  sale_id: number;
  number: string;
  sale_date: string;
  kind: LineKind;
  description: string;
  service_id: number | null;
  category: string;
  product_id: number | null;
  staff_id: number | null;
  staff_name: string | null;
  profile_id: number | null;
  profile_name: string | null;
  qty: Dec;
  net: Dec;
  discount: Dec;
  tax: Dec;
  materials: Dec;
  planned_materials: Dec;
  retail_cost: Dec;
  labor: Dec;
  overhead: Dec;
  other_direct: Dec;
  commission: Dec;
  fees: Dec;
  planned_minutes: number | null;
  actual_minutes: number | null;
}
export interface RefundFact {
  refund_id: number;
  number: string;
  sale_id: number;
  refund_date: string;
  kind: LineKind;
  description: string;
  net: Dec;
  tax: Dec;
  restock_cost: Dec;
}
export interface Drill {
  lines: LineFact[];
  refunds: RefundFact[];
}
export interface DashWidget {
  i: string;
  kind: WidgetKind;
  x: number;
  y: number;
  w: number;
  h: number;
  filters: Partial<ReportFilter>;
}
export type WidgetKind =
  | "kpi_revenue"
  | "kpi_profit"
  | "money_in"
  | "trend"
  | "by_service"
  | "by_profile"
  | "by_staff"
  | "costs"
  | "inventory"
  | "low_stock"
  | "estimate_vs_actual"
  | "upcoming"
  | "pricing_alerts"
  | "market"
  | "operating";
export interface Dashboard {
  id: number | null;
  name: string;
  layout: DashWidget[];
  filters: Partial<ReportFilter> & { preset?: string; compare?: boolean };
  is_default: boolean;
}
export interface MarketComparison {
  service_id: number;
  service_name: string;
  your_price: Dec | null;
  median: Dec | null;
  q1: Dec | null;
  q3: Dec | null;
  n: number;
  quality: string;
}
export interface Snapshot {
  inventory_value: Dec;
  products: number;
  low_stock: ReorderItem[];
  upcoming: AppointmentView[];
  pricing_alerts: ServiceSummary[];
  market: MarketComparison[];
}

// ---------- market ----------
export interface CompetitorInput {
  id: number | null;
  name: string;
  address: string;
  city: string;
  state: string;
  postal_code: string;
  latitude: string | null;
  longitude: string | null;
  website: string;
  phone: string;
  notes: string;
}
export interface Competitor {
  input: CompetitorInput;
  source: "manual" | "osm" | "csv";
  source_ref: string | null;
  attribution: string;
  archived: boolean;
  distance_km: number | null;
  observations: number;
}
export interface ObservationInput {
  id: number | null;
  competitor_id: number;
  service_id: number | null;
  service_label: string;
  price: Dec;
  price_type: "exact" | "starting_at" | "range";
  price_max: Dec | null;
  duration_min: number | null;
  hair_length: "" | "short" | "medium" | "long";
  stylist_level: string;
  inclusions: string;
  source_url: string;
  observed_on: string;
  notes: string;
}
export interface Observation {
  input: ObservationInput;
  source_type: "user_observed" | "csv_import";
  competitor_name: string;
  service_name: string | null;
  excluded: boolean;
  distance_km: number | null;
}
export interface MarketStats {
  n: number;
  businesses: number;
  min: Dec;
  q1: Dec;
  median: Dec;
  q3: Dec;
  p90: Dec;
  max: Dec;
  median_age_days: number;
  newest: string;
  oldest: string;
  with_distance: number;
  max_distance_km: number | null;
}
export interface Decision {
  id: number;
  included: boolean;
  reason: string | null;
}
export interface MarketEvidenceView {
  evidence: { quality: "none" | "low" | "medium" | "high"; reasons: string[]; stats: MarketStats | null; decisions: Decision[]; starting_at_count: number };
  rules: { reference_date: string; max_age_days: number; radius_km: number | null; hair_length: string; stylist_level: string; include_starting_at: boolean };
  observations: Observation[];
  position: Position | null;
  suggested: Dec | null;
  suggestion_note: string;
  conflict: string | null;
  cpi_adjusted_median: Dec | null;
  cpi_note: string | null;
}
export interface EvidenceQuery {
  service_id: number;
  max_age_days: number | null;
  radius_km: number | null;
  hair_length: string;
  stylist_level: string;
  include_starting_at: boolean;
  position: Position | null;
  target_price: Dec | null;
}
export interface ObservationMapping {
  business: number | null;
  service_label: number | null;
  price: number | null;
  price_type: number | null;
  price_max: number | null;
  observed_on: number | null;
  source_url: number | null;
  hair_length: number | null;
  stylist_level: number | null;
  duration_min: number | null;
  inclusions: number | null;
  notes: number | null;
  matched_service: number | null;
}
export interface ObsImportRow {
  line: number;
  status: "new" | "duplicate" | "error";
  message: string | null;
  business: string;
  service_label: string;
  price: string;
  observed_on: string;
  matched_service: string | null;
}
export interface ContextFigure {
  label: string;
  value: string | null;
  geography: string;
  dataset: string;
  note: string;
}
export interface SeriesPoint {
  year: string;
  period: string;
  value: Dec;
}
export interface LocalContext {
  census: ContextFigure[];
  census_error: string | null;
  wages: [string, SeriesPoint[]][];
  cpi: SeriesPoint[];
  bls_error: string | null;
}
export interface TaxRow {
  month: string;
  rate: string;
  taxable_sales: Dec;
  exempt_sales: Dec;
  tax_collected: Dec;
  tax_refunded: Dec;
  net_tax: Dec;
}
