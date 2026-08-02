/* Getting a file into the tab: the drop zone, the picker, and the demo path. */

import { ForceFile } from './pkg/plateforce_wasm.js';
import { $, state } from './state.js';
import { element, showStage } from './format.js';
import { renderColumnChooser, confirmColumns, loadDemonstration } from './import-columns.js';
import { runAnalysis } from './analysis.js';

export function wireGlobalControls() {
  const dropzone = $('dropzone');
  const input = $('file-input');

  const open = () => input.click();
  $('choose-file').addEventListener('click', (event) => { event.stopPropagation(); open(); });
  // Clicking the whole zone is a convenience on top of the real buttons inside it, not
  // the only way in, so the zone itself is not a control and does not take focus.
  dropzone.addEventListener('click', (event) => {
    if (event.target.closest('button')) return;
    open();
  });
  input.addEventListener('change', () => { if (input.files[0]) readFile(input.files[0]); });

  for (const type of ['dragenter', 'dragover']) {
    dropzone.addEventListener(type, (event) => { event.preventDefault(); dropzone.classList.add('is-over'); });
  }
  for (const type of ['dragleave', 'drop']) {
    dropzone.addEventListener(type, () => dropzone.classList.remove('is-over'));
  }
  dropzone.addEventListener('drop', (event) => {
    event.preventDefault();
    const file = event.dataTransfer?.files?.[0];
    if (file) readFile(file);
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
