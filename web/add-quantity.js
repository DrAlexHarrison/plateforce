/*
 * Reaching a quantity the current path does not visit.
 *
 * A search over the field's spoken words rather than a list of every construct the registry
 * declares: a construct nobody has asked for is not on the path, so no rule instantiates it
 * and it raises no decision, and putting all of them on screen would ask a reader to decline
 * fifty-odd choices to get to one.
 *
 * What is offered is what a rule can produce on the recording that is open. Nothing here
 * names a construct, and nothing here decides a method.
 */

import { $, state } from './state.js';
import { element } from './format.js';
import { buildDecisionModel } from './registry.js';
import { initialiseMissingSelections } from './startup.js';
import { renderDecisions } from './decisions.js';
import { runAnalysis, withSources } from './analysis.js';

/* Every construct a bound rule can fill that the path does not already visit, by the words
 * the registry says the field speaks. */
export function offerableConstructs() {
  const visited = new Set(state.slots.map((slot) => slot.construct));
  const offers = [];
  for (const construct of askableConstructs()) {
    if (visited.has(construct)) continue;
    visited.add(construct);
    const entry = state.registry.constructs.find((c) => c.id === construct);
    offers.push({
      construct,
      label: entry?.label || entry?.title || construct,
      notes: entry?.notes || '',
    });
  }
  return offers.sort((a, b) => a.label.localeCompare(b.label));
}

/*
 * Every construct a request may name, in the order the build declares them.
 *
 * The build says which map each construct is asked for through, because a construct id
 * looks identical whichever route reaches it and a request naming one in the wrong map is
 * refused whole. The three the request names by its own fields are already on the rail from
 * the first paint, so they are not here to be offered a second time.
 */
export function askableConstructs() {
  return [...state.build.derived_constructs, ...state.build.conditioning_constructs];
}

/*
 * Put a construct on the path and analyse again.
 *
 * The values behind the rule this opens on came from the registry with the reader asked
 * about none of them, so they are stamped as defaults here rather than travelling as though
 * the reader stated them.
 */
export function addToPath(construct) {
  if (!putOnThePath(construct)) return;
  renderPicker();
  renderDecisions();
  runAnalysis();
  $('add-quantity-search').value = '';
}

/*
 * The path change on its own, without the run.
 *
 * A caller that puts a construct on the path and then states a value for the rule it will run
 * needs the two to reach the engine together. Running in between asks the rule for a number
 * before the value it needs has been stated, so a reader adding a window by selecting one would
 * meet the refusal for the window they were in the middle of stating.
 */
export function putOnThePath(construct) {
  if (state.path.includes(construct)) return false;
  state.path.push(construct);
  state.slots = buildDecisionModel(state.registry, state.build, state.path);
  initialiseMissingSelections();
  for (const slot of state.slots) {
    const selection = withSources(state.selection[slot.key]);
    if (!selection.methodId) continue;
    for (const name of Object.keys(selection.values)) selection.fromDefault.add(name);
    for (const name of Object.keys(selection.options || {})) selection.fromDefault.add(name);
  }
  return true;
}

/*
 * Take a construct back off the path, with whatever was bound to it.
 *
 * The inverse of the gesture that put it there. A construct a reader reached by selecting a
 * span on the trace has to leave when they clear that span, or a rule stays bound to values
 * describing a selection that is gone.
 */
export function removeFromPath(construct) {
  const at = state.path.indexOf(construct);
  if (at === -1) return false;
  state.path.splice(at, 1);
  delete state.selection[construct];
  state.slots = buildDecisionModel(state.registry, state.build, state.path);
  initialiseMissingSelections();
  renderPicker();
  return true;
}

export function renderPicker() {
  const block = $('add-quantity');
  const offers = offerableConstructs();
  block.hidden = offers.length === 0;
  if (block.hidden) return;

  const search = $('add-quantity-search');
  const typed = search.value.trim().toLowerCase();
  const list = $('add-quantity-list');
  list.replaceChildren();

  // Everything on offer while the box has the reader's attention, narrowed as they type. A
  // box that answered nothing until a word was typed asked a first-time reader for the
  // field's vocabulary before it would tell them what the field's vocabulary is, and nobody
  // guesses `epoch impulse`. It closes when they leave, because the list is longer than the
  // rail and the rail carries the choices behind the numbers.
  const looking = typed.length > 0 || document.activeElement === search;
  list.hidden = !looking;
  if (!looking) return;

  const matching = typed
    ? offers.filter((offer) => offer.label.toLowerCase().includes(typed))
    : offers;
  if (matching.length === 0) {
    list.append(element('li', 'add-quantity__none', 'No matches.'));
    return;
  }
  for (const offer of matching) {
    const item = element('li');
    const choose = element('button', 'add-quantity__option', offer.label);
    choose.type = 'button';
    choose.dataset.construct = offer.construct;
    if (offer.notes) choose.title = offer.notes;
    choose.addEventListener('click', () => addToPath(offer.construct));
    item.append(choose);
    list.append(item);
  }
}

export function wirePicker() {
  const search = $('add-quantity-search');
  search.addEventListener('input', renderPicker);
  search.addEventListener('focus', renderPicker);
  // Closed on the way out rather than on blur alone, so a reader tabbing from the box into
  // the list it opened does not have it shut under their hands.
  search.addEventListener('blur', () => window.setTimeout(renderPicker, 120));
  $('add-quantity-list').addEventListener('focusout', () => window.setTimeout(renderPicker, 120));
}
