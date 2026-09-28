#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

await runEdgeGolden(
  "tc12", "Edge — range slider driven by arrow keys",
  "/horizontal_slider", ".sliderContainer input",
  async (b) => {
    await b.openPage();
    await b.pressOn(".sliderContainer input", "ArrowRight", "nudge the slider right");
    await b.assertElementText("#range", "0.5", "slider value echoed");
  },
);
