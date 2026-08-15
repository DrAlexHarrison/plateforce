/*
 * The download page offers a file, a version and a size, and this follows all three.
 *
 * A page promising a 6.9 MB download that 404s is worse than the release page it replaces, so
 * the claim is not read off the source: the page is loaded, the button is read, the address it
 * carries is fetched, and the bytes that come back are compared with the megabytes printed
 * under it. The three machines it can name are each asked for, and so are the two it cannot,
 * because the answer for an unrecognised machine is the part with no user to notice it broken.
 *
 * Exit 1 is a claim that did not hold. Exit 3 is this being unable to look, which is reported
 * apart from a failure because a run that could not measure is not a run that found nothing.
 *
 * Usage: node scripts/check-download-page.mjs <assembled site directory> <port>
 */

import { spawn } from 'node:child_process';
import { rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { chromeArguments, chromeExecutable, scratchDirectory } from './browser.mjs';

const [root, port] = [process.argv[2] || 'site', Number(process.argv[3] || 8751)];
const LATEST = 'https://api.github.com/repos/DrAlexHarrison/plateforce/releases/latest';
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };

const failures = [];
const held = [];
const record = (ok, what) => (ok ? held : failures).push(what);

/* The releases API answers 60 times an hour to an address that has not identified itself, and
 * a shared runner spends that on everybody at once. A page this cannot read is a page this
 * cannot judge, so it says so rather than reporting the reader's link as broken. */
const answer = await fetch(LATEST, { headers: { Accept: 'application/vnd.github+json' } });
if (answer.status === 403 || answer.status === 429) {
  console.error('the releases API is rate limiting this address, so the offer was not followed');
  process.exit(3);
}
if (!answer.ok) {
  console.error(`the releases API answered ${answer.status}, so the offer was not followed`);
  process.exit(3);
}
const release = await answer.json();

const server = createServer(async (request, response) => {
  const asked = normalize(request.url.split('?')[0].split('#')[0]).replace(/^(\.\.[/\\])+/, '');
  const path = join(root, asked.endsWith('/') ? `${asked}index.html` : asked === '/' ? '/index.html' : asked);
  try {
    const body = await readFile(path);
    response.writeHead(200, { 'content-type': TYPES[extname(path)] || 'application/octet-stream' });
    response.end(body);
  } catch {
    response.writeHead(404).end('not found');
  }
});
/* A port somebody else is holding is this being unable to look, and an unhandled listen error
 * leaves a stack trace that reads like the page is broken. */
server.on('error', (problem) => {
  console.error(`port ${port} could not be served: ${problem.code}`);
  process.exit(3);
});
await new Promise((resolve) => server.listen(port, resolve));

const profile = scratchDirectory(`plateforce-download-check-${port}`);
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
  console.error('chrome did not open a debugging port, so the page was not read');
  process.exit(3);
})();

const socket = new WebSocket(targets.find((t) => t.type === 'page').webSocketDebuggerUrl);
await new Promise((resolve) => socket.addEventListener('open', resolve));

let nextId = 0;
const pending = new Map();
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

await send('Runtime.enable');
await send('Network.enable');
/* A headless document is never the focused one, and the clipboard refuses an unfocused page.
 * Without this the working path takes the refusal branch and reads as a pass of the wrong half. */
await send('Emulation.setFocusEmulationEnabled', { enabled: true });

const AGENTS = {
  mac: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36',
  windows: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36',
  linux: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36',
  android: 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36',
  nonsense: 'Mozilla/5.0 (Amiga; U; AmigaOS 1.3; en; rv:1.8.1.19) Gecko/20081204 SeaMonkey/1.1.14',
};

/* The client-hint platform is what the page asks first, so an override that moves only the
 * user-agent string would leave the real machine answering underneath it and every reading
 * below would be about this computer rather than about the one being named. */
const platformOf = { mac: 'macOS', windows: 'Windows', linux: 'Linux', android: 'Android', nonsense: '' };

