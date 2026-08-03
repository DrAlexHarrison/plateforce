/*
 * The rail is a query against the registry, not a list.
 *
 * Two halves. The first calls the decision model directly, because the model is pure and a
 * fixture registry can state the exact shape a real one is not currently in: a construct
 * whose loudest entry is not its bound one, three constructs from different classes in
 * scrambled order, and a construct that did not exist when this file was written. The
 * second drives the running page, because a model nothing renders is a model that proves
 * nothing about the interface.
 *
 * Usage: node scripts/check-grammar.mjs <root directory> <port>
 */

import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { buildDecisionModel } from '../web/registry.js';

const [root, port] = [process.argv[2] || 'web', Number(process.argv[3] || 8791)];
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };

const results = [];
const check = (name, passed, read) => results.push({ name, passed, read });

/* A build and a registry stating one shape, so a claim about the model is a claim about
 * that shape rather than about whichever shape the real registry happens to be in. */
function fixture({ constructs, methods, bindings, spine }) {
  return [
    { constructs, methods },
    { bindings, spine_constructs: spine },
  ];
}

const surfacingRow = (id, construct, surfacing, status = 'accepted') => ({
  id, construct, status, title: `rule ${id}`, gui: { surfacing },
});
const bindingRow = (id, construct, slot = construct) => ({
  id, slot, construct, title: `rule ${id}`, composed_from: null, note: '', quantities: [],
});

/* PP-GRAMMAR-02. One entry displayed unasked does not make the whole construct a decision,
 * and the row carries no aggregate verdict for a renderer to reach for. */
{
  const methods = [surfacingRow('quiet.0', 'landmark', 'default_and_show')];
  for (let n = 1; n <= 10; n += 1) methods.push(surfacingRow(`quiet.${n}`, 'landmark', 'default_and_hide'));
  const [registry, build] = fixture({
    constructs: [{ id: 'landmark', title: 'Landmark', label: 'A landmark', notes: '' }],
    methods,
    bindings: methods.map((method) => bindingRow(method.id, 'landmark')),
    spine: ['landmark'],
  });
  const [row] = buildDecisionModel(registry, build);
  const bound = row.candidates.find((candidate) => candidate.id === 'quiet.5');
  check('a slot presents as the entry bound to it, not as its loudest member',
    row.surfacing === undefined && bound.surfacing === 'default_and_hide' && row.forcesDecision === false,
    `row carries ${row.surfacing === undefined ? 'no aggregate verdict' : `an aggregate verdict ${row.surfacing}`}, ` +
      `the bound entry is ${bound.surfacing}, forcesDecision ${row.forcesDecision}, over ${row.candidates.length} entries`);
}

/* PP-GRAMMAR-04. A construct that forces a decision leaves every value below it
 * provisional, so it is read first however late in the pipeline it runs. */
{
  const methods = [
    surfacingRow('a.rule', 'first_run', 'default_and_hide'),
    surfacingRow('b.rule', 'second_run', 'default_and_hide'),
    surfacingRow('c.rule', 'third_run', 'force_a_decision'),
    surfacingRow('c.other', 'third_run', 'accepted'),
  ];
  const [registry, build] = fixture({
    constructs: ['first_run', 'second_run', 'third_run'].map((id) => ({ id, title: id, label: id, notes: '' })),
    methods,
    bindings: [
      bindingRow('a.rule', 'first_run'), bindingRow('b.rule', 'second_run'),
      bindingRow('c.rule', 'third_run'), bindingRow('c.other', 'third_run'),
    ],
    spine: [],
  });
  const order = buildDecisionModel(registry, build, ['first_run', 'second_run', 'third_run'])
    .map((row) => row.construct);
  check('the rail reads down by consequence, then by the order the pipeline runs them',
    order.join(' > ') === 'third_run > first_run > second_run', order.join(' > '));
}

