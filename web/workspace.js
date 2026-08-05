/* The trace, the landmarks drawn on it, and the gestures that move them. */

import { TraceChart, landmarkDefinitions } from './chart.js';
import { $, state } from './state.js';
import { element, setWindowTitle, showStage } from './format.js';
import { windowLengthParameter } from './registry.js';
import { resetSelections, candidateFor } from './startup.js';
import { renderDecisions } from './decisions.js';
import { runAnalysis, recordStated, withSources } from './analysis.js';
import { endingOf } from './batch-run.js';
import { renderPicker, putOnThePath, removeFromPath } from './add-quantity.js';
import { findMethod } from './registry.js';

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
  if (state.run) action.textContent = `Run all ${named} trials in this folder`;
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
}

function selectionChanged(selection, event) {
  // Mid-drag the span is not a selection yet, so it is drawn and reported and binds nothing.
  if (event.dragging) return renderSelectionReadout(selection, event);

  if (selection.active) bindTheWindow(selection.active);
  else releaseTheWindow();
  renderSelectionReadout(selection, event);
  renderDecisions();
  runAnalysis();
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
  host.append(element('span', 'chart-selection__span', `${from} to ${to} s, ${samples.toLocaleString()} samples`));

  if (event.dragging) {
    host.append(element('span', 'chart-selection__origin', 'Release to select this window.'));
  } else if (span.stated) {
    host.append(element('span', 'chart-selection__origin', 'Selected by you, so every number over it records the window as yours.'));
  } else {
    const rules = (span.placed.placed_by || []).join(', ');
    const label = phaseLabels().get(span.placed.phase) || span.placed.phase;
    host.append(element('span', 'chart-selection__origin', `${label}, placed by ${rules}`));
  }
  if (selection.regions.length > 1) {
    host.append(element('span', 'chart-selection__origin', `${selection.regions.length} windows selected. Numbers are taken over this one.`));
  }
  updateSelectionControls(selection, event);
}

/* The row arrives with the first window and stays while a zoom it drove can still be stepped
 * back out of. A reader with nothing selected has nothing here to act on, and on a phone the
 * space it would hold is two screens of the reading order it sits above. */
function updateSelectionControls(selection, event = {}) {
  const selected = selection.regions.length > 0;
  const undoable = state.chart.canUndoZoom();
  $('chart-selection').hidden = !(selected || undoable || event.dragging || event.placedNothingHere);
  $('selection-zoom').disabled = !selected;
  $('selection-clear').disabled = !selected;
  $('selection-undo-zoom').disabled = !undoable;
  $('selection-reset-zoom').disabled = state.chart.isFit() && !undoable;
}

function wireSelectionControls() {
  const controls = $('chart-selection');
  if (controls.dataset.wired === 'true') return;
  controls.dataset.wired = 'true';
  $('selection-zoom').addEventListener('click', () => state.chart.zoomToSelection());
  $('selection-undo-zoom').addEventListener('click', () => state.chart.undoZoom());
  $('selection-reset-zoom').addEventListener('click', () => state.chart.resetZoom());
  $('selection-clear').addEventListener('click', () => state.chart.clearSelection());
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
