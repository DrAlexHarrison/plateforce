/* The trace, the landmarks drawn on it, and the gestures that move them. */

import { TraceChart } from './chart.js';
import { $, state } from './state.js';
import { element, showStage } from './format.js';
import { windowLengthParameter } from './registry.js';
import { resetSelections, candidateFor } from './startup.js';
import { renderDecisions } from './decisions.js';
import { runAnalysis } from './analysis.js';

export function enterWorkspace() {
  state.overrides = { onset: null, takeoff: null, touchdown: null };
  resetSelections();
  showStage('stage-workspace');

  const info = JSON.parse(state.loadedTrial.infoJson());
  state.info = info;
  $('trial-summary').textContent =
    `${info.sample_count.toLocaleString()} samples at ${info.sample_rate_hz} Hz, ${info.duration_seconds.toFixed(2)} s` +
    (info.synthetic ? '. Demo trial.' : '') +
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
        if (placed) state.selection.weighing = { methodId: placed.id, values: {}, unresolved: [] };
        const selection = state.selection.weighing;
        const length = windowLengthParameter(candidateFor('weighing', selection.methodId));
        if (length) {
          selection.values[length] = durationSeconds;
          selection.unresolved = (selection.unresolved || []).filter((name) => name !== length);
        }
        renderDecisions();
        runAnalysis();
      },
    });
    container.addEventListener('chart:resize', () => refreshEnvelope());
  }

  renderLegend();
  refreshEnvelope();
  renderDecisions();
  runAnalysis();
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
