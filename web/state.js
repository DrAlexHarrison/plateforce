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
  /* Where the plate is, when the operator has said. Null means nobody has stated it and the
   * engine supplies standard gravity, which is the one place that constant lives. */
  gravity: null,
  file: null,
  loadedTrial: null,
  envelope: null,
  analysis: null,
  chart: null,
  /* The folder the reader handed over, once they have handed one over: the files
   * themselves, the name endings they declared to be trials, and how every file in it is
   * read. Declared once for the whole run, because a run that read each file its own way
   * could produce as many conventions as it has files without saying so. */
  run: null,
  /* The panel opens on jump height because that is what the audience was set. Five of six
   * course documents ask a student to compute it two or three ways and explain why the
   * answers differ, and none of them asks that about time to takeoff. */
  spread: { quantity: 'jump_height_from_takeoff_meters', axes: new Set() },
};
