/* One round trip into WebAssembly, and the numbers it hands back with their provenance. */

import { $, state } from './state.js';
import { element, formatNumber, reply, secondaryDisplay } from './format.js';
import { rankCandidates, initialParameters, findMethod } from './registry.js';
import { candidateFor } from './startup.js';
import { unresolvedDecisions, renderDecisions } from './decisions.js';
import { renderSpreadControls, scheduleSpread } from './spread.js';
import { openDrawer } from './drawer.js';

/*
 * The rule a slot is running under right now.
 *
 * A slot nobody has resolved runs provisionally under the registry's own first-ranked
 * runnable candidate, which is the same rule the recommendation offers. Three method ids
 * written here instead would be a silent default in the one place the provisional state
 * exists to prevent one, and the browser's provisional rule and its recommended rule could
 * then differ with neither surface saying so.
 */
export function boundMethodId(slotKey) {
  const chosen = state.selection[slotKey]?.methodId;
  if (chosen) return chosen;
  const slot = state.slots.find((entry) => entry.key === slotKey);
  return rankCandidates(slot?.available || [])[0]?.id || null;
}

/*
 * Where each bound value came from, tracked as the reader acts rather than inferred after.
 *
 * Two sets per slot, both naming entries in the slot's parameters. A name in `fromDefault`
 * is a registry default nobody was asked about. A name in `recommended` was filled by the
 * one act of taking the recommendation. A name in neither was stated by the reader.
 *
 * The clearing is the load-bearing half. A name left in `fromDefault` after the reader
 * typed over it under-claims an act that did happen; a name left in `recommended` after
 * they hand-picked claims one that did not, which forges a signature. Without this the
 * browser sends a registry default the reader never saw as a value they stated.
 */
export function withSources(selection) {
  selection.fromDefault ??= new Set();
  selection.recommended ??= new Set();
  return selection;
}

/* The reader supplied this value themselves, so it belongs to no other source. */
export function recordStated(selection, name) {
  withSources(selection);
  selection.fromDefault.delete(name);
  selection.recommended.delete(name);
}

/* A rule the reader picked, with every parameter the registry filled in behind it. They
 * chose the rule and were not asked about any of those values. */
export function selectionFromChosenRule(candidate, forcesDecision) {
  const filled = initialParameters(candidate, forcesDecision);
  return {
    methodId: candidate.id,
    ...filled,
    fromDefault: new Set(Object.keys(filled.values)),
    recommended: new Set(),
    methodFromRecommendation: false,
    // Set where the reader names the rule. A slot that opened under the registry's first
    // ranked candidate carries the same shape and a false here, so arriving at a rule and
    // choosing it stay two different records rather than one.
    methodStated: false,
  };
}

/* The slots running under a rule nobody picked, and therefore the values that carry the
 * provisional state and the exports that refuse. */
export function provisionallyBoundSlots() {
  const seen = new Set();
  return unresolvedDecisions()
    .map(({ slot }) => slot)
    .filter((slot) => !seen.has(slot.key) && seen.add(slot.key));
}

/*
 * Where each value in a slot came from, sent with the values rather than kept in the tab.
 *
 * The request always carries every name, and an empty pair here means the engine reads all
 * of them as stated by the reader. So omitting these is not the cautious reading, it is the
 * strongest claim the request can make, asserted by default about values nobody ever saw.
 */
function whereTheValuesCameFrom(slot) {
  const selection = state.selection[slot];
  return {
    recommended: [...(selection?.recommended ?? [])],
    from_registry_default: [...(selection?.fromDefault ?? [])],
    method_from_recommendation: selection?.methodFromRecommendation ?? false,
  };
}

/*
 * The evidence a slot the request names by its own field carries beyond its rule.
 *
 * Two of those fields take a dragged marker and the third takes where the weighing window
 * starts, and each refuses the other's name, so what a slot sends is read off what the tab
 * is holding for it rather than off a list of field names written here.
 */
