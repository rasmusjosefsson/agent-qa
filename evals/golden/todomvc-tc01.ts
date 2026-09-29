import { runEdgeGolden } from "./edge-pages-lib";

// New site: todomvc typescript-react — a stateful SPA backed by localStorage
// (`react-todos`). Covers fill + Enter-to-commit, the item counter, dense-list
// bounds, and the storage claim subject.
await runEdgeGolden(
  "todo01",
  "todomvc: add todos, item counter, and localStorage persistence key",
  "https://todomvc.com/examples/typescript-react/",
  "input.new-todo",
  async (b) => {
    await b.openPage();
    await b.fillSelector(".new-todo", "buy milk", "type first todo");
    await b.pressOn(".new-todo", "Enter", "commit first todo");
    await b.fillSelector(".new-todo", "write tests", "type second todo");
    await b.pressOn(".new-todo", "Enter", "commit second todo");
    await b.fillSelector(".new-todo", "ship it", "type third todo");
    await b.pressOn(".new-todo", "Enter", "commit third todo");
    await b.waitSelectorText(".todo-count", "3 items left", "counter reads 3 items left");
    await b.assertElementPresent(".todo-list li:nth-of-type(3)", "three todos rendered");
    await b.assertElementAbsent(".todo-list li:nth-of-type(4)", "no phantom fourth todo");
    await b.assertStorage("react-todos", true, "todos persisted to localStorage");
  },
  { label: "todomvc" },
);
