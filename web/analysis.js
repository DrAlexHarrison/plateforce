/* One round trip into WebAssembly, and the numbers it hands back with their provenance. */

import { $, state } from './state.js';
import { element, formatNumber, secondaryDisplay } from './format.js';
import { rankCandidates, initialParameters, findMethod } from './registry.js';
import { candidateFor } from './startup.js';
import { unresolvedDecisions, renderDecisions } from './decisions.js';
import { renderSpreadControls, runSpread } from './spread.js';
import { openDrawer } from './drawer.js';

function boundMethodId(slotKey) {
  return (
    state.selection[slotKey]?.methodId ||
    { weighing: 'bwepoch.fixed_window', onset: 'onset.threshold.noise_relative', takeoff: 'takeoff.threshold.absolute_force' }[slotKey]
  );
}

export function buildRequest() {
  const weighingId = boundMethodId('weighing');
  return {
    weighing: {
      method_id: weighingId,
      start_index: state.weighing.startIndex,
      parameters: state.selection.weighing?.values || {},
      options: {},
    },
    onset: {
      method_id: boundMethodId('onset'),
      parameters: state.selection.onset?.values || {},
      options: {},
      manual_index: state.overrides.onset,
    },
    takeoff: {
      method_id: boundMethodId('takeoff'),
      parameters: state.selection.takeoff?.values || {},
      options: {},
      manual_index: state.overrides.takeoff,
    },
    touchdown_index: state.overrides.touchdown,
    // Sent only when the operator has stated it. A literal here would be standard gravity's
    // second home, and the engine already carries the one the registry declares.
    ...(state.gravity != null && { gravity_meters_per_second_squared: state.gravity }),
    // A method is only reported as registry backed when the registry both carries it and
    // passes its own validator.
    registry_backed_ids: state.build.registry_valid
      ? state.registry.methods.map((method) => method.id)
      : [],
  };
}

export function runAnalysis() {
  if (!state.loadedTrial) return;
  $('reset-markers').disabled = !Object.values(state.overrides).some((value) => value != null);

  const pending = unresolvedDecisions();
  if (pending.length) {
    renderPendingDecisions(pending);
    $('spread-controls-wrap').hidden = true;
    state.analysis = null;
    state.chart.setAnalysis(null);
    state.chart.schedule();
    renderDecisions();
    return;
  }

  try {
    state.analysis = JSON.parse(state.loadedTrial.analyse(JSON.stringify(buildRequest())));
  } catch (error) {
    $('metric-grid').replaceChildren();
    $('analysis-warnings').replaceChildren(notice('danger', 'The analysis could not run', String(error.message || error)));
    return;
  }

  state.chart.setAnalysis(state.analysis);
  state.chart.schedule();
  renderMetrics();
  renderSpreadControls();
  runSpread();
  renderDecisions();
}

export function notice(kind, title, body) {
  const node = element('div', `notice notice--${kind}`);
  node.append(element('strong', null, title), element('p', null, body));
  return node;
}

function renderPendingDecisions(pending) {
  const grid = $('metric-grid');
  grid.replaceChildren();
  const host = $('analysis-warnings');
  host.replaceChildren();

  const card = element('div', 'notice notice--warning');
  card.append(element('strong', null, `${pending.length} decisions have to be made before there is a number`));
  card.append(
    element(
      'p',
      null,
      'Published methods disagree here, so plateforce does not pick one for you. ' +
        'Whichever you take travels with the number.',
    ),
  );
  const list = element('ul');
  for (const item of pending) {
    list.append(element('li', null, `${item.slot.title}: ${item.what}`));
  }
  card.append(list);

  const accept = element('button', 'button button--primary', 'Take the recommended method for each');
  accept.type = 'button';
  accept.addEventListener('click', acceptRecommended);
  card.append(accept);
  host.append(card);
  $('spread-result').replaceChildren();
}

/* Resolving every forced decision at once is still an explicit act, and it is recorded as
 * one. It is not the same as never having been asked. */
