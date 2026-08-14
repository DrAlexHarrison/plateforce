/* The trace, the landmarks drawn on it, and the gestures that move them. */

import { TraceChart, landmarkDefinitions } from './chart.js';
import { $, state } from './state.js';
import { counted, element, formatNumber, setWindowTitle, showStage, typesetUnit } from './format.js';
import { windowLengthParameter, buildDecisionModel } from './registry.js';
import { resetSelections, candidateFor } from './startup.js';
import { renderDecisions } from './decisions.js';
import { boundMethodId, runAnalysis, recordStated, withSources, HEADLINE } from './analysis.js';
import { endingOf } from './batch-run.js';
import { renderPicker, putOnThePath, removeFromPath } from './add-quantity.js';
import { findMethod } from './registry.js';
import { copyButton, putImage } from './copy.js';
import { buildRequest } from './analysis.js';
import { captureJson } from './plate.js';
import {
  snapshot, remember, clearHistory, undo, redo, restore, updateHistoryControls,
} from './history.js';

/*
 * The two rules a span selected on the trace binds, named by their registry ids the way the
 * hand-placed weighing window is. The construct they fill is read off the build rather than
 * written down here, so the rail can gain or lose a row without an edit in this file.
 *
 * They are two rules and not one because the two spans are two claims. A span the reader
 * dragged has ends nobody published; a span they double-clicked into has ends a bound rule
 * placed, and the record has to be able to say which of the two produced a number.
 */
const STATED_WINDOW = 'window.stated.by_caller';
const PHASE_WINDOW = 'window.from_named_phase';
const PHASE_NAME = 'phase';

export function enterWorkspace() {
  setWindowTitle(state.fileName);
  state.overrides = { onset: null, takeoff: null, touchdown: null };
  resetSelections();
  clearHistory();
  recordTheOpeningSelection();
  showStage('stage-workspace');
  offerTheRun();

  const info = JSON.parse(state.loadedTrial.infoJson());
  state.info = info;
  $('trial-summary').textContent =
    `${info.sample_count.toLocaleString()} samples, ${info.sample_rate_hz} Hz, ${info.duration_seconds.toFixed(2)} s` +
    (info.samples_matching_the_convention ? `, ${info.samples_matching_the_convention} matched the missing-value convention` : '') +
    (info.samples_carrying_no_number ? `, ${info.samples_carrying_no_number} missing samples` : '');

  if (!state.chart) {
    const container = $('chart');
    state.chart = new TraceChart({
      container,
      canvas: $('chart-canvas'),
      overlay: $('chart-overlay'),
      markers: landmarkDefinitions(state.registry, state.slots),
      onMarkerMove: (key, index) => {
        state.overrides[key] = Math.max(0, Math.min(state.info.sample_count - 1, index));
        runAnalysis();
      },
      onMarkerEditStart: () => {
        rememberTheNumbers();
        return snapshot();
      },
      onMarkerEditEnd: (key, before) => {
        const landmark = state.chart.markers.find((marker) => marker.key === key);
        remember(before, `moving ${landmark?.label ?? key}`);
        announceLandmark(key);
      },
      onWindowChange: (startIndex, durationSeconds) => {
        // Placing the window by hand is a registry entry in its own right, so the drag
        // rebinds the method rather than overriding whichever rule was selected.
        state.weighing = { startIndex };
        const placed = candidateFor('weighing', 'bwepoch.manual_placement');
        // A window placed by hand is the reader's own statement, and a stronger one than a
        // pick from a list: the span they dragged to is in no paper.
        if (placed) {
          state.selection.weighing = {
            methodId: placed.id, values: {}, options: {}, unresolved: [],
            fromDefault: new Set(), recommended: new Set(), methodFromRecommendation: false,
            methodStated: true,
            // A rule that arrived by a gesture rather than by a pick from the list, which is
            // the population the recommendation offers to take back. A rule the reader chose
            // from the list is a published rule and is not among them.
            placedByHand: true,
          };
        }
        const selection = withSources(state.selection.weighing);
        const length = windowLengthParameter(candidateFor('weighing', selection.methodId));
        if (length) {
          selection.values[length] = durationSeconds;
          selection.unresolved = (selection.unresolved || []).filter((name) => name !== length);
          recordStated(selection, length);
        }
        renderDecisions();
        runAnalysis();
      },
      onWindowEditStart: () => {
        rememberTheNumbers();
        return snapshot();
      },
      onWindowEditEnd: (before) => {
        remember(before, 'moving the standing-still window');
        announceWindow();
      },
      onViewChange: () => {
        refreshEnvelope();
        updateChartNavigation();
      },
      onSelectionChange: (selection, event) => selectionChanged(selection, event),
      regionLabel: (phase) => phaseLabels().get(phase) || phase,
    });
    container.addEventListener('chart:resize', () => refreshEnvelope());
  }

  state.chart.setRecording(info.sample_count, info.sample_rate_hz);
  wireChartNavigation();
  wireSelectionControls();
  updateChartNavigation();
  renderLegend();
  refreshEnvelope();
  renderPicker();
  renderDecisions();
  runAnalysis();
}

