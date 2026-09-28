import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: notification messages — "Click here" 303s through
// /notification_message and re-renders a random "successful"/"unsuccesful"
// flash. The page opened directly has no flash at all; pin the
// deterministic prefix ("Action"), never the random outcome.
await runEdgeGolden(
  "tc33",
  "notification flash renders an Action message, twice",
  "/notification_message_rendered",
  "a[href='/notification_message']",
  async (b) => {
    await b.openPage();
    await b.clickSelector("a[href='/notification_message']", "first trigger");
    await b.waitSelector("#flash", "first flash rendered");
    await b.assertElementAttribute(
      "#flash",
      "text",
      "contains",
      "Action",
      "first flash carries an Action message",
    );
    await b.clickSelector("a[href='/notification_message']", "re-trigger");
    await b.waitSelector("#flash", "second flash rendered");
    await b.assertElementAttribute(
      "#flash",
      "text",
      "contains",
      "Action",
      "second flash also carries an Action message",
    );
  },
);
