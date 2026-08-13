/* How much the method choice moves this number, over every defensible alternative the
 * registry publishes. */

import { $, state } from './state.js';
import { counted, element, formatNumber, reply, typesetUnit } from './format.js';
import { availableAxes, constructLabel, GRAVITY_AXIS } from './registry.js';
import { candidateFor } from './startup.js';
import { notice, buildRequest, methodTitle } from './analysis.js';

/* A slot is identified by the construct it stands for, on every surface and in the request
 * the engine reads. The interface key a slot happens to carry is not that identity, so an
 * axis records the construct it varies and nothing here matches on a slot's key. */
function currentAxes() {
  const axes = [];
  for (const slot of state.slots) {
    const candidate = candidateFor(slot.key, state.selection[slot.key]?.methodId);
    for (const axis of availableAxes(slot, candidate)) axes.push({ ...axis, construct: slot.construct });
  }
  axes.push(GRAVITY_AXIS);
  return axes;
}

const variesTheRule = (axis) => Boolean(axis.methodIds?.length);

/* The opening population is every rule-varying construct on the quantity's path. */
export function openingAxes(axes) {
  return axes.filter(variesTheRule);
}

function openingSummary(axes) {
  if (!axes.length) return 'Choose at least one setting.';
  const names = axes.map((axis) => axis.label.split(':')[0]);
  return `Varying: ${names.join(', ')}.`;
}

export function renderSpreadControls() {
  $('spread-controls-wrap').hidden = false;
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
  const opening = openingAxes(axes);
  if (!state.spread.initialised) {
    for (const axis of opening) state.spread.axes.add(axis.id);
    state.spread.opened = opening.map((axis) => axis.id);
    state.spread.initialised = true;
  }

  // The opening setting is the one choice on this panel nobody made, so it is named until
  // the reader changes it, after which the tick boxes say what is varying.
  const untouched =
    state.spread.opened?.length === state.spread.axes.size &&
    state.spread.opened.every((id) => state.spread.axes.has(id));
  $('spread-opening').textContent = untouched ? openingSummary(opening) : '';

  const host = $('spread-axis-list');
  host.replaceChildren();
  for (const axis of axes) {
    const label = element('label');
    // The construct is the axis's identity. The text beside it is the label a reader
    // sees, and the two are allowed to be edited independently.
    if (axis.construct) label.dataset.construct = axis.construct;
    const box = document.createElement('input');
    box.type = 'checkbox';
    box.checked = state.spread.axes.has(axis.id);
    box.addEventListener('change', () => {
      if (box.checked) state.spread.axes.add(axis.id);
      else state.spread.axes.delete(axis.id);
      runSpread();
    });
    label.append(box);
    label.append(document.createTextNode(`${axis.label}: ${axis.display} ${axis.unit}`.trim()));
    host.append(label);
  }
}

/*
 * The panel answers how much the method choice moves this number, which is a question read
 * once the marker has come to rest. Recomputing every published alternative on each frame
 * of a drag is work nobody asked for, and it is what puts the drag itself over its budget.
 *
 * Waiting for the trace to settle rather than for a pointer release covers the arrow keys
 * too, which produce the same burst and no release event.
 */
const SETTLE_MILLISECONDS = 120;
let settling = null;

export function scheduleSpread() {
  const host = $('spread-result');
  if (host.firstChild) showPreviousPositionAsPrevious();
  else host.replaceChildren(element('p', 'panel__sub spread-pending', 'Calculating spread…'));
  clearTimeout(settling);
  settling = setTimeout(() => {
    settling = null;
    runSpread();
  }, SETTLE_MILLISECONDS);
}

/* The figures on screen were computed for a position the reader has moved away from. A
 * number that is no longer about what the reader is looking at, still drawn as though it
 * is, is a confident wrong number, so it says which position it is for until it catches up. */
