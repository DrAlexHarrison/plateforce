/*
 * The one button, resolved against the newest release at the moment the page is read.
 *
 * The alternative was to write the version and the file names into the page when the site is
 * published. It was rejected on a measurement rather than a preference: the installer names
 * carry the version, so `releases/latest/download/plateforce_0.1.0_universal.dmg` answers 200
 * today and `plateforce_0.1.1_universal.dmg` answers 404, and the two swap the moment a tag is
 * cut. The site publishes from a push to main and a release publishes from a tag, so a name
 * written in at publication is a name that outlives its file every time a release lands
 * without a push behind it. Read at load, the page is right the instant the release exists and
 * nobody has to touch anything.
 *
 * What that buys is paid for in the failure direction, so nothing here is load bearing: the
 * complete list of routes is what index.html already holds, and it is written over only once a
 * real release has answered with a real file. No script, no network, no releases API and no
 * recognisable machine all end at that same list.
 */

const RELEASES = 'https://github.com/DrAlexHarrison/plateforce/releases';
const LATEST = 'https://api.github.com/repos/DrAlexHarrison/plateforce/releases/latest';
const THEME_KEY = 'plateforce.theme';

/* The colour the reader chose in the application, which is the same product on the same
 * origin, so arriving here does not undo a choice they already made. */
function restoreTheme() {
  try {
    const held = window.localStorage.getItem(THEME_KEY);
    if (held === 'light' || held === 'dark') document.documentElement.dataset.theme = held;
  } catch {
    /* the automatic setting stands */
  }
}

/*
 * Which desktop this is, or nothing.
 *
 * Nothing is a real answer and the common one on a phone, so it is returned rather than
 * guessed at: Android carries "Linux" in its user agent and would otherwise be handed a
 * package it cannot run. The client-hint platform is asked first because it is the field the
 * browser maintains, and the user-agent string is the reading for browsers without it.
 */
function thisMachine() {
  const hinted = navigator.userAgentData?.platform ?? '';
  const agent = navigator.userAgent ?? '';
  const said = `${hinted} ${agent}`;

  if (/Android/i.test(said)) return null;
  if (/iPhone|iPad|iPod/i.test(said)) return null;
  if (/CrOS/i.test(said)) return null;
  if (/Mac OS X|macOS|Macintosh/i.test(said)) return 'mac';
  if (/Windows/i.test(said)) return 'windows';
  if (/Linux|X11/i.test(said)) return 'linux';
  return null;
}

/* Names carry the version, so the file is found by what it is rather than by what it is
 * called, and a version bump needs no edit here. */
const WANTED = {
  mac: (name) => name.endsWith('.dmg'),
  windows: (name) => name.endsWith('-setup.exe'),
  linux: (name) => name.endsWith('.AppImage'),
};

const LABEL = { mac: 'Mac', windows: 'Windows', linux: 'Linux' };

/*
 * Megabytes as the operating systems that wrote the file report them, a million bytes to the
 * megabyte, so the figure on the button matches the one in the reader's own downloads list.
 */
function megabytes(bytes) {
  return `${(bytes / 1e6).toFixed(1)} MB`;
}

function element(tag, className, text) {
  const made = document.createElement(tag);
  if (className) made.className = className;
  if (text !== undefined) made.textContent = text;
  return made;
}

/* Steps are written as a list of parts so a file name reaches the page as text and never as
 * markup, whatever the release calls it. */
function steps(platform, assetName) {
  if (platform === 'mac') {
    return [
      [{ text: 'Open the file in your Downloads folder' }],
      [{ text: 'Drag plateforce into Applications' }],
      [{ text: 'Open it from Applications' }],
    ];
  }
  if (platform === 'windows') {
    return [
      [{ text: 'Open the file in your Downloads folder' }],
      [{ text: 'Windows shows "Windows protected your PC". Choose More info, then Run anyway' }],
      [{ text: 'Open plateforce from the Start menu' }],
    ];
  }
  return [
    [{ text: 'Open a terminal in your Downloads folder' }],
    [{ text: 'Run ' }, { code: `chmod +x ${assetName}` }],
    [{ text: 'Run ' }, { code: `./${assetName}` }],
  ];
}

function stepsBlock(platform, assetName) {
  const block = element('div', 'steps');
  block.append(element('p', 'steps__label', 'Then:'));

  const list = element('ol', 'steps__list');
  for (const [index, parts] of steps(platform, assetName).entries()) {
    const item = element('li', 'steps__item');
    item.append(element('span', 'steps__number', String(index + 1)));
    const body = element('span');
    for (const part of parts) {
      body.append(part.code === undefined ? part.text : element('code', null, part.code));
    }
    item.append(body);
    list.append(item);
  }
  block.append(list);
  return block;
}

