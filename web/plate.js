/*
 * The plate the tab is analysing against: what was answered about it, which saved plate the
 * answers came from, and what a result says it ran under.
 *
 * Nothing here decides whether a block is complete and nothing here measures a revision. The
 * block's members come from the module's own manifest, the completeness of a result comes
 * from the record, and the revision of a saved plate is the one the engine reported for it.
 * A tab measuring its own revision would be a second implementation of the one thing that
 * tells two revisions of a plate apart.
 */

import { capabilityJson } from './pkg/plateforce_wasm.js';
import { $, state } from './state.js';
import { element, reply } from './format.js';
import { runAnalysis } from './analysis.js';
import { drawRun } from './batch-run.js';
import { GRAVITY_AXIS, publishedGravityValues } from './registry.js';

/* Saved plates live on this machine and are never sent anywhere, for the reason the trace is
 * never sent anywhere. The key carries its own version so a shape written by an older build
 * is read as the different shape it is rather than as a broken one. */
const STORAGE_KEY = 'plateforce.plates.v1';

/* Every member the acquisition block declares, in the order the fingerprint is taken over.
 * Read from the manifest rather than written here, so a member the block gains is a field on
 * this page without an edit to it. */
function readMembers() {
  const { ok } = reply(capabilityJson());
  state.plate.members = ok?.acquisition?.members ?? [];
}

function readStorage() {
  try {
    const held = JSON.parse(window.localStorage.getItem(STORAGE_KEY) || '{}');
    const saved = held.plates;
    state.plate.saved = saved && typeof saved === 'object' ? saved : {};
    state.plate.picked = state.plate.saved[held.picked] ? held.picked : null;
  } catch {
    state.plate.saved = {};
    state.plate.picked = null;
  }
}

/* Storage a browser refuses is storage this tab keeps in memory instead, so the answers still
 * reach every result the tab produces and are gone when it closes. */
function writeStorage() {
  try {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ version: 1, plates: state.plate.saved, picked: state.plate.picked }),
    );
  } catch {
    /* held for this tab */
  }
}

/* A member somebody wrote something into. Whitespace alone is nothing written, so a field
 * cleared back to blank stops travelling rather than travelling as an empty answer. */
function written(value) {
  return String(value ?? '').trim().length > 0;
}

/* How many of the block's members this map answers, against the block's own count. Both the
 * chip and the result panel ask this, of two different maps, so the two cannot come to
 * different arithmetic about one question. */
function answered(members) {
  return state.plate.members.filter((member) => written(members?.[member])).length;
}

function savedMembers() {
  return (state.plate.picked && state.plate.saved[state.plate.picked]?.members) || {};
}

/* The saved plate's answers with the ones typed on this capture laid over them, which is what
 * the chip counts and what saving writes. The engine lays the same two the same way and
 * records what each stated answer displaced. */
function effectiveMembers() {
  const merged = { ...savedMembers() };
  for (const [member, value] of Object.entries(state.plate.stated)) {
    if (written(value)) merged[member] = value;
  }
  return merged;
}

/*
 * What this tab says about the plate, or nothing where nobody has said anything.
 *
 * The saved plate travels as the members it holds rather than as a name the engine would have
 * to look up, because the engine has no store to look one up in and the revision it reports
 * has to be taken over the answers this run actually used.
 */
export function statedCapture() {
  const acquisition = {};
  for (const [member, value] of Object.entries(state.plate.stated)) {
    if (written(value)) acquisition[member] = String(value).trim();
  }
  const picked = state.plate.picked && state.plate.saved[state.plate.picked];
  if (!picked && Object.keys(acquisition).length === 0) return null;
  return {
    acquisition,
    ...(picked && { plate: { name: state.plate.picked, members: picked.members } }),
  };
}

/* The third argument `analyse` takes, and undefined where the run has nothing to say about
 * the plate. A run told nothing fingerprints as incomplete and names what would fill it,
 * which is a different thing from a run that cannot happen. */
export function captureJson() {
  const capture = statedCapture();
  return capture ? JSON.stringify(capture) : undefined;
}

