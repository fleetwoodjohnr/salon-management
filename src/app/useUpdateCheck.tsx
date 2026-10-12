import { Anchor } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { check } from "@tauri-apps/plugin-updater";
import { useEffect, useRef } from "react";
import { useNavigate } from "react-router";
import { usePrefs } from "../lib/distance";
import { saveLastCheck } from "../features/settings/UpdatesTab";

/** Once a day at startup (if allowed in Settings), ask GitHub for a newer release and say so.
 *  Failures are silent here; Settings → Application updates shows them. */
export function useUpdateCheck() {
  const prefs = usePrefs();
  const navigate = useNavigate();
  const done = useRef(false);
  useEffect(() => {
    const p = prefs.data;
    if (!p || done.current) return;
    done.current = true;
    if (!p.auto_update_check || (p.last_update_check && Date.now() - Date.parse(p.last_update_check) < 24 * 3600 * 1000)) return;
    check({ timeout: 20000 })
      .then(async (u) => {
        await saveLastCheck(p);
        if (u)
          notifications.show({
            id: "update-available",
            title: `Update available: version ${u.version}`,
            message: (
              <Anchor size="sm" onClick={() => (notifications.hide("update-available"), navigate("/settings?tab=updates"))}>
                See what's new and install
              </Anchor>
            ),
            autoClose: false,
          });
      })
      .catch(() => {});
  }, [prefs.data, navigate]);
}
