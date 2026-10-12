// Slice 7 (0.2.0): distance units, a full market-research run against live OpenStreetMap and real
// salon websites, the market overview, and the in-app update check. Runs after slices 1–6 in any
// workspace (it searches around a ZIP code, so no located business is needed).
import { launch, clickText, clickLabel, fill, inputFor, waitText, stopDisplay, bodyText, go as goTo } from "./harness.mjs";
import assert from "node:assert/strict";
const dataDir = process.argv[2];
const app = await launch({ dataDir });
const b = app.browser;
const go = (p) => goTo(b, p);
const invoke = (cmd, args) => b.executeAsync((c, a, done) => window.__TAURI_INTERNALS__.invoke(c, a).then((r) => done({ ok: r }), (e) => done({ err: e })), cmd, args);
const within = async () => (await (await inputFor(b, "Within")).getValue()).trim();
try {
  await waitText(b, "Dashboard", 20000);
  // Units: default miles; switching shows the same radius in km and back, without changing it.
  const prefs0 = (await invoke("prefs_get", {})).ok;
  assert.equal(prefs0.distance_unit, "mi");
  await go("/market?tab=research");
  await waitText(b, "Start research");
  const mi = await within();
  assert.match(mi, / mi$/);
  await go("/settings?tab=general");
  await waitText(b, "Units");
  await clickLabel(b, "Kilometers (km)");
  await waitText(b, "Preference saved");
  await app.shot("70-units-km");
  await go("/market?tab=research");
  await waitText(b, "Start research");
  const km = await within();
  assert.match(km, / km$/);
  assert.ok(Math.abs(parseFloat(km) - parseFloat(mi) * 1.609344) < 0.02, `${mi} → ${km}`);
  assert.equal((await invoke("prefs_get", {})).ok.research_radius_km, prefs0.research_radius_km);
  await go("/settings?tab=general");
  await clickLabel(b, "Miles (mi)");
  await waitText(b, "Preference saved");

  // Market research around a ZIP code, reading websites.
  await go("/market?tab=research");
  await waitText(b, "Start research");
  await clickLabel(b, "A ZIP code");
  await fill(b, "ZIP code", "98501");
  await fill(b, "Within", "4");
  await clickText(b, "Start research");
  await waitText(b, "Searching", 30000);
  await app.shot("71-research-running");
  await waitText(b, "Last search", 8 * 60 * 1000);
  const p = (await invoke("research_progress", {})).ok;
  console.log("research summary:", JSON.stringify(p.summary));
  assert.equal(p.stage, "done", p.error ?? "");
  assert.ok(p.summary.sources.some((s) => s.name === "OpenStreetMap"));
  assert.ok(p.summary.in_radius > 0 && p.summary.websites_checked > 0, "found and checked something");
  await waitText(b, "Websites checked");
  await app.shot("72-research-done");

  // Overview and competitors reflect the run.
  await go("/market?tab=overview");
  await waitText(b, p.summary.prices_saved > 0 ? "Spread" : "No comparable prices yet", 20000);
  await app.shot("73-overview");
  await go("/market?tab=competitors");
  await waitText(b, "Website check");
  const t = await bodyText(b);
  assert.ok(/Prices found|No prices online|Couldn't reach|Site asks not to be read|Blocked|Platform terms/.test(t));
  await app.shot("74-competitors");

  // Application updates: the check reaches GitHub and reports one of the three outcomes.
  await go("/settings?tab=updates");
  await waitText(b, "Check for updates");
  await clickText(b, "Check for updates");
  await b.waitUntil(async () => /You have the latest version|Update available|No update information is published yet|Couldn't check for updates/.test(await bodyText(b)), { timeout: 40000, timeoutMsg: "no update result" });
  console.log("update check:", (await bodyText(b)).match(/You have the latest version|Update available[^\n]*|No update information[^\n]*|Couldn't check[^\n]*/)?.[0]);
  await app.shot("75-updates");
  console.log("slice7 ok");
} catch (e) {
  await app.shot("fail");
  throw e;
} finally {
  await app.close();
  stopDisplay();
}