/*
 * The revision the engine measured for the plate the result on screen ran under.
 *
 * Kept against the saved plate so a run recorded earlier can be held against what that plate
 * reads now. The tab never computes one: this is the engine's answer, filed under the name it
 * was asked about.
 */
export function recordAttribution() {
  const attribution = state.analysis?.plate_profile;
  const saved = attribution && state.plate.saved[attribution.name];
  if (!saved || saved.revision === attribution.revision) return;
  saved.revision = attribution.revision;
  writeStorage();
}

/* What a named plate reads now, and null where no result has been produced under it yet. */
export function revisionNow(name) {
  return state.plate.saved[name]?.revision ?? null;
}

/*
 * What the result on screen says about the plate it was computed under.
 *
 * Read off the record rather than off what the tab sent, so a reader sees what the number
 * carries. A result whose plate nobody stated says so and counts what it holds, because that
 * is exactly the result that cannot be declared to match another lab's.
 */
export function plateRows() {
  if (!state.analysis) return [];
  const attribution = state.analysis?.plate_profile;
  const block = state.analysis?.acquisition ?? {};
  const rows = [
    [
      'Plate',
      `${attribution ? attribution.name : 'no saved plate'}, ` +
        `${answered(block)} of ${state.plate.members.length} answered`,
    ],
  ];
  if (attribution) rows.push(['Plate revision', attribution.revision]);
  for (const [member, was] of Object.entries(attribution?.superseded_members ?? {})) {
    rows.push([`Replaced ${member}`, was]);
  }
  return rows;
}

/* The chip in the corner: which plate, and how much of it has been answered. */
export function renderChip() {
  $('plate-chip-name').textContent = state.plate.picked || 'Plate';
  $('plate-chip-count').textContent = `${answered(effectiveMembers())} of ${state.plate.members.length}`;
}

/* A plate written to this machine under a name. Saving over a name replaces it, which is the
 * act that leaves a result recorded earlier resting on answers this plate no longer holds. */
export function savePlate(name, members, gravity = null) {
  state.plate.saved[name] = { members, revision: null, gravity };
  writeStorage();
}

/* The run already on screen is redrawn as well as the trial, because the drawer opens over
 * whichever stage the reader is on. A folder analysed a moment ago keeps its own answers, and
 * what moves is what the plate reads now, which is the line that says the two differ. */
function pick(name) {
  state.plate.picked = name;
  $('plate-name').value = name || '';
  $('plate-save').disabled = !name;
  // A plate that carries a gravity carries it because somebody measured it where that plate
  // stands. Picking the plate is asking for its site, so the number comes with it. A plate
  // holding none leaves whatever the reader has stated alone rather than clearing it.
  const site = state.plate.saved[name]?.gravity;
  if (site != null) state.gravity = site;
  writeStorage();
  renderPlatePanel();
  renderChip();
  runAnalysis();
  drawRun();
}

function save() {
  const name = $('plate-name').value.trim();
  if (!name) return;
  savePlate(name, effectiveMembers(), state.gravity);
  state.plate.stated = {};
  pick(name);
}

function forget() {
  delete state.plate.saved[state.plate.picked];
  pick(null);
}

function renderSavedList() {
  const host = $('plate-options');
  host.replaceChildren();
  const offered = [[null, 'No plate'], ...Object.keys(state.plate.saved).sort().map((n) => [n, n])];
  for (const [name, label] of offered) {
    const option = element('button', 'column-card');
    option.type = 'button';
    option.setAttribute('role', 'radio');
    option.setAttribute('aria-checked', String(state.plate.picked === name));
    if (name) option.dataset.plate = name;
    const head = element('div', 'column-card__name');
    head.append(element('span', null, label));
    if (name) {
      head.append(
        element(
          'span',
          'column-card__stats',
          `${answered(state.plate.saved[name].members)} of ${state.plate.members.length}`,
        ),
      );
    }
    option.append(head);
    option.addEventListener('click', () => pick(name));
    host.append(option);
  }
}

/*
 * One field per member of the block, with the saved answer beside it.
 *
 * The field holds what this capture states and the hint holds what the plate already says, so
 * a reader sees which answer they are about to replace before they replace it. Every member is
 * a text field: the manifest publishes the members and not the kind each one holds, so the
 * block's own parser decides whether an answer is a number, and it says so when it is not.
 */
