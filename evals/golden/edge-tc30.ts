import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: the site's forgot-password POST is known-broken — it serves a
// 500 inline. The golden pins reality instead of a fantasy happy path:
// the POST request 500s in the network log and the error page renders.
await runEdgeGolden(
  "tc30",
  "forgot-password POST pins the site's known 500",
  "/forgot_password",
  "input#email",
  async (b) => {
    await b.openPage();
    await b.fillSelector("#email", "qa@example.com", "email");
    await b.clickSelector("#form_submit", "retrieve password");
    await b.waitSelector("h1", "response page rendered");
    await b.assertElementAttribute(
      "body",
      "text",
      "contains",
      "Internal Server Error",
      "the site's known 500 body",
    );
    await b.assertNetworkStatus(
      { urlMatches: "forgot_password", method: "POST" },
      "equals",
      "500",
      "the POST itself 500'd",
    );
  },
);
