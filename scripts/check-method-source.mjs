/*
 * Every rule the page renders says who chose it, and says the right thing.
 *
 * The record has carried `method_source` on every bound row since the engine half landed, and
 * the page rendered none of it: eleven of eleven rules behind the numbers on a freshly opened
 * tab are the software's own choice, and a rule the reader picked rendered identically to one
 * nobody named. That is the founding defect at the interface layer, so it gets a guard that
 * goes red rather than a comment saying it was fixed.
 *
 * Three things are asked, and the second is what makes the first mean anything:
 *
 *   1. Every claim on screen agrees with the record's word for that same rule, paired by the
 *      id the claim carries rather than by position.
 *   2. The page is driven through the acts that change the answer, so the population holds
 *      three different sources at once. A page printing one fixed sentence passes (1) on a
 *      fresh tab, where every rule really is assumed, and fails here.
 *   3. Every word the vocabulary can record has a sentence. The words are read out of the Rust
 *      macro that declares them, so a source added there turns this red until the page words
 *      it, and the sentences below are hand-spelled rather than asked of the page, which would
 *      be comparing the render with itself.
 *
 * Usage: node scripts/check-method-source.mjs <root directory> <port>
 */

import { spawn } from 'node:child_process';
import { readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { listenForConsoleErrors } from './console-errors.mjs';
import { chromeArguments, chromeExecutable, scratchDirectory } from './browser.mjs';

const [root, port] = [process.argv[2] || 'web', Number(process.argv[3] || 8757)];
const FIXTURES = 'crates/plateforce-conformance/fixtures';
const TRIAL = 'subject01_trial2.force.txt';
const SAMPLE_RATE_HZ = 1200;
const VOCABULARY = 'crates/plateforce-core/src/provenance.rs';
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };

/*
 * What each source has to say on screen, written here rather than read from the page.
 *
 * The subject varies: a row whose control names no rule names the rule in the sentence, so
 * these are the parts that do not move. They are pairwise disjoint, which is what lets a claim
 * be checked for saying one source and not any of the others: "nobody chose" is not a
 * substring of "nobody has chosen", and neither reaches "you chose".
 */
const SAYS = {
  stated: 'Chosen by you',
  recommended: 'Recommended',
  assumed: 'Default',
  measured: 'Measured here',
  cited: 'pipeline',
  provisional: 'Not chosen',
};

/* Every source this build can record, read out of the macro that declares them beside the
 * variants, so this list cannot fall behind the vocabulary the record can carry. */
function everySourceTheRecordCanCarry() {
  const source = readFileSync(VOCABULARY, 'utf8');
  const opens = source.indexOf('pub enum ParameterSource {');
  if (opens < 0) throw new Error(`${VOCABULARY} no longer declares ParameterSource where this reads it`);
  const closes = source.indexOf('\n    }\n', opens);
  const block = source.slice(opens, closes < 0 ? undefined : closes);
  const words = [...block.matchAll(/^\s*(\w+)\s*=>\s*"([a-z_]+)"\s*,/gm)].map((match) => match[2]);
  if (!words.length) throw new Error(`no wire names read out of ${VOCABULARY}`);
  return words;
}

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

// The profile lives in memory and is removed on every exit, the check-account shape: each
// leaked /tmp profile is ~160 MB and these scripts run many times over while a guard is
// broken and put back.
const profile = scratchDirectory(`plateforce-check-method-source-${port}`);
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

// The reader's own route: a recording dropped on the drop zone and declared.
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
 * What the rail says, and what the record says, as two separately gathered lists.
 *
 * The claims come off the rendered nodes and the words come off the analysis the page is
 * holding, so a page that composed the sentence from something other than the record is caught
 * rather than agreeing with itself. Coverage is counted here too: a rule the rail names and
 * leaves unclaimed is the defect this file exists for, arriving one row at a time.
 */
/*
 * The record's word for every rule id it speaks about, spelled here rather than asked of the
 * page, so the comparison below has two sides.
 *
 * Two populations, because the record speaks about a rule two ways. A rule that ran has a row.
 * A rule that ran as another rule's named value is a value on that rule's row, and the source
 * beside that name is the record's word about it: `integration.rule.trapezoid` reaches a card
 * beside a number and never gets a row of its own.
 */
