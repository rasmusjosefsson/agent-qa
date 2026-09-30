# agent-qa showreel

A 30-second, 1920×1080 @ 30fps motion-graphics explainer for agent-qa, built with
[Remotion](https://www.remotion.dev/).

```bash
cd showreel
npm install
npm run studio   # live preview / scrubbing
npm run render   # -> out/agent-qa-showreel.mp4
```

Storyboard: hook → broken-selector problem → logo reveal → 01 Record →
02 Replay → 03 Auto-heal (fallback ladder) → 04 Compare → install CTA.
Scene lengths live in `SCENES` in `src/Showreel.tsx`; each scene is a
self-contained component in `src/scenes/`.
