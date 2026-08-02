/* Loading the module and the registry, and the opening selection for every slot. */

import init, { buildInfoJson, registryJson } from './pkg/plateforce_wasm.js';
import { $, state } from './state.js';
import { element, showStage } from './format.js';
import { buildDecisionModel, preferredCandidate, initialParameters } from './registry.js';
import { wireGlobalControls } from './import-file.js';

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

  state.slots = buildDecisionModel(state.registry, state.build.bindings);
  renderRegistryBanner();
  renderBuildInfo();
  resetSelections();
  wireGlobalControls();
  showStage('stage-empty');
}

function renderRegistryBanner() {
  if (state.build.registry_valid) return;
  const banner = $('registry-banner');
  banner.hidden = false;
  $('registry-banner-text').textContent =
    `${state.build.registry_violations.length} violations across ${state.build.registry_file_count} registry files. ` +
    'No number below carries a citation until they clear.';
  const list = $('registry-violations');
  list.replaceChildren(...state.build.registry_violations.map((line) => element('li', null, line)));
}

function renderBuildInfo() {
  const census = state.registry.census;
  const rows = [
    ['Version', state.build.version],
    ['Registry', state.build.registry_digest],
    ['Registry status', state.build.registry_valid ? 'valid' : `${state.build.registry_violations.length} violations`],
    ['Constructs', String(census.constructs)],
    ['Computation entries', String(census.computation_entries)],
    ['Protocol entries', String(census.protocol_entries)],
  ];
  const list = $('build-info');
  list.replaceChildren();
  for (const [term, definition] of rows) {
    list.append(element('dt', null, term), element('dd', null, definition));
  }
}

export function resetSelections() {
  state.selection = {};
  for (const slot of state.slots) {
    const candidate = preferredCandidate(slot);
    state.selection[slot.key] = candidate
      ? { methodId: candidate.id, ...initialParameters(candidate, slot.forcesDecision) }
      : { methodId: null, values: {}, unresolved: [] };
  }
  state.weighing = { startIndex: null };
}

export function candidateFor(slotKey, id) {
  const slot = state.slots.find((entry) => entry.key === slotKey);
  return slot?.candidates.find((candidate) => candidate.id === id) || null;
}
