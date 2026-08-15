/* A folder of trials as one table, with the record of what produced every number.
 *
 * One stage in the shipped sequence rather than a second application, and one render
 * function rather than two front doors: the rendering is an argument. */

import { counted, element, formatNumber, secondaryDisplay, showStage } from './format.js';
import { constructLabel } from './registry.js';
import { openAccounts } from './drawer.js';

export const WITH_PROVENANCE = 'with-provenance';
export const WITHOUT_PROVENANCE = 'without-provenance';

/* The column the shorter rendering hides. It hides a column and drops no record: the chain
 * is in the result either way, and what differs is whether the table joins it into view. */
const JOINED_COLUMN = 'provenance_id';

const KEY_COLUMNS = ['trial_id', 'source_path', 'provenance_id', 'refusal_code'];

/* Which columns a rendering shows, in the order the analysis reported the quantities. A key
 * column no row carries is left out: a run read here walked no filesystem, so `source_path` is
 * a heading over blank cells. The relations keep the field, so a saved run carries it still. */
function columnsFor(result, rendering) {
  const quantities = result.quantities ?? [];
  const rows = result.results ?? [];
  const carried = (name) => rows.length === 0 || rows.some((row) => row[name]);
  const keys = KEY_COLUMNS.filter(
    (name) => (rendering === WITH_PROVENANCE || name !== JOINED_COLUMN) && carried(name),
  );
  return [...keys, ...quantities];
}

/* The unit comes with the result rather than being read off the column name, so no surface
 * decides a unit that no rule decided. */
function cellFor(row, column, units) {
  if (KEY_COLUMNS.includes(column)) return row[column] ?? '';
  const value = row.values?.[column];
  if (value == null) return '';
  return formatNumber(value, units[column]) ?? String(value);
}

/* A count of the user's own data, never a statement about the software. Printed rather than
 * inferred from a finished run, because a run that quietly covered six trials instead of
 * two hundred is the failure this is here to make visible. */
function coverageLine(run) {
  if (!run) return '';
  const present = run.files_found + run.files_without_declared_suffix;
  return `${counted(run.files_found, 'trial file')} of ${present}` +
    (run.files_without_declared_suffix ? ` · ${run.files_without_declared_suffix} excluded by suffix` : '') +
    (run.trials_excluded ? ` · ${run.trials_excluded} excluded by a rule` : '');
}

/* Both counts carry the denominator they were taken over, because the run counts two
 * populations and the line beneath this one counts the other. */
function resultSummary(result) {
  const run = result.run;
  if (!run) return '';
  const records = new Set((result.results ?? []).map((row) => row.provenance_id).filter(Boolean)).size;
  return `${run.computed_count} of ${counted(run.trial_count, 'trial')} analysed · ` +
    `${run.refusal_count} of ${run.trial_count} trials declined` +
    (records ? ` · ${counted(records, 'method record')}` : '');
}

/*
 * The other population the run counts, with the denominator that keeps it from reading as the
 * first one.
 *
 * A rule that declines one quantity inside a trial that produced numbers is not a trial that
 * declined, so the two counts carry different nouns and different denominators. Printed on a
 * run that declined nothing as well, because a reader who meets this line only when something
 * declined cannot tell a clean run from a line that moved.
 */
function declinedQuantities(result) {
  const rows = result.results ?? [];
  const quantities = result.quantities ?? [];
  // A refusal naming no quantity is the trial's own, and the line above has already counted it.
  const declined = (result.refusals ?? []).filter((refusal) => refusal.quantity);
  const trials = new Set(declined.map((refusal) => refusal.trial_id)).size;
  const asked = counted(quantities.length * rows.length, 'quantity', 'quantities');
  if (declined.length === 0) return `${declined.length} of ${asked} declined`;
  return `${declined.length} of ${asked} declined, on ${trials} of ${counted(rows.length, 'trial')}`;
}

/* What the table holds, because most of it is past the right edge on any screen the run is read
 * on, and a reader who cannot see that a jump height is in there reads the table as if it is
 * not. */
function tableShape(result, rendering) {
  return `${counted(columnsFor(result, rendering).length, 'column')}. Scroll the table sideways ` +
    'to read the ones past the edge.';
}

/*
 * The plate this run was told about, and what that plate reads now.
 *
 * A run keeps the answers it was given, so a plate edited afterwards leaves the table resting
 * on a revision the machine no longer holds. Both are printed rather than reconciled: hiding
 * the difference would make two results taken off two configurations read as one.
 */
