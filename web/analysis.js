/* One round trip into WebAssembly, and the numbers it hands back with their provenance. */

import { $, state } from './state.js';
import { element, formatNumber, reply, secondaryDisplay } from './format.js';
import { rankCandidates, initialParameters, namedValues, findMethod } from './registry.js';
import { candidateFor, renderBuildInfo } from './startup.js';
import { unresolvedDecisions, renderDecisions } from './decisions.js';
import { renderSpreadControls, scheduleSpread } from './spread.js';
import { openMetricRecord } from './drawer.js';
import { captureJson, recordAttribution, renderChip } from './plate.js';
// The one import that points back at the module importing this one. Both sides export function
// declarations, so the cycle resolves before either runs. It is here rather than behind a hook
// on the state because the numbers beside a selection have to be redrawn by every analysis, not
// only by the ones a drag started: a rule changed on the rail moves them too, and a panel still
// showing the previous run's figures is the confident wrong number this software exists to stop.
import { renderSelectionNumbers } from './workspace.js';

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
 * chose the rule and were not asked about any of those values. Both kinds of value are
 * stamped, because a named alternative the registry chose is as much a default nobody was
 * asked about as a number is. */
export function selectionFromChosenRule(candidate, forcesDecision) {
  const filled = initialParameters(candidate, forcesDecision);
  return {
    methodId: candidate.id,
    ...filled,
    fromDefault: new Set([...Object.keys(filled.values), ...Object.keys(filled.options)]),
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
    // A slot the reader never opened is bound to the registry's own first ranked rule by
    // `boundMethodId`, so the request always names one. Sending nothing here reported that
    // arrival as a pick, which is the reader's signature on a rule they never saw.
    method_from_registry_default: selection?.methodStated !== true,
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
    // Sent only when the operator has stated it, and the value never travels without the
    // claim that they stated it. A literal here would be standard gravity's second home,
    // and a value sent alone arrives as one the engine filled in for a reader nobody asked,
    // which is the record a reader who measured a gravity at their own plate would get.
    ...(state.gravity != null && {
      gravity_meters_per_second_squared: state.gravity,
      gravity_source: 'stated',
    }),
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
      // A choice between named alternatives moves the number as far as a quantity does, and
      // the two travel in separate maps because the engine reads a name off one and a number
      // off the other. An empty map here is a slot whose rule takes no such choice, never a
      // slot whose reader was never offered one.
      options: state.selection[slot.key]?.options || {},
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
  $('reset-markers').hidden = !Object.values(state.overrides).some((value) => value != null);

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
    clearMetricGrids();
    $('analysis-warnings').replaceChildren(
      notice(
        'danger',
        'Analysis unavailable',
        `No published rule for ${unbound.map((slot) => slot.title).join(' or ')}.`,
      ),
    );
    return;
  }

  /* A rule that declines is an answer, not an exception. It arrives as the record it built,
   * carrying the code, the rule and what could have been asked for instead.
   *
   * What the tab says about the plate is read by the block's own parser, which refuses a name
   * the block does not hold and a member given text of the wrong kind rather than dropping
   * either. That refusal crosses as a throw, so it is caught and shown: an answer read and
   * silently discarded is the fingerprint claiming a plate nobody stated. */
  let answer;
  try {
    // Timed, because what this costs decides how often a reader dragging a window may be
    // answered, and the cost is the recording's length rather than the number of rules bound:
    // measured at 43 ms over 6,000 samples with 23 rules and 410 ms over 72,000 with the same
    // 23. A caller that recomputes on a trailing edge reads this rather than a constant.
    const began = performance.now();
    answer = reply(
      state.loadedTrial.analyse(JSON.stringify(buildRequest()), state.fileName, captureJson()),
    );
    state.analysisMilliseconds = performance.now() - began;
  } catch (raised) {
    clearMetricGrids();
    $('analysis-warnings').replaceChildren(
      notice('danger', 'Plate data error', String(raised?.message ?? raised)),
    );
    return;
  }
  if (answer.refusal) {
    state.analysisRefusal = answer.refusal;
    clearMetricGrids();
    $('analysis-warnings').replaceChildren(
      notice('danger', 'Analysis declined', refusalSentence(answer.refusal)),
    );
    return;
  }
  state.analysisRefusal = null;
  state.analysis = answer.ok;

  recordAttribution();
  renderChip();
  renderBuildInfo();
  state.chart.setAnalysis(state.analysis);
  state.chart.schedule();
  renderMetrics();
  renderSpreadControls();
  scheduleSpread();
  renderDecisions();
  renderSelectionNumbers();
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
      const named = namedValues(parameter);
      if (named.length) {
        selection.options[name] = parameter.default_key ?? named[0].key;
      } else {
        selection.values[name] = parameter?.default ?? parameter?.published_values?.[0];
      }
      selection.recommended.add(name);
      selection.fromDefault.delete(name);
    }
    selection.unresolved = [];
  }
  renderDecisions();
  runAnalysis();
}

