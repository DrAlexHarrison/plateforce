// The browser's answer to one committed request, analysed or swept.
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
const { buildDecisionModel, availableAxes } = await import(pathToFileURL('web/registry.js').href);
const { resetSelections, candidateFor } = await import(pathToFileURL('web/startup.js').href);
const { buildRequest, selectionFromChosenRule, recordStated } = await import(
  pathToFileURL('web/analysis.js').href
);
const { savePlate, captureJson } = await import(pathToFileURL('web/plate.js').href);

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
  // The act the rail records when a reader opens the list and picks a rule, which is what
  // the request names for these three. Without it the arm sends a rule the page arrived at,
  // and asks a different question from the one the other three surfaces are asked.
  selection.methodStated = true;
  for (const [name, value] of Object.entries(wanted.parameters)) {
    selection.values[name] = value;
    recordStated(selection, name);
  }
  state.selection[key] = selection;
}

// Rules computed from the landmarks, reached the way the picker reaches them: the construct
// goes on the path, the model is rebuilt around it, and the rule is chosen off the rail. A
// selection written straight into the map would skip the ranking the page does and could bind
// a rule the page never offers.
const asked_derived = asked.derived ?? {};
if (Object.keys(asked_derived).length) {
  state.path.push(...Object.keys(asked_derived));
  state.slots = buildDecisionModel(state.registry, state.build, state.path);
  for (const [construct, wanted] of Object.entries(asked_derived)) {
    const slot = state.slots.find((entry) => entry.key === construct);
    const candidate = slot && candidateFor(construct, wanted.method_id);
    if (!candidate) {
      console.error(`the page offers no rule ${wanted.method_id} for ${construct}`);
      process.exit(1);
    }
    const selection = selectionFromChosenRule(candidate, slot.forcesDecision);
    selection.methodStated = true;
    for (const [name, value] of Object.entries(wanted.parameters ?? {})) {
      selection.values[name] = value;
      recordStated(selection, name);
    }
    for (const [name, value] of Object.entries(wanted.options ?? {})) {
      selection.options[name] = value;
      recordStated(selection, name);
    }
    selection.unresolved = [];
    state.selection[construct] = selection;
  }
}

// What the reader answered about the plate, through the two acts the page keeps apart: a
// saved plate picked in the drawer, and members typed over it on this capture. The capture
// the engine is handed is then the page's own `captureJson`, for the reason the request is
// the page's own `buildRequest`: a capture assembled here would be a second construction,
// right by the care of whoever wrote this file rather than by the page being asked.
if (asked.capture) {
  if (asked.capture.plate) {
    savePlate(asked.capture.plate.name, asked.capture.plate.members);
    state.plate.picked = asked.capture.plate.name;
  }
  Object.assign(state.plate.stated, asked.capture.acquisition ?? {});
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

// A request carrying a `sweep` block asks how far the number moves, and one without it asks
// what the analysis reports. The terminal's arm and Python's make the same test in the same
// words.
//
// The axes come from `availableAxes`, which is what the panel's own tick list is drawn from,
// so this arm sweeps the rules the page offers a reader rather than a set assembled here.
// The panel's axis list also carries a dimension per published parameter and one for
// gravity; the request names the slots whose rule is varied, and those are what the terminal
// varies too.
const json = asked.sweep
  ? trial.spread(
      JSON.stringify({
        base: buildRequest(),
        axes: asked.sweep.slots.map((named) => {
          const slot = state.slots.find((entry) => entry.key === named);
          if (!slot) {
            console.error(`the page offers no slot named ${named}, so it cannot sweep it`);
            process.exit(1);
          }
          const candidate = candidateFor(named, state.selection[named]?.methodId);
          const axis = availableAxes(slot, candidate).find((entry) => entry.methodIds?.length);
          if (!axis) {
            console.error(`the page offers one rule for ${named}, so it varies nothing there`);
            process.exit(1);
          }
          return { slot: axis.slot, parameter: null, values: [], method_ids: axis.methodIds };
        }),
        quantity_key: asked.sweep.quantity_key,
        maximum_combinations: asked.sweep.maximum_combinations,
      }),
    )
  : trial.analyse(JSON.stringify(buildRequest()), asked.trial, captureJson());
// Read through the page's own reader. A surface that parsed past this envelope would find
// every field undefined and report that as an answer.
const answer = reply(json);
if (!answer.ok) {
  console.error(`the browser declined this request: ${JSON.stringify(answer.refusal)}`);
  process.exit(1);
}
// The engine's own bytes, not a document rebuilt from what was read out of them.
process.stdout.write(json);
