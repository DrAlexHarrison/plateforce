/*
 * A folder of trials, read in the tab and put through the same engine one trial goes
 * through.
 *
 * The run carries the rules the reader bound on the trace they opened, so a folder is
 * analysed under choices they watched move a number rather than under a second set nobody
 * saw. Nothing here computes a quantity, aggregates one, or decides a method.
 */

import { batchJson } from './pkg/plateforce_wasm.js';
import { $, state } from './state.js';
import { element, showStage } from './format.js';
import {
  renderBatch,
  renderProgress,
  renderAnalysisProgress,
  WITH_PROVENANCE,
  WITHOUT_PROVENANCE,
} from './batch.js';
import { buildRequest } from './analysis.js';
import { statedCapture, revisionNow } from './plate.js';

/*
 * What a file name ends with, from its first full stop.
 *
 * The engine matches a suffix against the end of the name so a compound ending like
 * `.force.txt` is expressible, and grouping from the first stop offers the reader the
 * compound rather than the last three characters it ends in.
 */
export function endingOf(fileName) {
  const stop = fileName.indexOf('.');
  return stop > 0 ? fileName.slice(stop) : '';
}

/* Every distinct ending among the files the reader chose, commonest first, each with the
 * count of files carrying it. */
export function endingsChosen(files) {
  const counts = new Map();
  for (const file of files) counts.set(endingOf(file.name), (counts.get(endingOf(file.name)) || 0) + 1);
  return [...counts.entries()]
    .map(([ending, count]) => ({ ending, count }))
    .sort((a, b) => b.count - a.count || a.ending.localeCompare(b.ending));
}

/* The files a declared ending names, and the ones it does not. Both are returned because a
 * file the reader handed over and the run did not read is a file that has to be counted. */
export function partitionByDeclaredEndings(files, endings) {
  const named = files.filter((file) => endings.has(endingOf(file.name)));
  const passedOver = files.filter((file) => !endings.has(endingOf(file.name)));
  return { named, passedOver };
}

/*
 * The constructs the reader settled by an act rather than by arriving at a default.
 *
 * The engine holds a run open until the forcing decisions on the path are made, and it
 * takes this list as the statement that they were. Sending a construct the reader never
 * touched would sign a default with their name, which is the failure the whole record
 * exists to prevent.
 */
export function resolvedConstructs() {
  return state.slots
    .filter((slot) => {
      const selection = state.selection[slot.key];
      return Boolean(selection?.methodStated || selection?.methodFromRecommendation);
    })
    .map((slot) => slot.construct);
}

function currentRendering() {
  return $('batch-provenance').checked ? WITH_PROVENANCE : WITHOUT_PROVENANCE;
}

function showDeclined() {
  return $('batch-declined').checked;
}

/* What the reader chose, stated before anything runs, against the denominator they handed
 * over. A file no declared ending names is listed by name with the ending that left it out. */
export function declarationLine() {
  const { named, passedOver } = partitionByDeclaredEndings(state.run.files, state.run.endings);
  const line = `${state.run.files.length} files chosen, ${named.length} named as trials by ${[...state.run.endings].join(' and ')}`;
  if (passedOver.length === 0) return line;
  const listed = passedOver
    .slice(0, 4)
    .map((file) => `${file.name} (${endingOf(file.name) || 'no full stop in the name'})`)
    .join(', ');
  const rest = passedOver.length > 4 ? `, and ${passedOver.length - 4} more` : '';
  return `${line}. ${passedOver.length} left out because no declared ending names them: ${listed}${rest}`;
}

/*
 * Run every named trial under the request the workspace is holding.
 *
 * Each file is read one at a time so the count on screen is a count of the reader's own
 * trials as it climbs, rather than a bar with no number behind it.
 */
export async function runFolder() {
  const host = $('batch-result');
  const { named } = partitionByDeclaredEndings(state.run.files, state.run.endings);
  showStage('stage-batch');
  $('batch-declaration').textContent = declarationLine();

  const files = [];
  renderProgress(host, state.run.files.length, named.length, 0);
  for (const file of named) {
    files.push({ name: file.name, text: await file.text() });
    renderProgress(host, state.run.files.length, named.length, files.length);
  }

  // Stated once for the run rather than per file, because a folder is one plate's recordings
  // and a trace of forces carries nothing about the plate that wrote it.
  const capture = statedCapture();
  const request = {
    files,
    format: {
      delimiter: state.run.delimiter,
      force_column_index: state.chosenColumn,
      sample_rate_hz: state.run.sampleRateHz,
      sentinel: state.run.sentinel,
      trial_file_suffixes: [...state.run.endings],
    },
    identity: { kind: 'file_stem' },
    analysis: buildRequest(),
    resolved: resolvedConstructs(),
    ...(capture && { capture }),
  };

  try {
    renderAnalysisProgress(host, named.length);
    await new Promise((resolve) => requestAnimationFrame(() => resolve()));
    state.run.envelope = batchJson(JSON.stringify(request));
  } catch (error) {
    const panel = element('section', 'panel panel--standalone');
    panel.append(element('h2', null, 'This folder could not be read'));
    panel.append(element('p', 'panel__sub', String(error.message || error)));
    panel.append(element('p', 'panel__sub', declarationLine()));
    host.replaceChildren(panel);
    return;
  }
  drawRun();
}

/* The run already computed, drawn again under whichever rendering is selected. Re-rendering
 * reads the envelope the run returned rather than running it a second time, so the two
 * renderings cannot be two answers.
 *
 * `revisionNow` travels rather than a revision, because the table is redrawn long after the
 * run and the plate behind it may have moved since. */
export function drawRun() {
  if (!state.run?.envelope) return;
  renderBatch(
    $('batch-result'),
    state.run.envelope,
    currentRendering(),
    revisionNow,
    showDeclined(),
  );
}

export function wireBatchControls() {
  $('batch-back').addEventListener('click', () => showStage('stage-workspace'));
  $('batch-provenance').addEventListener('change', drawRun);
  $('batch-declined').addEventListener('change', drawRun);
  $('run-folder').addEventListener('click', runFolder);
}
