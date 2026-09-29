import { runEdgeGolden } from "./edge-pages-lib";

// Fresh site: formy-project.herokuapp.com — bootstrap modal open/close.
const BASE = "https://formy-project.herokuapp.com";

await runEdgeGolden(
  "formy-tc02",
  "bootstrap modal opens and closes",
  `${BASE}/modal`,
  "#modal-button",
  async (b) => {
    await b.openPage();
    await b.assertElementAbsent(
      ".modal.show",
      "no modal before the trigger",
    );
    await b.clickSelector("#modal-button", "open the modal");
    await b.waitSelector(".modal.show, .modal[style*='block']", "modal visible");
    await b.assertElementAttribute(
      ".modal-title",
      "text",
      "equals",
      "Modal title",
      "modal title rendered",
    );
    await b.clickSelector("#close-button", "close the modal");
    await b.waitSelectorAbsent(
      ".modal.show",
      "modal dismissed",
    );
  },
  { label: "formy" },
);
