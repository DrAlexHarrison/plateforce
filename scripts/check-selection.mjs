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

/*
 * What has been established so far, printed whatever happens next.
 *
 * A step that drives the page rather than asserting about it can throw when the page is broken,
 * and a run that ends there used to print nothing at all: sixty settled results, green and red
 * alike, discarded because the sixty-first could not find a control. A harness that only
 * reports from a working product cannot be used to find out whether the product works.
 */
let reported = false;
function report(raised = null) {
  if (reported) return;
  reported = true;
  for (const result of results) {
    console.log(`${result.passed ? 'pass' : 'FAIL'}  ${result.name}\n      ${result.read}`);
  }
  if (consoleLines.length) console.log(`\nconsole errors:\n  ${consoleLines.join('\n  ')}`);
  const failed = results.filter((result) => !result.passed).length;
  console.log(`\n${results.length - failed} of ${results.length} checks passed`);
  if (raised) console.log(`\nthe run stopped early: ${raised?.message ?? raised}`);
}
for (const ending of ['uncaughtException', 'unhandledRejection']) {
  process.on(ending, (raised) => {
    report(raised);
    process.exit(1);
  });
}

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

await pause(300);
const automaticValues = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const host = document.getElementById('chart-selection-numbers');
  const labels = [...host.querySelectorAll('.chart-selection__figure dt')].map((node) => node.textContent);
  const methods = (state.analysis?.metrics || [])
    .filter((metric) => labels.includes(metric.label))
    .map((metric) => metric.computed_by);
  return { labels, methods };
})()`);
check('a plain range immediately describes peak force and rate of force development',
  automaticValues.labels.includes('Peak force')
    && automaticValues.labels.includes('Rate of force development')
    && automaticValues.methods.includes('force.peak.gross')
    && automaticValues.methods.includes('rfd.peak_sliding_window'),
  `${automaticValues.labels.join(', ') || 'no selected values'}, from ` +
    `${automaticValues.methods.join(', ') || 'no selected rules'}`);

const initialDescription = await evaluate(`(() => {
  const host = document.getElementById('chart-selection-numbers');
  return {
    methods: [...host.querySelectorAll('.chart-selection__method')].map((node) => node.textContent),
    landmarkHeading: host.querySelector('.chart-selection__heading')?.textContent ?? '',
    landmarks: [...host.querySelectorAll('.chart-selection__landmarks dt')].map((node) => node.textContent),
    noLandmarks: host.querySelector('.chart-selection__no-landmarks')?.textContent ?? '',
  };
})()`);
check('every selected value keeps the rule that produced it beside the number',
  initialDescription.methods.some((line) => line.includes('force.peak.gross'))
    && initialDescription.methods.some((line) => line.includes('rfd.peak_sliding_window')),
  initialDescription.methods.join(' | ') || 'no selected value carried a visible rule');

check('a range containing no major landmark says so instead of leaving the reader to infer it',
  initialDescription.landmarkHeading === 'Landmarks in this window'
    && initialDescription.landmarks.length === 0
    && initialDescription.noLandmarks.includes('No start, takeoff, or landing'),
  `${initialDescription.landmarkHeading || 'no heading'}; ` +
    `${initialDescription.noLandmarks || 'no empty result'}`);

const imageActions = await evaluate(`(() => ({
  copy: document.getElementById('copy-chart-image')?.textContent ?? null,
  save: document.getElementById('save-chart-image')?.textContent ?? null,
}))()`);
check('the chart names image copy and image save as two distinct actions',
  imageActions.copy === 'Copy chart image' && imageActions.save === 'Save chart image',
  JSON.stringify(imageActions));

const copiedImage = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const button = document.getElementById('copy-chart-image');
  const originalImageBlob = state.chart.imageBlob.bind(state.chart);
  const originalWrite = navigator.clipboard.write;
  let notes = [];
  let written = null;
  state.chart.imageBlob = (given) => {
    notes = given;
    return originalImageBlob(given);
  };
  navigator.clipboard.write = async (items) => { written = items; };
  button.click();
  await new Promise((resolve) => setTimeout(resolve, 100));
  const blob = written ? await written[0].getType('image/png') : null;
  const bitmap = blob ? await createImageBitmap(blob) : null;
  const result = {
    confirmation: button.textContent,
    type: blob?.type ?? null,
    size: blob?.size ?? 0,
    width: bitmap?.width ?? 0,
    height: bitmap?.height ?? 0,
    chartWidth: state.chart.canvas.width,
    chartHeight: state.chart.canvas.height,
    notes,
  };
  bitmap?.close();
  state.chart.imageBlob = originalImageBlob;
  navigator.clipboard.write = originalWrite;
  return result;
})()`);
check('copy chart image writes a real PNG of the visible chart and its method footer',
  copiedImage.confirmation === 'Copied chart image'
    && copiedImage.type === 'image/png' && copiedImage.size > 1000
    && copiedImage.width === copiedImage.chartWidth
    && copiedImage.height > copiedImage.chartHeight,
  `${copiedImage.confirmation}; ${copiedImage.type} ${copiedImage.size} bytes, ` +
    `${copiedImage.width}x${copiedImage.height} against chart ${copiedImage.chartWidth}x${copiedImage.chartHeight}`);

/* The landmark names are read from the chart rather than written here, so the check asserts
 * that the footer calls each landmark what the trace calls it. A literal here passes while the
 * two drift apart, which is the whole failure a footer on a figure exists to prevent. */
const drawnLandmarks = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  return state.chart.markers
    .filter((marker) => state.analysis[marker.key + '_index'] != null)
    .map((marker) => marker.label);
})()`);
const footerNames = drawnLandmarks.filter((label) =>
  copiedImage.notes.some((line) => line.startsWith(`${label}:`) && /Rules?: \S+\.\S+/.test(line)));
check('the chart image footer carries the registry, landmarks, selection and their rules',
  copiedImage.notes.some((line) => line.includes('Registry revision'))
    && drawnLandmarks.length === 3
    && footerNames.length === drawnLandmarks.length
    && copiedImage.notes.some((line) => line.includes('Selected window') && line.includes('window.stated.by_caller')),
  `${footerNames.length} of ${drawnLandmarks.length} landmarks the chart draws are named in the footer under a rule ` +
    `(${drawnLandmarks.join(', ')}) | ${copiedImage.notes.join(' | ')}`);

const savedImage = await evaluate(`(async () => {
  const clicked = HTMLAnchorElement.prototype.click;
  let download = null;
  let href = null;
  HTMLAnchorElement.prototype.click = function () {
    download = this.download;
    href = this.href;
  };
  const button = document.getElementById('save-chart-image');
  button.click();
  await new Promise((resolve) => setTimeout(resolve, 40));
  HTMLAnchorElement.prototype.click = clicked;
  return { download, href, confirmation: button.textContent };
})()`);
check('save chart image downloads a clearly named PNG and confirms the saved name',
  savedImage.download === 'demonstration-chart.png'
    && savedImage.href.startsWith('blob:')
    && savedImage.confirmation === 'Saved demonstration-chart.png',
  JSON.stringify(savedImage));

const wheelBefore = await geometry();
const wheelX = wheelBefore.left + (wheelBefore.right - wheelBefore.left) * 0.72;
const wheelAnchorBefore = Math.round(
  wheelBefore.viewStart + 0.72 * (wheelBefore.viewEnd - wheelBefore.viewStart));
await send('Input.dispatchMouseEvent', {
  type: 'mouseWheel', x: wheelX, y: wheelBefore.middle,
  deltaX: 0, deltaY: -120, modifiers: 2,
});
await pause(120);
const wheelZoomed = await geometry();
const wheelAnchorAfter = Math.round(
  wheelZoomed.viewStart + 0.72 * (wheelZoomed.viewEnd - wheelZoomed.viewStart));
check('Control + scroll zooms at the pointer instead of at the middle of the chart',
  wheelZoomed.viewEnd - wheelZoomed.viewStart < wheelBefore.viewEnd - wheelBefore.viewStart
    && Math.abs(wheelAnchorAfter - wheelAnchorBefore) <= 3,
  `view ${wheelBefore.viewStart}-${wheelBefore.viewEnd} became ` +
    `${wheelZoomed.viewStart}-${wheelZoomed.viewEnd}; pointer sample ${wheelAnchorBefore} became ${wheelAnchorAfter}`);

await send('Input.dispatchMouseEvent', {
  type: 'mouseWheel', x: wheelX, y: wheelBefore.middle,
  deltaX: 0, deltaY: 180, modifiers: 8,
});
await pause(120);
const wheelPanned = await geometry();
check('Shift + scroll moves across a zoomed trace without changing its width',
  wheelPanned.viewStart > wheelZoomed.viewStart
    && wheelPanned.viewEnd - wheelPanned.viewStart === wheelZoomed.viewEnd - wheelZoomed.viewStart,
  `view ${wheelZoomed.viewStart}-${wheelZoomed.viewEnd} became ` +
    `${wheelPanned.viewStart}-${wheelPanned.viewEnd}`);

const key = async (value, code, virtual, modifiers = 0) => {
  await send('Input.dispatchKeyEvent', { type: 'keyDown', key: value, code, windowsVirtualKeyCode: virtual, modifiers });
  await send('Input.dispatchKeyEvent', { type: 'keyUp', key: value, code, windowsVirtualKeyCode: virtual, modifiers });
  await pause(80);
};
const beforeMinus = await geometry();
await key('-', 'Minus', 189);
const afterMinus = await geometry();
await key('+', 'Equal', 187, 8);
const afterPlus = await geometry();
await key('0', 'Digit0', 48);
const afterZero = await geometry();
check('minus and plus zoom the chart, and 0 fits the whole recording',
  afterMinus.viewEnd - afterMinus.viewStart > beforeMinus.viewEnd - beforeMinus.viewStart
    && afterPlus.viewEnd - afterPlus.viewStart < afterMinus.viewEnd - afterMinus.viewStart
    && afterZero.viewStart === 0 && afterZero.viewEnd === wheelBefore.viewEnd,
  `minus ${beforeMinus.viewStart}-${beforeMinus.viewEnd} to ${afterMinus.viewStart}-${afterMinus.viewEnd}; ` +
    `plus to ${afterPlus.viewStart}-${afterPlus.viewEnd}; 0 to ${afterZero.viewStart}-${afterZero.viewEnd}`);

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

const afterClearingValues = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  return state.path.filter((construct) =>
    construct === 'peak_force' || construct === 'rate_of_force_development');
})()`);
check('clearing a range removes the descriptive quantities that range added',
  afterClearingValues.length === 0,
  afterClearingValues.length ? `still on the path: ${afterClearingValues.join(', ')}` : 'neither remains');

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

