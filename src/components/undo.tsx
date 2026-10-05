import { Button, Group, Text } from "@mantine/core";
import { notifications } from "@mantine/notifications";

/** Toast with an Undo button for reversible actions. */
export function notifyUndo(message: string, undo: () => Promise<unknown> | void) {
  const id = notifications.show({
    color: "teal",
    autoClose: 8000,
    message: (
      <Group justify="space-between" wrap="nowrap" gap="sm">
        <Text size="sm">{message}</Text>
        <Button
          size="compact-xs"
          variant="light"
          onClick={async () => {
            notifications.hide(id);
            await undo();
          }}
        >
          Undo
        </Button>
      </Group>
    ),
  });
}
