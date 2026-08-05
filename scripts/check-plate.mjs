/*
 * The plate, asserted rather than described.
 *
 * A lab states what its plate is once, the chip in the corner says which plate the tab is
 * analysing against, and every result carries the members and the saved plate behind them.
 * Every one of those is a claim about a running page, so this drives the page.
 *
 * Each check states what it read as well as whether it passed. The load-bearing ones assert
 * an exact set rather than a count: a page that carried four of five members into the record
 * reads exactly like one that carried five, to any check that only counts them.
 *
 * Usage: node scripts/check-plate.mjs <root directory> <port>
 */

import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { rmSync } from 'node:fs';
import { readFile, readdir } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';

const [root, port] = [process.argv[2] || 'web', Number(process.argv[3] || 8771)];
const FIXTURES = 'crates/plateforce-conformance/fixtures';
const TRIAL_SUFFIX = '.force.txt';
const SAMPLE_RATE_HZ = 1200;
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };

// The answers a lab would give, per member the block declares. A member the block gains is
// answered with a word rather than being left out, so this walk still fills the whole block
// without being told about it, and only the member holding a number needs naming here.
const ANSWERED = {
  filter_at_capture: 'none',
  tare_state: 'tared_before_trial',
  plate_natural_frequency_hz: '400',
  floor_surface: 'concrete',
  firmware_version: '2.4.1',
};
const ANSWERED_FALLBACK = 'stated';
const RESTATED_MEMBER = 'firmware_version';
const RESTATED_VALUE = '2.4.2';
const PLATE_NAME = 'Lab A';

const server = createServer(async (request, response) => {
  const url = normalize(request.url === '/' ? '/index.html' : request.url).replace(/^(\.\.[/\\])+/, '');
  const path = url.startsWith('/fixtures/') ? join(FIXTURES, url.slice('/fixtures/'.length)) : join(root, url);
  try {
    const body = await readFile(path);
    response.writeHead(200, { 'content-type': TYPES[extname(path)] || 'text/plain' });
    response.end(body);
  } catch {
    response.writeHead(404).end('not found');
  }
});
await new Promise((listening) => server.listen(port, listening));

// In memory and removed on every exit, including the exit a timeout leaves through. A profile
// per run on the root disk reaches gigabytes while a guard is being broken and put back.
const profile = `/dev/shm/plateforce-check-plate-${port}`;
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
  new Promise((settled) => {
    const id = (nextId += 1);
    pending.set(id, settled);
    socket.send(JSON.stringify({ id, method, params }));
  });
const evaluate = async (expression) => {
  const reply = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (reply.result?.exceptionDetails) throw new Error(JSON.stringify(reply.result.exceptionDetails));
  return reply.result.result.value;
};
const settle = async (expression, label) => {
  let lastRaise = null;
  for (let attempt = 0; attempt < 120; attempt += 1) {
    try {
      if (await evaluate(expression)) return;
    } catch (raised) {
      lastRaise = raised;
    }
    await new Promise((wait) => setTimeout(wait, 125));
  }
  throw new Error(`timed out waiting for ${label}${lastRaise ? `, last raise: ${lastRaise.message}` : ''}`);
};

/* Waiting on something a check below asserts, answered rather than raised. A raise here ends
 * the run, and a run that ends reports no check red at all, so breaking the very thing a check
 * exists to catch would read as this file being broken rather than as the check working. */
const waitFor = async (expression, attempts = 60) => {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      if (await evaluate(expression)) return true;
    } catch { /* the page has not reached that far */ }
    await new Promise((wait) => setTimeout(wait, 125));
  }
  return false;
};

const results = [];
const check = (name, passed, read) => results.push({ name, passed, read });

await send('Runtime.enable');
await send('Log.enable');
await send('Page.navigate', { url: `http://127.0.0.1:${port}/index.html` });
await settle("!document.getElementById('stage-empty').hidden", 'the empty stage');
// A plate saved by an earlier run of this file would answer for one this run never stated.
await evaluate('window.localStorage.clear()');
await send('Page.navigate', { url: `http://127.0.0.1:${port}/index.html` });
await settle("!document.getElementById('stage-empty').hidden", 'the empty stage');

