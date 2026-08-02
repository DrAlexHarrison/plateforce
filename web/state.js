/*
 * What the tab is holding right now: the registry it loaded, the trial it opened, the
 * methods the user has picked, and the answer that came back.
 *
 * Every analysis is a round trip into WebAssembly. Nothing is computed in JavaScript,
 * because a second implementation of any quantity is the failure this project documents.
 */

export const $ = (id) => document.getElementById(id);

export const state = {
  registry: null,
  build: null,
  slots: [],
  selection: {},
  weighing: { startIndex: null },
  overrides: { onset: null, takeoff: null, touchdown: null },
  file: null,
  loadedTrial: null,
  envelope: null,
  analysis: null,
  chart: null,
  spread: { quantity: 'time_to_takeoff_seconds', axes: new Set() },
};