/* PP-GRAMMAR-06. The build declares one more construct than it did a moment ago and the
 * model carries one more row, with no edit to any file under web/. */
{
  const before = fixture({
    constructs: [{ id: 'landmark', title: 'Landmark', label: 'A landmark', notes: 'what it is' }],
    methods: [surfacingRow('one.rule', 'landmark', 'default_and_show')],
    bindings: [bindingRow('one.rule', 'landmark')],
    spine: ['landmark'],
  });
  const rowsBefore = buildDecisionModel(before[0], before[1]).length;

  const after = fixture({
    constructs: [
      { id: 'landmark', title: 'Landmark', label: 'A landmark', notes: 'what it is' },
      { id: 'arrived_later', title: 'Arrived later', label: 'A construct nobody wrote down', notes: 'stated here' },
    ],
    methods: [
      surfacingRow('one.rule', 'landmark', 'default_and_show'),
      surfacingRow('later.rule', 'arrived_later', 'default_and_show'),
    ],
    bindings: [bindingRow('one.rule', 'landmark'), bindingRow('later.rule', 'arrived_later')],
    spine: ['landmark'],
  });
  const rowsAfter = buildDecisionModel(after[0], after[1], ['arrived_later']);
  const added = rowsAfter.find((row) => row.construct === 'arrived_later');
  check('a construct the build did not carry before appears as a row, labelled from the registry',
    rowsAfter.length === rowsBefore + 1 && added?.title === 'A construct nobody wrote down'
      && added?.key === 'arrived_later' && added?.spine === false,
    `${rowsBefore} rows became ${rowsAfter.length}, the new one titled "${added?.title}" and reached as ${added?.key}`);
}

/* A construct nobody asked for raises no decision, which is why fifty-eight constructs do
 * not become fifty-eight rows. */
{
  const [registry, build] = fixture({
    constructs: ['named', 'unasked'].map((id) => ({ id, title: id, label: id, notes: '' })),
    methods: [surfacingRow('n.rule', 'named', 'default_and_show'), surfacingRow('u.rule', 'unasked', 'force_a_decision')],
    bindings: [bindingRow('n.rule', 'named'), bindingRow('u.rule', 'unasked')],
    spine: ['named'],
  });
  const rows = buildDecisionModel(registry, build).map((row) => row.construct);
  check('a construct nobody asked for is not on the path and raises no decision',
    rows.length === 1 && rows[0] === 'named', `rows: ${rows.join(', ')} of 2 the build can run`);
}

/* The running page. */
const server = createServer(async (request, response) => {
  const path = join(root, normalize(request.url === '/' ? '/index.html' : request.url).replace(/^(\.\.[/\\])+/, ''));
  try {
    const body = await readFile(path);
    response.writeHead(200, { 'content-type': TYPES[extname(path)] || 'application/octet-stream' });
    response.end(body);
  } catch {
    response.writeHead(404).end('not found');
  }
});
await new Promise((listening) => server.listen(port, listening));

const chrome = spawn('google-chrome', [
  '--headless=new', `--remote-debugging-port=${port + 1}`, '--no-sandbox',
  '--disable-gpu', `--user-data-dir=/tmp/plateforce-check-grammar-${port}`, 'about:blank',
], { stdio: 'ignore', detached: true });
process.on('exit', () => {
  try { process.kill(-chrome.pid, 'SIGKILL'); } catch { /* already gone */ }
});

const targets = await (async () => {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try {
      return await (await fetch(`http://127.0.0.1:${port + 1}/json/list`)).json();
    } catch {
      await new Promise((wait) => setTimeout(wait, 250));
    }
  }
  throw new Error('chrome did not open a debugging port');
})();

const socket = new WebSocket(targets.find((target) => target.type === 'page').webSocketDebuggerUrl);
await new Promise((open) => socket.addEventListener('open', open));

let nextId = 0;
const pending = new Map();
const consoleLines = [];
socket.addEventListener('message', (event) => {
  const message = JSON.parse(event.data);
  if (pending.has(message.id)) {
    pending.get(message.id)(message);
    pending.delete(message.id);
  }
  if (message.method === 'Log.entryAdded' && message.params.entry.level === 'error') {
    consoleLines.push(message.params.entry.text);
  }
});
const send = (method, params = {}) =>
  new Promise((settle) => {
    const id = (nextId += 1);
    pending.set(id, settle);
    socket.send(JSON.stringify({ id, method, params }));
  });
const evaluate = async (expression) => {
  const reply = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (reply.result?.exceptionDetails) throw new Error(JSON.stringify(reply.result.exceptionDetails));
  return reply.result.result.value;
};
const settle = async (expression, label) => {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    try {
      if (await evaluate(expression)) return;
    } catch { /* the page has not parsed that far yet */ }
    await new Promise((wait) => setTimeout(wait, 125));
  }
  throw new Error(`timed out waiting for ${label}`);
};

await send('Runtime.enable');
await send('Log.enable');
await send('Page.navigate', { url: `http://127.0.0.1:${port}/index.html` });
await settle("!document.getElementById('stage-empty').hidden", 'the empty stage');
await evaluate("document.getElementById('load-demo').click()");
await settle("!document.getElementById('stage-workspace').hidden", 'the workspace');
await settle("document.querySelectorAll('#decision-list .decision').length > 0", 'the rail');

