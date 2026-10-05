import {
  ActionIcon,
  Alert,
  AppShell,
  Button,
  Group,
  Kbd,
  Menu,
  Modal,
  NavLink,
  ScrollArea,
  Text,
  Tooltip,
  UnstyledButton,
  useComputedColorScheme,
  useMantineColorScheme,
} from "@mantine/core";
import { useHotkeys } from "@mantine/hooks";
import { Spotlight, spotlight, type SpotlightActionData } from "@mantine/spotlight";
import { IconChevronDown, IconMoon, IconSearch, IconSun } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask } from "@tauri-apps/plugin-dialog";
import { useEffect } from "react";
import { NavLink as RouterNavLink, Outlet, useBlocker, useLocation, useNavigate } from "react-router";
import { call } from "../api/ipc";
import type { AppStatus } from "../api/types";
import { anyDirty } from "../components/dirty";
import { STATUS_KEY, useApplyStatus } from "./App";
import { navItems, navSections } from "./nav";

function WorkspaceMenu({ status }: { status: AppStatus }) {
  const apply = useApplyStatus();
  const navigate = useNavigate();
  const cur = status.current!;
  return (
    <Menu position="bottom-start" width={280} shadow="md">
      <Menu.Target>
        <UnstyledButton aria-label="Switch workspace" px="xs" py={4} style={{ borderRadius: 8 }}>
          <Group gap={6} wrap="nowrap">
            <Text fw={650} size="sm" truncate maw={220}>
              {cur.name}
            </Text>
            <IconChevronDown size={14} />
          </Group>
        </UnstyledButton>
      </Menu.Target>
      <Menu.Dropdown>
        <Menu.Label>Workspaces on this computer</Menu.Label>
        {status.workspaces.map((w) => (
          <Menu.Item
            key={w.id}
            disabled={w.id === cur.id}
            rightSection={w.kind === "demo" ? <Text size="xs" c="dimmed">demo</Text> : undefined}
            onClick={async () => apply(await call<AppStatus>("workspace_open", { id: w.id }))}
          >
            {w.name}
          </Menu.Item>
        ))}
        <Menu.Divider />
        <Menu.Item onClick={() => navigate("/settings?tab=workspaces")}>Manage workspaces and backups</Menu.Item>
        <Menu.Item onClick={async () => apply(await call<AppStatus>("workspace_close"))}>Close workspace</Menu.Item>
      </Menu.Dropdown>
    </Menu>
  );
}

function UnsavedChangesGuard() {
  const blocker = useBlocker(({ currentLocation, nextLocation }) => anyDirty() && currentLocation.pathname !== nextLocation.pathname);
  return (
    <Modal opened={blocker.state === "blocked"} onClose={() => blocker.reset?.()} title="Leave without saving?">
      <Text size="sm" mb="lg">
        You have changes on this page that haven't been saved. Leaving will discard them.
      </Text>
      <Group justify="flex-end">
        <Button variant="default" onClick={() => blocker.reset?.()}>
          Keep editing
        </Button>
        <Button color="red" onClick={() => blocker.proceed?.()}>
          Discard changes
        </Button>
      </Group>
    </Modal>
  );
}

export function Shell() {
  const { data: status } = useQuery<AppStatus>({ queryKey: STATUS_KEY, enabled: false });
  const navigate = useNavigate();
  const location = useLocation();
  const { setColorScheme } = useMantineColorScheme();
  const scheme = useComputedColorScheme("light");

  useHotkeys([
    ["mod+K", () => spotlight.open()],
    ...navItems.slice(0, 9).map((it, i) => [`mod+${i + 1}`, () => navigate(it.to)] as [string, () => void]),
  ]);

  // Ask before the window closes with unsaved changes.
  useEffect(() => {
    const win = getCurrentWindow();
    const un = win.onCloseRequested(async (e) => {
      if (!anyDirty()) return;
      e.preventDefault();
      if (await ask("You have unsaved changes. Close Salon Resource Manager anyway?", { title: "Unsaved changes", kind: "warning" })) {
        await win.destroy();
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  const actions: SpotlightActionData[] = navItems.map((it) => ({
    id: it.to,
    label: it.label,
    description: it.description,
    leftSection: <it.icon size={18} />,
    onClick: () => navigate(it.to),
  }));

  if (!status?.current) return null;
  const isDemo = status.current.kind === "demo";

  return (
    <AppShell header={{ height: isDemo ? 84 : 52 }} navbar={{ width: 232, breakpoint: 0 }} padding="xl">
      <AppShell.Header>
        {isDemo && (
          <Alert color="yellow" variant="filled" radius={0} py={6} styles={{ message: { textAlign: "center", fontWeight: 600 } }}>
            Demo workspace — sample data only. Your real business data lives in a separate workspace.
          </Alert>
        )}
        <Group h={52} px="md" justify="space-between" wrap="nowrap">
          <Group gap="sm" wrap="nowrap">
            <Text className="srm-figure srm-brand" fz={19} style={{ whiteSpace: "nowrap" }}>
              Salon Resource Manager
            </Text>
            <WorkspaceMenu status={status} />
          </Group>
          <Group gap="xs" wrap="nowrap">
            <Button
              variant="default"
              size="xs"
              leftSection={<IconSearch size={14} />}
              rightSection={<Kbd size="xs">{/Mac/.test(navigator.userAgent) ? "⌘ K" : "Ctrl K"}</Kbd>}
              onClick={() => spotlight.open()}
            >
              Go to
            </Button>
            <Tooltip label={scheme === "dark" ? "Use light theme" : "Use dark theme"}>
              <ActionIcon
                variant="default"
                size="lg"
                aria-label="Toggle color theme"
                onClick={() => setColorScheme(scheme === "dark" ? "light" : "dark")}
              >
                {scheme === "dark" ? <IconSun size={18} /> : <IconMoon size={18} />}
              </ActionIcon>
            </Tooltip>
          </Group>
        </Group>
      </AppShell.Header>
      <AppShell.Navbar>
        <ScrollArea type="hover" px="xs" pb="md">
          <nav aria-label="Main">
            {navSections.map((sec) => (
              <div key={sec.title}>
                <div className="srm-nav-section">{sec.title}</div>
                {sec.items.map((it) => {
                  const active = it.to === "/" ? location.pathname === "/" : location.pathname.startsWith(it.to);
                  return (
                    <NavLink
                      key={it.to}
                      component={RouterNavLink}
                      to={it.to}
                      label={it.label}
                      active={active}
                      leftSection={<it.icon size={18} stroke={1.7} />}
                      variant="light"
                      style={{ borderRadius: 8 }}
                      aria-current={active ? "page" : undefined}
                    />
                  );
                })}
              </div>
            ))}
          </nav>
        </ScrollArea>
      </AppShell.Navbar>
      <AppShell.Main className="srm-main">
        <Outlet />
      </AppShell.Main>
      <UnsavedChangesGuard />
      <Spotlight actions={actions} nothingFound="No matching page" shortcut={null} searchProps={{ placeholder: "Go to…" }} />
    </AppShell>
  );
}
