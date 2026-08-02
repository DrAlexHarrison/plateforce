/*
 * The entry point. Loads the WebAssembly module and hands control to the modules that hold
 * the trace, the decisions and the numbers.
 *
 * Each module below owns one stage of the minute: the trial arrives, its columns are
 * declared, the workspace draws it, the rail carries the choices, the analysis returns the
 * numbers, the drawer carries a method's paperwork, and the spread says how far the choice
 * moved the answer. `start` wires them and nothing here computes anything.
 */

import { start } from './startup.js';

start();
