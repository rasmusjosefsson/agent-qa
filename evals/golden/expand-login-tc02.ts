import { runAuthoredGolden } from "./edge-pages-lib";

// expandtesting login — failure path: wrong password stays on /login and
// shows the invalid-credentials flash.
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

await runAuthoredGolden(
  "expand-login-tc02",
  "expandtesting: invalid password stays on /login with a flash error",
  {
    steps: [
      step("s0", "login page", { verb: "goto", value: lit("https://practice.expandtesting.com/login") }),
      step("s1", "username", { verb: "type", on: css("#username"), value: lit("practice") }),
      step("s2", "bad password", { verb: "type", on: css("#password"), value: lit("wrong") }),
      step("s3", "submit", { verb: "click", on: css("button[type='submit']") }),
      {
        id: "s4",
        intent: "did NOT navigate to /secure",
        kind: "check",
        claim: { subject: { url: true }, predicate: "contains", value: "/login" },
      },
      {
        id: "s5",
        intent: "error banner shown",
        kind: "check",
        claim: {
          subject: { element: css("#flash"), attribute: "text" },
          predicate: "contains",
          value: "password is invalid",
        },
      },
    ],
  },
  {},
);