/* The two Linux packages that are not the AppImage, named only when the release carries them. */
function linuxPackages(assets) {
  const debian = assets.find((asset) => asset.name.endsWith('.deb'));
  const fedora = assets.find((asset) => asset.name.endsWith('.rpm'));
  if (!debian && !fedora) return null;

  const aside = element('p', 'get__aside');
  aside.append('On Debian, Ubuntu or Fedora you can install a package instead: ');
  const links = [];
  if (debian) {
    const link = element('a', null, 'the deb');
    link.href = debian.browser_download_url;
    links.push(link);
  }
  if (fedora) {
    const link = element('a', null, 'the rpm');
    link.href = fedora.browser_download_url;
    links.push(link);
  }
  links.forEach((link, index) => {
    if (index > 0) aside.append(' or ');
    aside.append(link);
  });
  aside.append('.');
  return aside;
}

/*
 * The prompt control, wired once and moved between views rather than rebuilt.
 *
 * It installs the Python package, which every machine can run, so it is offered on the three
 * this page can name and on the ones it cannot alike.
 */
function wireAssistant() {
  const block = document.getElementById('assistant');
  const button = document.getElementById('assistant-copy');
  const said = document.getElementById('assistant-said');
  const text = document.getElementById('assistant-text');
  block.hidden = false;

  button.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(text.value);
      said.textContent = 'Copied. Paste it into your assistant.';
      said.classList.remove('assistant__said--by-hand');
    } catch {
      // A refused clipboard leaves a button that looks like it did nothing, so the text comes
      // out and is selected: the reader finishes the job the browser would not.
      text.hidden = false;
      text.focus();
      text.select();
      said.textContent = 'Your browser did not allow copying. The prompt is below, selected and ready.';
      said.classList.add('assistant__said--by-hand');
    }
  });
  return block;
}

/* Every route this reader is not on, including the one that needs no download. */
function elsewhere(platform) {
  const row = element('nav', 'elsewhere');
  row.setAttribute('aria-label', 'Other ways to run plateforce');

  const routes = [];
  for (const other of ['mac', 'windows', 'linux']) {
    if (other === platform) continue;
    const link = element('a', null, LABEL[other]);
    link.href = `#${other}`;
    routes.push(link);
  }
  const browser = element('a', null, 'Use in browser');
  browser.href = './app/';
  routes.push(browser);

  routes.forEach((link, index) => {
    if (index > 0) row.append(element('span', 'elsewhere__divider', '·'));
    row.append(link);
  });
  return row;
}

function show(platform, release, assistant) {
  const asset = release.assets.find((candidate) => WANTED[platform](candidate.name));
  if (!asset) return false;

  const offer = element('div', 'get__offer');

  const download = element('div', 'get__download');
  const button = element('a', 'button button--primary', `Download for ${LABEL[platform]}`);
  button.href = asset.browser_download_url;
  download.append(button);
  download.append(element(
    'p',
    'get__meta',
    `version ${release.tag_name.replace(/^v/, '')} · ${megabytes(asset.size)}`,
  ));
  offer.append(download);

  offer.append(stepsBlock(platform, asset.name));

  if (platform === 'linux') {
    const packages = linuxPackages(release.assets);
    if (packages) offer.append(packages);
  }

  // Moved rather than copied, so the prompt a reader is handed is one node with one text,
  // and switching platform cannot leave two of it on the page.
  offer.append(assistant);
  offer.append(elsewhere(platform));

  offer.id = 'offer';
  document.getElementById('offer').replaceWith(offer);
  return true;
}

/* A platform named in the address wins over the machine, so one reader can send another the
 * instructions for the machine that reader is actually sitting at. */
function asked() {
  const named = window.location.hash.replace('#', '');
  return named in LABEL ? named : null;
}

async function start() {
  restoreTheme();
  // Wired before the release is asked for, so the prompt is offered on the complete list too,
  // which is where a reader whose machine this cannot name ends up.
  const assistant = wireAssistant();

  const platform = asked() ?? thisMachine();
  if (!platform) return;

  let release;
  try {
    const answer = await fetch(LATEST, { headers: { Accept: 'application/vnd.github+json' } });
    if (!answer.ok) return;
    release = await answer.json();
  } catch {
    return;
  }
  if (!Array.isArray(release?.assets) || !release.tag_name) return;

  if (!show(platform, release, assistant)) return;

  window.addEventListener('hashchange', () => {
    const next = asked();
    if (next) show(next, release, assistant);
  });
}

start();
