/*
 * The screens the guides show, captured from the running interface.
 *
 * Headless, from the same bundle a reader loads, so a picture in the guide is the page as
 * it is rather than as it was when somebody last looked. Run `scripts/build-web.sh` first:
 * a capture is a claim about `web/pkg/`, never about the source tree.
 *
 * It also writes `captured.json`, the numbers on screen at the moment of the capture, so the
 * builder can hold the guide's prose to the same run rather than to a memory of one.
 *
 * Usage: node docs/quickstart/capture.mjs [port]
 */

import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { rmSync, writeFileSync, mkdirSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize, resolve } from 'node:path';
import { chromeExecutable, scratchDirectory } from '../../scripts/browser.mjs';

const ROOT = resolve(import.meta.dirname, '..', '..');
const OUT = join(ROOT, 'docs/quickstart/img');
const WEB = join(ROOT, 'web');
const PORT = Number(process.argv[2] || 9701);
const WIDTH = 1360;
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };
const FIXTURES = join(ROOT, 'crates/plateforce-conformance/fixtures');
const TRIAL = (n) => join(FIXTURES, `subject01_trial${n}.force.txt`);

mkdirSync(OUT, { recursive: true });

const server = createServer(async (request, response) => {
  const target = request.url.split('?')[0];
  const path = join(WEB, normalize(target === '/' ? '/index.html' : target).replace(/^(\.\.[/\\])+/, ''));
  try {
    const body = await readFile(path);
    response.writeHead(200, { 'content-type': TYPES[extname(path)] || 'application/octet-stream' });
    response.end(body);
  } catch {
    response.writeHead(404).end('not found');
  }
});
await new Promise((ready) => server.listen(PORT, ready));

// Its own process group and a profile in memory, so a run that throws still takes its
// browser tree and its 160 MB of profile with it.
const profile = scratchDirectory(`plateforce-capture-${PORT}`);
const chrome = spawn(chromeExecutable(), [
  '--headless=new', `--remote-debugging-port=${PORT + 1}`, '--no-sandbox', '--disable-gpu',
  '--hide-scrollbars', `--user-data-dir=${profile}`, 'about:blank',
], { stdio: 'ignore', detached: true });

process.on('exit', () => {
  try { process.kill(-chrome.pid, 'SIGKILL'); } catch { /* already gone */ }
  try { rmSync(profile, { recursive: true, force: true }); } catch { /* already gone */ }
});

const targets = await (async () => {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try { return await (await fetch(`http://127.0.0.1:${PORT + 1}/json/list`)).json(); }
    catch { await new Promise((wait) => setTimeout(wait, 250)); }
  }
  throw new Error('chrome did not open a debugging port');
})();

const socket = new WebSocket(targets.find((target) => target.type === 'page').webSocketDebuggerUrl);
await new Promise((open) => socket.addEventListener('open', open));

let nextId = 0;
const pending = new Map();
socket.addEventListener('message', (event) => {
  const message = JSON.parse(event.data);
  if (pending.has(message.id)) { pending.get(message.id)(message); pending.delete(message.id); }
});
const send = (method, params = {}) => new Promise((answer) => {
  const id = (nextId += 1);
  pending.set(id, answer);
  socket.send(JSON.stringify({ id, method, params }));
});

const evaluate = async (expression) => {
  const reply = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (reply.result?.exceptionDetails) throw new Error(JSON.stringify(reply.result.exceptionDetails));
  return reply.result.result.value;
};

// A probe that reads through an element the page has not parsed yet raises rather than
// returning false, so a raise is the "not yet" it means and only the last one is reported.
const settle = async (expression, label) => {
  let raised = null;
  for (let attempt = 0; attempt < 160; attempt += 1) {
    try { if (await evaluate(expression)) return; } catch (error) { raised = error; }
    await new Promise((wait) => setTimeout(wait, 125));
  }
  throw new Error(`timed out waiting for ${label}${raised ? `, last raise: ${raised.message}` : ''}`);
};

const setFiles = async (selector, files) => {
  const document = await send('DOM.getDocument', { depth: -1 });
  const node = await send('DOM.querySelector', { nodeId: document.result.root.nodeId, selector });
  if (!node.result?.nodeId) throw new Error(`no input ${selector}`);
  await send('DOM.setFileInputFiles', { nodeId: node.result.nodeId, files });
};

