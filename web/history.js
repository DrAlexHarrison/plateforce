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

/*
 * One completed action, with the reader's own words for what it was.
 *
 * `label` names the act that moved away from the state being kept, so the control offering to
 * reverse it can say which edit that is. Undo on a page holding five edits is otherwise a
 * button whose effect a reader learns by pressing it. `same` compares four named fields, so
 * the label rides alongside without entering the comparison.
 */
export function remember(before, label = null) {
  if (state.history.restoring || same(before, snapshot())) return;
  state.history.past.push({ ...before, label });
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

/* The act each control would carry out, so the two can name it before the reader presses. */
export function undoLabel() {
  return state.history.past.at(-1)?.label ?? null;
}

export function redoLabel() {
  return state.history.future.at(-1)?.label ?? null;
}

export function undo() {
  if (!canUndo()) return null;
  const held = state.history.past.pop();
  // The state being left carries the same act's name, because the act that produced it is the
  // one a Redo would carry out again.
  state.history.future.push({ ...snapshot(), label: held.label });
  return held;
}

export function redo() {
  if (!canRedo()) return null;
  const held = state.history.future.pop();
  state.history.past.push({ ...snapshot(), label: held.label });
  return held;
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

/*
 * The two controls, each naming the act it would carry out.
 *
 * The word on the button stays put, because a control whose width follows the length of the
 * last edit moves the two beside it every time a marker is dragged. The act is named where a
 * reader asks what a control does: its accessible name and its tooltip.
 */
export function updateHistoryControls() {
  const naming = [
    [$('undo-edit'), 'Undo', canUndo(), undoLabel()],
    [$('redo-edit'), 'Redo', canRedo(), redoLabel()],
  ];
  for (const [button, verb, available, label] of naming) {
    if (!button) continue;
    button.disabled = !available;
    const sentence = available && label ? `${verb} ${label}` : `${verb} the last edit on the trace`;
    button.title = sentence;
    button.setAttribute('aria-label', sentence);
  }
}
