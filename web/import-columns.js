/* The two things the software will not guess: which column is the vertical force, and how
 * many samples per second the file holds. */

import { LoadedTrial } from './pkg/plateforce_wasm.js';
import { $, state } from './state.js';
import { counted, element, showStage } from './format.js';
import { reportInline, clearInlineReport, openFirstDeclaredTrial } from './import-file.js';
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

/*
 * How the values sit in the file, in words rather than in the engine's token for them.
 *
 * The engine names the separator it detected, and the name is a token: "whitespace separated"
 * is not a phrase somebody opening their first force export can act on, and a file holding one
 * column is separated by nothing at all, so naming a separator for it describes something that
 * is not there. A token this has no words for prints as itself, which is where it started.
 */
const SEPARATOR_WORDS = {
  tab: 'separated by tabs',
  comma: 'separated by commas',
  semicolon: 'separated by semicolons',
  whitespace: 'separated by spaces',
};

function layoutOf(summary) {
  if (summary.column_count === 1) return 'one value per row';
  return SEPARATOR_WORDS[summary.delimiter] ?? `separated by ${summary.delimiter}`;
}

/*
 * Why the rate has to be typed, in terms of the reader's own file.
 *
 * Two states, and one sentence covering both is only true of one of them. A file whose columns
 * carry no clock has to be told so. A file with a column that reads as a clock whose steps are
 * not all the same length has to be told that instead: saying "no time column" to somebody
 * looking at a column headed Time is the sentence that makes a reader stop believing the rest
 * of the screen.
 *
 * Which of the two is the record's own field. A column merely climbing is not the question,
 * and reading it that way told a reader whose force channel happens to ramp that its steps
 * were uneven, while the record beside it said that column is evenly spaced.
 */
function whyNoRate(summary) {
  const uneven = summary.columns[summary.uneven_time_like_column];
  if (!uneven) return 'No column in this file runs as a clock. Enter the rate the plate recorded at.';
  const name = uneven.header || `Column ${uneven.index + 1}`;
  return `The steps in ${name} are not all the same length, so the rate cannot be read from ` +
    'them. Enter the rate the plate recorded at.';
}

export function renderColumnChooser(fileName, summary) {
  state.columnSummary = summary;
  state.chosenColumn = summary.suggested_force_column ?? 0;

  $('columns-lead').textContent =
    `${fileName}: ${counted(summary.row_count, 'row')}, ` +
    `${counted(summary.column_count, 'column')}, ${layoutOf(summary)}` +
    (summary.skipped_leading_lines
      ? `, ${counted(summary.skipped_leading_lines, 'header line')} skipped`
      : '') +
    (summary.ragged_rows_dropped
      ? `, ${counted(summary.ragged_rows_dropped, 'ragged row')} dropped`
      : '') +
    '.';

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
          (column.exact_zero_count ? `, ${counted(column.exact_zero_count, 'exact zero')}` : ''),
      ),
    );
    card.addEventListener('click', () => {
      state.chosenColumn = column.index;
      for (const node of grid.children) node.setAttribute('aria-checked', 'false');
      card.setAttribute('aria-checked', 'true');
      describeTheZeros(summary);
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
  $('sample-rate-hint').textContent = derived ? summary.sample_rate_source : whyNoRate(summary);
  rate.oninput = updateColumnsReady;
  describeTheZeros(summary);
  renderRunDeclaration(summary);
  updateColumnsReady();
}

/*
 * The one fact on this screen that decides the missing-value answer, said where that answer is
 * given.
 *
 * A vendor export writing 0 for a sample it never took is indistinguishable from an athlete in
 * the air, and the count that tells them apart was sitting in a card three fields away from the
 * question it settles. It states the count and stops: which of the two a zero is here is the
 * reader's to say, and a line leaning either way would be answering for them.
 */
function describeTheZeros(summary) {
  const hint = $('sentinel-hint');
  if (!hint) return;
  const column = summary.columns[state.chosenColumn];
  const name = column?.header || 'This column';
  hint.textContent = column?.exact_zero_count
    ? `${name} holds ${counted(column.exact_zero_count, 'exact zero')}.`
    : '';
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
    row.append(element('span', null,
      `${ending || 'no full stop in the name'}, ${counted(count, 'file')}`));
    list.append(row);
  }

  const separator = $('run-delimiter');
  // The reader's own file reports which of these it reads as, and the option values are
  // spelled the way the reader reports it, so the opening selection needs no second table.
  // A separator none of the options carries selects nothing at all, and a select with no
  // selection draws as an empty box rather than as the question it is asking.
  const detected = summary.column_count === 1 ? 'single' : summary.delimiter;
  separator.value = [...separator.options].some((option) => option.value === detected) ? detected : '';
  separator.onchange = updateColumnsReady;
  $('run-delimiter-hint').textContent =
    summary.column_count === 1
      ? 'One value per row.'
      : `This file reads as ${counted(summary.column_count, 'column')}, ${layoutOf(summary)}.`;
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
    // Bound before the trial it replaces is released, for the reason `readFile` parses before
    // it releases: a column or a rate this file will not take leaves the handle on the state
    // pointing at nothing, and the next trial the reader confirms is refused for it.
    const loaded = LoadedTrial.fromForceFile(state.file, state.chosenColumn, rate, $('sentinel').value);
    state.loadedTrial?.free?.();
    state.loadedTrial = loaded;
    enterWorkspace();
  } catch (error) {
    reportInline(String(error.message || error));
    showStage('stage-empty');
  }
}

export function loadDemonstration() {
  state.loadedTrial?.free?.();
  clearInlineReport();
  state.loadedTrial = LoadedTrial.demonstration();
  // The trace the interface opens with is a recording rather than a file the reader chose,
  // and a result computed from it says so rather than reporting whatever was opened before.
  state.fileName = 'demonstration';
  state.trialText = null;
  enterWorkspace();
}