export function acceptRecommended() {
  for (const slot of state.slots) {
    const selection = state.selection[slot.key];
    if (!selection.methodId && slot.available.length) {
      const candidate = rankCandidates(slot.available)[0];
      state.selection[slot.key] = { methodId: candidate.id, ...initialParameters(candidate, slot.forcesDecision) };
    }
    const candidate = candidateFor(slot.key, state.selection[slot.key].methodId);
    for (const name of state.selection[slot.key].unresolved || []) {
      const parameter = (candidate?.method?.parameter || []).find((entry) => entry.name === name);
      state.selection[slot.key].values[name] = parameter?.default ?? parameter?.published_values?.[0];
    }
    state.selection[slot.key].unresolved = [];
  }
  renderDecisions();
  runAnalysis();
}

const HEADLINE = new Set(['time_to_takeoff_seconds', 'jump_height_from_takeoff_meters']);

function renderMetrics() {
  const grid = $('metric-grid');
  grid.replaceChildren();

  for (const metric of state.analysis.metrics) {
    const card = element('div', `metric${HEADLINE.has(metric.key) ? ' metric--headline' : ''}`);
    card.append(element('span', 'metric__label', metric.label));

    const formatted = formatNumber(metric.value, metric.unit);
    if (formatted == null) {
      card.append(element('p', 'metric__value metric__value--absent', 'No value, the rule found no crossing'));
    } else {
      const value = element('p', 'metric__value', formatted);
      value.append(element('small', null, metric.unit_symbol));
      const secondary = secondaryDisplay(metric);
      if (secondary) value.append(element('small', null, `= ${secondary}`));
      card.append(value);
    }

    if (metric.note) card.append(element('p', 'metric__note', metric.note));
    card.append(provenanceRow(metric.contributing_method_ids));
    grid.append(card);
  }

  const host = $('analysis-warnings');
  host.replaceChildren();
  for (const warning of state.analysis.warnings) {
    host.append(notice('warning', 'The rule reported a problem', warning));
  }
}

export function methodTitle(id) {
  return (
    findMethod(state.registry, id)?.title ||
    state.build.bindings.find((binding) => binding.id === id)?.title ||
    id
  );
}

/* A value the request did not carry moved the number as far as one it did, so every value
 * in the fingerprint says which of the two it was. */
export function boundValueText(bound, separator = ' ') {
  const assumed = new Set(bound?.assumed_parameters || []);
  return (bound?.bound_parameters || []).map(
    ([name, value]) => `${name}${separator}${value}${assumed.has(name) ? ' (assumed)' : ''}`,
  );
}

function provenanceRow(methodIds) {
  const row = element('div', 'metric__provenance');
  const seen = new Set();
  for (const id of methodIds) {
    if (seen.has(id)) continue;
    seen.add(id);
    const bound = state.analysis.bound_methods.find((entry) => entry.method_id === id);
    const method = findMethod(state.registry, id);

    const item = element('button', 'provenance');
    item.type = 'button';
    if (!bound?.registry_backed) item.classList.add('provenance--unbacked');

    item.append(element('span', `status-dot status-dot--${method?.status || 'legacy'}`));
    item.append(element('span', 'provenance__name', methodTitle(id)));

    const badges = element('span', 'provenance__badges');
    if (method?.failure) {
      badges.append(element('span', 'tag tag--fails', `${(method.failure.rate * 100).toFixed(0)}% fail`));
    }
    if (bound?.manual_override) badges.append(element('span', 'tag tag--decide', 'dragged'));
    const binding = state.build.bindings.find((entry) => entry.id === id);
    if (!bound?.registry_backed) {
      badges.append(element('span', 'tag tag--advanced', binding?.composed_from ? 'composed' : 'unfiled'));
    }
    item.append(badges);

    const parameters = boundValueText(bound).join(', ');
    const unread = bound?.unread_parameters?.length
      ? `not taken by this rule: ${bound.unread_parameters.join(', ')}`
      : '';
    const absence = binding?.composed_from
      ? `composition of ${binding.composed_from}`
      : 'no registry row carries this id';
    item.title = [id, parameters, unread, bound?.registry_backed ? '' : absence]
      .filter(Boolean)
      .join(' | ');
    item.addEventListener('click', () => openDrawer(method, id, bound));
    row.append(item);
  }
  return row;
}