/*
 * One picture, clipped to the union of the elements named.
 *
 * Clipping to real elements rather than to a fraction of the viewport is what keeps a figure
 * the size of what it shows: a page holds one tall rail and several wide panels, and a whole
 * viewport of either wastes most of a printed page on background.
 */
const shot = async (name, selectors, { pad = 16, bottom = pad } = {}) => {
  await new Promise((wait) => setTimeout(wait, 400));
  const box = await evaluate(`(() => {
    const nodes = ${JSON.stringify(selectors)}.map((s) => document.querySelector(s)).filter(Boolean);
    if (nodes.length === 0) return null;
    const boxes = nodes.map((n) => n.getBoundingClientRect());
    return {
      left: Math.min(...boxes.map((b) => b.left)) + scrollX,
      top: Math.min(...boxes.map((b) => b.top)) + scrollY,
      right: Math.max(...boxes.map((b) => b.right)) + scrollX,
      bottom: Math.max(...boxes.map((b) => b.bottom)) + scrollY,
    };
  })()`);
  if (!box) throw new Error(`nothing matched ${selectors.join(', ')} for ${name}`);
  const clip = {
    x: Math.max(0, box.left - pad),
    y: Math.max(0, box.top - pad),
    width: box.right - box.left + pad * 2,
    // The foot is its own measurement, because an even margin below the last element named
    // reaches into whatever follows it, and a heading caught by its top half reads as a
    // cropped screenshot exactly as a sliced line does.
    height: box.bottom - box.top + pad + bottom,
    scale: 2,
  };
  const reply = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true, clip });
  writeFileSync(join(OUT, `${name}.png`), Buffer.from(reply.result.data, 'base64'));
  console.log(`${name}.png  ${Math.round(clip.width)} x ${Math.round(clip.height)}`);
};

const openTrials = async (files, rateHz) => {
  await setFiles('#file-input', files);
  await evaluate("document.getElementById('file-input').dispatchEvent(new Event('change', { bubbles: true }))");
  await settle("!document.getElementById('stage-columns').hidden", 'the column stage');
  await evaluate(`(() => {
    const rate = document.getElementById('sample-rate');
    rate.value = '${rateHz}';
    rate.dispatchEvent(new Event('input', { bubbles: true }));
    rate.dispatchEvent(new Event('change', { bubbles: true }));
    const separator = document.getElementById('run-delimiter');
    if (separator && !separator.value) {
      separator.value = 'single';
      separator.dispatchEvent(new Event('change', { bubbles: true }));
    }
  })()`);
  await new Promise((wait) => setTimeout(wait, 300));
};

const enterWorkspace = async () => {
  await evaluate("document.getElementById('columns-confirm').click()");
  await settle("!document.getElementById('stage-workspace').hidden", 'the workspace');
  await settle(
    "document.querySelectorAll('#headline-metric-grid .metric').length > 0",
    'the first paint',
  );
  await new Promise((wait) => setTimeout(wait, 1200));
};

const chooseRecommended = async () => {
  const label = await evaluate(`(() => {
    const control = [...document.querySelectorAll('#decision-list button')]
      .find((node) => /recommend/i.test(node.textContent));
    if (control) control.click();
    return control ? control.textContent.trim() : null;
  })()`);
  await new Promise((wait) => setTimeout(wait, 1800));
  return label;
};

await send('Runtime.enable');
await send('DOM.enable');
await send('Page.enable');
// Headless Chrome reports a dark colour scheme, and the guides are printed on white.
await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }] });
await send('Emulation.setDeviceMetricsOverride', { width: WIDTH, height: 1000, deviceScaleFactor: 2, mobile: false });
await send('Page.navigate', { url: `http://127.0.0.1:${PORT}/index.html` });
await settle("!document.getElementById('stage-empty').hidden", 'the empty stage');

await shot('open', ['.app-header', '#dropzone']);

/*
 * The same screen as the desktop guides show it.
 *
 * The handle the application runs behind is injected and the page is loaded again, so the
 * install offer is removed by `web/startup.js` on the branch the application itself takes.
 * Editing the page for the photograph would produce a picture of a page nobody ever loads,
 * and it would keep printing the tidy version after that branch broke.
 */
const asApplication = await send('Page.addScriptToEvaluateOnNewDocument', {
  source: 'window.__TAURI_INTERNALS__ = { invoke: () => Promise.resolve() };',
});
await send('Page.navigate', { url: `http://127.0.0.1:${PORT}/index.html` });
await settle("!document.getElementById('stage-empty').hidden", 'the empty stage as an application');
await settle("!document.querySelector('.app-header__install')", 'the install offer to be withdrawn');
await shot('open-desktop', ['.app-header', '#dropzone']);