/* ---------------------------------------------------- saying what an edit did */

/*
 * What an edit on the trace did, for a reader who cannot see the trace change.
 *
 * A landmark under the keyboard moves by one sample, which is a change of well under a pixel
 * on a fitted 5-second trace and nothing at all to look at. The record already carries the
 * new instant and the numbers that followed it, so what is spoken is read from the analysis
 * rather than composed: the landmark and where it now sits, then the two headline quantities
 * that moved, then how many others did.
 */
function announce(sentence) {
  const host = $('chart-announcement');
  if (!host) return;
  // Replaced rather than appended, and cleared first, so two nudges to the same place are two
  // announcements rather than one unchanged string a reader is never told about again.
  host.textContent = '';
  window.setTimeout(() => { host.textContent = sentence; }, 0);
}

/* The reported numbers as they stood before the edit began, so the sentence after it can say
 * which of them the edit moved. */
let numbersBeforeTheEdit = new Map();

function rememberTheNumbers() {
  numbersBeforeTheEdit = new Map(
    (state.analysis?.metrics || []).map((metric) => [metric.key, metric.value]),
  );
}

function shown(metric) {
  const formatted = formatNumber(metric.value, metric.unit);
  return formatted == null ? 'no value' : `${formatted} ${metric.unit_symbol}`;
}

/* The quantities this edit moved, the headline ones by name and the rest by count. Reading
 * every changed number aloud is eleven sentences on a trial that reports eleven. */
function whatFollowed() {
  const moved = (state.analysis?.metrics || []).filter(
    (metric) => numbersBeforeTheEdit.has(metric.key)
      && numbersBeforeTheEdit.get(metric.key) !== metric.value,
  );
  if (!moved.length) return 'No reported number changed.';
  const named = moved.filter((metric) => HEADLINE.has(metric.key));
  const rest = moved.length - named.length;
  const sentences = named.map((metric) => {
    const was = formatNumber(numbersBeforeTheEdit.get(metric.key), metric.unit);
    return `${metric.label} ${shown(metric)}, was ${was == null ? 'no value' : `${was} ${metric.unit_symbol}`}.`;
  });
  if (rest) sentences.push(`${counted(rest, 'other reported number')} changed.`);
  return sentences.join(' ');
}

function announceLandmark(key) {
  const landmark = state.chart?.markers.find((marker) => marker.key === key);
  const index = state.analysis?.[`${key}_index`];
  if (!landmark || index == null || !state.info) return;
  announce(
    `${landmark.label} at ${(index / state.info.sample_rate_hz).toFixed(4)} seconds, ` +
    `sample ${index}. ${whatFollowed()}`,
  );
}

function announceWindow() {
  const start = state.analysis?.weighing_start_index;
  const end = state.analysis?.weighing_end_index;
  if (start == null || end == null || !state.info) return;
  const rate = state.info.sample_rate_hz;
  announce(
    `Standing-still window ${(start / rate).toFixed(4)} to ${(end / rate).toFixed(4)} seconds. ` +
    whatFollowed(),
  );
}

/* ---------------------------------------------------------------- history */

