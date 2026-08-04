/* A folder of trials as one table, with the record of what produced every number.
 *
 * One stage in the shipped sequence rather than a second application, and one render
 * function rather than two front doors: the rendering is an argument. */

import { element, formatNumber, secondaryDisplay } from './format.js';
import { openAccounts } from './drawer.js';

export const WITH_PROVENANCE = 'with-provenance';
export const WITHOUT_PROVENANCE = 'without-provenance';

/* The column the shorter rendering hides. It hides a column and drops no record: the chain
 * is in the result either way, and what differs is whether the table joins it into view. */
const JOINED_COLUMN = 'provenance_id';

const KEY_COLUMNS = ['trial_id', 'source_path', 'provenance_id', 'refusal_code'];

/* Which columns a rendering shows, in the order the analysis reported the quantities. */
function columnsFor(result, rendering) {
  const quantities = result.quantities ?? [];
  const keys = KEY_COLUMNS.filter(
    (name) => rendering === WITH_PROVENANCE || name !== JOINED_COLUMN,
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
  return (
    `files ${present}, ${run.files_found} carrying a declared trial suffix and ` +
    `${run.files_without_declared_suffix} not, ${run.trial_count} named, ` +
    `${run.computed_count} of ${run.trial_count} computed, ` +
    `${run.refusal_count} of ${run.trial_count} declined, ` +
    `${run.trials_excluded} of ${run.trial_count} left out by a rule`
  );
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
  const moved = now && now !== attribution.revision ? ` The plate now reads ${now}.` : '';
  return `${attribution.name}, revision ${attribution.revision}.${moved}`;
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
  const scroll = element('div', 'table-scroll');
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
      if (column === 'refusal_code' && raw) cell.className = 'failed';
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
    list.append(element('dt', null, open.construct));
    list.append(
      element(
        'dd',
        null,
        `${open.published_alternatives.length} published rules, of which ` +
          `${open.forcing_entries.length} force the choice`,
      ),
    );
    panel.append(list);
  }
  return panel;
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
export function renderBatch(container, envelope, rendering = WITH_PROVENANCE, revisionNow = null) {
  container.replaceChildren();
  const parsed = typeof envelope === 'string' ? JSON.parse(envelope) : envelope;

  if (parsed.refusal) {
    container.append(refusedRun(parsed.refusal));
    return;
  }

  const result = parsed.ok;
  const panel = element('section', 'panel panel--standalone');
  const head = element('div', 'panel__head');
  head.append(element('h2', null, 'Every trial in this folder'));
  panel.append(head);
  panel.append(element('p', 'panel__sub', coverageLine(result.run)));
  panel.append(element('p', 'panel__sub', 'Each value carries the methods that produced it.'));
  const plate = plateLine(result.run, revisionNow);
  if (plate) panel.append(element('p', 'panel__sub', plate));
  panel.append(table(result, rendering));

  const reduced = summary(result);
  if (reduced) panel.append(reduced);

  const declined = refusals(result);
  if (declined) {
    panel.append(element('h3', 'panel__lead', 'Trials that declined'));
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
  panel.append(element('h2', null, 'Reading this folder'));
  panel.append(
    element(
      'p',
      'panel__sub',
      `${trialsRead} of ${trialCount} trials read, from ${filesChosen} files chosen`,
    ),
  );
  container.append(panel);
}