async function load(agent, width, { blockTheApi = false } = {}) {
  await send('Emulation.setUserAgentOverride', {
    userAgent: AGENTS[agent],
    userAgentMetadata: { platform: platformOf[agent], platformVersion: '', architecture: 'x86', model: '', mobile: agent === 'android', brands: [] },
  });
  await send('Emulation.setDeviceMetricsOverride', { width, height: 900, deviceScaleFactor: 1, mobile: agent === 'android' });
  await send('Network.setBlockedURLs', { urls: blockTheApi ? ['*api.github.com*'] : [] });
  await send('Page.navigate', { url: `http://127.0.0.1:${port}/index.html?run=${Date.now()}` });

  for (let attempt = 0; attempt < 80; attempt += 1) {
    if (await evaluate("document.readyState === 'complete' && !!document.getElementById('offer')")) break;
    await new Promise((resolve) => setTimeout(resolve, 125));
  }
  // The offer is written over once the release has answered, and settling on readyState alone
  // reads the resting list every time and would report the focused view as never arriving.
  await new Promise((resolve) => setTimeout(resolve, 900));
}

/* What the page says, read as the reader reads it rather than out of the source. */
const readOffer = () => evaluate(`(() => {
  const primary = document.querySelector('#offer .get__download .button');
  const list = [...document.querySelectorAll('#offer .route__go')];
  return {
    focused: !!primary,
    label: primary?.textContent ?? null,
    href: primary?.href ?? null,
    meta: document.querySelector('#offer .get__meta')?.textContent ?? null,
    steps: [...document.querySelectorAll('#offer .steps__item')].map((item) => item.textContent.trim()),
    elsewhere: [...document.querySelectorAll('#offer .elsewhere a')].map((a) => a.textContent.trim()),
    routes: list.map((a) => a.textContent.trim()),
    buttonHeight: primary ? primary.getBoundingClientRect().height : null,
    shortestRouteHeight: list.length ? Math.min(...list.map((a) => a.getBoundingClientRect().height)) : null,
    // The control is an anchor, and an anchor is underlined until something says otherwise,
    // which leaves the one thing on the page to press reading as a line of prose.
    underlined: [primary, ...list].filter(Boolean)
      .some((control) => getComputedStyle(control).textDecorationLine.includes('underline')),
    overflows: document.documentElement.scrollWidth > window.innerWidth + 1,
    tokensLoaded: getComputedStyle(document.documentElement).getPropertyValue('--accent').trim() !== '',
    focusVisible: !!document.querySelector('#offer a'),
  };
})()`);

/* Follow the address the button carries, and compare the bytes with the megabytes printed
 * under it by the same arithmetic the page used. */
async function followsToARealFile(what, href, meta) {
  if (!href || !href.startsWith('https://github.com/DrAlexHarrison/plateforce/releases/download/')) {
    record(false, `${what}: the button points at ${href}`);
    return;
  }
  const reply = await fetch(href, { redirect: 'follow' });
  if (!reply.ok) {
    record(false, `${what}: ${href} answered ${reply.status}`);
    return;
  }
  const bytes = Number(reply.headers.get('content-length'));
  record(reply.ok && bytes > 0, `${what}: the file resolves, ${bytes} bytes`);

  const claimed = meta?.match(/([\d.]+) MB/)?.[1];
  const actual = (bytes / 1e6).toFixed(1);
  record(claimed === actual, `${what}: the page claims ${claimed} MB and the file is ${actual} MB`);

  const version = meta?.match(/version ([\w.]+)/)?.[1];
  record(
    version === release.tag_name.replace(/^v/, ''),
    `${what}: the page names version ${version} and the newest release is ${release.tag_name}`,
  );
}

// The three machines the page can name, each sensed from its own user agent rather than
// asked for by address, because sensing is the half a reader never sees fail.
for (const [agent, label, ending] of [['mac', 'Mac', '.dmg'], ['windows', 'Windows', '-setup.exe'], ['linux', 'Linux', '.AppImage']]) {
  await load(agent, 1440);
  const offer = await readOffer();

  record(offer.focused, `${agent} at 1440: one button rather than the whole list`);
  record(offer.label === `Download for ${label}`, `${agent} at 1440: the button reads "${offer.label}"`);
  record(offer.href?.endsWith(ending), `${agent} at 1440: the button fetches a ${ending}`);
  record(offer.steps.length === 3, `${agent} at 1440: ${offer.steps.length} steps`);
  record(offer.elsewhere.includes('Use in browser'), `${agent} at 1440: the browser route is offered`);
  record(!offer.elsewhere.includes(label), `${agent} at 1440: the row does not offer the machine already shown`);
  record(offer.tokensLoaded, `${agent} at 1440: the application's tokens are loaded`);
  record(!offer.underlined, `${agent} at 1440: the control reads as a button rather than as prose`);
  record(!offer.overflows, `${agent} at 1440: nothing scrolls sideways`);
  await followsToARealFile(`${agent} at 1440`, offer.href, offer.meta);

  await load(agent, 390);
  const narrow = await readOffer();
  record(!narrow.overflows, `${agent} at 390: nothing scrolls sideways`);
  record(narrow.buttonHeight >= 44, `${agent} at 390: the button is ${narrow.buttonHeight}px tall`);
  record(narrow.focused, `${agent} at 390: the offer survives a phone-width viewport`);
}

