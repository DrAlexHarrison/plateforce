/*
 * The application. Loads the WebAssembly module, holds one trial, and keeps the trace,
 * the decisions and the numbers in agreement.
 *
 * Every analysis is a round trip into WebAssembly. Nothing is computed in JavaScript,
 * because a second implementation of any quantity is the failure this project documents.
 */

import init, { buildInfoJson, registryJson, ForceFile, Session } from './pkg/plateforce_wasm.js';
import { TraceChart } from './chart.js';
import {
  SLOTS,
  buildDecisionModel,
  preferredMethod,
  initialParameters,
  availableAxes,
  WEIGHING_DURATION_AXIS,
  findMethod,
} from './registry.js';

const $ = (id) => document.getElementById(id);

const state = {
  registry: null,
  build: null,
  slots: [],
  selection: {},
  weighing: { startIndex: 0, durationSeconds: null },
  overrides: { onset: null, takeoff: null, touchdown: null },
  file: null,
  session: null,
  envelope: null,
  analysis: null,
  chart: null,
  spread: { quantity: 'time_to_takeoff_seconds', axes: new Set() },
};

/* ---------------------------------------------------------------- formatting */

const DECIMALS = { s: 4, N: 1, kg: 2, 'm/s': 3, m: 3, 'N.s': 2 };

function formatNumber(value, unit) {
  if (value == null || !Number.isFinite(value)) return null;
  return value.toFixed(DECIMALS[unit] ?? 3);
}

function secondaryDisplay(metric) {
  if (metric.unit === 'm' && metric.value != null) return `${(metric.value * 100).toFixed(1)} cm`;
  if (metric.unit === 's' && metric.value != null) return `${(metric.value * 1000).toFixed(0)} ms`;
  return null;
}

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text != null) node.textContent = text;
  return node;
}

function showStage(id) {
  for (const stage of document.querySelectorAll('.stage')) stage.hidden = stage.id !== id;
}

/* ---------------------------------------------------------------- startup */

async function start() {
  try {
    await init();
    state.build = JSON.parse(buildInfoJson());
    state.registry = JSON.parse(registryJson());
  } catch (error) {
    $('fatal-message').textContent = String(error && error.message ? error.message : error);
    showStage('stage-fatal');
    return;
  }

  state.slots = buildDecisionModel(state.registry, state.build.executable_method_ids);
  renderRegistryBanner();
  renderBuildInfo();
  resetSelections();
  wireGlobalControls();
  showStage('stage-empty');
}

function renderRegistryBanner() {
  if (state.build.registry_valid) return;
  const banner = $('registry-banner');
  banner.hidden = false;
  $('registry-banner-text').textContent =
    `${state.build.registry_violations.length} violations across ${state.build.registry_file_count} registry files. ` +
    'Until they clear, every method below is reported as not registry backed, so no citation is claimed for a number that does not have one.';
  const list = $('registry-violations');
  list.replaceChildren(...state.build.registry_violations.map((line) => element('li', null, line)));
}

function renderBuildInfo() {
  const census = state.registry.census;
  const rows = [
    ['Version', state.build.version],
    ['Registry', state.build.registry_digest],
    ['Registry status', state.build.registry_valid ? 'valid' : `${state.build.registry_violations.length} violations`],
    ['Constructs', String(census.constructs)],
    ['Computation entries', String(census.computation_entries)],
    ['Protocol entries', String(census.protocol_entries)],
    ['Executable here', `${state.build.executable_method_ids.length} of ${census.computation_entries}`],
    ['Threads', state.build.threads ? 'yes' : 'no, so no isolation headers are needed'],
  ];
  const list = $('build-info');
  list.replaceChildren();
  for (const [term, definition] of rows) {
    list.append(element('dt', null, term), element('dd', null, definition));
  }
}

function resetSelections() {
  state.selection = {};
  for (const slot of state.slots) {
    const method = preferredMethod(slot);
    state.selection[slot.key] = method
      ? { methodId: method.id, ...initialParameters(method, slot.forcesDecision) }
      : { methodId: null, values: {}, unresolved: [] };
  }
  const weighingSlot = state.slots.find((s) => s.key === 'weighing');
  state.weighing = { startIndex: 0, durationSeconds: weighingSlot?.forcesDecision ? null : 1.0 };
}

/* ---------------------------------------------------------------- file input */

