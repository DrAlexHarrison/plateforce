/*
 * The minute, asserted rather than described.
 *
 * A first-time user opens the demonstration trial and reads a jump height without having
 * answered a question, sees which rule produced it, and sees how far the method choice
 * moves it. Every one of those is a claim about a running page, so this drives the page.
 *
 * Each check states what it read as well as whether it passed, because a check that only
 * prints "ok" cannot tell a working panel from an empty one it never looked at. The sweep
 * checks in particular assert that the swept setting moved the number, not that the sweep
 * ran: a sweep pointed at a setting nothing reads returns as many identical values as it
 * was asked for and reports success.
 *
 * Usage: node scripts/check-minute.mjs <root directory> <port>
 */

import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { validate } from './validate_palette.js';

const [root, port] = [process.argv[2] || 'web', Number(process.argv[3] || 8741)];
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
  '--disable-gpu', `--user-data-dir=/tmp/plateforce-check-minute-${port}`, 'about:blank',
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

const results = [];
const check = (name, passed, read) => results.push({ name, passed, read });

await settle("!document.getElementById('stage-empty').hidden", 'the empty stage');

const empty = await evaluate(`(() => ({
  heading: document.querySelector('#stage-empty h2')?.textContent ?? '',
  actions: [...document.querySelectorAll('#stage-empty .dropzone__actions button')].map((b) => b.textContent.trim()),
}))()`);
check('the first screen offers a file and a demonstration trial as peers',
  empty.heading === 'Drop a force trace here' && empty.actions.join(' / ') === 'Choose a file / Open a demo trial',
  `${empty.heading}: ${empty.actions.join(' / ')}`);

await evaluate("document.getElementById('load-demo').click()");
await settle("!document.getElementById('stage-workspace').hidden", 'the workspace');
await settle("document.querySelectorAll('#metric-grid .metric').length > 0 || document.querySelector('#analysis-warnings button')", 'the first paint');

const paint = await evaluate(`(() => {
  const card = [...document.querySelectorAll('#metric-grid .metric')]
    .find((c) => c.querySelector('.metric__label')?.textContent?.startsWith('Jump height'));
  return {
    wall: document.querySelector('#analysis-warnings button')?.textContent ?? null,
    jumpHeight: card?.querySelector('.metric__value')?.textContent ?? null,
    provisional: card?.querySelector('.metric__provisional')?.textContent?.replace(/\\s+/g, ' ').trim() ?? null,
    rule: [...(card?.querySelectorAll('.provenance__name') ?? [])].map((n) => n.textContent).join(' + '),
    spreadHeadline: document.querySelector('.spread-headline__figure')?.textContent ?? null,
    spreadRows: [...document.querySelectorAll('#spread-result table.data tbody tr')].map((r) =>
      [...r.children].map((c) => c.textContent.trim())),
    opening: document.getElementById('spread-opening')?.textContent ?? '',
    axes: [...document.querySelectorAll('#spread-axis-list label')].map((l) => ({
      label: l.textContent.trim(), construct: l.dataset.construct ?? '', ticked: l.querySelector('input').checked,
    })),
  };
})()`);

check('a jump height is on screen with no decision made',
  paint.wall === null && paint.jumpHeight != null,
  paint.wall ? `the wall is up: "${paint.wall}"` : `${paint.jumpHeight}, from ${paint.rule || 'no named rule'}`);

check('that value is marked provisional and names the rule that produced it',
  Boolean(paint.provisional) && paint.rule.length > 0,
  paint.provisional ?? 'no provisional line');

check('the spread panel is populated on that same first paint',
  paint.spreadRows.length > 1,
  `headline ${paint.spreadHeadline ?? 'absent'}, ${paint.spreadRows.length} rows`);

// The sweep checks below are about which setting the panel varies and whether varying it
// reaches the engine. They are independent of whether a decision has been resolved, so
// they are read after resolving one if the run stopped for one.
const resolveAnyWall = async () => {
  const wall = await evaluate(`(() => {
    const button = [...document.querySelectorAll('#analysis-warnings button')]
      .find((b) => b.textContent.startsWith('Take the recommended'));
    if (button) button.click();
    return Boolean(button);
  })()`);
  if (wall) await settle("document.querySelectorAll('#metric-grid .metric').length > 0", 'the metric grid');
};
await resolveAnyWall();

// Owen et al. 2014's onset rule, which is the one every measurement in this workstream's
// plan was taken on, and which runs sub-rules the reader was never asked about.
const OWEN_ONSET = 'onset.threshold.noise_relative';
await evaluate(`(() => {
  const select = document.querySelector('#decision-list select[data-construct="movement_onset"]');
  if (!select) return false;
  select.value = ${JSON.stringify(OWEN_ONSET)};
  select.dispatchEvent(new Event('change'));
  return true;
})()`);
// A rule picked on a forcing slot leaves its multi-valued parameters unresolved, which is
// the wall again. Taking the recommendation fills those and keeps the rule just picked.
await resolveAnyWall();

