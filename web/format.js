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

export function secondaryDisplay(metric) {
  if (metric.value == null) return null;
  if (metric.unit === 'meters') return `${(metric.value * 100).toFixed(1)} cm`;
  if (metric.unit === 'seconds') return `${(metric.value * 1000).toFixed(0)} ms`;
  return null;
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
