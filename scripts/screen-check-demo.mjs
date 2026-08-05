/*
 * What the demo trial renders, as text a diff can read.
 *
 * The browser is the one surface no cargo command reaches, so a change to `web/` is proved
 * by loading the page, opening the demo trial and reading back which stages rendered and
 * what every metric card says. Chrome is driven over the DevTools protocol with no
 * dependencies beyond node's own WebSocket.
 *
 * Usage: node scripts/screen-check-demo.mjs <root directory> <port>
 */

import { spawn } from 'node:child_process';
import { rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { listenForConsoleErrors } from './console-errors.mjs';

const [root, port] = [process.argv[2], Number(process.argv[3] || 8731)];
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };

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
await new Promise((resolve) => server.listen(port, resolve));

const profile = `/dev/shm/plateforce-screen-check-${port}`;
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
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
  }
  throw new Error('chrome did not open a debugging port');
})();

const socket = new WebSocket(targets.find((t) => t.type === 'page').webSocketDebuggerUrl);
await new Promise((resolve) => socket.addEventListener('open', resolve));

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
  new Promise((resolve) => {
    const id = (nextId += 1);
    pending.set(id, resolve);
    socket.send(JSON.stringify({ id, method, params }));
  });

const evaluate = async (expression) => {
  const reply = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (reply.result?.exceptionDetails) throw new Error(JSON.stringify(reply.result.exceptionDetails));
  return reply.result.result.value;
};

await send('Runtime.enable');
await send('Log.enable');

await send('Page.navigate', { url: `http://127.0.0.1:${port}/index.html` });

const settle = async (expression, label) => {
  for (let attempt = 0; attempt < 80; attempt += 1) {
    if (await evaluate(expression)) return;
    await new Promise((resolve) => setTimeout(resolve, 125));
  }
  throw new Error(`timed out waiting for ${label}`);
};

await settle("!!document.getElementById('load-demo') && !document.getElementById('stage-empty').hidden", 'the empty stage');
await evaluate("document.getElementById('load-demo').click()");
await settle("!document.getElementById('stage-workspace').hidden", 'the workspace');

// On this trial both weighing and onset force a decision, so the first paint is the wall
// rather than a number. Taking the recommendation is the act a user performs, and it is
// what the numbers below are the numbers for.
const wall = await evaluate(`(() => {
  const button = document.getElementById('accept-recommended');
  if (!button) return null;
  const text = document.querySelector('#analysis-warnings strong')?.textContent ?? '';
  button.click();
  return text;
})()`);
await settle(
  "document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric').length > 0",
  'the metric grids',
);
// The sweep settles after the markers do, so reading the panel the instant the metrics
// appear reads it before it has run. Waiting for the rows is the difference between an
// instrument that reports a regression and one that causes it.
await settle(
  "!!document.querySelector('.spread-headline__figure')",
  'the spread panel',
);

const screenshotDirectory = process.env.PLATEFORCE_SCREENSHOT_DIR;
if (screenshotDirectory) {
  const screenshotSet = process.env.PLATEFORCE_SCREENSHOT_SET || 'full';
  await mkdir(screenshotDirectory, { recursive: true });
  const capture = async (name, width, height, deviceScaleFactor, prepare) => {
    await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor, mobile: width <= 640 });
    await evaluate(`window.scrollTo(0, 0)`);
    if (prepare) await evaluate(prepare);
    await new Promise((resolve) => setTimeout(resolve, 200));
    const reply = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
    const path = join(screenshotDirectory, name);
    await writeFile(path, Buffer.from(reply.result.data, 'base64'));
    console.log(`screenshot: ${path}`);
  };

  if (screenshotSet === 'full') {
    await capture('desktop-overview.png', 1440, 900, 1, `(() => {
      document.getElementById('method-drawer').hidden = true;
      window.scrollTo(0, 0);
    })()`);
    await capture('desktop-spread.png', 1440, 900, 1, `(() => {
      document.getElementById('method-drawer').hidden = true;
      document.querySelector('.panel--spread').scrollIntoView({ block: 'start' });
    })()`);
  }
  await capture('mobile-results.png', 390, 844, 2, `(() => {
    document.getElementById('method-drawer').hidden = true;
    document.querySelector('.panel--headlines').scrollIntoView({ block: 'start' });
  })()`);
  await capture('mobile-method-record.png', 390, 844, 2, `(() => {
    document.querySelector('#headline-metric-grid .metric-record').click();
  })()`);
  await send('Emulation.clearDeviceMetricsOverride');
}

const report = await evaluate(`(() => {
  const WALL = ${JSON.stringify(wall ?? 'numbers on first paint')};
  const stages = [...document.querySelectorAll('.stage')].map((s) => s.id + (s.hidden ? ' hidden' : ' SHOWN'));
  const metrics = [...document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric')].map((card) => [
    card.querySelector('.metric__label')?.textContent,
    card.querySelector('.metric__value')?.textContent,
    [...card.querySelectorAll('.provenance__name')].map((n) => n.textContent).join(' + '),
  ].join(' | '));
  const decisions = [...document.querySelectorAll('#decision-list .decision__title')].map((n) => n.textContent);
  const spread = document.querySelector('.spread-headline__figure')?.textContent ?? 'no spread';
  const rows = [...document.querySelectorAll('#spread-result .spread-summary table.data tbody tr')].map((r) =>
    [...r.children].map((c) => c.textContent).join(' | '));
  return [
    'first paint: ' + WALL,
    'stages: ' + stages.join(', '),
    'trial: ' + document.getElementById('trial-summary').textContent,
    'decision rows: ' + decisions.join(', '),
    'metrics (' + metrics.length + '):',
    ...metrics.map((m) => '  ' + m),
    'spread headline: ' + spread,
    'spread rows (' + rows.length + '):',
    ...rows.map((r) => '  ' + r),
  ].join('\\n');
})()`);

console.log(report);
if (consoleLines.length) console.log('console errors:\n' + consoleLines.join('\n'));

// A page that renders NaN into every card logs nothing, so console errors alone certify it.
// The report is printed above and reads like a checked table, which is the reason to check
// it rather than to trust that a reader will.
const complaints = [];
const notFinite = report.split('\n').filter((line) => /\b(NaN|Infinity|undefined|null)\b/.test(line));
if (notFinite.length) complaints.push('a rendered value is not a number:\n' + notFinite.join('\n'));

const metricCount = Number((report.match(/^metrics \((\d+)\):$/m) || [])[1]);
if (!(metricCount > 0)) complaints.push('no metric cards rendered, so nothing above was measured');

// A spread is a percentage of a median. Outside this band the page is not reporting a
// spread, whatever it printed, and the band is wide enough that no real trial approaches it.
const headline = Number((report.match(/^spread headline: (-?[\d.]+)%$/m) || [])[1]);
if (!Number.isFinite(headline) || headline < 0 || headline > 500) {
  complaints.push(`the spread headline reads ${headline}, which is not a percentage of a median`);
}

if (complaints.length) console.log('rendered values:\n' + complaints.join('\n'));

socket.close();
server.close();
process.exit(consoleLines.length || complaints.length ? 1 : 0);
