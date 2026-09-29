import { runEdgeGolden } from "./edge-pages-lib";

// dblclick-to-edit: double-clicking a label swaps the row to .editing with a
// focused .edit input; Enter commits the rename and the row leaves edit mode.
await runEdgeGolden(
  "todo03",
  "todomvc: double-click a todo to rename it inline",
  "https://todomvc.com/examples/typescript-react/",
  "input.new-todo",
  async (b) => {
    await b.openPage();
    await b.fillSelector(".new-todo", "old name", "type the todo");
    await b.pressOn(".new-todo", "Enter", "commit the todo");
    await b.dblclickSelector(".todo-list li:nth-of-type(1) label", "double-click to edit");
    await b.waitSelector(".todo-list li.editing", "row enters edit mode");
    await b.fillSelector(".todo-list li.editing .edit", "renamed task", "type the new name");
    await b.pressOn(".todo-list li.editing .edit", "Enter", "commit the rename");
    await b.waitSelectorAbsent(".todo-list li.editing", "row leaves edit mode");
    await b.assertElementText(".todo-list li:nth-of-type(1) label", "renamed task", "label updated");
  },
  { label: "todomvc" },
);