await evaluate("document.getElementById('load-demo').click()");
await settle("!document.getElementById('stage-workspace').hidden", 'the workspace');
await settle(
  "document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric').length > 0",
  'the first paint',
);

/*
 * The members the page offers are the members the module says the block holds.
 *
 * Read from the manifest inside the page rather than from a list written here, and compared as
 * an ordered set. A page carrying a list of its own passes a count and fails this the day a
 * member is added or renamed, which is the whole reason the list is not written in `web/`.
 */
const declared = await evaluate(`(async () => {
  const { capabilityJson } = await import('./pkg/plateforce_wasm.js');
  const { reply } = await import('./format.js');
  const { state } = await import('./state.js');
  return { manifest: reply(capabilityJson()).ok.acquisition.members, held: state.plate.members };
})()`);
const opening = await evaluate(`(() => ({
  name: document.getElementById('plate-chip-name').textContent,
  count: document.getElementById('plate-chip-count').textContent,
  drawerHidden: document.getElementById('plate-drawer').hidden,
}))()`);
check('before anybody states a plate the chip counts the block the module declares',
  declared.manifest.length > 0 && opening.name === 'Plate'
    && opening.count === `0 of ${declared.manifest.length}`,
  `chip "${opening.name} ${opening.count}" against ${declared.manifest.length} members: ${declared.manifest.join(', ')}`);

await evaluate("document.getElementById('plate-chip').click()");
await settle("!document.getElementById('plate-drawer').hidden", 'the plate');
const fields = await evaluate(`(() => [...document.querySelectorAll('#plate-members label')].map((l) => l.textContent))()`);
check('the chip opens the plate, with one field per member the block declares',
  fields.join(',') === declared.manifest.join(',') && declared.held.join(',') === declared.manifest.join(','),
  `${fields.length} fields (${fields.join(', ')}) against ${declared.manifest.length} the manifest names ` +
  `(${declared.manifest.join(', ')})`);

/*
 * Every member answered, then saved under a name, which is the one act the decree asks for.
 *
 * A member the page offers no field for is collected rather than raised on: the check above is
 * the one that owns whether the fields match the block, and a raise here would end the run
 * before it could say so.
 */
const stateThePlate = async (values) => evaluate(`(() => {
  const answers = ${JSON.stringify(values)};
  const unoffered = [];
  for (const [member, value] of Object.entries(answers)) {
    const field = document.getElementById('plate-member-' + member);
    if (!field) { unoffered.push(member); continue; }
    field.value = value;
    field.dispatchEvent(new Event('change'));
  }
  return unoffered;
})()`);
const answers = Object.fromEntries(declared.manifest.map((m) => [m, ANSWERED[m] ?? ANSWERED_FALLBACK]));
const unoffered = await stateThePlate(answers);
await evaluate(`(() => {
  const name = document.getElementById('plate-name');
  name.value = ${JSON.stringify(PLATE_NAME)};
  name.dispatchEvent(new Event('input'));
  document.getElementById('plate-save').click();
  return true;
})()`);
await waitFor(`document.getElementById('plate-chip-name').textContent === ${JSON.stringify(PLATE_NAME)}`);

