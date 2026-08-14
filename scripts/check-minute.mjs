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
import { rmSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { listenForConsoleErrors } from './console-errors.mjs';
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

// The browser writes its profile here. In memory rather than on the root disk, because this
// script is meant to be run many times over while a guard is broken and put back, and each
// run leaves about 160 MB behind: 95 of them, 2.9 GB, were found on a root disk at 80 percent,
// and 14.5 GB had already been aged out before anybody looked.
const profile = `/dev/shm/plateforce-check-minute-${port}`;

// Its own process group, so the browser and every renderer it spawns go together. A
// browser is a tree, and terminating the process that was launched leaves the rest of it
// running; this script is meant to be run many times over while a guard is broken and put
// back, and one leaked tree per run reaches the hundreds.
const chrome = spawn('google-chrome', [
  '--headless=new', `--remote-debugging-port=${port + 1}`, '--no-sandbox',
  '--disable-gpu', `--user-data-dir=${profile}`, 'about:blank',
], { stdio: 'ignore', detached: true });

// On every exit rather than on the one at the bottom: a check that times out waiting for
// the page leaves through a thrown error, which is exactly the run whose browser nobody
// closes. The profile goes with it, for the same reason and on the same exits: a handler
// that ends the process tree and leaves its directory behind cleans up the half that is
// visible in `ps` and none of the half that fills a disk.
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

// A probe that reads through an element the page has not parsed yet raises rather than
// returning false, and a raise escapes the loop instead of being the "not yet" it means.
// The last one is kept so a genuine failure still names itself rather than reading as a
// timeout with no cause.
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

const empty = await evaluate(`(() => ({
  heading: document.querySelector('#stage-empty h2')?.textContent ?? '',
  actions: [...document.querySelectorAll('#stage-empty .dropzone__actions button')].map((b) => b.textContent.trim()),
}))()`);
check('the first screen offers a file, a folder and a demonstration trial as peers',
  empty.heading === 'Drop a force trace or a folder of them here'
    && empty.actions.join(' / ') === 'Choose a file / Choose a folder / Open a demo trial',
  `${empty.heading}: ${empty.actions.join(' / ')}`);

await evaluate("document.getElementById('load-demo').click()");
await settle("!document.getElementById('stage-workspace').hidden", 'the workspace');
await settle(
  "document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric').length > 0"
    + " || document.querySelector('#analysis-warnings button')",
  'the first paint',
);
// The panel settles before it sweeps, so that it is not recomputing five alternatives on
// every frame of a drag. It is a settling window rather than a gate: nothing about it is
// conditioned on a decision.
await settle("document.querySelectorAll('#spread-result table.data tbody tr').length > 0", 'the spread panel');

const paint = await evaluate(`(() => {
  const card = [...document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric')]
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

await send('Emulation.setDeviceMetricsOverride', {
  width: 1440, height: 900, deviceScaleFactor: 1, mobile: false,
});
await new Promise((resolve) => setTimeout(resolve, 200));
const firstPaintShape = await evaluate(`(() => {
  const cards = [...document.querySelectorAll('#stage-workspace .metric')];
  const rect = (selector) => {
    const node = document.querySelector(selector);
    if (!node) return null;
    const box = node.getBoundingClientRect();
    return { top: box.top, left: box.left, right: box.right, bottom: box.bottom };
  };
  return {
    headlineCards: document.querySelectorAll('#headline-metric-grid .metric').length,
    remainingCards: document.querySelectorAll('#metric-grid .metric').length,
    headlineCardsStillInWall: document.querySelectorAll('#metric-grid .metric--headline').length,
    cards: cards.length,
    methodRecords: cards.map((card) => card.querySelectorAll('.metric-record').length),
    cardChoiceButtons: cards.flatMap((card) => [...card.querySelectorAll('.metric__provisional button')]
      .filter((button) => /^Choose the rules?$/.test(button.textContent.trim()))
      .map((button) => button.textContent.trim())),
    verboseLines: [...document.querySelectorAll('#stage-workspace .panel__sub, #stage-workspace .chart-help')]
      .map((node) => node.textContent.replace(/\\s+/g, ' ').trim())
      .filter((text) => /Every (choice|number|dependent|defensible)|Each value carries|Where these numbers come from/.test(text)),
    desktop: {
      height: innerHeight,
      trace: rect('.panel--trace'),
      headlines: rect('.panel--headlines'),
      decisions: rect('.panel--decisions'),
      spread: rect('.panel--spread'),
      metrics: rect('.panel--metrics'),
    },
  };
})()`);
await send('Emulation.clearDeviceMetricsOverride');

check('the two headline values have their own region above the metric catalogue',
  firstPaintShape.headlineCards === 2 && firstPaintShape.remainingCards > 0
    && firstPaintShape.headlineCardsStillInWall === 0,
  `${firstPaintShape.headlineCards} headline cards, ${firstPaintShape.remainingCards} remaining, ` +
    `${firstPaintShape.headlineCardsStillInWall} headline cards still in the wall`);

check('each metric carries one compact method record and no repeated decision button',
  firstPaintShape.cards > 0
    && firstPaintShape.methodRecords.every((count) => count === 1)
    && firstPaintShape.cardChoiceButtons.length === 0,
  `${firstPaintShape.methodRecords.join(', ')} method-record controls across ${firstPaintShape.cards} cards; ` +
    `${firstPaintShape.cardChoiceButtons.length} repeated decision buttons`);

check('the workspace copy states data and actions without narrating the interface',
  firstPaintShape.verboseLines.length === 0,
  firstPaintShape.verboseLines.join(' | ') || 'no explanatory narration');

const desktop = firstPaintShape.desktop;
const desktopRegions = [desktop.trace, desktop.headlines, desktop.decisions, desktop.spread, desktop.metrics];
check('at desktop width the decisions stay beside the trace and the spread precedes the metric catalogue',
  desktopRegions.every(Boolean)
    && desktop.decisions.left >= desktop.trace.right
    && desktop.decisions.top <= desktop.headlines.bottom
    && desktop.trace.top < desktop.headlines.top
    && desktop.headlines.top < desktop.spread.top
    && desktop.spread.top < desktop.metrics.top
    && desktop.spread.top - desktop.trace.top < desktop.height * 2,
  desktopRegions.every(Boolean)
    ? `trace ${desktop.trace.top.toFixed(0)}, headlines ${desktop.headlines.top.toFixed(0)}, ` +
      `decisions ${desktop.decisions.top.toFixed(0)}, spread ${desktop.spread.top.toFixed(0)}, metrics ${desktop.metrics.top.toFixed(0)}`
    : 'one or more workspace regions are missing');

check('a jump height is on screen with no decision made',
  paint.wall === null && paint.jumpHeight != null,
  paint.wall ? `the wall is up: "${paint.wall}"` : `${paint.jumpHeight}, from ${paint.rule || 'no named rule'}`);

// Two verdicts oblige the interface to say something about a rule nobody was asked about.
// Asked of the rail, because the rail is built from what ran and a card is built from what
// a metric read, and only the first is the population the verdict is owed to.
//
// A rule that reaches a reader only as a provenance name on a card has been named and has
// not been given the treatment either verdict asks for: `default_and_show` wants the value
// the rule used, which a card carries in a tooltip, and `surface_on_demand` wants an
// affordance that says alternatives are behind it, which a button labelled with the rule
// does not. So where a rule is missing from the rail this reports where it does reach,
// which is a different sentence from nowhere at all and sends the reader somewhere real.
const entitledToAPlace = `(async () => {
  const state = (await import('./state.js')).state;
  const verdict = new Map(state.registry.methods.map((m) => [m.id, [m.gui?.surfacing, m.title]]));
  const OWED = new Set(['surface_on_demand', 'default_and_show']);
  const drawn = [...document.querySelectorAll('#decision-list .ran-beside__title')]
    .map((node) => node.textContent.trim());
  const owed = [], missing = [];
  for (const bound of state.analysis?.bound_methods ?? []) {
    const [surfacing, title] = verdict.get(bound.method_id) ?? [];
    if (!OWED.has(surfacing)) continue;
    owed.push(bound.method_id);
    if (drawn.includes(title)) continue;
    const elsewhere = [...new Set([...document.querySelectorAll('#stage-workspace *')]
      .filter((node) => [...node.childNodes].some((child) => child.nodeType === 3 && child.textContent.includes(title)))
      .map((node) => String(node.className || node.tagName)))];
    missing.push(bound.method_id + ' (' + surfacing + ', drawn as ' + (elsewhere.join(' and ') || 'nothing at all') + ')');
  }
  return { owed, missing, drawn: drawn.length };
})()`;

const entitled = (surfaced) => [
  surfaced.owed.length > 0 && surfaced.missing.length === 0,
  surfaced.owed.length === 0
    ? 'no bound rule carries either verdict, so this check compared nothing'
    : `${surfaced.owed.length - surfaced.missing.length} of ${surfaced.owed.length} entitled rules are drawn` +
      (surfaced.missing.length ? `; without the treatment the verdict asks for: ${surfaced.missing.join(', ')}` : ''),
];

// The first paint is the moment the claim above is about, and it is the moment a reader has
// settled nothing. A rail that waits for a choice before saying what ran withholds the
// record for exactly as long as the reader has not acted, which is when they most need it.
check('before any act, every rule entitled to a place on screen already has one',
  ...entitled(await evaluate(entitledToAPlace)));

check('that value is marked provisional and names the rule that produced it',
  Boolean(paint.provisional) && paint.rule.length > 0,
  paint.provisional ?? 'no provisional line');

check('the spread panel is populated on that same first paint',
  paint.spreadRows.length > 1,
  `headline ${paint.spreadHeadline ?? 'absent'}, ${paint.spreadRows.length} rows`);

/*
 * The rule a row is running, on the row, where the row's own control names none.
 *
 * A row awaiting a decision draws its control empty, so a rule nobody picked is never drawn
 * as picked, and the claim beside the title reads "Default". With the rule itself named
 * nowhere on the row, that is a record that a rule was defaulted and no way to read which
 * rule, in the panel this software exists to put there. Read off what is on screen: the
 * options inside a closed select are in the document and not in front of anybody.
 */
const provisionalRows = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const { boundMethodId, methodTitle } = await import('./analysis.js');
  return state.provisional.map((slot) => {
    const row = [...document.querySelectorAll('#decision-list .decision')]
      .find((node) => node.querySelector('select[data-construct="' + slot.construct + '"]'));
    const running = boundMethodId(slot.key);
    const shown = row
      ? [...row.querySelectorAll('.decision__running, .ran-beside__title')].map((node) => node.textContent)
      : [];
    return { slot: slot.title, title: methodTitle(running), shown };
  });
})()`);
const naming = provisionalRows.filter((row) => row.shown.includes(row.title));
check('every rule a row is running under no decision is named on that row',
  provisionalRows.length > 0 && naming.length === provisionalRows.length,
  `${naming.length} of ${provisionalRows.length} rows awaiting a decision name the rule they ran` +
    (provisionalRows.length
      ? `, first: ${provisionalRows[0].slot} ran ${provisionalRows[0].title}`
      : ', no row was awaiting one'));

const spreadCells = await evaluate(`(() => {
  return [...document.querySelectorAll('#spread-result table.data tbody tr')]
    .filter((row) => row.children.length === 3)
    .map((row) => ({
      settings: row.children[0].textContent.trim(),
      title: row.children[0].getAttribute('title') ?? '',
      clipped: row.children[0].scrollWidth > row.children[0].clientWidth + 1,
      value: row.children[1].textContent.trim(),
      difference: row.children[2].textContent.trim(),
    }));
})()`);

/* What is left when the number is taken off the front of a cell. A difference reported in no
 * unit is a distance a reader has to guess the units of, beside a value that states them. */
const unitOf = (text) => text.replace(/^[+-]?[\d.]+\s*/, '');
const differences = spreadCells.filter((cell) => cell.difference !== '--');
const carryingTheUnit = differences.filter(
  (cell) => unitOf(cell.value) !== '' && unitOf(cell.difference) === unitOf(cell.value),
);
check('a difference in the spread table is stated in the unit of the value beside it',
  differences.length > 0 && carryingTheUnit.length === differences.length,
  `${carryingTheUnit.length} of ${differences.length} differences` +
    (differences.length ? `, first: ${differences[0].value} against ${differences[0].difference}` : ''));

// The rules behind each swept number are the reason the row is there, and the column holding
// them is narrow enough to cut a set of three off mid-phrase.
const whole = spreadCells.filter((cell) => cell.title === cell.settings);
check('every cell naming the rules behind a swept number carries the whole of them',
  spreadCells.length > 0 && whole.length === spreadCells.length,
  `${whole.length} of ${spreadCells.length} cells, ${spreadCells.filter((cell) => cell.clipped).length} ` +
    'of them too narrow to show it');

/*
 * A landmark's name against the track it is printed on, in both themes.
 *
 * Asked of both because the tracks are a different set in each: the dark set is lightened for
 * a dark chart, and the ink that reads on the light set is the ink that disappears on the
 * lightened one. A check that read only the theme the machine running it happens to prefer
 * would pass on the half that is right and never see the other.
 */
const markerInk = await evaluate(`(() => {
  const channel = (value) => { const v = value / 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
  const luminance = (colour) => { const [r, g, b] = colour.map(channel); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
  const parse = (text) => (text.match(/[\\d.]+/g) || []).slice(0, 3).map(Number);
  const root = document.documentElement;
  const was = root.dataset.theme;
  const read = [];
  for (const theme of ['light', 'dark']) {
    root.dataset.theme = theme;
    for (const node of document.querySelectorAll('.marker__label')) {
      const style = getComputedStyle(node);
      const [ink, ground] = [luminance(parse(style.color)), luminance(parse(style.backgroundColor))];
      const [high, low] = [ink, ground].sort((a, b) => b - a);
      read.push({ theme, label: node.textContent.trim(), ratio: Number(((high + 0.05) / (low + 0.05)).toFixed(2)) });
    }
  }
  root.dataset.theme = was;
  return read;
})()`);
const dim = markerInk.filter((mark) => mark.ratio < 4.5);
check('every landmark name reads against the track it is printed on, in both themes',
  markerInk.length > 0 && dim.length === 0,
  `${markerInk.length} labels across two themes, lowest ` +
    `${markerInk.length ? Math.min(...markerInk.map((mark) => mark.ratio)) : 'none'}` +
    (dim.length ? `, under 4.5: ${dim.map((mark) => `${mark.theme} ${mark.label} ${mark.ratio}`).join(', ')}` : ''));

// The sweep checks below are about which setting the panel varies and whether varying it
// reaches the engine. They are independent of whether a decision has been resolved, so
// they are read after resolving one if the run stopped for one.
const resolveAnyWall = async () => {
  const wall = await evaluate(`(() => {
    const button = document.getElementById('accept-recommended');
    if (button) button.click();
    return Boolean(button);
  })()`);
  if (wall) await settle(
    "document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric').length > 0",
    'the metric grids',
  );
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

// A default is legal under both these verdicts, and each owes the reader something:
// displayed unasked, or named with its alternatives one interaction away.
const shown = paint.ranBeside.filter((row) => row.kind.endsWith('default-and-show'));
const onDemand = paint.ranBeside.filter((row) => row.kind.endsWith('surface-on-demand'));
check('a rule the registry says to display unasked is on screen with the value it used',
  shown.length > 0,
  shown.map((row) => row.text).join(' / ') || 'nothing displayed');
check('a rule the registry says to name is on screen',
  onDemand.length > 0,
  onDemand.map((row) => row.text).join(' / ') || 'nothing named');

// The two checks above ask whether the treatment appears at all. This one asks whether it
// reached every rule entitled to it, which is a different question and the one that goes
// wrong quietly.
//
// The same question as the one asked at the first paint, asked again here because the two
// moments hold different populations. Picking a rule on a forcing slot binds the sub-rules
// it runs, so this state carries entries the opening one does not, and a rail that reaches
// every rule at the first paint can still lose one the moment a reader chooses.
const surfaced = await evaluate(entitledToAPlace);
check('every rule the registry entitles to a place on screen has one',
  ...entitled(surfaced));

// The row existing is half the verdict. The other half is that the alternatives are one
// interaction away, so the check takes the interaction rather than reading the row and
// claiming what the row does not say. The alternative ids come back so a pass cannot be
// an empty list rendered under a heading.
const alternatives = await evaluate(`(() => {
  const row = document.querySelector('#decision-list .ran-beside__row--surface-on-demand');
  if (!row) return { reached: false, why: 'no rule on screen under this verdict' };
  row.click();
  const drawer = document.getElementById('method-drawer');
  const named = [...document.querySelectorAll('#drawer-body li')]
    .map((item) => item.textContent.trim())
    .filter((text) => /^[a-z_]+(\\.[a-z_0-9]+)+/.test(text));
  return {
    reached: true,
    open: drawer && !drawer.hidden,
    label: row.querySelector('.ran-beside__action')?.textContent.trim() ?? row.textContent.trim(),
    named,
  };
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

check('the panel opens varying every rule-bearing construct on the path',
  ticked.length === 3
    && ['system_weight', 'movement_onset', 'takeoff'].every((construct) =>
      ticked.some((axis) => axis.construct === construct)),
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

await send('Emulation.setDeviceMetricsOverride', {
  width: 390, height: 844, deviceScaleFactor: 2, mobile: true,
});
await new Promise((resolve) => setTimeout(resolve, 300));
const chartInspection = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const chart = state.chart;
  const frame = () => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));

  const markerLabels = chart.markers.map((marker) => marker.labelElement.textContent.trim());
  const legendLabels = [...document.querySelectorAll('#chart-legend > span')]
    .slice(-chart.markers.length)
    .map((row) => row.textContent.trim());

  chart.setAnalysis({ ...state.analysis, onset_index: 0, touchdown_index: state.info.sample_count - 1 });
  chart.schedule();
  await frame();
  const chartBox = document.getElementById('chart').getBoundingClientRect();
  const edgeLabels = chart.markers
    .filter((marker) => ['onset', 'touchdown'].includes(marker.key))
    .map((marker) => {
      const box = marker.labelElement.getBoundingClientRect();
      return {
        key: marker.key,
        left: box.left - chartBox.left,
        right: box.right - chartBox.left,
        unclipped: marker.labelElement.scrollWidth <= marker.labelElement.clientWidth,
      };
    });
  chart.setAnalysis(state.analysis);
  chart.schedule();
  await frame();

  const canvas = document.getElementById('chart-canvas');
  const bounds = canvas.getBoundingClientRect();
  document.getElementById('chart').dispatchEvent(new PointerEvent('pointermove', {
    bubbles: true,
    clientX: bounds.left + chart.plot.left + chart.plot.width * 0.05,
    clientY: bounds.top + chart.plot.top + chart.plot.height / 2,
  }));
  const readout = document.querySelector('.chart-crosshair__label');
  const readoutBox = readout.getBoundingClientRect();

  const full = chart.visibleRange();
  chart.zoom(0.5);
  await frame();
  const zoomed = chart.visibleRange();
  const windowEnvelope = {
    start: state.envelope.start_index,
    end: state.envelope.end_index,
    buckets: state.envelope.lower.length,
  };
  chart.fit();
  await frame();

  return {
    markerLabels,
    legendLabels,
    edgeLabels,
    plotLeft: chart.plot.left,
    plotRight: chart.plot.right,
    crosshair: {
      visible: !document.querySelector('.chart-crosshair').hidden,
      text: readout.textContent.trim(),
      left: readoutBox.left - chartBox.left,
      right: readoutBox.right - chartBox.left,
    },
    full,
    zoomed,
    windowEnvelope,
    plotBuckets: chart.plotWidthPx(),
  };
})()`);
await send('Emulation.clearDeviceMetricsOverride');

check('the legend and marker pills use the same registry names',
  JSON.stringify(chartInspection.markerLabels) === JSON.stringify(chartInspection.legendLabels),
  `markers ${chartInspection.markerLabels.join(', ')}; legend ${chartInspection.legendLabels.join(', ')}`);
check('marker labels stay inside the plot at both edges',
  chartInspection.edgeLabels.length === 2 && chartInspection.edgeLabels.every((label) =>
    label.unclipped && label.left >= chartInspection.plotLeft - 1 && label.right <= chartInspection.plotRight + 1),
  chartInspection.edgeLabels.map((label) =>
    `${label.key} ${label.left.toFixed(0)}–${label.right.toFixed(0)} px, unclipped ${label.unclipped}`).join('; '));
check('the trace crosshair reads time, force and the nearest landmark',
  chartInspection.crosshair.visible
    && /\d+\.\d{3} s · .+ N · .+/.test(chartInspection.crosshair.text)
    && chartInspection.crosshair.left >= chartInspection.plotLeft - 1
    && chartInspection.crosshair.right <= chartInspection.plotRight + 1,
  chartInspection.crosshair.text
    ? `${chartInspection.crosshair.text}; ${chartInspection.crosshair.left.toFixed(0)}–${chartInspection.crosshair.right.toFixed(0)} px in ${chartInspection.plotLeft.toFixed(0)}–${chartInspection.plotRight.toFixed(0)}`
    : 'no crosshair readout');
check('zooming requests a width-bounded envelope for the visible sample window',
  chartInspection.zoomed.end - chartInspection.zoomed.start < chartInspection.full.end - chartInspection.full.start
    && chartInspection.windowEnvelope.start === chartInspection.zoomed.start
    && chartInspection.windowEnvelope.end === chartInspection.zoomed.end
    && chartInspection.windowEnvelope.buckets <= chartInspection.plotBuckets,
  `full ${chartInspection.full.start}–${chartInspection.full.end}, zoomed ` +
    `${chartInspection.zoomed.start}–${chartInspection.zoomed.end}, envelope ` +
    `${chartInspection.windowEnvelope.start}–${chartInspection.windowEnvelope.end} in ` +
    `${chartInspection.windowEnvelope.buckets} buckets`);

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
  // The class as well as the tag, because a rule is written against a class. Eleven bare
  // "button" entries name the same eleven boxes without saying which selector missed the
  // floor, and reading them costs a second run against the page to find out.
  const names = (node) => node.tagName.toLowerCase() + (node.id ? '#' + node.id : '') +
    (typeof node.className === 'string' && node.className.trim()
      ? '.' + node.className.trim().split(/\\s+/).join('.') : '');
  return {
    overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    small: boxes
      .filter((node) => !inRunningText(node))
      .map((node) => [names(node), target(node).getBoundingClientRect()])
      .map(([what, box]) => [what, Math.round(Math.min(box.width, box.height))])
      .filter(([, side]) => side < 44),
    tiny: [...document.querySelectorAll('body *')].filter(visible)
      .filter((node) => [...node.childNodes].some((child) => child.nodeType === 3 && child.textContent.trim()))
      .map((node) => [node.className || node.tagName.toLowerCase(), parseFloat(getComputedStyle(node).fontSize)])
      .filter(([, size]) => size < 12),
    unlabelled: boxes
      .filter((node) => !node.textContent.trim() && !node.getAttribute('aria-label') && !node.getAttribute('title') && !node.labels?.length)
      .map(names),
    counted: boxes.length,
    readingOrder: [
      '.panel--trace', '.panel--headlines', '.panel--decisions', '.panel--spread', '.panel--metrics',
    ].map((selector) => document.querySelector(selector)?.getBoundingClientRect().top ?? null),
    workspaceTop: document.getElementById('stage-workspace')?.getBoundingClientRect().top ?? null,
    spreadTop: document.querySelector('.panel--spread')?.getBoundingClientRect().top ?? null,
    viewportHeight: innerHeight,
    decisionRows: [...document.querySelectorAll('#decision-list > .decision')].map((row) => ({
      title: row.querySelector('.decision__title')?.textContent ?? 'untitled',
      height: Math.round(row.getBoundingClientRect().height),
    })),
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
check('at 390 px the workspace follows the ruled reading order and reaches spread within two screens',
  narrow.readingOrder.every(Number.isFinite)
    && narrow.readingOrder.every((top, index) => index === 0 || top > narrow.readingOrder[index - 1])
    && narrow.spreadTop - narrow.workspaceTop < narrow.viewportHeight * 2,
  `${narrow.readingOrder.map((top) => top == null ? 'missing' : Math.round(top)).join(' < ')}, ` +
    `spread ${narrow.spreadTop == null || narrow.workspaceTop == null ? 'missing' : Math.round(narrow.spreadTop - narrow.workspaceTop)} px from workspace start; ` +
    narrow.decisionRows.map((row) => `${row.title} ${row.height}px`).join(', '));

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

// The four checks above read what the tab remembers. This one reads what the engine was
// told, which is a different question and the one that decides what a fingerprint asserts.
// A tab that tracks every source correctly and drops all of it at the boundary passes those
// four while the record says the reader stated values they never saw.
const recorded = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const sources = {};
  for (const bound of state.analysis?.bound_methods ?? []) {
    for (const [name, source] of Object.entries(bound.parameter_sources ?? {})) {
      (sources[source] ??= []).push(bound.method_id + '.' + name);
    }
  }
  return sources;
})()`);
const heard = Object.entries(recorded).map(([source, names]) => `${source} ${names.length}`).sort();
// The property is exact rather than a threshold, and the walk supplies its own expectation:
// this walk typed one value by hand, so that is the one name the engine may record as
// stated. A count or a largest-bucket test is too loose to catch it, because a request that
// declares two of its three sources still leaves most values correctly attributed while
// silently promoting the rest to the reader's signature.
const typedByHand = picked ? [picked.picked] : [];
const saidStated = (recorded.stated ?? []).map((entry) => entry.slice(entry.lastIndexOf('.') + 1)).sort();
check('the engine records as stated exactly the values the reader typed, and no others',
  picked !== null && saidStated.join() === [...typedByHand].sort().join(),
  `${heard.join(', ')}; stated ${JSON.stringify(saidStated)} against ${JSON.stringify(typedByHand)} typed by hand`);

/*
 * The confident wrong number, reached the way a reader reaches it: the start marker dragged
 * past the unweighting, which counts every newton from a start that is already inside the
 * movement and inflates the impulse route.
 *
 * `crates/plateforce-wasm/tests/quality_signals.rs` reaches the same case by the same act at
 * the same offset, so the browser and the engine are asking one question. Its argument for
 * preferring the act over a recording is that both of this signal's earlier fixtures were
 * defects in the engine and both were fixed out from under it, while a dragged marker is
 * something a reader does and cannot be repaired away.
 *
 * Measured on the demonstration trial, the disagreement runs 23.2, 54.1, 68.2 and 47.4
 * percent at 200, 300, 400 and 500 samples late against a threshold of 20, so the case is
 * reached across a wide band rather than balanced on the edge of one. The threshold is the
 * engine's and nothing here moves it: the signal has to come from the engine, so the check
 * refuses a result it produced itself.
 */
const DRAGGED_PAST_THE_UNWEIGHTING_SAMPLES = 400;
const remedy = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const analysis = await import('./analysis.js');
  state.overrides.onset = state.analysis.onset_index + ${DRAGGED_PAST_THE_UNWEIGHTING_SAMPLES};
  analysis.runAnalysis();
  await new Promise((resolve) => requestAnimationFrame(() => resolve()));

  const height = (key) => state.analysis.metrics.find((m) => m.key === key)?.value ?? null;
  // A signal states its paragraph once and carries a reference on every other value it
  // qualifies, so a card meets it either way. Counting only the paragraph would read the
  // second height as unqualified when the reader can reach the reason from it in one click.
  const cards = [...document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric')].filter((card) =>
    card.querySelector('.metric__signal, .metric__signal-elsewhere'));
  const stating = [...document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric')].filter((card) =>
    card.querySelector('.metric__signal'));
  const referring = [...document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric')].filter((card) =>
    card.querySelector('.metric__signal-elsewhere'));
  return {
    fromTheEngine: (state.analysis.signals ?? []).length,
    impulse: height('jump_height_from_takeoff_meters'),
    flight: height('jump_height_from_flight_time_meters'),
    warnings: state.analysis.warnings.length,
    beside: cards.map((card) => card.querySelector('.metric__label').textContent),
    stated: stating.length,
    referred: referring.length,
    pointsAtTheStatement: referring.every((card) =>
      card.querySelector('.metric__signal-elsewhere')?.textContent
        ?.includes(stating[0]?.querySelector('.metric__label')?.textContent ?? '\u0000')),
    figure: stating[0]?.querySelector('.metric__signal-figure')?.textContent ?? null,
    remedy: stating[0]?.querySelector('.metric__signal-remedy')?.textContent ?? null,
    reaches: Boolean(stating[0]?.querySelector('.metric__signal button')),
    inAPanel: Boolean(document.querySelector('#analysis-warnings .metric__signal')),
  };
})()`);

