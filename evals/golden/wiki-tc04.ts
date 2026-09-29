import { runEdgeGolden } from "./edge-pages-lib";

// Fresh site: en.wikipedia.org — the public REST summary endpoint answers
// JSON, claimable via the network subject + the rendered body text.
const BASE = "https://en.wikipedia.org";

await runEdgeGolden(
  "wiki-tc04",
  "REST summary endpoint returns JSON over the wire",
  `${BASE}/api/rest_v1/page/summary/Terraform`,
  "body",
  async (b) => {
    await b.openPage();
    await b.assertElementAttribute(
      "body",
      "text",
      "contains",
      "\"extract\"",
      "JSON payload carries the extract field",
    );
    await b.assertNetworkStatus(
      { urlMatches: "api/rest_v1/page/summary/Terraform", method: "GET" },
      "equals",
      "200",
      "REST endpoint answered 200",
    );
    await b.assertNetworkStatus(
      { urlMatches: "api/rest_v1/page/summary/Terraform" },
      "equals",
      "200",
      "matched on url alone too",
    );
  },
  { label: "wiki" },
);
