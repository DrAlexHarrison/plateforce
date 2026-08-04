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
import { rmSync } from 'node:fs';
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

// The profile lives in memory and is removed on every exit, the check-minute shape: each
// leaked /tmp profile is ~160 MB and these scripts run many times over while a guard is
// broken and put back.
const profile = `/dev/shm/plateforce-check-grammar-${port}`;
const chrome = spawn('google-chrome', [
  '--headless=new', `--remote-debugging-port=${port + 1}`, '--no-sandbox',
  '--disable-gpu', `--user-data-dir=${profile}`, 'about:blank',
], { stdio: 'ignore', detached: true });
process.on('exit', () => {
  try { process.kill(-chrome.pid, 'SIGKILL'); } catch { /* already gone */ }
  try { rmSync(profile, { recursive: true, force: true }); } catch { /* already gone */ }
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
// The expectation is the constructs the build declares a request may name, which is the
// engine's own reach, rather than every construct a binding row mentions. Those differ:
// the signal the landmark rules read is produced before the recording is searched for
// anything and is named through a different map, so counting binding rows counted a
// construct the picker has to offer under other rules than the rest.
//
// This half asserts the picker hides nothing the engine can reach. The other direction,
// that nothing it offers is refused, is asserted in check-minute.mjs by clicking each one,
// and neither check stands up on its own.
const picker = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const { offerableConstructs } = await import('./add-quantity.js');
  const runnable = new Set([...state.build.derived_constructs, ...state.build.conditioning_constructs]);
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

/*
 * A choice between named alternatives is stated from the rail, and the record says who
 * stated it.
 *
 * Seventeen of the entries a result can name carry one, and until the tab drew a control for
 * them the browser could state none while the terminal, Python and R could state all. The
 * property is not that a control exists. It is that a name the reader picks arrives at the
 * engine and comes back recorded as theirs, and that a name they never touched comes back
 * recorded as the rule's, because those two are the same request otherwise.
 *
 * Every offered name is swept rather than one, from the rail the page rendered, because a
 * check that picks a single name reports the same green whether the other sixteen work or
 * were never drawn. Each pick starts from a freshly opened trial, through the call a reader's
 * own click goes through.
 */
const choices = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const { buildRequest, acceptRecommended } = await import('./analysis.js');
  const { enterWorkspace } = await import('./workspace.js');
  const { namedValues } = await import('./registry.js');

  // A trial just opened, with every slot bound the way the interface's own one act binds
  // them. A slot that forces a decision holds no rule until somebody makes it and therefore
  // offers no value either, so a sweep run before that act reads two constructs out of four
  // and reports the same green as one that reads them all.
  const opened = () => { enterWorkspace(); acceptRecommended(); };

  // What the rail is holding for a slot, read out of the request the engine is handed rather
  // than out of the tab's own bookkeeping, because the request is what the engine sees.
  const asked = (slotKey, construct, spine) => {
    const request = buildRequest();
    return spine ? request[slotKey] : (request.derived[construct] || request.conditioning[construct]);
  };
  const entryFor = (methodId) => state.registry.methods.find((m) => m.id === methodId) || null;
  // Which rule under this construct answered for a name, whether by recording it or by
  // reporting that it never read it. Narrowed to the construct because an operator reads its
  // value off the slot the rule above it reads, and two constructs can spell one name the
  // same way: dispersion is on both a weighing rule and a takeoff rule.
  const answeredFor = (name, construct) => {
    const under = (state.analysis?.bound_methods || [])
      .filter((bound) => entryFor(bound.method_id)?.construct === construct);
    return under.find((bound) => (bound.bound_parameters || []).some(([written]) => written === name))
      || under.find((bound) => (bound.unread_parameters || []).includes(name))
      || null;
  };
  const valueIn = (record, name) =>
    (record?.bound_parameters || []).find(([written]) => written === name)?.[1] ?? null;

  const controls = () => [...document.querySelectorAll('#decision-list select[data-option]')]
    .map((node) => ({
      name: node.dataset.option,
      construct: node.closest('.decision').querySelector('select[data-construct]').dataset.construct,
    }));
  // A control inside its own construct's row. Three rail rows spell dispersion the same way,
  // and a lookup by name alone drives the first of them three times over while reporting
  // three constructs.
  const node0 = (construct, name) => document
    .querySelector('#decision-list select[data-construct="' + construct + '"]')
    .closest('.decision')
    .querySelector('select[data-option="' + name + '"]');

  // Binds one rule on its own row, the way the reader's own click does, and reports what the
  // rail then offers under it against what the entries that ran declare.
  const bind = (construct, methodId) => {
    opened();
    const chooser = document.querySelector('#decision-list select[data-construct="' + construct + '"]');
    chooser.value = methodId;
    chooser.dispatchEvent(new Event('change'));
    const declared = new Set();
    for (const bound of state.analysis?.bound_methods || []) {
      const method = entryFor(bound.method_id);
      if (!method || method.construct !== construct) continue;
      if (method.gui?.surfacing === 'never_a_user_choice' || method.gui?.surfacing === 'refuse') continue;
      for (const parameter of method.parameter || []) {
        if (namedValues(parameter).length) declared.add(parameter.name);
      }
    }
    // What every control in this row is showing, against what the request carries for it. A
    // control showing a value the request does not carry is the number on screen disagreeing
    // with the number that ran, which is the shape a silent default takes in an interface.
    const slot = state.slots.find((s) => s.construct === construct);
    const asking = slot ? asked(slot.key, construct, slot.spine) : null;
    const row = document.querySelector('#decision-list select[data-construct="' + construct + '"]').closest('.decision');
    const showing = [...row.querySelectorAll('select[data-option], select[data-parameter]')].map((node) => ({
      name: node.dataset.option ?? node.dataset.parameter,
      shown: node.value,
      carried: node.dataset.option
        ? (asking?.options?.[node.dataset.option] ?? '')
        : String(asking?.parameters?.[node.dataset.parameter] ?? ''),
    }));
    return {
      declared: [...declared],
      offered: controls().filter((control) => control.construct === construct).map((control) => control.name),
      unasked: showing.filter((entry) => entry.shown !== entry.carried),
    };
  };

  // Every rule every rail row can bind, because the rules beneath a row change with the rule
  // above it and a sweep over one opening state reads five names out of twenty-five. A rule
  // that leaves the trial unanalysable is recorded as that rather than skipped, so a sweep
  // that reached nothing cannot read as one that found nothing to fault.
  const rules = [];
  for (const slot of state.slots) {
    for (const candidate of slot.available) {
      const seen = bind(slot.construct, candidate.id);
      rules.push({
        construct: slot.construct,
        methodId: candidate.id,
        refusedWhole: state.analysisRefusal?.code ?? null,
        ...seen,
        missing: seen.declared.filter((name) => !seen.offered.includes(name)),
      });
    }
  }

  const opening = [];
  const stated = [];
  const refused = [];
  const unread = [];
  const dropped = [];

  for (const rule of rules) {
    if (rule.refusedWhole) continue;
    const slot = state.slots.find((s) => s.construct === rule.construct);
    for (const name of rule.offered) {
      bind(rule.construct, rule.methodId);
      const before = asked(slot.key, rule.construct, slot.spine);
      const ranUnder = valueIn(answeredFor(name, rule.construct), name);
      const where = rule.construct + '.' + name + ' under ' + rule.methodId;
      // Only the names the registry's own default filled. A name the one act of taking the
      // recommendation filled is a different claim and is recorded as the act it was.
      if ((before.from_registry_default || []).includes(name)) {
        opening.push({
          name: where,
          value: before.options[name],
          selected: node0(rule.construct, name)?.value ?? null,
          source: answeredFor(name, rule.construct)?.parameter_sources?.[name] ?? null,
          recorded: ranUnder,
        });
      }

      // Every alternative the entry lists, not one. A build implements a subset of what the
      // literature contains, and the value it declines is the one a sweep over first
      // alternatives never reaches.
      const alternatives = [...node0(rule.construct, name).options]
        .map((option) => option.value).filter(Boolean);
      for (const pick of alternatives) {
        if (pick === ranUnder) continue;
        bind(rule.construct, rule.methodId);
        const node = node0(rule.construct, name);
        node.value = pick;
        node.dispatchEvent(new Event('change'));

        const after = asked(slot.key, rule.construct, slot.spine);
        const record = answeredFor(name, rule.construct);
        // Both places a refusal lands: a rule that declined inside a result, and a request
        // the engine would not take at all, which leaves the last result standing.
        const declined = (state.analysis?.refusals || []).some((r) => r.parameter === name)
          || state.analysisRefusal?.parameter === name;
        const row = {
          name: where,
          pick,
          ranUnder,
          reachedTheRequest: after.options[name] === pick,
          claimedAsDefault: (after.from_registry_default || []).includes(name),
          source: record?.parameter_sources?.[name] ?? null,
          written: valueIn(record, name),
          // Said once, and by the rule that said it. The engine writes a declining rule's
          // sentence into the warnings as well, so a page drawing both says one fact twice
          // and a page drawing only the warning says it with no rule attached. Counting the
          // notices carrying the pair tells all three apart.
          onScreen: declined ? (() => {
            const carrying = [...document.querySelectorAll('#analysis-warnings .notice')]
              .filter((n) => n.textContent.includes(name) && n.textContent.includes(pick));
            const said = (state.analysis?.refusals || []).find((r) => r.parameter === name);
            const titled = said && carrying.some((n) =>
              n.querySelector('p').textContent.startsWith(
                (entryFor(said.method_id)?.title ?? said.method_id) + ':'));
            return carrying.length === 1 && Boolean(titled);
          })() : null,
        };
        if (declined) refused.push(row);
        else if ((record?.unread_parameters || []).includes(name)) unread.push(row);
        else if (row.source === 'stated' && row.written === pick) stated.push(row);
        else dropped.push(row);
      }
    }
  }
  enterWorkspace();
  return {
    rules,
    offered: rules.reduce((total, rule) => total + rule.offered.length, 0),
    declared: rules.reduce((total, rule) => total + rule.declared.length, 0),
    unoffered: rules.filter((rule) => rule.missing.length)
      .map((rule) => rule.methodId + ': ' + rule.missing.join(', ')),
    unanalysable: rules.filter((rule) => rule.refusedWhole).map((rule) => rule.methodId),
    controlsRead: rules.reduce((total, rule) => total + rule.offered.length + rule.unasked.length, 0),
    unasked: rules.filter((rule) => rule.unasked.length)
      .map((rule) => rule.methodId + ': ' + rule.unasked.map((e) => e.name + ' shows ' + e.shown + ', request carries ' + (e.carried || 'nothing')).join('; ')),
    opening, stated, refused, unread, dropped,
  };
})()`);

check('every enumerated choice a bindable rule declares carries a control',
  choices.offered > 0 && choices.unoffered.length === 0 && choices.unanalysable.length === 0,
  `${choices.offered} of ${choices.declared} declared across ${choices.rules.length} rules the rail can bind, ` +
    `over ${new Set(choices.rules.map((rule) => rule.construct)).size} constructs` +
    (choices.unoffered.length ? ` | no control for: ${choices.unoffered.join('; ')}` : '') +
    (choices.unanalysable.length ? ` | left the trial unanalysable: ${choices.unanalysable.join(', ')}` : ''));

check('no control shows a value the request does not carry',
  choices.rules.length > 0 && choices.unasked.length === 0,
  `${choices.rules.length} rules bound, every control on each row read against the request` +
    (choices.unasked.length ? ` | ${choices.unasked.join(' | ')}` : ''));

// The opening state, which is the half a control can silently get wrong: a value the registry
// chose, shown as though the reader had. Each has to be the value on screen, the value the
// rule ran, and a source that says nobody was asked, all three at once.
const openingWrong = choices.opening.filter(
  (row) => row.source !== 'assumed' || row.recorded !== row.value || row.selected !== row.value);
check('a declared default opens selected and records as the rule’s own, not the reader’s',
  choices.opening.length > 0 && openingWrong.length === 0,
  choices.opening.length === 0
    ? 'no offered choice opens on a registry default, so this arm read nothing'
    : `${choices.opening.length} defaults, each on screen, run by the rule and recorded assumed: ` +
      choices.opening.map((row) => `${row.name}=${row.value}`).join(', ') +
      (openingWrong.length ? ` | wrong: ${openingWrong.map((row) => `${row.name} shown ${row.selected} source ${row.source} recorded ${row.recorded}`).join('; ')}` : ''));

// A name the tab stated has to reach the request whatever the rule does with it afterwards,
// which is the half that belongs to the browser rather than to the engine.
const missedTheRequest = [...choices.stated, ...choices.refused, ...choices.unread, ...choices.dropped]
  .filter((row) => !row.reachedTheRequest);
const picked = choices.stated.length + choices.refused.length + choices.unread.length + choices.dropped.length;
check('a name the reader picks reaches the request and claims no registry default',
  picked > 0 && missedTheRequest.length === 0
    && ![...choices.stated, ...choices.unread].some((row) => row.claimedAsDefault),
  `${picked} values picked across ${choices.offered} controls, ${missedTheRequest.length} missing from the request` +
    (missedTheRequest.length ? `: ${missedTheRequest.map((row) => row.name).join(', ')}` : ''));

const namesRead = new Set(choices.stated.map((row) => row.name));
check('a stated choice comes back recorded as stated, under the value that was picked',
  choices.stated.length > 0 && choices.dropped.length === 0,
  `${choices.stated.length} of ${picked} recorded stated, over ${namesRead.size} names the rules read; ` +
    `${choices.refused.length} declined by the rule, ${choices.unread.length} the rule never read` +
    (choices.unread.length ? ` (${[...new Set(choices.unread.map((row) => row.name))].join(', ')})` : '') +
    (choices.dropped.length ? ` | dropped silently: ${choices.dropped.map((row) => `${row.name} source ${row.source} recorded ${row.written}`).join('; ')}` : ''));

check('a name this build does not run is declined, and the page says so once, under the rule that declined',
  choices.refused.length > 0 && choices.refused.every((row) => row.onScreen === true),
  choices.refused.length === 0
    ? 'no offered name was declined by its rule, so nothing here was read'
    : `${choices.refused.length} declined, ${choices.refused.filter((row) => row.onScreen).length} said once and named: ` +
      choices.refused.map((row) => `${row.name}=${row.pick}`).join(', '));

const failures = results.filter((result) => !result.passed);
for (const result of results) {
  console.log(`${result.passed ? 'pass' : 'FAIL'}  ${result.name}\n      ${result.read}`);
}
if (consoleLines.length) console.log(`\nconsole errors:\n  ${consoleLines.join('\n  ')}`);
console.log(`\n${results.length - failures.length} of ${results.length} checks passed`);

socket.close();
server.close();
process.exit(failures.length || consoleLines.length ? 1 : 0);
