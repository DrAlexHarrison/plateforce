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

// Its own process group, so the browser and every renderer it spawns go together. A
// browser is a tree, and terminating the process that was launched leaves the rest of it
// running; this script is meant to be run many times over while a guard is broken and put
// back, and one leaked tree per run reaches the hundreds.
const chrome = spawn('google-chrome', [
  '--headless=new', `--remote-debugging-port=${port + 1}`, '--no-sandbox',
  '--disable-gpu', `--user-data-dir=/tmp/plateforce-check-minute-${port}`, 'about:blank',
], { stdio: 'ignore', detached: true });

// On every exit rather than on the one at the bottom: a check that times out waiting for
// the page leaves through a thrown error, which is exactly the run whose browser nobody
// closes.
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
// The panel settles before it sweeps, so that it is not recomputing five alternatives on
// every frame of a drag. It is a settling window rather than a gate: nothing about it is
// conditioned on a decision.
await settle("document.querySelectorAll('#spread-result table.data tbody tr').length > 0", 'the spread panel');

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
  await settle("document.querySelectorAll('#spread-result table.data tbody tr').length > 0", 'the spread panel');
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
await settle("document.querySelectorAll('#spread-result table.data tbody tr').length > 0", 'the spread panel');

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
check('a rule the registry says to name is on screen',
  onDemand.length > 0,
  onDemand.map((row) => row.text).join(' / ') || 'nothing named');

// The row existing is half the verdict. The other half is that the alternatives are one
// interaction away, so the check takes the interaction rather than reading the row and
// claiming what the row does not say. The alternative ids come back so a pass cannot be
// an empty list rendered under a heading.
const alternatives = await evaluate(`(() => {
  const row = document.querySelector('#decision-list .ran-beside__row--surface-on-demand');
  if (!row) return { reached: false, why: 'no rule on screen under this verdict' };
  const button = [...row.querySelectorAll('button')].pop();
  if (!button) return { reached: false, why: 'the row names no interaction' };
  button.click();
  const drawer = document.getElementById('method-drawer');
  const named = [...document.querySelectorAll('#drawer-body li')]
    .map((item) => item.textContent.trim())
    .filter((text) => /^[a-z_]+(\\.[a-z_0-9]+)+/.test(text));
  return { reached: true, open: drawer && !drawer.hidden, label: button.textContent.trim(), named };
})()`);
check('and its alternatives are one interaction away, named by id',
  alternatives.reached && alternatives.open && alternatives.named.length > 0,
  alternatives.reached
    ? `"${alternatives.label}" opens ${alternatives.named.length} alternatives: ${alternatives.named.map((t) => t.split(' ')[0]).join(', ') || 'none'}`
    : alternatives.why);
