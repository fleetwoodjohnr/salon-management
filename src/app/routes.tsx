import { createHashRouter } from "react-router";
import { CalendarPage } from "../features/calendar/CalendarPage";
import { ClientDetailPage, ClientsPage } from "../features/clients/ClientsPage";
import { DashboardPage } from "../features/dashboard/DashboardPage";
import { ExpensesPage } from "../features/expenses/ExpensesPage";
import { MarketPage } from "../features/market/MarketPage";
import { ReportsPage } from "../features/reports/ReportsPage";
import { PrintPage } from "../features/print/PrintPage";
import { SaleEditorPage } from "../features/sales/SaleEditor";
import { SalesPage } from "../features/sales/SalesPage";
import { InventoryPage } from "../features/inventory/InventoryPage";
import { ProductEditorPage } from "../features/inventory/ProductEditor";
import { ReceivePage } from "../features/inventory/ReceivePage";
import { ServiceEditorPage } from "../features/services/ServiceEditor";
import { ServicesPage } from "../features/services/ServicesPage";
import { SetupPage } from "../features/onboarding/SetupPage";
import { PricingPage } from "../features/pricing/PricingPage";
import { TaxPage } from "../features/tax/TaxPage";
import { ProfileEditorPage } from "../features/profiles/ProfileEditor";
import { ProfilesPage } from "../features/profiles/ProfilesPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { RouteError } from "./RouteError";
import { Shell } from "./Shell";

export const router = createHashRouter([
  {
    path: "/",
    element: <Shell />,
    errorElement: <RouteError />,
    children: [
      { index: true, element: <DashboardPage /> },
      { path: "setup", element: <SetupPage /> },
      { path: "calendar", element: <CalendarPage /> },
      { path: "sales", element: <SalesPage /> },
      { path: "sales/:id", element: <SaleEditorPage /> },
      { path: "print/:kind/:id", element: <PrintPage /> },
      { path: "clients", element: <ClientsPage /> },
      { path: "clients/:id", element: <ClientDetailPage /> },
      { path: "services", element: <ServicesPage /> },
      { path: "services/:id", element: <ServiceEditorPage /> },
      { path: "pricing", element: <PricingPage /> },
      { path: "inventory", element: <InventoryPage /> },
      { path: "inventory/receive", element: <ReceivePage /> },
      { path: "inventory/products/:id", element: <ProductEditorPage /> },
      { path: "expenses", element: <ExpensesPage /> },
      { path: "reports", element: <ReportsPage /> },
      { path: "market", element: <MarketPage /> },
      { path: "profiles", element: <ProfilesPage /> },
      { path: "profiles/:id", element: <ProfileEditorPage /> },
      { path: "tax", element: <TaxPage /> },
      { path: "settings", element: <SettingsPage /> },
    ],
  },
]);
