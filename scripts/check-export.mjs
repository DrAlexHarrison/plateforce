/*
 * The file a reader downloads from the batch stage, against the folder the terminal writes
 * for the same trials under the same declared format and the same stated rules.
 *
 * The route is driven rather than described: the files arrive on the drop zone as a real
 * drop event, the methods are stated on the rail a reader states them on, the download is a
 * real click captured through the browser's own download machinery, and the archive is
 * opened by python3's zipfile rather than by the code that wrote it. The comparison is byte
 * by byte, file by file, because the product's claim is that the tab's file and the
 * terminal's file are the same file.
 *
 * One column is compared under a mask, and the mask is asserted as tightly as the equality:
 * `source_path` in results.csv is the path as walked for the terminal and empty for a tab,
 * which has no filesystem to walk (identity.rs states this per source kind). Everything
 * else must match to the byte, run.json included.
 *
 * The single-trial workspace download is held to the same bar against a one-file folder
 * through the terminal, so one trial at a time and a folder at once end in the same file.
 *
 * Usage: node scripts/check-export.mjs <root directory> <port>
 */

import { spawn, execFileSync } from 'node:child_process';
import { rmSync, existsSync, mkdirSync } from 'node:fs';
import { createServer } from 'node:http';
import { readFile, readdir, mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { extname, join, normalize } from 'node:path';
import { listenForConsoleErrors } from './console-errors.mjs';

const [root, port] = [process.argv[2] || 'web', Number(process.argv[3] || 8761)];
const FIXTURES = 'crates/plateforce-conformance/fixtures';
const TRIAL_SUFFIX = '.force.txt';
const SAMPLE_RATE_HZ = 1200;
const SPINE_CONSTRUCTS = ['system_weight', 'movement_onset', 'takeoff'];
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };

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
await new Promise((settle) => server.listen(port, settle));

// Profile and downloads live in memory and go on every exit, the check-minute shape: these
// scripts run many times over while a guard is broken and put back.
const profile = `/dev/shm/plateforce-check-export-${port}`;
const downloads = `${profile}-downloads`;
mkdirSync(downloads, { recursive: true });
const chrome = spawn('google-chrome', [
  '--headless=new', `--remote-debugging-port=${port + 1}`, '--no-sandbox',
  '--disable-gpu', `--user-data-dir=${profile}`, 'about:blank',
], { stdio: 'ignore', detached: true });
process.on('exit', () => {
  try { process.kill(-chrome.pid, 'SIGKILL'); } catch { /* already gone */ }
  try { rmSync(profile, { recursive: true, force: true }); } catch { /* already gone */ }
  try { rmSync(downloads, { recursive: true, force: true }); } catch { /* already gone */ }
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

const openSocket = async (url) => {
  const socket = new WebSocket(url);
  await new Promise((open) => socket.addEventListener('open', open));
  let nextId = 0;
  const pending = new Map();
  const listeners = [];
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data);
    if (pending.has(message.id)) {
      pending.get(message.id)(message);
      pending.delete(message.id);
    }
    for (const listener of listeners) listener(message);
  });
  const send = (method, params = {}) =>
    new Promise((settle) => {
      const id = (nextId += 1);
      pending.set(id, settle);
      socket.send(JSON.stringify({ id, method, params }));
    });
  return { socket, send, listeners };
};

const page = await openSocket(targets.find((t) => t.type === 'page').webSocketDebuggerUrl);
const consoleLines = listenForConsoleErrors(page.socket);

// The download rides the browser target, not the page: the page socket cannot name where a
// file lands, and a check that read the Blob out of the module would prove a function while
// the control the reader presses stayed unproven.
const version = await (await fetch(`http://127.0.0.1:${port + 1}/json/version`)).json();
const browser = await openSocket(version.webSocketDebuggerUrl);
await browser.send('Browser.setDownloadBehavior', {
  behavior: 'allow', downloadPath: downloads, eventsEnabled: true,
});
const downloadNames = new Map();
let downloadsCompleted = 0;
browser.listeners.push((message) => {
  if (message.method === 'Browser.downloadWillBegin') {
    downloadNames.set(message.params.guid, message.params.suggestedFilename);
  }
  if (message.method === 'Browser.downloadProgress' && message.params.state === 'completed') {
    downloadsCompleted += 1;
  }
});