function showPreviousPositionAsPrevious() {
  const host = $('spread-result');
  if (!host.firstChild || host.dataset.forPreviousPosition === 'true') return;
  host.dataset.forPreviousPosition = 'true';
  host.prepend(
    notice(
      'warning',
      'Updating spread',
      'Values below use the previous marker position.',
    ),
  );
}

export function runSpread() {
  const host = $('spread-result');
  delete host.dataset.forPreviousPosition;
  const axes = currentAxes().filter((axis) => state.spread.axes.has(axis.id));
  if (!axes.length) {
    host.replaceChildren(
      notice('warning', 'Nothing selected', 'Choose at least one setting to vary.'),
    );
    return;
  }

  const answer = reply(
    state.loadedTrial.spread(
      JSON.stringify({
        base: buildRequest(),
        axes: axes.map((axis) => ({
          slot: axis.slot,
          parameter: axis.parameter ?? null,
          values: axis.values || [],
          method_ids: axis.methodIds || [],
        })),
        quantity_key: state.spread.quantity,
        maximum_combinations: 512,
      }),
    ),
  );
  if (answer.refusal) {
    state.spread.result = null;
    host.replaceChildren(notice('danger', 'Spread unavailable', answer.refusal.message));
    return;
  }
  const result = answer.ok;
  state.spread.result = result;

  host.replaceChildren();
  const label = state.analysis.metrics.find((m) => m.key === result.quantity_key)?.label || result.quantity_key;

  if (result.succeeded === 0) {
    host.append(notice('danger', 'No combination produced a value', `${result.failed} of ${result.combinations_run} failed.`));
    return;
  }

  const headline = element('div', 'spread-headline');
  const percent = result.spread_percent_of_median;
  headline.append(element('span', 'spread-headline__figure', percent == null ? '--' : `${percent.toFixed(1)}%`));
  // A sweep that produced one number carries no spread, so the sentence is the count alone.
  // Written through the figure unconditionally it read "jump height: null m across 1 of 1".
  const moved = formatNumber(result.spread_absolute, result.unit);
  const countLine =
    `${result.succeeded} of ${counted(result.combinations_run, 'combination')}.` +
    (result.capped ? ` Capped from ${result.combinations_requested}.` : '') +
    (result.failed ? ` ${result.failed} failed.` : '');
  headline.append(
    element(
      'p',
      'spread-headline__text',
      moved == null ? `${label}: ${countLine}` : `${label}: ${moved} ${typesetUnit(result.unit_symbol)} across ${countLine}`,
    ),
  );
  host.append(headline);
  host.append(whatMoved(result));
  host.append(spreadAxisPlot(result));
  host.append(spreadTables(result, label));
}

/* Which choices this figure is a spread over, and which stood still.
 *
 * A spread is a number over a set, and a reader cannot judge it without the set. The panel
 * reported a count of combinations and no account of what was combined, so a figure taken
 * while the rule that computes the quantity stood still read exactly like a figure taken over
 * everything. Both halves are shown, because a run that did vary it has to say so too. */
function whatMoved(result) {
  const varied = (result.axes_varied ?? []).filter((axis) => axis.rules_varied > 1);
  const held = result.held_fixed ?? [];
  if (varied.length === 0 && held.length === 0) return element('div');

  const wrap = element('div', 'spread-scope');
  if (varied.length > 0) {
    const names = varied.map((axis) => `${constructLabel(axis.construct)} (${axis.rules_varied} rules)`).join(', ');
    wrap.append(element('p', 'panel__sub', `Varied: ${names}.`));
  }
  if (held.length > 0) {
    const names = held.map((rule) => `${constructLabel(rule.construct)} at ${rule.method_id}`).join('; ');
    wrap.append(element('p', 'panel__sub', `Fixed: ${names}.`));
  }
  return wrap;
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
    tick.title = `${readableLabel(variant)}: ${formatNumber(variant.value, result.unit)} ${typesetUnit(result.unit_symbol)}`;
    wrap.append(tick);
  }
  if (result.baseline_value != null) {
    const tick = element('div', 'spread-tick spread-tick--baseline');
    tick.style.left = position(result.baseline_value);
    tick.title = `Current setting: ${formatNumber(result.baseline_value, result.unit)} ${typesetUnit(result.unit_symbol)}`;
    wrap.append(tick);
  }

  const lowLabel = element('span', 'spread-bound', `${formatNumber(low, result.unit)} ${typesetUnit(result.unit_symbol)}`);
  lowLabel.style.left = '2%';
  const highLabel = element('span', 'spread-bound spread-bound--max', `${formatNumber(high, result.unit)} ${typesetUnit(result.unit_symbol)}`);
  highLabel.style.left = '98%';
  wrap.append(lowLabel, highLabel);
  return wrap;
}

