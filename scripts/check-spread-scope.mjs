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
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { listenForConsoleErrors } from './console-errors.mjs';

const [root, port] = [process.argv[2] || 'web', Number(process.argv[3] || 8781)];
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };
const runFile = promisify(execFile);

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

// The profile lives in memory and is removed on every exit, the check-minute shape: each
// leaked /tmp profile is ~160 MB and these scripts run many times over while a guard is
// broken and put back.
const profile = `/dev/shm/plateforce-check-spread-scope-${port}`;
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

const painted = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const { buildRequest } = await import('./analysis.js');
  const host = document.getElementById('spread-result');
  const lines = [...host.querySelectorAll('.spread-scope p')].map((p) => p.textContent.trim());
  return {
    lines,
    headline: document.querySelector('.spread-headline__figure')?.textContent ?? null,
    spread: state.spread.result,
    request: buildRequest(),
    quantity: state.spread.quantity,
    boundMethods: state.analysis.bound_methods,
    methods: state.registry.methods,
  };
})()`);

// The account is on the page a reader is looking at, not only in a document they would have
// to fetch. A figure with no account beside it is the state this check exists to forbid.
check(
  'the spread figure on screen is accompanied by what it was taken over',
  painted.lines.length > 0,
  `headline ${painted.headline ?? 'absent'}, ${painted.lines.length} scope lines`,
);

const varied = painted.lines.find((line) => line.startsWith('Varied:'));
const fixed = painted.lines.find((line) => line.startsWith('Fixed:'));

check(
  'the steps the sweep varied are named, with how many rules each carried',
  Boolean(varied) && (varied.match(/\(\d+ rules\)/g) ?? []).length === 3,
  varied ?? 'no varied line',
);

// The half that matters, and the one the JSON guards cover from the other side. On the
// demonstration trial nothing derived is bound, so the arithmetic runs under the spine's own
// default and the request names it nowhere. A reader who cannot see that reads the figure as
// wider than the set it came from.
check(
  'the arithmetic the figure was not taken over is named on screen, with the rule it was pinned to',
  Boolean(fixed) && fixed.includes('jumpheight.takeoff.'),
  fixed ?? 'no fixed-rule line',
);

check(
  'the screen distinguishes varied rules from fixed rules without explanatory prose',
  Boolean(varied) && Boolean(fixed),
  painted.lines.join(' | '),
);

function addChoiceArguments(args, prefix, choice, flag) {
  if (!choice?.method_id) return;
  args.push(flag, flag === '--derive' || flag === '--condition'
    ? `${prefix}=${choice.method_id}`
    : choice.method_id);
  const parameters = { ...(choice.parameters || {}) };
  const bound = painted.boundMethods.find((method) => method.method_id === choice.method_id);
  for (const [name, value] of bound?.bound_parameters || []) {
    if (typeof value === 'number' && !(name in parameters)) parameters[name] = value;
  }
  const method = painted.methods.find((entry) => entry.id === choice.method_id);
  for (const parameter of method?.parameter || []) {
    if (Number.isFinite(parameter.default) && !(parameter.name in parameters)) {
      parameters[parameter.name] = parameter.default;
    }
  }
  for (const [name, value] of Object.entries(parameters)) {
    args.push('--set', `${prefix}.${name}=${value}`);
  }
  const options = { ...(choice.options || {}) };
  for (const parameter of method?.parameter || []) {
    if (parameter.default_key && !(parameter.name in options)) options[parameter.name] = parameter.default_key;
  }
  for (const [name, value] of Object.entries(options)) {
    args.push('--choose', `${prefix}.${name}=${value}`);
  }
  if (choice.manual_index != null) args.push('--place', `${prefix}=${choice.manual_index}`);
}

const cliArgs = [
  '--registry', 'registry', '--format', 'json', 'spread',
  'crates/plateforce-conformance/fixtures/subject01_trial1.force.txt',
  '--column', '0', '--sample-rate-hz', '1200', '--sentinel', 'none',
  '--quantity', painted.quantity,
];
addChoiceArguments(cliArgs, 'weighing', painted.request.weighing, '--weighing');
addChoiceArguments(cliArgs, 'onset', painted.request.onset, '--onset');
addChoiceArguments(cliArgs, 'takeoff', painted.request.takeoff, '--takeoff');
for (const [construct, choice] of Object.entries(painted.request.conditioning || {})) {
  addChoiceArguments(cliArgs, construct, choice, '--condition');
}
for (const [construct, choice] of Object.entries(painted.request.derived || {})) {
  addChoiceArguments(cliArgs, construct, choice, '--derive');
}
if (painted.request.gravity_meters_per_second_squared != null) {
  cliArgs.push('--gravity', String(painted.request.gravity_meters_per_second_squared));
}

let terminal = null;
let terminalError = null;
try {
  const { stdout } = await runFile('target/debug/plateforce', cliArgs, { maxBuffer: 16 * 1024 * 1024 });
  terminal = JSON.parse(stdout).ok ?? null;
} catch (error) {
  terminalError = `${error.stderr || error.message}\nargs: ${cliArgs.join(' ')}`;
}

const population = (spread) => (spread?.axes_varied || [])
  .map((axis) => `${axis.construct}:${axis.rules_varied}`)
  .sort();
const browserPopulation = population(painted.spread);
const terminalPopulation = population(terminal);
check(
  'the browser and terminal default to the same swept population',
  terminal != null && JSON.stringify(browserPopulation) === JSON.stringify(terminalPopulation),
  terminalError || `browser ${browserPopulation.join(', ')}; terminal ${terminalPopulation.join(', ')}`,
);
check(
  'the browser and terminal run the same number of combinations',
  terminal != null && painted.spread?.combinations_run === terminal.combinations_run,
  terminalError || `browser ${painted.spread?.combinations_run ?? 'none'}; terminal ${terminal?.combinations_run ?? 'none'}`,
);
// The sweep runs 512 requests through the module from one click, so a page that starts
// raising on the tenth of them still paints a table and every assertion above still reads.
check(
  'the page raised nothing while the sweep ran',
  consoleLines.length === 0,
  consoleLines.length ? consoleLines.join(' | ') : 'no console errors',
);

for (const { name, passed, read } of results) {
  process.stdout.write(`${passed ? 'pass' : 'FAIL'}  ${name}\n      ${read}\n`);
}
const failed = results.filter((result) => !result.passed).length;
process.stdout.write(`\n${results.length - failed} of ${results.length} checks passed\n`);
server.close();
process.exit(failed === 0 ? 0 : 1);