function plateLine(run, revisionNow) {
  const attribution = run?.plate_profile;
  if (!attribution) return null;
  const now = revisionNow?.(attribution.name);
  if (now && now !== attribution.revision) {
    return `${attribution.name} · run ${attribution.revision} · current ${now}`;
  }
  return `${attribution.name} · revision ${attribution.revision}`;
}

/*
 * The account every number gives of itself, grouped under the trial it is about.
 *
 * A row reaches its own trial's accounts rather than the folder's, so two hundred trials
 * stay two hundred records instead of one list nobody can index. The rows arrive on the
 * result in the order the analysis reported the quantities.
 */
function accountsByTrial(result) {
  const grouped = new Map();
  for (const row of result.descriptions ?? []) {
    if (!grouped.has(row.trial_id)) grouped.set(row.trial_id, []);
    grouped.get(row.trial_id).push([row.quantity, row.account]);
  }
  return grouped;
}

function table(result, rendering) {
  const columns = columnsFor(result, rendering);
  const accounts = accountsByTrial(result);
  const units = result.units ?? {};
  const scroll = element('div', 'table-scroll batch-table');
  const node = element('table', 'data');

  const head = element('thead');
  const headRow = element('tr');
  for (const column of columns) headRow.append(element('th', null, column));
  head.append(headRow);
  node.append(head);

  const body = element('tbody');
  for (const row of result.results ?? []) {
    const line = element('tr');
    for (const column of columns) {
      const raw = cellFor(row, column, units);
      const numeric = !KEY_COLUMNS.includes(column);
      const cell = element('td', numeric ? 'numeric' : null, raw === '' ? '' : String(raw));
      // The failure colour is the trial that produced nothing, which is the row whose
      // `provenance_id` is empty. A row that answered nine quantities and declined two carries
      // the codes for the two, and colouring it as a failure says the trial declined.
      if (column === 'refusal_code' && raw && !row.provenance_id) cell.className = 'failed';
      // The trial's own name opens the trial's own record, so the accounts stay reachable
      // under both renderings rather than travelling with a column one of them hides.
      const held = column === 'trial_id' ? accounts.get(row.trial_id) : null;
      if (held?.length) cell.replaceChildren(accountControl(row.trial_id, held));
      line.append(cell);
    }
    body.append(line);
  }
  node.append(body);
  scroll.append(node);
  return scroll;
}

function accountControl(trialId, accounts) {
  const open = element('button', 'row-record', trialId);
  open.type = 'button';
  open.addEventListener('click', () => openAccounts(trialId, accounts));
  return open;
}

/* The reduction renders beneath the table, from `aggregates`, with the count it was taken
 * over beside it. It is a rendering of a separate relation, never a row inside the trials. */
function summary(result) {
  const rows = result.aggregates ?? [];
  if (rows.length === 0) return null;
  const units = result.units ?? {};
  const list = element('dl', 'summary');
  for (const row of rows) {
    list.append(element('dt', null, `${row.group_key} ${row.quantity}`));
    const unit = units[row.quantity];
    const shown = row.value == null ? '' : (formatNumber(row.value, unit) ?? String(row.value));
    const beside = secondaryDisplay({ value: row.value, unit });
    const rule = row.method_id ? ` under ${row.method_id}` : '';
    const spread =
      row.dispersion == null ? '' : `, sd ${formatNumber(row.dispersion, unit) ?? row.dispersion}`;
    list.append(
      element('dd', null, `${shown}${beside ? ` (${beside})` : ''}${spread}, n = ${row.n}${rule}`),
    );
  }
  return list;
}

/* Every trial that declined, by name, with the rule and the parameter that declined it. A
 * trial that produced some numbers and declined one landmark appears in both places. */
function refusals(result) {
  const rows = result.refusals ?? [];
  if (rows.length === 0) return null;
  const scroll = element('div', 'table-scroll');
  const node = element('table', 'data');
  const head = element('thead');
  const headRow = element('tr');
  for (const column of ['trial_id', 'code', 'method_id', 'parameter', 'message']) {
    headRow.append(element('th', null, column));
  }
  head.append(headRow);
  node.append(head);

  const body = element('tbody');
  for (const row of rows) {
    const line = element('tr');
    line.append(element('td', null, row.trial_id));
    line.append(element('td', 'failed', row.code));
    line.append(element('td', null, row.method_id));
    line.append(element('td', null, row.parameter));
    line.append(element('td', 'failed', row.message));
    body.append(line);
  }
  node.append(body);
  scroll.append(node);
  return scroll;
}

/* A run that declined before reading a trial names every choice that is still open, with
 * what could be bound instead, so the next act is one interaction away. */
