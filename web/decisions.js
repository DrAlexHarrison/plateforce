/* The rail: one row per construct on the path, in the order the pipeline runs them. */

import { $, state } from './state.js';
import { element } from './format.js';
import { rankCandidates, findMethod, namedValues, NOT_A_CHOICE } from './registry.js';
import { candidateFor } from './startup.js';
import {
  runAnalysis,
  acceptRecommended,
  recordStated,
  selectionFromChosenRule,
  boundMethodId,
  boundRecordFor,
  methodTitle,
  ruleSourceLine,
} from './analysis.js';
import { openDrawer } from './drawer.js';

/*
 * Two of the registry's surfacing verdicts oblige the interface to say something about a
 * rule nobody was asked about, and they differ in what is owed. `default_and_show` is
 * displayed unasked. `surface_on_demand` is named, with its alternatives one interaction
 * away. A value the rule chose for itself, under a verdict that says show it, that the
 * interface never shows, is a defect in the interface rather than in the record.
 */
const SHOWS_UNASKED = 'default_and_show';
const NAMES_ITS_ALTERNATIVES = 'surface_on_demand';

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
    ? `${pending.length} ${pending.length === 1 ? 'choice' : 'choices'} open.`
    : '';

  // One act covering every open choice, which is a different act from choosing each and is
  // recorded as the one it is. It sits with the choices rather than in front of the
  // numbers, because the numbers are already there.
  if (pending.length) {
    const accept = element('button', 'button button--primary button--small', 'Use recommended rules');
    accept.type = 'button';
    accept.id = 'accept-recommended';
    accept.addEventListener('click', acceptRecommended);
    host.append(accept);
  }

  // Which constructs got a row is read off the rows that were drawn rather than off the
  // slots, because a slot with nothing runnable under it is skipped here and its rules would
  // then have a row that never appears to hold them.
  const drawn = new Set();
  for (const slot of state.slots) {
    if (!slot.available.length) continue;
    drawn.add(slot.construct);
    host.append(renderSlot(slot));
  }
  const records = rulesThatRanUnderNoRow(drawn);
  if (records) host.append(records);
}

function renderSlot(slot) {
  const selection = state.selection[slot.key];
  const wrap = element('div', 'decision');

  /*
   * Two different questions, and collapsing them is the bug. How the row is presented
   * comes from the entry bound to it now. Whether it is still a decision comes from the
   * construct, because one unresolved entry anywhere in the construct leaves every value
   * below it provisional.
   */
  const boundEntry = selection.methodId ? candidateFor(slot.key, selection.methodId) : null;
  const decisionPending = slot.forcesDecision && !selection.methodId && slot.available.length > 0;

  const head = element('div', 'decision__head');
  head.append(element('span', 'decision__title', slot.title));
  if (decisionPending) {
    wrap.classList.add('decision--provisional');
    head.append(element('span', 'tag tag--provisional', 'provisional'));
  } else if (boundEntry?.surfacing === 'default_and_hide') {
    head.append(element('span', 'tag tag--advanced', 'advanced'));
  }
  wrap.append(head);
  const select = document.createElement('select');
  select.setAttribute('aria-label', `${slot.title} method`);
  // The construct is the row's identity, and it stays readable when the label changes.
  select.dataset.construct = slot.construct;
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
    state.selection[slot.key] = selectionFromChosenRule(candidate, slot.forcesDecision);
    state.selection[slot.key].methodStated = true;
    renderDecisions();
    runAnalysis();
  });
  const control = element('div', 'decision__control');
  control.append(select);

  const candidate = boundEntry;
  if (candidate?.method) {
    const inspect = element('button', 'chip decision__inspect', 'Details');
    inspect.type = 'button';
    inspect.addEventListener('click', () =>
      openDrawer(candidate.method, candidate.id, boundRecordFor(candidate.id)));
    control.append(inspect);
  }
  wrap.append(control);

  const source = ruleSourceNode(slot);
  if (source) head.append(source);
  const running = runningRuleNode(slot, selection);
  if (running) wrap.append(running);

  if (candidate) {
    const settings = element('div', 'decision__settings-body');
    // The rate, its denominator and the corpus it was measured on. What counted as a failure
    // is the definition, and it reads in full in the drawer this row's Details button opens,
    // which is a place a reader on a touch screen can reach.
    const failure = candidate.method?.failure;
    if (failure) {
      settings.append(element(
        'p',
        'undecided',
        `${(failure.rate * 100).toFixed(1)}% failure rate · ${failure.numerator} of ${failure.denominator} · ` +
          `${failure.corpus} · ${failure.detectability}`,
      ));
    }
    const parameters = renderParameters(slot, candidate, selection);
    const parametersOpen = (selection.unresolved || []).length > 0;
    if (parametersOpen) wrap.append(parameters);
    else if (parameters.childElementCount) settings.append(parameters);
    const beneath = choicesBeneath(slot, selection.methodId);
    if (beneath) settings.append(beneath);
    if (settings.childElementCount) {
      const disclosure = element('details', 'decision__settings');
      disclosure.append(element('summary', null, 'Settings'), settings);
      wrap.append(disclosure);
    }
  }
  const ranBeside = renderRulesThatRanBeside(slot, selection.methodId);
  if (ranBeside) wrap.append(ranBeside);

  return wrap;
}

