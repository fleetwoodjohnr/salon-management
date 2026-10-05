import { Alert, Center, Loader } from "@mantine/core";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { RouterProvider } from "react-router/dom";
import { call, errorMessage } from "../api/ipc";
import type { AppStatus } from "../api/types";
import { Welcome } from "../features/workspace/Welcome";
import { router } from "./routes";

export const STATUS_KEY = ["app_status"];

/** Replace app status after a workspace change and drop every cached query from the old one. */
export function useApplyStatus() {
  const qc = useQueryClient();
  return (s: AppStatus) => {
    // Drop every cached query from the previous workspace, but keep the status query object
    // itself (its observer lives in <App>).
    qc.removeQueries({ predicate: (q) => !(q.queryKey.length === 1 && q.queryKey[0] === STATUS_KEY[0]) });
    qc.setQueryData(STATUS_KEY, s);
    if (s.current) router.navigate("/");
  };
}

export function App() {
  const status = useQuery({ queryKey: STATUS_KEY, queryFn: () => call<AppStatus>("app_status") });
  if (status.isLoading) {
    return (
      <Center h="100%">
        <Loader aria-label="Starting" />
      </Center>
    );
  }
  if (status.error) {
    return (
      <Center h="100%" p="xl">
        <Alert color="red" title="Salon Resource Manager couldn't start" maw={560}>
          {errorMessage(status.error)}
        </Alert>
      </Center>
    );
  }
  const s = status.data!;
  if (!s.current) return <Welcome status={s} />;
  return <RouterProvider router={router} />;
}
