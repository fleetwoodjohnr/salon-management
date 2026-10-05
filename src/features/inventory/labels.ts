import type { ProductCategory } from "../../api/types";

export const categoryLabels: Record<ProductCategory, string> = {
  professional: "Professional supply",
  consumable: "Disposable consumable",
  retail: "Retail product",
};

export const ledgerKindLabels: Record<string, string> = {
  purchase: "Purchase",
  opening: "Opening stock",
  service_use: "Used in service",
  retail_sale: "Retail sale",
  waste: "Waste",
  adjustment: "Count adjustment",
  transfer_out: "Transfer out",
  transfer_in: "Transfer in",
  supplier_return: "Returned to supplier",
  customer_return: "Customer return",
  reversal: "Correction",
  revaluation: "Revaluation",
};

export const reversibleKinds = new Set(["waste", "adjustment", "supplier_return", "transfer_out", "transfer_in"]);