function restoreEdit(held, verb) {
  if (!held) return;
  restore(held);
  state.slots = buildDecisionModel(state.registry, state.build, state.path);
  renderPicker();
  renderDecisions();
  runAnalysis();
  announce(held.label ? `${verb} ${held.label}.` : `${verb} the last edit on the trace.`);
}

export function undoEdit() {
  rememberTheNumbers();
  restoreEdit(undo(), 'Undid');
}

export function redoEdit() {
  rememberTheNumbers();
  restoreEdit(redo(), 'Redid');
}

export function resetLandmarks() {
  const before = snapshot();
  rememberTheNumbers();
  state.overrides = { onset: null, takeoff: null, touchdown: null };
  remember(before, 'resetting the landmarks');
  runAnalysis();
  announce(`Landmarks back where their rules place them. ${whatFollowed()}`);
}

export function applySelectionAsStandingStill() {
  const selected = state.chart?.selection().active;
  if (!selected || !state.info) return;
  const before = snapshot();
  rememberTheNumbers();
  const durationSeconds = (selected.endIndex - selected.startIndex) / state.info.sample_rate_hz;
  state.chart.onWindowChange(selected.startIndex, durationSeconds);
  remember(before, 'using the selection as the standing-still window');
  announceWindow();
}

export function wireHistoryControls() {
  const undoButton = $('undo-edit');
  if (undoButton.dataset.wired === 'true') return;
  undoButton.dataset.wired = 'true';
  undoButton.addEventListener('click', undoEdit);
  $('redo-edit').addEventListener('click', redoEdit);
  updateHistoryControls();
}

/*
 * The rest of the folder, offered from the trial it was declared on.
 *
 * The action names the reader's own count so the run they are about to start is the run
 * they think it is, and it sits beside the trace because the rules it will carry are the
 * ones the rail beside that trace is holding.
 */
function offerTheRun() {
  const action = $('run-folder');
  const named = state.run
    ? state.run.files.filter((file) => state.run.endings.has(endingOf(file.name))).length
    : 0;
  action.hidden = !state.run;
  if (state.run) {
    action.textContent = named === 1
      ? 'Run the one trial in this folder'
      : `Run all ${named} trials in this folder`;
  }
}

/*
 * Everything the software bound before the reader arrived, stamped beside the call that
 * binds it.
 *
 * A slot that forces no decision opens with its rule already chosen and its registry
 * defaults already filled, so this is where most defaulted values on the page enter, and
 * every one of them would otherwise be indistinguishable from a value the reader stated.
 */
function recordTheOpeningSelection() {
  for (const slot of state.slots) {
    const selection = withSources(state.selection[slot.key]);
    if (!selection.methodId) continue;
    for (const name of Object.keys(selection.values)) selection.fromDefault.add(name);
    for (const name of Object.keys(selection.options || {})) selection.fromDefault.add(name);
  }
}

export function refreshEnvelope() {
  if (!state.loadedTrial || !state.chart) return;
  const { start, end } = state.chart.visibleRange();
  state.envelope = JSON.parse(
    state.loadedTrial.windowEnvelopeJson(state.chart.plotWidthPx(), start, end + 1),
  );
  state.envelope.start_index = start;
  state.envelope.end_index = end;
  state.chart.setEnvelope(state.envelope);
  state.chart.schedule();
}

