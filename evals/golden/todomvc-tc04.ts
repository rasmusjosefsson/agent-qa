import { runEdgeGolden } from "./edge-pages-lib";

// toggle-all + reload persistence + hover-reveal destroy: state survives a
// full page reload via localStorage, and the per-row delete button only
// becomes clickable after a real hover (CSS :hover reveal).
await runEdgeGolden(
  "todo04",
  "todomvc: toggle-all, persistence across reload, hover destroy, empty state",
  "https://todomvc.com/examples/typescript-react/",
  "input.new-todo",
  async (b) => {
    await b.openPage();
    await b.fillSelector(".new-todo", "first", "type first todo");
    await b.pressOn(".new-todo", "Enter", "commit first todo");
    await b.fillSelector(".new-todo", "second", "type second todo");
    await b.pressOn(".new-todo", "Enter", "commit second todo");
    await b.clickSelector(".toggle-all", "mark all complete");
    await b.waitSelectorText(".todo-count", "0 items left", "counter reads 0 left");
    await b.reload("reload the page");
    await b.waitSelector(".todo-list li:nth-of-type(2)", "todos survive the reload");
    await b.assertElementAttribute(
      ".todo-list li:nth-of-type(1)",
      "class",
      "contains",
      "completed",
      "completed state persists across reload",
    );
    await b.hoverSelector(".todo-list li:nth-of-type(1)", "hover to reveal the delete button");
    await b.clickSelector(".todo-list li:nth-of-type(1) button.destroy", "delete the first todo");
    await b.assertElementAbsent(".todo-list li:nth-of-type(2)", "one todo remains");
    await b.clickSelector(".clear-completed", "clear the last completed todo");
    await b.assertElementAbsent(".todo-list li", "list is empty");
    await b.assertElementAbsent(".footer", "footer unmounts when no todos remain");
  },
  { label: "todomvc" },
);
