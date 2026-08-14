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
import { listenForConsoleErrors } from './console-errors.mjs';
import { chromeArguments, chromeExecutable, scratchDirectory } from './browser.mjs';

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
const profile = scratchDirectory(`plateforce-check-account-${port}`);
const chrome = spawn(chromeExecutable(), chromeArguments(port + 1, profile), { stdio: 'ignore', detached: true });
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
const consoleLines = listenForConsoleErrors(socket);
socket.addEventListener('message', (event) => {
  const message = JSON.parse(event.data);
  if (pending.has(message.id)) {
    pending.get(message.id)(message);
    pending.delete(message.id);
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
await settle(
  "document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric').length > 0",
  'the results panel',
);

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
  const cards = [...document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric')];
  const read = cards.map((card) => {
    const label = card.querySelector('.metric__label').textContent;
    const metric = metrics.find((candidate) => candidate.label === label);
    const control = card.querySelector('.metric-record');
    const entry = {
      label,
      key: metric ? metric.key : null,
      paired: Boolean(metric),
      valued: Boolean(card.querySelector('.metric__value')) && !card.querySelector('.metric__value--absent'),
      offered: Boolean(control),
      wording: control ? control.textContent.trim() : null,
      count: control?.querySelector('.metric-record__count')?.textContent.trim() ?? null,
      written: metric ? state.analysis.descriptions[metric.key] ?? null : null,
      // Off the rendered row rather than off the record the row was drawn from, so the two
      // sides of the comparison below are two things a reader meets. Both read from the
      // record would agree by construction whatever the page drew. The id leads the title
      // the row carries, ahead of the values the rule was bound to.
      rules: [],
      title: null,
      shown: null,
      blocks: 0,
    };
    if (control) {
      control.click();
      entry.title = document.getElementById('drawer-title').textContent;
      entry.blocks = document.querySelectorAll('#drawer-body pre.account').length;
      entry.shown = document.querySelector('#drawer-body pre.account')?.textContent ?? null;
      // The first line is where the engine puts the figure: an account opens with the value
      // and its unit. So whether an account claims a measurement is readable there and nowhere
      // else, and a rule's own sentence about declining does not begin that way.
      entry.opensWithAFigure = /^\\s*-?\\d/.test((entry.shown ?? '').split('\\n')[0]);
      entry.rules = [...document.querySelectorAll('#drawer-body .method-list .provenance')]
        .map((rule) => rule.title);
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

/*
 * Both halves against their own denominator.
 *
 * A value with no account is the state this exists to forbid. The other half is the one worth
 * stating carefully: what an account under a card showing no number must not do is **assert a
 * measurement nobody made**, which is the distinction `carried_no_number` was separated out to
 * keep. It is not that such a card may carry nothing.
 *
 * So an absent quantity may carry its rule record and may carry the account its rule wrote,
 * and what it may not carry is an account opening with a value and a unit. Written this way
 * because a rule that declines is an answer: a landing rule refusing a flight time where the
 * plate never unloads has a great deal to tell the reader, and forbidding the account would
 * hide the declining rule on exactly the quantities where they most need to see it.
 */
const claimingAMeasurement = unvalued.filter((card) => card.opensWithAFigure);
check('every result offers one method record, and no absent number carries an account claiming a measurement',
  valued.length > 0
    && cards.every((card) => card.offered)
    && valued.every((card) => card.blocks === 1 && card.opensWithAFigure)
    && unvalued.every((card) => card.blocks <= 1)
    && claimingAMeasurement.length === 0,
  `${valued.filter((card) => card.offered).length} of ${valued.length} values offer one and open on their figure, ` +
    `${unvalued.filter((card) => card.offered).length} of ${unvalued.length} absent values retain their rule record, ` +
    `${unvalued.filter((card) => card.blocks === 1).length} of ${unvalued.length} of those carry their rule's own account, ` +
    `${claimingAMeasurement.length} of ${unvalued.length} claiming a measurement` +
    (claimingAMeasurement.length
      ? `: ${claimingAMeasurement[0].key} opens "${(claimingAMeasurement[0].shown ?? '').split('\n')[0]}"`
      : ''));

check('the panel opens on the number it was opened from, and on that one alone',
  opened.length > 0
    && opened.every((card) => card.title === card.label)
    && valued.every((card) => card.blocks === 1)
    && unvalued.every((card) => card.blocks <= 1)
    && claimingAMeasurement.length === 0,
  `${opened.filter((card) => card.title === card.label).length} of ${opened.length} titled with their own value, ` +
    `${valued.filter((card) => card.blocks === 1).length} of ${valued.length} carrying one account, ` +
    `${claimingAMeasurement.length} of ${unvalued.length} absent values claiming a measurement`);

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
const accounted = opened.filter((card) => card.valued);
const namedRules = accounted.reduce((total, card) => total + card.rules.length, 0);
const unnamed = accounted.flatMap((card) =>
  card.rules.filter((id) => !(card.shown ?? '').includes(id)).map((id) => `${card.key} ${id}`));
check('the account names every rule the card names beside the number',
  namedRules > 0 && unnamed.length === 0,
  `${namedRules - unnamed.length} of ${namedRules} rules across ${accounted.length} accounts` +
    (unnamed.length ? `, missing ${unnamed.slice(0, 3).join('; ')}` : ''));

// The one label this rendering writes, held to the words the audience uses. The gate over
// the markup cannot see it: it is built in a module and reaches the reader as a control.
check('the control is worded in the reader’s words',
  opened.length > 0
    && opened.every((card) => /^\d+ rules?$/.test(card.count ?? ''))
    && opened.every((card) => !/\b(provenance|fingerprint|where this came from)\b/i.test(card.wording ?? '')),
  `${opened.length} compact rule counts, first "${opened[0]?.wording ?? 'nothing'}"`);

const returnsToList = await evaluate(`(async () => {
  const control = document.querySelector('#headline-metric-grid .metric-record, #metric-grid .metric-record');
  control.click();
  const rowsBefore = document.querySelectorAll('#drawer-body .method-list .provenance').length;
  const first = document.querySelector('#drawer-body .method-list .provenance');
  const listTitle = document.getElementById('drawer-title').textContent;
  first.click();
  const detailTitle = document.getElementById('drawer-title').textContent;
  const back = document.getElementById('drawer-back');
  const offered = !back.hidden && back.textContent === 'Back';
  back.click();
  await new Promise((resolve) => setTimeout(resolve, 20));
  const rowsAfter = document.querySelectorAll('#drawer-body .method-list .provenance').length;
  const restoredTitle = document.getElementById('drawer-title').textContent;
  const focusRestored = document.activeElement === first;
  const active = document.activeElement
    ? [document.activeElement.tagName, document.activeElement.id, document.activeElement.className].join('|')
    : null;
  document.querySelector('#method-drawer [data-close-drawer]').click();
  return { rowsBefore, rowsAfter, listTitle, detailTitle, restoredTitle, offered, focusRestored, active };
})()`);
check('a rule detail offers Back to the same result rule list and restores its focus',
  returnsToList.rowsBefore > 0
    && returnsToList.rowsAfter === returnsToList.rowsBefore
    && returnsToList.restoredTitle === returnsToList.listTitle
    && returnsToList.detailTitle !== returnsToList.listTitle
    && returnsToList.offered && returnsToList.focusRestored,
  JSON.stringify(returnsToList));

// The narrow viewport, with the panel open, where a block of preformatted text takes the
// page sideways rather than scrolling inside its own frame.
await send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 2, mobile: true });
await new Promise((resolve) => setTimeout(resolve, 400));
const narrow = await evaluate(`(() => {
  const control = document.querySelector('#metric-grid .metric-record');
  if (!control) return { reached: false };
  const box = control.getBoundingClientRect();
  control.click();
  const details = document.querySelector('#drawer-body .metric-account');
  if (details) details.open = true;
  const account = document.querySelector('#drawer-body pre.account');
  const panel = document.querySelector('.drawer__panel');
  const rows = [...document.querySelectorAll('#drawer-body .method-list .provenance')];
  return {
    reached: Boolean(account),
    side: Math.round(Math.min(box.width, box.height)),
    overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    // Both, because the page cannot be taken sideways by a panel that is positioned against
    // the viewport whatever the text inside it does: a reading of the document alone passes
    // on an account that has escaped its frame. The frame's own carriage is what moves.
    carriage: account ? getComputedStyle(account).overflowX : null,
    wider: account ? account.scrollWidth > account.clientWidth : null,
    size: account ? parseFloat(getComputedStyle(account).fontSize) : null,
    panelOverflow: panel ? panel.scrollWidth - panel.clientWidth : null,
    rowsFit: rows.every((row) => row.scrollWidth <= row.clientWidth),
  };
})()`);
await send('Emulation.clearDeviceMetricsOverride');

check('at 390 px the control clears 44 px on its short side',
  narrow.reached && narrow.side >= 44,
  narrow.reached ? `${narrow.side} px` : 'no account to open at 390 px');
check('at 390 px a line too long for the panel scrolls inside the account, not across the page',
  narrow.reached
    && narrow.overflow <= 0
    && narrow.panelOverflow <= 0
    && narrow.rowsFit
    && ['auto', 'scroll'].includes(narrow.carriage),
  narrow.reached
    ? `${narrow.overflow} px page overflow, ${narrow.panelOverflow} px panel overflow, ` +
      `${narrow.rowsFit ? 'method rows fit' : 'method rows escape'}, the account carrying overflow-x ${narrow.carriage} and ` +
      `${narrow.wider ? 'wider than' : 'inside'} its frame at ${narrow.size} px`
    : 'no account to open at 390 px');

/*
 * What a reader meets above the account, which is the half of the audit they can check against
 * the picture in front of them.
 *
 * The account spells its steps as registry ids. A first-time reader holding
 * `onset.threshold.adaptive_trailing_window` and looking at a line labelled `Start of jump` has
 * an audit and no way to tie it to the trace. Every instant here is compared against the number
 * the analysis reported for it, so a panel that worked one out for itself is caught: a second
 * answer to where a landmark is is exactly the defect the ids were spelt out to prevent.
 */
const landmarkHeads = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const cards = [...document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric')];
  const read = [];
  for (const card of cards) {
    const control = card.querySelector('.metric-record');
    if (!control) continue;
    control.click();
    const body = document.getElementById('drawer-body');
    const heading = [...body.querySelectorAll('h3')].map((node) => node.textContent);
    const rows = [...body.querySelectorAll('.kv--landmarks dt')].map((term, position) => ({
      label: term.textContent,
      time: body.querySelectorAll('.kv--landmarks .landmark-time')[position].textContent,
      rule: body.querySelectorAll('.kv--landmarks .landmark-rule')[position].textContent,
    }));
    read.push({
      label: card.querySelector('.metric__label').textContent,
      leads: heading[0] ?? null,
      rows,
      // The instants the record itself reports, to compare the panel against.
      reported: Object.fromEntries(state.chart.markers
        .map((marker) => [marker.label, state.analysis[marker.key + '_index']])
        .filter(([, index]) => index != null)),
      rate: state.info.sample_rate_hz,
    });
    document.querySelector('#method-drawer [data-close-drawer]').click();
  }
  return read;
})()`);

const withLandmarks = landmarkHeads.filter((entry) => entry.rows.length > 0);
const wrongTime = withLandmarks.flatMap((entry) => entry.rows
  .filter((row) => {
    const index = entry.reported[row.label];
    return index == null || !row.time.startsWith((index / entry.rate).toFixed(4));
  })
  .map((row) => `${entry.label}/${row.label} shows ${row.time}`));
const ruleless = withLandmarks.flatMap((entry) => entry.rows.filter((row) => !row.rule.trim()));

check('a number that rests on a landmark opens with that landmark, at the instant the record reports, under its rule',
  withLandmarks.length > 0
    && withLandmarks.every((entry) => entry.leads === 'On this trace')
    && wrongTime.length === 0
    && ruleless.length === 0,
  `${withLandmarks.length} of ${landmarkHeads.length} numbers rest on a landmark, ` +
    `${withLandmarks.reduce((total, entry) => total + entry.rows.length, 0)} landmarks named, ` +
    `${wrongTime.length} at the wrong instant${wrongTime.length ? `: ${wrongTime[0]}` : ''}, ` +
    `${ruleless.length} without a rule`);

/*
 * The rules that would have produced this number instead, beside it rather than two surfaces
 * away, each carrying what it would give.
 *
 * The row for the rule that is running has to read the number on the card. A comparison whose
 * own baseline disagrees with the value it is explaining is worse than no comparison.
 */
const comparison = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  // The page's own formatter, so the figure this check expects is written by the one site that
  // writes every figure a reader sees rather than by a second rounding rule living here.
  const { formatNumber } = await import('./format.js');
  const began = performance.now();
  const cards = [...document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric')];
  const read = [];
  for (const card of cards) {
    const control = card.querySelector('.metric-record');
    if (!control) continue;
    const label = card.querySelector('.metric__label').textContent;
    const metric = state.analysis.metrics.find((entry) => entry.label === label);
    control.click();
    const rows = [...document.querySelectorAll('#drawer-body .competing__row')].map((row) => ({
      name: row.querySelector('.competing__name').textContent.trim(),
      id: row.querySelector('.competing__name').title,
      running: row.classList.contains('competing__row--running'),
      value: row.querySelector('.competing__value').textContent.trim(),
    }));
    if (rows.length) {
      read.push({
        label,
        rows,
        // One section per construct the number rests on, so the count of rules that are
        // running has to be that many rather than one.
        constructs: [...document.querySelectorAll('#drawer-body .competing__construct')]
          .map((node) => node.textContent),
        cardValue: card.querySelector('.metric__primary')?.textContent.trim() ?? null,
        expected: metric ? formatNumber(metric.value, metric.unit) : null,
        heading: [...document.querySelectorAll('#drawer-body h3')]
          .find((node) => node.textContent.startsWith('Competing rules'))?.textContent ?? null,
      });
    }
    document.querySelector('#method-drawer [data-close-drawer]').click();
  }
  return { read, milliseconds: Math.round(performance.now() - began) };
})()`);

const compared = comparison.read;
const valuedRows = compared.flatMap((entry) => entry.rows.filter((row) => /\d/.test(row.value)));
// Exactly one running rule per construct offered. Fewer means a section whose active rule is
// not among the alternatives it lists, which is a comparison with nothing to compare against.
const miscounted = compared.filter(
  (entry) => entry.rows.filter((row) => row.running).length !== entry.constructs.length,
);
// Every running row against the figure the page itself writes for that number, so a
// comparison whose own baseline disagrees with the value it is explaining is caught. Against
// the card as well as the record, because a panel agreeing with one and not the other is two
// different faults and the card is the one a reader meets.
const disagreeing = compared.filter((entry) => {
  if (entry.expected == null) return false;
  const active = entry.rows.filter((row) => row.running);
  return active.some((row) => !row.value.startsWith(entry.expected))
    || !(entry.cardValue || '').startsWith(entry.expected);
});

check('a number with competing rules shows them beside it, each with what it would give',
  compared.length > 0
    && compared.every((entry) => entry.rows.length > 1 && entry.constructs.length > 0)
    && miscounted.length === 0
    && valuedRows.length > compared.length
    && disagreeing.length === 0,
  `${compared.length} quantities carry a comparison over ` +
    `${compared.reduce((total, entry) => total + entry.constructs.length, 0)} choices, ` +
    `${compared.reduce((total, entry) => total + entry.rows.length, 0)} rules across them, ` +
    `${valuedRows.length} carrying a number, ${miscounted.length} miscounting their running rules, ` +
    `${disagreeing.length} disagreeing with their own card` +
    `${disagreeing.length ? `: ${disagreeing[0].label} card "${disagreeing[0].cardValue}" against ${disagreeing[0].expected}` : ''}` +
    `, ${comparison.milliseconds} ms to open every panel`);

/*
 * A value still resting on a choice nobody has made, and whether the reader can reach that
 * choice. The names lived in a `title` attribute, which is nothing on a touch screen and
 * nothing from a keyboard, on the one card whose number depends on the answer.
 */
const provisional = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const analysis = await import('./analysis.js');
  const cards = [...document.querySelectorAll('.metric--provisional')];
  if (!cards.length) return { present: false };
  const naming = cards.find((card) => card.querySelector('.metric__provisional-reach'));
  if (!naming) return { present: true, cards: cards.length, reachable: false };
  const reach = naming.querySelector('.metric__provisional-reach');
  const named = reach.textContent.trim();

  // Which choices that card is actually about, read off the record rather than out of the
  // sentence, so the check compares the rail against the slots and not against itself.
  const label = naming.querySelector('.metric__label').textContent;
  const metric = state.analysis.metrics.find((entry) => entry.label === label);
  const resting = state.provisional.filter((slot) =>
    metric.contributing_method_ids.includes(analysis.boundMethodId(slot.key)));

  // Which set of open choices each provisional value rests on, so a pair named twice and a
  // pair named nowhere are both caught. Two distinct sets on this trial, so a check expecting
  // one naming card would fail on a page that is doing exactly the right thing.
  const setOf = (card) => {
    const held = state.analysis.metrics.find(
      (entry) => entry.label === card.querySelector('.metric__label').textContent,
    );
    return state.provisional
      .filter((slot) => held.contributing_method_ids.includes(analysis.boundMethodId(slot.key)))
      .map((slot) => slot.key).sort().join('|');
  };
  const sets = new Set(cards.map(setOf));
  const namers = cards.filter((card) => card.querySelector('.metric__provisional-reach'));

  reach.click();
  const landed = document.activeElement;
  return {
    present: true,
    cards: cards.length,
    reachable: true,
    named,
    namesEvery: resting.every((slot) => named.includes(slot.title)),
    distinctSets: sets.size,
    namingCards: namers.length,
    // One naming per set, never two for one set.
    namedSetsAreDistinct: new Set(namers.map(setOf)).size === namers.length,
    // Every value that is not the one naming its set still says it is provisional.
    counted: cards.filter((card) => !namers.includes(card)).map((card) =>
      card.querySelector('.metric__provisional-count')?.textContent ?? null),
    landedOn: landed ? [landed.tagName, landed.dataset.construct ?? ''].join('|') : 'none',
    insideTheRail: Boolean(landed && landed.closest('#decision-list')),
    // Naming a construct id rather than the words the reader was shown would be the rail
    // speaking a different language from the card that pointed at it.
    railLabel: landed ? landed.getAttribute('aria-label') : null,
    railIsOneOfThem: resting.some((slot) =>
      (landed?.getAttribute('aria-label') ?? '').startsWith(slot.title)),
  };
})()`);

check('a provisional number names the choices it rests on and puts the keyboard on them',
  provisional.present && provisional.reachable
    && provisional.namesEvery
    && provisional.insideTheRail
    && provisional.railIsOneOfThem
    && provisional.namingCards === provisional.distinctSets
    && provisional.namedSetsAreDistinct
    && provisional.counted.length > 0
    && provisional.counted.every((said) => /^Provisional · \d+ method choices? open$/.test(said ?? '')),
  provisional.present
    ? `"${provisional.named}" lands on ${provisional.landedOn} ` +
      `(${provisional.insideTheRail ? 'inside' : 'outside'} the rail) labelled "${provisional.railLabel}"; ` +
      `${provisional.namingCards} naming cards for ${provisional.distinctSets} distinct sets of open choices, ` +
      `${provisional.counted.length} of ${provisional.cards} further values stating the count without renaming a pair`
    : 'no provisional value on this trial, so the reach could not be exercised');

check('no console errors', consoleLines.length === 0, consoleLines.join(' | ') || 'none');

for (const { name, passed, read } of results) {
  process.stdout.write(`${passed ? 'pass' : 'FAIL'}  ${name}\n      ${read}\n`);
}
const failed = results.filter((result) => !result.passed).length;
process.stdout.write(`\n${results.length - failed} of ${results.length} checks passed\n`);
socket.close();
server.close();
process.exit(failed === 0 ? 0 : 1);
