// Slice 6: export everything, back up, restore into a new workspace and confirm the data, then the
// demo workspace and a visual pass of main pages in both themes. Run after slices 1–5.
// The restore and export steps call the app's own commands (the native file dialogs can't be driven
// by WebDriver), so the real backend code paths run end to end.
import { existsSync, statSync } from "node:fs";
import { launch, clickText, waitText, stopDisplay, bodyText, go as goTo } from "./harness.mjs";
import assert from "node:assert/strict";
const dataDir = process.argv[2];
const app = await launch({ dataDir });
const b = app.browser;
const go = (p) => goTo(b, p);
const invoke = (cmd, args) => b.executeAsync((c, a, done) => window.__TAURI_INTERNALS__.invoke(c, a).then((r) => done({ ok: r }), (e) => done({ err: e })), cmd, args);
try {
  await waitText(b, "Rowan & Ash Hair Studio");
  // Portable export
  const exportPath = `${dataDir}/../export-${Date.now()}.zip`;
  const ex = await invoke("export_all", { path: exportPath });
  assert.ok(ex.ok > 30, JSON.stringify(ex));
  assert.ok(existsSync(exportPath) && statSync(exportPath).size > 1000);
  // Backup then restore as a new workspace
  await go("/settings?tab=workspaces");
  await waitText(b, "Back up now");
  await clickText(b, "Back up now");
  await waitText(b, "Backup created");
  const list = await invoke("backup_list", {});
  const manual = list.ok.find((x) => x.manifest?.reason === "manual");
  assert.ok(manual, "manual backup listed");
  const restored = await invoke("backup_restore", { path: manual.path, replaceCurrent: false });
  assert.ok(restored.ok?.current?.name.includes("(restored"), JSON.stringify(restored));
  await b.execute(() => window.location.reload());
  await waitText(b, "(restored", 20000);
  await go("/sales");
  await waitText(b, "S-00001");
  await go("/inventory");
  await waitText(b, "27.5 fl oz");
  await app.shot("70-restored-workspace");
  // Demo workspace
  await go("/settings?tab=workspaces");
  await waitText(b, "Create demo workspace");
  await clickText(b, "Create demo workspace");
  await waitText(b, "Demo workspace — sample data only", 120000);
  await waitText(b, "Profitability by service", 30000);
  await new Promise((r) => setTimeout(r, 2500));
  await app.shot("71-demo-dashboard-dark");
  for (const [path, name, text] of [
    ["/calendar", "72-demo-calendar", "Drag to reschedule"],
    ["/inventory", "73-demo-inventory", "Permanent color 7N"],
    ["/services", "74-demo-services", "Partial highlights"],
    ["/clients", "75-demo-clients", "Mia"],
    ["/market?tab=prices", "76-demo-market", "Market evidence"],
    ["/reports", "77-demo-reports", "Period result"],
  ]) {
    await go(path);
    await waitText(b, text, 20000);
    await new Promise((r) => setTimeout(r, 1200));
    await app.shot(name);
  }
  await (await b.$('[aria-label="Toggle color theme"]')).click();
  await go("/");
  await waitText(b, "Profitability by service", 20000);
  await new Promise((r) => setTimeout(r, 2500));
  await app.shot("78-demo-dashboard-light");
  await go("/sales");
  await waitText(b, "Finalized");
  await app.shot("79-demo-sales-light");
  const t = await bodyText(b);
  assert.match(t, /S-0\d{4}/);
  console.log("slice6 ok");
} catch (e) {
  await app.shot("fail");
  throw e;
} finally {
  await app.close();
  stopDisplay();
}
