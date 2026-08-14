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
    // The construct rides with the track, so a caller asking which rule placed this landmark
    // reads it from the one table that pairs the two rather than naming it a second time.
    return { key, construct, className, label: slot?.title || entry?.label || entry?.title || construct };
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

/* A time axis whose ticks are far enough apart to read as different instants. Two ticks a
 * ten-thousandth of a second apart both print as 0.0 at one decimal, so the axis takes the
 * fewest decimals that tell its own ticks apart, which is what lets the view zoom to a pair of
 * samples and still say where they are. */
function timeDecimals(ticks) {
  for (let places = 1; places < 6; places += 1) {
    const shown = ticks.map((seconds) => seconds.toFixed(places));
    if (new Set(shown).size === shown.length) return places;
  }
  return 6;
}

function wrapLine(context, text, width) {
  const words = String(text).split(/\s+/);
  const lines = [];
  let line = '';
  for (const word of words) {
    const next = line ? `${line} ${word}` : word;
    if (line && context.measureText(next).width > width) {
      lines.push(line);
      line = word;
    } else {
      line = next;
    }
  }
  if (line) lines.push(line);
  return lines;
}

function pngBlob(canvas) {
  const encoded = canvas.toDataURL('image/png').split(',')[1];
  const bytes = atob(encoded);
  const data = new Uint8Array(bytes.length);
  for (let index = 0; index < bytes.length; index += 1) data[index] = bytes.charCodeAt(index);
  return new Blob([data], { type: 'image/png' });
}

export class TraceChart {
  constructor({
    container, canvas, overlay, markers,
    onMarkerMove, onMarkerEditStart, onMarkerEditEnd,
    onWindowChange, onWindowEditStart, onWindowEditEnd,
    onViewChange, onSelectionChange, regionLabel,
  }) {
    this.container = container;
    this.canvas = canvas;
    this.overlay = overlay;
    this.onMarkerMove = onMarkerMove;
    this.onMarkerEditStart = onMarkerEditStart;
    this.onMarkerEditEnd = onMarkerEditEnd;
    this.onWindowChange = onWindowChange;
    this.onWindowEditStart = onWindowEditStart;
    this.onWindowEditEnd = onWindowEditEnd;
    this.onViewChange = onViewChange;
    this.onSelectionChange = onSelectionChange;
    /* The words for an interval the analysis placed. Supplied rather than held here, because
     * those words are the registry's and this file names nothing the registry declares. */
    this.regionLabel = regionLabel || ((name) => name);
    this.markerDefinitions = markers;

    this.envelope = null;
    this.analysis = null;
    this.sampleRateHz = 1;
    this.totalSamples = 1;
    this.viewStart = 0;
    this.viewEnd = 0;
    this.plot = { left: 0, right: 0, top: 0, bottom: 0, width: 0, height: 0 };

    /* The spans a reader has selected, each carrying where its two ends came from. A span the
     * reader dragged is theirs; a span they double-clicked into is the phase rules', and the
     * two are held apart here because everything computed over them says which it was. */
    this.regions = [];
    this.activeRegion = -1;
    /* Intervals the analysis placed, as the engine reported them. Nothing here decides what a
     * region is or what it is called. */
    this.placedRegions = [];
    this.drag = null;
    this.pendingClear = null;
    /* Views a zoom replaced, so a reader who zoomed to a span can step back out of it. */
    this.viewStack = [];
    this.markerEdits = new Map();
    this.windowEdit = null;

    this.buildOverlay();
    this.attachSelection();

    this.resizeObserver = new ResizeObserver(() => this.handleResize());
    this.resizeObserver.observe(container);
    this.pendingFrame = null;
  }

  /* ---------------------------------------------------------------- overlay */