// The Linux steps name the file, so they are the one set that goes wrong silently when a
// version changes under them.
await load('linux', 1440);
const linux = await readOffer();
const named = linux.href?.split('/').pop();
record(
  linux.steps.some((step) => step.includes(`chmod +x ${named}`)),
  `linux: the step names the file the button fetches, ${named}`,
);

// A machine the page cannot name, and a phone, both of which have no desktop file to be
// offered. This is the path with no user to report it broken.
for (const agent of ['nonsense', 'android']) {
  for (const width of [1440, 390]) {
    await load(agent, width);
    const offer = await readOffer();
    record(!offer.focused, `${agent} at ${width}: no machine is guessed at`);
    record(offer.routes.length === 4, `${agent} at ${width}: ${offer.routes.length} routes listed`);
    record(offer.routes.includes('Use in browser'), `${agent} at ${width}: the browser route is listed`);
    record(!offer.underlined, `${agent} at ${width}: the controls read as buttons rather than as prose`);
    record(!offer.overflows, `${agent} at ${width}: nothing scrolls sideways`);
    record(offer.shortestRouteHeight >= 44, `${agent} at ${width}: the smallest control is ${offer.shortestRouteHeight}px tall`);
  }
}

// The releases API unreachable, which is the same page with the network taken away.
await load('mac', 1440, { blockTheApi: true });
const offline = await readOffer();
record(!offline.focused, 'mac with the API unreachable: no version or size is claimed');
record(offline.routes.length === 4, `mac with the API unreachable: ${offline.routes.length} routes listed`);
record(
  offline.routes.every((route) => route.length > 0),
  'mac with the API unreachable: every route is still named',
);

/*
 * The row of other platforms switches the page in place. Asserting the links are present says
 * nothing about that: a link reading "Windows" that navigates to a release page is the same
 * markup and a different product, so the switch is performed rather than read.
 */
await load('mac', 1440);
const before = await readOffer();
const windowsLink = await evaluate(`(() => {
  const link = [...document.querySelectorAll('#offer .elsewhere a')].find((a) => a.textContent.trim() === 'Windows');
  return link ? { href: link.href, staysHere: link.getAttribute('href').startsWith('#') } : null;
})()`);
record(windowsLink !== null, 'the row offers a link to Windows');
// Read before pressing, because a link that leaves for a release page cannot be pressed here
// without tearing down the document this is reading, which would end the run rather than fail it.
record(windowsLink?.staysHere === true,
  `the Windows link switches this page rather than leaving it, it points at ${windowsLink?.href ?? 'nothing'}`);

let after = null;
if (windowsLink?.staysHere) {
  await evaluate(
    "[...document.querySelectorAll('#offer .elsewhere a')].find((a) => a.textContent.trim() === 'Windows').click()",
  );
  await new Promise((resolve) => setTimeout(resolve, 400));
  after = await readOffer();
}
record(before.label === 'Download for Mac' && after?.label === 'Download for Windows',
  `pressing Windows moved the page from "${before.label}" to "${after?.label ?? 'nowhere'}"`);
record(after?.href?.endsWith('-setup.exe'), 'pressing Windows moved the button to the Windows file');
record(Boolean(after?.steps.some((step) => step.includes('Start menu'))),
  'pressing Windows moved the steps to the Windows steps');
record(Boolean(after?.elsewhere.includes('Mac') && !after.elsewhere.includes('Windows')),
  'the row now offers the machine it moved away from and not the one it is on');

/*
 * The prompt control. The text is Alex's, verbatim, so it is compared character for character
 * against a copy held here: two independent copies is what makes a silent rewording fail.
 */
