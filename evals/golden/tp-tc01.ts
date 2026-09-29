import { runEdgeGolden } from "./edge-pages-lib.ts";

// testpages.eviltester.com ajax cascade: picking a category repopulates
// the second select via XHR; submitting the pair echoes the ids. The
// backend assigns arbitrary option values per request, so waits and
// assertions key on the option *labels*, not values.
await runEdgeGolden(
  "tp-tc01",
  "cascading selects repopulate via XHR and post the picked pair",
  "https://testpages.eviltester.com/styled/basic-ajax-test.html",
  "#combo1",
  async (b) => {
    await b.openPage();
    // #combo1 exists at parse time but its change->ajax handler binds
    // late — a remote script stalls `load` indefinitely on this site, so
    // a bounded settle is the only reliable gate.
    await b.waitMs(2500, "change handler bound");
    await b.selectOption("#combo1", "Web", "pick the Web category");
    // The ajax repopulates #combo2 on change — no submit needed.
    await b.waitSelectorText(
      "#combo2",
      "Javascript",
      "languages repopulated",
    );
    await b.assertNetworkFired(
      { urlMatches: "api/ajaxselect" },
      "the ajax lookup call fired",
    );
    await b.selectOption("#combo2", "Javascript", "pick Javascript");
    await b.clickSelector(
      "input[value='Code In It']",
      "submit the pair",
    );
    await b.waitSelector("#_valuelanguage_id", "result panel rendered");
    await b.assertElementAttribute(
      "#_valuelanguage_id",
      "text",
      "matches",
      "^[0-9]+$",
      "numeric language id echoed back",
    );
  },
  { label: "tp" },
);
