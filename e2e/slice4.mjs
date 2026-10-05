// Slice 4: book → check out with actual usage → tip → pay → finalize → stock deducted → receipt;
// client formula; expense. Runs on the data from smoke + slice2 + slice3.
import { launch, clickText, fill, choose, waitText, stopDisplay, bodyText, go as goTo } from "./harness.mjs";
import assert from "node:assert/strict";
const dataDir = process.argv[2];
const app = await launch({ dataDir });
const b = app.browser;
const go = (p) => goTo(b, p);
try {
  await waitText(b, "Rowan & Ash Hair Studio");
  await go("/calendar");
  await waitText(b, "Drag to reschedule");
  await clickText(b, "New appointment");
  await waitText(b, "Save appointment");
  await clickText(b, "New client");
  await fill(b, "First name", "Mia");
  await fill(b, "Last name", "Ortiz");
  await clickText(b, "Add");
  await choose(b, "Appointment service 1", "Root touch-up");
  await waitText(b, "Chair time");
  await app.shot("40-appointment-modal");
  await clickText(b, "Save appointment");
  await waitText(b, "Appointment saved");
  await app.shot("41-calendar");
  await (await b.$("//*[contains(concat(' ', normalize-space(@class), ' '), ' rbc-event ')]")).click();
  await waitText(b, "Complete and check out");
  await clickText(b, "Complete and check out");
  await waitText(b, "Products used");
  await fill(b, "Actual amount of Developer 20 vol", "2.5");
  await clickText(b, "Add tip");
  await fill(b, "Tip amount", "15");
  await waitText(b, "Draft saved", 15000);
  await app.shot("42-sale-draft");
  await clickText(b, "Record payment");
  await waitText(b, "Card");
  await waitText(b, "Balance due");
  await clickText(b, "Finalize sale");
  await waitText(b, "finalized", 15000);
  const t = await bodyText(b);
  assert.match(t, /Sale S-00001/);
  assert.match(t, /Materials \$10\.88 \(planned \$10\.50\)/);
  await app.shot("43-sale-finalized");
  await clickText(b, "Receipt");
  await waitText(b, "Receipt S-00001");
  await app.shot("44-receipt");

  // Stock deducted exactly once: 30 − 2.5 = 27.5 fl oz
  await go("/inventory");
  await waitText(b, "27.5 fl oz");

  // Client formula
  await go("/clients");
  await waitText(b, "Mia");
  await clickText(b, "Mia");
  await waitText(b, "Visit history");
  await clickText(b, "New formula");
  await fill(b, "Name", "Root color");
  await fill(b, "Formula", "7N 60 g + 20 vol 75 ml, 35 min");
  await clickText(b, "Save formula");
  await waitText(b, "Version 1");
  await app.shot("45-client");

  // Expense
  await go("/expenses");
  await waitText(b, "Add expense");
  await clickText(b, "Add expense");
  await fill(b, "Amount", "1200");
  await fill(b, "Vendor", "Harbor Properties");
  await clickText(b, "Save expense");
  await waitText(b, "Harbor Properties");
  await app.shot("46-expenses");
  console.log("slice4 ok");
} catch (e) {
  await app.shot("fail");
  throw e;
} finally {
  await app.close();
  stopDisplay();
}