const PROMPT = [
  'I want to use plateforce, which is force-plate analysis where every',
  'result carries the method that produced it.',
  '',
  'Install it for Python:  pip install plateforce',
  'Docs: https://github.com/DrAlexHarrison/plateforce',
  '',
  'Then read my force-plate file, compute jump height, and show me',
  'which published rule produced each number.',
].join('\n');

for (const agent of ['mac', 'nonsense']) {
  for (const width of [1440, 390]) {
    await load(agent, width);
    const control = await evaluate(`(() => {
      const block = document.getElementById('assistant');
      const button = document.getElementById('assistant-copy');
      return {
        shown: !!block && !block.hidden,
        label: document.getElementById('assistant-label')?.textContent.trim() ?? null,
        says: button?.textContent.trim() ?? null,
        height: button ? button.getBoundingClientRect().height : null,
        text: document.getElementById('assistant-text')?.value ?? null,
        textHidden: document.getElementById('assistant-text')?.hidden ?? null,
        said: document.getElementById('assistant-said')?.textContent ?? null,
        overflows: document.documentElement.scrollWidth > window.innerWidth + 1,
        mentionsR: (block?.textContent ?? '').toLowerCase().includes('install.packages'),
      };
    })()`);

    record(control.shown, `${agent} at ${width}: the prompt control is offered`);
    record(control.text === PROMPT, `${agent} at ${width}: the prompt is the text it is meant to be`);
    record(control.label?.length > 0 && control.says?.length > 0,
      `${agent} at ${width}: it says what it does before it is pressed, "${control.label}" over "${control.says}"`);
    record(control.height >= 44, `${agent} at ${width}: the control is ${control.height}px tall`);
    record(control.textHidden === true, `${agent} at ${width}: the prompt is behind the control until it is needed`);
    record(control.said === '', `${agent} at ${width}: nothing is confirmed before anything is pressed`);
    record(!control.mentionsR, `${agent} at ${width}: only the route that installs is offered`);
    record(!control.overflows, `${agent} at ${width}: nothing scrolls sideways with the control on the page`);
  }
}

// Pressing it, with the clipboard working and then refusing. The refusal is the half a reader
// only meets on an insecure origin, which is exactly where nobody is watching.
await load('mac', 1440);
await send('Browser.grantPermissions', { permissions: ['clipboardReadWrite', 'clipboardSanitizedWrite'] });
const copied = await evaluate(`(async () => {
  document.getElementById('assistant-copy').click();
  await new Promise((r) => setTimeout(r, 300));
  return {
    said: document.getElementById('assistant-said').textContent,
    onClipboard: await navigator.clipboard.readText(),
  };
})()`);
record(copied.said.length > 0, `pressing it says what happened: "${copied.said}"`);
record(copied.onClipboard === PROMPT, 'pressing it puts the prompt on the clipboard, character for character');

// The message has to still be there later, because one that clears itself goes while somebody
// is reading it. Four seconds outlives every timer anybody writes.
await new Promise((resolve) => setTimeout(resolve, 4000));
const persisted = await evaluate("document.getElementById('assistant-said').textContent");
record(persisted === copied.said, 'the confirmation is still on the page four seconds later');

await load('mac', 1440);
const refused = await evaluate(`(async () => {
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    get: () => ({ writeText: () => Promise.reject(new Error('refused')) }),
  });
  document.getElementById('assistant-copy').click();
  await new Promise((r) => setTimeout(r, 300));
  const text = document.getElementById('assistant-text');
  return {
    said: document.getElementById('assistant-said').textContent,
    revealed: !text.hidden,
    selected: text.selectionEnd - text.selectionStart === text.value.length && text.value.length > 0,
    overflows: document.documentElement.scrollWidth > window.innerWidth + 1,
    // Copying by hand is what the reader is being asked to do, so the whole prompt has to be
    // in front of them rather than behind a scroll inside the box.
    wholeThingVisible: text.scrollHeight <= text.clientHeight + 1,
  };
})()`);
record(refused.revealed, 'a refused clipboard puts the prompt on the page');
record(refused.selected, 'a refused clipboard leaves the prompt selected, ready to copy by hand');
record(refused.said.length > 0, `a refused clipboard says so: "${refused.said}"`);
record(!refused.overflows, 'the revealed prompt does not widen the page');
record(refused.wholeThingVisible, 'the revealed prompt is shown whole rather than cut off');

