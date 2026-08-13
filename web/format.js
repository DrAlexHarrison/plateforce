/* How a number and a stage reach the page. No value is computed here, only rendered. */

const DECIMALS = {
  seconds: 4,
  newtons: 1,
  kilograms: 2,
  meters_per_second: 3,
  meters: 3,
  newton_seconds: 2,
};

export function formatNumber(value, unit) {
  if (value == null || !Number.isFinite(value)) return null;
  return value.toFixed(DECIMALS[unit] ?? 3);
}

/*
 * The engine spells unit symbols in characters every terminal renders, and a page is not a
 * terminal: the product dot and the exponents are typeset here, on the way to the DOM only,
 * never into a request, an envelope or a file.
 */
export function typesetUnit(symbol) {
  if (symbol == null) return symbol;
  return String(symbol)
    .replaceAll('.', '·')
    .replace(/([A-Za-z])2(?![0-9])/g, '$1²')
    .replace(/([A-Za-z])3(?![0-9])/g, '$1³');
}

export function secondaryDisplay(metric) {
  if (metric.value == null) return null;
  if (metric.unit === 'meters') return `${(metric.value * 100).toFixed(1)} cm`;
  if (metric.unit === 'seconds') return `${(metric.value * 1000).toFixed(0)} ms`;
  return null;
}

/*
 * A count and the noun it counts, agreeing, grouped the way the reader's locale groups digits.
 *
 * Every count a reader meets goes through here. A single-column force export is the ordinary
 * case in this field, so "1 columns" is not an edge a reader has to be unlucky to reach: it is
 * the first sentence on the first screen, and it was being written a fifth different way in a
 * fifth place. An irregular plural is passed rather than derived.
 */
export function counted(count, singular, plural = `${singular}s`) {
  return `${count.toLocaleString()} ${count === 1 ? singular : plural}`;
}

export function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text != null) node.textContent = text;
  return node;
}

export function showStage(id) {
  for (const stage of document.querySelectorAll('.stage')) stage.hidden = stage.id !== id;
}

export function setWindowTitle(fileName = null) {
  const title = fileName ? `${fileName} · plateforce` : 'plateforce';
  document.title = title;
  window.__TAURI_INTERNALS__?.invoke('plugin:window|set_title', { label: null, value: title }).catch(() => {});
}

/* The one reply shape the engine answers in, read once here. `{ok}` carries the result and
 * `{refusal}` carries the record: a code to branch on, the rule that declined, and what
 * could have been asked for instead. A refusal used to arrive as a thrown string, so the
 * page held the sentence and none of the fields. */
export function reply(json) {
  const parsed = JSON.parse(json);
  return { ok: parsed.ok ?? null, refusal: parsed.refusal ?? null };
}
