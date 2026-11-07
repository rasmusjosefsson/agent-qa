#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

await runEdgeGolden(
  "tc10", "Edge — file download captured and content-asserted",
  "/download", 'a[href="download/some-file.txt"]',
  async (b) => {
    await b.openPage();
    // hello.json was removed from the public download directory (404).
    // some-file.txt is the original fixture that is still served.
    await b.downloadBySelector('a[href="download/some-file.txt"]', "download/some-file.txt", "download the text fixture");
    await b.assertFileExists("download/some-file.txt", "file landed");
    await b.assertFileName("download/some-file.txt", "some-file.txt", "filename preserved");
    await b.assertFileContent("download/some-file.txt", "asdf", "file content intact");
  },
);
