import { Alert, Button, Code, Stack } from "@mantine/core";
import { useRouteError } from "react-router";
import { errorMessage } from "../api/ipc";

/** Shown instead of a blank screen if a page crashes. Saved data is unaffected. */
export function RouteError() {
  const error = useRouteError();
  return (
    <Stack p="xl" maw={720}>
      <Alert color="red" title="This page hit a problem">
        Your saved data is safe. Go back or reload the window to continue.
        <Code block mt="sm">
          {errorMessage(error)}
        </Code>
      </Alert>
      <Button variant="default" w="fit-content" onClick={() => window.location.reload()}>
        Reload window
      </Button>
    </Stack>
  );
}
