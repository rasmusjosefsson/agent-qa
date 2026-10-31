import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: forgot-password used to 500 inline; the site now POSTs and
// lands on /email_sent with "Your e-mail's been sent!" (no <h1>). Pin the
// current path — body text + email_sent URL + 200 on the destination.
await runEdgeGolden(
  "tc30",
  "forgot-password POST lands on email_sent confirmation",
  "/forgot_password",
  "input#email",
  async (b) => {
    await b.openPage();
    await b.fillSelector("#email", "qa@example.com", "email");
    await b.clickSelector("#form_submit", "retrieve password");
    await b.waitSelectorText("body", "e-mail's been sent", "confirmation rendered");
    await b.assertElementAttribute(
      "body",
      "text",
      "contains",
      "e-mail's been sent",
      "confirmation body shown",
    );
    await b.assertUrlContains("email_sent", "redirected to email_sent");
    await b.assertNetworkStatus(
      { urlMatches: "the-internet\\.herokuapp\\.com/email_sent$" },
      "equals",
      "200",
      "email_sent answered 200",
    );
  },
);
