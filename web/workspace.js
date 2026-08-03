/* The trace, the landmarks drawn on it, and the gestures that move them. */

import { TraceChart } from './chart.js';
import { $, state } from './state.js';
import { element, showStage } from './format.js';
import { windowLengthParameter } from './registry.js';
import { resetSelections, candidateFor } from './startup.js';
import { renderDecisions } from './decisions.js';
import { runAnalysis, recordStated, withSources } from './analysis.js';
import { endingOf } from './batch-run.js';
import { renderPicker } from './add-quantity.js';

export function enterWorkspace() {
  state.overrides = { onset: null, takeoff: null, touchdown: null };
  resetSelections();
  recordTheOpeningSelection();
  showStage('stage-workspace');
  offerTheRun();

  const info = JSON.parse(state.loadedTrial.infoJson());
  state.info = info;
  $('trial-summary').textContent =
    `${info.sample_count.toLocaleString()} samples at ${info.sample_rate_hz} Hz, ${info.duration_seconds.toFixed(2)} s` +
    (info.synthetic ? '. Drawn rather than recorded.' : '') +
    (info.sentinel_samples_replaced ? ` ${info.sentinel_samples_replaced} samples were flagged missing and held at the last reading.` : '');

  if (!state.chart) {
    const container = $('chart');
    state.chart = new TraceChart({
      container,
      canvas: $('chart-canvas'),
      overlay: $('chart-overlay'),
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
            methodId: placed.id, values: {}, unresolved: [],
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
    });
    container.addEventListener('chart:resize', () => refreshEnvelope());
  }

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
  }
}

export function refreshEnvelope() {
  if (!state.loadedTrial || !state.chart) return;
  state.envelope = JSON.parse(state.loadedTrial.envelopeJson(state.chart.plotWidthPx()));
  state.chart.setEnvelope(state.envelope);
  state.chart.schedule();
}

function renderLegend() {
  const entries = [
    ['var(--trace)', 'vGRF'],
    ['var(--accent)', 'System weight and the k SD band'],
    ['var(--mark-threshold)', 'The force takeoff is called at'],
    ['var(--track-onset)', 'Start of the jump'],
    ['var(--track-takeoff)', 'Takeoff'],
    ['var(--track-touchdown)', 'Touchdown'],
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
