/* Loading the module and the registry, and the opening selection for every slot. */

import init, { buildInfoJson, registryJson } from './pkg/plateforce_wasm.js';
import { $, state } from './state.js';
import { element, showStage, typesetUnit } from './format.js';
import { buildDecisionModel, preferredCandidate, initialParameters } from './registry.js';
import { wireGlobalControls } from './import-file.js';
import { wireBatchControls } from './batch-run.js';
import { wirePicker } from './add-quantity.js';
import { startPlate, plateRows } from './plate.js';

/* The desktop application loads this same page, so the offer to install it is addressed to a
 * reader who has already done so, and its relative href does not reach the download page from
 * the application's own scheme. Removed rather than hidden, so nothing reaches it by keyboard
 * either. `__TAURI_INTERNALS__` is the same handle format.js asks for the window title. */
function removeInstallOfferInTheApplication() {
  if (!window.__TAURI_INTERNALS__) return;
  document.querySelector('.app-header__install')?.remove();
}

export async function start() {
  removeInstallOfferInTheApplication();
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
  return (state.analysis?.bound_globals || []).map((boundGlobal) => [
    // The record spells a name with its unit on the end, and the row already carries the unit
    // beside the value.
    readableName(boundGlobal.name, boundGlobal.unit),
    `${boundGlobal.value} ${typesetUnit(boundGlobal.unit_symbol)}, ${boundGlobal.source}`,
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
    // The number that scales every velocity, every impulse and every height, beside whose
    // answer it is. A record carrying the registry digest and not this one lets a recording
    // declared at the wrong rate leave the building looking as reproducible as a right one.
    ...(state.info ? [['Sample rate', sampleRateLine()]] : []),
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

/* The rate the analysis ran at, and whose answer it is. The value is the engine's own, read
 * off the trial rather than off the field the reader typed into, so a rate the module refused
 * can never be reported as the one that produced the numbers. */
function sampleRateLine() {
  const source = state.sampleRate?.source;
  return `${state.info.sample_rate_hz} Hz${source ? `, ${source}` : ''}`;
}

export function resetSelections() {
  state.selection = {};
  state.selectionEssentials = new Set();
  initialiseMissingSelections();
  state.weighing = { startIndex: null };
  state.windowCameFromASelection = false;
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

/*
 * The reader's choices, kept for the next trial they open in the same session.
 *
 * A student picking rules for trial 1, pressing New file and opening trial 3 had every choice
 * silently discarded, so two trials of one athlete were computed under different rules and both
 * pastes read as authoritative. The folder route already applies one path to every trial in it;
 * the one-at-a-time route was the one that diverged.
 *
 * Rules and the quantities asked for, never a placement. A landmark index and a hand-drawn
 * window are samples of one recording, and carrying them onto another would be the same defect
 * with the opposite sign.
 */
export function rememberTheChoices() {
  if (!state.analysis || !state.slots?.length) return;
  const kept = {};
  for (const slot of state.slots) {
    const selection = state.selection[slot.key];
    if (!selection?.methodId || selection.placedByHand) continue;
    kept[slot.key] = {
      methodId: selection.methodId,
      values: { ...(selection.values || {}) },
      options: { ...(selection.options || {}) },
      methodStated: selection.methodStated === true,
      methodFromRecommendation: selection.methodFromRecommendation === true,
      fromDefault: [...(selection.fromDefault || [])],
      recommended: [...(selection.recommended || [])],
    };
  }
  state.carried = {
    trialName: state.fileName,
    selection: kept,
    path: [...state.path],
    sampleRateHz: state.sampleRate?.hz ?? null,
  };
}

/*
 * Those choices, put back on a trial just opened. A slot the new trial does not raise is
 * dropped rather than forced, and the values ride with the rule that reads them so a rule the
 * reader chose does not arrive under another rule's numbers.
 */
export function applyTheCarriedChoices() {
  if (!state.carried) return [];
  const applied = [];
  for (const [key, held] of Object.entries(state.carried.selection)) {
    const slot = state.slots.find((entry) => entry.key === key);
    if (!slot || !slot.available.some((candidate) => candidate.id === held.methodId)) continue;
    state.selection[key] = {
      methodId: held.methodId,
      values: { ...held.values },
      options: { ...held.options },
      unresolved: [],
      fromDefault: new Set(held.fromDefault),
      recommended: new Set(held.recommended),
      methodFromRecommendation: held.methodFromRecommendation,
      methodStated: held.methodStated,
    };
    applied.push(slot.title);
  }
  return applied;
}

export function candidateFor(slotKey, id) {
  const slot = state.slots.find((entry) => entry.key === slotKey);
  return slot?.candidates.find((candidate) => candidate.id === id) || null;
}
