/*
 * The force trace, its landmarks, and the interactions that move them.
 *
 * The trace is drawn on a canvas as a per-pixel min and max envelope, so a spike that
 * lasts one sample still reaches the screen. Everything a user can grab is a real focusable
 * element in an overlay rather than a hit-tested canvas region, which is what makes the
 * markers keyboard operable and screen-reader legible.
 */

const MARGIN = { left: 58, right: 14, top: 16, bottom: 30 };
const MARKERS = [
  { key: 'onset', label: 'Onset', className: 'marker--onset' },
  { key: 'takeoff', label: 'Takeoff', className: 'marker--takeoff' },
  { key: 'touchdown', label: 'Touchdown', className: 'marker--touchdown' },
];

function readColour(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function niceTicks(low, high, target) {
  const span = high - low;
  if (!(span > 0)) return [low];
  const rough = span / target;
  const magnitude = 10 ** Math.floor(Math.log10(rough));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * magnitude).find((s) => s >= rough) || magnitude * 10;
  const ticks = [];
  for (let value = Math.ceil(low / step) * step; value <= high + step * 1e-6; value += step) {
    ticks.push(value);
  }
  return ticks;
}

export class TraceChart {
  constructor({ container, canvas, overlay, onMarkerMove, onWindowChange }) {
    this.container = container;
    this.canvas = canvas;
    this.overlay = overlay;
    this.onMarkerMove = onMarkerMove;
    this.onWindowChange = onWindowChange;

    this.envelope = null;
    this.analysis = null;
    this.sampleRateHz = 1;
    this.plot = { left: 0, right: 0, top: 0, bottom: 0, width: 0, height: 0 };

    this.buildOverlay();

    this.resizeObserver = new ResizeObserver(() => this.handleResize());
    this.resizeObserver.observe(container);
    this.pendingFrame = null;
  }

  /* ---------------------------------------------------------------- overlay */

  buildOverlay() {
    this.overlay.replaceChildren();

    this.windowBody = document.createElement('button');
    this.windowBody.type = 'button';
    this.windowBody.className = 'window-body';
    this.windowBody.setAttribute('aria-label', 'Weighing window, drag to move');
    this.attachWindowDrag(this.windowBody, 'move');
    this.overlay.append(this.windowBody);

    this.windowHandles = ['start', 'end'].map((edge) => {
      const handle = document.createElement('button');
      handle.type = 'button';
      handle.className = 'window-handle';
      handle.setAttribute('aria-label', `Weighing window ${edge}, drag to resize`);
      this.attachWindowDrag(handle, edge);
      this.overlay.append(handle);
      return { edge, element: handle };
    });

    this.markers = MARKERS.map((definition) => {
      const element = document.createElement('button');
      element.type = 'button';
      element.className = `marker ${definition.className}`;
      element.setAttribute('role', 'slider');
      element.setAttribute('aria-label', `${definition.label} marker`);
      const label = document.createElement('span');
      label.className = 'marker__label';
      label.textContent = definition.label;
      element.append(label);
      this.attachMarkerDrag(element, definition.key);
      this.overlay.append(element);
      return { ...definition, element, labelElement: label };
    });
  }

  /* ---------------------------------------------------------------- geometry */

  indexToX(index) {
    const count = this.envelope ? this.envelope.sample_count : 1;
    return this.plot.left + (index / Math.max(1, count - 1)) * this.plot.width;
  }

  xToIndex(x) {
    const count = this.envelope ? this.envelope.sample_count : 1;
    const ratio = (x - this.plot.left) / Math.max(1, this.plot.width);
    return Math.round(Math.min(1, Math.max(0, ratio)) * (count - 1));
  }

  forceToY(force) {
    const { low, high } = this.forceRange;
    return this.plot.bottom - ((force - low) / Math.max(1e-9, high - low)) * this.plot.height;
  }

