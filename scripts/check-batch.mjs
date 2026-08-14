/*
 * A folder of trials through the browser, against the same folder through the terminal.
 *
 * The route is driven rather than described: the files arrive on the drop zone as a real
 * drop event, the run is declared on the stage a reader declares it on, and the table is
 * read out of the rendered document. A check that called the module's functions directly
 * would pass with no path from the page to them, which is the condition this file exists to
 * prove is gone.
 *
 * The comparison is value by value rather than byte by byte, because two renderings of one
 * double agree as text and disagree as numbers.
 *
 * Usage: node scripts/check-batch.mjs <root directory> <port>
 */

import { spawn, execFileSync } from 'node:child_process';
import { rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { readFile, readdir, mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { extname, join, normalize, resolve } from 'node:path';
import { listenForConsoleErrors } from './console-errors.mjs';
import { chromeArguments, chromeExecutable, scratchDirectory } from './browser.mjs';

const [root, port] = [process.argv[2] || 'web', Number(process.argv[3] || 8751)];
const FIXTURES = 'crates/plateforce-conformance/fixtures';
const TRIAL_SUFFIX = '.force.txt';
const SAMPLE_RATE_HZ = 1200;
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };

// The fixture folder is served beside the bundle so the page can read the same bytes the
// terminal reads, rather than a copy of them written into this file.
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
await new Promise((resolve) => server.listen(port, resolve));

// The profile lives in memory and is removed on every exit, the check-minute shape: each
// leaked /tmp profile is ~160 MB and these scripts run many times over while a guard is
// broken and put back.
const profile = scratchDirectory(`plateforce-check-batch-${port}`);
const chrome = spawn(chromeExecutable(), chromeArguments(port + 1, profile), { stdio: 'ignore', detached: true });
process.on('exit', () => {
  try { process.kill(-chrome.pid, 'SIGKILL'); } catch { /* already gone */ }
  try { rmSync(profile, { recursive: true, force: true }); } catch { /* already gone */ }
});

const targets = await (async () => {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try {
      return await (await fetch(`http://127.0.0.1:${port + 1}/json/list`)).json();
    } catch {
      await new Promise((settle) => setTimeout(settle, 250));
    }
  }
  throw new Error('chrome did not open a debugging port');
})();

const socket = new WebSocket(targets.find((t) => t.type === 'page').webSocketDebuggerUrl);
await new Promise((open) => socket.addEventListener('open', open));

