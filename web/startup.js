/* Loading the module and the registry, and the opening selection for every slot. */

import init, { buildInfoJson, registryJson } from './pkg/plateforce_wasm.js';
import { $, state } from './state.js';
import { element, showStage } from './format.js';
import { buildDecisionModel, preferredCandidate, initialParameters } from './registry.js';
import { wireGlobalControls } from './import-file.js';
import { wireBatchControls } from './batch-run.js';
import { wirePicker } from './add-quantity.js';
import { startPlate, plateRows } from './plate.js';

export async function start() {
  try {
    await init();
    state.build = JSON.parse(buildInfoJson());
    state.registry = JSON.parse(registryJson());
  } catch (error) {
    $('fatal-message').textContent = String(error && error.message ? error.message : error);
    showStage('stage-fatal');
    return;
  }

  state.slots = buildDecisionModel(state.registry, state.build, state.path);
  renderRegistryBanner();
  startPlate();
  renderBuildInfo();
  resetSelections();
  wireGlobalControls();
  wireBatchControls();
  wirePicker();
  showStage('stage-empty');
}

function renderRegistryBanner() {
  if (state.build.registry_valid) return;
  const banner = $('registry-banner');
  banner.hidden = false;
  $('registry-banner-text').textContent =
    `${state.build.registry_violations.length} violations across ${state.build.registry_file_count} registry files. ` +
    'Results are uncited.';
  const list = $('registry-violations');
  list.replaceChildren(...state.build.registry_violations.map((line) => element('li', null, line)));
}

/*
 * Every value the record says this analysis was bound to rather than any one rule, with the
 * claim that says whether anybody chose it.
 *
 * The engine names them and the record carries the unit beside each, so nothing here decides
 * which exist or what they are called. The value prints as the record spells it: rounding
 * gravity to three places would put 9.80665 and 9.81 on screen as the same number, and the
 * whole reason the field is offered is that they are not.
 */
function globalRows() {
  return (state.analysis?.bound_globals || []).map((global) => [
    // The record spells a name with its unit on the end, and the row already carries the unit
    // beside the value.
    readableName(global.name, global.unit),
    `${global.value} ${global.unit_symbol}, ${global.source}`,
  ]);
}

function readableName(name, unit) {
  const stem = name.endsWith(`_${unit}`) ? name.slice(0, -(unit.length + 1)) : name;
  const words = stem.replace(/_/g, ' ');
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/* What produced the numbers on screen: the build, the registry it was compiled against, and
 * what the result says the plate and the analysis were bound to. Redrawn after every
 * analysis, because those are the parts a reader changes while the tab is open. */
export function renderBuildInfo() {
  const census = state.registry.census;
  const rows = [
    ['Version', state.build.version],
    // The revision is the name a reader cites and the digest is the bytes behind it. The
    // revision lives beside the rules rather than among them, so the digest cannot stand in
    // for it, and this panel showed only the digest.
    ['Registry revision', state.build.registry_declared_version ?? 'none declared'],
    ['Registry digest', state.build.registry_digest],
    ['Registry status', state.build.registry_valid ? 'valid' : `${state.build.registry_violations.length} violations`],
    ['Constructs', String(census.constructs)],
    ['Computation entries', String(census.computation_entries)],
    ['Protocol entries', String(census.protocol_entries)],
    ...plateRows(),
    ...globalRows(),
  ];
  const list = $('build-info');
  list.replaceChildren();
  for (const [term, definition] of rows) {
    list.append(element('dt', null, term), element('dd', null, definition));
  }
}

export function resetSelections() {
  state.selection = {};
  initialiseMissingSelections();
  state.weighing = { startIndex: null };
  state.spread = { quantity: 'jump_height_from_takeoff_meters', axes: new Set() };
}

/* A slot with nothing bound to it yet opens on the registry's own first-ranked runnable
 * rule, or on nothing where the construct forces a choice. A slot the reader has already
 * bound keeps what they bound, so putting another quantity on the path does not quietly
 * undo a decision they made. */
export function initialiseMissingSelections() {
  for (const slot of state.slots) {
    if (state.selection[slot.key]) continue;
    const candidate = preferredCandidate(slot);
    state.selection[slot.key] = candidate
      ? { methodId: candidate.id, ...initialParameters(candidate, slot.forcesDecision) }
      : { methodId: null, values: {}, options: {}, unresolved: [] };
  }
}

export function candidateFor(slotKey, id) {
  const slot = state.slots.find((entry) => entry.key === slotKey);
  return slot?.candidates.find((candidate) => candidate.id === id) || null;
}
