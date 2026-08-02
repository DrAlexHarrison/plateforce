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
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';

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

const chrome = spawn('google-chrome', [
  '--headless=new', `--remote-debugging-port=${port + 1}`, '--no-sandbox',
  '--disable-gpu', `--user-data-dir=/tmp/plateforce-screen-check-${port}`, 'about:blank',
], { stdio: 'ignore' });

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

const consoleLines = [];
await send('Runtime.enable');
await send('Log.enable');
socket.addEventListener('message', (event) => {
  const message = JSON.parse(event.data);
  if (message.method === 'Log.entryAdded' && message.params.entry.level === 'error') {
    consoleLines.push(message.params.entry.text);
  }
});

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
  const button = [...document.querySelectorAll('#analysis-warnings button')]
    .find((b) => b.textContent.startsWith('Take the recommended'));
  if (!button) return null;
  const text = document.querySelector('#analysis-warnings strong')?.textContent ?? '';
  button.click();
  return text;
})()`);
await settle("document.querySelectorAll('#metric-grid .metric').length > 0", 'the metric grid');
// The sweep settles after the markers do, so reading the panel the instant the metrics
// appear reads it before it has run. Waiting for the rows is the difference between an
// instrument that reports a regression and one that causes it.
await settle(
  "!!document.querySelector('.spread-headline__figure')"
    + " || !!document.querySelector('#spread-result .notice')",
  'the spread panel',
);

const report = await evaluate(`(() => {
  const WALL = ${JSON.stringify(wall ?? 'numbers on first paint')};
  const stages = [...document.querySelectorAll('.stage')].map((s) => s.id + (s.hidden ? ' hidden' : ' SHOWN'));
  const metrics = [...document.querySelectorAll('#metric-grid .metric')].map((card) => [
    card.querySelector('.metric__label')?.textContent,
    card.querySelector('.metric__value')?.textContent,
    [...card.querySelectorAll('.provenance__name')].map((n) => n.textContent).join(' + '),
  ].join(' | '));
  const decisions = [...document.querySelectorAll('#decision-list .decision__title')].map((n) => n.textContent);
  const spread = document.querySelector('.spread-headline__figure')?.textContent ?? 'no spread';
  const rows = [...document.querySelectorAll('#spread-result table.data tbody tr')].map((r) =>
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

socket.close();
chrome.kill();
server.close();
process.exit(consoleLines.length ? 1 : 0);