const recorded = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const rows = [...document.querySelectorAll('#build-info dt')].map((dt) => [dt.textContent, dt.nextElementSibling.textContent]);
  return {
    chip: document.getElementById('plate-chip-name').textContent + ' ' + document.getElementById('plate-chip-count').textContent,
    block: state.analysis?.acquisition ?? {},
    complete: state.analysis?.acquisition_complete ?? null,
    attribution: state.analysis?.plate_profile ?? null,
    rows,
  };
})()`);
// Sorted before comparing, because the record and the walk name the members in two different
// orders and a comparison of the two serialisations reads that as a different set.
const asPairs = (held) => Object.entries(held)
  .filter(([, value]) => value != null)
  .map(([member, value]) => `${member}=${value}`)
  .sort()
  .join(', ');
const carried = asPairs(recorded.block);
check('a plate stated once carries every member into the record, complete and attributed',
  recorded.complete === true
    && unoffered.length === 0
    && carried === asPairs(answers)
    && recorded.attribution?.name === PLATE_NAME
    && (recorded.attribution?.revision ?? '').length > 0,
  `complete ${recorded.complete}, the record holds ${carried || 'nothing'} against ${asPairs(answers)} stated` +
  (unoffered.length ? `, with no field offered for ${unoffered.join(', ')}` : '') +
  `, under ${recorded.attribution?.name ?? 'no plate'} ${recorded.attribution?.revision ?? ''}`);

const named = new Map(recorded.rows);
check('the result names its plate where it names what produced the numbers',
  named.get('Plate') === `${PLATE_NAME}, ${declared.manifest.length} of ${declared.manifest.length} answered`
    && named.get('Plate revision') === recorded.attribution?.revision,
  `${recorded.rows.length} rows, Plate "${named.get('Plate')}", revision "${named.get('Plate revision')}"`);

/*
 * An answer stated on this capture beside a saved plate.
 *
 * Both halves asserted. The stated answer has to be what ran, and the saved one has to survive
 * in the record as what it displaced: an overlay that kept only the winner leaves a reader
 * unable to see that a replacement happened at all.
 */
const revisionBefore = recorded.attribution?.revision ?? null;
await stateThePlate({ [RESTATED_MEMBER]: RESTATED_VALUE });
await new Promise((wait) => setTimeout(wait, 200));
const displaced = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const rows = [...document.querySelectorAll('#build-info dt')].map((dt) => [dt.textContent, dt.nextElementSibling.textContent]);
  return {
    ran: state.analysis?.acquisition?.${RESTATED_MEMBER} ?? null,
    superseded: state.analysis?.plate_profile?.superseded_members ?? {},
    revision: state.analysis?.plate_profile?.revision ?? null,
    replaced: rows.filter(([term]) => term.startsWith('Replaced ')),
  };
})()`);
check('an answer stated over the plate displaces it, and the result carries both',
  displaced.ran === RESTATED_VALUE
    && displaced.superseded[RESTATED_MEMBER] === answers[RESTATED_MEMBER]
    && displaced.replaced.length === 1
    && revisionBefore !== null
    && displaced.revision === revisionBefore,
  `${RESTATED_MEMBER} ran as ${displaced.ran}, replacing ${JSON.stringify(displaced.superseded)}, ` +
  `shown as ${JSON.stringify(displaced.replaced)}; the plate's own revision is ${displaced.revision === revisionBefore ? 'unmoved' : 'moved, though nobody saved it'}`);

