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
  /* Constructs the reader has asked for, beyond the three the request names by its own
   * fields. A construct nobody asked for is not on the path, so no rule instantiates it and
   * it raises no decision. */
  path: [],
  slots: [],
  selection: {},
  weighing: { startIndex: null },
  overrides: { onset: null, takeoff: null, touchdown: null },
  /* Where the plate is, when the operator has said. Null means nobody has stated it and the
   * engine supplies standard gravity, which is the one place that constant lives. */
  gravity: null,
  /* What the reader has said about the plate the trace came off. `members` is the block's own
   * list, read from the module. `stated` is what this capture answers, `saved` is every plate
   * on this machine with the revision the engine last reported for it, and `picked` is the one
   * the chip names. A member in `stated` displaces the picked plate's answer for the run. */
  plate: { members: [], stated: {}, saved: {}, picked: null },
  file: null,
  /* What the reader called the trace they opened, carried onto every result computed from
   * it. The module is handed text and never a file, so a name it is not given is a name it
   * cannot report. */
  fileName: null,
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