  pointerIndex(event) {
    const bounds = this.canvas.getBoundingClientRect();
    return this.xToIndex(event.clientX - bounds.left);
  }

  /* ---------------------------------------------------------------- dragging */

  attachMarkerDrag(element, key) {
    element.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      element.setPointerCapture(event.pointerId);
      element.dataset.dragging = 'true';
    });
    element.addEventListener('pointermove', (event) => {
      if (element.dataset.dragging !== 'true') return;
      this.onMarkerMove(key, this.pointerIndex(event));
    });
    const release = (event) => {
      if (element.dataset.dragging !== 'true') return;
      delete element.dataset.dragging;
      element.releasePointerCapture?.(event.pointerId);
    };
    element.addEventListener('pointerup', release);
    element.addEventListener('pointercancel', release);

    // Keyboard is not a courtesy here. A researcher placing an onset to the sample cannot
    // do it by dragging, and the arrow keys are the only path that reaches one sample.
    element.addEventListener('keydown', (event) => {
      const steps = { ArrowLeft: -1, ArrowRight: 1, PageDown: -100, PageUp: 100, Home: null, End: null };
      if (!(event.key in steps)) return;
      event.preventDefault();
      const current = Number(element.dataset.index || 0);
      if (event.key === 'Home') return this.onMarkerMove(key, 0);
      if (event.key === 'End') return this.onMarkerMove(key, this.envelope.sample_count - 1);
      const step = steps[event.key] * (event.shiftKey ? 10 : 1);
      this.onMarkerMove(key, current + step);
    });
  }

  attachWindowDrag(element, mode) {
    let originIndex = 0;
    let originStart = 0;
    element.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      element.setPointerCapture(event.pointerId);
      element.dataset.dragging = 'true';
      originIndex = this.pointerIndex(event);
      originStart = this.analysis ? this.analysis.weighing_start_index : 0;
    });
    element.addEventListener('pointermove', (event) => {
      if (element.dataset.dragging !== 'true' || !this.analysis) return;
      const index = this.pointerIndex(event);
      const start = this.analysis.weighing_start_index;
      const end = this.analysis.weighing_end_index;
      const minimumSamples = Math.max(2, Math.round(this.sampleRateHz * 0.05));

      if (mode === 'move') {
        const shifted = Math.max(0, originStart + (index - originIndex));
        this.onWindowChange(shifted, (end - start) / this.sampleRateHz);
      } else if (mode === 'start') {
        const nextStart = Math.min(Math.max(0, index), end - minimumSamples);
        this.onWindowChange(nextStart, (end - nextStart) / this.sampleRateHz);
      } else {
        const nextEnd = Math.max(index, start + minimumSamples);
        this.onWindowChange(start, (nextEnd - start) / this.sampleRateHz);
      }
    });
    const release = (event) => {
      if (element.dataset.dragging !== 'true') return;
      delete element.dataset.dragging;
      element.releasePointerCapture?.(event.pointerId);
    };
    element.addEventListener('pointerup', release);
    element.addEventListener('pointercancel', release);
  }

  /* ---------------------------------------------------------------- data in */

  setEnvelope(envelope) {
    this.envelope = envelope;
    this.sampleRateHz = envelope.sample_rate_hz;
  }

  setAnalysis(analysis) {
    this.analysis = analysis;
  }

  plotWidthPx() {
    return Math.max(120, Math.round(this.container.clientWidth - MARGIN.left - MARGIN.right));
  }

  handleResize() {
    this.container.dispatchEvent(new CustomEvent('chart:resize', { bubbles: true }));
  }

  schedule() {
    if (this.pendingFrame) return;
    this.pendingFrame = requestAnimationFrame(() => {
      this.pendingFrame = null;
      this.render();
    });
  }

  /* ---------------------------------------------------------------- drawing */

  render() {
    if (!this.envelope) return;
    const ratio = window.devicePixelRatio || 1;
    const width = this.container.clientWidth;
    const height = this.container.clientHeight;
    this.canvas.width = Math.round(width * ratio);
    this.canvas.height = Math.round(height * ratio);

    const context = this.canvas.getContext('2d');
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, width, height);

    this.plot = {
      left: MARGIN.left,
      right: width - MARGIN.right,
      top: MARGIN.top,
      bottom: height - MARGIN.bottom,
      width: width - MARGIN.left - MARGIN.right,
      height: height - MARGIN.top - MARGIN.bottom,
    };

    const { lower, upper } = this.envelope;
    let low = Math.min(...lower);
    let high = Math.max(...upper);
    const pad = Math.max(10, (high - low) * 0.06);
    this.forceRange = { low: Math.min(0, low) - pad, high: high + pad };

    const colours = {
      grid: readColour('--grid'),
      text: readColour('--text-tertiary'),
      trace: readColour('--trace'),
      accent: readColour('--accent'),
      border: readColour('--border-strong'),
      danger: readColour('--danger'),
    };

    this.drawWeighingWindow(context, colours);
    this.drawBands(context, colours);
    this.drawGrid(context, colours);
    this.drawTrace(context, colours);
    this.drawLevels(context, colours);
    this.positionOverlay();
  }

  drawGrid(context, colours) {
    context.save();
    context.strokeStyle = colours.grid;
    context.fillStyle = colours.text;
    context.lineWidth = 1;
    context.font = '11px ui-monospace, SFMono-Regular, Menlo, Consolas, monospace';

    context.textAlign = 'right';
    context.textBaseline = 'middle';
    for (const value of niceTicks(this.forceRange.low, this.forceRange.high, 6)) {
      const y = Math.round(this.forceToY(value)) + 0.5;
      if (y < this.plot.top || y > this.plot.bottom) continue;
      context.beginPath();
      context.moveTo(this.plot.left, y);
      context.lineTo(this.plot.right, y);
      context.stroke();
      context.fillText(String(Math.round(value)), this.plot.left - 8, y);
    }
    context.textAlign = 'center';
    context.textBaseline = 'top';
    const durationSeconds = this.envelope.sample_count / this.sampleRateHz;
    for (const seconds of niceTicks(0, durationSeconds, 8)) {
      const x = Math.round(this.indexToX(seconds * this.sampleRateHz)) + 0.5;
      if (x < this.plot.left - 1 || x > this.plot.right + 1) continue;
      context.fillText(seconds.toFixed(1), x, this.plot.bottom + 8);
    }

    context.fillStyle = colours.text;
    context.textAlign = 'left';
    context.save();
    context.translate(12, this.plot.top + this.plot.height / 2);
    context.rotate(-Math.PI / 2);
    context.textAlign = 'center';
    context.fillText('vGRF (N)', 0, 0);
    context.restore();

    context.textAlign = 'right';
    context.fillText('time (s)', this.plot.right, this.plot.bottom + 8);
    context.restore();
  }

  drawWeighingWindow(context, colours) {
    if (!this.analysis) return;
    const left = this.indexToX(this.analysis.weighing_start_index);
    const right = this.indexToX(this.analysis.weighing_end_index);
    context.save();
    context.fillStyle = colours.accent;
    context.globalAlpha = 0.1;
    context.fillRect(left, this.plot.top, Math.max(1, right - left), this.plot.height);
    context.restore();
  }

  /* The k SD band runs the full width so a reader can see where the trace leaves it,
   * which is the instant the noise-relative onset rule fires. */
  drawBands(context, colours) {
    if (!this.analysis) return;
    const { onset_band_lower_newtons: lower, onset_band_upper_newtons: upper } = this.analysis.levels;
    if (lower == null || upper == null) return;
    const top = this.forceToY(upper);
    const bottom = this.forceToY(lower);
    context.save();
    context.fillStyle = colours.accent;
    context.globalAlpha = 0.12;
    context.fillRect(this.plot.left, top, this.plot.width, Math.max(1, bottom - top));
    context.globalAlpha = 0.5;
    context.strokeStyle = colours.accent;
    context.setLineDash([3, 3]);
    context.lineWidth = 1;
    for (const y of [top, bottom]) {
      context.beginPath();
      context.moveTo(this.plot.left, Math.round(y) + 0.5);
      context.lineTo(this.plot.right, Math.round(y) + 0.5);
      context.stroke();
    }
    context.restore();
  }

  drawTrace(context, colours) {
    const { lower, upper } = this.envelope;
    const step = this.plot.width / Math.max(1, lower.length - 1);
    context.save();
    context.fillStyle = colours.trace;
    context.strokeStyle = colours.trace;
    context.lineWidth = 1;
    context.beginPath();
    for (let index = 0; index < upper.length; index += 1) {
      const x = this.plot.left + index * step;
      const y = this.forceToY(upper[index]);
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    }
    for (let index = lower.length - 1; index >= 0; index -= 1) {
      context.lineTo(this.plot.left + index * step, this.forceToY(lower[index]));
    }
    context.closePath();
    context.fill();
    context.stroke();
    context.restore();
  }

  drawLevels(context, colours) {
    if (!this.analysis) return;
    const entries = [
      { value: this.analysis.levels.system_weight_newtons, colour: colours.accent, dash: [] },
      { value: this.analysis.levels.takeoff_threshold_newtons, colour: colours.danger, dash: [6, 4] },
    ];
    context.save();
    context.lineWidth = 1.5;
    for (const entry of entries) {
      if (entry.value == null) continue;
      const y = Math.round(this.forceToY(entry.value)) + 0.5;
      if (y < this.plot.top || y > this.plot.bottom) continue;
      context.strokeStyle = entry.colour;
      context.setLineDash(entry.dash);
      context.beginPath();
      context.moveTo(this.plot.left, y);
      context.lineTo(this.plot.right, y);
      context.stroke();
    }
    context.restore();
  }

  /* ---------------------------------------------------------------- overlay out */

  positionOverlay() {
    const top = `${this.plot.top}px`;
    const height = `${this.plot.height}px`;

    if (this.analysis) {
      const left = this.indexToX(this.analysis.weighing_start_index);
      const right = this.indexToX(this.analysis.weighing_end_index);
      this.windowBody.style.cssText = `top:${top};height:${height};left:${left}px;width:${Math.max(2, right - left)}px`;
      this.windowHandles[0].element.style.cssText = `top:${top};height:${height};left:${left}px`;
      this.windowHandles[1].element.style.cssText = `top:${top};height:${height};left:${right}px`;
    }

    let labelRow = 0;
    for (const marker of this.markers) {
      const index = this.analysis ? this.analysis[`${marker.key}_index`] : null;
      if (index == null) {
        marker.element.hidden = true;
        continue;
      }
      marker.element.hidden = false;
      marker.element.style.top = top;
      marker.element.style.height = height;
      marker.element.style.left = `${this.indexToX(index)}px`;
      marker.element.dataset.index = String(index);
      marker.labelElement.style.top = `${labelRow * 20}px`;
      labelRow += 1;

      const seconds = index / this.sampleRateHz;
      marker.element.setAttribute('aria-valuemin', '0');
      marker.element.setAttribute('aria-valuemax', String(this.envelope.sample_count - 1));
      marker.element.setAttribute('aria-valuenow', String(index));
      marker.element.setAttribute('aria-valuetext', `${marker.label} at ${seconds.toFixed(4)} seconds, sample ${index}`);

      const dragged = this.analysis.bound_methods?.some(
        (method) => method.manual_override && method.method_id.startsWith(`${marker.key}.`),
      );
      marker.element.classList.toggle('marker--dragged', Boolean(dragged));
    }
  }
}