function wireChartNavigation() {
  const nav = $('chart-nav');
  if (nav.dataset.wired === 'true') return;
  nav.dataset.wired = 'true';
  $('chart-zoom-in').addEventListener('click', () => state.chart.zoom(0.5));
  $('chart-zoom-out').addEventListener('click', () => state.chart.zoom(2));
  $('chart-fit').addEventListener('click', () => state.chart.fit());
  $('chart-pan').addEventListener('input', () => state.chart.pan(Number($('chart-pan').value) / 1000));

  let wheelScale = 1;
  let wheelPan = 0;
  let wheelAnchor = null;
  let wheelTimer = null;
  $('chart').addEventListener('wheel', (event) => {
    const zooming = event.metaKey || event.ctrlKey;
    const panning = event.shiftKey && !zooming;
    if (!zooming && !panning) return;
    event.preventDefault();
    const delta = event.deltaY || event.deltaX;
    if (zooming) {
      wheelScale *= Math.exp(Math.min(0.35, Math.max(-0.35, delta * 0.002)));
      wheelAnchor = state.chart.sampleAtClientX(event.clientX);
    } else {
      wheelPan += Math.min(0.5, Math.max(-0.5, delta / 600));
    }
    window.clearTimeout(wheelTimer);
    wheelTimer = window.setTimeout(() => {
      if (wheelScale !== 1 && wheelAnchor != null) state.chart.zoomAt(wheelScale, wheelAnchor);
      if (wheelPan !== 0) state.chart.panBy(wheelPan);
      wheelScale = 1;
      wheelPan = 0;
      wheelAnchor = null;
    }, 24);
  }, { passive: false });

  document.addEventListener('keydown', (event) => {
    if ($('stage-workspace').hidden || event.altKey) return;
    const field = event.target instanceof Element
      && event.target.closest('input, textarea, select, [contenteditable="true"]');
    if (field) return;
    const actions = {
      '+': () => state.chart.zoom(0.8),
      '=': () => state.chart.zoom(0.8),
      '-': () => state.chart.zoom(1.25),
      '0': () => state.chart.fit(),
    };
    const action = actions[event.key];
    if (!action) return;
    event.preventDefault();
    action();
  });

  $('copy-chart-image').addEventListener('click', () => copyChartImage());
  $('save-chart-image').addEventListener('click', () => saveChartImage());
}

function chartImageNotes() {
  const rate = state.info.sample_rate_hz;
  const view = state.chart.visibleRange();
  const notes = [
    `plateforce chart. Trial: ${state.fileName}.`,
    `Visible range: ${(view.start / rate).toFixed(4)} to ${(view.end / rate).toFixed(4)} s. Display range chosen in chart.`,
    `Registry revision: ${state.build.registry_declared_version ?? 'none declared'}. Registry digest: ${state.build.registry_digest}.`,
  ];
  // The rules behind the levels and bands the trace draws, which are the rules for the three
  // constructs the request names by its own fields. Read off the build rather than listed.
  const traceRules = uniqueRules(
    state.slots.filter((slot) => slot.spine).map((slot) => boundMethodId(slot.key)),
  );
  if (traceRules.length) notes.push(`Trace levels and bands. ${ruleText(traceRules)}.`);
  for (const landmark of majorLandmarks()) {
    notes.push(
      `${landmark.label}: ${(landmark.index / rate).toFixed(4)} s, sample ${landmark.index}. ` +
      `${ruleText(landmark.rules)}.`,
    );
  }
  const selected = state.chart.selection().active;
  if (selected) {
    notes.push(
      `Selected window: ${(selected.startIndex / rate).toFixed(4)} to ` +
      `${(selected.endIndex / rate).toFixed(4)} s, ` +
      `${selected.endIndex - selected.startIndex + 1} samples. ` +
      `${ruleText(uniqueRules(boundWindowRule()))}.`,
    );
  }
  return notes;
}

function imageName() {
  const stem = String(state.fileName || 'plateforce')
    .replace(/\.[^.]+$/, '')
    .replace(/[^a-z0-9_-]+/gi, '-')
    .replace(/^-+|-+$/g, '') || 'plateforce';
  return `${stem}-chart.png`;
}

function reportImageAction(button, message) {
  const label = button.dataset.label || button.textContent;
  button.dataset.label = label;
  button.textContent = message;
  window.clearTimeout(button.reportTimer);
  button.reportTimer = window.setTimeout(() => { button.textContent = label; }, 2000);
}

async function copyChartImage() {
  const button = $('copy-chart-image');
  const copied = await putImage(state.chart.imageBlob(chartImageNotes()));
  reportImageAction(button, copied ? 'Copied chart image' : 'Could not reach the clipboard');
}

