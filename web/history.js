/* Undo and Redo for choices a reader makes in the workspace. */

import { $, state } from './state.js';

function cloneSelection(selection) {
  return Object.fromEntries(Object.entries(selection).map(([key, choice]) => [key, {
    ...choice,
    values: { ...(choice.values || {}) },
    options: { ...(choice.options || {}) },
    ...(choice.unresolved && { unresolved: [...choice.unresolved] }),
    fromDefault: new Set(choice.fromDefault || []),
    recommended: new Set(choice.recommended || []),
  }]));
}

export function snapshot() {
  return {
    overrides: { ...state.overrides },
    weighing: { ...state.weighing },
    selection: cloneSelection(state.selection),
    path: [...state.path],
  };
}

function same(left, right) {
  const serial = (held) => JSON.stringify({
    overrides: held.overrides,
    weighing: held.weighing,
    path: held.path,
    selection: Object.fromEntries(Object.entries(held.selection).map(([key, choice]) => [key, {
      ...choice,
      fromDefault: [...(choice.fromDefault || [])].sort(),
      recommended: [...(choice.recommended || [])].sort(),
    }])),
  });
  return serial(left) === serial(right);
}

export function remember(before) {
  if (state.history.restoring || same(before, snapshot())) return;
  state.history.past.push(before);
  state.history.future.length = 0;
  updateHistoryControls();
}

export function clearHistory() {
  state.history = { past: [], future: [], restoring: false };
  updateHistoryControls();
}

export function canUndo() {
  return state.history.past.length > 0;
}

export function canRedo() {
  return state.history.future.length > 0;
}

export function undo() {
  if (!canUndo()) return null;
  state.history.future.push(snapshot());
  return state.history.past.pop();
}

export function redo() {
  if (!canRedo()) return null;
  state.history.past.push(snapshot());
  return state.history.future.pop();
}

export function restore(held) {
  state.history.restoring = true;
  state.overrides = { ...held.overrides };
  state.weighing = { ...held.weighing };
  state.selection = cloneSelection(held.selection);
  state.path = [...held.path];
  state.history.restoring = false;
  updateHistoryControls();
}

export function updateHistoryControls() {
  const undoButton = $('undo-edit');
  const redoButton = $('redo-edit');
  if (undoButton) undoButton.disabled = !canUndo();
  if (redoButton) redoButton.disabled = !canRedo();
}
