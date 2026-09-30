#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// letcode.in /file — upload a fixture into #resume (the input's `value`
// reads back the fakepath+name), then exercise the `download` verb on the
// sample text file with file-exists + file-content claims.
await runEdgeGolden(
  "tc04",
  "letcode file — upload fixture + download claim",
  "https://letcode.in/file",
  "a[href*='sample.txt']",
  async (b) => {
    await b.openPage();
    // The file page is the most ad-walled page on letcode — the
    // fundingchoices overlay mounts mid-session. Dismiss it once and the
    // replay keeps re-removing it before each interactive step.
    await b.dismissBySelector(".fc-monetization-dialog-container, .fc-dialog-overlay, .fc-message-root", "dismiss the consent wall if mounted");
    await b.upload("input#resume", "evals/fixtures/upload-valid.txt", "upload the txt fixture");
    await b.assertElementAttribute("input#resume", "value", "contains", "upload-valid.txt", "file input holds the chosen file");
    await b.downloadBySelector("a[href*='sample.txt']", "sample.txt", "download the sample text file");
    await b.assertFileExists("sample.txt", "download landed on disk");
    await b.assertFileContent("sample.txt", "Lorem ipsum", "downloaded content is intact");
    await b.assertPageError(true, "notExists", undefined, "page raised no uncaught exceptions");
  },
);
