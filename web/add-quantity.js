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
  for (const binding of state.build.bindings) {
    if (visited.has(binding.construct)) continue;
    visited.add(binding.construct);
    const entry = state.registry.constructs.find((c) => c.id === binding.construct);
    offers.push({
      construct: binding.construct,
      label: entry?.label || entry?.title || binding.construct,
      notes: entry?.notes || '',
    });
  }
  return offers.sort((a, b) => a.label.localeCompare(b.label));
}

/*
 * Put a construct on the path and analyse again.
 *
 * The values behind the rule this opens on came from the registry with the reader asked
 * about none of them, so they are stamped as defaults here rather than travelling as though
 * the reader stated them.
 */
export function addToPath(construct) {
  if (state.path.includes(construct)) return;
  state.path.push(construct);
  state.slots = buildDecisionModel(state.registry, state.build, state.path);
  initialiseMissingSelections();
  for (const slot of state.slots) {
    const selection = withSources(state.selection[slot.key]);
    if (!selection.methodId) continue;
    for (const name of Object.keys(selection.values)) selection.fromDefault.add(name);
  }
  renderPicker();
  renderDecisions();
  runAnalysis();
  $('add-quantity-search').value = '';
}

export function renderPicker() {
  const block = $('add-quantity');
  const offers = offerableConstructs();
  block.hidden = offers.length === 0;
  if (block.hidden) return;

  const search = $('add-quantity-search');
  const typed = search.value.trim().toLowerCase();
  const matching = offers.filter((offer) => offer.label.toLowerCase().includes(typed));

  const list = $('add-quantity-list');
  list.replaceChildren();
  if (matching.length === 0) {
    list.append(element('li', 'add-quantity__none', 'No quantity here goes by that name.'));
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
  $('add-quantity-search').addEventListener('input', renderPicker);
}