await evaluate(`(() => { document.getElementById('method-drawer').hidden = true; return true; })()`);

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
  let saidSoMidDrag = null;
  for (const offset of [-200, -100, 100, 200]) {
    state.overrides.onset = onset + offset;
    const started = performance.now();
    analysis.runAnalysis();
    await new Promise((resolve) => requestAnimationFrame(() => resolve()));
    whole.push(performance.now() - started);
    const host = document.getElementById('spread-result');
    if (host.dataset.forPreviousPosition === 'true') {
      saidSoMidDrag = host.querySelector('.notice strong')?.textContent ?? 'marked';
    }
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
  return { whole, one, panel, saidSoMidDrag, slowest: Math.max(...whole), rules: axes[0].method_ids.length };
})()`);
const slowest = Math.max(...recompute.whole);
/*
 * Two claims, and only one of them is about this machine.
 *
 * The design property is that a drag recomputes the number and not the panel, so the drag
 * costs about one analysis rather than an analysis plus a sweep. That ratio holds whatever
 * else the machine is doing. The 100 ms budget is wall-clock and it is only meaningful when
 * one analysis is itself well inside it: measured on a build machine under load, the same
 * wasm on the same trial took 131 ms for a single analysis against 18 ms quiet, so an
 * absolute assertion there would report a regression that is not in the code. When the
 * machine cannot answer the question, the check says so rather than failing or passing.
 */
const ratio = recompute.slowest / Math.max(recompute.one, 0.001);
const measurable = recompute.one < 40;
check('a marker drag recomputes the number and not the panel',
  ratio < 3 && (!measurable || slowest < 100),
  `slowest of ${recompute.whole.length} placements ${slowest.toFixed(1)} ms, ` +
  `${ratio.toFixed(1)} times one analysis at ${recompute.one.toFixed(1)} ms; the spread it no ` +
  `longer waits for costs ${recompute.panel.toFixed(1)} ms. ` +
  (measurable
    ? `The 100 ms budget applies and is ${slowest < 100 ? 'met' : 'missed'}.`
    : `One analysis alone is over 40 ms here, so the 100 ms budget is not measurable on this machine right now.`));

// The budget is met by computing less, never by drawing something older without saying so.
// While the marker is moving the panel holds figures for a position the reader has left.
check('while the marker is moving the panel says which position its figures are for',
  Boolean(recompute.saidSoMidDrag),
  recompute.saidSoMidDrag || 'the panel kept drawing the previous figures as current');

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

// The narrow viewport, where a layout that merely reflows on a desktop breaks. Four
// mechanical floors, each read off the rendered box rather than off the stylesheet.
await send('Emulation.setDeviceMetricsOverride', {
  width: 390, height: 844, deviceScaleFactor: 2, mobile: true,
});
await new Promise((resolve) => setTimeout(resolve, 400));
const narrow = await evaluate(`(() => {
  const visible = (node) => node.getClientRects().length > 0;
  const boxes = [...document.querySelectorAll('button, select, input, a[href], summary')].filter(visible);
  // The target is the box a finger lands on. A tick box inside a label is toggled by the
  // whole label, and a link inside a sentence is a word in running text rather than a
  // control laid out on its own. Everything else is measured as it renders.
  const target = (node) => {
    if (/^(checkbox|radio)$/.test(node.type) && node.closest('label')) return node.closest('label');
    return node;
  };
  const inRunningText = (node) => node.tagName === 'A' &&
    [...(node.parentElement?.childNodes ?? [])].some((c) => c.nodeType === 3 && c.textContent.trim());
  return {
    overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    small: boxes
      .filter((node) => !inRunningText(node))
      .map((node) => [node.tagName.toLowerCase() + (node.id ? '#' + node.id : ''), target(node).getBoundingClientRect()])
      .map(([what, box]) => [what, Math.round(Math.min(box.width, box.height))])
      .filter(([, side]) => side < 44),
    tiny: [...document.querySelectorAll('body *')].filter(visible)
      .filter((node) => [...node.childNodes].some((child) => child.nodeType === 3 && child.textContent.trim()))
      .map((node) => [node.className || node.tagName.toLowerCase(), parseFloat(getComputedStyle(node).fontSize)])
      .filter(([, size]) => size < 12),
    unlabelled: boxes
      .filter((node) => !node.textContent.trim() && !node.getAttribute('aria-label') && !node.getAttribute('title') && !node.labels?.length)
      .map((node) => node.tagName.toLowerCase() + (node.id ? '#' + node.id : '')),
    counted: boxes.length,
  };
})()`);
await send('Emulation.clearDeviceMetricsOverride');

check('at 390 px the page does not scroll sideways', narrow.overflow <= 0, `${narrow.overflow} px of horizontal overflow`);
check('at 390 px every control clears 44 px on its short side',
  narrow.small.length === 0,
  narrow.small.length ? narrow.small.map(([what, side]) => `${what} ${side} px`).join(', ') : `${narrow.counted} controls checked`);
check('at 390 px no text renders under 12 px',
  narrow.tiny.length === 0,
  narrow.tiny.length ? narrow.tiny.map(([what, size]) => `${what} ${size} px`).join(', ') : 'none under 12 px');
check('every control carries a name a screen reader can read',
  narrow.unlabelled.length === 0,
  narrow.unlabelled.join(', ') || `${narrow.counted} controls checked`);

// Where each bound value came from. The severest defect this build can carry is a
// fingerprint claiming the reader chose a value they were never shown, so the record is
// read back off the running page after each of the three acts that write it.
const provenance = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const analysis = await import('./analysis.js');
  const workspace = await import('./workspace.js');
  const names = (set) => [...(set ?? [])].sort();

  // Back to a trial just opened, through the path a reader takes rather than by resetting
  // the state directly, so what is read back is what a reader would have.
  workspace.enterWorkspace();
  const beforeAnyAct = Object.fromEntries(state.slots.map((slot) => [slot.construct, {
    method: state.selection[slot.key].methodId,
    fromDefault: names(state.selection[slot.key].fromDefault),
    recommended: names(state.selection[slot.key].recommended),
  }]));

  analysis.acceptRecommended();
  const afterTakingTheRecommendation = Object.fromEntries(state.slots.map((slot) => [slot.construct, {
    fromDefault: names(state.selection[slot.key].fromDefault),
    recommended: names(state.selection[slot.key].recommended),
    methodFromRecommendation: state.selection[slot.key].methodFromRecommendation === true,
  }]));

  const select = [...document.querySelectorAll('#decision-list .decision')]
    .find((row) => row.querySelector('select[data-construct="movement_onset"]'))
    ?.querySelector('.param select');
  let afterPickingByHand = null;
  if (select) {
    const picked = select.id.replace(/^param-[a-z_]+-/, '');
    select.value = select.options[select.options.length - 1].value;
    select.dispatchEvent(new Event('change'));
    const onset = state.slots.find((slot) => slot.construct === 'movement_onset');
    afterPickingByHand = {
      picked,
      fromDefault: names(state.selection[onset.key].fromDefault),
      recommended: names(state.selection[onset.key].recommended),
      untouched: Object.keys(state.selection[onset.key].values).filter((name) => name !== picked),
    };
  }
  return { beforeAnyAct, afterTakingTheRecommendation, afterPickingByHand };
})()`);