const HEADLINE = new Set(['time_to_takeoff_seconds', 'jump_height_from_takeoff_meters']);

function clearMetricGrids() {
  $('headline-metric-grid').replaceChildren();
  $('metric-grid').replaceChildren();
}

function renderMetrics() {
  const headlineGrid = $('headline-metric-grid');
  const grid = $('metric-grid');
  clearMetricGrids();

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
      const primary = element('span', 'metric__primary', formatted);
      primary.append(element('small', null, metric.unit_symbol));
      value.replaceChildren(primary);
      const secondary = secondaryDisplay(metric);
      if (secondary) value.append(element('small', 'metric__secondary', `= ${secondary}`));
      card.append(value);
    }

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
    const ruleIds = [metric.computed_by, ...metric.contributing_method_ids].filter(Boolean);
    // After the parts, the whole: the rules above are the pieces, and the account is the
    // assembly with every value each was bound to. A quantity the trial produced no number
    // for has none, because an account is written around a measurement.
    const account = state.analysis.descriptions?.[metric.key];
    card.append(metricRecord(metric.label, account, ruleIds));
    (HEADLINE.has(metric.key) ? headlineGrid : grid).append(card);
  }

  const host = $('analysis-warnings');
  host.replaceChildren();
  // A rule that declined and a rule that ran and complained are different answers, and the
  // record carries them in two lists. Only the second was drawn, so a declining rule reached
  // the reader as a sentence with nothing attached saying which rule said it, and on a trial
  // where no landmark was placed it reached them as nothing at all.
  //
  // The engine writes a declining rule's sentence into both lists, so the sentence a refusal
  // carries is dropped from the second: one fact, said once, by the rule that said it.
  const declined = new Set((state.analysis.refusals || []).map((refusal) => refusal.message));
  for (const refusal of state.analysis.refusals || []) {
    host.append(notice('danger', 'Declined', refusalSentence(refusal)));
  }
  for (const warning of state.analysis.warnings) {
    if (declined.has(warning)) continue;
    host.append(notice('warning', 'Warning', warning));
  }
}

/* What the rule said, led by the rule that said it. The sentence already names what could
 * have been asked for instead, so the alternatives are not repeated beside it. A refusal
 * raised before any rule was reached carries no id and is shown on its own. */
function refusalSentence(refusal) {
  const rule = refusal.method_id ? `${methodTitle(refusal.method_id)}: ` : '';
  return `${rule}${refusal.message}`;
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
  line.append(element('span', null, `See ${said.label}`));
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
    const choose = element('button', 'chip', 'Choose method');
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
  const count = slots.length;
  line.textContent = `Provisional · ${count} method ${count === 1 ? 'choice' : 'choices'} open`;
  line.title = slots.map((slot) => `${slot.title}: ${boundMethodId(slot.key)}`).join('; ');
  return line;
}

