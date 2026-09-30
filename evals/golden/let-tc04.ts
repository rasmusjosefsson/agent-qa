import { runEdgeGolden } from "./edge-pages-lib";

// letcode.in /radio — radio groups, checked-state claims, a disabled option,
// and the page's "Remember me" checkbox.
await runEdgeGolden(
  "let-tc04",
  "letcode radio page — radio groups and checked state",
  "https://letcode.in/radio",
  "input#yes",
  async (b) => {
    await b.openPage();
    await b.checkSelector("input#yes", "answer yes");
    await b.assertElementAttribute(
      "input#yes",
      "checked",
      "equals",
      "true",
      "yes checked",
    );
    await b.assertElementAttribute(
      "input#no",
      "checked",
      "equals",
      "false",
      "no unchecked",
    );

    await b.checkSelector("input#no", "switch to no");
    await b.assertElementAttribute(
      "input#no",
      "checked",
      "equals",
      "true",
      "no checked",
    );
    await b.assertElementAttribute(
      "input#yes",
      "checked",
      "equals",
      "false",
      "yes flipped off",
    );

    await b.checkSelector("input#foo", "pick foo in the foobar group");
    await b.assertElementAttribute(
      "input#foo",
      "checked",
      "equals",
      "true",
      "foo checked",
    );
    await b.assertElementAttribute(
      "input#notfoo",
      "checked",
      "equals",
      "false",
      "notfoo unchecked",
    );

    await b.assertElementAttribute(
      "input#maybe",
      "disabled",
      "equals",
      "true",
      "maybe is disabled",
    );
  },
  { label: "let" },
);