/*
 * Every row of the copied table names the rules behind its number, including the rows the
 * record roots on a landmark rule rather than on an arithmetic step.
 *
 * Those rows carry no `computed_by`, and they are exactly the quantities whose rule choice
 * moves the answer furthest: the column used to fall back to a phrase naming this software as
 * their author. The population is read off the record rather than listed here, so a quantity
 * that stops being rooted that way leaves the check rather than silently passing it.
 */
const attribution = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const analysis = await import('./analysis.js');
  const whole = state.loadedTrial.markdown(JSON.stringify(analysis.buildRequest()), state.fileName, null, undefined);
  const rows = whole.split('\\n')
    .filter((line) => line.startsWith('| ') && !line.startsWith('| Quantity') && !line.startsWith('|---'))
    .map((line) => line.split('|').map((cell) => cell.trim()).filter(Boolean))
    .map((cells) => ({ label: cells[0], rules: cells[cells.length - 1] }));
  return {
    rows,
    // The quantities the record names no arithmetic step for, which is the population this is about.
    rootedOnALandmark: state.analysis.metrics.filter((m) => !m.computed_by).map((m) => m.label),
    namesTheSoftware: /the analysis itself/.test(whole),
  };
})()`);

const unattributed = attribution.rows.filter((row) => !/`[a-z0-9_]+\.[a-z0-9_.]+`/.test(row.rules));
const landmarkRows = attribution.rows.filter((row) => attribution.rootedOnALandmark.includes(row.label));
const landmarkUnattributed = landmarkRows.filter((row) => !/`[a-z0-9_]+\.[a-z0-9_.]+`/.test(row.rules));

check('every quantity in the copied table names the rules behind it, and none names the software',
  attribution.rows.length > 0
    && attribution.rootedOnALandmark.length > 0
    && landmarkRows.length === attribution.rootedOnALandmark.length
    && unattributed.length === 0
    && landmarkUnattributed.length === 0
    && attribution.namesTheSoftware === false,
  `${attribution.rows.length - unattributed.length} of ${attribution.rows.length} rows name a rule, ` +
    `including ${landmarkRows.length - landmarkUnattributed.length} of ${attribution.rootedOnALandmark.length} ` +
    `the record roots on a landmark rule (${attribution.rootedOnALandmark.join(', ')})` +
    (attribution.namesTheSoftware ? '; the block still names the software as an author' : '') +
    (unattributed.length ? `; first without one: ${unattributed[0].label} "${unattributed[0].rules}"` : ''));

check('the window block carries the same provenance and only the numbers taken over that window',
  carries(blocks.window).length === 0
    && blocks.windowRows > 0 && blocks.windowRows < blocks.wholeRows
    && blocks.window.includes(blocks.boundRule),
  `${blocks.windowRows} quantities against ${blocks.wholeRows} in the whole result, ` +
    `naming ${blocks.boundRule}` +
    (carries(blocks.window).length ? ` | missing ${carries(blocks.window).join(', ')}` : ''));

const copyConfirmation = await evaluate(`(async () => {
  const original = navigator.clipboard.writeText;
  navigator.clipboard.writeText = async () => {};
  document.querySelector('#selection-copy button').click();
  await new Promise((resolve) => setTimeout(resolve, 80));
  const text = document.querySelector('#selection-copy button').textContent;
  navigator.clipboard.writeText = original;
  return text;
})()`);
check('copy confirmation names the selected values that reached the clipboard',
  copyConfirmation.toLowerCase().includes('peak force')
    && copyConfirmation.toLowerCase().includes('rate of force development')
    && copyConfirmation.toLowerCase().includes('no landmarks'),
  copyConfirmation);

// ---------------------------------------------------------------- landmarks and phase context
const aroundTakeoff = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  state.chart.clearSelection();
  const box = document.getElementById('chart').getBoundingClientRect();
  const plot = state.chart.plot;
  const span = state.chart.viewEnd - state.chart.viewStart;
  const x = (index) => box.left + plot.left + ((index - state.chart.viewStart) / span) * plot.width;
  const index = state.analysis.takeoff_index;
  const fromX = x(index - 300);
  const toX = x(index + 300);
  const y = box.top + plot.top + plot.height / 2;
  return {
    fromX, toX, y, index, viewStart: state.chart.viewStart, viewEnd: state.chart.viewEnd,
    target: document.elementFromPoint(fromX, y)?.className ?? null,
  };
})()`);
await dragAcross(aroundTakeoff.fromX, aroundTakeoff.toX, aroundTakeoff.y);
await pause(240);
const takeoffSelectionCount = await evaluate("(async () => (await import('./state.js')).state.chart.regions.length)()");
if (takeoffSelectionCount !== 1) {
  throw new Error(`the takeoff selection reached ${takeoffSelectionCount} ranges from ${JSON.stringify(aroundTakeoff)}`);
}

const takeoffDescription = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const analysis = await import('./analysis.js');
  const host = document.getElementById('chart-selection-numbers');
  const bound = state.selection[state.build.bindings.find((b) => b.id === 'window.stated.by_caller').construct];
  const markdown = state.loadedTrial.markdown(
    JSON.stringify(analysis.buildRequest()), state.fileName, null, bound.methodId);
  const original = navigator.clipboard.writeText;
  navigator.clipboard.writeText = async () => {};
  document.querySelector('#selection-copy button').click();
  await new Promise((resolve) => setTimeout(resolve, 80));
  const confirmation = document.querySelector('#selection-copy button').textContent;
  navigator.clipboard.writeText = original;
  return {
    labels: [...host.querySelectorAll('.chart-selection__landmarks dt')].map((node) => node.textContent),
    methods: [...host.querySelectorAll('.chart-selection__landmarks .chart-selection__method')]
      .map((node) => node.textContent),
    markdown, confirmation,
  };
})()`);
check('a range crossing takeoff names takeoff and no landmark outside the range',
  takeoffDescription.labels.length === 1 && takeoffDescription.labels[0] === 'Takeoff',
  takeoffDescription.labels.join(', ') || 'no landmark was named');

check('the takeoff in the range keeps its placement rule on screen and in the copied block',
  takeoffDescription.methods.some((line) => line.includes('takeoff.threshold'))
    && takeoffDescription.markdown.includes('## Landmarks in this window')
    && takeoffDescription.markdown.includes('- Takeoff at')
    && !takeoffDescription.markdown.includes('- Landing at'),
  `${takeoffDescription.methods.join(' | ') || 'no visible rule'}; ` +
    `${takeoffDescription.markdown.includes('- Takeoff at') ? 'copy has takeoff' : 'copy lacks takeoff'}`);

check('copy confirmation names the landmark it found in the range',
  takeoffDescription.confirmation.includes('Takeoff'),
  takeoffDescription.confirmation);

const insideFlight = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  state.chart.clearSelection();
  const box = document.getElementById('chart').getBoundingClientRect();
  const plot = state.chart.plot;
  const span = state.chart.viewEnd - state.chart.viewStart;
  const x = (index) => box.left + plot.left + ((index - state.chart.viewStart) / span) * plot.width;
  return {
    fromX: x(state.analysis.takeoff_index + 250),
    toX: x(state.analysis.touchdown_index - 250),
    y: box.top + plot.top + plot.height / 2,
  };
})()`);
await dragAcross(insideFlight.fromX, insideFlight.toX, insideFlight.y);
await settle("(async () => (await import('./state.js')).state.chart.regions.length === 1)()", 'the flight selection');
await pause(240);

