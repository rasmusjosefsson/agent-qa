import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: add/remove elements — each click on "Add Element" appends a
// Delete button; clicking one removes just that row.
await runEdgeGolden(
  "tc21",
  "add element inserts a Delete button; Delete removes it",
  "/add_remove_elements/",
  "button[onclick='addElement()']",
  async (b) => {
    await b.openPage();
    await b.assertElementAbsent(".added-manually", "no Delete buttons initially");
    await b.clickSelector("button[onclick='addElement()']", "add first element");
    await b.clickSelector("button[onclick='addElement()']", "add second element");
    await b.waitSelector(".added-manually", "Delete buttons rendered");
    await b.clickSelector(".added-manually", "delete the first");
    await b.assertElementPresent(".added-manually", "second Delete remains");
    await b.clickSelector(".added-manually", "delete the second");
    await b.assertElementAbsent(".added-manually", "all Deletes removed");
  },
);