const evaluate = async (expression) => {
  const reply = await page.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
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

await page.send('Runtime.enable');
await page.send('Log.enable');
await page.send('Page.enable');

const everyFixture = (await readdir(FIXTURES)).sort();
const trialNames = everyFixture.filter((name) => name.endsWith(TRIAL_SUFFIX));

/* Drives the page from the drop to a computed run: drop the named files, declare the rate,
 * state the three spine methods on the rail (a change event on a select is the act
 * decisions.js records as stated, which is the claim the terminal's flags carry), and run
 * the folder. */
async function driveRun(names) {
  await page.send('Page.navigate', { url: `http://127.0.0.1:${port}/index.html` });
  await settle("!document.getElementById('stage-empty').hidden", 'the empty stage');

  await evaluate(`(async () => {
    const names = ${JSON.stringify(names)};
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
  await settle("!document.getElementById('stage-columns').hidden", 'the columns stage');

  await evaluate(`(() => {
    document.getElementById('sample-rate').value = '${SAMPLE_RATE_HZ}';
    document.getElementById('sample-rate').dispatchEvent(new Event('input'));
    document.getElementById('columns-confirm').click();
  })()`);
  await settle("!document.getElementById('stage-workspace').hidden", 'the workspace');
  await settle(
    "document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric').length > 0"
      + " || document.querySelector('#analysis-warnings button')",
    'the first paint',
  );

  const stated = await evaluate(`(() => {
    const constructs = ${JSON.stringify(SPINE_CONSTRUCTS)};
    const acted = [];
    for (const construct of constructs) {
      const select = document.querySelector('#decision-list select[data-construct="' + construct + '"]');
      if (!select) continue;
      // A slot still on its placeholder is given its first published rule; one already
      // showing an arrived-at rule is re-picked, which records the arrival as a choice.
      if (select.value === '') {
        const real = [...select.options].find((option) => option.value !== '');
        if (!real) continue;
        select.value = real.value;
      }
      select.dispatchEvent(new Event('change'));
      acted.push(construct + '=' + select.value);
    }
    return acted;
  })()`);
  check('the three spine methods are stated by an act on the rail',
    stated.length === SPINE_CONSTRUCTS.length, stated.join(', '));

  // Every value the run is holding open, stated through the rail's own control: a select
  // still on its placeholder is a decision nobody has made, and the engine refuses the
  // folder while one remains. Each act rebuilds the rail, so the walk restarts each round.
  const valuesStated = [];
  for (let round = 0; round < 24; round += 1) {
    const acted = await evaluate(`(() => {
      const open = [...document.querySelectorAll('#decision-list select')]
        .find((select) => select.value === '');
      if (!open) return null;
      const real = [...open.options].find((option) => option.value !== '');
      if (!real) return 'a placeholder with nothing to choose';
      open.value = real.value;
      open.dispatchEvent(new Event('change'));
      return (open.dataset.parameter || open.dataset.option || 'unknown') + '=' + real.value;
    })()`);
    if (acted === null) break;
    valuesStated.push(acted);
  }
  check('every value the run held open is stated on the rail',
    !valuesStated.includes('a placeholder with nothing to choose'),
    valuesStated.join(', ') || 'none were open');
}

/* What the page bound, read off its own request, so the terminal is told the same thing
 * rather than a second set written here. Only the values the page claims the reader stated
 * become flags: a registry default the reader never touched is left unspoken on both
 * surfaces, so both records claim the same acts. */
async function pageRequest() {
  return evaluate(`(async () => {
    const { state } = await import('./state.js');
    const { buildRequest } = await import('./analysis.js');
    const built = buildRequest();
    const values = [];
    const choices = [];
    for (const slot of ['weighing', 'onset', 'takeoff']) {
      const unstated = new Set([
        ...(built[slot].from_registry_default ?? []),
        ...(built[slot].recommended ?? []),
      ]);
      for (const [name, value] of Object.entries(built[slot].parameters ?? {})) {
        if (!unstated.has(name)) values.push(slot + '.' + name + '=' + value);
      }
      for (const [name, key] of Object.entries(built[slot].options ?? {})) {
        if (!unstated.has(name)) choices.push(slot + '.' + name + '=' + key);
      }
    }
    return {
      bound: ['weighing', 'onset', 'takeoff'].map((slot) => built[slot].method_id),
      values,
      choices,
      run: state.run && {
        delimiter: state.run.delimiter,
        sampleRateHz: state.run.sampleRateHz,
        sentinel: state.run.sentinel,
        endings: [...state.run.endings],
      },
      column: state.chosenColumn,
    };
  })()`);
}

/* One press of a download control, returned as the saved file's path once the browser has
 * finished writing it. */
async function capture(clickExpression) {
  const before = downloadsCompleted;
  const guidsBefore = new Set(downloadNames.keys());
  await evaluate(clickExpression);
  for (let attempt = 0; attempt < 240; attempt += 1) {
    if (downloadsCompleted > before) break;
    await new Promise((wait) => setTimeout(wait, 125));
  }
  if (downloadsCompleted === before) throw new Error('the press produced no completed download');
  const guid = [...downloadNames.keys()].find((key) => !guidsBefore.has(key));
  const name = downloadNames.get(guid);
  const path = join(downloads, name);
  for (let attempt = 0; attempt < 80 && !existsSync(path); attempt += 1) {
    await new Promise((wait) => setTimeout(wait, 125));
  }
  if (!existsSync(path)) throw new Error(`${name} was reported complete and is not at ${path}`);
  return { name, path };
}

/* The archive opened by a reader that did not write it, checksums included. */
function extract(zipPath, into) {
  mkdirSync(into, { recursive: true });
  return execFileSync('python3', ['-c', [
    'import sys, zipfile',
    'archive = zipfile.ZipFile(sys.argv[1])',
    'bad = archive.testzip()',
    "assert bad is None, f'checksum failed on {bad}'",
    'archive.extractall(sys.argv[2])',
    "print('\\n'.join(entry.filename for entry in archive.infolist()))",
  ].join('\n'), zipPath, into], { encoding: 'utf8' }).trim().split('\n');
}

/* The same trials through the terminal, told the same format and the same stated rules. */
function runTerminal(trialsDir, outDir, request) {
  const flags = [
    'run', '-q', '-p', 'plateforce-cli', '--',
    '--registry', 'registry', 'batch', trialsDir,
    '--out-dir', outDir,
    '--column', String(request.column),
    '--sample-rate-hz', String(request.run?.sampleRateHz ?? SAMPLE_RATE_HZ),
    '--sentinel', request.run?.sentinel == null ? 'none' : String(request.run.sentinel),
    '--weighing', request.bound[0], '--onset', request.bound[1], '--takeoff', request.bound[2],
    ...request.values.flatMap((assignment) => ['--set', assignment]),
    ...(request.choices ?? []).flatMap((assignment) => ['--choose', assignment]),
    '--format', 'json',
  ];
  for (const ending of request.run?.endings ?? [TRIAL_SUFFIX]) {
    flags.push('--trial-suffix', ending);
  }
  const delimiter = request.run?.delimiter ?? ' ';
  if (delimiter !== ' ') flags.push('--delimiter', delimiter);
  // Most committed recordings end while the athlete is still airborne, so the rules resting
  // on flight time decline by name inside a complete document and the run ends 0. A non-zero
  // code is a run that produced no document, raised rather than read.
  try {
    execFileSync('cargo', flags, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, env: { ...process.env, NO_COLOR: '1' } });
  } catch (failure) {
    if (failure.status !== 65) {
      throw new Error(`the terminal batch ended ${failure.status}: ${failure.stderr || failure.message}`);
    }
  }
}

/* Byte equality file by file, with the one masked column asserted as tightly as the rest. */
async function compareSets(tag, extractedDir, entryNames, terminalDir, walkedPrefix) {
  const terminalFiles = (await readdir(terminalDir)).sort();
  check(`${tag}: the archive holds the file set the terminal wrote`,
    JSON.stringify([...entryNames].sort()) === JSON.stringify(terminalFiles),
    `archive ${entryNames.join(', ')} · terminal ${terminalFiles.join(', ')}`);

  let identical = 0;
  const divergences = [];
  for (const name of terminalFiles) {
    const fromTab = await readFile(join(extractedDir, name));
    const fromTerminal = await readFile(join(terminalDir, name));
    if (name === 'results.csv') {
      const masked = maskSourcePath(tag, fromTab.toString('utf8'), fromTerminal.toString('utf8'), walkedPrefix);
      if (masked === true) identical += 1;
      else divergences.push(`${name}: ${masked}`);
      continue;
    }
    if (fromTab.equals(fromTerminal)) {
      identical += 1;
    } else {
      divergences.push(`${name}: ${firstDifference(fromTab, fromTerminal)}`);
    }
  }
  check(`${tag}: every file matches the terminal's byte for byte, source_path aside`,
    divergences.length === 0 && identical === terminalFiles.length,
    divergences.length ? divergences.join(' · ') : `${identical} of ${terminalFiles.length} files identical`);
}

/*
 * results.csv under the one mask: the tab's source_path cell is empty because a tab has no
 * path to walk, the terminal's is the walked path. Both shapes are asserted, so a mask that
 * hid anything else would fail rather than forgive.
 */
function maskSourcePath(tag, tabText, terminalText, walkedPrefix) {
  const tabLines = tabText.split('\n');
  const terminalLines = terminalText.split('\n');
  if (tabLines.length !== terminalLines.length) {
    return `tab wrote ${tabLines.length} lines, terminal ${terminalLines.length}`;
  }
  for (let index = 0; index < tabLines.length; index += 1) {
    if (tabLines[index] === terminalLines[index]) continue;
    if (index === 0 || tabLines[index] === '') return `line ${index + 1} differs beyond the mask`;
    const fromTab = tabLines[index].split(',');
    const fromTerminal = terminalLines[index].split(',');
    if (fromTab.length !== fromTerminal.length) {
      return `line ${index + 1}: tab ${fromTab.length} cells, terminal ${fromTerminal.length}`;
    }
    for (let cell = 0; cell < fromTab.length; cell += 1) {
      if (fromTab[cell] === fromTerminal[cell]) continue;
      const walked = fromTerminal[cell].startsWith(walkedPrefix) && fromTerminal[cell].endsWith(TRIAL_SUFFIX);
      if (cell !== 2 || fromTab[cell] !== '' || !walked) {
        return `line ${index + 1} cell ${cell + 1}: tab '${fromTab[cell]}', terminal '${fromTerminal[cell]}'`;
      }
    }
  }
  return true;
}

function firstDifference(a, b) {
  const length = Math.min(a.length, b.length);
  for (let at = 0; at < length; at += 1) {
    if (a[at] !== b[at]) {
      const from = Math.max(0, at - 40);
      return `first difference at byte ${at}: tab '${a.toString('utf8', from, at + 40)}' terminal '${b.toString('utf8', from, at + 40)}'`;
    }
  }
  return `one file is a prefix of the other, ${a.length} against ${b.length} bytes`;
}

/* run.json compared field by field when the bytes differ, so a red names the field. */
async function explainRunDifference(tag, extractedDir, terminalDir) {
  const fromTab = JSON.parse(await readFile(join(extractedDir, 'run.json'), 'utf8'));
  const fromTerminal = JSON.parse(await readFile(join(terminalDir, 'run.json'), 'utf8'));
  const keys = [...new Set([...Object.keys(fromTab), ...Object.keys(fromTerminal)])].sort();
  const differing = keys.filter(
    (key) => JSON.stringify(fromTab[key]) !== JSON.stringify(fromTerminal[key]),
  );
  if (differing.length) {
    const shown = differing.map(
      (key) => `${key}: tab ${JSON.stringify(fromTab[key])} terminal ${JSON.stringify(fromTerminal[key])}`,
    );
    check(`${tag}: run.json fields`, false, shown.join(' · '));
  }
}

// ---- The folder, through the batch stage. ----

await driveRun(everyFixture);
await evaluate("document.getElementById('run-folder').click()");
await settle("document.querySelector('#batch-result table.data tbody tr')", 'the batch table');

const offered = await evaluate(`(() => {
  const control = document.getElementById('batch-download');
  return { hidden: control.hidden, label: control.textContent };
})()`);
check('the batch stage offers one download control once the run has computed',
  !offered.hidden && offered.label === 'Download results (CSV)', offered.label);

const request = await pageRequest();
const batchZip = await capture("document.getElementById('batch-download').click()");
check('the saved file is named by the run it holds',
  /^plateforce-results-[A-Za-z0-9]+\.zip$/.test(batchZip.name), batchZip.name);

const work = await mkdtemp(join(tmpdir(), 'plateforce-check-export-'));
const batchExtracted = join(work, 'batch-tab');
const entryNames = extract(batchZip.path, batchExtracted);
check('python3 zipfile opens the archive and every checksum holds',
  entryNames.includes('results.csv') && entryNames[0] === 'run.json', entryNames.join(', '));

const batchTerminal = join(work, 'batch-terminal');
runTerminal(FIXTURES, batchTerminal, request);
await compareSets('folder', batchExtracted, entryNames, batchTerminal, FIXTURES);
if (!(await readFile(join(batchExtracted, 'run.json'))).equals(await readFile(join(batchTerminal, 'run.json')))) {
  await explainRunDifference('folder', batchExtracted, batchTerminal);
}

// ---- One trial, through the workspace. ----

const oneTrial = trialNames[0];
await driveRun([oneTrial]);
const workspaceControl = await evaluate(`(() => {
  const buttons = [...document.querySelectorAll('#result-actions button')];
  return buttons.map((button) => button.textContent);
})()`);
check('the workspace offers the same download beside Copy as Markdown',
  workspaceControl.includes('Download results (CSV)'), workspaceControl.join(' · '));

// The engine spells unit symbols for terminals; the page typesets them. A card showing the
// terminal spelling is a symbol that reached the DOM around the one rendering.
const unitTexts = await evaluate(`(() => {
  return [...document.querySelectorAll('#headline-metric-grid .metric small, #metric-grid .metric small')]
    .map((node) => node.textContent);
})()`);
const rawSpelled = unitTexts.filter((text) => /[A-Za-z]\.[A-Za-z]|[A-Za-z][23](?![0-9])/.test(text));
check('unit symbols reach the reader typeset, never in the terminal spelling',
  unitTexts.length > 0 && rawSpelled.length === 0,
  rawSpelled.length ? rawSpelled.join(' · ') : `${unitTexts.length} symbols read, none in the terminal spelling`);

// The cards this trial computes may carry no symbol the typesetting visibly changes, so the
// transform itself is asked in the running page: the two spellings a thesis would be marked
// down for, put through the module the sites import.
const transformed = await evaluate(`(async () => {
  const { typesetUnit } = await import('./format.js');
  return [typesetUnit('N.s'), typesetUnit('m/s2')];
})()`);
check('the typesetting turns the terminal spellings into the typographic ones',
  transformed[0] === 'N·s' && transformed[1] === 'm/s²', transformed.join(' · '));

// The DOM read above sees the cards it queries and nothing else, so the coverage claim is
// made against the source: every line that renders a symbol goes through the one typesetting.
const moduleFiles = (await readdir(root)).filter((name) => name.endsWith('.js'));
const unwrapped = [];
for (const name of moduleFiles) {
  const lines = (await readFile(join(root, name), 'utf8')).split('\n');
  lines.forEach((line, index) => {
    if (line.includes('unit_symbol') && !line.includes('typesetUnit')) {
      unwrapped.push(`${name}:${index + 1}`);
    }
  });
}
check('every source line rendering a unit symbol goes through the one typesetting',
  unwrapped.length === 0, unwrapped.join(' · ') || `${moduleFiles.length} modules scanned`);

const trialRequest = await pageRequest();
try {
const trialZip = await capture(`(() => {
  const control = [...document.querySelectorAll('#result-actions button')]
    .find((button) => button.textContent === 'Download results (CSV)');
  if (!control) throw new Error('the workspace offers no download control');
  control.click();
})()`);
check('one trial saves under its own name',
  trialZip.name === `plateforce-results-${oneTrial.replace(TRIAL_SUFFIX, '')}.zip`, trialZip.name);

const trialExtracted = join(work, 'trial-tab');
const trialEntries = extract(trialZip.path, trialExtracted);

// The terminal is pointed at a folder holding exactly that file, which is the run the
// workspace claims to have made: one trial, same format, same stated rules.
const trialFolder = join(work, 'one-trial');
mkdirSync(trialFolder);
execFileSync('cp', [join(FIXTURES, oneTrial), trialFolder]);
const trialTerminal = join(work, 'trial-terminal');
trialRequest.run = {
  delimiter: ' ', sampleRateHz: SAMPLE_RATE_HZ, sentinel: null, endings: [TRIAL_SUFFIX],
};
runTerminal(trialFolder, trialTerminal, trialRequest);
await compareSets('one trial', trialExtracted, trialEntries, trialTerminal, trialFolder);
if (!(await readFile(join(trialExtracted, 'run.json'))).equals(await readFile(join(trialTerminal, 'run.json')))) {
  await explainRunDifference('one trial', trialExtracted, trialTerminal);
}
} catch (raised) {
  check('one trial downloads the same table set a folder run writes', false, String(raised.message));
}

check('the page raised no console error', consoleLines.length === 0, consoleLines.join(' · ') || 'none');

// ---- The verdict, each check with what it read. ----

let failed = 0;
for (const { name, passed, read } of results) {
  if (!passed) failed += 1;
  process.stdout.write(`${passed ? 'ok ' : 'RED'} ${name}\n    ${read}\n`);
}
process.stdout.write(`${results.length - failed} of ${results.length} checks passed\n`);
rmSync(work, { recursive: true, force: true });
server.close();
process.exit(failed ? 1 : 0);
