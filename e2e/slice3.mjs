// Slice 3: official tax lookup (live WA DOR), taxability decisions, geocoding, providers, what-if pricing.
import { launch, clickText, fill, choose, waitText, stopDisplay, bodyText, go as goTo } from "./harness.mjs";
import assert from "node:assert/strict";
const dataDir = process.argv[2];
const app = await launch({ dataDir });
const b = app.browser;
const go = (p) => goTo(b, p);
try {
  await waitText(b, "Rowan & Ash Hair Studio");
  await go("/tax");
  await waitText(b, "Sales can't be finalized");
  await app.shot("30-tax-unresolved");
  await clickText(b, "Look up official rate");
  await waitText(b, "Matched address", 30000);
  await app.shot("31-tax-lookup");
  await clickText(b, "Use this rate");
  await waitText(b, "Official lookup");
  // Decide taxability with a recorded basis
  await fill(b, "Salon services basis", "WA DOR: personal services are retail sales (verify for your services)");
  const rows = await b.$$("//*[@role='group'][@aria-label='Salon services']//label[normalize-space(.)='Taxable']");
  await rows[0].click();
  await (await b.$("//*[@role='group'][@aria-label='Salon services']//button[normalize-space(.)='Save']")).click();
  await waitText(b, "Salon services updated");
  await fill(b, "Retail products basis", "WA DOR: retail product sales are taxable");
  await (await b.$("//*[@role='group'][@aria-label='Retail products']//label[normalize-space(.)='Taxable']")).click();
  await (await b.$("//*[@role='group'][@aria-label='Retail products']//button[normalize-space(.)='Save']")).click();
  await waitText(b, "Retail products updated");
  await fill(b, "Tips and gratuities basis", "Voluntary tips are not part of the sale price (confirm with WA DOR)");
  await (await b.$("//*[@role='group'][@aria-label='Tips and gratuities']//label[normalize-space(.)='Not taxable']")).click();
  await (await b.$("//*[@role='group'][@aria-label='Tips and gratuities']//button[normalize-space(.)='Save']")).click();
  await waitText(b, "Tax is set up");
  await app.shot("32-tax-resolved");

  // Service estimate now includes tax
  await go("/services/1");
  await waitText(b, "Client pays");
  const t = await bodyText(b);
  assert.match(t, /Sales tax \(9\.8%/);
  await app.shot("33-service-with-tax");

  // Pricing what-if
  await go("/pricing");
  await waitText(b, "What if");
  await fill(b, "Billable utilization", "60");
  await waitText(b, "utilization changed from 80% to 60%");
  await app.shot("34-pricing-whatif");

  // Geocode the location and view providers
  await go("/settings?tab=locations");
  await waitText(b, "Locate");
  await clickText(b, "Locate");
  await waitText(b, "County 53067", 30000);
  await go("/settings?tab=providers");
  await waitText(b, "Washington Department of Revenue");
  await app.shot("35-providers");
  console.log("slice3 ok");
} catch (e) {
  await app.shot("fail");
  throw e;
} finally {
  await app.close();
  stopDisplay();
}