const live = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const { buildRequest } = await import('./analysis.js');
  const labelOf = new Map(state.registry.constructs.map((c) => [c.id, c.label || c.title]));
  const request = buildRequest();
  return {
    spine: state.build.spine_constructs,
    rowConstructs: state.slots.map((slot) => slot.construct),
    rowTitles: state.slots.map((slot) => slot.title),
    expectedTitles: state.slots.map((slot) => labelOf.get(slot.construct)),
    rendered: [...document.querySelectorAll('#decision-list select[data-construct]')]
      .map((node) => node.dataset.construct),
    named: Object.keys(request).filter((key) => request[key] && request[key].method_id),
    derived: Object.keys(request.derived),
    constructsInRegistry: state.registry.constructs.length,
  };
})()`);

check('the browser reports the same spine the engine does',
  live.spine.length === 3 && live.rowConstructs.length === live.spine.length,
  `${live.rowConstructs.length} rows for ${live.spine.length} constructs on the path, of ${live.constructsInRegistry} the registry declares`);
check('every row is titled with the registry label for its construct',
  live.rowTitles.length > 0 && live.rowTitles.every((title, index) => title === live.expectedTitles[index]),
  live.rowTitles.join(' / '));
check('every row on the path is a row the rail rendered',
  live.rendered.join(',') === live.rowConstructs.join(','),
  `rendered ${live.rendered.join(', ')}`);
check('the request names every construct on the path, by its field or through derived',
  live.named.length + live.derived.length === live.rowConstructs.length,
  `${live.named.length} named by a field (${live.named.join(', ')}), ${live.derived.length} through derived`);

/*
 * PP-GRAMMAR-03. A quantity the path does not visit is reached from the workspace, by
 * searching the words the field speaks, in two interactions: type, then choose.
 */
const picker = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const { offerableConstructs } = await import('./add-quantity.js');
  const runnable = new Set(state.build.bindings.map((b) => b.construct));
  const visited = new Set(state.slots.map((s) => s.construct));
  return {
    offered: [...document.querySelectorAll('#add-quantity-list button')].map((b) => b.textContent),
    offerable: offerableConstructs().length,
    runnableNotVisited: [...runnable].filter((c) => !visited.has(c)).length,
    copy: document.querySelector('#add-quantity label').textContent,
    hidden: document.getElementById('add-quantity').hidden,
  };
})()`);

check('the picker offers every quantity a rule can produce that the path does not visit',
  !picker.hidden && picker.offered.length === picker.runnableNotVisited && picker.offerable === picker.runnableNotVisited,
  `${picker.offered.length} offered against ${picker.runnableNotVisited} the build can run and the path does not visit: ${picker.offered.join(', ')}`);
check('the picker names the reader’s quantity and not the software’s inventory',
  picker.copy === 'Add a quantity', picker.copy);

/*
 * Typing narrows the list to the words the field speaks, which is the first of the two
 * interactions that reach a quantity. The term is the one the founding measurement is about,
 * and the construct chosen from the matches is whichever carries the most published rules,
 * so the sweep below is over the widest disagreement the build can run rather than over
 * whichever row happened to sort first.
 */
const SPOKEN = 'jump height';
const searched = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const search = document.getElementById('add-quantity-search');
  search.value = ${JSON.stringify(SPOKEN)};
  search.dispatchEvent(new Event('input'));
  const shown = [...document.querySelectorAll('#add-quantity-list button')];
  const rulesFor = (construct) => state.build.bindings.filter((b) => b.construct === construct).length;
  const widest = shown.slice().sort((a, b) => rulesFor(b.dataset.construct) - rulesFor(a.dataset.construct))[0];
  return {
    labels: shown.map((b) => b.textContent),
    constructs: shown.map((b) => b.dataset.construct),
    chosen: widest?.dataset.construct ?? null,
    chosenRules: widest ? rulesFor(widest.dataset.construct) : 0,
  };
})()`);
check('searching the spoken words narrows the list rather than listing every construct',
  searched.labels.length > 0 && searched.labels.length < picker.offered.length
    && searched.labels.every((label) => label.toLowerCase().includes(SPOKEN)),
  `${searched.labels.length} of ${picker.offered.length} match "${SPOKEN}": ${searched.labels.join(', ')}`);

/* The second interaction. A row appears in the rail, a card appears in the results, and the
 * card carries the rules that produced it. */
const beforeAdd = await evaluate(`(() => document.querySelectorAll('#metric-grid .metric').length)()`);
await evaluate(`document.querySelector('#add-quantity-list button[data-construct="${searched.chosen}"]').click()`);
await settle(`document.querySelector('#decision-list select[data-construct="${searched.chosen}"]')`,
  'the row for the quantity just added');

const added = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const { buildRequest } = await import('./analysis.js');
  const construct = ${JSON.stringify(searched.chosen)};
  const slot = state.slots.find((s) => s.construct === construct);
  const quantities = new Set(
    state.build.bindings.filter((b) => b.construct === construct).flatMap((b) => b.quantities.map((q) => q.key)),
  );
  const cards = state.analysis.metrics.filter((m) => quantities.has(m.key));
  return {
    onThePath: state.path.includes(construct),
    railRow: Boolean(document.querySelector('#decision-list select[data-construct="' + construct + '"]')),
    inTheRequest: Boolean(buildRequest().derived[construct]),
    rowTitle: slot?.title ?? null,
    cards: cards.map((m) => m.label),
    withProvenance: cards.filter((m) => (m.contributing_method_ids || []).length > 0).length,
    metricsNow: state.analysis.metrics.length,
    stillOffered: [...document.querySelectorAll('#add-quantity-list button')].map((b) => b.dataset.construct),
  };
})()`);