const onsetBefore = provenance.beforeAnyAct.movement_onset;
const onsetAfter = provenance.afterTakingTheRecommendation.movement_onset;
check('before any act, a forced slot claims no rule and no value',
  onsetBefore.method === null && onsetBefore.fromDefault.length === 0 && onsetBefore.recommended.length === 0,
  `movement_onset: rule ${onsetBefore.method}, from a default ${JSON.stringify(onsetBefore.fromDefault)}, recommended ${JSON.stringify(onsetBefore.recommended)}`);

check('taking the recommendation records it as the one act it is',
  onsetAfter.recommended.length > 0 && onsetAfter.methodFromRecommendation,
  `movement_onset: recommended ${JSON.stringify(onsetAfter.recommended)}, the rule too ${onsetAfter.methodFromRecommendation}`);

// The trap: a parameter already sitting on a slot nobody was asked about was not accepted
// by that click, so it stays a default rather than borrowing the reader's signature. A slot
// that never forced a decision is opened with its rule already bound, which is where most
// of these values are.
const takeoffAfter = provenance.afterTakingTheRecommendation.takeoff;
check('a value nobody was asked about is recorded as a default rather than as a choice',
  takeoffAfter.fromDefault.length > 0 && takeoffAfter.recommended.length === 0,
  `takeoff: from a default ${JSON.stringify(takeoffAfter.fromDefault)}, recommended ` +
  `${JSON.stringify(takeoffAfter.recommended)}. An empty pair here means the opening selection ` +
  `claims the reader stated it`);

const picked = provenance.afterPickingByHand;
check('a value picked by hand belongs to no other source, and its neighbours keep theirs',
  picked !== null && !picked.fromDefault.includes(picked.picked) && !picked.recommended.includes(picked.picked) &&
    picked.untouched.every((name) => picked.fromDefault.includes(name) || picked.recommended.includes(name)),
  picked
    ? `picked ${picked.picked}; from a default ${JSON.stringify(picked.fromDefault)}, recommended ` +
      `${JSON.stringify(picked.recommended)}, untouched ${JSON.stringify(picked.untouched)}`
    : 'no parameter control to pick from');