/* A variant's settings come back as raw ids. The table reads them back as the titles the
 * picker used, so the same rule is not called two different things in one screen. */
function readableLabel(variant) {
  if (!variant.settings.length) return 'your current setting';
  return variant.settings
    .map(([name, value]) => (value.includes('.') ? methodTitle(value) : `${name} ${value}`))
    .join(', ');
}

function spreadTables(result, label) {
  if (result.variants.length <= 12) return spreadTable(result, label, result.variants);

  const valued = result.variants.filter((variant) => variant.value != null);
  const low = valued.reduce((found, variant) => !found || variant.value < found.value ? variant : found, null);
  const high = valued.reduce((found, variant) => !found || variant.value > found.value ? variant : found, null);
  const endpoints = [...new Set([low, high].filter(Boolean))];

  const wrap = element('div', 'spread-tables');
  wrap.append(element('p', 'panel__sub spread-table-count', `${endpoints.length} shown of ${result.variants.length} combinations.`));
  wrap.append(spreadTable(result, label, endpoints, 'spread-summary'));

  const all = element('details', 'spread-all');
  all.append(element('summary', null, `Show all ${result.variants.length} combinations`));
  all.append(spreadTable(result, label, result.variants));
  wrap.append(all);
  return wrap;
}

function spreadTable(result, label, variants, className = '') {
  const scroll = element('div', 'table-scroll');
  if (className) scroll.classList.add(className);
  const table = element('table', 'data');
  const head = element('thead');
  const headRow = element('tr');
  for (const heading of ['Settings', label, 'Difference from your setting']) {
    headRow.append(element('th', null, heading));
  }
  head.append(headRow);
  table.append(head);

  const body = element('tbody');
  const sorted = [...variants].sort((a, b) => {
    if (a.value == null) return 1;
    if (b.value == null) return -1;
    return a.value - b.value;
  });

  for (const variant of sorted) {
    const row = element('tr');
    if (variant.value === result.minimum) row.dataset.extreme = 'low';
    if (variant.value === result.maximum) row.dataset.extreme = 'high';
    // The rules behind the number are the reason the row is here, and the column they sit in
    // is narrow enough to clip a set of three, so the cell carries them whole as well.
    const settings = element('td', null, readableLabel(variant));
    settings.title = readableLabel(variant);
    row.append(settings);
    if (variant.value == null) {
      const cell = element('td', 'failed', variant.failure_reason?.message || 'no value');
      cell.colSpan = 2;
      row.append(cell);
    } else {
      row.append(element('td', 'numeric', `${formatNumber(variant.value, result.unit)} ${typesetUnit(result.unit_symbol)}`));
      const delta = result.baseline_value == null ? null : variant.value - result.baseline_value;
      // The difference is a quantity in the same unit as the value beside it, and it read as a
      // bare number under a heading that names no unit.
      row.append(element('td', 'numeric', delta == null
        ? '--'
        : `${delta >= 0 ? '+' : ''}${formatNumber(delta, result.unit)} ${typesetUnit(result.unit_symbol)}`));
    }
    body.append(row);
  }
  table.append(body);
  scroll.append(table);
  return scroll;
}