/* A second plate, and the chip changed in one click, which is the rest of the decree. */
await evaluate(`(() => {
  const name = document.getElementById('plate-name');
  name.value = 'Rig 2';
  name.dispatchEvent(new Event('input'));
  document.getElementById('plate-save').click();
  return true;
})()`);
await waitFor("document.getElementById('plate-chip-name').textContent === 'Rig 2'");
const changed = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const option = [...document.querySelectorAll('#plate-options [role="radio"]')]
    .find((node) => node.dataset.plate === ${JSON.stringify(PLATE_NAME)});
  const offered = [...document.querySelectorAll('#plate-options [role="radio"]')].map((n) => n.textContent.trim());
  if (option) option.click();
  return { offered, chip: document.getElementById('plate-chip-name').textContent,
           ranUnder: state.analysis?.plate_profile?.name ?? null };
})()`);
check('the plate the tab analyses against changes in one click on the chip',
  changed.offered.length === 3 && changed.chip === PLATE_NAME && changed.ranUnder === PLATE_NAME,
  `${changed.offered.length} offered (${changed.offered.join(' / ')}), the chip reads "${changed.chip}" ` +
  `and the result ran under ${changed.ranUnder}`);

/* The narrow viewport, with the plate open, which is the state the four floors are never read
 * in by the gate that reads them: a drawer that is hidden is measured as nothing at all. */
await send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 2, mobile: true });
await new Promise((wait) => setTimeout(wait, 400));
const narrow = await evaluate(`(() => {
  const visible = (node) => node.getClientRects().length > 0;
  const names = (node) => node.tagName.toLowerCase() + (node.id ? '#' + node.id : '');
  const boxes = [...document.querySelectorAll('#plate-drawer button, #plate-drawer input, .plate-chip')].filter(visible);
  return {
    overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    small: boxes.map((node) => [names(node), Math.round(Math.min(
      node.getBoundingClientRect().width, node.getBoundingClientRect().height))])
      .filter(([, side]) => side < 44),
    unlabelled: boxes.filter((node) => !node.textContent.trim() && !node.getAttribute('aria-label') && !node.labels?.length).map(names),
    counted: boxes.length,
  };
})()`);
await send('Emulation.clearDeviceMetricsOverride');
check('at 390 px the plate does not scroll the page sideways and every control clears 44 px',
  narrow.overflow <= 0 && narrow.small.length === 0 && narrow.unlabelled.length === 0 && narrow.counted > 5,
  `${narrow.overflow} px of overflow, ${narrow.counted} controls checked` +
  (narrow.small.length ? `, under 44 px: ${narrow.small.map(([w, s]) => `${w} ${s}`).join(', ')}` : '') +
  (narrow.unlabelled.length ? `, unnamed: ${narrow.unlabelled.join(', ')}` : ''));

/*
 * A folder run under a plate, and then the plate edited underneath it.
 *
 * The run keeps the answers it was given, so the two revisions are on screen together and a
 * reader can tell one run from the other. Hiding the difference is what would make two sets of
 * numbers taken off two configurations read as one.
 */
await evaluate("document.querySelector('#plate-drawer [data-close-drawer]').click()");
const trialNames = (await readdir(FIXTURES)).filter((name) => name.endsWith(TRIAL_SUFFIX)).sort();
await evaluate("document.getElementById('change-file').click()");
await settle("!document.getElementById('stage-empty').hidden", 'the empty stage');
await evaluate(`(async () => {
  const transfer = new DataTransfer();
  for (const name of ${JSON.stringify(trialNames)}) {
    transfer.items.add(new File([await (await fetch('/fixtures/' + name)).text()], name, { type: 'text/plain' }));
  }
  document.getElementById('dropzone').dispatchEvent(
    new DragEvent('drop', { dataTransfer: transfer, bubbles: true, cancelable: true }),
  );
  return true;
})()`);
await settle("!document.getElementById('stage-columns').hidden", 'the columns stage');
await evaluate(`(() => {
  const rate = document.getElementById('sample-rate');
  rate.value = '${SAMPLE_RATE_HZ}';
  rate.dispatchEvent(new Event('input'));
  document.getElementById('columns-confirm').click();
  return true;
})()`);
await settle("!document.getElementById('stage-workspace').hidden", 'the workspace');
await evaluate(`(() => {
  const take = document.getElementById('accept-recommended');
  if (take) take.click();
  return true;
})()`);
await evaluate("document.getElementById('run-folder').click()");
await settle("document.querySelector('#batch-result table.data tbody tr')", 'the batch table');

const ranUnder = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  return {
    lines: [...document.querySelectorAll('#batch-result .panel__sub')].map((p) => p.textContent),
    revision: JSON.parse(state.run.envelope).ok.run.plate_profile?.revision ?? null,
    plate: JSON.parse(state.run.envelope).ok.run.plate_profile?.name ?? null,
    // The first table only. The trials that declined are drawn in a second one, and a count
    // over both reports fourteen rows for eight trials.
    rows: document.querySelector('#batch-result table.data tbody').querySelectorAll('tr').length,
  };
})()`);
check('a folder run says which plate and which revision of it produced the table',
  ranUnder.plate === PLATE_NAME && ranUnder.rows === trialNames.length
    && ranUnder.lines.some((line) => line === `${PLATE_NAME} · revision ${ranUnder.revision}`),
  `${ranUnder.rows} of ${trialNames.length} trials under ${ranUnder.plate} ${ranUnder.revision}; ` +
  `lines: ${ranUnder.lines.join(' | ')}`);

