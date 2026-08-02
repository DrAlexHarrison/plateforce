/* The rail: one row per construct on the path, in the order the pipeline runs them. */

import { $, state } from './state.js';
import { element } from './format.js';
import { rankCandidates, initialParameters } from './registry.js';
import { candidateFor } from './startup.js';
import { runAnalysis } from './analysis.js';
import { openDrawer } from './drawer.js';

export function unresolvedDecisions() {
  const pending = [];
  for (const slot of state.slots) {
    const selection = state.selection[slot.key];
    if (slot.forcesDecision && !selection.methodId && slot.available.length) {
      pending.push({ slot, what: 'method' });
    }
    for (const name of selection.unresolved || []) pending.push({ slot, what: name });
  }
  return pending;
}

export function renderDecisions() {
  const host = $('decision-list');
  host.replaceChildren();

  const pending = unresolvedDecisions();
  $('decisions-sub').textContent = pending.length
    ? `${pending.length} unresolved. Each one moves the number, so plateforce does not pick for you.`
    : 'All resolved. Every choice below appears in the provenance of the numbers.';

  for (const slot of state.slots) {
    if (!slot.available.length) continue;
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

  const select = document.createElement('select');
  select.setAttribute('aria-label', `${slot.title} method`);
  if (!selection.methodId) {
    const placeholder = element('option', null, 'Choose a method');
    placeholder.value = '';
    select.append(placeholder);
  }
  for (const candidate of rankCandidates(slot.available)) {
    const suffix = candidate.registryBacked
      ? ` (${candidate.status})`
      : candidate.composedFrom
        ? ' (composition)'
        : ' (unfiled)';
    const option = element('option', null, candidate.title + suffix);
    option.value = candidate.id;
    option.selected = candidate.id === selection.methodId;
    select.append(option);
  }
  select.addEventListener('change', () => {
    const candidate = candidateFor(slot.key, select.value);
    state.selection[slot.key] = { methodId: candidate.id, ...initialParameters(candidate, slot.forcesDecision) };
    renderDecisions();
    runAnalysis();
  });
  wrap.append(select);

  const candidate = selection.methodId ? candidateFor(slot.key, selection.methodId) : null;
  if (candidate) {
    const failure = candidate.method?.failure;
    if (failure) {
      const note = element(
        'p',
        'undecided undecided--clamped',
        `Fails on ${(failure.rate * 100).toFixed(1)}% of trials (${failure.numerator} of ${failure.denominator}, ${failure.corpus}), detectability ${failure.detectability}: ${failure.definition}`,
      );
      note.title = failure.definition;
      wrap.append(note);
    }
    wrap.append(renderParameters(slot, candidate, selection));

    if (candidate.method) {
      const inspect = element('button', 'chip', 'Rule, citations and bias');
      inspect.type = 'button';
      inspect.addEventListener('click', () => openDrawer(candidate.method));
      const row = element('div', 'metric__provenance');
      row.append(inspect);
      wrap.append(row);
    }
  }

  return wrap;
}

function renderParameters(slot, candidate, selection) {
  const host = element('div', 'decision__params');
  for (const parameter of candidate.method?.parameter || []) {
    const values = parameter.published_values || [];
    // A parameter with neither a published value nor a default has nothing to bind, so
    // there is no control to draw.
    if (!values.length && !Number.isFinite(parameter.default)) continue;
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
      // A window dragged on the trace lands on a span no paper published, and it is the
      // span the number was computed over, so it belongs in the list.
      const chosen = selection.values[parameter.name];
      if (!unresolved && Number.isFinite(chosen) && !values.includes(chosen)) {
        const option = element('option', null, `${Number(chosen.toFixed(3))} (placed by hand)`);
        option.value = String(chosen);
        option.selected = true;
        select.append(option);
      }
      select.addEventListener('change', () => {
        selection.values[parameter.name] = Number(select.value);
        selection.unresolved = (selection.unresolved || []).filter((name) => name !== parameter.name);
        renderDecisions();
        runAnalysis();
      });
      row.append(select);
    } else if (values.length === 1 || Number.isFinite(parameter.default)) {
      const input = document.createElement('input');
      input.type = 'number';
      input.id = id;
      input.step = 'any';
      input.value = String(selection.values[parameter.name] ?? parameter.default ?? values[0] ?? '');
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
