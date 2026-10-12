import { Alert, Anchor, Badge, Button, Group, Paper, Progress, ScrollArea, Stack, Switch, Text, Title } from "@mantine/core";
import { IconCheck, IconDownload, IconRefresh, IconRotateClockwise } from "@tabler/icons-react";
import { useQueryClient } from "@tanstack/react-query";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useEffect, useRef, useState } from "react";
import { call } from "../../api/ipc";
import { useCmd } from "../../api/queries";
import type { Prefs } from "../../api/types";
import { usePrefs } from "../../lib/distance";
import { dateLabel, isoToLocal } from "../../lib/format";
import { explainUpdateError, fmtBytes } from "../../lib/updates";

export const RELEASES_URL = "https://github.com/fleetwoodjohnr/salon-management/releases/latest";

interface InstallInfo {
  bundle: "appimage" | "deb" | "rpm" | "nsis" | "msi" | "app" | null;
  os: string;
  arch: string;
}

const bundleLabel: Record<string, string> = { appimage: "AppImage", deb: "Debian/Ubuntu package (.deb)", rpm: "Fedora/RPM package", nsis: "Windows installer", msi: "Windows installer (MSI)", app: "macOS app" };

type Phase = "idle" | "checking" | "none" | "available" | "preparing" | "downloading" | "installing" | "ready" | "error";

export async function saveLastCheck(prefs: Prefs) {
  await call("prefs_set", { prefs: { ...prefs, last_update_check: new Date().toISOString() } });
}

