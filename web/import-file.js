/* Getting a file into the tab: the drop zone, the picker, and the demo path. */

import { ForceFile } from './pkg/plateforce_wasm.js';
import { $, state } from './state.js';
import { element, setWindowTitle, showStage } from './format.js';
import { renderColumnChooser, confirmColumns, loadDemonstration } from './import-columns.js';
import { rememberTheChoices } from './startup.js';
import { runAnalysis } from './analysis.js';
import { endingOf, endingsChosen } from './batch-run.js';
import { focusableWithin, hidePanel, returnInDrawer } from './drawer.js';
import {
  resetLandmarks, undoEdit, redoEdit, wireHistoryControls,
} from './workspace.js';

const THEME_KEY = 'plateforce.theme';

/*
 * The colour the reader last chose, which they chose once and had to choose again on every
 * visit. Kept on this machine beside the plates, and never sent anywhere.
 *
 * Only a colour the reader picked is kept. Storing the resolved colour of the automatic
 * setting would freeze a reader whose system moves between light and dark at dusk into
 * whichever it was the first time they pressed anything.
 */
function restoreTheme() {
  let held = null;
  try {
    held = window.localStorage.getItem(THEME_KEY);
  } catch {
    held = null;
  }
  if (held === 'light' || held === 'dark') document.documentElement.dataset.theme = held;
  describeTheme();
}

/* The control says which colour it would switch to, because an icon alone says only that
 * something about colour will happen. */
function describeTheme() {
  const root = document.documentElement;
  const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
  const showing = root.dataset.theme === 'auto' ? (dark ? 'dark' : 'light') : root.dataset.theme;
  $('theme-toggle').setAttribute('aria-label', showing === 'dark' ? 'Switch to light colours' : 'Switch to dark colours');
}

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
  $('columns-cancel').addEventListener('click', () => { setWindowTitle(); showStage('stage-empty'); });
  $('columns-confirm').addEventListener('click', confirmColumns);
  $('change-file').addEventListener('click', () => {
    // Taken before the workspace is left, because this is the last moment the choices exist.
    rememberTheChoices();
    setWindowTitle();
    showStage('stage-empty');
  });
  $('reset-markers').addEventListener('click', resetLandmarks);
  wireHistoryControls();

  restoreTheme();
  $('theme-toggle').addEventListener('click', () => {
    const root = document.documentElement;
    const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    const current = root.dataset.theme === 'auto' ? (dark ? 'dark' : 'light') : root.dataset.theme;
    root.dataset.theme = current === 'dark' ? 'light' : 'dark';
    describeTheme();
    try {
      window.localStorage.setItem(THEME_KEY, root.dataset.theme);
    } catch {
      /* held for this tab */
    }
    state.chart?.render();
  });

  // Closed through the panel the control sits in rather than by name, so a second drawer is
  // closed by its own scrim instead of by the first one's.
  for (const node of document.querySelectorAll('[data-close-drawer]')) {
    node.addEventListener('click', () => hidePanel(node.closest('.drawer')));
  }
  $('drawer-back').addEventListener('click', returnInDrawer);
  document.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape') return;
    const drawer = [...document.querySelectorAll('.drawer')].find((node) => !node.hidden);
    if (drawer) {
      event.preventDefault();
      event.stopImmediatePropagation();
      hidePanel(drawer);
      return;
    }
    if (state.chart?.selection().regions.length) {
      event.preventDefault();
      event.stopImmediatePropagation();
      state.chart.clearSelection();
    }
  });

  document.addEventListener('keydown', (event) => {
    if (!(event.metaKey || event.ctrlKey) || event.altKey || event.key.toLowerCase() !== 'z') return;
    const field = event.target instanceof Element && event.target.closest('input, textarea, select, [contenteditable="true"]');
    if (field) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    if (event.shiftKey) redoEdit();
    else undoEdit();
  });

  // A panel declaring itself modal keeps the tab key, so the reader cannot walk out of the
  // front of it into the page it is covering while the scrim is still there.
  document.addEventListener('keydown', (event) => {
    if (event.key !== 'Tab') return;
    const drawer = [...document.querySelectorAll('.drawer')].find((node) => !node.hidden);
    if (!drawer) return;
    const inside = focusableWithin(drawer);
    if (!inside.length) return;
    const edge = event.shiftKey ? inside[0] : inside[inside.length - 1];
    if (document.activeElement !== edge && drawer.contains(document.activeElement)) return;
    event.preventDefault();
    (event.shiftKey ? inside[inside.length - 1] : inside[0]).focus();
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
    // Parsed before the file it replaces is released, so a file that will not parse leaves
    // the tab holding a handle it can still release. Released first, the handle stayed on the
    // state pointing at nothing, and the next file the reader dropped, and every file after
    // it, came back as unreadable with the engine's words for a freed pointer.
    const parsed = ForceFile.parse(text);
    state.file?.free?.();
    state.file = parsed;
    state.fileName = file.name;
    // The bytes the reader handed over, kept so one trial can leave through the same door a
    // folder leaves through rather than through a second one written for it.
    state.trialText = text;
    setWindowTitle(file.name);
    clearInlineReport();
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

/* The report is about the file that failed, so it goes when a file is read. It used to
 * outlive the file it named, and a reader who dropped a second file and came back to this
 * screen met a failure that was no longer true of anything they were holding. */
export function clearInlineReport() {
  $('dropzone').querySelector('.notice')?.remove();
}