/*
 * Who chose the rule this row is running, read off the record rather than off the control.
 *
 * The control is the wrong witness. A slot that forces no decision opens with the registry's
 * first-ranked rule already selected, so the dropdown shows a rule as chosen that nobody
 * picked, and the entry's `recommended` status renders in the same option text and reads like
 * an endorsement the reader acted on. The record is the only place the act itself is written.
 */
function ruleSourceNode(slot) {
  const running = boundMethodId(slot.key);
  const bound = running ? boundRecordFor(running) : null;
  return bound ? ruleSourceLine(bound) : null;
}

/*
 * The rule the row is running, spelt out where nothing else on the row spells it.
 *
 * A row awaiting a decision draws its control empty, so that a rule nobody picked is never
 * drawn as picked. The claim beside the title then reads "Default" over a control reading
 * "Choose a method": a record that a rule was defaulted, on the one row where the reader has
 * least idea which rule that was, and the numbers under it rest on that rule.
 *
 * Named once per row rather than once per mention. A rule the registry says to surface
 * already gets a row of its own below, and repeating its title here would put the same
 * sentence on the row twice. Read off what ran, so no id is named in this file.
 */
function runningRuleNode(slot, selection) {
  if (selection.methodId) return null;
  const running = boundMethodId(slot.key);
  if (!running) return null;
  for (const { bound } of ranUnasked((entry) => entry.construct === slot.construct)) {
    if (bound.method_id === running) return null;
  }
  const line = element('span', 'decision__running', methodTitle(running));
  line.dataset.method = running;
  return line;
}

/* Which slots the reader has opened, so a choice made inside one does not shut the panel it
 * was made in. The rail is rebuilt on every change, and a disclosure whose state lived only
 * in the DOM closed under the reader on every pick. */
const opened = new Set();

/*
 * The choices the rules running beneath the bound one take.
 *
 * They read their values off the same slot the rule above reads, so a name stated here
 * reaches the operator that composes the bound rule and lands in the same record. Behind a
 * disclosure because the registry's verdict on most of them is to bind a value and not
 * display it, which governs what is shown unasked rather than what a reader may change. An
 * entry the registry rules out as a user choice is not here at all.
 *
 * Read off the rules that ran rather than off a list, so no id is named in this file and a
 * rule that stops running stops offering.
 */
function choicesBeneath(slot, statedId) {
  const body = element('div', 'decision__params');
  const drawn = new Set();
  for (const bound of state.analysis?.bound_methods || []) {
    if (bound.method_id === statedId) continue;
    const method = findMethod(state.registry, bound.method_id);
    if (!method || method.construct !== slot.construct) continue;
    if (NOT_A_CHOICE.has(method.gui?.surfacing)) continue;
    for (const parameter of method.parameter || []) {
      if (!namedValues(parameter).length || drawn.has(parameter.name)) continue;
      drawn.add(parameter.name);
      body.append(namedChoiceRow(slot, parameter, state.selection[slot.key]));
    }
  }
  if (!drawn.size) return null;

  const wrap = element('details', 'beneath');
  wrap.open = opened.has(slot.key);
  wrap.addEventListener('toggle', () => {
    if (wrap.open) opened.add(slot.key);
    else opened.delete(slot.key);
  });
  wrap.append(element('summary', null, 'More choices'), body);
  return wrap;
}

/*
 * The rules that ran inside this row's construct and were never asked about.
 *
 * Read off what ran rather than off what the reader has settled, because the row is
 * drawn before anybody has chosen anything and the rules had already run by then. The
 * exclusion is the rule the row states through its own control: where the reader has
 * bound nothing the row states nothing, so the rule running under it belongs in the list
 * with the others rather than being suppressed by a row that never named it.
 *
 * The registry's verdict on each entry decides whether it appears at all, so the list
 * follows the data and no id is named in this file.
 */
function renderRulesThatRanBeside(slot, statedId) {
  const host = element('div', 'ran-beside');
  for (const { method, bound } of ranUnasked((entry) => entry.construct === slot.construct)) {
    if (bound.method_id === statedId) continue;
    host.append(ranBesideRow(method, bound));
  }
  return host.childElementCount ? host : null;
}