// The confident wrong number, selected the way a first-time user would meet it: one click
// in the onset picker. The signal has to come from the engine, so the check refuses a
// result it produced itself.
const remedy = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const analysis = await import('./analysis.js');
  const select = document.querySelector('#decision-list select[data-construct="movement_onset"]');
  select.value = 'onset.threshold.last_within_band';
  select.dispatchEvent(new Event('change'));
  await new Promise((resolve) => requestAnimationFrame(() => resolve()));

  const height = (key) => state.analysis.metrics.find((m) => m.key === key)?.value ?? null;
  const cards = [...document.querySelectorAll('#metric-grid .metric')].filter((card) =>
    card.querySelector('.metric__signal'));
  return {
    fromTheEngine: (state.analysis.signals ?? []).length,
    impulse: height('jump_height_from_takeoff_meters'),
    flight: height('jump_height_from_flight_time_meters'),
    warnings: state.analysis.warnings.length,
    beside: cards.map((card) => card.querySelector('.metric__label').textContent),
    figure: cards[0]?.querySelector('.metric__signal-figure')?.textContent ?? null,
    remedy: cards[0]?.querySelector('.metric__signal-remedy')?.textContent ?? null,
    reaches: Boolean(cards[0]?.querySelector('.metric__signal button')),
    inAPanel: Boolean(document.querySelector('#analysis-warnings .metric__signal')),
  };
})()`);

check('the engine flags the confident wrong number, in line beside both heights',
  remedy.fromTheEngine === 1 && remedy.beside.length === 2 && Boolean(remedy.remedy) &&
    remedy.reaches && !remedy.inAPanel,
  `${remedy.fromTheEngine} signal from the engine, ${remedy.warnings} warnings; ` +
  `impulse ${remedy.impulse}, flight ${remedy.flight}; beside ${remedy.beside.join(' and ') || 'nothing'}; ` +
  `${remedy.figure ?? 'no figure'}`);

// An intermediate frame of a number counting up to its new value is a number no method
// produced, rendered convincingly, in a tool whose premise is that every number on screen
// is attributable to a rule.
const animated = await evaluate(`(() => {
  const moving = (node) => {
    const style = getComputedStyle(node);
    const durations = (style.transitionDuration + ',' + style.animationDuration).split(',');
    const properties = style.transitionProperty;
    const timed = durations.some((d) => parseFloat(d) > 0.05);
    return timed && /all|content|width|height|transform/.test(properties) ? properties : null;
  };
  return [...document.querySelectorAll('.metric__value, .spread-headline__figure, .metric__signal-figure')]
    .map((node) => [node.className, moving(node)])
    .filter(([, property]) => property);
})()`);
check('no number animates to its new value',
  animated.length === 0,
  animated.length ? animated.map(([what, property]) => `${what} transitions ${property}`).join(', ') : 'every numeric surface repaints in one frame');

// The one large figure on the page is the disagreement, not the measurement. Leading with
// a measurement silently resolves a live debate in favour of the side that reads loudest.
const hierarchy = await evaluate(`(() => {
  const size = (selector) => Math.max(0, ...[...document.querySelectorAll(selector)]
    .map((node) => parseFloat(getComputedStyle(node).fontSize)));
  return { spread: size('.spread-headline__figure'), metric: size('.metric__value') };
})()`);
check('the spread is the largest figure on the page',
  hierarchy.spread > hierarchy.metric,
  `spread headline ${hierarchy.spread} px against the largest metric ${hierarchy.metric} px`);

// Five states of a value, and a reader has to be able to tell them apart without reading
// the words. Compared as rendered, in both themes, because a token that collapses two of
// them in dark mode alone would be invisible in a light-mode screenshot.
//
// Two of the five cannot share one paint: the rule that disagrees with the flight-time
// route is not the rule that runs the sub-rule the registry displays unasked. Each is read
// under the rule that produces it and they are compared afterwards. They are states of one
// interface either way, and nothing here depends on them co-occurring.
const STATE_SELECTORS = {
  provisional: '.metric:not(.metric--headline).metric--provisional',
  resolved: '.metric:not(.metric--headline):not(.metric--provisional)',
  warned: '.metric__signal',
  displayed: '.ran-beside__row--default-and-show',
  named: '.ran-beside__row--surface-on-demand',
};
const states = {};
for (const rule of ['onset.threshold.noise_relative', 'onset.threshold.last_within_band']) {
  const painted = await evaluate(`(async () => {
    (await import('./workspace.js')).enterWorkspace();
    const onset = document.querySelector('#decision-list select[data-construct="movement_onset"]');
    onset.value = ${JSON.stringify(rule)};
    onset.dispatchEvent(new Event('change'));
    const selectors = ${JSON.stringify(STATE_SELECTORS)};
    const seen = {};
    for (const theme of ['light', 'dark']) {
      document.documentElement.dataset.theme = theme;
      seen[theme] = {};
      for (const [state, selector] of Object.entries(selectors)) {
        const node = document.querySelector(selector);
        if (!node) continue;
        const style = getComputedStyle(node);
        seen[theme][state] = [style.backgroundColor, style.borderStyle, style.borderColor, style.color].join(' ');
      }
    }
    document.documentElement.dataset.theme = 'auto';
    return seen;
  })()`);
  for (const [theme, found] of Object.entries(painted)) {
    states[theme] ??= {};
    for (const [state, value] of Object.entries(found)) states[theme][state] ??= value;
  }
}
const collapsed = [];
const read = [];
for (const [theme, painted] of Object.entries(states)) {
  const drawn = Object.entries(painted);
  read.push(...drawn.map(([state]) => state));
  for (let i = 0; i < drawn.length; i += 1) {
    for (let j = i + 1; j < drawn.length; j += 1) {
      if (drawn[i][1] === drawn[j][1]) collapsed.push(`${theme}: ${drawn[i][0]} and ${drawn[j][0]}`);
    }
  }
}
const missing = Object.keys(STATE_SELECTORS).filter((state) => !read.includes(state));
check('the five states of a value are told apart without reading the words',
  collapsed.length === 0 && missing.length === 0,
  collapsed.length ? `render identically, ${collapsed.join('; ')}`
    : missing.length ? `never rendered, so never compared: ${missing.join(', ')}`
    : `${read.length} readings across two themes, all distinct: ${[...new Set(read)].join(', ')}`);

check('no console errors', consoleLines.length === 0, consoleLines.join(' | ') || 'none');

const failed = results.filter((result) => !result.passed);
for (const result of results) {
  console.log(`${result.passed ? 'pass' : 'FAIL'}  ${result.name}\n        ${result.read}`);
}
console.log(`\n${results.length - failed.length} of ${results.length} checks passed`);

socket.close();
server.close();
process.exit(failed.length ? 1 : 0);
