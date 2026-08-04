/*
 * A spread figure on screen says what it is a spread over.
 *
 * The panel reported a percentage and a count of combinations and no account of what was
 * combined, so a figure taken while the rule that computes the quantity stood still read
 * exactly like a figure taken over everything. On the demonstration trial it read 7.6 percent
 * while a published rule for the same quantity answered a value outside the range it printed.
 *
 * `check-minute.mjs` drives this same panel and passes either way: it asserts the spread is
 * populated, is the largest figure on the page, and that the swept setting moved the number,
 * and none of those can see an axis that was never offered. That is why this exists rather
 * than a line added there.
 *
 * Usage: node scripts/check-spread-scope.mjs <root directory> <port>
 */

import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';

const [root, port] = [process.argv[2] || 'web', Number(process.argv[3] || 8781)];
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
  '--disable-gpu', `--user-data-dir=/tmp/plateforce-check-spread-scope-${port}`, 'about:blank',
], { stdio: 'ignore', detached: true });
process.on('exit', () => {
  try { process.kill(-chrome.pid, 'SIGKILL'); } catch { /* already gone */ }
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
await send('Page.navigate', { url: `http://127.0.0.1:${port}/index.html` });

const settle = async (expression, label) => {
  let lastRaise = null;
  for (let attempt = 0; attempt < 80; attempt += 1) {
    try {
      if (await evaluate(expression)) return;
    } catch (raised) {
      lastRaise = raised;
    }
    await new Promise((resolve) => setTimeout(resolve, 125));
  }
  throw new Error(`timed out waiting for ${label}${lastRaise ? `, last raise: ${lastRaise.message}` : ''}`);
};

const results = [];
const check = (name, passed, read) => results.push({ name, passed, read });

await settle("!document.getElementById('stage-empty').hidden", 'the empty stage');
await evaluate("document.getElementById('load-demo').click()");
await settle("!document.getElementById('stage-workspace').hidden", 'the workspace');
await settle("document.querySelectorAll('#spread-result table.data tbody tr').length > 0", 'the spread panel');

const painted = await evaluate(`(() => {
  const host = document.getElementById('spread-result');
  const lines = [...host.querySelectorAll('.spread-scope p')].map((p) => p.textContent.trim());
  return {
    lines,
    headline: document.querySelector('.spread-headline__figure')?.textContent ?? null,
    envelopeAxes: (window.__spreadEnvelope?.axes_varied ?? []).length,
  };
})()`);

// The account is on the page a reader is looking at, not only in a document they would have
// to fetch. A figure with no account beside it is the state this check exists to forbid.
check(
  'the spread figure on screen is accompanied by what it was taken over',
  painted.lines.length > 0,
  `headline ${painted.headline ?? 'absent'}, ${painted.lines.length} scope lines`,
);

const varied = painted.lines.find((line) => line.startsWith('Varied '));
// Every held line, not the first. A run holds as many rules as it did not vary, and reading
// only the first asserts against whichever construct happens to sort earliest.
const heldLines = painted.lines.filter((line) => line.startsWith('Held '));
const held = heldLines.find((line) => line.includes('jump_height.takeoff_frame'));

check(
  'the steps the sweep varied are named, with how many rules each carried',
  Boolean(varied) && /\(\d+ rules\)/.test(varied ?? ''),
  varied ?? 'no varied line',
);

// The half that matters, and the one the JSON guards cover from the other side. On the
// demonstration trial nothing derived is bound, so the arithmetic runs under the spine's own
// default and the request names it nowhere. A reader who cannot see that reads the figure as
// wider than the set it came from.
check(
  'the arithmetic the figure was not taken over is named on screen, with the rule it was pinned to',
  Boolean(held) && held.includes('jumpheight.takeoff.'),
  held ?? `no held line for the arithmetic among ${heldLines.length}: ${heldLines.join(' | ')}`,
);

check(
  'every held line says what it means rather than only naming a rule',
  heldLines.length > 0 && heldLines.every((line) => line.includes('this spread is not over it')),
  `${heldLines.length} held lines: ${heldLines.join(' | ')}`,
);

for (const { name, passed, read } of results) {
  process.stdout.write(`${passed ? 'pass' : 'FAIL'}  ${name}\n      ${read}\n`);
}
const failed = results.filter((result) => !result.passed).length;
process.stdout.write(`\n${results.length - failed} of ${results.length} checks passed\n`);
server.close();
process.exit(failed === 0 ? 0 : 1);