function wireGlobalControls() {
  const dropzone = $('dropzone');
  const input = $('file-input');

  const open = () => input.click();
  $('choose-file').addEventListener('click', (event) => { event.stopPropagation(); open(); });
  dropzone.addEventListener('click', open);
  dropzone.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); open(); }
  });
  input.addEventListener('change', () => { if (input.files[0]) readFile(input.files[0]); });

  for (const type of ['dragenter', 'dragover']) {
    dropzone.addEventListener(type, (event) => { event.preventDefault(); dropzone.classList.add('is-over'); });
  }
  for (const type of ['dragleave', 'drop']) {
    dropzone.addEventListener(type, () => dropzone.classList.remove('is-over'));
  }
  dropzone.addEventListener('drop', (event) => {
    event.preventDefault();
    const file = event.dataTransfer?.files?.[0];
    if (file) readFile(file);
  });

  $('load-demo').addEventListener('click', (event) => { event.stopPropagation(); loadDemonstration(); });
  $('columns-cancel').addEventListener('click', () => showStage('stage-empty'));
  $('columns-confirm').addEventListener('click', confirmColumns);
  $('change-file').addEventListener('click', () => showStage('stage-empty'));
  $('reset-markers').addEventListener('click', () => {
    state.overrides = { onset: null, takeoff: null, touchdown: null };
    runAnalysis();
  });

  $('theme-toggle').addEventListener('click', () => {
    const root = document.documentElement;
    const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    const current = root.dataset.theme === 'auto' ? (dark ? 'dark' : 'light') : root.dataset.theme;
    root.dataset.theme = current === 'dark' ? 'light' : 'dark';
    state.chart?.render();
  });

  for (const node of document.querySelectorAll('[data-close-drawer]')) {
    node.addEventListener('click', () => { $('method-drawer').hidden = true; });
  }
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') $('method-drawer').hidden = true;
  });
}

async function readFile(file) {
  try {
    const text = await file.text();
    state.file?.free?.();
    state.file = ForceFile.parse(text);
    renderColumnChooser(file.name, JSON.parse(state.file.summaryJson()));
    showStage('stage-columns');
  } catch (error) {
    showStage('stage-empty');
    reportInline(`That file could not be read. ${error.message || error}`);
  }
}

function reportInline(message) {
  const zone = $('dropzone');
  let notice = zone.querySelector('.notice');
  if (!notice) {
    notice = element('div', 'notice notice--danger');
    zone.append(notice);
  }
  notice.replaceChildren(element('strong', null, 'Could not read that file'), element('p', null, message));
}

/* ---------------------------------------------------------------- column binding */

function sparkline(values) {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('class', 'column-card__spark');
  svg.setAttribute('viewBox', '0 0 100 32');
  svg.setAttribute('preserveAspectRatio', 'none');
  svg.setAttribute('aria-hidden', 'true');
  const finite = values.filter(Number.isFinite);
  const low = Math.min(...finite);
  const high = Math.max(...finite);
  const span = high - low || 1;
  const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
  path.setAttribute(
    'd',
    values
      .map((value, index) => {
        const x = (index / Math.max(1, values.length - 1)) * 100;
        const y = 30 - ((value - low) / span) * 28;
        return `${index === 0 ? 'M' : 'L'}${x.toFixed(2)} ${Number.isFinite(y) ? y.toFixed(2) : 16}`;
      })
      .join(' '),
  );
  svg.append(path);
  return svg;
}