/*
 * The rules that ran under a construct with no row on the rail.
 *
 * Both verdicts oblige the interface to say something about a rule nobody was asked about,
 * and the obligation is carried by the entry rather than by whether the reader happened to
 * put its construct on the path. A rule reached that way still ran, still fixed the values
 * it ran under, and still stands behind a number on screen, so it is owed the same sentence
 * it would get one row higher.
 *
 * A record and not a question: these rows carry no control that would bind a rule, because
 * a construct nobody asked for raises no decision, and turning what ran into a choice
 * nobody made would put a fourth and a fifth quantity in front of a reader who asked for
 * neither. The quantity picker below is where a reader takes one of these over.
 *
 * They carry no sentence under the title either. What a decision row prints there is what
 * choosing costs, and a row holding no choice has no cost to name.
 */
function rulesThatRanUnderNoRow(drawn) {
  const ran = [];
  for (const { method, bound } of ranUnasked((entry) => !drawn.has(entry.construct))) {
    ran.push({ method, bound });
  }
  if (!ran.length) return null;

  const wrap = element('div', 'decision decision--record');
  const head = element('div', 'decision__head');
  head.append(element('span', 'decision__title', 'Applied rules'));
  wrap.append(head);

  const host = element('div', 'ran-beside');
  for (const { method, bound } of ran) host.append(ranBesideRow(method, bound));
  wrap.append(host);
  return wrap;
}

/* Every rule this analysis ran that the registry says to display unasked or to name, in the
 * order the pipeline ran them, narrowed by where the caller is putting them. */
function* ranUnasked(wanted) {
  for (const bound of state.analysis?.bound_methods || []) {
    const method = findMethod(state.registry, bound.method_id);
    if (!method || !wanted(method)) continue;
    const verdict = method.gui?.surfacing;
    if (verdict !== SHOWS_UNASKED && verdict !== NAMES_ITS_ALTERNATIVES) continue;
    yield { method, bound };
  }
}

/* One rule that ran unasked, wherever it is placed. Both placements render it the same way,
 * because a reader meeting the same fact in two parts of the rail is meeting one fact. */
function ranBesideRow(method, bound) {
  const verdict = method.gui.surfacing;
  const row = element('button', `ran-beside__row ran-beside__row--${verdict.replace(/_/g, '-')}`);
  row.type = 'button';
  row.append(element('span', 'ran-beside__title', method.title));
  // The claim about the rule leads the claims about its values, in one voice: a reader who can
  // see where every number came from and not where the rule came from is reading three
  // quarters of a methods section.
  const source = ruleSourceLine(bound);
  if (source) row.append(source);
  const values = valuesWithTheirSource(method, bound);
  if (values) row.append(element('span', 'ran-beside__value', values));
  row.append(element('span', 'ran-beside__action', verdict === NAMES_ITS_ALTERNATIVES ? 'Alternatives' : 'Details'));
  row.title = values ? `${method.title}: ${values}` : method.title;
  row.addEventListener('click', () => openDrawer(method, bound.method_id, bound));
  return row;
}

/*
 * A value the caller did not supply is shown with whoever proposed it, because a number on
 * screen that nobody asked for is only checkable if the reader can reach where it came from.
 *
 * The record names a source per value, so a value the rule fell back to and a value the
 * rule measured off this trace read as the two different things they are. A registry
 * default carries the entry that published it as well, which the source word alone does
 * not say.
 */
function valuesWithTheirSource(method, bound) {
  const sources = bound.parameter_sources || {};
  return (bound.bound_parameters || [])
    .map(([name, value]) => {
      const source = sources[name];
      const published = (method.parameter || []).find((entry) => entry.name === name)?.default_source;
      const sourceLabel = source === 'assumed'
        ? `default${published ? `: ${published}` : ''}`
        : source === 'measured'
          ? 'measured'
          : source === 'stated'
            ? 'entered'
            : source === 'recommended'
              ? 'recommended'
              : source;
      return `${name} ${value}${sourceLabel ? ` · ${sourceLabel}` : ''}`;
    })
    .join(', ');
}

function renderParameters(slot, candidate, selection) {
  const host = element('div', 'decision__params');
  for (const parameter of candidate.method?.parameter || []) {
    const row = namedValues(parameter).length
      ? namedChoiceRow(slot, parameter, selection)
      : quantityRow(slot, parameter, selection);
    if (row) host.append(row);
  }
  return host;
}

