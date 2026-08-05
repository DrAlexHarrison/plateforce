/*
 * Reading the trace by hand, asserted against a running page.
 *
 * A window a reader drags and a window a phase rule placed can cover the same samples and are
 * not the same claim, so every check here reads the record as well as the drawing: a span that
 * looks right on screen and records as the wrong kind is the defect this software exists to
 * expose, reproduced in it.
 *
 * The drag is driven with real input events rather than by calling the chart's own methods. A
 * check that calls `setRegions` proves the bookkeeping and says nothing about whether a pointer
 * can reach it, and the pointer is the whole feature.
 *
 * Usage: node scripts/check-selection.mjs <root directory> <port>
 */

import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { rmSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { listenForConsoleErrors } from './console-errors.mjs';

const [root, port] = [process.argv[2] || 'web', Number(process.argv[3] || 8771)];
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

const profile = `/dev/shm/plateforce-check-selection-${port}`;
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

const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const results = [];
const check = (name, passed, read) => results.push({ name, passed, read });

await send('Runtime.enable');
await send('Log.enable');
await send('Page.navigate', { url: `http://127.0.0.1:${port}/index.html` });
await settle("!document.getElementById('stage-empty').hidden", 'the empty stage');
await evaluate("document.getElementById('load-demo').click()");
await settle("!document.getElementById('stage-workspace').hidden", 'the workspace');
await settle(
  "document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric').length > 0",
  'the first paint',
);

/* The plot's own geometry, so every gesture below lands on the trace rather than on the
 * margins where the axis labels sit. */
const geometry = async () => evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const box = document.getElementById('chart').getBoundingClientRect();
  const plot = state.chart.plot;
  return {
    left: box.left + plot.left, right: box.left + plot.right,
    middle: box.top + plot.top + plot.height / 2,
    viewStart: state.chart.viewStart, viewEnd: state.chart.viewEnd,
    sampleRateHz: state.info.sample_rate_hz, sampleCount: state.info.sample_count,
  };
})()`);

/* What the tab holds about the selection, the record and the request, read together, because
 * the fault this file exists to catch is exactly the three of them disagreeing. */
const readSelection = async () => evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const analysis = await import('./analysis.js');
  const request = analysis.buildRequest();
  const chosen = Object.entries(request.derived || {})
    .find(([, choice]) => String(choice.method_id).startsWith('window.'));
  const bound = (state.analysis?.bound_methods || [])
    .find((row) => String(row.method_id).startsWith('window.'));
  const window = (state.analysis?.metrics || [])
    .filter((metric) => metric.key.startsWith('analysis_window_'))
    .map((metric) => [metric.key, metric.value]);
  return {
    regions: state.chart.regions.map((region) => ({
      startIndex: region.startIndex, endIndex: region.endIndex,
      stated: region.stated, phase: region.placed ? region.placed.phase : null,
    })),
    active: state.chart.activeRegion,
    labels: [...document.querySelectorAll('.selection-region')].map((node) => node.getAttribute('aria-label')),
    readout: document.getElementById('chart-selection-readout').textContent.replace(/\\s+/g, ' ').trim(),
    rowHidden: document.getElementById('chart-selection').hidden,
    zoomDisabled: document.getElementById('selection-zoom').disabled,
    undoDisabled: document.getElementById('selection-undo-zoom').disabled,
    requestedMethod: chosen ? chosen[1].method_id : null,
    requestedValues: chosen ? chosen[1].parameters : null,
    requestedOptions: chosen ? chosen[1].options : null,
    fromRegistryDefault: chosen ? chosen[1].from_registry_default : null,
    recordedMethod: bound ? bound.method_id : null,
    recordedSource: bound ? bound.method_source : null,
    recordedValues: bound ? bound.bound_parameters : null,
    recordedSources: bound ? bound.parameter_sources : null,
    window,
    placedRegions: state.chart.placedRegions.map((region) => region.phase),
    viewStart: state.chart.viewStart, viewEnd: state.chart.viewEnd,
  };
})()`);

const mouse = (type, x, y, extra = {}) => send('Input.dispatchMouseEvent', {
  type, x, y, button: 'left', buttons: type === 'mouseReleased' ? 0 : 1, clickCount: 1, ...extra,
});

