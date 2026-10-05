import { Group, Stack, Text } from "@mantine/core";
import type { ReactNode } from "react";

export function PageHeader({ title, description, actions }: { title: string; description?: ReactNode; actions?: ReactNode }) {
  return (
    <Group justify="space-between" align="flex-end" mb="lg" wrap="nowrap" gap="xl">
      <Stack gap={4} maw={720}>
        <h1 className="srm-page-title" style={{ margin: 0 }}>
          {title}
        </h1>
        {description && (
          <Text c="dimmed" size="sm">
            {description}
          </Text>
        )}
      </Stack>
      {actions && (
        <Group gap="sm" wrap="nowrap" className="srm-no-print" style={{ flexShrink: 0 }}>
          {actions}
        </Group>
      )}
    </Group>
  );
}