let nextId = 0;
const pending = new Map();
const consoleLines = listenForConsoleErrors(socket);
socket.addEventListener('message', (event) => {
  const message = JSON.parse(event.data);
  if (pending.has(message.id)) {
    pending.get(message.id)(message);
    pending.delete(message.id);
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

const results = [];
const check = (name, passed, read) => results.push({ name, passed, read });

await send('Runtime.enable');
await send('Log.enable');
await send('Page.navigate', { url: `http://127.0.0.1:${port}/index.html` });
await settle("!document.getElementById('stage-empty').hidden", 'the empty stage');

// Every file in the fixture folder, trials and the rest, because a route that only ever
// receives files it can read never exercises the count of the ones it cannot.
const everyFixture = (await readdir(FIXTURES)).sort();
const trialNames = everyFixture.filter((name) => name.endsWith(TRIAL_SUFFIX));

const dropped = await evaluate(`(async () => {
  const names = ${JSON.stringify(everyFixture)};
  const transfer = new DataTransfer();
  for (const name of names) {
    const text = await (await fetch('/fixtures/' + name)).text();
    transfer.items.add(new File([text], name, { type: 'text/plain' }));
  }
  document.getElementById('dropzone').dispatchEvent(
    new DragEvent('drop', { dataTransfer: transfer, bubbles: true, cancelable: true }),
  );
  return transfer.files.length;
})()`);
check('every file the reader drops arrives on the page', dropped === everyFixture.length,
  `${dropped} of ${everyFixture.length} files dropped`);

await settle("!document.getElementById('stage-columns').hidden", 'the columns stage');

const declaration = await evaluate(`(() => ({
  shown: !document.getElementById('run-declaration').hidden,
  count: document.getElementById('run-count').textContent,
  endings: [...document.querySelectorAll('#run-suffix-list input')].map((i) => i.value),
  ticked: [...document.querySelectorAll('#run-suffix-list input')].filter((i) => i.checked).map((i) => i.value),
  opened: document.getElementById('columns-lead').textContent,
  separator: document.getElementById('run-delimiter').value,
}))()`);

const strays = everyFixture.length - trialNames.length;
check('the count of files found is stated before the run starts',
  declaration.shown && declaration.count.startsWith(`${everyFixture.length} files chosen`),
  declaration.count);
check('every distinct name ending in the folder is offered',
  declaration.endings.includes(TRIAL_SUFFIX) && declaration.endings.length > 1,
  declaration.endings.join(' / '));
check('a file no declared ending names is reported as excluded, with the ending that left it out',
  declaration.count.includes(`${strays} left out`) && declaration.count.includes('README.md (.md)'),
  declaration.count);
// The folder's first file by name is not one of its trials, so the stage opening on a file
// the declaration does not name would describe columns the run never reads.
check('the trial on screen is one the declaration names',
  trialNames.some((name) => declaration.opened.startsWith(name)),
  declaration.opened.slice(0, 80));

const ready = await evaluate(`(() => {
  const before = document.getElementById('columns-confirm').disabled;
  document.getElementById('sample-rate').value = '${SAMPLE_RATE_HZ}';
  document.getElementById('sample-rate').dispatchEvent(new Event('input'));
  return { before, after: document.getElementById('columns-confirm').disabled };
})()`);
check('the run cannot start until the rate these files were sampled at is stated',
  ready.before && !ready.after,
  `confirm ${ready.before ? 'blocked' : 'open'} with no rate, ${ready.after ? 'blocked' : 'open'} with one`);

await evaluate("document.getElementById('columns-confirm').click()");
await settle("!document.getElementById('stage-workspace').hidden", 'the workspace');
await settle(
  "document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric').length > 0"
    + " || document.querySelector('#analysis-warnings button')",
  'the first paint',
);

const offered = await evaluate(`(() => {
  const action = document.getElementById('run-folder');
  return { hidden: action.hidden, label: action.textContent };
})()`);
check('the folder is offered from the trial it was declared on',
  !offered.hidden && offered.label === `Run all ${trialNames.length} trials in this folder`,
  offered.label);

/*
 * The run started before the choices it needs are settled.
 *
 * The engine holds it open and the reader meets a panel naming what is missing. Everything
 * below this point runs on a settled path, so that panel is on nobody's way and it is the one
 * screen of the folder route where a first-time reader can be left with nothing to press.
 * Asked here, before the decisions, rather than never.
 */
await evaluate("document.getElementById('run-folder').click()");
await settle("Boolean(document.querySelector('#batch-result dt'))", 'the run held open for a choice');

const heldOpen = await evaluate(`(async () => {
  const { constructLabel } = await import('./registry.js');
  const openings = [...document.querySelectorAll('#batch-result button[data-construct]')]
    .map((node) => node.dataset.construct);
  return {
    terms: [...document.querySelectorAll('#batch-result dt')].map((node) => node.textContent),
    openings,
    spoken: openings.map((construct) => constructLabel(construct)),
    // The rail carries a row for each, so the control has somewhere to send the reader.
    onTheRail: openings.filter((construct) =>
      document.querySelector('#decision-list select[data-construct="' + construct + '"]')).length,
  };
})()`);
check('a run held open for a choice names the quantity in the words the rail names it by',
  heldOpen.terms.length > 0
    && heldOpen.spoken.length === heldOpen.terms.length
    && heldOpen.spoken.every((word, index) => word === heldOpen.terms[index])
    // A construct's id is the registry's spelling of it, and no label the field uses carries one.
    && heldOpen.terms.every((term) => !term.includes('_')),
  `${heldOpen.terms.length} open choices named ${heldOpen.terms.join('; ')}, ` +
    `${heldOpen.openings.length} of them offered, ${heldOpen.onTheRail} reachable on the rail`);

/*
 * The third route a construct id reaches a reader by, which no static scan can see.
 *
 * `check-web-names-no-construct.py` scans the interface modules and the registry's own prose.
 * This one is neither: the sentence is composed in Rust, crosses as JSON while the page is
 * running, and is rendered verbatim. A module scan passes clean while the id is on screen.
 *
 * Only spellings no English sentence produces are looked for, which is the same discipline the
 * static scan applies: `takeoff` and `landing` are construct ids and are also ordinary words,
 * and a pattern matching those reports the panel's own prose as a leak.
 */
const constructIds = [...(await readFile('registry/constructs.toml', 'utf8')).matchAll(/^id = "([^"]+)"/gm)]
  .map((match) => match[1]);
const identifierForms = [...new Set(constructIds.flatMap((id) => [id, id.split('.').pop()]))]
  .filter((token) => token.includes('_') || token.includes('.'));
const heldOpenText = await evaluate("document.getElementById('batch-result').innerText");
const spelt = identifierForms.filter((token) =>
  new RegExp(`(?<![\\w.])${token.replace(/\./g, '\\.')}(?![\\w.])`).test(heldOpenText));
check('a run held open for a choice spells no construct the registry declares',
  constructIds.length >= 40 && identifierForms.length >= 40 && heldOpenText.length > 0
    && spelt.length === 0,
  `${identifierForms.length} identifier forms from ${constructIds.length} declared constructs, ` +
    `against ${heldOpenText.length} characters on screen` +
    (spelt.length ? `, spelt: ${spelt.join(', ')}` : ''));

const wayOut = await evaluate(`(() => {
  const go = document.querySelector('#batch-result button[data-construct]');
  if (!go) return { pressed: false };
  const construct = go.dataset.construct;
  go.click();
  return {
    pressed: true,
    construct,
    onTheTrace: !document.getElementById('stage-workspace').hidden,
    focused: document.activeElement?.dataset?.construct ?? document.activeElement?.tagName ?? null,
  };
})()`);
check('a run held open for a choice opens that choice where it is made',
  wayOut.pressed && wayOut.onTheTrace && wayOut.focused === wayOut.construct,
  wayOut.pressed
    ? `${wayOut.construct}: ${wayOut.onTheTrace ? 'back on the trace' : 'still on the run'}, ` +
      `focus on ${wayOut.focused}`
    : 'the run named no open choice to press');

// A run under rules nobody chose is held open by the engine, so the decisions are made
// before the run rather than the refusal being read as the browser's own failure.
await evaluate(`(() => {
  const button = document.getElementById('accept-recommended');
  if (button) button.click();
  return Boolean(button);
})()`);

await evaluate("document.getElementById('run-folder').click()");
await settle("document.querySelector('#batch-result table.data tbody tr')", 'the batch table');

const table = await evaluate(`(() => {
  const read = (node) => [...node.querySelectorAll('tr')].map((r) => [...r.children].map((c) => c.textContent.trim()));
  const first = document.querySelector('#batch-result table.data');
  return {
    columns: read(first.querySelector('thead'))[0],
    rows: read(first.querySelector('tbody')),
    declaration: document.getElementById('batch-declaration').textContent,
    coverage: document.querySelector('#batch-result .panel__sub')?.textContent ?? '',
    summary: document.querySelector('#batch-result .batch-summary')?.textContent ?? '',
    tables: document.querySelectorAll('#batch-result table.data').length,
  };
})()`);

check('the browser draws a row per trial in the folder', table.rows.length === trialNames.length,
  `${table.rows.length} rows against ${trialNames.length} trials, columns ${table.columns.join(', ')}`);
check('the fingerprint each row was produced under is a column of the table',
  table.columns.includes('provenance_id'), table.columns.join(', '));

const withoutProvenance = await evaluate(`(() => {
  const toggle = document.getElementById('batch-provenance');
  toggle.checked = false;
  toggle.dispatchEvent(new Event('change'));
  const head = document.querySelector('#batch-result table.data thead tr');
  return [...head.children].map((c) => c.textContent.trim());
})()`);
check('the rendering without the provenance join is reachable from the same stage',
  !withoutProvenance.includes('provenance_id') && withoutProvenance.length === table.columns.length - 1,
  withoutProvenance.join(', '));

await evaluate(`(() => {
  const toggle = document.getElementById('batch-provenance');
  toggle.checked = true;
  toggle.dispatchEvent(new Event('change'));
})()`);

const declinedControl = await evaluate(`(() => {
  const toggle = document.getElementById('batch-declined');
  const before = document.querySelectorAll('#batch-result table.data').length;
  toggle.checked = false;
  toggle.dispatchEvent(new Event('change'));
  const hidden = document.querySelectorAll('#batch-result table.data').length;
  toggle.checked = true;
  toggle.dispatchEvent(new Event('change'));
  const restored = document.querySelectorAll('#batch-result table.data').length;
  const head = document.querySelector('#batch-result .batch-table thead th');
  const frame = document.querySelector('#batch-result .batch-table');
  return {
    before, hidden, restored,
    sticky: getComputedStyle(head).position,
    overflow: getComputedStyle(frame).overflowY,
  };
})()`);
check('declined trials can be isolated without leaving the batch stage',
  declinedControl.before > declinedControl.hidden && declinedControl.restored === declinedControl.before,
  `${declinedControl.before} tables shown, ${declinedControl.hidden} with declined hidden, ${declinedControl.restored} restored`);
check('the batch table keeps its headings while its rows scroll',
  declinedControl.sticky === 'sticky' && ['auto', 'scroll'].includes(declinedControl.overflow),
  `heading ${declinedControl.sticky}, rows overflow-y ${declinedControl.overflow}`);

/*
 * The account every number in the table gives of itself, read out of the panel a reader
 * opens rather than off the envelope behind it. An envelope carrying eighty-eight accounts
 * that no control reaches is the state this reads the rendered document to rule out.
 */
const pageAccounts = await evaluate(`(async () => {
  const read = [];
  const trials = document.querySelector('#batch-result table.data');
  for (const row of [...trials.querySelectorAll('tbody tr')]) {
    const named = row.children[0].textContent.trim();
    const control = row.querySelector('.row-record');
    if (!control) {
      read.push({ named, titled: null, opened: false, accounts: [] });
      continue;
    }
    control.click();
    read.push({
      named,
      titled: document.getElementById('drawer-title').textContent,
      opened: !document.getElementById('method-drawer').hidden,
      accounts: [...document.querySelectorAll('#drawer-body details.metric-account')].map((block) => [
        block.querySelector('summary').textContent,
        block.querySelector('pre').textContent,
      ]),
    });
    document.querySelector('#method-drawer [data-close-drawer]').click();
  }
  return read;
})()`);

/*
 * The same folder through the terminal, under the rules the page bound rather than under a
 * second set written here. A comparison between two different requests measures the
 * requests.
 */
const request = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const { buildRequest } = await import('./analysis.js');
  const built = buildRequest();
  const bound = ['weighing', 'onset', 'takeoff'].map((slot) => built[slot].method_id);
  // The terminal spells an assignment by the step, which is the slot word rather than the
  // construct id the derive flag takes.
  const values = [];
  for (const slot of ['weighing', 'onset', 'takeoff']) {
    for (const [name, value] of Object.entries(built[slot].parameters ?? {})) {
      values.push(slot + '.' + name + '=' + value);
    }
  }
  return { bound, values };
})()`);

const outDir = await mkdtemp(join(tmpdir(), 'plateforce-check-batch-'));
// A run over this folder ends 0 and the document it prints is complete. Most of these
// recordings end while the athlete is still off the plate, so no touchdown is in the record
// to find and the rules resting on flight time decline by name inside the result. That is the
// folder rather than a fault: the same trials are absent from the browser's table for the
// same reason, which is what this check compares.
//
// So the answer is read at 0 and at nothing else. Any other code is the terminal failing to
// produce a document at all, and it is raised carrying its code rather than swallowed,
// because a check that accepted every code would pass on a build that cannot run.
const terminal = JSON.parse(runBatch());

function runBatch() {
  try {
    return execFileSync(
      'cargo',
      [
        'run', '-q', '-p', 'plateforce-cli', '--',
        '--registry', 'registry', 'batch', FIXTURES,
        '--out-dir', outDir, '--trial-suffix', TRIAL_SUFFIX,
        '--column', '0', '--sample-rate-hz', String(SAMPLE_RATE_HZ), '--sentinel', 'none',
        '--weighing', request.bound[0], '--onset', request.bound[1], '--takeoff', request.bound[2],
        ...request.values.flatMap((assignment) => ['--set', assignment]),
        '--format', 'json',
      ],
      { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, env: { ...process.env, NO_COLOR: '1' } },
    );
  } catch (failure) {
    throw new Error(
      `the terminal batch ended ${failure.status}, and this check reads 0: ` +
        (failure.stderr || failure.message),
    );
  }
}

const terminalRows = new Map(
  (terminal.ok?.results ?? []).map((row) => [row.trial_id, row]),
);
const browserRows = new Map(table.rows.map((row) => [row[0], row]));
const quantityColumns = table.columns.slice(4);

let comparedCells = 0;
const disagreed = [];
for (const [trialId, row] of browserRows) {
  const counterpart = terminalRows.get(trialId);
  if (!counterpart) {
    disagreed.push(`${trialId} is on the page and not in the terminal's run`);
    continue;
  }
  quantityColumns.forEach((column, index) => {
    const shown = row[index + 4];
    const computed = counterpart.values?.[column];
    if (shown === '' && computed == null) return;
    comparedCells += 1;
    // Read back as numbers and compared at the precision the page printed, because the page
    // rounds each unit for display and the terminal writes the double. A byte comparison
    // would call that rounding a disagreement, and a fixed tolerance would call a genuine
    // divergence in a coarsely printed column agreement.
    const decimals = (shown.split('.')[1] ?? '').length;
    if (Math.abs(Number(shown) - Number(computed)) > 0.5 * 10 ** -decimals) {
      disagreed.push(`${trialId} ${column}: page ${shown}, terminal ${computed}`);
    }
  });
}