/* A drag the page sees as a drag: a press, several moves, a release. One move is enough for the
 * bookkeeping and says nothing about whether the band is drawn while the button is down, which
 * is what the milestone asks for. */
async function dragAcross(fromX, toX, y, modifiers = 0) {
  await mouse('mousePressed', fromX, y, { modifiers });
  const steps = 4;
  const seen = [];
  for (let step = 1; step <= steps; step += 1) {
    await mouse('mouseMoved', fromX + ((toX - fromX) * step) / steps, y, { modifiers });
    seen.push(await evaluate("document.getElementById('chart-selection-readout').textContent.replace(/\\s+/g, ' ').trim()"));
  }
  await mouse('mouseReleased', toX, y, { modifiers });
  await pause(60);
  return seen;
}

const plot = await geometry();
const quarter = plot.left + (plot.right - plot.left) * 0.25;
const half = plot.left + (plot.right - plot.left) * 0.5;

// ---------------------------------------------------------------- the drag
const duringTheDrag = await dragAcross(quarter, half, plot.middle);
await settle("(async () => (await import('./state.js')).state.chart.regions.length === 1)()", 'the first selection');
const dragged = await readSelection();

check('a drag across the trace selects a span and reports its extent while the button is down',
  duringTheDrag.every((line) => /\d+\.\d{4} to \d+\.\d{4} s/.test(line))
    && new Set(duringTheDrag).size > 1
    && dragged.regions.length === 1 && dragged.regions[0].endIndex > dragged.regions[0].startIndex,
  `${duringTheDrag.length} readings during the drag, ${new Set(duringTheDrag).size} of them different; ` +
    `settled at ${dragged.regions[0]?.startIndex} to ${dragged.regions[0]?.endIndex}`);

check('the selected span states its own extent in seconds and samples, in text',
  dragged.labels.length === 1
    && /\d+\.\d{4} to \d+\.\d{4} seconds, \d+ samples/.test(dragged.labels[0])
    && dragged.labels[0].includes('selected by you'),
  dragged.labels.join(' | ') || 'no span carried a label');

check('a dragged span binds the rule for a stated window and states both of its ends',
  dragged.requestedMethod === 'window.stated.by_caller'
    && Number.isFinite(dragged.requestedValues?.start_seconds)
    && Number.isFinite(dragged.requestedValues?.end_seconds)
    && (dragged.fromRegistryDefault || []).length === 0,
  `request carries ${dragged.requestedMethod} with ` +
    `${JSON.stringify(dragged.requestedValues)}, claiming ${(dragged.fromRegistryDefault || []).length} registry defaults`);

const statedEnds = Object.entries(dragged.recordedSources || {})
  .filter(([name]) => name.endsWith('_seconds'));
check('the record says the window was stated, and says it of both ends rather than of the rule alone',
  dragged.recordedMethod === 'window.stated.by_caller'
    && dragged.recordedSource === 'stated'
    && statedEnds.length === 2 && statedEnds.every(([, source]) => source === 'stated'),
  `${dragged.recordedMethod} recorded ${dragged.recordedSource}, ends ` +
    statedEnds.map(([name, source]) => `${name} ${source}`).join(', '));

check('the window the engine reports back is the window the reader drew',
  dragged.window.length === 2
    && Math.abs(dragged.window[0][1] - dragged.regions[0].startIndex / plot.sampleRateHz) < 1e-6
    && Math.abs(dragged.window[1][1] - dragged.regions[0].endIndex / plot.sampleRateHz) < 1e-6,
  dragged.window.map(([key, value]) => `${key} ${value}`).join(', ')
    + ` against ${dragged.regions[0].startIndex / plot.sampleRateHz} to ${dragged.regions[0].endIndex / plot.sampleRateHz}`);

// ---------------------------------------------------------------- zooming to it
await evaluate("document.getElementById('selection-zoom').click()");
await pause(80);
const zoomed = await readSelection();
check('zooming to the selection narrows the view and leaves the selection where it was',
  zoomed.viewEnd - zoomed.viewStart < plot.viewEnd - plot.viewStart
    && zoomed.regions.length === 1
    && zoomed.regions[0].startIndex === dragged.regions[0].startIndex
    && zoomed.regions[0].endIndex === dragged.regions[0].endIndex,
  `view ${plot.viewStart}-${plot.viewEnd} became ${zoomed.viewStart}-${zoomed.viewEnd}, ` +
    `${zoomed.regions.length} span still selected`);

