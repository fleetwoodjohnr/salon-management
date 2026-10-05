import "@fontsource-variable/figtree";
import "@fontsource-variable/bodoni-moda";
import "@mantine/core/styles.css";
import "@mantine/dates/styles.css";
import "@mantine/charts/styles.css";
import "@mantine/notifications/styles.css";
import "@mantine/spotlight/styles.css";
import "mantine-datatable/styles.css";
import "./app/theme.css";

import { MantineProvider } from "@mantine/core";
import { ModalsProvider } from "@mantine/modals";
import { Notifications } from "@mantine/notifications";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./app/App";
import { theme } from "./app/theme";

export const queryClient = new QueryClient({
  defaultOptions: {
    // Local database: data is fresh until we change it ourselves (mutations invalidate).
    queries: { staleTime: Infinity, retry: false, refetchOnWindowFocus: false },
  },
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <MantineProvider theme={theme} defaultColorScheme="auto">
      <QueryClientProvider client={queryClient}>
        <ModalsProvider>
          <Notifications position="bottom-right" limit={4} />
          <App />
        </ModalsProvider>
      </QueryClientProvider>
    </MantineProvider>
  </React.StrictMode>,
);
