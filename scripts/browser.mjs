/*
 * Where a headless Chrome runs, asked of the system rather than assumed.
 *
 * Three Linux facts used to be written into each of these checks as though they were
 * universal: the memory-backed scratch directory, the browser's file name, and the size of
 * the window it opens. A product whose bar is three desktops had eleven checks that could
 * only run on one of them, so the checks agreed with the software on the machine that wrote
 * them and could say nothing anywhere else.
 */
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

/*
 * A scratch directory for one run, named by the caller so two runs never share one.
 *
 * `/dev/shm` is memory-backed, and these checks are run many times over while a guard is
 * broken and put back, so it is worth preferring where it exists. macOS has no equivalent
 * and cannot be given one: `/dev` there is devfs, which does not take a mkdir at all, so a
 * script that writes the path itself fails with EPERM rather than falling back.
 */
export function scratchDirectory(name) {
  return join(existsSync('/dev/shm') ? '/dev/shm' : tmpdir(), name);
}

/*
 * The browser to run. A stated path wins, so a machine keeping Chrome somewhere this list
 * does not know about needs no change here; after that, the name each system actually uses.
 * On macOS Chrome is an application bundle and the executable sits inside it, so there is no
 * bare command to find on the PATH.
 */
export function chromeExecutable() {
  const stated = process.env.PLATEFORCE_CHROME ?? process.env.CHROME_PATH;
  if (stated) return stated;
  if (process.platform === 'darwin') {
    return '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  }
  return 'google-chrome';
}

/*
 * The window is stated rather than inherited, because every check here reads the chart by
 * its rectangle on screen and headless Chrome does not open the same window on every system.
 * Unstated, the geometry these checks measure is a property of the machine.
 *
 * The failure that costs the most is silent: on a smaller default the drag lands outside the
 * plot, nothing throws, and the run ends having asserted nothing while reporting no failures.
 */
export const CHROME_WINDOW = '--window-size=1280,900';

/* One argument list, because twelve identical copies drift one at a time. */
export function chromeArguments(debuggingPort, profileDirectory) {
  return [
    '--headless=new', `--remote-debugging-port=${debuggingPort}`, '--no-sandbox',
    '--disable-gpu', CHROME_WINDOW, `--user-data-dir=${profileDirectory}`, 'about:blank',
  ];
}
