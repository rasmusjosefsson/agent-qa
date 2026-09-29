import { runEdgeGolden } from "./edge-pages-lib";

// Toggle + hash-route filters + clear-completed: list re-renders conditionally
// per filter (the completed item leaves the DOM on the Active view).
await runEdgeGolden(
  "todo02",
  "todomvc: toggle complete, All/Active/Completed filters, clear completed",
  "https://todomvc.com/examples/typescript-react/",
  "input.new-todo",
  async (b) => {
    await b.openPage();
    await b.fillSelector(".new-todo", "alpha", "type first todo");
    await b.pressOn(".new-todo", "Enter", "commit first todo");
    await b.fillSelector(".new-todo", "beta", "type second todo");
    await b.pressOn(".new-todo", "Enter", "commit second todo");
    await b.clickSelector(".todo-list li:nth-of-type(1) .toggle", "complete the first todo");
    await b.assertElementAttribute(
      ".todo-list li:nth-of-type(1)",
      "class",
      "contains",
      "completed",
      "first todo marked completed",
    );
    await b.waitSelectorText(".todo-count", "1 item left", "counter drops to 1");
    await b.clickSelector('.filters a[href="#/active"]', "filter to active");
    await b.assertUrlContains("#/active", "hash route is /active");
    await b.assertElementAbsent(".todo-list li:nth-of-type(2)", "completed todo filtered out");
    await b.assertElementText(".todo-list li:nth-of-type(1) label", "beta", "only the active todo shows");
    await b.clickSelector('.filters a[href="#/completed"]', "filter to completed");
    await b.assertElementText(".todo-list li:nth-of-type(1) label", "alpha", "only the completed todo shows");
    await b.clickSelector('.filters a[href="#/"]', "back to all");
    await b.assertElementPresent(".todo-list li:nth-of-type(2)", "both todos back");
    await b.clickSelector(".clear-completed", "clear completed");
    await b.assertElementAbsent(".todo-list li:nth-of-type(2)", "completed todo cleared");
    await b.assertElementText(".todo-list li:nth-of-type(1) label", "beta", "active todo survives");
  },
  { label: "todomvc" },
);