function renderMemberFields() {
  const host = $('plate-members');
  host.replaceChildren();
  const saved = savedMembers();
  for (const member of state.plate.members) {
    const field = element('div', 'field');
    const id = `plate-member-${member}`;
    const label = element('label', null, member);
    label.htmlFor = id;
    const input = document.createElement('input');
    input.type = 'text';
    input.id = id;
    input.autocomplete = 'off';
    input.value = state.plate.stated[member] ?? '';
    input.addEventListener('change', () => {
      state.plate.stated[member] = input.value;
      renderChip();
      runAnalysis();
      drawRun();
    });
    field.append(label, input);
    // The picker one section up says which plate is picked, so the hint carries the answer
    // rather than a name that can run to forty characters under every field.
    if (written(saved[member])) {
      field.append(element('p', 'field__hint', `Plate: ${saved[member]}`));
    }
    host.append(field);
  }
}

/*
 * The gravity every number on screen is computed against, and the field a reader states one in.
 *
 * Gravity varies by half a percent across the Earth's surface, which is fifteen times the
 * difference between the two constants the published tools argue over, and it moves five of the
 * eleven quantities the tab reports. So a reader who measured it at their own plate has to be
 * able to say so, and until one does the field stands empty: a number written in here would be
 * standard gravity's second home, and it would arrive at the engine wearing the reader's
 * signature.
 *
 * The offered values are the registry's published ones, offered rather than imposed, because a
 * measured local gravity is the answer this field exists for.
 */
function renderGravityField() {
  const input = $('gravity');
  input.value = state.gravity ?? '';
  $('gravity-unit').textContent = GRAVITY_AXIS.unit;
  $('gravity-published').replaceChildren(
    ...publishedGravityValues().map((value) => {
      const option = document.createElement('option');
      option.value = value;
      return option;
    }),
  );
  renderGravityHint();
}

/* What the numbers on screen ran against, read off the record and said whenever the field is
 * not already saying it: nobody has stated one, or what is typed was not accepted. A box
 * holding a number the results were not computed against is a confident wrong number. */
function renderGravityHint() {
  const bound = state.analysis?.bound_globals?.find((global) => global.name === GRAVITY_AXIS.parameter);
  const typed = $('gravity').value.trim();
  const fieldSaysIt = bound != null && typed !== '' && Number(typed) === bound.value;
  $('gravity-hint').textContent =
    bound && !fieldSaysIt ? `Results use ${bound.value} ${bound.unit_symbol}.` : '';
}

/* An empty field is the reader declining to state one. A value outside the range the input
 * declares is left in the box unaccepted, marked and answered by the hint, rather than a typo
 * silently becoming the gravity every result is bound to. */
function stateGravity() {
  const input = $('gravity');
  const typed = input.value.trim();
  if (typed !== '' && !input.checkValidity()) {
    input.reportValidity();
    renderGravityHint();
    return;
  }
  state.gravity = typed === '' ? null : Number(typed);
  runAnalysis();
  drawRun();
  renderGravityField();
}

export function renderPlatePanel() {
  renderSavedList();
  renderMemberFields();
  renderGravityField();
  const forgetAction = $('plate-forget');
  forgetAction.hidden = !state.plate.picked;
  if (state.plate.picked) forgetAction.textContent = `Forget ${state.plate.picked}`;
}

export function startPlate() {
  readMembers();
  readStorage();
  // The plate this machine was left on brings its site with it, the same way picking one does.
  const site = state.plate.saved[state.plate.picked]?.gravity;
  if (site != null) state.gravity = site;
  $('plate-name').value = state.plate.picked || '';
  $('plate-save').disabled = !state.plate.picked;
  $('plate-chip').addEventListener('click', () => {
    renderPlatePanel();
    $('plate-drawer').hidden = false;
  });
  $('plate-save').addEventListener('click', save);
  $('plate-forget').addEventListener('click', forget);
  $('plate-name').addEventListener('input', () => {
    $('plate-save').disabled = !$('plate-name').value.trim();
  });
  $('gravity').addEventListener('change', stateGravity);
  renderPlatePanel();
  renderChip();
}