export function methodTitle(id) {
  return (
    findMethod(state.registry, id)?.title ||
    state.build.bindings.find((binding) => binding.id === id)?.title ||
    id
  );
}

/*
 * Who chose the rule itself, in the vocabulary its values are already shown in.
 *
 * A rule the reader picked and a rule that ran because nobody named one move the number by
 * exactly the same amount, so a surface rendering them alike hands a reader a methods section
 * they cannot check. The record names a source per rule and these are its words.
 *
 * Keyed by the wire word the record carries, and a word with no sentence here renders as
 * itself: a source added to the vocabulary reaches a reader as something they can look up
 * rather than as silence, which is the failure this whole surface exists to stop.
 */
const HOW_A_RULE_WAS_CHOSEN = {
  stated: () => 'Chosen by you',
  recommended: () => 'Recommended',
  assumed: () => 'Default',
  cited: (_rule, preset) => preset ? `${preset} pipeline` : 'Published pipeline',
  measured: () => 'Measured here',
  provisional: () => 'Not chosen',
};

/*
 * The record's claim about one rule, as a sentence.
 *
 * `rule` names what the sentence is about, because a row whose own control names no rule
 * leaves "this rule" pointing at nothing and the caller is the only one who knows which it is.
 */
export function ruleSourceText(bound, rule = 'this rule') {
  const source = bound?.method_source;
  if (!source) return null;
  const sentence = HOW_A_RULE_WAS_CHOSEN[source];
  return sentence ? sentence(rule, bound.preset?.id) : `${rule}: ${source}`;
}

/* The claim as a line, carrying the rule it is about, so a reader looking at one of these on
 * screen and a check reading them all are looking at the same pairing. */
export function ruleSourceLine(bound, rule) {
  const text = ruleSourceText(bound, rule);
  if (!text) return null;
  const line = element('span', 'rule-source', text);
  line.dataset.method = bound.method_id;
  return line;
}

/* The record's row for one rule, which carries where its values came from and who chose it. */
export function boundRecordFor(methodId) {
  return state.analysis?.bound_methods?.find((entry) => entry.method_id === methodId) || null;
}

/*
 * The record's claim about a rule that has no row of its own.
 *
 * A rule named beside a number can have run as another rule's named value. The impulse rule
 * binds `integration.rule.trapezoid` as a choice, and the record writes who chose it beside
 * that name on the rule that bound it, so the claim exists and sits one row away. Reading it
 * from there is the record's own word about this id; leaving it out hands a reader four
 * registry entries with no account, on twelve of the sixty-two rules a plain run names beside
 * its numbers.
 */
function claimAboutABoundValue(methodId) {
  for (const row of state.analysis?.bound_methods || []) {
    const pair = (row.bound_parameters || []).find(([, value]) => value === methodId);
    if (pair) return { method_id: methodId, method_source: row.parameter_sources?.[pair[0]] };
  }
  return null;
}

/* A value the request did not carry moved the number as far as one it did, so every value
 * in the fingerprint carries the source the record named for it. */
export function boundValueText(bound, separator = ' ') {
  const sources = bound?.parameter_sources || {};
  return (bound?.bound_parameters || []).map(
    ([name, value]) => `${name}${separator}${value}${sources[name] ? ` (${sources[name]})` : ''}`,
  );
}

function metricRecord(label, account, methodIds) {
  const ids = [...new Set(methodIds)];
  const button = element('button', 'metric-record');
  button.type = 'button';
  button.append(element('span', 'metric-record__name provenance__name', 'Methods'));

  const count = `${ids.length} ${ids.length === 1 ? 'rule' : 'rules'}`;
  button.append(element('span', 'metric-record__count', count));
  button.title = ids.join(' | ');
  button.addEventListener('click', () => openMetricRecord(label, account, ids));
  return button;
}