function placementFor(slotKey) {
  return slotKey in state.overrides
    ? { manual_index: state.overrides[slotKey] }
    : { start_index: state.weighing.startIndex };
}

/*
 * One request naming every construct on the path.
 *
 * Three maps rather than two, because the engine reaches a construct three ways and refuses
 * a request that names one through the wrong one. A construct the request names by its own
 * field goes there. A rule that conditions the signal the landmark rules then read runs
 * before them and goes in `conditioning`. Everything else goes into `derived` under the id
 * the registry declares for it, which is the same key the terminal takes.
 *
 * Which of the three is read off the build rather than decided here, so a rail that grows a
 * row sends that row without an edit and no construct is named in this file.
 *
 * A construct the reader has not put on the path is in none of them, and the engine runs it
 * under its declared default and records that it did. Sending it unasked would claim a
 * choice nobody made.
 */
export function buildRequest() {
  const request = {
    derived: {},
    conditioning: {},
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

  for (const slot of state.slots) {
    const choice = {
      method_id: boundMethodId(slot.key),
      parameters: state.selection[slot.key]?.values || {},
      options: {},
      ...whereTheValuesCameFrom(slot.key),
    };
    if (slot.spine) request[slot.key] = { ...choice, ...placementFor(slot.key) };
    else if (slot.conditioning) request.conditioning[slot.construct] = choice;
    else request.derived[slot.construct] = choice;
  }
  return request;
}

export function runAnalysis() {
  if (!state.loadedTrial) return;
  $('reset-markers').disabled = !Object.values(state.overrides).some((value) => value != null);

  /*
   * A decision nobody has made does not stop the number arriving. It changes what the
   * number is: it renders provisional, the rule that produced it is named beside it, the
   * choice is one interaction away, and nothing carrying it can leave the building. The
   * undergraduate meets their homework immediately; the graduate cannot ship a pipeline
   * they never resolved, because the artifact they need is the one provisional withholds.
   */
  state.provisional = provisionallyBoundSlots();

  const unbound = state.slots.filter((slot) => !boundMethodId(slot.key));
  if (unbound.length) {
    $('metric-grid').replaceChildren();
    $('analysis-warnings').replaceChildren(
      notice(
        'danger',
        'This trial cannot be analysed yet',
        `No rule this build runs is published for ${unbound.map((slot) => slot.title).join(' or ')}.`,
      ),
    );
    return;
  }

  /* A rule that declines is an answer, not an exception. It arrives as the record it built,
   * carrying the code, the rule and what could have been asked for instead. A throw here is
   * the bundle itself being broken, which is a different thing and reads differently. */
  const answer = reply(
    state.loadedTrial.analyse(JSON.stringify(buildRequest()), state.fileName),
  );
  if (answer.refusal) {
    state.analysisRefusal = answer.refusal;
    $('metric-grid').replaceChildren();
    $('analysis-warnings').replaceChildren(
      notice('danger', 'The analysis could not run', answer.refusal.message),
    );
    return;
  }
  state.analysisRefusal = null;
  state.analysis = answer.ok;

  state.chart.setAnalysis(state.analysis);
  state.chart.schedule();
  renderMetrics();
  renderSpreadControls();
  scheduleSpread();
  renderDecisions();
}

export function notice(kind, title, body) {
  const node = element('div', `notice notice--${kind}`);
  node.append(element('strong', null, title), element('p', null, body));
  return node;
}

/* Resolving every forced decision at once is still an explicit act, and it is recorded as
 * one. It is not the same as never having been asked. */
export function acceptRecommended() {
  for (const slot of state.slots) {
    const before = withSources(state.selection[slot.key]);
    const pickedTheRule = !before.methodId && slot.available.length > 0;
    if (pickedTheRule) {
      const candidate = rankCandidates(slot.available)[0];
      state.selection[slot.key] = selectionFromChosenRule(candidate, slot.forcesDecision);
      state.selection[slot.key].methodFromRecommendation = true;
    }

    const selection = withSources(state.selection[slot.key]);
    const candidate = candidateFor(slot.key, selection.methodId);
    // Only the names this click fills. A parameter that was already sitting there came
    // from the registry with nobody asked, and the reader accepted the rule rather than
    // that value, so marking it recommended would be a silent default wearing a signature.
    for (const name of selection.unresolved || []) {
      const parameter = (candidate?.method?.parameter || []).find((entry) => entry.name === name);
      selection.values[name] = parameter?.default ?? parameter?.published_values?.[0];
      selection.recommended.add(name);
      selection.fromDefault.delete(name);
    }
    selection.unresolved = [];
  }
  renderDecisions();
  runAnalysis();
}

const HEADLINE = new Set(['time_to_takeoff_seconds', 'jump_height_from_takeoff_meters']);

function renderMetrics() {
  const grid = $('metric-grid');
  grid.replaceChildren();

  // Where each signal was said in full, so the cards it also qualifies point at it rather
  // than repeating it. A signal about seven absent values is one finding, and a reader who
  // meets the same three sentences on the sixth card learns nothing from it.
  const saidUnder = new Map();

  for (const metric of state.analysis.metrics) {
    // Provisional taints its closure: a value rests on every rule that fed it, so a value
    // fed by a slot nobody has chosen is itself still to be chosen.
    const restingOn = state.provisional.filter((slot) =>
      metric.contributing_method_ids.includes(boundMethodId(slot.key)),
    );
    const card = element(
      'div',
      `metric${HEADLINE.has(metric.key) ? ' metric--headline' : ''}${restingOn.length ? ' metric--provisional' : ''}`,
    );
    card.append(element('span', 'metric__label', metric.label));

    const formatted = formatNumber(metric.value, metric.unit);
    if (formatted == null) {
      // The card knows the value is absent and not why. The rule that produced nothing says
      // so in its own words in the warnings, so naming a cause here would be a second and
      // possibly different answer to the same question.
      card.append(element('p', 'metric__value metric__value--absent', 'No value on this trial'));
    } else {
      const value = element('p', 'metric__value', formatted);
      value.append(element('small', null, metric.unit_symbol));
      const secondary = secondaryDisplay(metric);
      if (secondary) value.append(element('small', null, `= ${secondary}`));
      card.append(value);
    }

    if (metric.note) card.append(element('p', 'metric__note', metric.note));
    if (restingOn.length) card.append(stillToBeChosen(restingOn));
    for (const signal of signalsQualifying(metric.key)) {
      const already = saidUnder.get(signal);
      if (already) {
        card.append(signalSaidUnder(signal, already));
      } else {
        saidUnder.set(signal, { label: metric.label, card });
        card.append(renderSignal(signal));
      }
    }
    // The rule that produced this number leads the rules that fed it. The record names the
    // two separately and the card was drawing only the second, so seven of eleven values
    // listed their inputs and not the rule that computed them.
    card.append(provenanceRow([metric.computed_by, ...metric.contributing_method_ids].filter(Boolean)));
    grid.append(card);
  }

  const host = $('analysis-warnings');
  host.replaceChildren();
  for (const warning of state.analysis.warnings) {
    host.append(notice('warning', 'The rule reported a problem', warning));
  }
}

/*
 * What the software already knows about the number in this card.
 *
 * The engine says which metric keys each signal is about, so the placement needs no second
 * lookup table here and a signal cannot drift away from the value it qualifies.
 */
function signalsQualifying(metricKey) {
  return (state.analysis.signals || []).filter((signal) => (signal.qualifies || []).includes(metricKey));
}

/*
 * The two figures a signal compares, printed as the different numbers they are.
 *
 * One decimal each suits the magnitudes it was written for. Two instants a twentieth of a
 * second apart both render as 1.2, so a reader is shown one number twice and told it is a
 * comparison. Both figures take the fewest decimals that tell them apart, and the same number
 * of them, since two figures compared at different precisions is the same defect smaller.
 */
function figureAgainstThreshold(signal) {
  let places = 1;
  while (places < 4 && signal.value.toFixed(places) === signal.threshold.toFixed(places)) places += 1;
  return `${signal.value.toFixed(places)} ${signal.unit} against ${signal.threshold.toFixed(places)} ${signal.unit}`;
}

/*
 * A value this signal is also about, on a card that is not the one carrying the signal.
 *
 * It names the comparison and where that comparison is stated, so a reader who scrolls to
 * the fifth of seven values with nothing in them learns that all of them have one cause and
 * reaches it in one interaction.
 */
function signalSaidUnder(signal, said) {
  const line = element('button', `metric__signal-elsewhere metric__signal-elsewhere--${signal.status.replace(/_/g, '-')}`);
  line.type = 'button';
  // The comparison is named where it is stated rather than a second time here. Repeating a
  // label this long turns the sixth of seven of these into six lines a reader scrolls past.
  line.append(element('span', null, `The reason is stated under ${said.label}`));
  line.addEventListener('click', () => {
    said.card.scrollIntoView({ block: 'center' });
  });
  return line;
}

/* A rate stated with no action leaves the reader holding a diagnosis they cannot act on,
 * which is the half of this pattern that does the work. The threshold is shown because the
 * threshold is itself a choice, and a reader who disagrees with it can see what it was.
 *
 * A signal holding no value says which status it is under, taken from the record rather than
 * written here, so a status this page has never heard of still reaches the reader under its
 * own name. A sentence written for one signal is false of the next one to carry no value. */
function renderSignal(signal) {
  const wrap = element('p', `metric__signal metric__signal--${signal.status.replace(/_/g, '-')}`);
  wrap.append(element('span', 'metric__signal-label', signal.label));
  wrap.append(element('span', 'metric__signal-remedy', signal.remedy));
  wrap.append(
    element(
      'span',
      'metric__signal-figure',
      signal.value == null
        ? signal.status.replace(/_/g, ' ')
        : figureAgainstThreshold(signal),
    ),
  );

  if (signal.remedy_construct) {
    const choose = element('button', 'chip', 'Choose the rule');
    choose.type = 'button';
    choose.addEventListener('click', () => {
      const select = document.querySelector(`#decision-list select[data-construct="${signal.remedy_construct}"]`);
      select?.scrollIntoView({ block: 'center' });
      select?.focus();
    });
    wrap.append(choose);
  }
  return wrap;
}

/* Named beside the number rather than in a drawer, with the choice one interaction away.
 * The rule that produced this one is named because it is what the value is: a different
 * choice is a different number, which is the thing the reader is being asked to see. */
function stillToBeChosen(slots) {
  const line = element('p', 'metric__provisional');
  line.append(element('strong', null, 'provisional'));
  for (const slot of slots) {
    const id = boundMethodId(slot.key);
    const sentence = element('span');
    sentence.append(document.createTextNode(`${slot.title} is still to be chosen. `));
    sentence.append(element('code', null, id));
    sentence.append(document.createTextNode(' produced this one.'));
    line.append(sentence);
  }
  const choose = element('button', 'chip', slots.length > 1 ? 'Choose the rules' : 'Choose the rule');
  choose.type = 'button';
  choose.addEventListener('click', () => {
    const select = document.querySelector(`#decision-list select[data-construct="${slots[0].construct}"]`);
    select?.scrollIntoView({ block: 'center' });
    select?.focus();
  });
  line.append(choose);
  return line;
}

export function methodTitle(id) {
  return (
    findMethod(state.registry, id)?.title ||
    state.build.bindings.find((binding) => binding.id === id)?.title ||
    id
  );
}

/* A value the request did not carry moved the number as far as one it did, so every value
 * in the fingerprint carries the source the record named for it. */
export function boundValueText(bound, separator = ' ') {
  const sources = bound?.parameter_sources || {};
  return (bound?.bound_parameters || []).map(
    ([name, value]) => `${name}${separator}${value}${sources[name] ? ` (${sources[name]})` : ''}`,
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