function renderColumnChooser(fileName, summary) {
  state.columnSummary = summary;
  state.chosenColumn = summary.suggested_force_column ?? 0;

  $('columns-lead').textContent =
    `${fileName}: ${summary.row_count.toLocaleString()} rows, ${summary.column_count} columns, ${summary.delimiter} separated` +
    (summary.skipped_leading_lines ? `, ${summary.skipped_leading_lines} header lines skipped` : '') +
    (summary.ragged_rows_dropped ? `, ${summary.ragged_rows_dropped} ragged rows dropped` : '') +
    `. ${summary.suggested_force_column_reason}.`;

  const grid = $('column-grid');
  grid.replaceChildren();
  summary.columns.forEach((column) => {
    const card = element('button', 'column-card');
    card.type = 'button';
    card.setAttribute('role', 'radio');
    card.setAttribute('aria-checked', String(column.index === state.chosenColumn));

    const name = element('span', 'column-card__name');
    name.append(element('span', null, column.header || `Column ${column.index + 1}`));
    if (column.implied_sample_rate_hz) name.append(element('span', 'tag tag--advanced', 'time'));
    card.append(name);
    card.append(sparkline(column.sparkline));
    card.append(
      element(
        'span',
        'column-card__stats',
        `${column.minimum.toFixed(1)} to ${column.maximum.toFixed(1)}, SD ${column.standard_deviation.toFixed(1)}` +
          (column.exact_zero_count ? `, ${column.exact_zero_count} exact zeros` : ''),
      ),
    );
    card.addEventListener('click', () => {
      state.chosenColumn = column.index;
      for (const node of grid.children) node.setAttribute('aria-checked', 'false');
      card.setAttribute('aria-checked', 'true');
    });
    grid.append(card);
  });

  const rate = $('sample-rate');
  rate.value = summary.suggested_sample_rate_hz ? summary.suggested_sample_rate_hz.toFixed(4).replace(/\.?0+$/, '') : '1200';
  $('sample-rate-hint').textContent = summary.sample_rate_source + '. A wrong rate scales every time and every impulse.';
}

function confirmColumns() {
  const rate = Number($('sample-rate').value);
  if (!(rate > 0)) return;
  try {
    state.session?.free?.();
    state.session = Session.fromForceFile(state.file, state.chosenColumn, rate, $('sentinel').value);
    enterWorkspace();
  } catch (error) {
    reportInline(String(error.message || error));
    showStage('stage-empty');
  }
}

function loadDemonstration() {
  state.session?.free?.();
  state.session = Session.demonstration();
  enterWorkspace();
}

/* ---------------------------------------------------------------- workspace */

