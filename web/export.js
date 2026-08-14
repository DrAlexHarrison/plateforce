/*
 * Saving a run where it can be worked on: the relation set the terminal writes beside a
 * run, downloaded as one archive.
 *
 * The bytes are the engine's. Nothing here renders a table: a CSV assembled in the tab
 * would be a second home for what a result is, and the tab's file and the terminal's file
 * would stop being the same file the first time a rule gained a parameter. This module is
 * the button, the file name, and the browser's own download. Nothing leaves the tab: the
 * archive is handed to the reader through a local object URL, never a request.
 */

import { batchArchive, batchJson } from './pkg/plateforce_wasm.js';
import { $, state } from './state.js';
import { element } from './format.js';
import { buildRequest } from './analysis.js';
import { endingOf, resolvedConstructs } from './batch-run.js';
import { statedCapture } from './plate.js';

/*
 * What the saved file is called: the run's own identity, so two exports of two runs never
 * pair a table with the other's record. A run over one trial is named by that trial, which
 * is how ten one-trial exports stay tellable apart on a desktop.
 */
function archiveName(envelope) {
  const parsed = JSON.parse(envelope);
  const results = parsed.ok?.results ?? [];
  if (results.length === 1 && results[0].trial_id) {
    return `plateforce-results-${results[0].trial_id.replace(/[^\w.-]+/g, '_')}.zip`;
  }
  const run = parsed.ok?.run ?? {};
  const identity = String(run.run_fingerprint || run.request_digest || 'run');
  return `plateforce-results-${identity.replace(/[^a-zA-Z0-9]+/g, '').slice(-12)}.zip`;
}

function saveArchive(envelope) {
  const bytes = batchArchive(envelope);
  const url = URL.createObjectURL(new Blob([bytes], { type: 'application/zip' }));
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = archiveName(envelope);
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  // Revoked on a delay rather than at once, because the fetch the click starts reads the
  // URL after this handler returns.
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

/*
 * Arms a button to download what `produceEnvelope` returns.
 *
 * The envelope is produced at the press rather than when the button was armed, so the file
 * that lands is the run on screen. A run that refused has no relations to write, and the
 * refusal's own sentence stands in for the file, in the words the record carries.
 */
export function armDownload(button, produceEnvelope) {
  const label = button.textContent;
  let restore = null;
  button.addEventListener('click', () => {
    try {
      const envelope = produceEnvelope();
      const refusal = JSON.parse(envelope).refusal;
      if (refusal) {
        button.textContent = String(refusal.message).slice(0, 80);
      } else {
        saveArchive(envelope);
        return;
      }
    } catch (raised) {
      button.textContent = String(raised?.message ?? raised).slice(0, 80);
    }
    clearTimeout(restore);
    restore = setTimeout(() => { button.textContent = label; }, 4000);
  });
}

export function wireBatchExport() {
  armDownload($('batch-download'), () => state.run.envelope);
}

/*
 * Offered exactly when the stage is showing a computed run. The envelope carries one of two
 * keys and never both, pinned by the engine's own round-trip test, so the leading bytes say
 * which without parsing a folder-sized document on every redraw.
 */
export function refreshBatchExport() {
  const envelope = state.run?.envelope;
  $('batch-download').hidden = !envelope || envelope.startsWith('{"refusal"');
}

/*
 * The trial on screen as a one-trial run, so a reader who does ten trials one at a time
 * and a reader who drops a folder end up holding the same table.
 *
 * The request is the one the workspace is holding, the format is the one the trial was
 * read under, and the run goes through the batch engine rather than a second writer. A
 * provisional workspace refuses here the way a folder does: the choices still open are
 * named, and nothing carrying them leaves the building.
 */
export function trialEnvelope() {
  const info = JSON.parse(state.loadedTrial.infoJson());
  const summary = JSON.parse(state.file.summaryJson());
  // The character the columns stage maps the reader's word to, read off its own options
  // rather than spelled a second time here. A word with no option there, which is how the
  // reader reports a file held apart by runs of blanks, travels as itself: the engine
  // takes a character, the word whitespace, or nothing for a row holding one value.
  const delimiter = summary.column_count === 1
    ? ''
    : [...$('run-delimiter').options]
        .find((option) => option.value === summary.delimiter)?.dataset.character ?? summary.delimiter;
  const sentinel = [...$('sentinel').options]
    .find((option) => option.value === info.sentinel_convention)?.dataset.number;
  const capture = statedCapture();
  const request = {
    files: [{ name: state.fileName, text: state.trialText }],
    format: {
      delimiter,
      force_column_index: info.force_column,
      sample_rate_hz: info.sample_rate_hz,
      sentinel: sentinel == null ? null : Number(sentinel),
      trial_file_suffixes: [endingOf(state.fileName)],
    },
    identity: { kind: 'file_stem' },
    analysis: buildRequest(),
    resolved: resolvedConstructs(),
    ...(capture && { capture }),
  };
  return batchJson(JSON.stringify(request));
}

/* Offered only for a trace that arrived as a file: the demonstration trial has no source
 * text, so there is no file set to write for it. */
export function trialDownloadButton() {
  if (!state.trialText || !state.fileName) return null;
  const button = element('button', 'button button--ghost button--small', 'Download results (ZIP)');
  button.type = 'button';
  armDownload(button, trialEnvelope);
  return button;
}
