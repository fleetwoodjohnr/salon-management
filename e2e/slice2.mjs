// Slice 2: products, receiving (landed cost), movements, service recipe + live estimate.
// Run after smoke.mjs on the same data dir (also proves persistence across restarts).
import { launch, clickText, fill, choose, waitText, stopDisplay, bodyText, go as goTo } from "./harness.mjs";
import assert from "node:assert/strict";
const dataDir = process.argv[2];
const app = await launch({ dataDir });
const b = app.browser;
const go = (p) => goTo(b, p);
try {
  await waitText(b, "Rowan & Ash Hair Studio"); // reopened the last workspace after restart
  // Product 1: developer measured in fl oz
  await go("/inventory/products/new");
  await waitText(b, "Measuring");
  await fill(b, "Name", "Developer 20 vol");
  await fill(b, "Brand", "Lumen");
  await choose(b, "Stock unit", "US fluid ounces (fl oz)");
  await fill(b, "Reorder point", "40");
  await clickText(b, "Save product");
  await waitText(b, "Saved Developer 20 vol");
  // Product 2: color tubes by weight with a custom unit
  await go("/inventory/products/new");
  await waitText(b, "Measuring");
  await fill(b, "Name", "Permanent color 7N");
  await fill(b, "Brand", "Lumen");
  await choose(b, "Stock unit", "grams (g)");
  await clickText(b, "Add custom unit");
  await fill(b, "Custom unit name", "tube");
  await fill(b, "Custom unit size", "60");
  await clickText(b, "Save product");
  await waitText(b, "Saved Permanent color 7N");
  await app.shot("20-product-editor");

  // Receive: one 32 fl oz bottle at $24 (acceptance) + 3 tubes, with shipping/discount
  await go("/inventory/receive");
  await waitText(b, "Invoice extras");
  await choose(b, "Line 1 product", "Lumen Developer 20 vol");
  await fill(b, "Line 1 contents per package", "32");
  await fill(b, "Line 1 price", "24");
  await clickText(b, "Add line");
  await choose(b, "Line 2 product", "Lumen Permanent color 7N");
  await fill(b, "Line 2 packages", "3");
  await fill(b, "Line 2 contents per package", "1");
  await choose(b, "Line 2 unit", "tube (60 g)");
  await fill(b, "Line 2 price", "27");
  await waitText(b, "$0.75");
  await app.shot("21-receive");
  await clickText(b, "Receive stock");
  await waitText(b, "Opening stock", 15000); // purchases tab
  await go("/inventory");
  await waitText(b, "Developer 20 vol");
  const t = await bodyText(b);
  assert.match(t, /32 fl oz/);
  assert.match(t, /\$0\.75 \/ fl oz/);
  assert.match(t, /180 g/);
  await app.shot("22-products");

  // Waste 2 fl oz from the developer → 30 fl oz left, $22.50 value
  await clickText(b, "Developer 20 vol");
  await waitText(b, "on hand");
  await clickText(b, "Stock");
  await clickText(b, "Record waste");
  await fill(b, "Quantity", "2");
  await fill(b, "Note", "Spilled");
  await (await b.$("//form//button[@type='submit']")).click();
  await waitText(b, "30 fl oz");
  await waitText(b, "$22.50");
  await app.shot("23-product-after-waste");

  // Service with recipe and a variant
  await go("/services/new");
  await waitText(b, "Recipe");
  await fill(b, "Service name", "Root touch-up");
  await fill(b, "Category", "Color");
  await clickText(b, "Add product");
  await choose(b, "Recipe line 1 product", "Lumen Developer 20 vol");
  await fill(b, "Recipe line 1 amount", "2");
  await clickText(b, "Add product");
  await choose(b, "Recipe line 2 product", "Lumen Permanent color 7N");
  await fill(b, "Recipe line 2 amount", "1");
  await choose(b, "Recipe line 2 unit", "tube (60 g)");
  await fill(b, "Processing", "35");
  await fill(b, "Waste allowance", "10");
  await clickText(b, "Add variant");
  await fill(b, "Variant 1 option", "Long");
  await fill(b, "Variant 1 extra hands-on minutes", "15");
  await fill(b, "Variant 1 material factor", "1.5");
  await fill(b, "Variant 1 price adjustment", "20");
  await waitText(b, "Cost before fees");
  await waitText(b, "$10.50"); // materials: $1.50 developer + $9 color
  await fill(b, "Your price", "60");
  await waitText(b, "below cost");
  await app.shot("24-service-below-target");
  await clickText(b, "Use");
  await waitText(b, "Saved", 3000).catch(() => {});
  await clickText(b, "Save service");
  await waitText(b, "recipe version 1");
  await app.shot("25-service-saved");
  await go("/services");
  await waitText(b, "Root touch-up");
  await app.shot("26-services-list");
  console.log("slice2 ok");
} catch (e) {
  await app.shot("fail");
  throw e;
} finally {
  await app.close();
  stopDisplay();
}