await evaluate("document.getElementById('selection-undo-zoom').click()");
await pause(80);
const undone = await readSelection();
check('undo steps back to the view the zoom replaced, and reset returns the whole recording',
  undone.viewStart === plot.viewStart && undone.viewEnd === plot.viewEnd,
  `back to ${undone.viewStart}-${undone.viewEnd} from ${plot.viewStart}-${plot.viewEnd}`);

// ---------------------------------------------------------------- the zoom floor
const floor = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const before = { start: state.chart.viewStart, end: state.chart.viewEnd };
  const anchor = Math.round(state.info.sample_count / 2);
  state.chart.setView(anchor, anchor + 2);
  const reached = state.chart.viewEnd - state.chart.viewStart;
  state.chart.setView(before.start, before.end);
  return { reached, quarterSecond: Math.round(state.info.sample_rate_hz * 0.25) };
})()`);
check('the view reaches two adjacent samples, which the quarter-second floor put out of reach',
  floor.reached === 2 && floor.quarterSecond > 2,
  `asked for 2 samples and got ${floor.reached}; the floor this replaces was ${floor.quarterSecond} samples at this rate`);

// ---------------------------------------------------------------- escape, and the bare click
await mouse('mousePressed', quarter, plot.middle);
await mouse('mouseMoved', half, plot.middle);
await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
await mouse('mouseReleased', half, plot.middle);
await pause(80);
const escaped = await readSelection();
check('escape abandons a drag in progress rather than selecting what it had reached',
  escaped.regions.length === 1
    && escaped.regions[0].startIndex === dragged.regions[0].startIndex
    && escaped.regions[0].endIndex === dragged.regions[0].endIndex,
  `${escaped.regions.length} span selected, still ${escaped.regions[0]?.startIndex} to ${escaped.regions[0]?.endIndex}`);

/* A point on the trace that is nobody's control: outside every selected span, because a click
 * inside one is that span asking to become the active one, and clear of the markers and the
 * weighing window, because a press on one of those is that control's drag. Found rather than
 * guessed at a fraction of the width, which lands on a marker on some recordings and not
 * others and makes this check pass or fail on the fixture. */
const clearOfIt = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const box = document.getElementById('chart').getBoundingClientRect();
  const plot = state.chart.plot;
  const marks = ['onset', 'takeoff', 'touchdown']
    .map((key) => state.analysis && state.analysis[key + '_index']).filter((index) => index != null);
  const selected = state.chart.regions.map((region) => [region.startIndex, region.endIndex]);
  const busy = (index) => marks.some((mark) => Math.abs(mark - index) < 200)
    || selected.some(([from, to]) => index >= from - 60 && index <= to + 60)
    || (index >= state.analysis.weighing_start_index - 60 && index <= state.analysis.weighing_end_index + 60);
  const span = state.chart.viewEnd - state.chart.viewStart;
  for (let index = state.chart.viewStart + 30; index < state.chart.viewEnd - 30; index += 5) {
    if (!busy(index)) return box.left + plot.left + ((index - state.chart.viewStart) / span) * plot.width;
  }
  return null;
})()`);
if (clearOfIt === null) throw new Error('every sample in view is under a control, so this check could not be run');
await mouse('mousePressed', clearOfIt, plot.middle);
await mouse('mouseReleased', clearOfIt, plot.middle);
await pause(400);
const cleared = await readSelection();
check('a click on the trace clear of every span clears the selection rather than selecting a window of no width',
  cleared.regions.length === 0 && cleared.rowHidden && cleared.requestedMethod === null,
  `${cleared.regions.length} spans, the row is ${cleared.rowHidden ? 'gone' : 'still shown'}, ` +
    `the request names ${cleared.requestedMethod ?? 'no window rule'}`);

// ---------------------------------------------------------------- a placed phase
const beforePhases = await readSelection();
check('with no phase boundary on the path the analysis offers no interval to double-click into',
  beforePhases.placedRegions.length === 0,
  `${beforePhases.placedRegions.length} intervals offered`);