check('every trial the terminal ran is a row the browser drew',
  terminalRows.size === browserRows.size && terminalRows.size === trialNames.length,
  `terminal ${terminalRows.size}, browser ${browserRows.size}, folder ${trialNames.length}`);
check('every value on the page equals the value the terminal computed',
  disagreed.length === 0 && comparedCells >= trialNames.length,
  `${comparedCells} cells compared across ${browserRows.size} trials, ${disagreed.length} disagreed` +
    (disagreed.length ? `: ${disagreed.slice(0, 3).join('; ')}` : ''));
/*
 * The account each number gives of itself, on the page against the terminal's.
 *
 * Keyed by trial and quantity, which is the grain the engine writes them at: a run compared
 * trial by trial would pass while every quantity inside one carried another's account, and a
 * comparison of counts alone would pass while both surfaces held the same number of wrong
 * sentences. Compared character for character, because the account is one string from one
 * site and two surfaces rendering it differently is the defect, not a rounding.
 */
const at = (trial, quantity) => `${trial} ${quantity}`;
const terminalAccounts = new Map(
  (terminal.ok?.descriptions ?? []).map((row) => [at(row.trial_id, row.quantity), row.account]),
);
const pageAccountsAt = new Map();
for (const trial of pageAccounts) {
  for (const [quantity, account] of trial.accounts) pageAccountsAt.set(at(trial.named, quantity), account);
}

