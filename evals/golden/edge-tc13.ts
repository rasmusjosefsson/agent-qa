import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: entry ad modal — a foundation reveal-modal that opens on load
// and must be dismissed through its footer action before the page is usable.
await runEdgeGolden(
  "tc13",
  "entry ad modal opens over the page and closes via its footer",
  "/entry_ad",
  "#modal",
  async (b) => {
    await b.openPage();
    await b.clickSelector(".modal-footer p", "close the modal");
    await b.assertElementAttribute("#modal", "style", "contains", "display: none", "modal hidden after close");
  },
);
