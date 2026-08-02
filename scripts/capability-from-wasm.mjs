// What the browser's module says it can do, asked of the built bundle rather than of the
// source it was built from.
//
// The bundle is what a reader loads, and it can be older than the crate beside it, which is
// the drift a manifest comparing source to source cannot see. Nothing here reads a digest:
// one commit produces a different wasm digest on every machine that builds it, so a digest
// would make this a comparison that can only fail.

import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const bundle = join(root, "web", "pkg");

const module = await import(join(bundle, "plateforce_wasm.js")).catch((error) => {
  process.stderr.write(
    `the browser's bundle is not built at web/pkg: ${error.message}\n` +
      "build it with scripts/build-web.sh release\n",
  );
  process.exit(1);
});

// `--target web` resolves its own wasm by fetch, which node has no page to fetch from, so
// the bytes are handed over directly.
await module.default({
  module_or_path: await readFile(join(bundle, "plateforce_wasm_bg.wasm")),
});

// Every surface's answer reaches the comparison inside the same envelope. This module
// signals a refusal by throwing rather than by returning one, which is how a JavaScript
// caller expects to be told, so the envelope is put on here rather than asked of it. What
// the surface can do is untouched; only how it is carried.
process.stdout.write(JSON.stringify({ ok: JSON.parse(module.capabilityJson()) }));
