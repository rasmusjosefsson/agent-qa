#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

await runEdgeGolden(
  "tc11", "Edge — file upload round-trip",
  "/upload", "#file-upload",
  async (b) => {
    await b.openPage();
    await b.upload("#file-upload", "evals/fixtures/upload-valid.txt", "attach the fixture file");
    await b.clickSelector("#file-submit", "submit the upload");
    await b.assertUrlContains("/upload", "stayed on the upload flow");
    await b.assertElementText("#uploaded-files", "upload-valid.txt", "server echoes the filename");
  },
);