/*
 * Interface copy describes the reader's machine and their choices. The gate that holds the
 * rest of the interface to that reads web/ and nothing else, so this page would sit outside
 * every check of it. The reading is of the rendered page rather than of the source, because
 * half of what a reader sees here is written by script from what the release answered.
 */
const BANNED = [
  'in this build', 'not implemented yet', 'coming soon', 'build default',
  'available here', 'listed disabled', 'generated in this tab', '—',
];
for (const agent of ['mac', 'linux', 'nonsense']) {
  await load(agent, 1440);
  const said = (await evaluate('document.body.innerText')).toLowerCase();
  const found = BANNED.filter((phrase) => said.includes(phrase.toLowerCase()));
  record(found.length === 0, `${agent}: the page says nothing about itself${found.length ? `, but for ${found.join(', ')}` : ''}`);
}

// The application is where the page says it is.
const app = await fetch(`http://127.0.0.1:${port}/app/`);
record(app.ok, `the application answers at /app/, ${app.status}`);
const appBody = await app.text();
record(appBody.includes('id="dropzone"'), 'the application at /app/ is the application');

// The way back. A reader who opened the application and wants it on their own machine has no
// route otherwise, and `../` from /app/ is this page.
await send('Page.navigate', { url: `http://127.0.0.1:${port}/app/index.html` });
for (let attempt = 0; attempt < 80; attempt += 1) {
  if (await evaluate("document.readyState === 'complete'")) break;
  await new Promise((resolve) => setTimeout(resolve, 125));
}
const back = await evaluate(`(() => {
  const link = document.querySelector('.app-header__install');
  return link ? { href: link.href, says: link.textContent.trim(), height: link.getBoundingClientRect().height } : null;
})()`);
record(back !== null, 'the application offers a way to the download page');
record(back?.href.endsWith('/'), `that link resolves to ${back?.href}`);
record(back?.height >= 44, `that link is ${back?.height}px tall`);
const backLanded = back ? await fetch(back.href.replace(/^http:\/\/[^/]+/, `http://127.0.0.1:${port}`)) : null;
record(Boolean(backLanded?.ok), `following it back answers ${backLanded?.status}`);
record(
  Boolean(backLanded && (await backLanded.text()).includes('Download for Mac')),
  'following it back reaches the download page',
);

// The same page is what the desktop application loads, where that offer is addressed to a
// reader who has already installed it and its relative href does not reach this site at all.
// The check above is the control and this is the treatment: a rule that removes the link
// everywhere and a rule that removes it nowhere each satisfy one of them and fail the other.
// The application is stood in for by defining the handle it defines, before any page script
// runs, which is the handle `web/format.js` already asks for the window title.
// The Page domain has to be enabled before a script can be registered against the next
// document, and a registration that silently does nothing would leave the link on screen and
// read as the guard failing rather than as the probe never running.
await send('Page.enable');
await send('Page.addScriptToEvaluateOnNewDocument', {
  source: 'window.__TAURI_INTERNALS__ = { invoke: () => Promise.resolve() };',
});
await send('Page.navigate', { url: `http://127.0.0.1:${port}/app/index.html` });
for (let attempt = 0; attempt < 80; attempt += 1) {
  if (await evaluate("document.readyState === 'complete' && !!document.querySelector('.app-header')")) break;
  await new Promise((resolve) => setTimeout(resolve, 125));
}
await new Promise((resolve) => setTimeout(resolve, 1200));
const asApplication = await evaluate(`(() => ({
  offers: document.querySelectorAll('.app-header__install').length,
  header: Boolean(document.querySelector('.app-header')),
  dropzone: Boolean(document.getElementById('dropzone')),
}))()`);
record(asApplication.offers === 0, `the application does not offer to install itself, ${asApplication.offers} of 0 drawn`);
record(
  asApplication.header && asApplication.dropzone,
  'and the page it draws instead is whole, header and drop zone both present',
);

server.close();

for (const line of held) console.log(`  ${line}`);
console.log(`\n${held.length} of ${held.length + failures.length} claims held`);
if (failures.length) {
  console.error('\nheld nothing:');
  for (const line of failures) console.error(`  ${line}`);
  process.exit(1);
}
process.exit(0);
