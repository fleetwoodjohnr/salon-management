import {
  IconAdjustmentsDollar,
  IconBox,
  IconCalendarEvent,
  IconCashRegister,
  IconChartBar,
  IconLayoutDashboard,
  IconMapSearch,
  IconReceiptTax,
  IconScissors,
  IconSettings,
  IconUserCircle,
  IconUsers,
  IconWallet,
} from "@tabler/icons-react";
import type { ComponentType } from "react";

export interface NavItem {
  to: string;
  label: string;
  icon: ComponentType<{ size?: number; stroke?: number }>;
  description: string;
}

export const navSections: { title: string; items: NavItem[] }[] = [
  {
    title: "Today",
    items: [
      { to: "/", label: "Dashboard", icon: IconLayoutDashboard, description: "Revenue, profit, stock and appointments at a glance" },
      { to: "/calendar", label: "Calendar", icon: IconCalendarEvent, description: "Appointments by day and staff" },
      { to: "/sales", label: "Sales", icon: IconCashRegister, description: "Tickets, payments, refunds and receipts" },
      { to: "/clients", label: "Clients", icon: IconUsers, description: "Client records, history and formulas" },
    ],
  },
  {
    title: "Catalog",
    items: [
      { to: "/services", label: "Services", icon: IconScissors, description: "Services, recipes and cost breakdowns" },
      { to: "/pricing", label: "Pricing", icon: IconAdjustmentsDollar, description: "Recommended prices and what-if" },
      { to: "/inventory", label: "Inventory", icon: IconBox, description: "Products, purchases and stock ledger" },
    ],
  },
  {
    title: "Money",
    items: [
      { to: "/expenses", label: "Expenses", icon: IconWallet, description: "Rent, utilities and other spending" },
      { to: "/reports", label: "Reports", icon: IconChartBar, description: "Profitability and period reports" },
      { to: "/market", label: "Market", icon: IconMapSearch, description: "Competitor prices and local context" },
    ],
  },
  {
    title: "Setup",
    items: [
      { to: "/profiles", label: "Work profiles", icon: IconUserCircle, description: "Pay, hours and overhead used for costing" },
      { to: "/tax", label: "Sales tax", icon: IconReceiptTax, description: "Rates, jurisdictions and taxability" },
      { to: "/settings", label: "Settings", icon: IconSettings, description: "Business, locations, staff, backups" },
    ],
  },
];

export const navItems = navSections.flatMap((s) => s.items);
