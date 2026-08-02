/* How much the method choice moves this number, over every defensible alternative the
 * registry publishes. */

import { $, state } from './state.js';
import { element, formatNumber } from './format.js';
import { availableAxes, GRAVITY_AXIS } from './registry.js';
import { candidateFor } from './startup.js';
import { notice, buildRequest, methodTitle } from './analysis.js';

function currentAxes() {
  const axes = [];
  for (const slot of state.slots) {
    const candidate = candidateFor(slot.key, state.selection[slot.key]?.methodId);
    axes.push(...availableAxes(slot, candidate));
  }
  axes.push(GRAVITY_AXIS);
  return axes;
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
  if (!state.spread.initialised) {
    const methodAxis = axes.find((axis) => axis.id === 'onset:__method__');
    if (methodAxis) state.spread.axes.add(methodAxis.id);
    else for (const axis of axes) if (axis.slot === 'onset') state.spread.axes.add(axis.id);
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
    label.append(document.createTextNode(`${axis.label}: ${axis.display} ${axis.unit}`.trim()));
    host.append(label);
  }
}

export function runSpread() {
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
      `${label} spans ${formatNumber(result.spread_absolute, result.unit)} ${result.unit_symbol} across ` +
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
      const cell = element('td', 'failed', variant.failure_reason || 'no value');
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
