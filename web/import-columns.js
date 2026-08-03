/* The two things the software will not guess: which column is the vertical force, and how
 * many samples per second the file holds. */

import { LoadedTrial } from './pkg/plateforce_wasm.js';
import { $, state } from './state.js';
import { element, showStage } from './format.js';
import { reportInline, openFirstDeclaredTrial } from './import-file.js';
import { enterWorkspace } from './workspace.js';
import { endingsChosen, declarationLine } from './batch-run.js';

function sparkline(values) {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('class', 'column-card__spark');
  svg.setAttribute('viewBox', '0 0 100 32');
  svg.setAttribute('preserveAspectRatio', 'none');
  svg.setAttribute('aria-hidden', 'true');
  const finite = values.filter(Number.isFinite);
  const low = Math.min(...finite);
  const high = Math.max(...finite);
  const span = high - low || 1;
  const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
  path.setAttribute(
    'd',
    values
      .map((value, index) => {
        const x = (index / Math.max(1, values.length - 1)) * 100;
        const y = 30 - ((value - low) / span) * 28;
        return `${index === 0 ? 'M' : 'L'}${x.toFixed(2)} ${Number.isFinite(y) ? y.toFixed(2) : 16}`;
      })
      .join(' '),
  );
  svg.append(path);
  return svg;
}

export function renderColumnChooser(fileName, summary) {
  state.columnSummary = summary;
  state.chosenColumn = summary.suggested_force_column ?? 0;

  $('columns-lead').textContent =
    `${fileName}: ${summary.row_count.toLocaleString()} rows, ${summary.column_count} columns, ${summary.delimiter} separated` +
    (summary.skipped_leading_lines ? `, ${summary.skipped_leading_lines} header lines skipped` : '') +
    (summary.ragged_rows_dropped ? `, ${summary.ragged_rows_dropped} ragged rows dropped` : '') +
    `. ${summary.suggested_force_column_reason}.`;

  const grid = $('column-grid');
  grid.replaceChildren();
  summary.columns.forEach((column) => {
    const card = element('button', 'column-card');
    card.type = 'button';
    card.setAttribute('role', 'radio');
    card.setAttribute('aria-checked', String(column.index === state.chosenColumn));

    const name = element('span', 'column-card__name');
    name.append(element('span', null, column.header || `Column ${column.index + 1}`));
    if (column.implied_sample_rate_hz) name.append(element('span', 'tag tag--advanced', 'time'));
    card.append(name);
    card.append(sparkline(column.sparkline));
    card.append(
      element(
        'span',
        'column-card__stats',
        `${column.minimum.toFixed(1)} to ${column.maximum.toFixed(1)}, SD ${column.standard_deviation.toFixed(1)}` +
          (column.exact_zero_count ? `, ${column.exact_zero_count} exact zeros` : ''),
      ),
    );
    card.addEventListener('click', () => {
      state.chosenColumn = column.index;
      for (const node of grid.children) node.setAttribute('aria-checked', 'false');
      card.setAttribute('aria-checked', 'true');
    });
    grid.append(card);
  });

  // These exports do not carry a rate, and a wrong one scales every time, every velocity
  // and every impulse. Pre-filling a plausible number here would be the exact failure the
  // registry documents, so when it cannot be derived the field starts empty and blocks.
  const rate = $('sample-rate');
  const derived = summary.suggested_sample_rate_hz;
  rate.value = derived ? String(Number(derived.toFixed(4))) : '';
  rate.placeholder = 'state the rate';
  $('sample-rate-hint').textContent = derived
    ? `${summary.sample_rate_source}. A wrong rate scales every time and every impulse.`
    : 'The file carries no time column, so the rate cannot be recovered from it. It scales every time, every velocity and every impulse, so plateforce will not guess it.';
  rate.oninput = updateColumnsReady;
  renderRunDeclaration(summary);
  updateColumnsReady();
}

/*
 * What a folder is read as, declared once for every file in it.
 *
 * The endings offered are the endings the reader's own selection carries, so the control
 * describes their folder rather than a list of formats. A single ending arrives ticked
 * because with one ending there is no alternative to tick instead.
 */
function renderRunDeclaration(summary) {
  const block = $('run-declaration');
  block.hidden = !state.run;
  $('columns-confirm').textContent = state.run ? 'Analyse the first trial' : 'Analyse this trial';
  if (!state.run) return;

  const list = $('run-suffix-list');
  list.replaceChildren();
  for (const { ending, count } of endingsChosen(state.run.files)) {
    const row = element('label', 'run-suffix');
    const tick = document.createElement('input');
    tick.type = 'checkbox';
    tick.value = ending;
    tick.checked = state.run.endings.has(ending);
    // Re-opening rather than only re-counting: the column cards above describe one file, and
    // a declaration that no longer names that file would leave them describing a trial the
    // run will not read.
    tick.onchange = () => {
      if (tick.checked) state.run.endings.add(ending);
      else state.run.endings.delete(ending);
      openFirstDeclaredTrial();
      $('run-count').textContent = declarationLine();
      updateColumnsReady();
    };
    row.append(tick);
    row.append(element('span', null, `${ending || 'no full stop in the name'}, ${count} files`));
    list.append(row);
  }

  const separator = $('run-delimiter');
  // The reader's own file reports which of these it reads as, and the option values are
  // spelled the way the reader reports it, so the opening selection needs no second table.
  separator.value = summary.column_count === 1 ? 'single' : summary.delimiter;
  separator.onchange = updateColumnsReady;
  $('run-delimiter-hint').textContent =
    summary.column_count === 1
      ? 'Each row of the trial on screen holds one value.'
      : `The trial on screen reads as ${summary.delimiter} separated across ${summary.column_count} columns.`;
  $('run-count').textContent = declarationLine();
}

function updateColumnsReady() {
  const rate = Number($('sample-rate').value);
  const runDeclared = !state.run || (state.run.endings.size > 0 && $('run-delimiter').value !== '');
  $('columns-confirm').disabled = !(rate > 0 && runDeclared);
}

export function confirmColumns() {
  const rate = Number($('sample-rate').value);
  if (!(rate > 0)) return;
  if (state.run) {
    const separator = $('run-delimiter').selectedOptions[0];
    const sentinel = $('sentinel').selectedOptions[0];
    state.run.sampleRateHz = rate;
    // The engine reads an unstated separator as a row holding one field, which is what a
    // single-column export is, and the terminal writes the same value for the same reason.
    state.run.delimiter = separator.dataset.character ?? '\u0000';
    state.run.sentinel = sentinel.dataset.number == null ? null : Number(sentinel.dataset.number);
  }
  try {
    state.loadedTrial?.free?.();
    state.loadedTrial = LoadedTrial.fromForceFile(state.file, state.chosenColumn, rate, $('sentinel').value);
    enterWorkspace();
  } catch (error) {
    reportInline(String(error.message || error));
    showStage('stage-empty');
  }
}

export function loadDemonstration() {
  state.loadedTrial?.free?.();
  state.loadedTrial = LoadedTrial.demonstration();
  enterWorkspace();
}
