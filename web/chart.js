/*
 * The force trace, its landmarks, and the interactions that move them.
 *
 * The trace is drawn on a canvas as a per-pixel min and max envelope, so a spike that
 * lasts one sample still reaches the screen. Everything a user can grab is a real focusable
 * element in an overlay rather than a hit-tested canvas region, which is what makes the
 * markers keyboard operable and screen-reader legible.
 */

const MARGIN = { left: 58, right: 14, top: 16, bottom: 44 };

/* The three landmark tracks mirror the response's three index fields. Their spoken labels
 * still come from the loaded registry or decision model, so the trace and rail cannot drift. */
export function landmarkDefinitions(registry, slots) {
  const identity = [
    ['onset', 'movement_onset', 'marker--onset'],
    ['takeoff', 'takeoff', 'marker--takeoff'],
    ['touchdown', 'landing', 'marker--touchdown'],
  ];
  return identity.map(([key, construct, className]) => {
    const slot = slots.find((entry) => entry.key === key || entry.construct === construct);
    const entry = registry.constructs.find((candidate) => candidate.id === construct);
    return { key, className, label: slot?.title || entry?.label || entry?.title || construct };
  });
}

function readColour(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function formatForce(value) {
  return Math.abs(value) >= 100 ? value.toFixed(0) : value.toFixed(1);
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
  constructor({ container, canvas, overlay, markers, onMarkerMove, onWindowChange, onViewChange }) {
    this.container = container;
    this.canvas = canvas;
    this.overlay = overlay;
    this.onMarkerMove = onMarkerMove;
    this.onWindowChange = onWindowChange;
    this.onViewChange = onViewChange;
    this.markerDefinitions = markers;

    this.envelope = null;
    this.analysis = null;
    this.sampleRateHz = 1;
    this.totalSamples = 1;
    this.viewStart = 0;
    this.viewEnd = 0;
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

    this.markers = this.markerDefinitions.map((definition) => {
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

    this.crosshair = document.createElement('div');
    this.crosshair.className = 'chart-crosshair';
    this.crosshair.hidden = true;
    this.crosshair.innerHTML =
      '<i class="chart-crosshair__vertical"></i>' +
      '<i class="chart-crosshair__horizontal"></i>' +
      '<span class="chart-crosshair__label"></span>';
    this.crosshairLabel = this.crosshair.querySelector('.chart-crosshair__label');
    this.crosshairHorizontal = this.crosshair.querySelector('.chart-crosshair__horizontal');
    this.overlay.append(this.crosshair);

    this.container.addEventListener('pointermove', (event) => this.showCrosshair(event));
    this.container.addEventListener('pointerleave', () => { this.crosshair.hidden = true; });
  }

  /* ---------------------------------------------------------------- geometry */

  indexToX(index) {
    return this.plot.left + ((index - this.viewStart) / Math.max(1, this.viewEnd - this.viewStart)) * this.plot.width;
  }

  xToIndex(x) {
    const ratio = (x - this.plot.left) / Math.max(1, this.plot.width);
    return Math.round(this.viewStart + Math.min(1, Math.max(0, ratio)) * (this.viewEnd - this.viewStart));
  }

  forceToY(force) {
    const { low, high } = this.forceRange;
    return this.plot.bottom - ((force - low) / Math.max(1e-9, high - low)) * this.plot.height;
  }

  pointerIndex(event) {
    const bounds = this.canvas.getBoundingClientRect();
    return this.xToIndex(event.clientX - bounds.left);
  }

  showCrosshair(event) {
    if (!this.envelope || !this.analysis) return;
    if (event.target instanceof Element && event.target.closest('.marker, .window-body, .window-handle')) {
      this.crosshair.hidden = true;
      return;
    }
    const bounds = this.canvas.getBoundingClientRect();
    const pointerX = event.clientX - bounds.left;
    if (pointerX < this.plot.left || pointerX > this.plot.right) {
      this.crosshair.hidden = true;
      return;
    }

    const index = this.xToIndex(pointerX);
    const ratio = (index - this.viewStart) / Math.max(1, this.viewEnd - this.viewStart);
    const bucket = Math.min(
      this.envelope.lower.length - 1,
      Math.max(0, Math.round(ratio * Math.max(0, this.envelope.lower.length - 1))),
    );
    const low = this.envelope.lower[bucket];
    const high = this.envelope.upper[bucket];
    if (!Number.isFinite(low) || !Number.isFinite(high)) {
      this.crosshair.hidden = true;
      return;
    }

    const nearest = this.markers
      .map((marker) => ({ marker, index: this.analysis[`${marker.key}_index`] }))
      .filter((entry) => entry.index != null)
      .sort((left, right) => Math.abs(left.index - index) - Math.abs(right.index - index))[0];
    const force = Math.abs(high - low) < 0.05
      ? `${formatForce((low + high) / 2)} N`
      : `${formatForce(low)} to ${formatForce(high)} N`;
    this.crosshairLabel.textContent =
      `${(index / this.sampleRateHz).toFixed(3)} s · ${force}` +
      (nearest ? ` · ${nearest.marker.label}` : '');
    this.crosshairLabel.style.maxWidth = `${this.plot.width}px`;

    const x = this.indexToX(index);
    const y = this.forceToY((low + high) / 2);
    this.crosshair.style.left = `${x}px`;
    this.crosshair.style.top = `${this.plot.top}px`;
    this.crosshair.style.height = `${this.plot.height}px`;
    this.crosshairHorizontal.style.top = `${Math.min(this.plot.height, Math.max(0, y - this.plot.top))}px`;
    this.crosshairHorizontal.style.left = `${this.plot.left - x}px`;
    this.crosshairHorizontal.style.width = `${this.plot.width}px`;
    this.crosshair.hidden = false;
    this.anchorLabel(this.crosshairLabel, x, 'chart-crosshair__label');
  }

  anchorLabel(label, x, baseClass) {
    label.classList.remove(`${baseClass}--start`, `${baseClass}--end`);
    const origin = baseClass === 'marker__label' ? '50%' : '0px';
    label.style.left = origin;
    const width = label.offsetWidth;
    if (x - width / 2 < this.plot.left) {
      label.classList.add(`${baseClass}--start`);
      const offset = this.plot.left - x;
      label.style.left = baseClass === 'marker__label' ? `calc(50% + ${offset}px)` : `${offset}px`;
    } else if (x + width / 2 > this.plot.right) {
      label.classList.add(`${baseClass}--end`);
      const offset = this.plot.right - x;
      label.style.left = baseClass === 'marker__label' ? `calc(50% + ${offset}px)` : `${offset}px`;
    }
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

    // Dragging does not reach single-sample precision at any plot width, so the arrow
    // keys are the only path to placing an onset exactly.
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

  setRecording(sampleCount, sampleRateHz) {
    this.totalSamples = Math.max(1, sampleCount);
    this.sampleRateHz = sampleRateHz;
    this.viewStart = 0;
    this.viewEnd = this.totalSamples - 1;
  }

  visibleRange() {
    return { start: this.viewStart, end: this.viewEnd };
  }

  setView(start, end) {
    const fullSpan = Math.max(1, this.totalSamples - 1);
    const minimumSpan = Math.min(fullSpan, Math.max(2, Math.round(this.sampleRateHz * 0.25)));
    const span = Math.min(fullSpan, Math.max(minimumSpan, Math.round(end - start)));
    const nextStart = Math.min(fullSpan - span, Math.max(0, Math.round(start)));
    const changed = nextStart !== this.viewStart || nextStart + span !== this.viewEnd;
    this.viewStart = nextStart;
    this.viewEnd = nextStart + span;
    if (changed) this.onViewChange?.(this.visibleRange());
  }

  zoom(factor) {
    const centre = (this.viewStart + this.viewEnd) / 2;
    const span = (this.viewEnd - this.viewStart) * factor;
    this.setView(centre - span / 2, centre + span / 2);
  }

  pan(fraction) {
    const span = this.viewEnd - this.viewStart;
    const available = Math.max(0, this.totalSamples - 1 - span);
    this.setView(available * Math.min(1, Math.max(0, fraction)), available * Math.min(1, Math.max(0, fraction)) + span);
  }

  fit() {
    this.setView(0, this.totalSamples - 1);
  }

  isFit() {
    return this.viewStart === 0 && this.viewEnd === this.totalSamples - 1;
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
      threshold: readColour('--mark-threshold'),
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
    context.font = `${readColour('--text-xs')} ${readColour('--font-mono')}`;

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
    const startSeconds = this.viewStart / this.sampleRateHz;
    const endSeconds = this.viewEnd / this.sampleRateHz;
    for (const seconds of niceTicks(startSeconds, endSeconds, 8)) {
      const x = Math.round(this.indexToX(seconds * this.sampleRateHz)) + 0.5;
      if (x < this.plot.left - 1 || x > this.plot.right + 1) continue;
      context.fillText(seconds.toFixed(1), x, this.plot.bottom + 8);
    }

    context.fillStyle = colours.text;
    context.save();
    context.translate(12, this.plot.top + this.plot.height / 2);
    context.rotate(-Math.PI / 2);
    context.textAlign = 'center';
    context.fillText('vGRF (N)', 0, 0);
    context.restore();

    // Below the tick row. Beside it, the title overlaps the last tick once the plot is
    // narrow.
    context.textAlign = 'center';
    context.fillText('time (s)', this.plot.left + this.plot.width / 2, this.plot.bottom + 24);
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
      { value: this.analysis.levels.takeoff_threshold_newtons, colour: colours.threshold, dash: [6, 4] },
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
      const start = this.analysis.weighing_start_index;
      const end = this.analysis.weighing_end_index;
      const visibleStart = Math.max(start, this.viewStart);
      const visibleEnd = Math.min(end, this.viewEnd);
      this.windowBody.hidden = visibleEnd <= visibleStart;
      if (!this.windowBody.hidden) {
        const left = this.indexToX(visibleStart);
        const right = this.indexToX(visibleEnd);
        this.windowBody.style.cssText = `top:${top};height:${height};left:${left}px;width:${Math.max(2, right - left)}px`;
      }
      for (const [index, value] of [start, end].entries()) {
        const handle = this.windowHandles[index].element;
        handle.hidden = value < this.viewStart || value > this.viewEnd;
        if (!handle.hidden) handle.style.cssText = `top:${top};height:${height};left:${this.indexToX(value)}px`;
      }
    }

    let labelRow = 0;
    for (const marker of this.markers) {
      const index = this.analysis ? this.analysis[`${marker.key}_index`] : null;
      if (index == null || index < this.viewStart || index > this.viewEnd) {
        marker.element.hidden = true;
        continue;
      }
      marker.element.hidden = false;
      marker.element.style.top = top;
      marker.element.style.height = height;
      marker.element.style.left = `${this.indexToX(index)}px`;
      marker.element.dataset.index = String(index);
      marker.labelElement.style.top = `${labelRow * 20}px`;
      this.anchorLabel(marker.labelElement, this.indexToX(index), 'marker__label');
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