const flightDescription = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const analysis = await import('./analysis.js');
  const host = document.getElementById('chart-selection-numbers');
  const bound = state.selection[state.build.bindings.find((b) => b.id === 'window.stated.by_caller').construct];
  const markdown = state.loadedTrial.markdown(
    JSON.stringify(analysis.buildRequest()), state.fileName, null, bound.methodId);
  return {
    landmarks: [...host.querySelectorAll('.chart-selection__landmarks dt')].map((node) => node.textContent),
    empty: host.querySelector('.chart-selection__no-landmarks')?.textContent ?? '',
    phase: host.querySelector('.chart-selection__phase')?.textContent ?? '',
    markdown,
  };
})()`);
check('a flight-only range says that it contains no landmark and where it sits',
  flightDescription.landmarks.length === 0
    && flightDescription.empty.includes('No start, takeoff, or landing')
    && flightDescription.phase.includes('Inside flight'),
  `${flightDescription.empty || 'no empty result'}; ${flightDescription.phase || 'no phase context'}`);

check('flight context keeps both boundary rules on screen and in the copied block',
  flightDescription.phase.includes('takeoff.threshold')
    && flightDescription.phase.includes('flight_time.takeoff_to_touchdown')
    && flightDescription.markdown.includes('This window is inside flight')
    && flightDescription.markdown.includes('takeoff.threshold')
    && flightDescription.markdown.includes('flight_time.takeoff_to_touchdown'),
  `${flightDescription.phase || 'no phase context'}; ` +
    `${flightDescription.markdown.includes('This window is inside flight') ? 'copy has phase' : 'copy lacks phase'}`);

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

// ---------------------------------------------------------------- familiar desktop recovery
/* One marker drag is one edit even though it crosses many pointer positions. The native desktop
 * shortcuts have to restore the marker and every number recomputed from it together. */
await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  state.chart.clearSelection();
  document.getElementById('reset-markers').click();
  state.chart.fit();
  return true;
})()`);
await pause(160);

const beforeMarker = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const marker = state.chart.markers.find((entry) => entry.key === 'onset').element;
  marker.focus();
  return {
    index: state.analysis.onset_index,
    override: state.overrides.onset,
  };
})()`);
await send('Input.dispatchKeyEvent', {
  type: 'keyDown', key: 'ArrowLeft', code: 'ArrowLeft', windowsVirtualKeyCode: 37,
});
await send('Input.dispatchKeyEvent', {
  type: 'keyUp', key: 'ArrowLeft', code: 'ArrowLeft', windowsVirtualKeyCode: 37,
});
await settle("(async () => (await import('./state.js')).state.overrides.onset != null)()", 'the marker edit');
const movedMarker = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  return { index: state.analysis.onset_index, override: state.overrides.onset };
})()`);

await send('Input.dispatchKeyEvent', {
  type: 'keyDown', key: 'z', code: 'KeyZ', modifiers: 4, windowsVirtualKeyCode: 90,
});
await send('Input.dispatchKeyEvent', {
  type: 'keyUp', key: 'z', code: 'KeyZ', modifiers: 4, windowsVirtualKeyCode: 90,
});
await pause(160);
const markerUndone = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  return { index: state.analysis.onset_index, override: state.overrides.onset };
})()`);
check('Command+Z restores one whole marker drag and its recomputed landmark',
  movedMarker.index !== beforeMarker.index
    && markerUndone.index === beforeMarker.index && markerUndone.override === beforeMarker.override,
  `${beforeMarker.index} became ${movedMarker.index}, then Undo reached ${markerUndone.index}`);

await send('Input.dispatchKeyEvent', {
  type: 'keyDown', key: 'Z', code: 'KeyZ', modifiers: 12, windowsVirtualKeyCode: 90,
});
await send('Input.dispatchKeyEvent', {
  type: 'keyUp', key: 'Z', code: 'KeyZ', modifiers: 12, windowsVirtualKeyCode: 90,
});
await pause(160);
const markerRedone = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  return { index: state.analysis.onset_index, override: state.overrides.onset };
})()`);
check('Command+Shift+Z reapplies the marker drag and its recomputed landmark',
  movedMarker.index !== beforeMarker.index && markerUndone.index === beforeMarker.index
    && markerRedone.index === movedMarker.index && markerRedone.override === movedMarker.override,
  `Redo reached ${markerRedone.index}, against the moved ${movedMarker.index}`);

const visibleHistory = await evaluate(`(() => ({
  undo: document.getElementById('undo-edit')?.textContent ?? null,
  redo: document.getElementById('redo-edit')?.textContent ?? null,
  undoDisabled: document.getElementById('undo-edit')?.disabled ?? null,
  redoDisabled: document.getElementById('redo-edit')?.disabled ?? null,
}))()`);
check('visible Undo and Redo actions name the same history as the shortcuts',
  visibleHistory.undo === 'Undo' && visibleHistory.redo === 'Redo'
    && visibleHistory.undoDisabled === false && visibleHistory.redoDisabled === true,
  JSON.stringify(visibleHistory));

await evaluate("document.getElementById('reset-markers')?.click()");
await pause(160);

// ---------------------------------------------------------------- selection and the weighing band
/* The band is data already on the chart. Starting a range inside it still means range selection.
 * Turning that range into a standing-still window is a separate action whose label says so. */
const weighingBand = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const box = document.getElementById('chart').getBoundingClientRect();
  const plot = state.chart.plot;
  const span = state.chart.viewEnd - state.chart.viewStart;
  const x = (index) => box.left + plot.left + ((index - state.chart.viewStart) / span) * plot.width;
  const start = state.analysis.weighing_start_index;
  const end = state.analysis.weighing_end_index;
  const from = Math.round(start + (end - start) * 0.25);
  const to = Math.round(start + (end - start) * 0.70);
  return {
    start, end, from, to,
    method: state.selection.weighing?.methodId,
    fromX: x(from), toX: x(to), y: box.top + plot.top + plot.height / 2,
  };
})()`);
await dragAcross(weighingBand.fromX, weighingBand.toX, weighingBand.y);
await pause(200);
const selectedInsideWeighing = await readSelection();
const weighingAfterDrag = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  return { start: state.analysis.weighing_start_index, end: state.analysis.weighing_end_index };
})()`);
check('dragging inside the standing-still band selects data without moving the baseline',
  selectedInsideWeighing.regions.length === 1
    && selectedInsideWeighing.regions[0].startIndex === weighingBand.from
    && selectedInsideWeighing.regions[0].endIndex === weighingBand.to
    && weighingAfterDrag.start === weighingBand.start && weighingAfterDrag.end === weighingBand.end,
  `${selectedInsideWeighing.regions.length} selections; weighing ${weighingBand.start}-${weighingBand.end} ` +
    `became ${weighingAfterDrag.start}-${weighingAfterDrag.end}`);

const baselineAction = await evaluate(`(() => {
  const button = document.getElementById('selection-use-baseline');
  return { text: button?.textContent ?? null, disabled: button?.disabled ?? null };
})()`);
check('the selected interval offers an explicit standing-still action',
  baselineAction.text === 'Use as standing-still window' && baselineAction.disabled === false,
  JSON.stringify(baselineAction));

/* Isolate this action from the marker history above. Without this reset, Undo can recover the
 * old baseline by stepping back over an unrelated marker edit, so the interesting case never
 * proves the baseline action made an entry of its own. */
const isolatedBaselineHistory = await evaluate(`(async () => {
  const history = await import('./history.js');
  history.clearHistory();
  return {
    undoDisabled: document.getElementById('undo-edit').disabled,
    redoDisabled: document.getElementById('redo-edit').disabled,
  };
})()`);
check('the standing-still action starts this recovery check with no unrelated edit to undo',
  isolatedBaselineHistory.undoDisabled === true && isolatedBaselineHistory.redoDisabled === true,
  JSON.stringify(isolatedBaselineHistory));

