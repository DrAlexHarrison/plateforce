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
import { createServer } from 'node:http';
import { readFile, readdir, mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { extname, join, normalize, resolve } from 'node:path';

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

const chrome = spawn('google-chrome', [
  '--headless=new', `--remote-debugging-port=${port + 1}`, '--no-sandbox',
  '--disable-gpu', `--user-data-dir=/tmp/plateforce-check-batch-${port}`, 'about:blank',
], { stdio: 'ignore', detached: true });
process.on('exit', () => {
  try { process.kill(-chrome.pid, 'SIGKILL'); } catch { /* already gone */ }
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
await settle("document.querySelectorAll('#metric-grid .metric').length > 0 || document.querySelector('#analysis-warnings button')", 'the first paint');

const offered = await evaluate(`(() => {
  const action = document.getElementById('run-folder');
  return { hidden: action.hidden, label: action.textContent };
})()`);
check('the folder is offered from the trial it was declared on',
  !offered.hidden && offered.label === `Run all ${trialNames.length} trials in this folder`,
  offered.label);

// A run under rules nobody chose is held open by the engine, so the decisions are made
// before the run rather than the refusal being read as the browser's own failure.
await evaluate(`(() => {
  const button = [...document.querySelectorAll('#decision-list button')]
    .find((b) => b.textContent.startsWith('Take the recommended'));
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
// A run over this folder ends 65, EX_DATAERR, and the document it prints is complete. Most
// of these recordings end while the athlete is still off the plate, so no touchdown is in
// the record to find and the rules resting on flight time decline by name. That is the
// folder rather than a fault: the same trials are absent from the browser's table for the
// same reason, which is what this check compares.
//
// So the answer is read at 0 and at 65 and at nothing else. Any other code is the terminal
// failing to produce a document at all, and it is raised carrying its code rather than
// swallowed, because a check that accepted every code would pass on a build that cannot run.
const RECORDING_DECLINED = 65;
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
    if (failure.status === RECORDING_DECLINED && failure.stdout) return failure.stdout;
    throw new Error(
      `the terminal batch ended ${failure.status}, and this check reads 0 or ${RECORDING_DECLINED}: ` +
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
// Both numbers, because a line stating only the files a declared suffix kept reads as the
// whole folder, and the fixture folder holds files that are not traces.
check('the run states its coverage against the denominator it was taken over',
  /files \d+, \d+ carrying a declared trial suffix and \d+ not/.test(table.coverage),
  table.coverage);

const failures = results.filter((result) => !result.passed);
for (const result of results) {
  console.log(`${result.passed ? 'pass' : 'FAIL'}  ${result.name}\n      ${result.read}`);
}
if (consoleLines.length) console.log(`\nconsole errors:\n  ${consoleLines.join('\n  ')}`);
console.log(`\n${results.length - failures.length} of ${results.length} checks passed`);

socket.close();
server.close();
process.exit(failures.length || consoleLines.length ? 1 : 0);
