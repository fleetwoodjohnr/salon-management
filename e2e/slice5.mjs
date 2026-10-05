// Slice 5: dashboard widgets, layout edit, competitor discovery (live OSM), observed prices →
// evidence + conflict on Pricing, reports and sales tax.
import { launch, clickText, fill, choose, waitText, stopDisplay, bodyText, go as goTo } from "./harness.mjs";
import assert from "node:assert/strict";
const dataDir = process.argv[2];
const app = await launch({ dataDir });
const b = app.browser;
const go = (p) => goTo(b, p);
try {
  await waitText(b, "Rowan & Ash Hair Studio");
  await go("/");
  await waitText(b, "Net revenue");
  await waitText(b, "Profitability by service");
  await new Promise((r) => setTimeout(r, 1500));
  await app.shot("50-dashboard");
  const t = await bodyText(b);
  assert.match(t, /\$85\.00/); // net revenue from the one finalized sale
  // Edit layout: remove the market widget and save
  await clickText(b, "Edit layout");
  await (await b.$('[aria-label="Remove Your prices vs the local market"]')).click();
  await clickText(b, "Save layout");
  await waitText(b, 'Saved view "Overview"');
  // Competitors near the geocoded location
  await go("/market?tab=competitors");
  await waitText(b, "Search OpenStreetMap");
  await clickText(b, "Search OpenStreetMap");
  await waitText(b, "Found", 60000);
  await app.shot("51-competitors");
  // Record 4 observed prices from different businesses
  await go("/market?tab=prices");
  await waitText(b, "Record a price");
  const names = ["Great Clips", "Perfect Look", "Legacy Nails Spa", "Nail Creations"];
  const prices = ["70", "78", "82", "90"];
  for (let i = 0; i < names.length; i++) {
    await clickText(b, "Record a price");
    await waitText(b, "Record an observed price");
    await choose(b, "Business", names[i]);
    await fill(b, "Price", prices[i]);
    await fill(b, "Source (web page)", "https://example.com/menu");
    await clickText(b, "Save price");
    await waitText(b, "Price recorded");
    await new Promise((r) => setTimeout(r, 300));
  }
  await waitText(b, "Medium evidence", 15000);
  await app.shot("52-market-evidence");
  await go("/pricing");
  await waitText(b, "Market evidence");
  await waitText(b, "can't meet both", 15000);
  await app.shot("53-pricing-conflict");
  await go("/reports");
  await waitText(b, "Period result");
  await app.shot("54-reports");
  await (await b.$("//*[@role='tab'][normalize-space(.)='Sales tax']")).click();
  await waitText(b, "Taxable sales");
  await app.shot("55-reports-tax");
  console.log("slice5 ok");
} catch (e) {
  await app.shot("fail");
  throw e;
} finally {
  await app.close();
  stopDisplay();
}
