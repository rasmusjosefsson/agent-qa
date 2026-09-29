import { runEdgeGolden } from "./edge-pages-lib";

// quotes.toscrape.com/login — the practice login accepts any credentials,
// POSTs, 302s home, and drops a session cookie. Covers form POST + redirect
// navigation + cookie claims on a real site.
await runEdgeGolden(
  "tc05",
  "login form POST → redirect home → session cookie",
  "https://quotes.toscrape.com/login",
  "#username",
  async (b) => {
    await b.openPage();
    await b.fillSelector("#username", "qa-user", "username");
    await b.fillSelector("#password", "qa-pass", "password");
    await b.clickSelector("input[type='submit']", "submit login");
    await b.waitSelector("a[href='/logout']", "logout link appeared");
    await b.assertCookie("session", true, "session cookie set after login");
    await b.assertUrlContains("quotes.toscrape.com/", "landed back on the site");
    await b.assertNetworkFired(
      { urlMatches: "/login$", method: "POST" },
      "login POST fired",
    );
    // The 302 itself is invisible: redirect responses on navigations don't
    // reach the capture. The redirected GET proves the round-trip landed.
    await b.assertNetworkStatus(
      { urlMatches: "quotes.toscrape.com/$", method: "GET" },
      "equals",
      "200",
      "post-login home document is a 200",
    );
  },
  { label: "quotes" },
);
