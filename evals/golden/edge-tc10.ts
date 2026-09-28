#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

await runEdgeGolden(
  "tc10", "Edge — file download captured and content-asserted",
  "/download", 'a[href="download/hello.json"]',
  async (b) => {
    await b.openPage();
    await b.downloadBySelector('a[href="download/hello.json"]', "download/hello.json", "download the json fixture");
    await b.assertFileExists("download/hello.json", "file landed");
    await b.assertFileName("download/hello.json", "hello.json", "filename preserved");
    await b.assertFileContent("download/hello.json", "Hello from Playwright", "file content intact");
  },
);