function saveChartImage() {
  const button = $('save-chart-image');
  const url = URL.createObjectURL(state.chart.imageBlob(chartImageNotes()));
  const link = document.createElement('a');
  link.href = url;
  link.download = imageName();
  document.body.append(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
  reportImageAction(button, `Saved ${imageName()}`);
}

function updateChartNavigation() {
  if (!state.chart || !state.info) return;
  const nav = $('chart-nav');
  nav.hidden = state.info.duration_seconds <= 10;
  const { start, end } = state.chart.visibleRange();
  const span = end - start;
  const available = Math.max(0, state.info.sample_count - 1 - span);
  $('chart-pan').value = available > 0 ? String(Math.round((start / available) * 1000)) : '0';
  $('chart-pan').disabled = available === 0;
  $('chart-zoom-out').disabled = state.chart.isFit();
  $('chart-fit').disabled = state.chart.isFit();
  $('chart-window-label').textContent =
    `${(start / state.info.sample_rate_hz).toFixed(2)}–${(end / state.info.sample_rate_hz).toFixed(2)} s`;
  // The two zoom controls that read the view rather than the selection, so stepping back out
  // of a zoom leaves them saying what the view can still do.
  if (state.chart) updateSelectionControls(state.chart.selection());
}

/* ------------------------------------------------------- selecting a window */

/* The construct the two window rules fill, read off the build. */
function windowConstruct() {
  return state.build.bindings.find((binding) => binding.id === STATED_WINDOW)?.construct || null;
}

/* The registry's own words for each interval a caller can name, keyed by the name they state.
 * Written nowhere here: an interval the registry adds arrives with its label. */
function phaseLabels() {
  const parameter = (findMethod(state.registry, PHASE_WINDOW)?.parameter || [])
    .find((entry) => entry.name === PHASE_NAME);
  return new Map((parameter?.value || []).map((value) => [value.key, value.label]));
}

/*
 * A span the reader selected, bound as the rule that describes how they selected it.
 *
 * Dragging binds the stated rule and states the two instants, exactly as dragging the weighing
 * window binds the hand-placement rule and states its length. Double-clicking a placed phase
 * binds the rule that takes the window from that phase and names the phase, so the two ends stay
 * the boundary rules' and the record never reports them as the reader's.
 */
function bindTheWindow(region) {
  const construct = windowConstruct();
  if (!construct) return;
  putOnThePath(construct);
  state.windowCameFromASelection = true;
  const rate = state.info.sample_rate_hz;
  const stated = {
    methodId: STATED_WINDOW,
    values: { start_seconds: region.startIndex / rate, end_seconds: region.endIndex / rate },
    options: {},
  };
  const placed = { methodId: PHASE_WINDOW, values: {}, options: { [PHASE_NAME]: region.placed?.phase } };
  const choice = region.stated ? stated : placed;
  state.selection[construct] = {
    ...choice,
    unresolved: [],
    // Every name in this selection was put there by the act of selecting, so none of them is a
    // registry default and none was accepted from a recommendation.
    fromDefault: new Set(),
    recommended: new Set(),
    methodFromRecommendation: false,
    methodStated: true,
  };
}

/* Clearing takes the construct back off the path where selecting is what put it there, so the
 * reader ends where they began rather than with a window rule bound to a span that is gone. */
function releaseTheWindow() {
  const construct = windowConstruct();
  if (!construct || !state.windowCameFromASelection) return;
  state.windowCameFromASelection = false;
  removeFromPath(construct);
  for (const essential of state.selectionEssentials) removeFromPath(essential);
  state.selectionEssentials.clear();
}

function selectionChanged(selection, event) {
  // The extent is drawn and reported on every frame of a drag, because it costs nothing: it is
  // read off two indices. The numbers cost an analysis, so they follow on a trailing edge.
  if (event.dragging) {
    renderSelectionReadout(selection, event);
    scheduleTheNumbers(event.dragging);
    return;
  }

  window.clearTimeout(trailingEdge);
  if (event.nudged) rememberTheNumbers();
  if (selection.active) bindTheWindow(selection.active);
  else releaseTheWindow();
  renderSelectionReadout(selection, event);
  renderDecisions();
  runAnalysis();
  if (event.nudged && selection.active) announceSelection(selection.active, event.nudged);
}

/* A span moved by the keyboard, said out loud: what it now covers, and which of the numbers
 * taken over it followed. The two gestures are named apart because a span that grew and a
 * span that slid cover different samples for the same key press. */
function announceSelection(selected, how) {
  const rate = state.info.sample_rate_hz;
  const verb = how === 'end' ? 'Selection end moved' : 'Selection moved';
  announce(
    `${verb}. ${(selected.startIndex / rate).toFixed(4)} to ${(selected.endIndex / rate).toFixed(4)} seconds, ` +
    `${counted(selected.endIndex - selected.startIndex + 1, 'sample')}. ${whatFollowed()}`,
  );
}

let trailingEdge = null;

/*
 * The numbers for a window still being dragged, asked for once the pointer has paused.
 *
 * The wait is the last analysis's own duration rather than a constant, so the engine is never
 * asked again before it has had as long as its last answer took. That is the whole of what the
 * measurement settles: one analysis is 43 ms over 6,000 samples and 410 ms over 72,000, and the
 * count of rules on the path barely moves it, so a rate written down here would be right for one
 * recording length and wrong for every other.
 */
function scheduleTheNumbers(span) {
  window.clearTimeout(trailingEdge);
  const wait = Math.min(600, Math.max(60, Math.round(state.analysisMilliseconds ?? 100)));
  trailingEdge = window.setTimeout(() => {
    trailingEdge = null;
    bindTheWindow({ ...span, stated: true });
    renderDecisions();
    runAnalysis();
  }, wait);
}

/* What is selected, as the reader's own data: the extent both ways they work in, and where the
 * two ends came from. The origin is the half that matters, because two spans covering the same
 * samples are different claims when one was drawn and the other was placed. */
function renderSelectionReadout(selection, event) {
  const host = $('chart-selection-readout');
  const span = event.dragging || selection.active;
  host.replaceChildren();

  // Answered whether or not something is selected, because a reader who double-clicked and got
  // nothing asked a question, and reading back the window they already had is not the answer.
  // How to select at all is said once, under the trace, rather than repeated here.
  if (event.placedNothingHere) {
    host.append(element('span', 'chart-selection__origin',
      'No phase boundary reaches this point. Put a phase boundary on the path to select one.'));
  }
  if (!span) {
    updateSelectionControls(selection, event);
    return;
  }

  const rate = state.info.sample_rate_hz;
  const from = (span.startIndex / rate).toFixed(4);
  const to = (span.endIndex / rate).toFixed(4);
  const samples = span.endIndex - span.startIndex + 1;
  host.append(element('span', 'chart-selection__span', `${from} to ${to} s, ${counted(samples, 'sample')}`));

  if (event.dragging) {
    host.append(element('span', 'chart-selection__origin', 'Release to select this window.'));
  } else if (span.stated) {
    host.append(element('span', 'chart-selection__origin',
      `Selected by you. Window rule: ${STATED_WINDOW}.`));
  } else {
    const rules = (span.placed.placed_by || []).join(', ');
    const label = phaseLabels().get(span.placed.phase) || span.placed.phase;
    host.append(element('span', 'chart-selection__origin',
      `${label}, placed by ${rules}. Window rule: ${PHASE_WINDOW}.`));
  }
  if (selection.regions.length > 1) {
    host.append(element('span', 'chart-selection__origin', `${selection.regions.length} windows selected. Numbers are taken over this one.`));
  }
  updateSelectionControls(selection, event);
}

/* The rule the window is bound to right now, or nothing where the reader has no window. */
function boundWindowRule() {
  const construct = windowConstruct();
  if (!construct || !state.path.includes(construct)) return null;
  return state.selection[construct]?.methodId || null;
}

function methodForConstruct(construct) {
  const slot = state.slots.find((entry) => entry.construct === construct);
  if (slot) return boundMethodId(slot.key);
  return (state.analysis?.bound_methods || []).find((bound) =>
    findMethod(state.registry, bound.method_id)?.construct === construct)?.method_id || null;
}

function uniqueRules(...rules) {
  return [...new Set(rules.flat().filter(Boolean))];
}

function ruleText(rules) {
  return `${rules.length === 1 ? 'Rule' : 'Rules'}: ${rules.join(', ')}`;
}

/* The instants the trace draws, each with the rules that placed it. One home: the chart image
 * footer, the landmarks inside a selection, and the drawer that explains a number all read
 * this, so no two of them can name the same instant differently. */
export function majorLandmarks() {
  if (!state.analysis || !state.info || !state.chart) return [];
  const flightTimeRule = state.analysis.metrics
    .find((metric) => metric.key === 'flight_time_seconds')?.computed_by;
  // Each track carries the construct it stands for, so the pairing is read from the one table
  // that declares it rather than spelt out again here.
  return state.chart.markers
    .map((marker) => ({
      key: marker.key,
      label: marker.label,
      index: state.analysis[`${marker.key}_index`],
      // A landing the flight-time rule timed is that rule's answer as much as the boundary
      // rule's, and on a recording that ends in the air neither of them places one.
      rules: uniqueRules(
        methodForConstruct(marker.construct),
        marker.key === 'touchdown' ? flightTimeRule : null,
      ),
    }))
    .filter((event) => event.index != null && event.rules.length);
}

function landmarksInside(selected) {
  return majorLandmarks().filter(
    (event) => event.index >= selected.startIndex && event.index <= selected.endIndex,
  );
}

/* Flight runs from the last landmark before the athlete comes back down to the one that says
 * they have. Named by where they sit on the trace rather than by the constructs behind them,
 * so a recording that ends in the air, which is five of six fixtures, has no such interval
 * rather than a wrong one. */
function phaseContext(selected) {
  const landmarks = majorLandmarks();
  const landing = landmarks.find((event) => event.key === 'touchdown');
  if (!landing) return null;
  const takeoff = landmarks
    .filter((event) => event.index < landing.index)
    .sort((left, right) => right.index - left.index)[0];
  if (!takeoff) return null;
  if (selected.startIndex <= takeoff.index || selected.endIndex >= landing.index) return null;
  const rate = state.info.sample_rate_hz;
  return 'Inside flight. ' +
    `${takeoff.label}: ${(takeoff.index / rate).toFixed(4)} s under ${takeoff.rules.join(', ')}. ` +
    `${landing.label}: ${(landing.index / rate).toFixed(4)} s under ${landing.rules.join(', ')}.`;
}

function appendFigures(host, entries, className = '') {
  const figures = element('dl', `chart-selection__figures ${className}`.trim());
  for (const entry of entries) {
    const figure = element('div', 'chart-selection__figure');
    figure.append(element('dt', null, entry.label));
    figure.append(element('dd', null, entry.value));
    figure.append(element('span', 'chart-selection__method', ruleText(entry.rules)));
    figures.append(figure);
  }
  host.append(figures);
}

/*
 * The numbers this window produced, and the ones that are not about it.
 *
 * A quantity is over the window when the record puts the window's rule in its chain, which the
 * record already answers, so nothing here holds a list of which quantities those are and a rule
 * that starts reading the window appears without an edit in this file.
 *
 * The rest are not blanked and not printed beside them. A quantity resting on a landmark this
 * window does not bound is the trial's number rather than this window's, and a panel that showed
 * a jump height beside a peak taken over a hand-drawn span would be claiming the drag moved both.
 */
export function renderSelectionNumbers() {
  const host = $('chart-selection-numbers');
  if (!host) return;
  host.replaceChildren();
  const bound = boundWindowRule();
  const selected = state.chart?.selection().active;
  host.hidden = !bound || !selected || !state.analysis;
  if (host.hidden) {
    $('selection-copy').replaceChildren();
    return;
  }

  const over = [];
  const elsewhere = [];
  for (const metric of state.analysis.metrics) {
    if (metric.computed_by === bound) continue;
    const reads = (metric.contributing_method_ids || []).includes(bound);
    (reads ? over : elsewhere).push(metric);
  }

  if (over.length) {
    appendFigures(host, over.map((metric) => {
      const shown = formatNumber(metric.value, metric.unit);
      return {
        label: metric.label,
        value: shown == null ? 'no value' : `${shown} ${typesetUnit(metric.unit_symbol)}`,
        rules: uniqueRules(metric.computed_by, metric.contributing_method_ids),
      };
    }));
  }

  const contained = landmarksInside(selected);
  host.append(element('p', 'chart-selection__heading', 'Landmarks in this window'));
  if (contained.length) {
    const rate = state.info.sample_rate_hz;
    appendFigures(host, contained.map((event) => ({
      label: event.label,
      value: `${(event.index / rate).toFixed(4)} s`,
      rules: event.rules,
    })), 'chart-selection__landmarks');
  } else {
    host.append(element('p', 'chart-selection__no-landmarks',
      'No start, takeoff, or landing landmark falls inside this window.'));
  }
  const context = phaseContext(selected);
  if (context) host.append(element('p', 'chart-selection__phase', context));

  // What a reader copies from beside a selection is what that selection produced. The block
  // names the window's own rule and the values behind it, so a paste says which span the peak
  // was taken over rather than handing a model a number and no interval.
  $('selection-copy').replaceChildren(
    copyButton(
      'Copy selected values',
      () => state.loadedTrial.markdown(JSON.stringify(buildRequest()), state.fileName, captureJson(), bound),
      () => {
        const labels = [...over.map((metric) => metric.label), ...contained.map((event) => event.label)];
        const named = labels.length === 0
          ? 'selection and methods'
          : labels.length === 1
            ? labels[0]
            : `${labels.slice(0, -1).join(', ')} and ${labels.at(-1)}`;
        return `Copied ${named}${contained.length ? '' : '; no landmarks in range'}`;
      },
    ),
  );

  if (elsewhere.length) {
    const named = elsewhere.slice(0, 3).map((metric) => metric.label.toLowerCase());
    const rest = elsewhere.length - named.length;
    host.append(element('p', 'chart-selection__elsewhere',
      `${named.join(', ')}${rest > 0 ? ` and ${rest} more` : ''} are not taken over this window, ` +
      'so they are the trial\'s numbers rather than this window\'s.'));
  }
}

/* The row arrives with the first window and stays while a zoom it drove can still be stepped
 * back out of. A reader with nothing selected has nothing here to act on, and on a phone the
 * space it would hold is two screens of the reading order it sits above. */
function updateSelectionControls(selection, event = {}) {
  const selected = selection.regions.length > 0;
  const undoable = state.chart.canUndoZoom();
  $('chart-selection').hidden = !(selected || undoable || event.dragging || event.placedNothingHere);
  const states = [
    ['selection-zoom', !selected, 'Drag across the trace to select a window to zoom to'],
    ['selection-clear', !selected, 'Drag across the trace to select a window'],
    ['selection-use-baseline', !selected, 'Drag across a quiet stretch to select it'],
    ['selection-undo-zoom', !undoable, 'Zoom to a selected window to have a view to step back out of'],
    ['selection-reset-zoom', state.chart.isFit() && !undoable, 'The whole recording is already fitted'],
  ];
  // A control that cannot be pressed says what would let it be, because a row of grey
  // controls with no reason beside them is a reader guessing which gesture they are missing.
  for (const [id, off, reason] of states) {
    const control = $(id);
    control.disabled = off;
    if (off) control.title = reason;
    else control.removeAttribute('title');
  }
}

function wireSelectionControls() {
  const controls = $('chart-selection');
  if (controls.dataset.wired === 'true') return;
  controls.dataset.wired = 'true';
  $('selection-zoom').addEventListener('click', () => state.chart.zoomToSelection());
  $('selection-undo-zoom').addEventListener('click', () => state.chart.undoZoom());
  $('selection-reset-zoom').addEventListener('click', () => state.chart.resetZoom());
  $('selection-clear').addEventListener('click', () => state.chart.clearSelection());
  $('selection-use-baseline').addEventListener('click', applySelectionAsStandingStill);
  renderSelectionReadout({ regions: [], active: null }, {});
}

function renderLegend() {
  const entries = [
    ['var(--trace)', 'vGRF'],
    ['var(--accent)', 'System weight + k SD band'],
    ['var(--mark-threshold)', 'Takeoff force level'],
    ...state.chart.markers.map((marker) => [`var(--track-${marker.key})`, marker.label]),
  ];
  const legend = $('chart-legend');
  legend.replaceChildren(
    ...entries.map(([colour, label]) => {
      const wrap = element('span');
      const swatch = element('i');
      swatch.style.borderTopColor = colour;
      wrap.append(swatch, document.createTextNode(label));
      return wrap;
    }),
  );
}
