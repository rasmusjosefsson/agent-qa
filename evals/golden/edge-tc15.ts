import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: notification message — /notification_message 303s straight to
// the rendered page; each click of the 'Click here' link re-rolls the flash
// banner ('Action successful' vs 'Action unsuccesful…'), so assert the
// stable 'Action' prefix only.
await runEdgeGolden(
  "tc15",
  "click-through lands on a rendered page with a flash banner",
  "/notification_message",
  "a[href='/notification_message']",
  async (b) => {
    await b.openPage();
    await b.clickSelector("a[href='/notification_message']", "follow the notification link");
    await b.waitSelector("#flash", "flash banner rendered after redirect");
    await b.assertElementAttribute("#flash", "text", "contains", "Action", "flash carries the action outcome");
  },
);
