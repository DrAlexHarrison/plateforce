/*
 * Every number in the results panel opens the account it gives of itself.
 *
 * The document has carried one account per quantity on every surface since the accounts
 * moved beside the chains, and a page can hand that document to a reader while rendering
 * none of it: `check-minute.mjs` drives this same panel and passes either way, because it
 * asserts the value, the rules beside it and the five states it can be in, and an account
 * that never reaches the screen is none of those.
 *
 * Read out of the rendered document rather than off the record behind it, and compared with
 * the string the engine wrote for that same quantity, so a panel that composed a sentence of
 * its own, truncated one, or showed one number's account under another's is caught. That the
 * string itself agrees with the other three surfaces is `check-batch.mjs`'s question, asked
 * against a folder run through the terminal.
 *
 * Usage: node scripts/check-account.mjs <root directory> <port>
 */

import { spawn } from 'node:child_process';
import { rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';

const [root, port] = [process.argv[2] || 'web', Number(process.argv[3] || 8755)];
const FIXTURES = 'crates/plateforce-conformance/fixtures';
// A recording whose numbers are not all there, so both halves of the offering have a
// population: this trial ends while the athlete is still off the plate, so the two rules
// resting on flight time report nothing and the other nine report a number.
const TRIAL = 'subject01_trial2.force.txt';
const SAMPLE_RATE_HZ = 1200;
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };

const server = createServer(async (request, response) => {
  const url = normalize(request.url === '/' ? '/index.html' : request.url).replace(/^(\.\.[/\\])+/, '');
  const path = url.startsWith('/fixtures/') ? join(FIXTURES, url.slice('/fixtures/'.length)) : join(root, url);
  try {
    const body = await readFile(path);
    response.writeHead(200, { 'content-type': TYPES[extname(path)] || 'application/octet-stream' });
    response.end(body);
  } catch {
    response.writeHead(404).end('not found');
  }
});
await new Promise((resolve) => server.listen(port, resolve));

// The profile lives in memory and is removed on every exit, the check-minute shape: each
// leaked /tmp profile is ~160 MB and these scripts run many times over while a guard is
// broken and put back.
const profile = `/dev/shm/plateforce-check-account-${port}`;
const chrome = spawn('google-chrome', [
  '--headless=new', `--remote-debugging-port=${port + 1}`, '--no-sandbox',
  '--disable-gpu', `--user-data-dir=${profile}`, 'about:blank',
], { stdio: 'ignore', detached: true });
process.on('exit', () => {
  try { process.kill(-chrome.pid, 'SIGKILL'); } catch { /* already gone */ }
  try { rmSync(profile, { recursive: true, force: true }); } catch { /* already gone */ }
});

const targets = await (async () => {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try {
      return await (await fetch(`http://127.0.0.1:${port + 1}/json/list`)).json();
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
  }
  throw new Error('chrome did not open a debugging port');
})();

const socket = new WebSocket(targets.find((t) => t.type === 'page').webSocketDebuggerUrl);
await new Promise((resolve) => socket.addEventListener('open', resolve));

let nextId = 0;
const pending = new Map();
const consoleLines = [];
socket.addEventListener('message', (event) => {
  const message = JSON.parse(event.data);
  if (pending.has(message.id)) {
    pending.get(message.id)(message);
    pending.delete(message.id);
  }
  if (message.method === 'Log.entryAdded' && message.params.entry.level === 'error') {
    consoleLines.push(message.params.entry.text);
  }
});
const send = (method, params = {}) =>
  new Promise((resolve) => {
    const id = (nextId += 1);
    pending.set(id, resolve);
    socket.send(JSON.stringify({ id, method, params }));
  });
const evaluate = async (expression) => {
  const reply = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (reply.result?.exceptionDetails) throw new Error(JSON.stringify(reply.result.exceptionDetails));
  return reply.result.result.value;
};