const overNoPhase = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  state.chart.selectPlacedAt(Math.round(state.info.sample_count / 2), false);
  return document.getElementById('chart-selection-readout').textContent.replace(/\\s+/g, ' ').trim();
})()`);
check('a double-click where no rule placed an interval selects nothing and says why',
  overNoPhase.includes('No phase boundary reaches this point'),
  overNoPhase || 'the readout said nothing');

/* The three phase constructs, read off the build rather than written down, put on the path the
 * way the picker puts one there. */
await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const picker = await import('./add-quantity.js');
  const phaseWindow = state.registry.methods.find((method) => method.id === 'window.from_named_phase');
  const named = (phaseWindow.parameter.find((entry) => entry.name === 'phase').value || []).map((value) => value.key);
  const boundaries = new Set(named.flatMap((name) => name.split('_to_')));
  for (const binding of state.build.bindings) {
    const construct = binding.construct;
    if ([...boundaries].some((word) => construct.startsWith(word.replace(/_start$|_end$/, '')))) {
      picker.addToPath(construct);
    }
  }
  return state.path;
})()`);
await settle("(async () => (await import('./state.js')).state.chart.placedRegions.length > 0)()", 'the placed intervals');

const offered = await readSelection();
const inside = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const region = state.chart.placedRegions[state.chart.placedRegions.length - 1];
  return {
    phase: region.phase,
    sample: Math.round((region.start_index + region.end_index) / 2),
    startIndex: region.start_index, endIndex: region.end_index,
    placedBy: region.placed_by,
  };
})()`);

const x = plot.left + ((inside.sample - plot.viewStart) / (plot.viewEnd - plot.viewStart)) * (plot.right - plot.left);
await mouse('mousePressed', x, plot.middle, { clickCount: 1 });
await mouse('mouseReleased', x, plot.middle, { clickCount: 1 });
await mouse('mousePressed', x, plot.middle, { clickCount: 2 });
await mouse('mouseReleased', x, plot.middle, { clickCount: 2 });
await settle("(async () => (await import('./state.js')).state.chart.regions.length === 1)()", 'the phase selection');
await pause(120);
const phase = await readSelection();

check('double-clicking inside a placed phase selects the whole phase, on the boundaries its rules placed',
  phase.regions.length === 1
    && phase.regions[0].stated === false
    && phase.regions[0].phase === inside.phase
    && phase.regions[0].startIndex === inside.startIndex
    && phase.regions[0].endIndex === inside.endIndex,
  `${phase.regions[0]?.phase} at ${phase.regions[0]?.startIndex} to ${phase.regions[0]?.endIndex}, ` +
    `against the placed ${inside.startIndex} to ${inside.endIndex} from ${inside.placedBy.join(', ')}`);

check('a phase selection records the rule that placed it and never the reader',
  phase.requestedMethod === 'window.from_named_phase'
    && phase.requestedOptions?.phase === inside.phase
    && phase.recordedMethod === 'window.from_named_phase'
    && !Object.values(phase.recordedSources || {}).includes('stated')
      === false || phase.recordedSources?.phase === 'stated',
  `request ${phase.requestedMethod} naming ${phase.requestedOptions?.phase}, ` +
    `recorded ${phase.recordedMethod} with ${JSON.stringify(phase.recordedSources)}`);

check('the readout names the phase and the rules behind it rather than calling the span the reader’s',
  phase.readout.includes(inside.placedBy[0]) && !phase.readout.includes('Selected by you'),
  phase.readout);

// ---------------------------------------------------------------- adding a second region
const other = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const held = state.chart.regions[0].placed.phase;
  const region = state.chart.placedRegions.find((candidate) => candidate.phase !== held);
  return region ? { phase: region.phase, sample: Math.round((region.start_index + region.end_index) / 2) } : null;
})()`);
const otherX = plot.left + ((other.sample - plot.viewStart) / (plot.viewEnd - plot.viewStart)) * (plot.right - plot.left);
await mouse('mousePressed', otherX, plot.middle, { clickCount: 1, modifiers: 8 });
await mouse('mouseReleased', otherX, plot.middle, { clickCount: 1, modifiers: 8 });
await mouse('mousePressed', otherX, plot.middle, { clickCount: 2, modifiers: 8 });
await mouse('mouseReleased', otherX, plot.middle, { clickCount: 2, modifiers: 8 });
await pause(200);
const both = await readSelection();

