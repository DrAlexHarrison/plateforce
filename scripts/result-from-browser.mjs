// The browser's answer to the one committed request.
//
// Every value comes from the request file rather than from a line written here, so this arm
// and the others cannot drift into asking different questions.
//
// The request is built by the page's own `buildRequest`, reached through the page's own
// decision model and selection records, because the fault this arm exists to catch lives in
// request construction rather than in arithmetic: a surface that tells the engine the wrong
// thing about its own rules produces correct numbers carrying a false record. An arm that
// assembled a request here would be a third construction, right by the care of whoever wrote
// it rather than by the page being asked.
//
// What this does not exercise, stated because a green here is easy to over-read: the page's
// event wiring. Nothing here clicks anything, so an answer the page can compute and has no
// control path to still passes. `scripts/check-batch.mjs` drives the rendered page and is
// where that question is asked.

import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const request = process.env.PLATEFORCE_PARITY_REQUEST;
if (!request) {
  console.error('the harness names the request file in PLATEFORCE_PARITY_REQUEST');
  process.exit(1);
}
const asked = JSON.parse(readFileSync(request, 'utf8'));

const bundle = pathToFileURL('web/pkg/plateforce_wasm.js');
let wasm;
try {
  wasm = await import(bundle.href);
} catch (error) {
  console.error(
    `the browser bundle is not built: ${error.message}\n` +
      'run scripts/build-web.sh, which is what serves the page',
  );
  process.exit(1);
}

// The page calls `init()` bare and the browser fetches the bytes beside the module. Node has
// no such fetch, so the bytes are named. That is this arm's one departure from the page.
await wasm.default({ module_or_path: readFileSync('web/pkg/plateforce_wasm_bg.wasm') });

const { state } = await import(pathToFileURL('web/state.js').href);
const { reply } = await import(pathToFileURL('web/format.js').href);
const { buildDecisionModel } = await import(pathToFileURL('web/registry.js').href);
const { resetSelections, candidateFor } = await import(pathToFileURL('web/startup.js').href);
const { buildRequest, selectionFromChosenRule, recordStated } = await import(
  pathToFileURL('web/analysis.js').href
);

// What `start()` puts in the tab before a reader touches anything, minus the drawing. A
// literal decision model here would be a second copy of the one the page ranks against.
state.build = JSON.parse(wasm.buildInfoJson());
state.registry = JSON.parse(wasm.registryJson());
state.slots = buildDecisionModel(state.registry, state.build, state.path);
resetSelections();

// Naming a rule, then typing its values: the two acts the page records separately, because a
// value the registry filled in and a value the reader typed move the number identically.
for (const key of ['weighing', 'onset', 'takeoff']) {
  const wanted = asked[key];
  const slot = state.slots.find((entry) => entry.key === key);
  if (!slot) {
    console.error(`the page offers no slot named ${key}, so it cannot be asked this request`);
    process.exit(1);
  }
  const candidate = candidateFor(key, wanted.method_id);
  if (!candidate) {
    console.error(`the page offers no rule ${wanted.method_id} for ${key}`);
    process.exit(1);
  }
  const selection = selectionFromChosenRule(candidate, slot.forcesDecision);
  for (const [name, value] of Object.entries(wanted.parameters)) {
    selection.values[name] = value;
    recordStated(selection, name);
  }
  state.selection[key] = selection;
}

const file = wasm.ForceFile.parse(readFileSync(asked.trial, 'utf8'));
// The page sniffs the separator while parsing rather than being told it, so the request's
// delimiter is checked against what the reader found instead of being passed and forgotten.
const summary = JSON.parse(file.summaryJson());
if (summary.columns.length <= asked.force_column) {
  console.error(
    `the request reads column ${asked.force_column} and the page found ${summary.columns.length}`,
  );
  process.exit(1);
}
const trial = wasm.LoadedTrial.fromForceFile(
  file,
  asked.force_column,
  asked.sample_rate_hz,
  asked.sentinel_convention,
);

const json = trial.analyse(JSON.stringify(buildRequest()), asked.trial);
// Read through the page's own reader. A surface that parsed past this envelope would find
// every field undefined and report that as an answer.
const answer = reply(json);
if (!answer.ok) {
  console.error(`the browser declined this request: ${JSON.stringify(answer.refusal)}`);
  process.exit(1);
}
// The engine's own bytes, not a document rebuilt from what was read out of them.
process.stdout.write(json);
