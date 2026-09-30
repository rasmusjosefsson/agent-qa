import { runAuthoredGolden } from "./edge-pages-lib";

// expandtesting login — happy path round-trip: credential form → /secure
// with a flash banner → logout lands back on /login.
const css = (value: string) => ({
  raw: { kind: "css", value },
  reason: "authored expandtesting golden",
});
const lit = (literal: unknown) => ({ from: "literal", literal });
const step = (id: string, intent: string, extra: Record<string, unknown>) => ({
  id,
  intent,
  kind: "do",
  ...extra,
});
const textIs = (selector: string, predicate: string, value: string) => ({
  subject: { element: css(selector), attribute: "text" },
  predicate,
  value,
});

await runAuthoredGolden(
  "expand-login-tc01",
  "expandtesting: login happy path + logout",
  {
    steps: [
      step("s0", "login page", { verb: "goto", value: lit("https://practice.expandtesting.com/login") }),
      {
        id: "s1",
        intent: "form rendered",
        kind: "check",
        claim: { subject: { element: css("input#username") }, predicate: "isVisible" },
      },
      step("s2", "username", { verb: "type", on: css("#username"), value: lit("practice") }),
      step("s3", "password", { verb: "type", on: css("#password"), value: lit("SuperSecretPassword!") }),
      step("s4", "submit", { verb: "click", on: css("button[type='submit']") }),
      {
        id: "s5",
        intent: "landed on the secure area",
        kind: "check",
        claim: { subject: { url: true }, predicate: "contains", value: "/secure" },
      },
      {
        id: "s6",
        intent: "success banner shown",
        kind: "check",
        claim: textIs("#flash", "contains", "logged into"),
      },
      step("s7", "log out", { verb: "click", on: css("a.button.secondary.radius, a[href='/logout']") }),
      {
        id: "s8",
        intent: "back on the login page",
        kind: "check",
        claim: { subject: { url: true }, predicate: "contains", value: "/login" },
      },
      {
        id: "s9",
        intent: "logout banner shown",
        kind: "check",
        claim: textIs("#flash", "contains", "logged out"),
      },
    ],
  },
  {},
);