// Edited from the chip, which opens over the table the reader is looking at.
await evaluate("document.getElementById('plate-chip').click()");
await settle("!document.getElementById('plate-drawer').hidden", 'the plate');
await stateThePlate({ [RESTATED_MEMBER]: RESTATED_VALUE });
await evaluate(`(() => { document.getElementById('plate-save').click(); return true; })()`);
await evaluate("document.querySelector('#plate-drawer [data-close-drawer]').click()");
await waitFor(
  `[...document.querySelectorAll('#batch-result .panel__sub')].some((p) => p.textContent.includes('· current '))`,
);
const stale = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const { revisionNow } = await import('./plate.js');
  return {
    line: [...document.querySelectorAll('#batch-result .panel__sub')]
      .find((p) => p.textContent.includes(${JSON.stringify(PLATE_NAME + ' · run ')}))?.textContent ?? null,
    ranUnder: JSON.parse(state.run.envelope).ok.run.plate_profile?.revision ?? null,
    now: revisionNow(${JSON.stringify(PLATE_NAME)}),
    onScreen: state.analysis?.plate_profile?.revision ?? null,
  };
})()`);
check('a plate edited after a run leaves both revisions on screen, told apart',
  stale.ranUnder !== stale.now && stale.now === stale.onScreen
    && stale.line === `${PLATE_NAME} · run ${stale.ranUnder} · current ${stale.now}`,
  `the table ran under ${stale.ranUnder}, the trial on screen under ${stale.onScreen}, ` +
  `the plate now reads ${stale.now}: "${stale.line}"`);

/*
 * A member the block does not hold, arriving the way one does: saved by a build whose block
 * carried it. The block's own parser refuses it, and the refusal reaches the reader rather
 * than the member being dropped and the result claiming a plate nobody stated.
 */
const refused = await evaluate(`(async () => {
  const { savePlate } = await import('./plate.js');
  const { state } = await import('./state.js');
  const { runAnalysis } = await import('./analysis.js');
  savePlate('Old rig', { debounce_ms: '50' });
  state.plate.picked = 'Old rig';
  state.plate.stated = {};
  runAnalysis();
  const notice = document.querySelector('#analysis-warnings .notice--danger');
  return {
    heading: notice?.querySelector('strong')?.textContent ?? null,
    message: notice?.querySelector('p')?.textContent ?? null,
    metrics: document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric').length,
    members: state.plate.members,
  };
})()`);
const namesEveryMember = refused.members.every((member) => (refused.message ?? '').includes(member));
check('a member the block does not hold refuses, naming what the block does hold',
  refused.metrics === 0 && refused.heading === 'Plate data error'
    && (refused.message ?? '').includes('debounce_ms') && namesEveryMember,
  `"${refused.heading}": ${refused.message}; ${refused.metrics} values left on screen, ` +
  `${refused.members.filter((m) => (refused.message ?? '').includes(m)).length} of ${refused.members.length} members named`);

check('no console errors', consoleLines.length === 0, consoleLines.join(' | ') || 'none');

const failed = results.filter((result) => !result.passed);
for (const result of results) {
  console.log(`${result.passed ? 'pass' : 'FAIL'}  ${result.name}\n        ${result.read}`);
}
console.log(`\n${results.length - failed.length} of ${results.length} checks passed`);

socket.close();
server.close();
process.exit(failed.length ? 1 : 0);