// Back to a browser for everything below, which both guides print: only the header differs
// between the two, and a figure shared by five guides should be taken the way five of them
// are read rather than the way one is.
await send('Page.removeScriptToEvaluateOnNewDocument', {
  identifier: asApplication.result.identifier,
});
await send('Page.navigate', { url: `http://127.0.0.1:${PORT}/index.html` });
await settle("!document.getElementById('stage-empty').hidden", 'the empty stage again');

await openTrials([TRIAL(1)], 1200);
await shot('columns', ['.panel--standalone']);

await enterWorkspace();
await shot('trace', ['.panel--trace']);
await shot('results-provisional', ['.panel--headlines']);
// The rail's head and its first open choice, whole. Two choices reach past a printed page
// at a readable size, and a figure that stops mid-row reads as a cropped screenshot.
await shot('decisions', ['.panel--decisions .panel__head', '#decision-list .decision'], { pad: 12 });

const recommended = await chooseRecommended();
await shot('results-settled', ['.panel--headlines']);
await settle(
  "document.querySelectorAll('#spread-result table.data tbody tr').length > 0"
    + " || document.querySelector('.spread-headline__figure')",
  'the spread panel',
);
await new Promise((wait) => setTimeout(wait, 900));
// The panel's own head as well as its controls: the guide tells the reader to look for that
// question by name, so a figure that starts below it hides the thing it was sent to find.
await shot('spread', ['.panel--spread .panel__head', '.spread-controls', '.spread-headline'], { pad: 20 });
await shot('record', ['.panel--build']);

const single = await evaluate(`(() => ({
  trial: document.getElementById('trial-summary')?.textContent?.replace(/\\s+/g, ' ').trim(),
  headlines: [...document.querySelectorAll('#headline-metric-grid .metric')].map((card) => ({
    label: card.querySelector('.metric__label')?.textContent?.trim(),
    value: card.querySelector('.metric__value')?.textContent?.replace(/\\s+/g, ' ').trim(),
    provisional: card.querySelector('.metric__provisional')?.textContent?.replace(/\\s+/g, ' ').trim() || null,
  })),
  spread: document.querySelector('.spread-headline__figure')?.textContent?.trim() || null,
  record: [...document.querySelectorAll('#build-info dt, #build-info dd')]
    .map((node) => node.textContent.replace(/\\s+/g, ' ').trim()),
  recommendedControl: ${JSON.stringify(recommended)},
}))()`);

// The folder, from a fresh page, so the run carries only the choices this pass made.
await send('Page.reload', { ignoreCache: true });
await settle("!document.getElementById('stage-empty').hidden", 'the empty stage again');
await openTrials([1, 2, 3, 4, 5, 6].map(TRIAL), 1200);
// The declaration alone, not the whole stage: the column and rate questions are the same ones
// step 2 already shows at full size, and repeating them shrinks the one thing this figure is
// here for, which is the ending that decides what counts as a trial.
await shot('folder', ['#run-declaration']);
await enterWorkspace();
await chooseRecommended();
await evaluate("document.getElementById('run-folder').click()");
await settle("!document.getElementById('stage-batch').hidden", 'the batch stage');
await settle("document.querySelectorAll('#batch-result table').length > 0", 'the batch table');
await new Promise((wait) => setTimeout(wait, 900));
// The scroll container rather than the table: a wide table is several times the width of what
// a reader can see, and clipping to the table captures mostly background. The line under it
// says how many columns are out there, which is the line that stops a reader taking the
// visible columns for all of them, so it is inside the picture rather than under its edge.
await shot('batch', [
  '#batch-result .panel__head',
  '.batch-summary',
  '#batch-result .table-scroll',
  '#batch-result .table-scroll + .panel__sub',
], { bottom: 0 });

const folder = await evaluate(`(() => ({
  declaration: document.getElementById('batch-declaration')?.textContent?.replace(/\\s+/g, ' ').trim(),
  summary: document.querySelector('.batch-summary')?.textContent?.replace(/\\s+/g, ' ').trim(),
  columns: [...document.querySelectorAll('#batch-result table thead th')].map((cell) => cell.textContent.trim()),
  rowCount: document.querySelectorAll('#batch-result table tbody tr').length,
}))()`);

writeFileSync(join(OUT, '..', 'captured.json'), JSON.stringify({ single, folder }, null, 2));
console.log('captured.json');
process.exit(0);