const settle = async (expression, label) => {
  let lastRaise = null;
  for (let attempt = 0; attempt < 80; attempt += 1) {
    try {
      if (await evaluate(expression)) return;
    } catch (raised) {
      lastRaise = raised;
    }
    await new Promise((resolve) => setTimeout(resolve, 125));
  }
  throw new Error(`timed out waiting for ${label}${lastRaise ? `, last raise: ${lastRaise.message}` : ''}`);
};

const results = [];
const check = (name, passed, read) => results.push({ name, passed, read });

await send('Runtime.enable');
await send('Log.enable');
await send('Page.navigate', { url: `http://127.0.0.1:${port}/index.html` });
await settle("!document.getElementById('stage-empty').hidden", 'the empty stage');

// The reader's own route: a recording dropped on the drop zone and declared, rather than the
// demonstration trial, whose eleven quantities all report a number.
await evaluate(`(async () => {
  const transfer = new DataTransfer();
  const text = await (await fetch('/fixtures/${TRIAL}')).text();
  transfer.items.add(new File([text], '${TRIAL}', { type: 'text/plain' }));
  document.getElementById('dropzone').dispatchEvent(
    new DragEvent('drop', { dataTransfer: transfer, bubbles: true, cancelable: true }),
  );
})()`);
await settle("!document.getElementById('stage-columns').hidden", 'the columns stage');
await evaluate(`(() => {
  const rate = document.getElementById('sample-rate');
  rate.value = '${SAMPLE_RATE_HZ}';
  rate.dispatchEvent(new Event('input'));
  document.getElementById('columns-confirm').click();
})()`);
await settle("!document.getElementById('stage-workspace').hidden", 'the workspace');
await settle("document.querySelectorAll('#metric-grid .metric').length > 0", 'the results panel');

/*
 * Every card, walked in the order the analysis reported its quantities, opening whatever the
 * card offers and reading it back.
 *
 * Paired by position rather than by matching a label against a key, and the pairing is
 * asserted rather than assumed: a page drawing the cards in another order would otherwise be
 * compared against the wrong quantity's account and the mismatch would read as a rendering
 * fault.
 */