const unopened = pageAccounts.filter((trial) => !trial.opened || trial.titled !== trial.named);
check('every trial in the table opens the record for its own numbers',
  pageAccounts.length === trialNames.length && unopened.length === 0,
  `${pageAccounts.length} of ${trialNames.length} rows, ${pageAccounts.length - unopened.length} opening a panel titled with their own trial`);

const onlyTerminal = [...terminalAccounts.keys()].filter((key) => !pageAccountsAt.has(key));
const onlyPage = [...pageAccountsAt.keys()].filter((key) => !terminalAccounts.has(key));
const firstFew = (keys) => keys.slice(0, 3).join('; ');
check('every number the terminal accounted for is one the browser accounts for, and no other',
  terminalAccounts.size > 0 && onlyTerminal.length === 0 && onlyPage.length === 0,
  `${pageAccountsAt.size} on the page against ${terminalAccounts.size} in the terminal's run, ` +
    `${onlyTerminal.length} the page does not show${onlyTerminal.length ? ` (${firstFew(onlyTerminal)})` : ''}, ` +
    `${onlyPage.length} the terminal did not write${onlyPage.length ? ` (${firstFew(onlyPage)})` : ''}`);

const differing = [...terminalAccounts].filter(([key, account]) => pageAccountsAt.get(key) !== account);
const firstDifference = differing[0];
check('the account each number gives of itself reads the same in the browser as in the terminal',
  terminalAccounts.size > 0 && differing.length === 0,
  `${terminalAccounts.size - differing.length} of ${terminalAccounts.size} accounts identical` +
    (firstDifference
      ? `, first differing at ${firstDifference[0]}: page ${JSON.stringify(pageAccountsAt.get(firstDifference[0])).slice(0, 120)}, ` +
        `terminal ${JSON.stringify(firstDifference[1]).slice(0, 120)}`
      : ''));

