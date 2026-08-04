/* How much the method choice moves this number, over every defensible alternative the
 * registry publishes. */

import { $, state } from './state.js';
import { element, formatNumber, reply } from './format.js';
import { availableAxes, GRAVITY_AXIS } from './registry.js';
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

const MOVEMENT_ONSET = 'movement_onset';

const variesTheRule = (axis) => Boolean(axis.methodIds?.length);

/*
 * What the panel varies before anyone has asked it to vary anything.
 *
 * Net impulse reliability runs from 0.984 to 0.479 across published rules for the start of
 * the movement on identical data, the widest published disagreement of the three constructs
 * a jump height passes through, so that is where the panel opens. Exported because it is
 * the one choice on this screen nobody makes explicitly, which is what a guard has to be
 * able to reach.
 *
 * There is no substitute construct. A panel that quietly opened on a different one would
 * report a spread over a setting the reader never chose, under a heading that says the
 * spread is what choosing costs them, and it would read exactly like a working panel.
 * Returning nothing is visible; substituting is not.
 */
export function openingAxes(axes) {
  const onTheConstruct = axes.filter((axis) => axis.construct === MOVEMENT_ONSET);
  const chosen = onTheConstruct.find(variesTheRule) || onTheConstruct[0];
  return chosen ? [chosen] : [];
}

function openingSummary(axes) {
  if (!axes.length) {
    return 'Nothing is varying yet. Tick a setting below to see how far the number moves across its published values.';
  }
  return `Opening on ${axes.map((axis) => `${axis.label.toLowerCase()}, ${axis.display}`).join('; ')}.`;
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
  showPreviousPositionAsPrevious();
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
      'These figures are for where the marker was',
      'They were computed before you moved it, and they catch up when it comes to rest.',
    ),
  );
}

export function runSpread() {
  const host = $('spread-result');
  delete host.dataset.forPreviousPosition;
  const axes = currentAxes().filter((axis) => state.spread.axes.has(axis.id));
  if (!axes.length) {
    host.replaceChildren(
      notice('warning', 'Nothing selected to vary', 'Tick at least one parameter above to see how far the number moves across its published values.'),
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
    host.replaceChildren(notice('danger', 'The sweep could not run', answer.refusal.message));
    return;
  }
  const result = answer.ok;

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
      `${label} spans ${formatNumber(result.spread_absolute, result.unit)} ${result.unit_symbol} across ` +
        `${result.succeeded} defensible alternatives on this one trial, which is ${percent == null ? 'an undefined fraction' : `${percent.toFixed(1)} percent`} of its own median. ` +
        `Every one of those settings appears in the published literature.` +
        (result.capped ? ` Showing ${result.combinations_run} of ${result.combinations_requested} combinations.` : '') +
        (result.failed ? ` ${result.failed} combinations produced no value and are listed below.` : ''),
    ),
  );
  host.append(headline);
  host.append(whatMoved(result));
  host.append(spreadAxisPlot(result));
  host.append(spreadTable(result, label));
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
    const names = varied.map((axis) => `${axis.construct} (${axis.rules_varied} rules)`).join(', ');
    wrap.append(element('p', 'panel__sub', `Varied ${names}.`));
  }
  for (const rule of held) {
    wrap.append(
      element(
        'p',
        'panel__sub',
        `Held ${rule.construct} at ${rule.method_id}, so this spread is not over it.`,
      ),
    );
  }
  return wrap;
}

function spreadAxisPlot(result) {
  const wrap = element('div', 'spread-axis');
  const low = result.minimum;
  const high = result.maximum;
  const span = high - low || 1;
  const position = (value) => `${(((value - low) / span) * 96 + 2).toFixed(2)}%`;

  wrap.append(element('span', 'spread-axis__legend', 'each tick is one published alternative'));
  wrap.append(element('div', 'spread-axis__line'));
  const bar = element('div', 'spread-axis__span');
  bar.style.left = position(low);
  bar.style.width = `${(((high - low) / span) * 96).toFixed(2)}%`;
  wrap.append(bar);

  for (const variant of result.variants) {
    if (variant.value == null) continue;
    const tick = element('div', 'spread-tick');
    tick.style.left = position(variant.value);
    tick.title = `${readableLabel(variant)}: ${formatNumber(variant.value, result.unit)} ${result.unit_symbol}`;
    wrap.append(tick);
  }
  if (result.baseline_value != null) {
    const tick = element('div', 'spread-tick spread-tick--baseline');
    tick.style.left = position(result.baseline_value);
    tick.title = `your current setting: ${formatNumber(result.baseline_value, result.unit)} ${result.unit_symbol}`;
    wrap.append(tick);
  }

  const lowLabel = element('span', 'spread-bound', `${formatNumber(low, result.unit)} ${result.unit_symbol}`);
  lowLabel.style.left = '2%';
  const highLabel = element('span', 'spread-bound spread-bound--max', `${formatNumber(high, result.unit)} ${result.unit_symbol}`);
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
    row.append(element('td', null, readableLabel(variant)));
    if (variant.value == null) {
      const cell = element('td', 'failed', variant.failure_reason?.message || 'no value');
      cell.colSpan = 2;
      row.append(cell);
    } else {
      row.append(element('td', 'numeric', `${formatNumber(variant.value, result.unit)} ${result.unit_symbol}`));
      const delta = result.baseline_value == null ? null : variant.value - result.baseline_value;
      row.append(element('td', 'numeric', delta == null ? '--' : `${delta >= 0 ? '+' : ''}${formatNumber(delta, result.unit)}`));
    }
    body.append(row);
  }
  table.append(body);
  scroll.append(table);
  return scroll;
}