const RECORDED = `(() => {
  const word = {};
  const borrowed = {};
  for (const row of state.analysis.bound_methods) word[row.method_id] = row.method_source;
  for (const row of state.analysis.bound_methods) {
    for (const [name, value] of row.bound_parameters || []) {
      if (!(value in word) && row.parameter_sources?.[name]) borrowed[value] = row.parameter_sources[name];
    }
  }
  return { ...borrowed, ...word, __borrowed: Object.keys(borrowed) };
})()`;

const READ_THE_RAIL = `(async () => {
  const { state } = await import('./state.js');
  const said = [...document.querySelectorAll('#decision-list .rule-source')]
    .map((line) => ({ method: line.dataset.method ?? null, text: line.textContent.trim() }));
  return {
    said,
    recorded: ${RECORDED},
    // Every place the rail names a rule, against the places it claims one. A decision row names
    // its rule through a control, a row beside it names one in its own title, and both owe the
    // reader the claim.
    namingRows: document.querySelectorAll(
      '#decision-list .decision:has(select[data-construct]), #decision-list .ran-beside__row',
    ).length,
    claimingRows: document.querySelectorAll(
      '#decision-list .decision:has(select[data-construct]) > .decision__head .rule-source, #decision-list .ran-beside__row .rule-source',
    ).length,
  };
})()`;

/* Whether one claim says this source and says no other. Both halves: a sentence carrying two
 * verbs would otherwise pass on the one it was meant to carry. */
function claimAgrees(text, source) {
  const mine = SAYS[source];
  if (!mine || !text.includes(mine)) return false;
  return Object.entries(SAYS).every(([other, fragment]) => other === source || !text.includes(fragment));
}

/* Every rendered claim against the record's word for that same rule, and every rule the rail
 * names against the claims it makes. Both counts on every reading, because a page can lose
 * coverage on the second render and keep it on the first. */
function pairUp(seen, note) {
  const paired = seen.said.map((line) => ({
    ...line,
    source: seen.recorded[line.method] ?? null,
    agrees: line.method != null
      && seen.recorded[line.method] != null
      && claimAgrees(line.text, seen.recorded[line.method]),
  }));
  const wrong = paired.filter((line) => !line.agrees);
  check(`every rule the rail claims for reads as the record's word for it, ${note}`,
    paired.length > 0 && wrong.length === 0,
    `${paired.length - wrong.length} of ${paired.length} claims agree` +
      (wrong.length ? `, first wrong: ${wrong[0].method} recorded ${wrong[0].source}, screen says "${wrong[0].text}"` : ''));
  check(`every rule the rail names carries a claim about who chose it, ${note}`,
    seen.namingRows > 0 && seen.claimingRows === seen.namingRows,
    `${seen.claimingRows} of ${seen.namingRows} rows naming a rule also claim one`);
  return paired;
}

const fresh = await evaluate(READ_THE_RAIL);
pairUp(fresh, 'on a tab nobody has touched');

/*
 * The two acts that change the answer, so the population below holds three sources at once
 * rather than one repeated.
 *
 * Taking every recommendation is one act and picking a rule off a list is another, and the
 * record keeps them apart; the operators nobody can reach stay the software's own choice
 * throughout. Without this the check would read a page where every rule really is assumed, and
 * a render that printed that one sentence unconditionally would pass.
 */
await evaluate(`(() => {
  const accept = [...document.querySelectorAll('#decision-list button')]
    .find((button) => button.textContent.includes('recommended'));
  if (accept) accept.click();
})()`);
await settle("document.querySelectorAll('#decision-list .rule-source').length > 0", 'the rail after the recommendation');

const picked = await evaluate(`(() => {
  const select = [...document.querySelectorAll('#decision-list select[data-construct]')]
    .find((node) => node.options.length > 1);
  if (!select) return null;
  const wanted = [...select.options].filter((option) => option.value && !option.selected).pop();
  if (!wanted) return null;
  select.value = wanted.value;
  select.dispatchEvent(new Event('change'));
  return wanted.value;
})()`);
await settle("document.querySelectorAll('#decision-list .rule-source').length > 0", 'the rail after the pick');

const acted = await evaluate(READ_THE_RAIL);
const paired = pairUp(acted, 'after taking the recommendation and picking a rule');

const rendered = new Set(paired.filter((line) => line.agrees).map((line) => line.source));
check('the rail tells three different sources apart on one screen',
  rendered.size >= 3,
  `${rendered.size} distinct sources rendered: ${[...rendered].sort().join(', ') || 'none'}` +
    `, having picked ${picked ?? 'nothing'}`);

