#!/usr/bin/env bun
import { runFileUploadGolden } from "./file-upload-lib.ts";

await runFileUploadGolden("upload-tc02", "Upload TC02 selected file name is displayed after selection", async (golden) => {
  await golden.openPage();
  await golden.upload("#file-upload", "upload-valid.txt", "select upload-valid.txt");
  await golden.assertLiveCondition(
    `(() => { const input = document.querySelector('#file-upload'); const names = Array.from(input?.files || []).map((file) => file.name); if (!names.includes('upload-valid.txt')) throw new Error('upload-valid.txt not selected'); if (!document.body.innerText.includes('upload-valid.txt')) throw new Error('upload-valid.txt not displayed'); return true; })()`,
    "upload-valid.txt selected and displayed",
  );
  await golden.waitSelectorText("body", "upload-valid.txt", "selected file name is displayed");
});
