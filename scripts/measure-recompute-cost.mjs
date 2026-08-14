/*
 * What one analysis costs, measured on a running page rather than assumed.
 *
 * A readout that recomputes as a window is dragged asks the engine for an answer on every
 * pointer event, and whether that is affordable is a number nobody here had taken. Reporting a
 * frame rate that was not measured is the failure this file exists to avoid, so it prints the
 * distribution and the recording it was taken on, and decides nothing.
 *
 * Usage: node scripts/measure-recompute-cost.mjs <root directory> <port>
 */

import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { rmSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { chromeArguments, chromeExecutable, scratchDirectory } from './browser.mjs';

const [root, port] = [process.argv[2] || 'web', Number(process.argv[3] || 8801)];
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

const profile = scratchDirectory(`plateforce-recompute-${port}`);
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
  if (pending.has(message.id)) { pending.get(message.id)(message); pending.delete(message.id); }
});
const send = (method, params = {}) => new Promise((resolve) => {
  const id = (nextId += 1);
  pending.set(id, resolve);
  socket.send(JSON.stringify({ id, method, params }));
});
const evaluate = async (expression) => {
  const reply = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (reply.result?.exceptionDetails) throw new Error(JSON.stringify(reply.result.exceptionDetails));
  return reply.result.result.value;
};
const settle = async (expression) => {
  for (let attempt = 0; attempt < 80; attempt += 1) {
    try { if (await evaluate(expression)) return; } catch { /* not yet */ }
    await new Promise((resolve) => setTimeout(resolve, 125));
  }
  throw new Error(`timed out waiting for ${expression}`);
};

await send('Runtime.enable');
await send('Page.navigate', { url: `http://127.0.0.1:${port}/index.html` });
await settle("!document.getElementById('stage-empty').hidden");
await evaluate("document.getElementById('load-demo').click()");
await settle("!document.getElementById('stage-workspace').hidden");
await settle("document.querySelectorAll('#headline-metric-grid .metric').length > 0");

/*
 * One run per window, over windows that differ, so the compiler cannot answer from one cached
 * result and the figure is the cost of an analysis rather than the cost of a repeat.
 *
 * The whole round trip is timed: building the request, serialising it, the module's own work,
 * and parsing what comes back. A reader dragging pays all four, and timing only the middle one
 * would report a cost nobody incurs.
 */
const measure = async (label, howManyQuantities) => evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const analysis = await import('./analysis.js');
  const picker = await import('./add-quantity.js');
  const build = state.build;
  const windowRule = build.bindings.find((binding) => binding.id === 'window.stated.by_caller');
  for (const construct of build.derived_constructs.slice(0, ${howManyQuantities})) picker.addToPath(construct);
  picker.addToPath(windowRule.construct);

  const rate = state.info.sample_rate_hz;
  const last = state.info.sample_count - 1;
  const timings = [];
  for (let attempt = 0; attempt < 40; attempt += 1) {
    const from = (attempt * 7) % Math.max(1, last - 600);
    state.selection[windowRule.construct] = {
      methodId: windowRule.id,
      values: { start_seconds: from / rate, end_seconds: (from + 500) / rate },
      options: {}, unresolved: [],
      fromDefault: new Set(), recommended: new Set(),
      methodFromRecommendation: false, methodStated: true,
    };
    const request = JSON.stringify(analysis.buildRequest());
    const began = performance.now();
    JSON.parse(state.loadedTrial.analyse(request, state.fileName, null));
    timings.push(performance.now() - began);
  }
  timings.sort((low, high) => low - high);
  return {
    label: ${JSON.stringify(label)},
    constructs: state.path.length,
    rules: state.slots.length,
    samples: state.info.sample_count,
    rateHz: rate,
    median: timings[Math.floor(timings.length / 2)],
    worst: timings[timings.length - 1],
    best: timings[0],
    runs: timings.length,
  };
})()`);

const rows = [];
rows.push(await measure('the path a reader opens on', 0));
rows.push(await measure('eight quantities on the path', 8));
rows.push(await measure('twenty quantities on the path', 20));

/* A recording long enough that the cost cannot be read off a five-second demonstration. */
await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const wasm = await import('./pkg/plateforce_wasm.js');
  const rate = 1200;
  const rows = [];
  for (let index = 0; index < rate * 60; index += 1) {
    const phase = index % (rate * 5);
    let force = 600 + ((index % 17) - 8) * 0.4;
    if (phase > rate * 3 && phase < rate * 3.3) force = 600 - 300 * ((phase - rate * 3) / (rate * 0.3));
    if (phase >= rate * 3.3 && phase < rate * 3.6) force = 300 + 1500 * ((phase - rate * 3.3) / (rate * 0.3));
    if (phase >= rate * 3.6 && phase < rate * 4.2) force = 0;
    rows.push(force.toFixed(3));
  }
  const file = wasm.ForceFile.parse(rows.join('\\n'));
  state.loadedTrial = wasm.LoadedTrial.fromForceFile(file, 0, rate, 'none', null);
  state.info = JSON.parse(state.loadedTrial.infoJson());
  return state.info.sample_count;
})()`);
rows.push(await measure('a sixty-second recording, twenty quantities', 20));

const millisecond = (value) => `${value.toFixed(2)} ms`;
for (const row of rows) {
  console.log(
    `${row.label}\n` +
    `      ${row.samples.toLocaleString()} samples at ${row.rateHz} Hz, ${row.rules} rules bound, ` +
    `${row.runs} runs over different windows\n` +
    `      median ${millisecond(row.median)}, best ${millisecond(row.best)}, worst ${millisecond(row.worst)}, ` +
    `which is ${(1000 / row.median).toFixed(0)} answers a second at the median`,
  );
}

socket.close();
server.close();
process.exit(0);
