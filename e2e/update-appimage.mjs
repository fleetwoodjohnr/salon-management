// In-app update, Linux AppImage, end to end against a local "GitHub": an interrupted download and a
// badly signed package must leave the installed AppImage untouched; a good one must replace it,
// keep the data, make a pre-update backup, and start as the new version.
//
// Needs two AppImages built with an updater config pointing at http://127.0.0.1:8765/latest.json
// and a test key, in the same system as the WebDriver (an AppImage carries its own WebKitGTK,
// which must match WebKitWebDriver's). Build each with
//   TAURI_SIGNING_PRIVATE_KEY=test.key npx tauri build --debug --bundles appimage --config conf-<v>.json
// where conf-<v>.json sets "version", bundle.createUpdaterArtifacts, and plugins.updater.{pubkey,
// endpoints, dangerousInsecureTransportProtocol}; then run
//   APPIMAGE_EXTRACT_AND_RUN=1 node e2e/update-appimage.mjs old.AppImage new.AppImage <empty-data-dir>
import assert from "node:assert/strict";
import { execFileSync, execSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmodSync, copyFileSync, readdirSync, readFileSync } from "node:fs";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { clickText, launch, stopDisplay, waitText, go as goTo, bodyText } from "./harness.mjs";

if (process.argv[2] === "--verify") {
  // Second process: start the replaced AppImage and check the version and the data.
  const [, , , bin, dir, version, services] = process.argv;
  const app = await launch({ dataDir: dir, bin });
  const b = app.browser;
  try {
    await waitText(b, "Dashboard", 30000);
    const v = await b.executeAsync((done) => window.__TAURI_INTERNALS__.invoke("plugin:app|version").then(done));
    assert.equal(v, version);
    const list = await b.executeAsync((done) => window.__TAURI_INTERNALS__.invoke("services_list", { includeArchived: true }).then(done));
    assert.equal(list.length, Number(services));
    await app.shot("83-update-restarted");
    console.log(`restarted as ${v} with ${list.length} services`);
  } finally {
    await app.close();
    stopDisplay();
  }
  process.exit(0);
}

const [oldImage, newImage, dataDir] = process.argv.slice(2);
const work = path.join(dataDir, "..", "Salon-Resource-Manager.AppImage");
copyFileSync(oldImage, work);
chmodSync(work, 0o755);
const sha = (f) => createHash("sha256").update(readFileSync(f)).digest("hex");
const oldSha = sha(work);
const newBytes = readFileSync(newImage);
const newVersion = process.env.NEW_VERSION ?? "0.2.1";

let mode = "good";
const manifest = () => ({
  version: newVersion,
  notes: "Test release notes",
  pub_date: "2026-10-12T00:00:00Z",
  platforms: {
    "linux-x86_64-appimage": {
      // "badsig": a well-formed signature of a different file.
      signature: readFileSync(`${mode === "badsig" ? oldImage : newImage}.sig`, "utf8").trim(),
      url: "http://127.0.0.1:8765/new.AppImage",
    },
  },
});
const server = http
  .createServer((req, res) => {
    if (req.url === "/latest.json") return res.writeHead(200, { "Content-Type": "application/json" }).end(JSON.stringify(manifest()));
    if (req.url === "/new.AppImage") {
      res.writeHead(200, { "Content-Type": "application/octet-stream", "Content-Length": newBytes.length });
      if (mode === "truncate") {
        res.write(newBytes.subarray(0, Math.floor(newBytes.length / 3)));
        return setTimeout(() => res.destroy(), 200);
      }
      return res.end(newBytes);
    }
    res.writeHead(404).end();
  })
  .listen(8765, "127.0.0.1");

async function attempt(b, expect) {
  // Navigate (again if the app's own start-up redirect wins the race).
  for (let i = 0; i < 3 && !(await bodyText(b)).includes("Check for updates"); i++) {
    await goTo(b, "/settings?tab=updates");
    await waitText(b, "Check for updates", 5000).catch(() => {});
  }
  await waitText(b, "Check for updates");
  await clickText(b, "Check for updates");
  await waitText(b, `→ ${newVersion}`, 20000);
  await clickText(b, "Download & install update");
  await b.waitUntil(async () => expect.test(await bodyText(b)), { timeout: 180000, timeoutMsg: `expected ${expect}` });
}

let app = await launch({ dataDir, bin: work });
let b = app.browser;
const invoke = (cmd, args) => b.executeAsync((c, a, done) => window.__TAURI_INTERNALS__.invoke(c, a).then((r) => done({ ok: r }), (e) => done({ err: e })), cmd, args);
try {
  const st = (await invoke("demo_create", {})).ok;
  assert.equal(await b.executeAsync((done) => window.__TAURI_INTERNALS__.invoke("plugin:app|version").then(done)), "0.2.0");
  const before = (await invoke("services_list", { includeArchived: true })).ok.length;
  await b.execute(() => window.location.reload());
  await waitText(b, "Net revenue", 30000);

  mode = "truncate";
  await attempt(b, /download was interrupted/i);
  assert.equal(sha(work), oldSha, "interrupted download changed the AppImage");
  await app.shot("80-update-interrupted");

  mode = "badsig";
  await attempt(b, /couldn't be verified/i);
  assert.equal(sha(work), oldSha, "badly signed update changed the AppImage");
  await app.shot("81-update-bad-signature");

  mode = "good";
  await attempt(b, /Restart now/);
  assert.equal(sha(work), sha(newImage), "AppImage not replaced");
  await app.shot("82-update-installed");
  const ws = st.current.id;
  const backups = readdirSync(path.join(dataDir, "backups", ws)).filter((f) => f.startsWith(`pre-update-${newVersion}`));
  assert.equal(backups.length, 3, "one pre-update backup per install attempt");
  await app.close();
  stopDisplay();
  // The harness can't start a second app session in one process: verify the restart in another.
  // The app allows one instance, and the app an AppImage runtime started can outlive the session.
  execSync("pkill -f 'appimage_extracte[d]_[0-9a-f]+/usr/bin/salon-resource-manager' || true");
  execFileSync(process.execPath, [fileURLToPath(import.meta.url), "--verify", work, dataDir, newVersion, String(before)], { stdio: "inherit" });
  console.log(`update-appimage ok: ${oldSha.slice(0, 12)} → ${sha(work).slice(0, 12)}, ${backups.length} pre-update backups, ${before} services kept`);
} catch (e) {
  await app.shot("fail-update");
  throw e;
} finally {
  await app.close();
  server.close();
  stopDisplay();
}
