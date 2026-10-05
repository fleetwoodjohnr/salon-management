import { Alert, Center, Loader } from "@mantine/core";
import { IconAlertTriangle } from "@tabler/icons-react";
import type { ReactNode } from "react";
import { errorMessage } from "../api/ipc";

/** Standard loading / error presentation for a query. */
export function QueryState({ loading, error, children }: { loading: boolean; error: unknown; children: () => ReactNode }) {
  if (error) {
    return (
      <Alert color="red" icon={<IconAlertTriangle size={18} />} title="Couldn't load this">
        {errorMessage(error)}
      </Alert>
    );
  }
  if (loading) {
    return (
      <Center py="xl">
        <Loader size="sm" aria-label="Loading" />
      </Center>
    );
  }
  return <>{children()}</>;
}