/*
 * A trial's record on a phone.
 *
 * Read on the panel rather than on the document, because the panel is positioned against the
 * viewport and the document cannot be taken sideways by anything inside it: measured here,
 * with every account escaping its frame and widening the panel, the document reported no
 * horizontal overflow at all.
 */
await send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 2, mobile: true });
await new Promise((wait) => setTimeout(wait, 400));
const onAPhone = await evaluate(`(() => {
  document.querySelector('#batch-result .row-record').click();
  const body = document.getElementById('drawer-body');
  for (const account of document.querySelectorAll('#drawer-body details.metric-account')) account.open = true;
  const blocks = [...document.querySelectorAll('#drawer-body pre.account')];
  return {
    blocks: blocks.length,
    carriages: blocks.filter((block) => ['auto', 'scroll'].includes(getComputedStyle(block).overflowX)).length,
    widened: body.scrollWidth - body.clientWidth,
    reach: document.querySelector('#batch-result .row-record').getBoundingClientRect().height,
  };
})()`);
await send('Emulation.clearDeviceMetricsOverride');

check('at 390 px a trial’s record scrolls each account inside its own frame, not the panel',
  onAPhone.blocks > 0 && onAPhone.carriages === onAPhone.blocks && onAPhone.widened <= 0,
  `${onAPhone.carriages} of ${onAPhone.blocks} accounts carrying their own carriage, ` +
    `the panel ${onAPhone.widened} px wider than it shows, the row reaching ${Math.round(onAPhone.reach)} px`);

