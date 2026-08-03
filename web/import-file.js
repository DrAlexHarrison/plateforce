/* Getting a file into the tab: the drop zone, the picker, and the demo path. */

import { ForceFile } from './pkg/plateforce_wasm.js';
import { $, state } from './state.js';
import { element, showStage } from './format.js';
import { renderColumnChooser, confirmColumns, loadDemonstration } from './import-columns.js';
import { runAnalysis } from './analysis.js';
import { endingOf, endingsChosen } from './batch-run.js';

export function wireGlobalControls() {
  const dropzone = $('dropzone');
  const input = $('file-input');
  const folder = $('folder-input');

  const open = () => input.click();
  $('choose-file').addEventListener('click', (event) => { event.stopPropagation(); open(); });
  $('choose-folder').addEventListener('click', (event) => { event.stopPropagation(); folder.click(); });
  // Clicking the whole zone is a convenience on top of the real buttons inside it, not
  // the only way in, so the zone itself is not a control and does not take focus.
  dropzone.addEventListener('click', (event) => {
    if (event.target.closest('button')) return;
    open();
  });
  input.addEventListener('change', () => receive([...input.files]));
  folder.addEventListener('change', () => receive([...folder.files]));

  for (const type of ['dragenter', 'dragover']) {
    dropzone.addEventListener(type, (event) => { event.preventDefault(); dropzone.classList.add('is-over'); });
  }
  for (const type of ['dragleave', 'drop']) {
    dropzone.addEventListener(type, () => dropzone.classList.remove('is-over'));
  }
  dropzone.addEventListener('drop', (event) => {
    event.preventDefault();
    receive([...(event.dataTransfer?.files ?? [])]);
  });

  $('load-demo').addEventListener('click', (event) => { event.stopPropagation(); loadDemonstration(); });
  $('columns-cancel').addEventListener('click', () => showStage('stage-empty'));
  $('columns-confirm').addEventListener('click', confirmColumns);
  $('change-file').addEventListener('click', () => showStage('stage-empty'));
  $('reset-markers').addEventListener('click', () => {
    state.overrides = { onset: null, takeoff: null, touchdown: null };
    runAnalysis();
  });

  $('theme-toggle').addEventListener('click', () => {
    const root = document.documentElement;
    const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    const current = root.dataset.theme === 'auto' ? (dark ? 'dark' : 'light') : root.dataset.theme;
    root.dataset.theme = current === 'dark' ? 'light' : 'dark';
    state.chart?.render();
  });

  for (const node of document.querySelectorAll('[data-close-drawer]')) {
    node.addEventListener('click', () => { $('method-drawer').hidden = true; });
  }
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') $('method-drawer').hidden = true;
  });
}

/*
 * Everything the reader handed over at once.
 *
 * One file opens as one trial. Several are a folder, and the folder's commonest name ending
 * opens declared, with its count beside it, because a folder whose files nearly all end the
 * same way has said what its trials are. Sorted by name because the engine walks a set in
 * that order, so the trial on screen is the trial the run starts from.
 */
async function receive(chosen) {
  if (chosen.length === 0) return;
  const files = [...chosen].sort((a, b) => a.name.localeCompare(b.name));
  if (files.length === 1) {
    state.run = null;
    await readFile(files[0]);
    return;
  }
  state.run = { files, endings: new Set([endingsChosen(files)[0].ending]), envelope: null };
  await openFirstDeclaredTrial();
}

/*
 * The first file the declaration names, opened so the column and the rate are stated
 * against a trace the reader can see. Re-run when the declaration changes, so the trial on
 * screen is never a file the run will not read. With nothing declared there is no trial to
 * show, and the stage keeps the one it has while the run stays unstartable.
 */
export async function openFirstDeclaredTrial() {
  const first = state.run.files.find((file) => state.run.endings.has(endingOf(file.name)));
  if (first) await readFile(first);
}

async function readFile(file) {
  try {
    const text = await file.text();
    state.file?.free?.();
    state.file = ForceFile.parse(text);
    renderColumnChooser(file.name, JSON.parse(state.file.summaryJson()));
    showStage('stage-columns');
  } catch (error) {
    showStage('stage-empty');
    reportInline(String(error.message || error));
  }
}

export function reportInline(message) {
  const zone = $('dropzone');
  let notice = zone.querySelector('.notice');
  if (!notice) {
    notice = element('div', 'notice notice--danger');
    zone.append(notice);
  }
  notice.replaceChildren(element('strong', null, 'Could not read that file'), element('p', null, message));
}
