#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

await runEdgeGolden(
  "tc07", "Edge — native JS prompt: type into the dialog",
  "/javascript_alerts", 'button[onclick="jsPrompt()"]',
  async (b) => {
    await b.openPage();
    await b.clickSelector('button[onclick="jsPrompt()"]', "open the JS prompt");
    await b.assertDialogText("I am a JS prompt", "prompt message readable while pending");
    await b.dialogAccept("answer the prompt", "edge sweep");
    await b.assertDialogClosed("prompt is dismissed");
    await b.assertElementAttribute("#result", "text", "contains", "edge sweep", "page echoes the prompt text");
  },
  { keepDialogs: true },
);