const usedBaseline = await evaluate(`(async () => {
  const button = document.getElementById('selection-use-baseline');
  button?.click();
  await new Promise((resolve) => setTimeout(resolve, 160));
  const state = (await import('./state.js')).state;
  return {
    clicked: Boolean(button), start: state.analysis.weighing_start_index,
    end: state.analysis.weighing_end_index,
    method: state.selection.weighing?.methodId,
  };
})()`);
check('using the explicit action applies exactly the selected interval as the standing-still window',
  usedBaseline.clicked && usedBaseline.start === weighingBand.from
    && usedBaseline.end === weighingBand.to && usedBaseline.method === 'bwepoch.manual_placement',
  JSON.stringify(usedBaseline));

const historyAfterBaseline = await evaluate(`(() => ({
  undoDisabled: document.getElementById('undo-edit').disabled,
  redoDisabled: document.getElementById('redo-edit').disabled,
}))()`);
check('using a selection as the standing-still window becomes one undoable action',
  historyAfterBaseline.undoDisabled === false && historyAfterBaseline.redoDisabled === true,
  JSON.stringify(historyAfterBaseline));

await send('Input.dispatchKeyEvent', {
  type: 'keyDown', key: 'z', code: 'KeyZ', modifiers: 4, windowsVirtualKeyCode: 90,
});
await send('Input.dispatchKeyEvent', {
  type: 'keyUp', key: 'z', code: 'KeyZ', modifiers: 4, windowsVirtualKeyCode: 90,
});
await pause(160);
const baselineUndone = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  return {
    start: state.analysis.weighing_start_index,
    end: state.analysis.weighing_end_index,
    method: state.selection.weighing?.methodId,
    selectionStart: state.chart.selection().active?.startIndex,
    selectionEnd: state.chart.selection().active?.endIndex,
  };
})()`);
check('Command+Z restores the earlier standing-still window and its rule without losing the selection',
  baselineUndone.start === weighingBand.start && baselineUndone.end === weighingBand.end
    && baselineUndone.method === weighingBand.method
    && baselineUndone.selectionStart === weighingBand.from
    && baselineUndone.selectionEnd === weighingBand.to,
  JSON.stringify(baselineUndone));

await send('Input.dispatchKeyEvent', {
  type: 'keyDown', key: 'Z', code: 'KeyZ', modifiers: 12, windowsVirtualKeyCode: 90,
});
await send('Input.dispatchKeyEvent', {
  type: 'keyUp', key: 'Z', code: 'KeyZ', modifiers: 12, windowsVirtualKeyCode: 90,
});
await pause(160);
const baselineRedone = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  return {
    start: state.analysis.weighing_start_index,
    end: state.analysis.weighing_end_index,
    method: state.selection.weighing?.methodId,
  };
})()`);
check('Command+Shift+Z reapplies the selected standing-still window and its rule',
  baselineRedone.start === weighingBand.from && baselineRedone.end === weighingBand.to
    && baselineRedone.method === 'bwepoch.manual_placement',
  JSON.stringify(baselineRedone));

/* Start over before Escape, so the key is tested on a normal selection rather than a window
 * that intentionally changed the analysis underneath it. */
await evaluate(`(async () => {
  document.getElementById('change-file').click();
  document.getElementById('load-demo').click();
  return true;
})()`);
await settle("!document.getElementById('stage-workspace').hidden", 'the restarted workspace');
await settle("(async () => Boolean((await import('./state.js')).state.analysis))()", 'the restarted analysis');
const fresh = await geometry();
await dragAcross(
  fresh.left + (fresh.right - fresh.left) * 0.08,
  fresh.left + (fresh.right - fresh.left) * 0.18,
  fresh.middle,
);
await settle("(async () => (await import('./state.js')).state.chart.regions.length === 1)()", 'the Escape selection');
await send('Input.dispatchKeyEvent', {
  type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27,
});
await send('Input.dispatchKeyEvent', {
  type: 'keyUp', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27,
});
await pause(160);
const escapedSelection = await readSelection();
check('Escape clears a settled chart selection and its bound window rule',
  escapedSelection.regions.length === 0 && escapedSelection.rowHidden
    && escapedSelection.requestedMethod === null,
  `${escapedSelection.regions.length} selections, row ${escapedSelection.rowHidden ? 'hidden' : 'shown'}, ` +
    `request ${escapedSelection.requestedMethod ?? 'without a window rule'}`);

// ---------------------------------------------------------------- the keyboard on the trace
/*
 * A landmark under the keyboard, driven with real key and pointer events.
 *
 * Calling `focus()` from the check would prove the element accepts focus and say nothing about
 * whether a reader can put focus there, and being unable to put focus there was the defect.
 */
const tabKey = async (shift = false) => {
  for (const type of ['keyDown', 'keyUp']) {
    await send('Input.dispatchKeyEvent', {
      type, key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9, modifiers: shift ? 8 : 0,
    });
  }
  await pause(25);
};

const arrowKey = async (key, shift = false) => {
  const code = key === 'ArrowRight' ? 39 : 37;
  for (const type of ['keyDown', 'keyUp']) {
    await send('Input.dispatchKeyEvent', {
      type, key, code: key, windowsVirtualKeyCode: code, modifiers: shift ? 8 : 0,
    });
  }
  await pause(120);
};

const focused = async () => evaluate(`(() => {
  const node = document.activeElement;
  return node ? [node.tagName, node.className, node.id].join('|') : 'none';
})()`);

/* Tab from the control before the plot until the keyboard has walked past the last landmark,
 * so the order is read rather than a stop count being assumed: how many controls sit between
 * the two depends on whether the weighing window is inside the visible range. */
await evaluate("document.getElementById('change-file').focus()");
const tabOrder = [];
for (let step = 0; step < 14; step += 1) {
  await tabKey();
  const stop = await focused();
  tabOrder.push(stop);
  if (tabOrder.some((entry) => entry.includes('marker--touchdown')) && !stop.includes('marker ')) break;
}
const markerStops = tabOrder.filter((entry) => entry.includes('marker '));
check('the tab key reaches every chart landmark, in the order they sit on the trace',
  markerStops.length === 3
    && markerStops[0].includes('marker--onset')
    && markerStops[1].includes('marker--takeoff')
    && markerStops[2].includes('marker--touchdown'),
  `${markerStops.length} landmarks reached in ${tabOrder.length} stops: ${tabOrder.join(' > ')}`);

/*
 * What the focus indicator measures against the thing it is drawn on top of.
 *
 * A landmark's focus ring sits on the landmark's own hue, so a ring taking the accent read
 * 1.20 against the takeoff track. Read as rendered and scored against the 3:1 floor for a
 * control, in both themes, because the two palettes are two different questions.
 */
const focusRingContrast = async () => evaluate(`(() => {
  const marker = document.activeElement;
  if (!marker || !marker.classList.contains('marker')) return null;
  const line = getComputedStyle(marker, '::before');
  const channel = (text) => text.match(/[\\d.]+/g).slice(0, 3).map(Number);
  const relative = (rgb) => {
    const linear = rgb.map((value) => {
      const scaled = value / 255;
      return scaled <= 0.04045 ? scaled / 12.92 : ((scaled + 0.055) / 1.055) ** 2.4;
    });
    return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
  };
  const ratio = (left, right) => {
    const [high, low] = [relative(left), relative(right)].sort((a, b) => b - a);
    return (high + 0.05) / (low + 0.05);
  };
  const hue = channel(line.backgroundColor);
  const inner = channel(line.borderTopColor);
  const outer = channel(line.outlineColor);
  // The palette actually painted, not the setting. The automatic setting is two different
  // answers depending on the machine, and the two readings below have to be the two palettes.
  const chosen = document.documentElement.dataset.theme;
  return {
    theme: chosen === 'auto'
      ? (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
      : chosen,
    innerWidth: parseFloat(line.borderTopWidth),
    outerWidth: parseFloat(line.outlineWidth),
    innerAgainstHue: ratio(inner, hue),
    outerAgainstInner: ratio(outer, inner),
    innerIsTheHue: line.borderTopColor === line.backgroundColor,
  };
})()`);

/* The keyboard is walked onto a landmark rather than put there, because a ring is only drawn
 * for focus the browser judges visible and a programmatic call is not that. */
await evaluate("document.getElementById('change-file').focus()");
for (let step = 0; step < 14; step += 1) {
  await tabKey();
  if ((await focused()).includes('marker--takeoff')) break;
}
const litRing = await focusRingContrast();
// The same focused landmark under the other palette. Nothing moves but the colours, so the
// two readings are the same control and the comparison is the palette alone.
await evaluate("document.getElementById('theme-toggle').click()");
await pause(120);
const flippedRing = await focusRingContrast();
await evaluate("document.getElementById('theme-toggle').click()");
await pause(120);