/** Settings → Application updates: check GitHub Releases, then download, verify and install. */
export function UpdatesTab() {
  const qc = useQueryClient();
  const prefs = usePrefs();
  const kind = useCmd<InstallInfo>("app_install_kind");
  const [version, setVersion] = useState("");
  const [phase, setPhase] = useState<Phase>("idle");
  const [error, setError] = useState<{ kind: "check" | "install"; message: string } | null>(null);
  const [got, setGot] = useState(0);
  const [total, setTotal] = useState<number | null>(null);
  const [speed, setSpeed] = useState(0);
  const update = useRef<Update | null>(null);
  const samples = useRef<[number, number][]>([]);
  useEffect(() => {
    getVersion().then(setVersion);
  }, []);
  const canInstall = kind.data?.bundle != null;
  const windows = kind.data?.os === "windows";

  async function runCheck() {
    setPhase("checking");
    setError(null);
    try {
      update.current = await check({ timeout: 30000 });
      setPhase(update.current ? "available" : "none");
      if (prefs.data) await saveLastCheck(prefs.data);
      qc.invalidateQueries({ queryKey: ["prefs_get"] });
    } catch (e) {
      setError({ kind: "check", message: explainUpdateError(String(e), "check") });
      setPhase("error");
    }
  }

  async function install() {
    const u = update.current;
    if (!u) return;
    setError(null);
    setPhase("preparing");
    try {
      await call("update_prepare", { version: u.version });
    } catch (e) {
      setError({ kind: "install", message: `A safety backup couldn't be made, so the update wasn't started: ${String(e)}` });
      setPhase("error");
      return;
    }
    let received = 0;
    let stage: "download" | "install" = "download";
    samples.current = [];
    setGot(0);
    setTotal(null);
    setPhase("downloading");
    try {
      await u.downloadAndInstall((ev) => {
        if (ev.event === "Started") setTotal(ev.data.contentLength ?? null);
        else if (ev.event === "Progress") {
          received += ev.data.chunkLength;
          setGot(received);
          const now = performance.now();
          samples.current = [...samples.current.filter(([t]) => now - t < 3000), [now, received]];
          const [t0, b0] = samples.current[0];
          if (now > t0) setSpeed(((received - b0) / (now - t0)) * 1000);
        } else if (ev.event === "Finished") {
          stage = "install";
          setPhase("installing");
        }
      });
      setPhase("ready");
    } catch (e) {
      setError({ kind: "install", message: explainUpdateError(String(e), stage) });
      setPhase("error");
    }
  }

  const u = update.current;
  const busy = ["checking", "preparing", "downloading", "installing"].includes(phase);
  return (
    <Stack maw={820}>
      <Paper p="lg">
        <Group justify="space-between" align="flex-start">
          <div>
            <Title order={4}>Salon Resource Manager {version}</Title>
            <Text size="sm" c="dimmed">
              {kind.data?.bundle ? `Installed as: ${bundleLabel[kind.data.bundle] ?? kind.data.bundle} (${kind.data.os}, ${kind.data.arch})` : kind.data ? "Development or portable copy: updates must be installed by hand." : ""}
            </Text>
            {prefs.data?.last_update_check && (
              <Text size="xs" c="dimmed">
                Last checked {isoToLocal(prefs.data.last_update_check)}
              </Text>
            )}
          </div>
          <Button leftSection={<IconRefresh size={16} />} onClick={runCheck} loading={phase === "checking"} disabled={busy && phase !== "checking"}>
            Check for updates
          </Button>
        </Group>
        {prefs.data && (
          <Switch
            mt="md"
            label="Check for updates automatically (once a day, when the app starts)"
            description="Asks GitHub whether a newer release exists. Nothing about you or your data is sent."
            checked={prefs.data.auto_update_check}
            onChange={async (e) => {
              await call("prefs_set", { prefs: { ...prefs.data!, auto_update_check: e.currentTarget.checked } });
              qc.invalidateQueries({ queryKey: ["prefs_get"] });
            }}
          />
        )}
      </Paper>

      {phase === "none" && (
        <Alert color="teal" icon={<IconCheck size={16} />}>
          You have the latest version.
        </Alert>
      )}
      {error && (
        <Alert color={error.kind === "check" ? "yellow" : "red"} title={error.kind === "check" ? "Couldn't check for updates" : "Update not installed"}>
          {error.message}{" "}
          <Anchor size="sm" onClick={() => openUrl(RELEASES_URL)}>
            Releases on GitHub
          </Anchor>
        </Alert>
      )}

      {u && phase !== "none" && (
        <Paper p="lg">
          <Group justify="space-between" mb="xs">
            <Title order={4}>Update available</Title>
            <Badge size="lg">
              {u.currentVersion} → {u.version}
            </Badge>
          </Group>
          {u.date && (
            <Text size="sm" c="dimmed" mb="xs">
              Released {dateLabel(u.date.slice(0, 10))}
            </Text>
          )}
          {u.body && (
            <ScrollArea.Autosize mah={260} mb="md">
              <Text size="sm" style={{ whiteSpace: "pre-wrap" }}>
                {u.body}
              </Text>
            </ScrollArea.Autosize>
          )}
          {(phase === "downloading" || phase === "installing" || phase === "preparing") && (
            <Stack gap={4} mb="md">
              <Progress value={total ? (got / total) * 100 : 100} animated striped={!total || phase !== "downloading"} aria-label="Download progress" />
              <Text size="xs" c="dimmed">
                {phase === "preparing" && "Backing up your workspace…"}
                {phase === "downloading" && `Downloading ${fmtBytes(got)}${total ? ` of ${fmtBytes(total)} (${Math.floor((got / total) * 100)}%)` : ""}${speed > 0 ? ` · ${fmtBytes(Math.round(speed))}/s` : ""}`}
                {phase === "installing" && (windows ? "Verified. The installer is starting; the app will close and reopen by itself." : "Verified. Installing…")}
              </Text>
            </Stack>
          )}
          {phase === "ready" ? (
            <Group>
              <Alert color="teal" icon={<IconCheck size={16} />} style={{ flex: 1 }}>
                Version {u.version} is installed. Restart to start using it.
              </Alert>
              <Button leftSection={<IconRotateClockwise size={16} />} onClick={() => relaunch()}>
                Restart now
              </Button>
            </Group>
          ) : canInstall ? (
            <Stack gap="xs">
              <Group>
                <Button leftSection={<IconDownload size={16} />} onClick={install} loading={busy && phase !== "checking"} disabled={phase === "checking"}>
                  Download &amp; install update
                </Button>
              </Group>
              <Text size="xs" c="dimmed">
                The update is checked against the project's signing key before anything is installed. Your workspaces, backups and settings are kept, and a backup of the open workspace is made
                first.
                {(kind.data?.bundle === "deb" || kind.data?.bundle === "rpm") && " Your computer will ask for your password to install the package."}
              </Text>
            </Stack>
          ) : (
            <Button variant="light" onClick={() => openUrl(RELEASES_URL)}>
              Download from GitHub
            </Button>
          )}
        </Paper>
      )}
    </Stack>
  );
}
