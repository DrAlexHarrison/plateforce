// What the browser's module says it can do, asked of the built bundle rather than of the
// source it was built from.
//
// The bundle is what a reader loads, and it can be older than the crate beside it, which is
// the drift a manifest comparing source to source cannot see. Nothing here reads a digest:
// one commit produces a different wasm digest on every machine that builds it, so a digest
// would make this a comparison that can only fail.

import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const bundle = join(root, "web", "pkg");

const module = await import(pathToFileURL(join(bundle, "plateforce_wasm.js"))).catch(
  (error) => {
    process.stderr.write(
      `the browser's bundle is not built at web/pkg: ${error.message}\n` +
        "build it with scripts/build-web.sh release\n",
    );
    process.exit(1);
  },
);

// `--target web` resolves its own wasm by fetch, which node has no page to fetch from, so
// the bytes are handed over directly.
await module.default({
  module_or_path: await readFile(join(bundle, "plateforce_wasm_bg.wasm")),
});

// The surface's own bytes, forwarded rather than reshaped. A harness that puts the envelope
// on here makes the one gate that exists to compare the surfaces the reason they agree.
process.stdout.write(module.capabilityJson());