const painted = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const metrics = state.analysis.metrics;
  const cards = [...document.querySelectorAll('#metric-grid .metric')];
  const read = cards.map((card, index) => {
    const metric = metrics[index];
    const label = card.querySelector('.metric__label').textContent;
    const control = [...card.querySelectorAll('.metric__provenance .chip')][0] ?? null;
    const entry = {
      label,
      key: metric ? metric.key : null,
      paired: Boolean(metric) && label === metric.label,
      valued: Boolean(card.querySelector('.metric__value')) && !card.querySelector('.metric__value--absent'),
      offered: Boolean(control),
      wording: control ? control.textContent.trim() : null,
      written: metric ? state.analysis.descriptions[metric.key] ?? null : null,
      // Off the rendered row rather than off the record the row was drawn from, so the two
      // sides of the comparison below are two things a reader meets. Both read from the
      // record would agree by construction whatever the page drew. The id leads the title
      // the row carries, ahead of the values the rule was bound to.
      rules: [...card.querySelectorAll('.metric__provenance .provenance')]
        .map((rule) => rule.title.split(' | ')[0]),
      title: null,
      shown: null,
      blocks: 0,
    };
    if (control) {
      control.click();
      entry.title = document.getElementById('drawer-title').textContent;
      entry.blocks = document.querySelectorAll('#drawer-body pre.account').length;
      entry.shown = document.querySelector('#drawer-body pre.account')?.textContent ?? null;
      document.querySelector('#method-drawer [data-close-drawer]').click();
    }
    return entry;
  });
  return { read, cards: cards.length, metrics: metrics.length };
})()`);

const cards = painted.read;
const valued = cards.filter((card) => card.valued);
const unvalued = cards.filter((card) => !card.valued);
const opened = cards.filter((card) => card.offered);

check('every card carries the quantity the analysis reported in that position',
  painted.cards === painted.metrics && cards.every((card) => card.paired),
  `${cards.filter((card) => card.paired).length} of ${painted.cards} cards paired, against ${painted.metrics} quantities`);

// Both halves against their own denominator. A value with no account is the state this
// exists to forbid; an account under a card showing no number would assert a measurement
// nobody made, which is the distinction `carried_no_number` was separated out to keep.
check('every number on screen offers the account it gives of itself, and nothing else does',
  valued.length > 0 && valued.every((card) => card.offered) && unvalued.every((card) => !card.offered),
  `${valued.filter((card) => card.offered).length} of ${valued.length} values offer one, ` +
    `${unvalued.filter((card) => card.offered).length} of ${unvalued.length} cards showing no number offer one`);

check('the panel opens on the number it was opened from, and on that one alone',
  opened.length > 0 && opened.every((card) => card.title === card.label && card.blocks === 1),
  `${opened.filter((card) => card.title === card.label).length} of ${opened.length} titled with their own value, ` +
    `${opened.filter((card) => card.blocks === 1).length} carrying one account`);

const altered = opened.filter((card) => card.shown !== card.written);
check('the account on screen is the one the engine wrote, character for character',
  opened.length > 0 && altered.length === 0,
  `${opened.length - altered.length} of ${opened.length} unaltered` +
    (altered.length
      ? `, first differing at ${altered[0].key}: ${(altered[0].shown ?? 'nothing shown').slice(0, 60)}`
      : `, ${opened.reduce((total, card) => total + (card.shown ?? '').split('\n').length, 0)} lines read`));

// The account is the whole chain rather than the step that reported the number. A rule named
// beside the value and absent from the account is a reader shown two records of one figure
// that disagree about what produced it.
const namedRules = opened.reduce((total, card) => total + card.rules.length, 0);
const unnamed = opened.flatMap((card) =>
  card.rules.filter((id) => !(card.shown ?? '').includes(id)).map((id) => `${card.key} ${id}`));
check('the account names every rule the card names beside the number',
  namedRules > 0 && unnamed.length === 0,
  `${namedRules - unnamed.length} of ${namedRules} rules across ${opened.length} accounts` +
    (unnamed.length ? `, missing ${unnamed.slice(0, 3).join('; ')}` : ''));

// The one label this rendering writes, held to the words the audience uses. The gate over
// the markup cannot see it: it is built in a module and reaches the reader as a control.
check('the control is worded in the reader’s words',
  opened.length > 0 && opened.every((card) => card.wording === opened[0].wording) &&
    !/\b(onset|threshold|epoch|filter|provenance|fingerprint)\b/i.test(opened[0].wording ?? ''),
  `${opened.length} cards offering "${opened[0]?.wording ?? 'nothing'}"`);

// The narrow viewport, with the panel open, where a block of preformatted text takes the
// page sideways rather than scrolling inside its own frame.
await send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 2, mobile: true });
await new Promise((resolve) => setTimeout(resolve, 400));
const narrow = await evaluate(`(() => {
  const control = document.querySelector('#metric-grid .metric__provenance .chip');
  const box = control.getBoundingClientRect();
  control.click();
  const account = document.querySelector('#drawer-body pre.account');
  return {
    side: Math.round(Math.min(box.width, box.height)),
    overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    scrolls: account.scrollWidth > account.clientWidth,
    lines: parseFloat(getComputedStyle(account).fontSize),
  };
})()`);
await send('Emulation.clearDeviceMetricsOverride');

check('at 390 px the control clears 44 px on its short side',
  narrow.side >= 44, `${narrow.side} px`);
check('at 390 px an open account does not take the page sideways',
  narrow.overflow <= 0,
  `${narrow.overflow} px of horizontal overflow, the account ${narrow.scrolls ? 'scrolling' : 'sitting'} inside its own frame at ${narrow.lines} px`);

check('no console errors', consoleLines.length === 0, consoleLines.join(' | ') || 'none');

for (const { name, passed, read } of results) {
  process.stdout.write(`${passed ? 'pass' : 'FAIL'}  ${name}\n      ${read}\n`);
}
const failed = results.filter((result) => !result.passed).length;
process.stdout.write(`\n${results.length - failed} of ${results.length} checks passed\n`);
socket.close();
server.close();
process.exit(failed === 0 ? 0 : 1);