check('shift and double-click adds a second interval instead of replacing the first',
  both.regions.length === 2
    && both.regions.map((region) => region.phase).includes(inside.phase)
    && both.regions.map((region) => region.phase).includes(other.phase)
    && both.labels.length === 2 && both.labels.every((label) => /\d+ samples/.test(label)),
  `${both.regions.length} intervals selected: ${both.regions.map((region) => region.phase).join(' and ')}`);

check('with several intervals selected the record still names one window, and says which',
  both.requestedMethod === 'window.from_named_phase'
    && both.requestedOptions?.phase === both.regions[both.active]?.phase
    && both.readout.includes('windows selected'),
  `${both.regions.length} selected, the request takes numbers over ` +
    `${both.requestedOptions?.phase}, the active one is ${both.regions[both.active]?.phase}`);

// ---------------------------------------------------------------- the numbers beside it
/* Peak force is taken over the window and jump height is not, so a path carrying both is the
 * one that can tell a panel reading the record from a panel printing whatever it has. */
await evaluate(`(async () => {
  const picker = await import('./add-quantity.js');
  picker.addToPath('peak_force');
  return true;
})()`);
await settle("(async () => (await import('./state.js')).state.analysis.metrics.some((m) => m.key === 'peak_force_newtons'))()");

const dragForNumbers = plot.left + (plot.right - plot.left) * 0.15;
await dragAcross(dragForNumbers, dragForNumbers + (plot.right - plot.left) * 0.2, plot.middle);
await settle("(async () => (await import('./state.js')).state.chart.regions.some((r) => r.stated))()");
await pause(300);

const numbers = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const host = document.getElementById('chart-selection-numbers');
  const bound = state.selection[state.build.bindings.find((b) => b.id === 'window.stated.by_caller').construct];
  return {
    hidden: host.hidden,
    shown: [...host.querySelectorAll('.chart-selection__figure')].map((node) => ({
      label: node.querySelector('dt').textContent,
      value: node.querySelector('dd').textContent,
    })),
    elsewhere: host.querySelector('.chart-selection__elsewhere')?.textContent ?? '',
    boundRule: bound ? bound.methodId : null,
    readsTheWindow: state.analysis.metrics
      .filter((metric) => (metric.contributing_method_ids || []).includes('window.stated.by_caller')
        && metric.computed_by !== 'window.stated.by_caller')
      .map((metric) => metric.label),
    restsElsewhere: state.analysis.metrics
      .filter((metric) => !(metric.contributing_method_ids || []).includes('window.stated.by_caller'))
      .map((metric) => metric.label),
  };
})()`);

check('the panel beside the selection shows exactly the quantities the record says read the window',
  !numbers.hidden
    && numbers.shown.length > 0
    && numbers.shown.map((row) => row.label).sort().join('|') === numbers.readsTheWindow.sort().join('|'),
  `${numbers.shown.length} shown: ${numbers.shown.map((row) => row.label + ' ' + row.value).join(', ')}; ` +
    `the record says ${numbers.readsTheWindow.length} read the window`);

check('a quantity resting on a landmark outside the window is named as the trial’s rather than shown as the window’s',
  numbers.restsElsewhere.length > 0
    && numbers.elsewhere.includes('are not taken over this window')
    && !numbers.shown.some((row) => numbers.restsElsewhere.includes(row.label)),
  numbers.elsewhere || 'nothing was said about the quantities that do not read the window');

// ---------------------------------------------------------------- the clipboard
/* A block that pastes numbers without the methods that produced them is this project's founding
 * defect with a clipboard attached, so what the buttons produce is read rather than assumed. */
const blocks = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const analysis = await import('./analysis.js');
  const bound = state.selection[state.build.bindings.find((b) => b.id === 'window.stated.by_caller').construct];
  const request = JSON.stringify(analysis.buildRequest());
  const whole = state.loadedTrial.markdown(request, state.fileName, null, undefined);
  const window = state.loadedTrial.markdown(request, state.fileName, null, bound.methodId);
  return {
    whole, window,
    buttons: [
      document.querySelector('#result-actions button')?.textContent ?? null,
      document.querySelector('#selection-copy button')?.textContent ?? null,
    ],
    boundRule: bound.methodId,
    opening: String(whole).slice(0, 220),
    windowRows: window.split('\\n').filter((line) => line.startsWith('| ') && !line.startsWith('| Quantity') && !line.startsWith('|---')).length,
    wholeRows: whole.split('\\n').filter((line) => line.startsWith('| ') && !line.startsWith('| Quantity') && !line.startsWith('|---')).length,
  };
})()`);

