// Verifies window.print() opens the native GTK print dialog (used for receipts, estimates and PDF).
// Captures the whole virtual screen with xwd, because the dialog is outside the webview.
import { execSync } from "node:child_process";
import { launch, waitText, stopDisplay, go } from "./harness.mjs";
const app = await launch({ dataDir: process.argv[2] });
const b = app.browser;
try {
  await waitText(b, "Rowan & Ash Hair Studio");
  await go(b, "/print/sale/1");
  await waitText(b, "Receipt S-00001");
  // Call print asynchronously so the WebDriver call returns while the dialog is open.
  await b.execute(() => setTimeout(() => window.print(), 100));
  await new Promise((r) => setTimeout(r, 4000));
  execSync(`xwd -root -silent -display ${process.env.DISPLAY} | magick xwd:- e2e/screenshots/60-print-dialog.png`);
  console.log("print-check captured");
} finally {
  await app.close().catch(() => {});
  stopDisplay();
}