// Both numbers, because a line stating only the files a declared suffix kept reads as the
// whole folder, and the fixture folder holds files that are not traces.
check('the run states its coverage against the denominator it was taken over',
  table.declaration.includes(`${everyFixture.length} files chosen, ${trialNames.length} named as trials`)
    && table.declaration.includes(`${strays} left out`)
    && new RegExp(
      `${trialNames.length} of ${trialNames.length} trials analysed · \\d+ of ${trialNames.length} trials declined`,
    ).test(table.summary),
  `${table.summary}; ${table.declaration}`);

/*
 * The two populations a run counts, told apart.
 *
 * A rule that declines one quantity inside a trial that produced numbers is not a trial that
 * declined, and both were called declined, at the same size, eight lines apart. A run reading
 * "0 declined" over a list of six declines reads as a broken count, on a product whose whole
 * proposition is that the record can be trusted. Each count carries the denominator it was
 * taken over, and neither noun is the other's.
 */
const populations = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const ok = JSON.parse(state.run.envelope).ok;
  const rows = (ok.results ?? []).length;
  return {
    trials: rows,
    trialsDeclined: ok.run.refusal_count,
    quantitiesDeclined: (ok.refusals ?? []).length,
    asked: (ok.quantities ?? []).length * rows,
    summary: document.querySelector('#batch-result .batch-summary')?.textContent ?? '',
    headings: [...document.querySelectorAll('#batch-result h3')].map((node) => node.textContent),
    said: document.getElementById('batch-result').innerText,
  };
})()`);
check('a declined trial and a declined quantity are two counts, each carrying its denominator',
  populations.quantitiesDeclined > 0
    && populations.summary.includes(`${populations.trialsDeclined} of ${populations.trials} trials declined`)
    && populations.said.includes(
      `${populations.quantitiesDeclined} of ${populations.asked.toLocaleString()} quantities declined`,
    )
    && !populations.headings.includes('Declined trials'),
  `${populations.trialsDeclined} of ${populations.trials} trials declined and ` +
    `${populations.quantitiesDeclined} of ${populations.asked} quantities declined, ` +
    `under ${populations.headings.join(' / ') || 'no heading'}`);

const failures = results.filter((result) => !result.passed);
for (const result of results) {
  console.log(`${result.passed ? 'pass' : 'FAIL'}  ${result.name}\n      ${result.read}`);
}
if (consoleLines.length) console.log(`\nconsole errors:\n  ${consoleLines.join('\n  ')}`);
console.log(`\n${results.length - failures.length} of ${results.length} checks passed`);

socket.close();
server.close();
process.exit(failures.length || consoleLines.length ? 1 : 0);