const rings = [litRing, flippedRing].filter(Boolean);
check('a focused landmark carries a two-colour ring that clears 3:1 on its own hue, in both themes',
  rings.length === 2
    && rings[0].theme !== rings[1].theme
    && rings.every((ring) => ring.innerWidth >= 2 && ring.outerWidth >= 2)
    && rings.every((ring) => !ring.innerIsTheHue)
    && rings.every((ring) => ring.innerAgainstHue >= 3 && ring.outerAgainstInner >= 3),
  rings.map((ring) => `${ring.theme}: ring on hue ${ring.innerAgainstHue.toFixed(2)}, ` +
    `ring on ring ${ring.outerAgainstInner.toFixed(2)}, ${ring.innerWidth} and ${ring.outerWidth} px`).join('; ')
    || 'no landmark held focus');

const beforeNudge = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  return {
    holding: document.activeElement.className,
    index: state.analysis.takeoff_index,
    // The landmark beside the one being held, read at the same moment, so the instant on the
    // held label is the difference between them rather than a claim about a different moment.
    restingLabel: document.querySelector('.marker--onset .marker__label').textContent,
  };
})()`);
await arrowKey('ArrowRight');
const oneSample = await evaluate("(async () => (await import('./state.js')).state.analysis.takeoff_index)()");
await arrowKey('ArrowRight', true);
const tenMore = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  return {
    index: state.analysis.takeoff_index,
    said: document.getElementById('chart-announcement').textContent,
    label: document.querySelector('.marker--takeoff .marker__label').textContent,
    undoDisabled: document.getElementById('undo-edit').disabled,
    undoNames: document.getElementById('undo-edit').getAttribute('aria-label'),
  };
})()`);

check('an arrow key moves a focused landmark one sample and a modifier moves it ten',
  beforeNudge.holding.includes('marker--takeoff')
    && oneSample === beforeNudge.index + 1
    && tenMore.index === beforeNudge.index + 11,
  `${beforeNudge.index} then ${oneSample} then ${tenMore.index}, holding ${beforeNudge.holding}`);

check('a nudge says which landmark moved, where it now sits, and what changed with it',
  /Takeoff/i.test(tenMore.said)
    && tenMore.said.includes((tenMore.index / plot.sampleRateHz).toFixed(4))
    && /was \d/.test(tenMore.said),
  tenMore.said || 'nothing announced');

check('a landmark states its own instant while the reader is holding it, and its neighbours do not',
  !/\d+\.\d{4} s$/.test(beforeNudge.restingLabel.trim())
    && /\d+\.\d{4} s$/.test(tenMore.label.trim())
    && tenMore.label.includes((tenMore.index / plot.sampleRateHz).toFixed(4)),
  `the landmark beside it reads "${beforeNudge.restingLabel}", the held one reads "${tenMore.label}"`);

check('each nudge is one undoable edit, and Undo names the edit it would reverse',
  tenMore.undoDisabled === false && /^Undo moving /.test(tenMore.undoNames || ''),
  `${tenMore.undoNames ?? 'no name'}, undo ${tenMore.undoDisabled ? 'unavailable' : 'available'}`);

/*
 * Two nudges, two steps back, read after each of them.
 *
 * Reading only where the landmark ends up cannot tell one edit at a time from a burst
 * collapsed into one: a history holding a single entry reaches the original index on the first
 * press and ignores the second, and the final reading is the same either way. The state between
 * the two presses is the only place the difference shows.
 */
const undoneStepByStep = [];
for (const _ of [1, 2]) {
  await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'z', code: 'KeyZ', modifiers: 4, windowsVirtualKeyCode: 90 });
  await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'z', code: 'KeyZ', modifiers: 4, windowsVirtualKeyCode: 90 });
  await pause(140);
  undoneStepByStep.push(await evaluate("(async () => (await import('./state.js')).state.analysis.takeoff_index)()"));
}
check('two nudges undo as two edits, one step at a time, and return the landmark exactly',
  undoneStepByStep[0] === beforeNudge.index + 1
    && undoneStepByStep[1] === beforeNudge.index,
  `${tenMore.index} back through ${undoneStepByStep.join(' then ')}, ` +
    `against ${beforeNudge.index + 1} then ${beforeNudge.index} for two separate edits`);

/* A press on a landmark, with real pointer events. The press is the gesture that stopped
 * granting focus, so the arrow key that follows it is the half that could not work. */
const markerAt = await evaluate(`(() => {
  const box = document.querySelector('.marker--onset').getBoundingClientRect();
  return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
})()`);
await evaluate("document.getElementById('change-file').focus()");
await mouse('mousePressed', markerAt.x, markerAt.y);
await mouse('mouseReleased', markerAt.x, markerAt.y);
await pause(80);
const afterPress = await focused();
check('pressing a landmark leaves the keyboard on it',
  afterPress.includes('marker--onset'),
  `focus after the press: ${afterPress}`);

// ---------------------------------------------------------------- the keyboard on a selection
await evaluate("document.getElementById('reset-markers')?.click()");
await pause(140);
const spanPlot = await geometry();
await dragAcross(
  spanPlot.left + (spanPlot.right - spanPlot.left) * 0.3,
  spanPlot.left + (spanPlot.right - spanPlot.left) * 0.42,
  spanPlot.middle,
);
await settle("(async () => (await import('./state.js')).state.chart.regions.length === 1)()", 'the span to nudge');
const spanBefore = await evaluate(`(async () => {
  const region = (await import('./state.js')).state.chart.regions[0];
  return { startIndex: region.startIndex, endIndex: region.endIndex };
})()`);
await evaluate("document.querySelector('.selection-region')?.focus()");
await arrowKey('ArrowRight');
const spanMoved = await evaluate(`(async () => {
  const region = (await import('./state.js')).state.chart.regions[0];
  return {
    startIndex: region.startIndex, endIndex: region.endIndex,
    said: document.getElementById('chart-announcement').textContent,
  };
})()`);
await arrowKey('ArrowRight', true);
const spanExtended = await evaluate(`(async () => {
  const region = (await import('./state.js')).state.chart.regions[0];
  return {
    startIndex: region.startIndex, endIndex: region.endIndex,
    said: document.getElementById('chart-announcement').textContent,
  };
})()`);

check('an arrow key slides a selected span and a modifier moves its far end',
  spanMoved.startIndex === spanBefore.startIndex + 1
    && spanMoved.endIndex === spanBefore.endIndex + 1
    && spanExtended.startIndex === spanMoved.startIndex
    && spanExtended.endIndex === spanMoved.endIndex + 1,
  `${spanBefore.startIndex}-${spanBefore.endIndex} slid to ${spanMoved.startIndex}-${spanMoved.endIndex} ` +
    `then extended to ${spanExtended.startIndex}-${spanExtended.endIndex}`);

check('moving a span says what it now covers',
  /Selection moved/.test(spanMoved.said) && /Selection end moved/.test(spanExtended.said)
    && spanExtended.said.includes((spanExtended.endIndex / plot.sampleRateHz).toFixed(4)),
  `"${spanMoved.said}" then "${spanExtended.said}"`);

// A control that cannot be pressed says what would let it be, rather than sitting grey.
await evaluate("document.getElementById('selection-clear')?.click()");
await pause(140);
const disabledReasons = await evaluate(`(() => {
  const ids = ['selection-zoom', 'selection-clear', 'selection-use-baseline'];
  return ids.map((id) => {
    const control = document.getElementById(id);
    return { id, disabled: control.disabled, title: control.getAttribute('title') };
  });
})()`);
check('a selection control that cannot be pressed says what would let it be',
  disabledReasons.every((entry) => entry.disabled && (entry.title || '').length > 10),
  disabledReasons.map((entry) => `${entry.id}: ${entry.disabled ? 'off' : 'ON'} "${entry.title ?? 'no reason'}"`).join('; '));

// ------------------------------------------- the route back from a rule a gesture bound
/*
 * The reader puts the standing-still window somewhere no rule would have chosen it, closes
 * every open choice, and asks for the recommended rules back. Before this the control that
 * offers them had left the page by then.
 */