check('choosing it puts the construct on the path, in the rail and in the request',
  added.onThePath && added.railRow && added.inTheRequest,
  `${searched.chosen} titled "${added.rowTitle}", rail row ${added.railRow}, in the request ${added.inTheRequest}`);
check('a card appears in the results for it, carrying the rules that produced it',
  added.cards.length > 0 && added.withProvenance === added.cards.length,
  `${added.cards.length} cards (${added.cards.join(', ')}), ${added.withProvenance} carrying provenance, ${beforeAdd} metrics became ${added.metricsNow}`);
check('a quantity already on the path is no longer offered',
  !added.stillOffered.includes(searched.chosen),
  `still offered: ${added.stillOffered.join(', ') || 'none matching the search'}`);

/*
 * A sweep over a construct reached through `derived` moves the number.
 *
 * The engine resolves an axis it does not recognise as a landmark against the request's
 * `derived` map and refuses one the request does not carry, so an axis on such a row is
 * applied rather than ignored. Asserting more than one distinct value is what tells a
 * working sweep from one that returns its starting number as many times as it was asked.
 */
const swept = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const { buildRequest } = await import('./analysis.js');
  const { reply } = await import('./format.js');
  const construct = ${JSON.stringify(searched.chosen)};
  const slot = state.slots.find((s) => s.construct === construct);
  if (!slot || slot.available.length < 2) return { skipped: slot ? slot.available.length : 0 };
  const quantity = state.build.bindings.find((b) => b.construct === construct && b.quantities.length)
    ?.quantities[0]?.key;
  // Read through the page's own unwrapper rather than parsing here. The engine answers every
  // export in one envelope, {ok} or {refusal}, and a check that reaches past it reads undefined
  // for every field and reports that as a sweep returning one value many times, which is a
  // different fault from the one it would be describing.
  const { ok, refusal } = reply(state.loadedTrial.spread(JSON.stringify({
    base: buildRequest(),
    axes: [{ slot: slot.key, parameter: null, values: [], method_ids: slot.available.map((c) => c.id) }],
    quantity_key: quantity,
    maximum_combinations: 512,
  })));
  if (!ok) return { quantity, rules: slot.available.length, refused: refusal?.code ?? 'a refusal carrying no code' };
  const values = (ok.variants || []).map((v) => v.value).filter((v) => v != null);
  return {
    quantity,
    rules: slot.available.length,
    succeeded: ok.succeeded,
    distinct: new Set(values).size,
  };
})()`);

check('a sweep over a construct reached through derived varies the number rather than repeating it',
  swept.skipped === undefined && swept.refused === undefined && swept.distinct > 1,
  swept.skipped !== undefined
    ? `skipped: the added construct has ${swept.skipped} runnable rules`
    : swept.refused !== undefined
      ? `the sweep over ${swept.quantity} was declined: ${swept.refused}`
      : `${swept.rules} rules over ${swept.quantity}, ${swept.succeeded} succeeded, ${swept.distinct} distinct values`);

const failures = results.filter((result) => !result.passed);
for (const result of results) {
  console.log(`${result.passed ? 'pass' : 'FAIL'}  ${result.name}\n      ${result.read}`);
}
if (consoleLines.length) console.log(`\nconsole errors:\n  ${consoleLines.join('\n  ')}`);
console.log(`\n${results.length - failures.length} of ${results.length} checks passed`);

socket.close();
server.close();
process.exit(failures.length || consoleLines.length ? 1 : 0);