function refusedRun(refusal) {
  const panel = element('section', 'panel panel--standalone');
  const head = element('div', 'panel__head');
  head.append(element('h2', null, 'Choices still to be made'));
  panel.append(head);
  panel.append(element('p', 'panel__sub', refusal.message));

  for (const open of refusal.unresolved ?? []) {
    const list = element('dl', 'summary');
    // The quantity in the field's own words, taken off the record rather than looked up a
    // second time here, so the heading and the sentence above it cannot come to name the same
    // quantity two different ways.
    list.append(element('dt', null, open.title || constructLabel(open.construct)));
    const detail = element(
      'dd',
      null,
      `${open.published_alternatives.length} published rules, of which ` +
        `${open.forcing_entries.length} force the choice`,
    );
    detail.append(openTheChoice(open.construct));
    list.append(detail);
    panel.append(list);
  }
  return panel;
}

/* The choice the run is waiting on, opened where it is made. Naming what is missing and
 * leaving the reader to go and find it is a stop with directions rather than a way on. */
function openTheChoice(construct) {
  const go = element('button', 'chip', 'Choose method');
  go.type = 'button';
  go.dataset.construct = construct;
  go.addEventListener('click', () => {
    showStage('stage-workspace');
    const select = document.querySelector(`#decision-list select[data-construct="${construct}"]`);
    select?.scrollIntoView({ block: 'center' });
    select?.focus();
  });
  return go;
}

/*
 * Render one batch envelope into a container.
 *
 * `envelope` is the string every surface returns, so the browser reads exactly what the
 * command line and a notebook read. Nothing here computes a number and nothing here decides
 * a method.
 *
 * No reliability or agreement figure renders in this stage. The entry that labels a
 * reliability interval is shown-by-default, that treatment has no component yet, and showing
 * a figure through a surface with no rule for showing it would be a choice nobody made.
 */
export function renderBatch(
  container,
  envelope,
  rendering = WITH_PROVENANCE,
  revisionNow = null,
  showDeclined = true,
) {
  container.replaceChildren();
  const parsed = typeof envelope === 'string' ? JSON.parse(envelope) : envelope;

  if (parsed.refusal) {
    container.append(refusedRun(parsed.refusal));
    return;
  }

  const result = parsed.ok;
  const panel = element('section', 'panel panel--standalone');
  const head = element('div', 'panel__head');
  head.append(element('h2', null, 'Batch results'));
  panel.append(head);
  panel.append(element('p', 'batch-summary', resultSummary(result)));
  // Beside the trial counts and above the table, because the table's `refusal_code` column is
  // where a declined quantity is met, and a reader who meets it under a trial count alone has
  // been shown a refusal and a nought over the same six rows.
  panel.append(element('p', 'batch-summary', declinedQuantities(result)));
  panel.append(element('p', 'panel__sub', coverageLine(result.run)));
  const plate = plateLine(result.run, revisionNow);
  if (plate) panel.append(element('p', 'panel__sub', plate));
  panel.append(table(result, rendering));
  panel.append(element('p', 'panel__sub', tableShape(result, rendering)));

  const reduced = summary(result);
  if (reduced) panel.append(reduced);

  const declined = refusals(result);
  if (declined && showDeclined) {
    // Quantities rather than trials, said in the heading rather than left to the columns: a
    // trial that produced numbers and declined one landmark is in this list and is not a
    // declined trial. Its count is stated once, with the counts at the top.
    panel.append(element('h3', 'panel__lead', 'Quantities that declined'));
    panel.append(declined);
  }
  container.append(panel);
}

/*
 * The progress a long run shows.
 *
 * The recompute budget after a marker drag does not apply to a walk over hundreds of files,
 * and what fills in is the count of the user's own trials rather than an indeterminate
 * spinner, so the wait says something true about their data. The coverage line below cannot
 * serve here: every count in it is taken over a finished run, so none of them moves while
 * one is still being read.
 */
export function renderProgress(container, filesChosen, trialCount, trialsRead) {
  container.replaceChildren();
  const panel = element('section', 'panel panel--standalone');
  panel.append(element('h2', null, 'Preparing batch'));
  panel.append(
    element(
      'p',
      'panel__sub',
      `${trialsRead} of ${counted(trialCount, 'trial')} loaded · ${counted(filesChosen, 'file')} chosen`,
    ),
  );
  container.append(panel);
}

export function renderAnalysisProgress(container, trialCount) {
  container.replaceChildren();
  const panel = element('section', 'panel panel--standalone');
  panel.append(element('h2', null, 'Analysing batch'));
  panel.append(element('p', 'panel__sub', counted(trialCount, 'trial')));
  container.append(panel);
}
