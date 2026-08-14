/*
 * Every address the page reaches for while a trial is analysed.
 *
 * The browser holds the researcher's trace in a tab, and `web/README.md` tells its reader the
 * page makes no outbound requests. This measures that by recording every request the page
 * issues, over the whole journey a person performs: load, open a trial, take a recommendation,
 * read the numbers. A request to the server this script started is the page fetching its own
 * files; any other host is the trace leaving the machine.
 *
 * Usage: node scripts/verify-browser-reaches-no-address.mjs <root directory> <port>
 *
 * Exit 0 the page reached only its own origin, 1 it reached elsewhere, 2 the run could not
 * measure. A page that never loaded issues no requests and would otherwise pass.
 */

import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { chromeArguments, chromeExecutable, scratchDirectory } from './browser.mjs';

const [root, port] = [process.argv[2], Number(process.argv[3] || 8741)];
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };

// The page's own origin, and the only authority a request may name.
const ORIGIN = `127.0.0.1:${port}`;

const cannotMeasure = (why) => {
  console.error(`the addresses this page reaches cannot be read: ${why}`);
  process.exit(2);
};

const served = [];
const server = createServer(async (request, response) => {
  const path = join(root, normalize(request.url === '/' ? '/index.html' : request.url).replace(/^(\.\.[/\\])+/, ''));
  try {
    const body = await readFile(path);
    served.push(request.url);
    response.writeHead(200, { 'content-type': TYPES[extname(path)] || 'application/octet-stream' });
    response.end(body);
  } catch {
    response.writeHead(404).end('not found');
  }
});
await new Promise((resolve) => server.listen(port, resolve));

const profile = scratchDirectory(`plateforce-outbound-check-${port}`);
const chrome = spawn(chromeExecutable(), chromeArguments(port + 1, profile), { stdio: 'ignore' });

const targets = await (async () => {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try {
      return await (await fetch(`http://127.0.0.1:${port + 1}/json/list`)).json();
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
  }
  return null;
})();
if (!targets) cannotMeasure('chrome did not open a debugging port');

const socket = new WebSocket(targets.find((t) => t.type === 'page').webSocketDebuggerUrl);
await new Promise((resolve) => socket.addEventListener('open', resolve));

let nextId = 0;
const pending = new Map();

// Every address the page named, in the order it named them. A websocket and an EventSource
// do not arrive as requestWillBeSent, so each is recorded from its own event.
const reached = [];

socket.addEventListener('message', (event) => {
  const message = JSON.parse(event.data);
  if (pending.has(message.id)) {
    pending.get(message.id)(message);
    pending.delete(message.id);
    return;
  }
  if (message.method === 'Network.requestWillBeSent') {
    reached.push({ how: message.params.type || 'request', url: message.params.request.url });
  }
  if (message.method === 'Network.webSocketCreated') {
    reached.push({ how: 'websocket', url: message.params.url });
  }
  if (message.method === 'Network.eventSourceMessageReceived') {
    reached.push({ how: 'eventsource', url: message.params.eventName });
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

await send('Network.enable');
await send('Runtime.enable');
await send('Page.navigate', { url: `http://127.0.0.1:${port}/index.html` });

const settle = async (expression, label) => {
  for (let attempt = 0; attempt < 80; attempt += 1) {
    if (await evaluate(expression).catch(() => false)) return true;
    await new Promise((resolve) => setTimeout(resolve, 125));
  }
  console.error(`timed out waiting for ${label}`);
  return false;
};

// The journey, in the order a person performs it. A trace exists in the tab only from the
// second step, so a check that stopped at page load would watch the least interesting moment.
const journey = [];
// The button exists before the module that answers it does, so waiting for the button alone
// clicks into a page that is still starting and the workspace never opens.
journey.push(await settle(
  "!!document.getElementById('load-demo') && !document.getElementById('stage-empty').hidden",
  'the opening screen',
));
if (journey[0]) {
  await evaluate("document.getElementById('load-demo').click()");
  journey.push(await settle("!document.getElementById('stage-workspace').hidden", 'the workspace'));
  await evaluate(`(() => {
    const button = document.getElementById('accept-recommended');
    if (button) button.click();
  })()`).catch(() => {});
  journey.push(await settle(
    "document.querySelectorAll('#headline-metric-grid .metric, #metric-grid .metric').length > 0",
    'the metric grids',
  ));
  journey.push(await settle(
    "!!document.querySelector('.spread-headline__figure')",
    'the spread panel',
  ));
}

// A last turn of the loop, so a request issued as the final render settles is still recorded.
await new Promise((resolve) => setTimeout(resolve, 1500));

socket.close();
chrome.kill();
server.close();

const authorityOf = (url) => {
  try {
    return new URL(url).host;
  } catch {
    return null;
  }
};

// A data: or blob: url names no host and reaches nothing. Anything with a host that is not
// this script's server left the machine, whatever it was carrying.
const offOrigin = reached.filter((r) => {
  const host = authorityOf(r.url);
  return host !== null && host !== ORIGIN;
});

const analysisRan = journey.length === 4 && journey.every(Boolean);

console.log(`the page named ${reached.length} addresses, ${served.length} files served from ${ORIGIN}`);
for (const r of reached) console.log(`  ${r.how} ${r.url.slice(0, 120)}`);

// The control. Zero outbound requests is what a page that never loaded reports, so the count
// of its own files is what tells the two apart.
if (!analysisRan || served.length === 0) {
  cannotMeasure(
    `the page did not reach the end of the journey, so no trial was held in the tab`
      + ` (${served.length} files served, stages reached ${journey.filter(Boolean).length} of 4)`,
  );
}

if (offOrigin.length) {
  const plural = offOrigin.length === 1 ? 'one address that is not its own' : `${offOrigin.length} addresses that are not its own`;
  console.error(`\nthe page reached ${plural}:`);
  for (const r of offOrigin) console.error(`  ${r.how} ${r.url}`);
  process.exit(1);
}

console.log(`\nthe trial was analysed in the tab and the page reached no address but its own`);
process.exit(0);
