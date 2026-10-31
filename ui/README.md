# ui

The TypeScript user interface: CodeMirror editor, three.js toolpath view,
the diagnostics panel and the statistics dialog. Plain TypeScript (no framework),
built with Vite into `dist/`, which the desktop app embeds. It contains no
G-code meaning: it shows the render buffer the core produced.

```bash
npm ci
npm run lint && npm run typecheck && npm test
npm run build
```

`src/generated/` is written by `cargo run -p gen-ts`; never edit it by
hand. The module layout is in `ARCHITECTURE.md` §8.6.