const sweep = await evaluate(`(() => ({
  opening: document.getElementById('spread-opening')?.textContent ?? '',
  axes: [...document.querySelectorAll('#spread-axis-list label')].map((l) => ({
    label: l.textContent.trim(), construct: l.dataset.construct ?? '', ticked: l.querySelector('input').checked,
  })),
  spreadRows: [...document.querySelectorAll('#spread-result table.data tbody tr')].map((r) =>
    [...r.children].map((c) => c.textContent.trim())),
  ranBeside: [...document.querySelectorAll('#decision-list .ran-beside__row')].map((r) => ({
    kind: [...r.classList].find((c) => c.startsWith('ran-beside__row--')) ?? '',
    text: r.textContent.replace(/\\s+/g, ' ').trim(),
  })),
}))()`);
Object.assign(paint, sweep);

// A default is legal under both these verdicts, and each owes the reader something the
// interface expressed nowhere before: displayed unasked, or named with its alternatives
// one interaction away.
const shown = paint.ranBeside.filter((row) => row.kind.endsWith('default-and-show'));
const onDemand = paint.ranBeside.filter((row) => row.kind.endsWith('surface-on-demand'));
check('a rule the registry says to display unasked is on screen with the value it used',
  shown.length > 0,
  shown.map((row) => row.text).join(' / ') || 'nothing displayed');
check('a rule the registry says to name is on screen with its alternatives one interaction away',
  onDemand.length > 0,
  onDemand.map((row) => row.text).join(' / ') || 'nothing named');

const ticked = paint.axes.filter((axis) => axis.ticked);
check('the setting the panel opened on is named on screen',
  paint.opening.length > 0 && ticked.length > 0,
  paint.opening || 'nothing named');

check('the panel opens varying the rule bound to the movement onset construct',
  ticked.length === 1 && ticked[0].construct === 'movement_onset',
  ticked.map((axis) => `${axis.construct || 'no construct'}: ${axis.label}`).join('; ') || 'nothing ticked');

// The sweep ran is not the claim. The claim is that the setting it swept reaches the
// engine, and the only evidence for that is a number that moved.
const swept = paint.spreadRows.map((row) => row[1]).filter(Boolean);
check('the swept setting moved the number',
  new Set(swept).size > 1,
  `${new Set(swept).size} distinct values across ${swept.length} rules: ${swept.join(', ')}`);

// Dragging a marker recomputes every dependent number, and the budget on that is a hard
// gate rather than a preference: a practitioner triggers it dozens of times a session, so
// the cost is paid dozens of times. Timed on the recompute the drag calls, over several
// placements, and reported with the split so a failure names where the time went.
const recompute = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const analysis = await import('./analysis.js');
  const onset = state.analysis.onset_index;
  const whole = [];
  for (const offset of [-200, -100, 100, 200]) {
    state.overrides.onset = onset + offset;
    const started = performance.now();
    analysis.runAnalysis();
    await new Promise((resolve) => requestAnimationFrame(() => resolve()));
    whole.push(performance.now() - started);
  }
  state.overrides.onset = null;
  const request = JSON.stringify(analysis.buildRequest());
  let started = performance.now();
  for (let i = 0; i < 10; i += 1) state.loadedTrial.analyse(request);
  const one = (performance.now() - started) / 10;
  const axes = [{
    slot: 'onset', parameter: null, values: [],
    method_ids: state.build.bindings.filter((b) => b.slot === 'onset').map((b) => b.id),
  }];
  const sweep = JSON.stringify({
    base: analysis.buildRequest(), axes,
    quantity_key: state.spread.quantity, maximum_combinations: 512,
  });
  started = performance.now();
  for (let i = 0; i < 5; i += 1) state.loadedTrial.spread(sweep);
  const panel = (performance.now() - started) / 5;
  analysis.runAnalysis();
  return { whole, one, panel, rules: axes[0].method_ids.length };
})()`);
const slowest = Math.max(...recompute.whole);
check('the recompute a marker drag triggers lands inside 100 ms',
  slowest < 100,
  `slowest of ${recompute.whole.length} placements ${slowest.toFixed(1)} ms; one analysis ` +
  `${recompute.one.toFixed(1)} ms, the spread over ${recompute.rules} rules ${recompute.panel.toFixed(1)} ms`);

// The three landmark tracks are read back from the running page rather than from the
// stylesheet, so what is checked is what renders, in the theme it renders in.
for (const theme of ['light', 'dark']) {
  const tokens = await evaluate(`(() => {
    document.documentElement.dataset.theme = ${JSON.stringify(theme)};
    const read = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return ['--track-onset', '--track-takeoff', '--track-touchdown'].map(read);
  })()`);
  const { report, ok } = validate(tokens, {
    mode: theme,
    surface: theme === 'dark' ? '#12171c' : '#ffffff',
    pairs: 'all',
  });
  const worst = report.filter(([name]) => name.includes('vision') || name.includes('CVD') || name.includes('Lightness'));
  check(`the three landmark tracks pass every computable check in ${theme}`,
    ok,
    `${tokens.join(', ')} | ${worst.map(([name, , detail]) => `${name}: ${detail}`).join(' | ')}`);
}
await evaluate("document.documentElement.dataset.theme = 'auto'");

check('no console errors', consoleLines.length === 0, consoleLines.join(' | ') || 'none');

const failed = results.filter((result) => !result.passed);
for (const result of results) {
  console.log(`${result.passed ? 'pass' : 'FAIL'}  ${result.name}\n        ${result.read}`);
}
console.log(`\n${results.length - failed.length} of ${results.length} checks passed`);

socket.close();
chrome.kill();
server.close();
process.exit(failed.length ? 1 : 0);