await dragAcross(
  spanPlot.left + (spanPlot.right - spanPlot.left) * 0.55,
  spanPlot.left + (spanPlot.right - spanPlot.left) * 0.68,
  spanPlot.middle,
);
await settle("(async () => (await import('./state.js')).state.chart.regions.length === 1)()", 'the ramp span');
await evaluate("document.getElementById('accept-recommended')?.click()");
await pause(200);
const ruleChosenWindow = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  return {
    start: state.analysis.weighing_start_index,
    end: state.analysis.weighing_end_index,
    method: state.selection.weighing?.methodId,
    height: state.analysis.metrics.find((m) => m.key === 'jump_height_from_takeoff_meters')?.value,
    offered: Boolean(document.getElementById('accept-recommended')),
  };
})()`);
await evaluate("document.getElementById('selection-use-baseline')?.click()");
await pause(220);
const handPlaced = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const control = document.getElementById('accept-recommended');
  return {
    start: state.analysis.weighing_start_index,
    end: state.analysis.weighing_end_index,
    method: state.selection.weighing?.methodId,
    height: state.analysis.metrics.find((m) => m.key === 'jump_height_from_takeoff_meters')?.value,
    openChoices: (await import('./decisions.js')).unresolvedDecisions().length,
    offered: Boolean(control),
    says: control ? control.getAttribute('title') : null,
  };
})()`);
check('a hand-placed standing-still window keeps the recommended rules within reach, and says so',
  handPlaced.method === 'bwepoch.manual_placement'
    && handPlaced.openChoices === 0
    && handPlaced.offered
    && /placed by hand/.test(handPlaced.says || ''),
  `${handPlaced.method} over ${handPlaced.start}-${handPlaced.end}, ${handPlaced.openChoices} choices open, ` +
    `control ${handPlaced.offered ? `offered: "${handPlaced.says}"` : 'gone'}`);

await evaluate("document.getElementById('accept-recommended')?.click()");
await pause(240);
const takenBack = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const bound = state.analysis.bound_methods.find((row) => row.method_id === state.selection.weighing?.methodId);
  return {
    start: state.analysis.weighing_start_index,
    end: state.analysis.weighing_end_index,
    method: state.selection.weighing?.methodId,
    source: bound ? bound.method_source : null,
    statedStart: state.weighing.startIndex,
    height: state.analysis.metrics.find((m) => m.key === 'jump_height_from_takeoff_meters')?.value,
    undoDisabled: document.getElementById('undo-edit').disabled,
    undoNames: document.getElementById('undo-edit').getAttribute('aria-label'),
  };
})()`);
check('the recommended rules take back a hand-placed window, and the record says the recommendation chose it',
  takenBack.method !== 'bwepoch.manual_placement'
    && takenBack.method === ruleChosenWindow.method
    && takenBack.start === ruleChosenWindow.start && takenBack.end === ruleChosenWindow.end
    && takenBack.statedStart === null
    && takenBack.source === 'recommended',
  `${handPlaced.method} over ${handPlaced.start}-${handPlaced.end} became ${takenBack.method} over ` +
    `${takenBack.start}-${takenBack.end} under ${takenBack.source}, against ${ruleChosenWindow.method} over ` +
    `${ruleChosenWindow.start}-${ruleChosenWindow.end} before the hand placed one`);

check('taking the rules back is one undoable edit that names itself',
  takenBack.undoDisabled === false && /recommended rules/.test(takenBack.undoNames || ''),
  `${takenBack.undoNames ?? 'no name'}`);

await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'z', code: 'KeyZ', modifiers: 4, windowsVirtualKeyCode: 90 });
await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'z', code: 'KeyZ', modifiers: 4, windowsVirtualKeyCode: 90 });
await pause(240);
const handPlacementBack = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  return {
    method: state.selection.weighing?.methodId,
    start: state.analysis.weighing_start_index,
    end: state.analysis.weighing_end_index,
  };
})()`);
check('Undo restores the hand-placed window the recommendation took back',
  handPlacementBack.method === 'bwepoch.manual_placement'
    && handPlacementBack.start === handPlaced.start && handPlacementBack.end === handPlaced.end,
  `${handPlacementBack.method} over ${handPlacementBack.start}-${handPlacementBack.end}`);

// ------------------------------------------- an edit that changes only the record
/*
 * The reader corrects a landmark, takes the recommended rules, then presses Undo meaning to
 * take back the landmark.
 *
 * Undo pops the accept, which is the right last-in-first-out answer and leaves every number on
 * screen identical. So the only thing that changed is which rules produced them, and a page
 * that says nothing here is an unrecorded change to the record on the control a reader trusts
 * most. Both halves are asserted: the numbers must not move, and the change must be reported.
 */
await evaluate(`(async () => {
  document.getElementById('change-file').click();
  document.getElementById('load-demo').click();
  return true;
})()`);
await settle("!document.getElementById('stage-workspace').hidden", 'the workspace for the record check');
await settle("(async () => Boolean((await import('./state.js')).state.analysis))()", 'its analysis');

await evaluate("document.getElementById('change-file').focus()");
for (let step = 0; step < 14; step += 1) {
  await tabKey();
  if ((await focused()).includes('marker--onset')) break;
}
await arrowKey('ArrowRight');
await evaluate("document.getElementById('accept-recommended')?.click()");
await pause(240);
const beforeTheStepBack = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  return {
    onset: state.analysis.onset_index,
    override: state.overrides.onset,
    timeToTakeoff: state.analysis.metrics.find((m) => m.key === 'time_to_takeoff_seconds')?.value,
    weighing: state.selection.weighing?.methodId ?? null,
    open: (await import('./decisions.js')).unresolvedDecisions().length,
    undoNames: document.getElementById('undo-edit').getAttribute('aria-label'),
  };
})()`);
await evaluate("document.getElementById('undo-edit').click()");
await pause(260);
const afterTheStepBack = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const line = document.getElementById('record-change');
  return {
    onset: state.analysis.onset_index,
    override: state.overrides.onset,
    timeToTakeoff: state.analysis.metrics.find((m) => m.key === 'time_to_takeoff_seconds')?.value,
    weighing: state.selection.weighing?.methodId ?? null,
    open: (await import('./decisions.js')).unresolvedDecisions().length,
    onScreen: Boolean(line) && !line.hidden,
    drawn: line ? line.textContent : null,
    spoken: document.getElementById('chart-announcement').textContent,
  };
})()`);

check('a step back over an accepted recommendation leaves the landmark and every number where they were',
  beforeTheStepBack.override != null
    && afterTheStepBack.override === beforeTheStepBack.override
    && afterTheStepBack.onset === beforeTheStepBack.onset
    && afterTheStepBack.timeToTakeoff === beforeTheStepBack.timeToTakeoff,
  `onset ${beforeTheStepBack.onset} to ${afterTheStepBack.onset}, override ${beforeTheStepBack.override} to ` +
    `${afterTheStepBack.override}, time to takeoff ${beforeTheStepBack.timeToTakeoff} to ${afterTheStepBack.timeToTakeoff}`);

check('it did change the record, and it says which rule went back to what, on screen and out loud',
  beforeTheStepBack.weighing !== null
    && afterTheStepBack.weighing === null
    && afterTheStepBack.open > beforeTheStepBack.open
    && afterTheStepBack.onScreen
    && /no rule chosen, was \S/.test(afterTheStepBack.drawn || '')
    && afterTheStepBack.drawn === afterTheStepBack.spoken
    && /recommended rules/.test(afterTheStepBack.drawn || ''),
  `${beforeTheStepBack.weighing} became ${afterTheStepBack.weighing}, ` +
    `${beforeTheStepBack.open} open choices became ${afterTheStepBack.open}; ` +
    `${afterTheStepBack.onScreen ? 'drawn' : 'NOT DRAWN'}: "${afterTheStepBack.drawn}"; ` +
    `spoken: "${afterTheStepBack.spoken}"`);

/*
 * The other direction: an edit that moves numbers and no rule is not a record change, so it is
 * spoken and does not write the line. Without this the check above passes on a page that draws
 * every edit.
 *
 * The claim is that this edit did not write the line, not that the line is blank. The line says
 * what last changed the record and stays saying it, so a previous change still showing is the
 * design rather than a failure: what would be wrong is this edit overwriting it.
 */
const lineBefore = await evaluate("document.getElementById('record-change').textContent");
await arrowKey('ArrowRight');
await evaluate("document.getElementById('undo-edit').click()");
await pause(240);
const plainEdit = await evaluate(`(() => ({
  drawn: document.getElementById('record-change').textContent,
  spoken: document.getElementById('chart-announcement').textContent,
}))()`);
check('an edit that moves a number and no rule is spoken without taking the reader’s eye',
  lineBefore.length > 0
    && plainEdit.drawn === lineBefore
    && /Undid moving /.test(plainEdit.spoken || '')
    && !/Undid moving /.test(plainEdit.drawn || ''),
  `the line still reads what last changed the record (${plainEdit.drawn === lineBefore ? 'untouched' : 'OVERWRITTEN'}), ` +
    `while the edit was spoken as "${plainEdit.spoken}"`);

