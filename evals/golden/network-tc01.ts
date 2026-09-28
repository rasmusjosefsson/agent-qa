#!/usr/bin/env bun
import { existsSync, statSync } from "fs";
import { dirname, join, resolve } from "path";
import { fileURLToPath } from "url";
import { runFileUploadGolden } from "./file-upload-lib.ts";

// file:// pages cannot fetch, so the fixture is served over http by an
// in-process static server: GET resolves under evals/fixtures, non-GET gets
// 405, missing files get 404 — giving the runner 200/404/405 to assert on.
const fixturesRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../fixtures");
const server = Bun.serve({
  port: 0,
  fetch(req) {
    if (req.method !== "GET") {
      return Response.json({ error: "method not allowed" }, { status: 405 });
    }
    const rel = decodeURIComponent(new URL(req.url).pathname).replace(/^\/+/, "");
    const file = join(fixturesRoot, rel);
    if (!file.startsWith(fixturesRoot) || !existsSync(file) || !statSync(file).isFile()) {
      return Response.json({ error: "not found" }, { status: 404 });
    }
    const type = file.endsWith(".json")
      ? "application/json"
      : file.endsWith(".html")
        ? "text/html"
        : "text/plain";
    return new Response(Bun.file(file), { headers: { "content-type": type } });
  },
});

const base = `http://127.0.0.1:${server.port}`;
try {
  await runFileUploadGolden(
    "network-tc01",
    "Network claims: fired / status / responseJsonPath / silent",
    async (golden) => {
      await golden.openFixture(`${base}/network.html`, "#load");

      await golden.clickSelector("#load", "load users (GET 200)");
      await golden.waitText("#out", "users:3", "users rendered");
      await golden.assertNetworkFired(
        { urlMatches: "/api/users\\.json", method: "GET" },
        "GET users request fired",
      );
      await golden.assertNetworkStatus(
        { urlMatches: "/api/users\\.json", method: "GET" },
        "equals",
        "200",
        "GET users status 200",
      );
      await golden.assertNetworkJson(
        { urlMatches: "/api/users\\.json" },
        "$.users[0].name",
        "equals",
        "Ada",
        "first user name in the response body",
      );

      await golden.clickSelector("#post", "POST → 405");
      await golden.waitText("#out", "post:405", "405 rendered");
      await golden.assertNetworkStatus(
        { urlMatches: "/api/users\\.json", method: "POST" },
        "equals",
        "405",
        "POST users rejected with 405",
      );

      await golden.clickSelector("#missing", "load missing (404)");
      await golden.waitText("#out", "missing:404", "404 rendered");
      await golden.assertNetworkStatus(
        { urlMatches: "/api/missing\\.json" },
        "gte",
        400,
        "missing resource is a 4xx",
      );

      await golden.clickSelector("#gql", "graphql-style request");
      await golden.waitText("#out", "user:Ada Lovelace", "graphql data rendered");
      await golden.assertNetworkJson(
        { operationName: "LoadUser" },
        "$.data.user.role",
        "equals",
        "admin",
        "operationName matcher finds the graphql call",
      );

      await golden.assertNetworkSilent(
        { urlMatches: "telemetry" },
        "no telemetry request fired",
      );
    },
  );
} finally {
  server.stop();
}