check('both places a result is shown offer it as Markdown',
  blocks.buttons.every((label) => typeof label === 'string' && label.toLowerCase().includes('copy')),
  blocks.buttons.map((label) => label ?? 'no button').join(' | '));

const carries = (text) => [
  ['the registry digest', /registry digest content-[0-9a-f]+/.test(text)],
  ['the registry revision', /registry revision \S+/.test(text)],
  ['the acquisition state', /acquisition (complete|incomplete)/.test(text)],
  ['a rule id', /\n[a-z0-9_]+\.[a-z0-9_.]+/.test(text)],
  ['a value with its source', /\n {2}\S+ = .+ \((stated|assumed|recommended|cited|measured|provisional)\)/.test(text)],
].filter(([, held]) => !held).map(([what]) => what);

check('the whole-result block carries the methods, the values with their sources, the digest and the acquisition state',
  carries(blocks.whole).length === 0,
  carries(blocks.whole).length ? `missing ${carries(blocks.whole).join(', ')}; the block opens ${blocks.opening}` :
    `${blocks.wholeRows} quantities, and the fenced block carries all five`);

check('the window block carries the same provenance and only the numbers taken over that window',
  carries(blocks.window).length === 0
    && blocks.windowRows > 0 && blocks.windowRows < blocks.wholeRows
    && blocks.window.includes(blocks.boundRule),
  `${blocks.windowRows} quantities against ${blocks.wholeRows} in the whole result, ` +
    `naming ${blocks.boundRule}` +
    (carries(blocks.window).length ? ` | missing ${carries(blocks.window).join(', ')}` : ''));

/* The engine is asked on a trailing edge, not once per frame. Counted rather than reasoned
 * about: the whole reason for the trailing edge is a measurement, so the claim it makes about
 * the running page is measured too. */
const asked = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const original = state.loadedTrial.analyse.bind(state.loadedTrial);
  window.__analysisCalls = 0;
  state.loadedTrial.analyse = (...given) => { window.__analysisCalls += 1; return original(...given); };
  return true;
})()`);
/* Dispatched without waiting on a reply between them, so the moves arrive faster than the
 * engine can answer. Sending them the way the drag above does puts a round trip between each
 * pair, every one of them longer than the wait, and the count then comes back equal to the
 * number of moves whatever the trailing edge does: a check that cannot fail. */
const from = plot.left + (plot.right - plot.left) * 0.55;
const moves = 24;
await mouse('mousePressed', from, plot.middle);
await Promise.all(Array.from({ length: moves }, (_, step) =>
  mouse('mouseMoved', from + ((plot.right - 10 - from) * (step + 1)) / moves, plot.middle)));
await mouse('mouseReleased', plot.right - 10, plot.middle);
await pause(400);
const calls = await evaluate('window.__analysisCalls');
check('a drag asks the engine on a trailing edge rather than once a frame',
  calls >= 1 && calls < moves / 2,
  `${moves} pointer moves dispatched without waiting produced ${calls} ` +
    `${calls === 1 ? 'analysis' : 'analyses'}, at a wait taken from the last one's own duration`);

const failures = results.filter((result) => !result.passed);
for (const result of results) {
  console.log(`${result.passed ? 'pass' : 'FAIL'}  ${result.name}\n      ${result.read}`);
}
if (consoleLines.length) console.log(`\nconsole errors:\n  ${consoleLines.join('\n  ')}`);
console.log(`\n${results.length - failures.length} of ${results.length} checks passed`);

socket.close();
server.close();
process.exit(failures.length || consoleLines.length ? 1 : 0);