const wording = new Map(paired.map((line) => [line.source, line.text]));
check('two rules from different sources do not read the same',
  new Set([...wording.values()]).size === wording.size,
  [...wording].map(([source, text]) => `${source}: "${text}"`).join(' | '));

/*
 * The cards, where the number is, and the panel a card's rule opens.
 *
 * The rail is where a reader goes for the methods, and the number is where they arrive. A rule
 * beside a value states its claim in the account that value's chip carries, and the panel the
 * chip opens states it in the reader's line of sight, which is the path a touch has: nothing
 * here may rest on a pointer resting on something.
 */
const beside = await evaluate(`(async () => {
  const { state } = await import('./state.js');
  const records = [...document.querySelectorAll('#headline-metric-grid .metric-record, #metric-grid .metric-record')];
  const read = [];
  for (const record of records) {
    record.click();
    for (const row of document.querySelectorAll('#drawer-body .method-list .provenance')) {
      const line = row.querySelector('.rule-source');
      read.push({
        method: line?.dataset.method ?? row.title ?? null,
        said: line?.textContent.trim() ?? null,
      });
    }
    document.querySelector('#method-drawer [data-close-drawer]').click();
  }

  records[0]?.click();
  document.querySelector('#drawer-body .method-list .provenance')?.click();
  const line = document.querySelector('#drawer-body .rule-source');
  const panel = { method: line?.dataset.method ?? null, text: line?.textContent.trim() ?? null };
  document.querySelector('#method-drawer [data-close-drawer]').click();
  return { read, panel, recorded: ${RECORDED} };
})()`);

const chipsWrong = beside.read.filter(
  (chip) => !beside.recorded[chip.method] || !chip.said || !claimAgrees(chip.said, beside.recorded[chip.method]),
);
const borrowed = new Set(beside.recorded.__borrowed);
check('every rule beside a number carries the claim in that value’s own account',
  beside.read.length > 0 && chipsWrong.length === 0,
  `${beside.read.length - chipsWrong.length} of ${beside.read.length} rules beside a number, ` +
    `${beside.read.filter((chip) => borrowed.has(chip.method)).length} of them holding no row of ` +
    `their own and claimed from the rule that bound them` +
    (chipsWrong.length ? `, first wrong: ${chipsWrong[0].method} recorded ${beside.recorded[chipsWrong[0].method]}, account says "${chipsWrong[0].said}"` : ''));

check('the panel a rule opens from beside a number states who chose it',
  beside.panel.method != null
    && beside.recorded[beside.panel.method] != null
    && claimAgrees(beside.panel.text ?? '', beside.recorded[beside.panel.method]),
  beside.panel.method
    ? `${beside.panel.method} recorded ${beside.recorded[beside.panel.method]}, panel says "${beside.panel.text}"`
    : 'the panel claimed nothing about the rule it opened on');

/*
 * Every word the record can carry, worded.
 *
 * The words come out of the Rust macro and the sentences out of the page's one home for them,
 * so a variant added to the vocabulary leaves this red until somebody writes what it says. The
 * page falls back to printing the bare word, which is visible rather than silent, and this
 * refuses the fallback: a reader meeting `this rule: adopted` has been handed the record's
 * spelling instead of a sentence.
 */
const words = everySourceTheRecordCanCarry();
const spoken = await evaluate(`(async () => {
  const { ruleSourceText } = await import('./analysis.js');
  return ${JSON.stringify(words)}.map((word) => [word, ruleSourceText({ method_id: 'x', method_source: word })]);
})()`);
const unworded = spoken.filter(([word, text]) => !text || text === `this rule: ${word}` || !claimAgrees(text, word));
check('every source the record can carry has a sentence on this page',
  spoken.length === words.length && unworded.length === 0,
  `${spoken.length - unworded.length} of ${words.length} sources worded` +
    (unworded.length ? `, unworded: ${unworded.map(([word, text]) => `${word} -> ${text ?? 'nothing'}`).join(', ')}` : '') +
    `, read from ${VOCABULARY}`);

check('no console errors', consoleLines.length === 0, consoleLines.join(' | ') || 'none');

for (const { name, passed, read } of results) {
  process.stdout.write(`${passed ? 'pass' : 'FAIL'}  ${name}\n      ${read}\n`);
}
const failed = results.filter((result) => !result.passed).length;
process.stdout.write(`\n${results.length - failed} of ${results.length} checks passed\n`);
socket.close();
server.close();
process.exit(failed === 0 ? 0 : 1);