// ------------------------------------------- choosing a rule is an edit like any other
const chosenByHand = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const select = document.querySelector('#decision-list select[data-construct]');
  const construct = select.dataset.construct;
  const slot = state.slots.find((entry) => entry.construct === construct);
  const before = state.selection[slot.key]?.methodId ?? null;
  const pick = [...select.options].find((option) => option.value && option.value !== before);
  select.value = pick.value;
  select.dispatchEvent(new Event('change', { bubbles: true }));
  await new Promise((resolve) => setTimeout(resolve, 200));
  return {
    key: slot.key,
    before,
    after: state.selection[slot.key]?.methodId ?? null,
    stated: state.selection[slot.key]?.methodStated ?? null,
    undoDisabled: document.getElementById('undo-edit').disabled,
    undoNames: document.getElementById('undo-edit').getAttribute('aria-label'),
  };
})()`);
await evaluate("document.getElementById('undo-edit').click()");
await pause(240);
const choiceUndone = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  return {
    after: state.selection['${chosenByHand.key}']?.methodId ?? null,
    drawn: document.getElementById('record-change').textContent,
    spoken: document.getElementById('chart-announcement').textContent,
  };
})()`);
check('choosing a rule is one undoable edit that names itself and goes back exactly',
  chosenByHand.after !== chosenByHand.before
    && chosenByHand.stated === true
    && chosenByHand.undoDisabled === false
    && /^Undo choosing /.test(chosenByHand.undoNames || '')
    && choiceUndone.after === chosenByHand.before
    && /was \S/.test(choiceUndone.drawn || ''),
  `${chosenByHand.before} to ${chosenByHand.after} under "${chosenByHand.undoNames}", ` +
    `back to ${choiceUndone.after}; drawn "${choiceUndone.drawn}"; spoken "${choiceUndone.spoken}"`);

/*
 * A reader who tried an alternative and wants to leave the decision, not the value.
 *
 * A rule is bound first, inside the check, because the state this is about is the one where
 * something is already chosen. Read after the undo above, the row carries no rule, the
 * placeholder is offered for the ordinary reason, and a page that removed it the moment a rule
 * was picked would pass.
 */
const noPreference = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const select = document.querySelector('#decision-list select[data-construct]');
  const slot = state.slots.find((entry) => entry.construct === select.dataset.construct);
  const pick = [...select.options].find((option) => option.value);
  select.value = pick.value;
  select.dispatchEvent(new Event('change', { bubbles: true }));
  await new Promise((resolve) => setTimeout(resolve, 200));

  const bound = document.querySelector('#decision-list select[data-construct="' + slot.construct + '"]');
  const boundTo = state.selection[slot.key]?.methodId ?? null;
  const offered = [...bound.options].some((option) => option.value === '');
  const select2 = bound;
  select2.value = '';
  select2.dispatchEvent(new Event('change', { bubbles: true }));
  await new Promise((resolve) => setTimeout(resolve, 200));
  const analysis = await import('./analysis.js');
  const running = analysis.boundMethodId(slot.key);
  const record = state.analysis.bound_methods.find((row) => row.method_id === running);
  return {
    boundTo,
    offered,
    methodId: state.selection[slot.key]?.methodId ?? null,
    stated: state.selection[slot.key]?.methodStated ?? null,
    running,
    source: record ? record.method_source : null,
  };
})()`);
check('a rule can be left as well as chosen, and the record stops carrying the reader’s signature',
  noPreference.boundTo !== null
    && noPreference.offered
    && noPreference.methodId === null
    && noPreference.stated === false
    && noPreference.running !== null
    && noPreference.source !== 'stated',
  `with ${noPreference.boundTo} bound the placeholder was ${noPreference.offered ? 'offered' : 'GONE'}; ` +
    `after leaving it the selection is ${noPreference.methodId}, still running ${noPreference.running} under ${noPreference.source}`);

// ---------------------------------------------------------------- reaching a quantity
const pickerClosed = await evaluate(`(() => {
  const list = document.getElementById('add-quantity-list');
  return { hidden: list.hidden, offers: list.querySelectorAll('.add-quantity__option').length };
})()`);
await evaluate("document.getElementById('add-quantity-search').focus()");
await pause(80);
const pickerOpen = await evaluate(`(() => {
  const list = document.getElementById('add-quantity-list');
  return {
    hidden: list.hidden,
    offers: list.querySelectorAll('.add-quantity__option').length,
    typed: document.getElementById('add-quantity-search').value,
  };
})()`);
check('the quantity picker says what it can offer before the reader has to guess a word',
  pickerClosed.hidden && !pickerOpen.hidden && pickerOpen.typed === '' && pickerOpen.offers > 1,
  `closed with ${pickerClosed.offers}, focused with ${pickerOpen.offers} offers and "${pickerOpen.typed}" typed`);

// ------------------------------------------- the rate, the grab radius, the value in force
/*
 * The number that scales every velocity, every impulse and every height, in the record.
 *
 * Declaring a 1200 Hz recording at 1000 moves jump height from 0.409 to 0.588 m and net impulse
 * by a fifth, and a 58.8 cm countermovement jump passes without comment. The record found room
 * for the registry digest and not for this.
 */
const rateInTheRecord = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const terms = [...document.querySelectorAll('#build-info dt')].map((node) => node.textContent);
  const at = terms.indexOf('Sample rate');
  return {
    terms,
    shown: at === -1 ? null : document.querySelectorAll('#build-info dd')[at].textContent,
    running: state.info.sample_rate_hz,
  };
})()`);
check('the record names the rate the analysis ran at, and whose answer it is',
  rateInTheRecord.shown !== null
    && rateInTheRecord.shown.startsWith(`${rateInTheRecord.running} Hz`)
    && rateInTheRecord.shown.length > `${rateInTheRecord.running} Hz`.length + 2,
  rateInTheRecord.shown === null
    ? `no rate in the record, which holds ${rateInTheRecord.terms.join(', ')}`
    : `"${rateInTheRecord.shown}" against a trial running at ${rateInTheRecord.running} Hz`);

/*
 * A press near a landmark without being on it belongs to the trace.
 *
 * Driven at measured offsets with real pointer events, because the defect is a press landing on
 * the wrong control and only a press can show it. Both directions: on the line it still drags,
 * and a miss that used to move a landmark now draws a selection instead of doing nothing.
 */