  buildOverlay() {
    this.overlay.replaceChildren();

    /* Before the weighing window and the markers, so a selection band sits behind everything a
     * reader grabs rather than swallowing the pointer that reaches for them. */
    this.selectionLayer = document.createElement('div');
    this.selectionLayer.className = 'selection-layer';
    this.overlay.append(this.selectionLayer);

    this.windowBody = document.createElement('button');
    this.windowBody.type = 'button';
    this.windowBody.className = 'window-body';
    this.windowBody.tabIndex = 0;
    this.windowBody.setAttribute('aria-label', 'Weighing window, drag to move');
    this.attachWindowDrag(this.windowBody, 'move');
    this.overlay.append(this.windowBody);

    this.windowHandles = ['start', 'end'].map((edge) => {
      const handle = document.createElement('button');
      handle.type = 'button';
      handle.className = 'window-handle';
      handle.tabIndex = 0;
      handle.setAttribute('aria-label', `Weighing window ${edge}, drag to resize`);
      this.attachWindowDrag(handle, edge);
      this.overlay.append(handle);
      return { edge, element: handle };
    });

    // The three landmarks in trace order, which is the order a reader reads them in and the
    // order the tab key walks them.
    this.markers = this.markerDefinitions.map((definition) => {
      const element = document.createElement('button');
      element.type = 'button';
      element.className = `marker ${definition.className}`;
      element.setAttribute('role', 'slider');
      // Spelt out rather than left to the button element. WebKit leaves a button out of the
      // tab order until the reader turns on full keyboard access, which is off by default, so
      // on one of the three desktops this software ships to the landmarks were unreachable.
      element.tabIndex = 0;
      element.setAttribute('aria-label', `${definition.label} marker`);
      const label = document.createElement('span');
      label.className = 'marker__label';
      label.textContent = definition.label;
      element.append(label);
      this.attachMarkerDrag(element, definition.key);
      // Focus changes nothing the canvas draws, so the overlay is repositioned rather than the
      // trace redrawn, which is what puts the instant on the label the keyboard just reached.
      for (const type of ['focus', 'blur']) {
        element.addEventListener(type, () => { if (this.envelope) this.positionOverlay(); });
      }
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
      // Near the line without being on it, so this press is the trace's rather than this
      // landmark's. It is left alone to bubble, and the container reads it as a selection.
      if (!this.markerUnder(event)) return;
      // Preventing the default suppresses the browser's own focus-on-press, so a reader who
      // clicked a landmark and then pressed an arrow key moved nothing. The default is still
      // prevented, because it also starts a native drag over the plot, and the focus the press
      // was owed is given here instead.
      event.preventDefault();
      element.focus();
      element.setPointerCapture(event.pointerId);
      element.dataset.dragging = 'true';
      this.markerEdits.set(key, this.onMarkerEditStart?.(key));
    });
    element.addEventListener('pointermove', (event) => {
      if (element.dataset.dragging !== 'true') return;
      this.onMarkerMove(key, this.pointerIndex(event));
    });
    const release = (event) => {
      if (element.dataset.dragging !== 'true') return;
      delete element.dataset.dragging;
      element.releasePointerCapture?.(event.pointerId);
      this.onMarkerEditEnd?.(key, this.markerEdits.get(key));
      this.markerEdits.delete(key);
    };
    element.addEventListener('pointerup', release);
    element.addEventListener('pointercancel', release);

    // Dragging does not reach single-sample precision at any plot width, so the arrow
    // keys are the only path to placing an onset exactly.
    element.addEventListener('keydown', (event) => {
      const steps = { ArrowLeft: -1, ArrowRight: 1, PageDown: -100, PageUp: 100, Home: null, End: null };
      if (!(event.key in steps)) return;
      event.preventDefault();
      const before = this.onMarkerEditStart?.(key);
      const current = Number(element.dataset.index || 0);
      let next;
      if (event.key === 'Home') next = 0;
      else if (event.key === 'End') next = this.envelope.sample_count - 1;
      else next = current + steps[event.key] * (event.shiftKey ? 10 : 1);
      this.onMarkerMove(key, next);
      this.onMarkerEditEnd?.(key, before);
    });
  }

