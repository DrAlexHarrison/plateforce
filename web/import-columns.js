/* The two things the software will not guess: which column is the vertical force, and how
 * many samples per second the file holds. */

import { LoadedTrial } from './pkg/plateforce_wasm.js';
import { $, state } from './state.js';
import { element, showStage } from './format.js';
import { reportInline } from './import-file.js';
import { enterWorkspace } from './workspace.js';

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
  rate.addEventListener('input', updateColumnsReady);
  updateColumnsReady();
}

function updateColumnsReady() {
  const rate = Number($('sample-rate').value);
  $('columns-confirm').disabled = !(rate > 0);
}

export function confirmColumns() {
  const rate = Number($('sample-rate').value);
  if (!(rate > 0)) return;
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