await evaluate("document.getElementById('reset-markers')?.click()");
await evaluate("document.getElementById('selection-clear')?.click()");
await pause(200);
const grab = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const box = document.querySelector('.marker--takeoff').getBoundingClientRect();
  return {
    centre: box.left + box.width / 2,
    y: box.top + box.height / 2,
    width: Math.round(box.width),
    takeoff: state.analysis.takeoff_index,
  };
})()`);
const pressAt = async (offset) => {
  // Measured immediately before the press. Anything that appears or leaves above the trace
  // moves it, and a coordinate taken a moment earlier lands somewhere else.
  const now = await evaluate(`(() => {
    // Brought into view first. Earlier checks move the keyboard onto the rail, which scrolls
    // the trace off the top, and a press dispatched at a negative viewport coordinate lands
    // nowhere and reads exactly like a control that ignored it.
    document.querySelector('.panel--trace').scrollIntoView({ block: 'center' });
    const box = document.querySelector('.marker--takeoff').getBoundingClientRect();
    return {
      centre: box.left + box.width / 2,
      y: box.top + box.height / 2,
      where: [Math.round(box.left), Math.round(box.top), Math.round(box.right), Math.round(box.bottom)].join(','),
      viewport: [innerWidth, innerHeight].join('x'),
      hidden: document.getElementById('stage-workspace').hidden,
    };
  })()`);
  await pause(120);
  await mouse('mousePressed', now.centre + offset, now.y);
  for (const step of [8, 16, 26]) await mouse('mouseMoved', now.centre + offset + step, now.y);
  await mouse('mouseReleased', now.centre + offset + 26, now.y);
  await pause(240);
  const seen = await evaluate(`(async () => {
    const state = (await import('./state.js')).state;
    return { moved: state.overrides.takeoff != null, regions: state.chart.regions.length };
  })()`);
  seen.where = now.where;
  seen.viewport = now.viewport;
  seen.hidden = now.hidden;
  await evaluate("document.getElementById('reset-markers')?.click()");
  await evaluate("document.getElementById('selection-clear')?.click()");
  await pause(200);
  return seen;
};
const onTheLine = await pressAt(0);
const nearMiss = await pressAt(14);

check('a press on a landmark drags it, and a press near one draws on the trace instead',
  grab.width >= 44
    && onTheLine.moved === true
    && nearMiss.moved === false
    && nearMiss.regions === 1,
  `the element is ${grab.width} px wide for a finger; on the line the landmark ` +
    `${onTheLine.moved ? 'moved' : 'DID NOT MOVE'}, 14 px away it ` +
    `${nearMiss.moved ? 'STILL MOVED' : 'stayed'} and left ${nearMiss.regions} selection` +
    `; the landmark sat at ${onTheLine.where} in a ${onTheLine.viewport} viewport, workspace ` +
    `${onTheLine.hidden ? 'HIDDEN' : 'shown'}`);

/*
 * A value on screen that the analysis is not running under.
 *
 * The field fell back to the first value the entry publishes, so a parameter with a published
 * 0.5 and no declared default read 0.5 while the request carried nothing and the record said
 * `0 (assumed)`. A reader who opens Settings, reads that and closes it has been told something
 * untrue about their own analysis, and it is the number they will write down.
 */
const fieldsAgainstTheRecord = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const analysis = await import('./analysis.js');
  // The rule this is about is named, because the case only exists where an entry publishes a
  // value and declares no default: the search rule publishes 0.5 for its variance floor and
  // the analysis runs at 0. Whichever rule happens to be bound when this check is reached does
  // not necessarily have such a parameter, and a check that reads whatever is there passes on
  // a page doing the wrong thing.
  const weighing = document.querySelector('#decision-list select[data-construct]');
  if ([...weighing.options].some((option) => option.value === 'bwepoch.adaptive_lowest_variance')) {
    weighing.value = 'bwepoch.adaptive_lowest_variance';
    weighing.dispatchEvent(new Event('change', { bubbles: true }));
    await new Promise((resolve) => setTimeout(resolve, 240));
  }
  for (const open of document.querySelectorAll('#decision-list details')) open.open = true;
  const read = [];
  for (const field of document.querySelectorAll('#decision-list input[type="number"]')) {
    const row = field.closest('.decision');
    const select = row?.querySelector('select[data-construct]');
    if (!select) continue;
    const slot = state.slots.find((entry) => entry.construct === select.dataset.construct);
    const running = analysis.boundMethodId(slot.key);
    const record = state.analysis.bound_methods.find((bound) => bound.method_id === running);
    const name = field.id.split('-').slice(2).join('-');
    const inForce = (record?.bound_parameters || []).find(([held]) => held === name)?.[1];
    // What the entry publishes for it, which is what the field used to fall back to. Carried so
    // the check can require the interesting case to be present rather than hope for it: a
    // population where every published value happens to equal the bound one proves nothing.
    const entry = (await import('./registry.js')).findMethod(state.registry, running);
    const parameter = (entry?.parameter || []).find((held) => held.name === name);
    const published = (parameter?.published_values || [])[0];
    read.push({
      name, shown: field.value, inForce: inForce ?? null,
      published: published ?? null, placeholder: field.placeholder,
    });
  }
  return read;
})()`);
const disagreeingFields = fieldsAgainstTheRecord.filter(
  (field) => field.inForce !== null && field.shown !== '' && Number(field.shown) !== Number(field.inForce),
);
// The case this is about: a parameter the entry publishes a value for and the analysis is
// running under a different one. Without at least one of those the check cannot fail.
const divergent = fieldsAgainstTheRecord.filter(
  (field) => field.published !== null && field.inForce !== null
    && Number(field.published) !== Number(field.inForce),
);
check('every parameter on screen is the value the analysis is running under',
  fieldsAgainstTheRecord.length > 0
    && divergent.length > 0
    && divergent.every((field) => Number(field.shown) === Number(field.inForce))
    && disagreeingFields.length === 0,
  `${fieldsAgainstTheRecord.length} fields read, ` +
    `${fieldsAgainstTheRecord.filter((field) => field.inForce !== null).length} of them bound in the record, ` +
    `${divergent.length} where the published value differs from the bound one` +
    (divergent.length ? ` (${divergent[0].name}: shows ${divergent[0].shown}, bound ${divergent[0].inForce}, published ${divergent[0].published})` : ', so the case this is about was not reachable') +
    `, ${disagreeingFields.length} disagreeing` +
    (disagreeingFields.length
      ? `: ${disagreeingFields[0].name} shows ${disagreeingFields[0].shown} against ${disagreeingFields[0].inForce}`
      : ''));

/*
 * Two trials of one athlete, opened one after the other, computed under the same rules.
 *
 * A student picking rules for trial 1, pressing New file and opening trial 3 had every choice
 * discarded, so the two were computed under different rules and both pastes read as equally
 * authoritative. The folder route already applies one path to every trial in it; this was the
 * route that diverged.
 *
 * Both directions. The rules carry and the page says so. A landmark does not, because an index
 * is a sample of one recording and carrying it onto another would be the same defect with the
 * opposite sign.
 */
await evaluate("document.getElementById('reset-markers')?.click()");
await pause(160);
const beforeTheNextTrial = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const select = document.querySelector('#decision-list select[data-construct]');
  const slot = state.slots.find((entry) => entry.construct === select.dataset.construct);
  const pick = [...select.options].find((option) => option.value);
  select.value = pick.value;
  select.dispatchEvent(new Event('change', { bubbles: true }));
  await new Promise((resolve) => setTimeout(resolve, 200));
  // A landmark placed by hand on this recording, which must not follow the reader to another.
  const marker = state.chart.markers.find((entry) => entry.key === 'onset').element;
  marker.focus();
  marker.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  await new Promise((resolve) => setTimeout(resolve, 200));
  return {
    key: slot.key,
    chose: state.selection[slot.key]?.methodId ?? null,
    override: state.overrides.onset,
  };
})()`);
await evaluate("document.getElementById('change-file').click()");
await settle("!document.getElementById('stage-empty').hidden", 'the empty stage between trials');
await evaluate("document.getElementById('load-demo').click()");
await settle("!document.getElementById('stage-workspace').hidden", 'the next trial');
await settle("(async () => Boolean((await import('./state.js')).state.analysis))()", 'its analysis');
await pause(200);
const onTheNextTrial = await evaluate(`(async () => {
  const state = (await import('./state.js')).state;
  const line = document.getElementById('record-change');
  return {
    carried: state.selection['${beforeTheNextTrial.key}']?.methodId ?? null,
    stated: state.selection['${beforeTheNextTrial.key}']?.methodStated ?? null,
    override: state.overrides.onset,
    weighingStart: state.weighing.startIndex,
    said: line && !line.hidden ? line.textContent : null,
    spoken: document.getElementById('chart-announcement').textContent,
    carriedCount: Object.keys(state.carried?.selection || {}).length,
  };
})()`);
check('the rules a reader chose follow them to the next trial, and the landmarks they placed do not',
  beforeTheNextTrial.chose !== null
    && beforeTheNextTrial.override != null
    && onTheNextTrial.carried === beforeTheNextTrial.chose
    && onTheNextTrial.stated === true
    && onTheNextTrial.override === null
    && onTheNextTrial.weighingStart === null,
  `${beforeTheNextTrial.chose} chosen with onset placed at ${beforeTheNextTrial.override}; ` +
    `the next trial runs ${onTheNextTrial.carried} with onset ${onTheNextTrial.override} ` +
    `and no hand-placed window (${onTheNextTrial.weighingStart})`);

check('and it says what it carried rather than letting the rules arrive unmentioned',
  /carried from/.test(onTheNextTrial.said || ''),
  onTheNextTrial.said
    ? `"${onTheNextTrial.said}"`
    : `nothing drawn; the live region holds "${onTheNextTrial.spoken}" and ${onTheNextTrial.carriedCount} rules were carried`);

// ---------------------------------------------------------------- a choice that outlives the tab
const themeChosen = await evaluate(`(() => {
  const before = document.documentElement.dataset.theme;
  document.getElementById('theme-toggle').click();
  return {
    before,
    after: document.documentElement.dataset.theme,
    stored: window.localStorage.getItem('plateforce.theme'),
    says: document.getElementById('theme-toggle').getAttribute('aria-label'),
  };
})()`);
await send('Page.navigate', { url: `http://127.0.0.1:${port}/index.html` });
await settle("!document.getElementById('stage-empty').hidden", 'the reloaded empty stage');
const themeKept = await evaluate("document.documentElement.dataset.theme");
check('the colour a reader chose outlives the tab, and the control says which it would switch to',
  themeChosen.after !== themeChosen.before
    && themeChosen.stored === themeChosen.after
    && themeKept === themeChosen.after
    && /Switch to (light|dark) colours/.test(themeChosen.says || ''),
  `${themeChosen.before} to ${themeChosen.after}, stored ${themeChosen.stored}, ` +
    `reopened on ${themeKept}, control says "${themeChosen.says}"`);

const failures = results.filter((result) => !result.passed);
report();

socket.close();
server.close();
process.exit(failures.length || consoleLines.length ? 1 : 0);