/* One control and its label, in the shape both kinds of value share. */
function parameterRow(slot, parameter, spoken) {
  const row = element('div', 'param');
  const id = `param-${slot.key}-${parameter.name}`;
  const label = element('label', null, spoken);
  label.htmlFor = id;
  row.append(label);
  return { row, id };
}

/*
 * A value that varies by name rather than by number, offered as the entry's own alternatives.
 *
 * The words are the registry's, because the key is what a result records and a reader
 * choosing between `population` and `sample` is choosing between two sentences the registry
 * already writes. Nothing here names a value, so an entry that gains one gains an option.
 *
 * Not stated is a state and not a value: unstated, the rule runs the value it carries and
 * the record says nobody was asked. Selecting the entry's declared default is a different
 * act from arriving at it, and the two produce different records.
 */
function namedChoiceRow(slot, parameter, selection) {
  const named = namedValues(parameter);
  const { row, id } = parameterRow(slot, parameter, parameter.name);
  const select = document.createElement('select');
  select.id = id;
  select.dataset.option = parameter.name;

  const unresolved = (selection.unresolved || []).includes(parameter.name);
  const chosen = selection.options?.[parameter.name];
  if (unresolved || chosen == null) {
    const placeholder = element('option', null, unresolved ? `choose from ${named.length}` : 'Not stated');
    placeholder.value = '';
    placeholder.selected = true;
    select.append(placeholder);
  }
  for (const value of named) {
    const isDefault = value.key === parameter.default_key;
    const option = element(
      'option',
      null,
      `${value.label || value.key}${isDefault ? ` (default, ${parameter.default_source || 'unsourced'})` : ''}`,
    );
    option.value = value.key;
    option.selected = !unresolved && chosen === value.key;
    select.append(option);
  }
  select.addEventListener('change', () => {
    selection.options ??= {};
    if (select.value === '') delete selection.options[parameter.name];
    else selection.options[parameter.name] = select.value;
    selection.unresolved = (selection.unresolved || []).filter((name) => name !== parameter.name);
    recordStated(selection, parameter.name);
    renderDecisions();
    runAnalysis();
  });
  row.append(select);
  return row;
}

/* A value that varies by number, offered as the values the literature published. */
function quantityRow(slot, parameter, selection) {
  const values = parameter.published_values || [];
  // A parameter with neither a published value nor a default has nothing to bind, so
  // there is no control to draw.
  if (!values.length && !Number.isFinite(parameter.default)) return null;
  const { row, id } = parameterRow(
    slot,
    parameter,
    `${parameter.name}${parameter.unit ? ` (${parameter.unit})` : ''}`,
  );

  if (values.length > 1) {
    const select = document.createElement('select');
    select.id = id;
    select.dataset.parameter = parameter.name;
    const unresolved = (selection.unresolved || []).includes(parameter.name);
    const chosen = selection.values[parameter.name];
    // Two states rather than one. A row awaiting a forced decision says how many there are
    // to choose from; a row whose entry publishes several values and declares no default has
    // nothing bound and must say so, because a value shown as selected while the request
    // carries none is the number on screen disagreeing with the number that ran.
    if (unresolved || !Number.isFinite(chosen)) {
      const placeholder = element('option', null, unresolved ? `choose from ${values.length}` : 'Not stated');
      placeholder.value = '';
      placeholder.selected = true;
      select.append(placeholder);
    }
    for (const value of values) {
      const option = element('option', null, `${value}${value === parameter.default ? ` (default, ${parameter.default_source || 'unsourced'})` : ''}`);
      option.value = String(value);
      option.selected = !unresolved && chosen === value;
      select.append(option);
    }
    // A window dragged on the trace lands on a span no paper published, and it is the
    // span the number was computed over, so it belongs in the list.
    if (!unresolved && Number.isFinite(chosen) && !values.includes(chosen)) {
      const option = element('option', null, `${Number(chosen.toFixed(3))} (placed by hand)`);
      option.value = String(chosen);
      option.selected = true;
      select.append(option);
    }
    select.addEventListener('change', () => {
      if (select.value === '') delete selection.values[parameter.name];
      else selection.values[parameter.name] = Number(select.value);
      selection.unresolved = (selection.unresolved || []).filter((name) => name !== parameter.name);
      recordStated(selection, parameter.name);
      renderDecisions();
      runAnalysis();
    });
    row.append(select);
  } else {
    const input = document.createElement('input');
    input.type = 'number';
    input.id = id;
    input.step = 'any';
    input.value = String(selection.values[parameter.name] ?? parameter.default ?? values[0] ?? '');
    input.addEventListener('change', () => {
      selection.values[parameter.name] = Number(input.value);
      recordStated(selection, parameter.name);
      runAnalysis();
    });
    row.append(input);
  }
  return row;
}
