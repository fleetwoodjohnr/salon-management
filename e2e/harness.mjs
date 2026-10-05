// Drives the real desktop binary through tauri-driver + WebKitWebDriver on a private Xvfb display.
// Usage from tests: const app = await launch({ dataDir }); ... await app.close();
import { spawn } from "node:child_process";
import { mkdirSync } from "node:fs";
import { setTimeout as sleep } from "node:timers/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { remote } from "webdriverio";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const BIN = process.env.SRM_BIN ?? path.join(root, "src-tauri/target/debug/salon-resource-manager");
export const SHOTS = process.env.SRM_SHOTS ?? path.join(root, "e2e/screenshots");

let xvfb;
export async function startDisplay(n = 98) {
  if (xvfb) return;
  xvfb = spawn("Xvfb", [`:${n}`, "-screen", "0", "1440x900x24", "-nolisten", "tcp"], { stdio: "ignore" });
  process.env.DISPLAY = `:${n}`;
  await sleep(800);
}
export function stopDisplay() {
  xvfb?.kill();
  xvfb = undefined;
}

export async function launch({ dataDir, port = 4444 }) {
  await startDisplay();
  mkdirSync(dataDir, { recursive: true });
  const env = { ...process.env, SRM_DATA_DIR: dataDir, GDK_BACKEND: "x11", WEBKIT_DISABLE_DMABUF_RENDERER: "1" };
  const driver = spawn("tauri-driver", ["--port", String(port), "--native-port", String(port + 1)], { env, stdio: ["ignore", "ignore", "pipe"] });
  let stderr = "";
  driver.stderr.on("data", (d) => (stderr += d));
  await sleep(1000);
  let browser;
  try {
    browser = await remote({
      hostname: "127.0.0.1",
      port,
      logLevel: "error",
      capabilities: { "tauri:options": { application: BIN } },
    });
  } catch (e) {
    driver.kill();
    throw new Error(`Could not start app session: ${e.message}\n${stderr}`);
  }
  await browser.setWindowSize?.(1360, 860).catch(() => {});
  return {
    browser,
    async shot(name) {
      mkdirSync(SHOTS, { recursive: true });
      await browser.saveScreenshot(path.join(SHOTS, `${name}.png`));
    },
    async close() {
      await browser.deleteSession().catch(() => {});
      driver.kill();
      await sleep(500);
    },
  };
}

/** Click the first interactive element (button, link, tab, option…) whose text contains `text`;
 *  falls back to the element whose own text equals `text` (e.g. a table cell). */
export async function clickText(browser, text) {
  const t = JSON.stringify(text);
  const interactive = `(//button|//a|//*[@role='button' or @role='tab' or @role='menuitem' or @role='option' or @role='radio'])[contains(normalize-space(.), ${t})]`;
  let el = await browser.$(interactive);
  if (!(await el.isExisting())) el = await browser.$(`//*[normalize-space(text())=${t}]`);
  await el.waitForClickable({ timeout: 10000 });
  await el.click();
}

export async function inputFor(browser, label) {
  const byLabel = await browser.$(`//label[normalize-space(.)=${JSON.stringify(label)} or normalize-space(text())=${JSON.stringify(label)}]`);
  let input;
  if (await byLabel.isExisting()) {
    const id = await byLabel.getAttribute("for");
    input = await browser.$(`#${id.replace(/([:.])/g, "\\$1")}`);
  } else {
    input = await browser.$(`[aria-label=${JSON.stringify(label)}]`);
  }
  await input.waitForDisplayed({ timeout: 10000 });
  return input;
}

/** Replace the value of the input labelled `label` (uses <label for> or aria-label). */
export async function fill(browser, label, value) {
  const input = await inputFor(browser, label);
  await input.click();
  // Select everything and type over it, like a person would; WebDriver's clear doesn't fire
  // React's onChange, which leaves formatted inputs (e.g. "0 min") with stale text.
  await browser.execute((el) => el.select?.(), input);
  await browser.keys(["Control", "a"]);
  if (value === "") await browser.keys(["Backspace"]);
  else await input.addValue(value);
}

export async function waitText(browser, text, timeout = 10000) {
  await browser.waitUntil(async () => (await bodyText(browser)).includes(text), { timeout, timeoutMsg: `Text not found: ${text}` });
}

/** Choose `option` in the Mantine Select labelled `label`. */
export async function choose(browser, label, option) {
  await (await inputFor(browser, label)).click();
  // Closed comboboxes can leave hidden options in the DOM: click the visible one.
  const xp = `//*[@role="option"][normalize-space(.)=${JSON.stringify(option)}]`;
  await browser.waitUntil(async () => {
    for (const o of await browser.$$(xp)) if (await o.isDisplayed()) return true;
    return false;
  }, { timeout: 10000, timeoutMsg: `Option not shown: ${option}` });
  for (const o of await browser.$$(xp)) {
    if (await o.isDisplayed()) {
      await o.click();
      return;
    }
  }
}

/** Rendered text of the whole page (WebKitWebDriver's getText skips button contents). */
export async function bodyText(browser) {
  return browser.execute(() => document.body.innerText);
}

/** Navigate inside the app (hash router) without reloading. */
export async function go(browser, path) {
  await browser.execute((p) => {
    window.location.hash = p;
  }, path);
}