function enterWorkspace() {
  state.overrides = { onset: null, takeoff: null, touchdown: null };
  resetSelections();
  showStage('stage-workspace');

  const info = JSON.parse(state.session.infoJson());
  state.info = info;
  $('trial-summary').textContent =
    `${info.sample_count.toLocaleString()} samples at ${info.sample_rate_hz} Hz, ${info.duration_seconds.toFixed(2)} s` +
    (info.synthetic ? '. Synthetic trial, generated in this tab.' : '') +
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
        state.weighing = { startIndex, durationSeconds };
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

function refreshEnvelope() {
  if (!state.session || !state.chart) return;
  state.envelope = JSON.parse(state.session.envelopeJson(state.chart.plotWidthPx()));
  state.chart.setEnvelope(state.envelope);
  state.chart.schedule();
}

function renderLegend() {
  const entries = [
    ['var(--trace)', 'vGRF'],
    ['var(--accent)', 'System weight and the k SD band'],
    ['var(--danger)', 'Takeoff threshold'],
    ['var(--track-onset)', 'Onset'],
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

/* ---------------------------------------------------------------- decisions */

function unresolvedDecisions() {
  const pending = [];
  for (const slot of state.slots) {
    const selection = state.selection[slot.key];
    if (slot.forcesDecision && !selection.methodId && slot.available.length) {
      pending.push({ slot, what: 'method' });
    }
    for (const name of selection.unresolved || []) pending.push({ slot, what: name });
  }
  if (state.weighing.durationSeconds == null) {
    pending.push({ slot: state.slots.find((s) => s.key === 'weighing'), what: 'duration' });
  }
  return pending;
}

function renderDecisions() {
  const host = $('decision-list');
  host.replaceChildren();

  const pending = unresolvedDecisions();
  $('decisions-sub').textContent = pending.length
    ? `${pending.length} unresolved. The registry marks these as choices that move the number, so nothing is chosen for you.`
    : 'All resolved. Every choice below appears in the provenance of the numbers.';

  for (const slot of state.slots) {
    host.append(renderSlot(slot));
  }
}

function renderSlot(slot) {
  const selection = state.selection[slot.key];
  const wrap = element('div', 'decision');

  const head = element('div', 'decision__head');
  head.append(element('span', 'decision__title', slot.title));
  if (slot.forcesDecision) head.append(element('span', 'tag tag--decide', 'decide'));
  else if (slot.surfacing === 'default_and_hide') head.append(element('span', 'tag tag--advanced', 'advanced'));
  wrap.append(head);
  wrap.append(element('p', 'decision__why', slot.why));

  if (!slot.available.length) {
    const note = element(
      'p',
      'decision__meta',
      slot.offered.length
        ? `The registry carries ${slot.offered.length} entries for this construct and this build can execute none of them, so a build default is used and marked as not registry backed.`
        : 'The registry carries no entry for this construct yet, so a build default is used and marked as not registry backed.',
    );
    wrap.append(note);
    wrap.append(renderWithheld(slot));
    return wrap;
  }

  const select = document.createElement('select');
  select.setAttribute('aria-label', `${slot.title} method`);
  if (!selection.methodId) {
    const placeholder = element('option', null, 'Choose a method');
    placeholder.value = '';
    select.append(placeholder);
  }
  for (const method of slot.available) {
    const option = element('option', null, `${method.title} (${method.status})`);
    option.value = method.id;
    option.selected = method.id === selection.methodId;
    select.append(option);
  }
  for (const method of slot.unavailable) {
    const option = element('option', null, `${method.title}, not executable in this build`);
    option.value = method.id;
    option.disabled = true;
    select.append(option);
  }
  select.addEventListener('change', () => {
    const method = findMethod(state.registry, select.value);
    state.selection[slot.key] = { methodId: method.id, ...initialParameters(method, slot.forcesDecision) };
    renderDecisions();
    runAnalysis();
  });
  wrap.append(select);

  if (!selection.methodId) {
    wrap.append(element('p', 'undecided', 'Unresolved. The registry flags this as force a decision, so there is no default.'));
  }

  const method = selection.methodId ? findMethod(state.registry, selection.methodId) : null;
  if (method) {
    if (method.failure) {
      const badge = element(
        'p',
        'undecided',
        `Fails on ${(method.failure.rate * 100).toFixed(1)}% of trials (${method.failure.numerator} of ${method.failure.denominator}, ${method.failure.corpus}), detectability ${method.failure.detectability}: ${method.failure.definition}.`,
      );
      wrap.append(badge);
    }
    wrap.append(renderParameters(slot, method, selection));

    const inspect = element('button', 'chip', 'Rule, citations and bias');
    inspect.type = 'button';
    inspect.addEventListener('click', () => openDrawer(method));
    const row = element('div', 'metric__provenance');
    row.append(inspect);
    wrap.append(row);
  }

  if (slot.key === 'weighing') wrap.append(renderWeighingDuration(slot));
  wrap.append(renderWithheld(slot));
  return wrap;
}

function renderParameters(slot, method, selection) {
  const host = element('div', 'decision__params');
  for (const parameter of method.parameter || []) {
    const values = parameter.published_values || [];
    const row = element('div', 'param');
    const id = `param-${slot.key}-${parameter.name}`;
    const label = element('label', null, `${parameter.name}${parameter.unit ? ` (${parameter.unit})` : ''}`);
    label.htmlFor = id;
    row.append(label);

    if (values.length > 1) {
      const select = document.createElement('select');
      select.id = id;
      const unresolved = (selection.unresolved || []).includes(parameter.name);
      if (unresolved) {
        const placeholder = element('option', null, `choose from ${values.length}`);
        placeholder.value = '';
        select.append(placeholder);
      }
      for (const value of values) {
        const option = element('option', null, `${value}${value === parameter.default ? ` (default, ${parameter.default_source || 'unsourced'})` : ''}`);
        option.value = String(value);
        option.selected = !unresolved && selection.values[parameter.name] === value;
        select.append(option);
      }
      select.addEventListener('change', () => {
        selection.values[parameter.name] = Number(select.value);
        selection.unresolved = (selection.unresolved || []).filter((name) => name !== parameter.name);
        renderDecisions();
        runAnalysis();
      });
      row.append(select);
    } else {
      const input = document.createElement('input');
      input.type = 'number';
      input.id = id;
      input.step = 'any';
      input.value = String(selection.values[parameter.name] ?? parameter.default ?? '');
      input.addEventListener('change', () => {
        selection.values[parameter.name] = Number(input.value);
        runAnalysis();
      });
      row.append(input);
    }
    host.append(row);
  }
  return host;
}

function renderWeighingDuration(slot) {
  const row = element('div', 'param');
  const label = element('label', null, 'duration (seconds)');
  label.htmlFor = 'weighing-duration';
  const select = document.createElement('select');
  select.id = 'weighing-duration';
  if (state.weighing.durationSeconds == null) {
    const placeholder = element('option', null, 'choose from 4');
    placeholder.value = '';
    select.append(placeholder);
  }
  for (const value of WEIGHING_DURATION_AXIS.values) {
    const option = element('option', null, `${value.toFixed(1)} s`);
    option.value = String(value);
    option.selected = state.weighing.durationSeconds === value;
    select.append(option);
  }
  const custom = state.weighing.durationSeconds;
  if (custom != null && !WEIGHING_DURATION_AXIS.values.includes(custom)) {
    const option = element('option', null, `${custom.toFixed(3)} s (dragged)`);
    option.value = String(custom);
    option.selected = true;
    select.append(option);
  }
  select.addEventListener('change', () => {
    state.weighing = { ...state.weighing, durationSeconds: Number(select.value) };
    renderDecisions();
    runAnalysis();
  });
  row.append(label, select);
  return row;
}

function renderWithheld(slot) {
  if (!slot.withheld.length) return document.createDocumentFragment();
  const details = element('details', 'not-offered');
  details.append(element('summary', null, `${slot.withheld.length} entries the registry does not offer as a choice`));
  const list = element('ul');
  for (const method of slot.withheld) {
    const item = element('li');
    item.append(element('code', null, method.id));
    item.append(
      document.createTextNode(
        ` ${method.gui.surfacing === 'refuse' ? 'refused' : 'never a user choice'}. ${method.gui.rationale || method.title}`,
      ),
    );
    list.append(item);
  }
  details.append(list);
  return details;
}

/* ---------------------------------------------------------------- analysis */

function boundMethodId(slotKey) {
  const selection = state.selection[slotKey];
  if (selection.methodId) return selection.methodId;
  return { weighing: 'bwepoch.fixed_window', onset: 'onset.threshold.noise_relative', takeoff: 'takeoff.threshold.absolute' }[slotKey];
}

function buildRequest() {
  return {
    weighing: {
      method_id: boundMethodId('weighing'),
      start_index: state.weighing.startIndex,
      duration_seconds: state.weighing.durationSeconds ?? 1.0,
    },
    onset: {
      method_id: boundMethodId('onset'),
      parameters: state.selection.onset?.values || {},
      manual_index: state.overrides.onset,
    },
    takeoff: {
      method_id: boundMethodId('takeoff'),
      parameters: state.selection.takeoff?.values || {},
      manual_index: state.overrides.takeoff,
    },
    touchdown_index: state.overrides.touchdown,
    registry_backed_ids: state.build.registry_valid
      ? state.registry.methods.map((method) => method.id)
      : [],
  };
}

function runAnalysis() {
  if (!state.session) return;
  $('reset-markers').disabled = !Object.values(state.overrides).some((value) => value != null);

  const pending = unresolvedDecisions();
  if (pending.length) {
    renderPendingDecisions(pending);
    state.analysis = null;
    state.chart.setAnalysis(null);
    state.chart.schedule();
    renderDecisions();
    return;
  }

  try {
    state.analysis = JSON.parse(state.session.analyze(JSON.stringify(buildRequest())));
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

function notice(kind, title, body) {
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
      'The registry marks each of these as a choice that moves the result, so plateforce will not pick one for you. ' +
        'Choosing the registry recommendation is fine, and it is recorded as a choice you made.',
    ),
  );
  const list = element('ul');
  for (const item of pending) {
    list.append(element('li', null, `${item.slot.title}: ${item.what}`));
  }
  card.append(list);

  const accept = element('button', 'button button--primary', 'Accept the registry recommendation for all of these');
  accept.type = 'button';
  accept.addEventListener('click', acceptRecommended);
  card.append(accept);
  host.append(card);
  $('spread-result').replaceChildren();
}

/* Resolving every forced decision at once is still an explicit act, and it is recorded as
 * one. It is not the same as never having been asked. */
function acceptRecommended() {
  for (const slot of state.slots) {
    const selection = state.selection[slot.key];
    if (!selection.methodId && slot.available.length) {
      const byStatus = ['recommended', 'accepted', 'contested', 'legacy', 'deprecated'];
      const method = [...slot.available].sort((a, b) => byStatus.indexOf(a.status) - byStatus.indexOf(b.status))[0];
      state.selection[slot.key] = { methodId: method.id, values: {}, unresolved: [] };
    }
    const method = findMethod(state.registry, state.selection[slot.key].methodId);
    for (const name of state.selection[slot.key].unresolved || []) {
      const parameter = (method?.parameter || []).find((p) => p.name === name);
      state.selection[slot.key].values[name] = parameter?.default ?? parameter?.published_values?.[0];
    }
    state.selection[slot.key].unresolved = [];
  }
  if (state.weighing.durationSeconds == null) state.weighing = { ...state.weighing, durationSeconds: 1.0 };
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
      value.append(element('small', null, metric.unit));
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

function provenanceRow(methodIds) {
  const row = element('div', 'metric__provenance');
  const seen = new Set();
  for (const id of methodIds) {
    if (seen.has(id)) continue;
    seen.add(id);
    const bound = state.analysis.bound_methods.find((method) => method.method_id === id);
    const method = findMethod(state.registry, id);

    const chip = element('button', 'chip');
    chip.type = 'button';
    if (!bound?.registry_backed) chip.classList.add('chip--unbacked');
    if (method?.failure) chip.classList.add('chip--failing');

    if (method) {
      const dot = element('span', `status-dot status-dot--${method.status}`);
      chip.append(dot);
    }
    chip.append(document.createTextNode(method ? method.title : id));
    if (method?.failure) chip.append(element('span', 'tag tag--fails', `${(method.failure.rate * 100).toFixed(0)}% fail`));
    if (!bound?.registry_backed) chip.append(element('span', 'tag tag--advanced', 'no registry entry'));

    const parameters = (bound?.bound_parameters || []).map(([name, value]) => `${name} ${value}`).join(', ');
    chip.title = parameters ? `${id}: ${parameters}` : id;
    chip.addEventListener('click', () => openDrawer(method, id, bound));
    row.append(chip);

    if (bound?.manual_override) {
      const override = element('span', 'chip chip--override', 'dragged by hand');
      row.append(override);
    }
  }
  return row;
}

/* ---------------------------------------------------------------- drawer */

function openDrawer(method, fallbackId, bound) {
  const drawer = $('method-drawer');
  const body = $('drawer-body');
  body.replaceChildren();
  $('drawer-title').textContent = method ? method.title : fallbackId;

  if (!method) {
    body.append(
      notice(
        'warning',
        'Not a registry entry',
        `${fallbackId} is a build default. The registry carries no entry for it, so there is no citation, no recorded bias and no failure rate. Numbers depending on it are marked accordingly.`,
      ),
    );
    drawer.hidden = false;
    return;
  }

  const section = (heading, node) => {
    const wrap = element('section');
    wrap.append(element('h3', null, heading));
    wrap.append(node);
    return wrap;
  };

  const identity = element('dl', 'kv');
  const rows = [
    ['Registry id', method.id],
    ['Construct', method.construct],
    ['Status', method.status],
    ['Confidence', method.confidence],
    ['Debate', method.debate || 'not stated'],
    ['Surfacing', method.gui?.surfacing || 'not stated'],
    ['Sensitivity', method.gui?.sensitivity || 'not stated'],
  ];
  if (bound?.bound_parameters?.length) {
    rows.push(['Bound here', bound.bound_parameters.map(([name, value]) => `${name} = ${value}`).join(', ')]);
  }
  for (const [term, definition] of rows) identity.append(element('dt', null, term), element('dd', null, definition));
  body.append(section('Entry', identity));

  body.append(section('Rule', element('p', 'rule-text', method.rule.trim())));

  if (method.gui?.rationale) body.append(section('Why it is surfaced this way', element('p', null, method.gui.rationale.trim())));

  if (method.failure) {
    body.append(
      section(
        'Failure rate',
        notice(
          'danger',
          `${(method.failure.rate * 100).toFixed(1)} percent, ${method.failure.numerator} of ${method.failure.denominator}`,
          `${method.failure.definition}. Corpus ${method.failure.corpus}. Detectability ${method.failure.detectability}, so a bias figure for this rule averages working with not working.`,
        ),
      ),
    );
  }

  if (method.parameter?.length) {
    const list = element('ul');
    for (const parameter of method.parameter) {
      const item = element('li');
      item.append(element('strong', null, parameter.name));
      const published = parameter.published_values?.length ? ` Published values: ${parameter.published_values.join(', ')}.` : '';
      const chosen = parameter.default != null ? ` Default ${parameter.default} from ${parameter.default_source || 'an unnamed source'}.` : '';
      item.append(document.createTextNode(`${parameter.unit ? ` (${parameter.unit})` : ''}.${published}${chosen}`));
      if (parameter.notes) item.append(element('p', 'metric__note', parameter.notes.trim()));
      list.append(item);
    }
    body.append(section('Parameters', list));
  }

  if (method.citation?.length) {
    const list = element('ul');
    for (const citation of method.citation) {
      const item = element('li');
      item.append(element('strong', null, `${citation.role}: `));
      item.append(document.createTextNode(citation.reference));
      if (citation.doi) {
        const link = element('a', null, ` doi:${citation.doi}`);
        link.href = `https://doi.org/${citation.doi}`;
        link.rel = 'noreferrer';
        link.target = '_blank';
        item.append(link);
      }
      if (!citation.obtained) item.append(element('span', 'tag tag--decide', 'source not obtained'));
      list.append(item);
    }
    body.append(section('Citations', list));
  }

  if (method.bias?.length) {
    const list = element('ul');
    for (const bias of method.bias) {
      list.append(
        element(
          'li',
          null,
          `${bias.magnitude} ${bias.unit}${bias.direction ? ` (${bias.direction})` : ''} against ${bias.criterion} (${bias.criterion_kind})` +
            `${bias.source ? `, reported by ${bias.source}` : ''}${bias.conditional_on_success ? '. Conditional on the rule having worked.' : ''}`,
        ),
      );
    }
    body.append(section('Known bias', list));
  }

  if (method.disagrees_with?.length) {
    const list = element('ul');
    for (const other of method.disagrees_with) {
      list.append(element('li', null, `${other.id} (${other.kind})${other.note ? `. ${other.note}` : ''}`));
    }
    body.append(section('Disagrees with', list));
  }

  drawer.hidden = false;
}

/* ---------------------------------------------------------------- spread */

function currentAxes() {
  const axes = [];
  for (const slot of state.slots) {
    const method = findMethod(state.registry, state.selection[slot.key]?.methodId);
    if (method) axes.push(...availableAxes(slot, method));
  }
  axes.push(WEIGHING_DURATION_AXIS);
  return axes;
}

function renderSpreadControls() {
  const quantitySelect = $('spread-quantity');
  const previous = state.spread.quantity;
  quantitySelect.replaceChildren();
  for (const metric of state.analysis.metrics) {
    const option = element('option', null, metric.label);
    option.value = metric.key;
    option.selected = metric.key === previous;
    quantitySelect.append(option);
  }
  if (!quantitySelect.value) quantitySelect.value = state.analysis.metrics[0]?.key || '';
  state.spread.quantity = quantitySelect.value;
  quantitySelect.onchange = () => {
    state.spread.quantity = quantitySelect.value;
    runSpread();
  };

  const axes = currentAxes();
  if (!state.spread.initialised) {
    for (const axis of axes) {
      if (axis.slot === 'onset') state.spread.axes.add(axis.id);
    }
    if (!state.spread.axes.size && axes.length) state.spread.axes.add(axes[0].id);
    state.spread.initialised = true;
  }

  const host = $('spread-axis-list');
  host.replaceChildren();
  for (const axis of axes) {
    const label = element('label');
    const box = document.createElement('input');
    box.type = 'checkbox';
    box.checked = state.spread.axes.has(axis.id);
    box.addEventListener('change', () => {
      if (box.checked) state.spread.axes.add(axis.id);
      else state.spread.axes.delete(axis.id);
      runSpread();
    });
    label.append(box);
    label.append(document.createTextNode(`${axis.label}: ${axis.values.join(', ')} ${axis.unit}`.trim()));
    host.append(label);
  }
}

function runSpread() {
  const host = $('spread-result');
  const axes = currentAxes().filter((axis) => state.spread.axes.has(axis.id));
  if (!axes.length) {
    host.replaceChildren(
      notice('warning', 'Nothing selected to vary', 'Tick at least one parameter above to see how far the number moves across its published values.'),
    );
    return;
  }

  let result;
  try {
    result = JSON.parse(
      state.session.spread(
        JSON.stringify({
          base: buildRequest(),
          axes: axes.map((axis) => ({ slot: axis.slot, parameter: axis.parameter, values: axis.values })),
          quantity_key: state.spread.quantity,
          maximum_combinations: 512,
        }),
      ),
    );
  } catch (error) {
    host.replaceChildren(notice('danger', 'The sweep could not run', String(error.message || error)));
    return;
  }

  host.replaceChildren();
  const label = state.analysis.metrics.find((m) => m.key === result.quantity_key)?.label || result.quantity_key;

  if (result.succeeded === 0) {
    host.append(notice('danger', 'Every alternative failed on this trial', `${result.failed} of ${result.combinations_run} combinations found no crossing.`));
    return;
  }

  const headline = element('div', 'spread-headline');
  const percent = result.spread_percent_of_median;
  headline.append(element('span', 'spread-headline__figure', percent == null ? '--' : `${percent.toFixed(1)}%`));
  headline.append(
    element(
      'p',
      'spread-headline__text',
      `${label} spans ${formatNumber(result.spread_absolute, result.unit)} ${result.unit} across ` +
        `${result.succeeded} defensible alternatives on this one trial, which is ${percent == null ? 'an undefined fraction' : `${percent.toFixed(1)} percent`} of its own median. ` +
        `Every one of those settings appears in the published literature.` +
        (result.capped ? ` Showing ${result.combinations_run} of ${result.combinations_requested} combinations.` : '') +
        (result.failed ? ` ${result.failed} combinations produced no value and are listed below.` : ''),
    ),
  );
  host.append(headline);
  host.append(spreadAxisPlot(result));
  host.append(spreadTable(result, label));
}

function spreadAxisPlot(result) {
  const wrap = element('div', 'spread-axis');
  const low = result.minimum;
  const high = result.maximum;
  const span = high - low || 1;
  const position = (value) => `${(((value - low) / span) * 96 + 2).toFixed(2)}%`;

  wrap.append(element('div', 'spread-axis__line'));
  const bar = element('div', 'spread-axis__span');
  bar.style.left = position(low);
  bar.style.width = `${(((high - low) / span) * 96).toFixed(2)}%`;
  wrap.append(bar);

  for (const variant of result.variants) {
    if (variant.value == null) continue;
    const tick = element('div', 'spread-tick');
    tick.style.left = position(variant.value);
    tick.title = `${variant.label}: ${formatNumber(variant.value, result.unit)} ${result.unit}`;
    wrap.append(tick);
  }
  if (result.baseline_value != null) {
    const tick = element('div', 'spread-tick spread-tick--baseline');
    tick.style.left = position(result.baseline_value);
    tick.title = `your current setting: ${formatNumber(result.baseline_value, result.unit)} ${result.unit}`;
    wrap.append(tick);
  }

  const lowLabel = element('span', 'spread-bound', `${formatNumber(low, result.unit)} ${result.unit}`);
  lowLabel.style.left = '2%';
  const highLabel = element('span', 'spread-bound spread-bound--max', `${formatNumber(high, result.unit)} ${result.unit}`);
  highLabel.style.left = '98%';
  wrap.append(lowLabel, highLabel);
  return wrap;
}

function spreadTable(result, label) {
  const scroll = element('div', 'table-scroll');
  const table = element('table', 'data');
  const head = element('thead');
  const headRow = element('tr');
  for (const heading of ['Settings', label, 'Difference from your setting']) {
    headRow.append(element('th', null, heading));
  }
  head.append(headRow);
  table.append(head);

  const body = element('tbody');
  const sorted = [...result.variants].sort((a, b) => {
    if (a.value == null) return 1;
    if (b.value == null) return -1;
    return a.value - b.value;
  });

  for (const variant of sorted) {
    const row = element('tr');
    if (variant.value === result.minimum) row.dataset.extreme = 'low';
    if (variant.value === result.maximum) row.dataset.extreme = 'high';
    row.append(element('td', null, variant.label));
    if (variant.value == null) {
      const cell = element('td', 'failed', variant.failure_reason || 'no value');
      cell.colSpan = 2;
      row.append(cell);
    } else {
      row.append(element('td', 'numeric', `${formatNumber(variant.value, result.unit)} ${result.unit}`));
      const delta = result.baseline_value == null ? null : variant.value - result.baseline_value;
      row.append(element('td', 'numeric', delta == null ? '--' : `${delta >= 0 ? '+' : ''}${formatNumber(delta, result.unit)}`));
    }
    body.append(row);
  }
  table.append(body);
  scroll.append(table);
  return scroll;
}

start();