  attachWindowDrag(element, mode) {
    let originIndex = 0;
    let originStart = 0;
    element.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      element.focus();
      element.setPointerCapture(event.pointerId);
      element.dataset.dragging = 'true';
      this.windowEdit = this.onWindowEditStart?.();
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
      this.onWindowEditEnd?.(this.windowEdit);
      this.windowEdit = null;
    };
    element.addEventListener('pointerup', release);
    element.addEventListener('pointercancel', release);
  }

  /* ---------------------------------------------------------------- selecting */

  /* Anything a reader can grab. A drag that starts on one of these is that control's drag and
   * not a selection, and the pointer has to reach it. A selected span is not among them: a
   * reader has to be able to draw a narrower span inside a wide one they already drew. */
  static GRABBABLE = '.marker';

  /*
   * How near a landmark's line a press has to be to become that landmark's drag.
   *
   * The element is 44 px wide because a finger needs that much to reach it at all, and that put
   * 183 ms of trace either side of a line drawn 1 px wide: measured at twelve distances, every
   * press out to 20 px moved a landmark, so a 12 px miss at any of the five moved one. Two of
   * those are dangerous. A miss near the landing moves it before takeoff and changes no number
   * the reader is looking at, and a miss inside the weighing band takes the marker rather than
   * the band's own handle 16 px away.
   *
   * Narrower than that 16 px gap, so the nearer control wins. Read off the press rather than a
   * media query, because a finger and a pointer that can be aimed arrive on the same element
   * and only the event knows which one this is. A media query cannot be asked here at all: a
   * browser driven with no input device matches neither fine nor coarse.
   */
  static GRAB_WITHIN_PX = 10;

  /*
   * The landmark this press belongs to, or nothing where it is near one without being on it.
   *
   * One home for the question, because the marker's own handler and the container's selection
   * have to give the same answer: two answers would leave a press that starts no drag also
   * starting no selection, which is a press that does nothing.
   */
  markerUnder(event) {
    const marker = event.target instanceof Element && event.target.closest(TraceChart.GRABBABLE);
    if (!marker) return null;
    const box = marker.getBoundingClientRect();
    const reach = event.pointerType === 'touch' ? box.width / 2 : TraceChart.GRAB_WITHIN_PX;
    return Math.abs(event.clientX - (box.left + box.width / 2)) <= reach ? marker : null;
  }

  attachSelection() {
    this.container.addEventListener('pointerdown', (event) => this.beginDrag(event));
    this.container.addEventListener('pointermove', (event) => this.growDrag(event));
    this.container.addEventListener('pointerup', (event) => this.endDrag(event));
    this.container.addEventListener('pointercancel', () => this.abandonDrag());
    this.container.addEventListener('dblclick', (event) => {
      // The click that opened this double-click already asked for the selection to be cleared.
      window.clearTimeout(this.pendingClear);
      this.pendingClear = null;
      event.preventDefault();
      this.selectPlacedAt(this.pointerIndex(event), event.shiftKey);
    });
  }

  beginDrag(event) {
    if (!this.envelope) return;
    if (event.button !== 0) return;
    if (this.markerUnder(event)) return;
    const bounds = this.canvas.getBoundingClientRect();
    const pointerX = event.clientX - bounds.left;
    if (pointerX < this.plot.left || pointerX > this.plot.right) return;

    // No `preventDefault` here. Preventing the default on `pointerdown` suppresses the mouse
    // events the browser synthesises from it, so the second click of a double-click never
    // becomes one and selecting a placed phase silently stops working. Text selection during a
    // drag is held off by `user-select` on the plot instead.
    this.container.setPointerCapture(event.pointerId);
    const anchor = this.pointerIndex(event);
    this.drag = {
      pointerId: event.pointerId,
      anchor,
      current: anchor,
      additive: event.shiftKey,
      // A press and release on a span the reader already has is that span's own button asking
      // to become the active one, so the clear below has to keep out of its way.
      onARegion: Boolean(event.target.closest?.('.selection-region')),
    };
    this.escapeListener = (key) => {
      if (key.key !== 'Escape' || !this.drag) return;
      key.preventDefault();
      this.abandonDrag();
    };
    window.addEventListener('keydown', this.escapeListener);
  }

  growDrag(event) {
    if (!this.drag || event.pointerId !== this.drag.pointerId) return;
    this.drag.current = this.pointerIndex(event);
    this.schedule();
    this.onSelectionChange?.(this.selection(), { dragging: this.draggedSpan() });
  }

  endDrag(event) {
    if (!this.drag || event.pointerId !== this.drag.pointerId) return;
    const { additive, onARegion } = this.drag;
    const span = this.draggedSpan();
    this.container.releasePointerCapture?.(event.pointerId);
    this.abandonDrag();
    if (span.endIndex <= span.startIndex && onARegion) return;

    // A press and release at one sample is a click, and a click is not a selection: it clears
    // one. Deferred, because the first click of a double-click looks exactly like this until
    // the second arrives, and a reader double-clicking a phase would otherwise watch their
    // selection go before it came.
    //
    // One timer, cancelled before another is set. A double-click ends two clicks, so leaving
    // the first one's timer running while the second replaced the handle left a clear pending
    // that nothing could cancel: the phase appeared and vanished a quarter of a second later.
    if (span.endIndex <= span.startIndex) {
      window.clearTimeout(this.pendingClear);
      this.pendingClear = window.setTimeout(() => {
        this.pendingClear = null;
        if (!this.regions.length) return;
        this.setRegions([]);
      }, 250);
      return;
    }
    this.setRegions(additive ? [...this.regions, { ...span, stated: true }] : [{ ...span, stated: true }]);
  }

  abandonDrag() {
    if (!this.drag) return;
    this.drag = null;
    window.removeEventListener('keydown', this.escapeListener);
    this.schedule();
    this.onSelectionChange?.(this.selection(), {});
  }

  /* The span under the pointer right now, in trace order whichever way the drag ran. */
  draggedSpan() {
    if (!this.drag) return null;
    const { anchor, current } = this.drag;
    return { startIndex: Math.min(anchor, current), endIndex: Math.max(anchor, current) };
  }

  /* The interval the analysis placed that holds this sample, or nothing where it placed none
   * here. Nothing is guessed: an interval no rule placed is not offered under a rule's name. */
  placedRegionAt(index) {
    return this.placedRegions.find((region) => index >= region.start_index && index <= region.end_index) || null;
  }

  selectPlacedAt(index, additive) {
    const placed = this.placedRegionAt(index);
    if (!placed) {
      this.onSelectionChange?.(this.selection(), { placedNothingHere: true });
      return;
    }
    const region = {
      startIndex: placed.start_index,
      endIndex: placed.end_index,
      stated: false,
      placed,
    };
    const already = this.regions.findIndex((held) => held.placed?.phase === placed.phase);
    if (additive && already === -1) this.setRegions([...this.regions, region]);
    else if (additive) this.setRegions(this.regions, already);
    else this.setRegions([region]);
  }

  setPlacedRegions(regions) {
    this.placedRegions = regions || [];
    // A region the reader picked off a rule that has since stopped placing it is a span with
    // nothing behind it, so it goes rather than sitting there under a rule's name.
    const surviving = this.regions.filter(
      (region) => region.stated || this.placedRegions.some((placed) => placed.phase === region.placed.phase),
    );
    if (surviving.length !== this.regions.length) this.setRegions(surviving);
  }

  /* One home for every change to the set, so the record, the drawing and the controls cannot
   * disagree about what is selected. `how` carries what moved the set, for a caller that says
   * out loud what an edit did and has no other way to tell a keyboard from a pointer. */
  setRegions(regions, active = regions.length - 1, how = {}) {
    this.regions = regions;
    this.activeRegion = regions.length ? Math.min(Math.max(0, active), regions.length - 1) : -1;
    this.drawSelectionHandles();
    this.schedule();
    this.onSelectionChange?.(this.selection(), how);
  }

  /* What is selected, as the caller reads it: every span, and which of them is the one a
   * number would be taken over. */
  selection() {
    return { regions: this.regions, active: this.regions[this.activeRegion] || null };
  }

  clearSelection() {
    this.setRegions([]);
  }

  /* The whole extent the selection covers, which is what a zoom to it shows. Several regions
   * zoom to the stretch holding all of them rather than to one of them. */
  selectionExtent() {
    if (!this.regions.length) return null;
    return {
      startIndex: Math.min(...this.regions.map((region) => region.startIndex)),
      endIndex: Math.max(...this.regions.map((region) => region.endIndex)),
    };
  }

  /* Zooming leaves the selection where it is: a reader who zoomed still means the span they
   * drew, and clearing it here would answer a question about the view by discarding the
   * answer to a different one. */
  zoomToSelection() {
    const extent = this.selectionExtent();
    if (!extent) return;
    this.viewStack.push({ start: this.viewStart, end: this.viewEnd });
    this.setView(extent.startIndex, extent.endIndex);
  }

  undoZoom() {
    const previous = this.viewStack.pop();
    if (!previous) return;
    this.setView(previous.start, previous.end);
  }

  resetZoom() {
    this.viewStack.length = 0;
    this.fit();
  }

  canUndoZoom() {
    return this.viewStack.length > 0;
  }

  /* One focusable element per selected span, so the set is reachable and legible without a
   * pointer. The band itself is drawn on the canvas; this is what a reader tabs to and what a
   * screen reader reads out. */
  drawSelectionHandles() {
    // A nudge rebuilds the set, and a reader holding an arrow key down would lose the span
    // they were moving on the first press. Which of them the keyboard was on survives the
    // rebuild, so a second press reaches the same span.
    const held = this.regionElements?.indexOf(document.activeElement) ?? -1;
    this.selectionLayer.replaceChildren();
    this.regionElements = this.regions.map((region, position) => {
      const element = document.createElement('button');
      element.type = 'button';
      element.className = `selection-region${position === this.activeRegion ? ' selection-region--active' : ''}`;
      element.dataset.stated = String(region.stated);
      element.tabIndex = 0;
      element.addEventListener('click', () => this.setRegions(this.regions, position));
      this.attachRegionKeys(element, position);
      this.selectionLayer.append(element);
      return element;
    });
    if (held >= 0) this.regionElements[Math.min(held, this.regionElements.length - 1)]?.focus();
  }

  /*
   * A selected span under the keyboard, in the grammar the reader already has for a selection.
   *
   * Arrows move the whole span and Shift extends its far end, which is what Shift and an arrow
   * do to a selection in every text field. A landmark takes Shift as a larger step instead,
   * because a landmark is a slider and that is the slider's own convention: two controls, each
   * following the convention of the widget it is.
   */
  attachRegionKeys(element, position) {
    element.addEventListener('keydown', (event) => {
      const steps = { ArrowLeft: -1, ArrowRight: 1, PageDown: -100, PageUp: 100 };
      if (!(event.key in steps) || !this.regions[position]) return;
      event.preventDefault();
      const region = this.regions[position];
      const limit = this.totalSamples - 1;
      const span = region.endIndex - region.startIndex;
      let { startIndex, endIndex } = region;
      if (event.shiftKey && event.key.startsWith('Arrow')) {
        endIndex = Math.min(limit, Math.max(startIndex + 1, endIndex + steps[event.key]));
      } else {
        startIndex = Math.min(limit - span, Math.max(0, startIndex + steps[event.key]));
        endIndex = startIndex + span;
      }
      if (startIndex === region.startIndex && endIndex === region.endIndex) return;
      // A span a rule placed becomes the reader's own the moment they move an end of it, so it
      // rebinds to the rule for a stated window rather than keeping the phase rules' name on
      // boundaries they no longer placed.
      const moved = this.regions.map((held, at) =>
        (at === position ? { startIndex, endIndex, stated: true } : held));
      this.setRegions(moved, position, { nudged: event.shiftKey ? 'end' : 'span' });
    });
  }

  /* ---------------------------------------------------------------- data in */

  setEnvelope(envelope) {
    this.envelope = envelope;
    this.sampleRateHz = envelope.sample_rate_hz;
  }

  setAnalysis(analysis) {
    this.analysis = analysis;
    this.setPlacedRegions(analysis?.regions || []);
  }

  /* A different recording, so nothing selected on the last one survives: two indices mean
   * different instants on a trace of a different length and rate. */
  setRecording(sampleCount, sampleRateHz) {
    this.totalSamples = Math.max(1, sampleCount);
    this.sampleRateHz = sampleRateHz;
    this.viewStart = 0;
    this.viewEnd = this.totalSamples - 1;
    this.viewStack.length = 0;
    this.placedRegions = [];
    this.setRegions([]);
  }

  visibleRange() {
    return { start: this.viewStart, end: this.viewEnd };
  }

  /* Two samples is the floor, because a view narrower than one sampling interval shows no
   * interval. A quarter of a second stood in front of it, which is 300 samples at 1200 Hz and
   * wider than the takeoff transition, so the one view this software exists to offer, the
   * samples either side of a placed landmark, was the view the chart refused. At full zoom the
   * plate's 1.398 N converter steps are visible, which is the right thing to see when the
   * question is whether a threshold rule fired on a step or on the movement. */
  setView(start, end) {
    const fullSpan = Math.max(1, this.totalSamples - 1);
    const minimumSpan = Math.min(fullSpan, 2);
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

  zoomAt(factor, anchor) {
    const span = this.viewEnd - this.viewStart;
    const held = Math.min(this.viewEnd, Math.max(this.viewStart, anchor));
    const position = span > 0 ? (held - this.viewStart) / span : 0.5;
    const next = span * factor;
    this.setView(held - next * position, held + next * (1 - position));
  }

  sampleAtClientX(clientX) {
    const bounds = this.canvas.getBoundingClientRect();
    return this.xToIndex(clientX - bounds.left);
  }

  panBy(fractionOfView) {
    const span = this.viewEnd - this.viewStart;
    const shift = span * fractionOfView;
    this.setView(this.viewStart + shift, this.viewEnd + shift);
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

  snapshot(notes = []) {
    this.render();
    const ratio = window.devicePixelRatio || 1;
    const width = this.container.clientWidth;
    const height = this.container.clientHeight;
    const font = `${readColour('--text-xs')} ${readColour('--font-mono')}`;
    const measure = document.createElement('canvas').getContext('2d');
    measure.font = font;
    const lines = notes.flatMap((line) => wrapLine(measure, line, Math.max(120, width - 32)));
    const lineHeight = 18;
    const footerHeight = lines.length ? 24 + lines.length * lineHeight : 0;
    const exported = document.createElement('canvas');
    exported.width = Math.round(width * ratio);
    exported.height = Math.round((height + footerHeight) * ratio);
    const context = exported.getContext('2d');
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.fillStyle = readColour('--surface');
    context.fillRect(0, 0, width, height + footerHeight);
    context.drawImage(this.canvas, 0, 0, width, height);

    if (this.analysis) {
      context.save();
      context.font = `600 ${readColour('--text-xs')} ${readColour('--font-body')}`;
      context.textBaseline = 'middle';
      let row = 0;
      for (const marker of this.markers) {
        const index = this.analysis[`${marker.key}_index`];
        if (index == null || index < this.viewStart || index > this.viewEnd) continue;
        const x = this.indexToX(index);
        const colour = readColour(`--track-${marker.key}`);
        context.strokeStyle = colour;
        context.lineWidth = 2;
        context.beginPath();
        context.moveTo(Math.round(x) + 0.5, this.plot.top);
        context.lineTo(Math.round(x) + 0.5, this.plot.bottom);
        context.stroke();
        const labelWidth = context.measureText(marker.label).width + 12;
        const left = Math.min(this.plot.right - labelWidth, Math.max(this.plot.left, x - labelWidth / 2));
        const top = this.plot.top + row * 20;
        context.fillStyle = colour;
        context.fillRect(left, top, labelWidth, 18);
        context.fillStyle = readColour('--marker-contrast');
        context.fillText(marker.label, left + 6, top + 9);
        row += 1;
      }
      context.restore();
    }

    if (lines.length) {
      context.strokeStyle = readColour('--border-strong');
      context.lineWidth = 1;
      context.beginPath();
      context.moveTo(0, height + 0.5);
      context.lineTo(width, height + 0.5);
      context.stroke();
      context.fillStyle = readColour('--text');
      context.font = font;
      context.textAlign = 'left';
      context.textBaseline = 'top';
      lines.forEach((line, index) => context.fillText(line, 16, height + 12 + index * lineHeight));
    }
    return exported;
  }

  imageBlob(notes = []) {
    return pngBlob(this.snapshot(notes));
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
    this.drawSelection(context, colours);
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
    const ticks = niceTicks(startSeconds, endSeconds, 8);
    const places = timeDecimals(ticks);
    for (const seconds of ticks) {
      const x = Math.round(this.indexToX(seconds * this.sampleRateHz)) + 0.5;
      if (x < this.plot.left - 1 || x > this.plot.right + 1) continue;
      context.fillText(seconds.toFixed(places), x, this.plot.bottom + 8);
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

  /* Selected spans, and the one under the pointer while a drag is running.
   *
   * Ink rather than a hue, because the three hues on this trace are the landmark tracks and a
   * selection is not a landmark. Which end of the pair a span came from is never carried by the
   * drawing alone: the readout beside the chart names each span's origin in words. */
  drawSelection(context, colours) {
    const spans = [...this.regions];
    const dragging = this.draggedSpan();
    if (dragging) spans.push(dragging);
    if (!spans.length) return;

    context.save();
    for (const span of spans) {
      const left = this.indexToX(Math.max(span.startIndex, this.viewStart));
      const right = this.indexToX(Math.min(span.endIndex, this.viewEnd));
      if (right < this.plot.left || left > this.plot.right) continue;
      // Heavier than the weighing window it can sit beside. That window is a rule's standing
      // answer and this is what the reader just did, so on a trace already carrying a band at
      // one tenth ink the selection has to be the thing the eye finds first.
      context.fillStyle = colours.text;
      context.globalAlpha = 0.16;
      context.fillRect(left, this.plot.top, Math.max(1, right - left), this.plot.height);
      context.globalAlpha = 0.9;
      context.strokeStyle = colours.text;
      context.lineWidth = 1.5;
      for (const x of [left, right]) {
        context.beginPath();
        context.moveTo(Math.round(x) + 0.5, this.plot.top);
        context.lineTo(Math.round(x) + 0.5, this.plot.bottom);
        context.stroke();
      }
    }
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

  /* What a span is, said in words rather than drawn. It carries the extent both ways a reader
   * works in, seconds and samples, and where its two ends came from, because those are the two
   * facts that separate a span somebody drew from a span a rule placed. */
  selectionSentence(region, position) {
    const from = (region.startIndex / this.sampleRateHz).toFixed(4);
    const to = (region.endIndex / this.sampleRateHz).toFixed(4);
    const samples = region.endIndex - region.startIndex + 1;
    const origin = region.stated
      ? 'selected by you'
      : `${this.regionLabel(region.placed.phase)}, from the rules that placed its boundaries`;
    return `Selection ${position + 1}, ${origin}, ${from} to ${to} seconds, ${samples} samples`;
  }

  positionOverlay() {
    const top = `${this.plot.top}px`;
    const height = `${this.plot.height}px`;

    for (const [position, region] of this.regions.entries()) {
      const element = this.regionElements?.[position];
      if (!element) continue;
      const left = this.indexToX(Math.max(region.startIndex, this.viewStart));
      const right = this.indexToX(Math.min(region.endIndex, this.viewEnd));
      element.hidden = right < this.plot.left || left > this.plot.right;
      if (element.hidden) continue;
      // The band on the canvas is the width of the span and says what it is. This is the thing
      // a finger has to hit, so it widens to a touch target around the same centre rather than
      // making the drawing lie: a third of a second at this plot width is 17 px.
      const width = Math.max(44, right - left);
      const centred = (left + right) / 2 - width / 2;
      element.style.cssText = `top:${top};height:${height};left:${centred}px;width:${width}px`;
      element.setAttribute('aria-label', this.selectionSentence(region, position));
      element.setAttribute('aria-pressed', String(position === this.activeRegion));
    }

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

      const seconds = index / this.sampleRateHz;
      // The instant, on the landmark the reader is holding. The crosshair that would otherwise
      // answer where the pointer is steps aside for a marker, so a drag used to move a line to
      // no stated time and a reader learnt where they had put it by reading a card afterwards.
      // Shown on the one being moved rather than on all three, which would put three times
      // across a trace whose whole width is five seconds.
      const holding = marker.element.dataset.dragging === 'true'
        || document.activeElement === marker.element;
      marker.labelElement.textContent = holding
        ? `${marker.label} ${seconds.toFixed(4)} s`
        : marker.label;
      this.anchorLabel(marker.labelElement, this.indexToX(index), 'marker__label');
      labelRow += 1;

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