check('the engine flags the confident wrong number, in line beside both heights',
  remedy.fromTheEngine === 1 && remedy.beside.length === 2 && Boolean(remedy.remedy) &&
    remedy.reaches && !remedy.inAPanel &&
    remedy.stated === 1 && remedy.referred === 1 && remedy.pointsAtTheStatement,
  `${remedy.fromTheEngine} signal from the engine, ${remedy.warnings} warnings; ` +
  `impulse ${remedy.impulse}, flight ${remedy.flight}; beside ${remedy.beside.join(' and ') || 'nothing'}, ` +
  `${remedy.stated} stating it and ${remedy.referred} pointing at the one that does` +
  `${remedy.pointsAtTheStatement ? '' : ', and a reference names a card that does not state it'}; ` +
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
// The five cannot share one paint, so each is read under an act that produces it and they
// are compared afterwards. The rule that runs the sub-rule the registry displays unasked is
// not the rule that leaves a value provisional, and neither of them warns: the warned state
// needs the two jump-height routes to disagree, which is the dragged start marker above.
// They are states of one interface either way, and nothing here depends on them co-occurring.
const STATE_SELECTORS = {
  provisional: '.metric:not(.metric--headline).metric--provisional',
  resolved: '.metric:not(.metric--headline):not(.metric--provisional)',
  warned: '.metric__signal',
  displayed: '.ran-beside__row--default-and-show',
  named: '.ran-beside__row--surface-on-demand',
};
// `answerTheDecisions` is what paints the resolved state, and it is not decoration. A card is
// provisional while any rule in its chain belongs to a decision nobody has answered, so a
// resolved card exists only once they are answered. A chain built from anything less than
// what each rule reads leaves a card resolved without anybody answering: `Takeoff` and
// `Flight time` omitting the weighing rule calls two numbers settled while the weighing
// decision behind them is open. The reading here is taken from a card that is resolved
// because the reader resolved it.
const PAINTED_BY = [
  { rule: 'onset.threshold.noise_relative', dragLate: false, answerTheDecisions: false },
  { rule: 'onset.threshold.last_within_band', dragLate: false, answerTheDecisions: false },
  { rule: 'onset.threshold.noise_relative', dragLate: true, answerTheDecisions: false },
  { rule: 'onset.threshold.noise_relative', dragLate: false, answerTheDecisions: true },
];
const readPaintedStates = () => evaluate(`(() => {
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
const states = {};
for (const { rule, dragLate, answerTheDecisions } of PAINTED_BY) {
  await evaluate(`(async () => {
    const state = (await import('./state.js')).state;
    const analysis = await import('./analysis.js');
    state.overrides.onset = null;
    (await import('./workspace.js')).enterWorkspace();
    const onset = document.querySelector('#decision-list select[data-construct="movement_onset"]');
    onset.value = ${JSON.stringify(rule)};
    onset.dispatchEvent(new Event('change'));
    if (${dragLate}) {
      state.overrides.onset = state.analysis.onset_index + ${DRAGGED_PAST_THE_UNWEIGHTING_SAMPLES};
      analysis.runAnalysis();
      await new Promise((resolve) => requestAnimationFrame(() => resolve()));
    }
  })()`);
  // The one act that answers every open choice, and it is the button beside the choices
  // rather than the one in front of the numbers: `resolveAnyWall` reads `#analysis-warnings`
  // and nothing is walled here. Two decisions stand open on this trial, the weighing rule
  // nobody has picked and the onset rule's own multi-valued parameter, and a card stays
  // provisional while either does.
  if (answerTheDecisions) {
    const answered = await evaluate(`(() => {
      const button = document.getElementById('accept-recommended');
      if (button) button.click();
      return Boolean(button);
    })()`);
    if (!answered) throw new Error('no act on the page answers the open decisions, so the resolved state cannot be painted');
    await settle(
      "document.querySelectorAll('#headline-metric-grid .metric:not(.metric--provisional), "
        + "#metric-grid .metric:not(.metric--provisional)').length > 0",
      'a card that rests on no unanswered decision');
  }
  const painted = await readPaintedStates();
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

// The picker names a quantity and clicking it puts that construct on the path. Whether the
// engine will take it is a second fact, and the browser reads the rules this build runs
// without being told how each one is reached, so the two lists can differ and nothing says
// so until a reader clicks. An offer the engine refuses does not degrade: the request comes
// back refused, and every number on the page goes with it.
//
// One at a time, and the path is put back after each, because a refused request stays
// refused while the construct that caused it is still on the path. Left to accumulate, one
// bad offer reports every offer after it as bad too, and the count names twelve culprits
// where there is one.
const offered = await evaluate(`(async () => (await import('./add-quantity.js')).offerableConstructs().map((o) => o.construct))()`);
const refused = [];
for (const construct of offered) {
  const outcome = await evaluate(`(async () => {
    const { state } = await import('./state.js');
    const { buildDecisionModel } = await import('./registry.js');
    const { runAnalysis } = await import('./analysis.js');
    const { renderDecisions } = await import('./decisions.js');
    (await import('./add-quantity.js')).addToPath(${JSON.stringify(construct)});
    const read = {
      refusal: state.analysisRefusal ? state.analysisRefusal.message : null,
      metrics: document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric').length,
    };
    state.path = state.path.filter((entry) => entry !== ${JSON.stringify(construct)});
    state.slots = buildDecisionModel(state.registry, state.build, state.path);
    renderDecisions();
    runAnalysis();
    return read;
  })()`);
  if (outcome.refusal) refused.push(`${construct}: ${outcome.refusal}`);
  else if (outcome.metrics === 0) refused.push(`${construct}: the grid emptied with no refusal to read`);
}
check('every quantity the picker offers is one the engine will take',
  offered.length > 0 && refused.length === 0,
  offered.length === 0
    ? 'the picker offered nothing, so this check compared nothing'
    : refused.length
      ? `${offered.length} offered, ${refused.length} refused: ${refused.join(' | ')}`
      : `${offered.length} offered, every one analysed`);

// An untrimmed recording takes the real import route. The chart controls are absent on the
// five-second demonstration, so exercising only that trial cannot prove the viewport exists.
await evaluate(`(async () => {
  const samples = Array.from({ length: 72000 }, (_, index) => {
    if (index < 4100) return 600;
    if (index < 4300) return 600 - (index - 4100) * 1.5;
    if (index < 5000) return 300 + (index - 4300) * 1.6;
    if (index < 5700) return 0;
    if (index < 5900) return 1400 - (index - 5700) * 4;
    return 600;
  });
  const transfer = new DataTransfer();
  transfer.items.add(new File([samples.join('\\n')], 'long-trial.txt', { type: 'text/plain' }));
  document.getElementById('dropzone').dispatchEvent(
    new DragEvent('drop', { dataTransfer: transfer, bubbles: true, cancelable: true }),
  );
})()`);
await settle("!document.getElementById('stage-columns').hidden", 'the long recording column stage');
await evaluate(`(() => {
  const rate = document.getElementById('sample-rate');
  rate.value = '1200';
  rate.dispatchEvent(new Event('input'));
  document.getElementById('columns-confirm').click();
})()`);
await settle("!document.getElementById('stage-workspace').hidden", 'the long recording workspace');
await settle("!document.getElementById('chart-nav').hidden", 'the long recording viewport controls');

const longRecording = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const frame = () => new Promise((resolve) => requestAnimationFrame(() => resolve()));
  const full = state.chart.visibleRange();
  document.getElementById('chart-zoom-in').click();
  await frame();
  const zoomed = state.chart.visibleRange();
  const pan = document.getElementById('chart-pan');
  pan.value = '750';
  pan.dispatchEvent(new Event('input'));
  await frame();
  const moved = state.chart.visibleRange();
  return {
    duration: state.info.duration_seconds,
    controlsVisible: !document.getElementById('chart-nav').hidden,
    panEnabled: !pan.disabled,
    label: document.getElementById('chart-window-label').textContent,
    full,
    zoomed,
    moved,
    envelope: [state.envelope.start_index, state.envelope.end_index, state.envelope.lower.length],
    plotBuckets: state.chart.plotWidthPx(),
  };
})()`);
check('a 60 second recording exposes a zoomable, pannable, width-bounded viewport',
  Math.abs(longRecording.duration - 60) < 1e-6
    && longRecording.controlsVisible
    && longRecording.panEnabled
    && longRecording.zoomed.end - longRecording.zoomed.start < longRecording.full.end - longRecording.full.start
    && longRecording.moved.start > longRecording.zoomed.start
    && longRecording.envelope[0] === longRecording.moved.start
    && longRecording.envelope[1] === longRecording.moved.end
    && longRecording.envelope[2] <= longRecording.plotBuckets,
  `${longRecording.duration} s, full ${longRecording.full.start}–${longRecording.full.end}, ` +
    `zoomed ${longRecording.zoomed.start}–${longRecording.zoomed.end}, panned ` +
    `${longRecording.moved.start}–${longRecording.moved.end}, ${longRecording.label}, ` +
    `${longRecording.envelope[2]} envelope buckets`);

/*
 * What a file that could not be read costs the files after it.
 *
 * The report names one file, so left standing after a file that read it stands over a screen
 * where nothing has failed. The heavier half is whether there is a file after it at all: the
 * reader who meets this is the one already recovering, and a tab that answers their next file
 * with a failure about the last one has no way on but a reload.
 */
await evaluate("document.getElementById('change-file').click()");
await settle("!document.getElementById('stage-empty').hidden", 'the drop zone again');
await evaluate(`(() => {
  const transfer = new DataTransfer();
  transfer.items.add(new File(['a line of prose with no numbers in it\\n'], 'notes.txt', { type: 'text/plain' }));
  document.getElementById('dropzone').dispatchEvent(
    new DragEvent('drop', { dataTransfer: transfer, bubbles: true, cancelable: true }),
  );
})()`);
await settle("Boolean(document.querySelector('#dropzone .notice'))", 'the report on the file that failed');
const reported = await evaluate(
  "document.querySelector('#dropzone .notice').innerText.replace(/\\s+/g, ' ').trim()",
);
await evaluate(`(() => {
  const samples = Array.from({ length: 6000 }, (_, index) => {
    if (index < 3400) return 600;
    if (index < 3600) return 600 - (index - 3400) * 1.5;
    if (index < 4200) return 300 + (index - 3600) * 1.6;
    return 0;
  });
  const transfer = new DataTransfer();
  transfer.items.add(new File([samples.join('\\n')], 'recovered.txt', { type: 'text/plain' }));
  document.getElementById('dropzone').dispatchEvent(
    new DragEvent('drop', { dataTransfer: transfer, bubbles: true, cancelable: true }),
  );
})()`);
// Read rather than settled for, because a tab that cannot open the next file stays on the
// drop zone forever and a settle would end this run instead of reporting the state it found.
const recovered = await (async () => {
  for (let attempt = 0; attempt < 40; attempt += 1) {
    const stage = await evaluate("document.getElementById('stage-columns').hidden ? null : 'columns'");
    if (stage) return true;
    await new Promise((resolve) => setTimeout(resolve, 125));
  }
  return false;
})();
const afterwards = await evaluate(`(() => ({
  notice: document.querySelector('#dropzone .notice')?.innerText.replace(/\\s+/g, ' ').trim() ?? null,
  lead: document.getElementById('columns-lead').textContent,
}))()`);
/*
 * A count and the noun it counts, agreeing, on the first screen a reader meets.
 *
 * A single-column force export is the ordinary case in this field, so "1 columns" is not an
 * edge somebody has to be unlucky to reach: it is the opening sentence, and the same shape was
 * being written a fifth different way in a fifth place. The control is the file itself, which
 * carries one column and one header line and one exact zero in its time channel, so a screen
 * that could not show the defect cannot pass this quietly.
 */
const agreement = await evaluate(`(() => ({
  said: document.getElementById('stage-columns').innerText,
  sentinelHint: document.getElementById('sentinel-hint')?.textContent ?? null,
}))()`);
// Not preceded by a digit, a minus or a point, so the sentinel option "-1 means missing" and
// any count ending in 1 are not read as a count of one.
const disagreeing = [...agreement.said.matchAll(/(?<![-\d.])1 ([a-z]{3,}s)\b/g)].map((match) => match[1]);
check('every count on the column screen agrees with the noun it counts',
  agreement.said.includes('1 column') && disagreeing.length === 0,
  `${agreement.said.split('\n')[1] ?? agreement.said.slice(0, 90)}` +
    (disagreeing.length ? `, disagreeing: 1 ${disagreeing.join(', 1 ')}` : ''));

// The count that tells a sample nobody took from an athlete in the air, said where the reader
// answers for it rather than three fields away in a card.
check('the missing-value question names the zeros in the column the reader chose',
  agreement.sentinelHint !== null
    && /^\S.* holds [\d,]+ exact zeros?\.$/.test(agreement.sentinelHint),
  agreement.sentinelHint ?? 'the question said nothing about the reader’s own column');

/*
 * A rate the reader answered for an earlier trial, on the screen for a later one.
 *
 * Read here rather than after the reload below, because this is the only state in the run
 * where an earlier trial has been analysed and a later one is on screen. The sentence names
 * the trial the number came from, which is the whole of what makes it the reader's own answer
 * rather than a plausible number this software chose: a rate carried silently is the defect
 * this field exists against, and one carrying the name of a recording is a claim the reader
 * can check against their own session.
 */
const carriedRate = await evaluate(`(() => ({
  hint: document.getElementById('sample-rate-hint').textContent,
  rate: document.getElementById('sample-rate').value,
  lead: document.getElementById('columns-lead').textContent,
}))()`);
const carriedFrom = /^Carried from (.+)\. Change it if this trial was recorded differently\.$/
  .exec(carriedRate.hint);
check('a rate carried onto the next trial names the trial it was answered for',
  Boolean(carriedFrom)
    && carriedFrom[1] === 'long-trial.txt'
    // The file on screen is not the one named, so the sentence is telling the reader something
    // they could not otherwise know rather than restating the heading above it.
    && carriedRate.lead.startsWith('recovered.txt')
    && carriedRate.rate !== '',
  `"${carriedRate.hint}" over ${carriedRate.lead.split(':')[0]}, box reads ` +
    `"${carriedRate.rate}"`);

/*
 * What the rate field says when the rate could not be read, over all three files it is said of.
 *
 * Two sentences, and the third file is the one that tells them apart. A file whose columns
 * carry no clock has to be told so. A file with a column that reads as a clock whose steps are
 * uneven has to be told that instead, because "no time column" beside a column headed Time is
 * what stops a reader believing the rest of the screen. And a **force channel that only ever
 * rises** is neither: it satisfies every column-climbs test and the record says its steps are
 * evenly spaced, so a surface reading the wrong field tells that reader their steps are uneven
 * while the record beside it says the opposite. Without that third file this check passes on
 * either field and proves nothing about which one the page reads.
 *
 * From a fresh page, because a reader who has already answered for one trial meets the
 * sentence checked directly above instead, and a probe run after them is asking what the page
 * says about an earlier answer rather than what it says about these files.
 */
await send('Page.reload', { ignoreCache: true });
await settle("!document.getElementById('stage-empty').hidden", 'the drop zone on a fresh page');
const noRate = await evaluate(`(async () => {
  const said = {};
  const rows = Array.from({ length: 3000 }, (_, index) => (index < 1500 ? 600 : 200));
  const climbing = rows.map((force, index) => {
    const seconds = index < 1000 ? index * 0.001 : 1 + (index - 1000) * 0.004;
    return seconds.toFixed(6) + ',' + force;
  });
  const drop = (name, text) => new Promise((resolve) => {
    const transfer = new DataTransfer();
    transfer.items.add(new File([text], name, { type: 'text/plain' }));
    document.getElementById('dropzone').dispatchEvent(
      new DragEvent('drop', { dataTransfer: transfer, bubbles: true, cancelable: true }));
    setTimeout(resolve, 700);
  });
  const read = async (name, text) => {
    await drop(name, text);
    said[name] = {
      hint: document.getElementById('sample-rate-hint').textContent,
      headers: [...document.querySelectorAll('#column-grid .column-card')]
        .map((card) => card.querySelector('.column-card__name span')?.textContent ?? ''),
      rate: document.getElementById('sample-rate').value,
    };
    document.getElementById('columns-cancel').click();
  };
  await read('no-clock.txt', rows.join('\\n'));
  await read('a-clock-with-a-gap.csv', 'Time,Fz\\n' + climbing.join('\\n'));
  await read('a-force-channel-that-only-rises.txt',
    rows.map((_, index) => (600 + index * 0.35).toFixed(4)).join('\\n'));
  return said;
})()`);
const withoutAClock = noRate['no-clock.txt'];
const withAClock = noRate['a-clock-with-a-gap.csv'];
const rising = noRate['a-force-channel-that-only-rises.txt'];
const noClockHere = 'No column in this file runs as a clock. Enter the rate the plate recorded at.';
check('a file with no rate in it is told why in terms of its own columns',
  withoutAClock.hint === noClockHere
    && withAClock.headers.includes('Time')
    && withAClock.hint === 'The steps in Time are not all the same length, so the rate cannot be '
      + 'read from them. Enter the rate the plate recorded at.'
    // The record calls this column evenly spaced, so the sentence about uneven steps is not
    // only the wrong one here, it is the opposite of what the record says.
    && rising.hint === noClockHere
    // No state offers a number, because a plausible rate in a sentence is typed into the box
    // beside it.
    && withoutAClock.rate === '' && withAClock.rate === '' && rising.rate === '',
  `without a clock: "${withoutAClock.hint}"; with one headed ` +
    `${withAClock.headers.join(' and ')}: "${withAClock.hint}"; ` +
    `with a force channel that only rises: "${rising.hint}"`);

check('a file that could not be read costs the reader nothing but that file',
  reported.length > 0 && recovered && afterwards.notice === null,
  `reported "${reported.slice(0, 62)}", the next file ` +
    `${recovered ? `opened as ${afterwards.lead.slice(0, 44)}` : 'never opened'}, report ` +
    `${afterwards.notice === null ? 'gone' : `still shown: ${afterwards.notice.slice(0, 62)}`}`);

check('no console errors', consoleLines.length === 0, consoleLines.join(' | ') || 'none');

const failed = results.filter((result) => !result.passed);
for (const result of results) {
  console.log(`${result.passed ? 'pass' : 'FAIL'}  ${result.name}\n        ${result.read}`);
}
console.log(`\n${results.length - failed.length} of ${results.length} checks passed`);

socket.close();
server.close();
process.exit(failed.length ? 1 : 0);
