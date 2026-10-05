import type { CompModel, OverheadBasis, Position, ProfileKind, RoundMode, TargetKind } from "../../api/types";

export const kindLabels: Record<ProfileKind, string> = {
  individual: "Individual stylist (owner)",
  chair_renter: "Chair or booth renter",
  independent: "Independent professional",
  employee: "Employee",
};

export const compLabels: Record<CompModel, { label: string; description: string }> = {
  owner_target_hourly: {
    label: "Owner pay per hour",
    description: "What you want to earn for each hour you work. Prices are set so this is covered before profit.",
  },
  hourly_wage: {
    label: "Hourly wage",
    description: "An employee paid by the hour. Add employer payroll costs (burden) if you pay them.",
  },
  commission: {
    label: "Commission only",
    description: "Paid a percentage of each service price. No hourly cost is added, so labor isn't counted twice.",
  },
  hourly_plus_commission: {
    label: "Hourly plus commission",
    description: "Both are paid. Use only when the hourly rate doesn't already include the commission.",
  },
};

export const targetLabels: Record<TargetKind, string> = { margin: "Margin", markup: "Markup" };

export const positionLabels: Record<Position, { label: string; description: string }> = {
  budget: { label: "Budget", description: "Lower quarter of observed local prices" },
  standard: { label: "Standard", description: "Middle of observed local prices (median)" },
  premium: { label: "Premium", description: "Upper quarter of observed local prices" },
  luxury: { label: "Luxury", description: "Top tenth of observed local prices" },
};

export const basisLabels: Record<OverheadBasis, string> = {
  occupied: "All chair time (hands-on, processing, setup and cleanup)",
  hands_on: "Hands-on time only (you serve other clients while color processes)",
};

export const roundLabels: Record<RoundMode, string> = { up: "Round up", nearest: "Round to nearest", down: "Round down" };

export const experienceLevels = [
  { value: "new", label: "New (under 2 years)" },
  { value: "experienced", label: "Experienced" },
  { value: "senior", label: "Senior" },
  { value: "master", label: "Master / educator" },
];
