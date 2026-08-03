/*
 * Turning the registry into the decisions the interface presents.
 *
 * Nothing here names a construct or a method. The rows come from the rules this build
 * declares it can run, the label and the note on each row come from the registry's own
 * entry for that construct, and the surfacing field on the entry bound to a row decides how
 * hard the choice is pushed at the reader.
 *
 * A candidate is offered only when a rule exists to run it. A runnable rule the registry
 * has not documented still appears, flagged as carrying no citation, because presenting
 * it as citable would be the misattribution the registry exists to prevent. That splits
 * two ways: a composition binds parameters on a registry entry and inherits its
 * citations, while an unfiled rule has no entry anywhere.
 */

/* Entries the registry rules out as user choices. The reasoning lives in the registry and
 * in the docs, not in the interface. */
const NOT_A_CHOICE = new Set(['never_a_user_choice', 'refuse']);

/* The loaded registry, so an axis can read the values the literature contains rather than
 * carrying a copy of them. Set once when the model is built. */
let loaded = null;

/*
 * One row per construct on the path, in the order the pipeline runs them.
 *
 * `path` is the constructs beyond the three the request names by its own fields. A
 * construct nobody asked for is not on the path, so it is never instantiated and raises no
 * decision, which is why the registry's fifty-eight constructs do not become fifty-eight
 * rows.
 */
export function buildDecisionModel(registry, build, path = []) {
  loaded = registry;
  const documentedByConstruct = new Map();
  for (const method of registry.methods) {
    if (!documentedByConstruct.has(method.construct)) documentedByConstruct.set(method.construct, []);
    documentedByConstruct.get(method.construct).push(method);
  }

  const spine = new Set(build.spine_constructs);
  const onThePath = new Set([...build.spine_constructs, ...path]);

  const rows = [];
  const seen = new Set();
  build.bindings.forEach((binding, pipelineIndex) => {
    if (seen.has(binding.construct) || !onThePath.has(binding.construct)) return;
    seen.add(binding.construct);
    const runnable = build.bindings.filter((entry) => entry.construct === binding.construct);
    rows.push(
      rowFor(registry, documentedByConstruct.get(binding.construct) || [], runnable, {
        construct: binding.construct,
        // The word the request uses for this construct: its own field for the three it
        // names that way, and the construct id for every rule reached through `derived`.
        key: spine.has(binding.construct) ? binding.slot : binding.construct,
        spine: spine.has(binding.construct),
        pipelineIndex,
      }),
    );
  });
  return orderByConsequence(rows);
}

function rowFor(registry, documented, runnable, identity) {
  const executableIds = new Set(runnable.map((binding) => binding.id));
  const offered = documented.filter((method) => !NOT_A_CHOICE.has(method.gui?.surfacing));

  const candidates = offered.map((method) => ({
    id: method.id,
    title: method.title,
    status: method.status,
    surfacing: method.gui?.surfacing || null,
    method,
    registryBacked: true,
    composedFrom: null,
    executable: executableIds.has(method.id),
    note: '',
  }));

  for (const binding of runnable) {
    if (candidates.some((candidate) => candidate.id === binding.id)) continue;
    candidates.push({
      id: binding.id,
      title: binding.title,
      status: null,
      surfacing: null,
      method: null,
      registryBacked: false,
      composedFrom: binding.composed_from || null,
      executable: true,
      note: binding.note || 'No registry row carries this id.',
    });
  }

  const entry = registry.constructs.find((c) => c.id === identity.construct) || null;
  return {
    ...identity,
    constructEntry: entry,
    /* The field's spoken words for this quantity, which is what the terminal prints beside
     * each construct it lists. */
    title: entry?.label || entry?.title || identity.construct,
    /* What the registry says about the quantity itself, shown where the entry bound to the
     * row says nothing about what the choice costs. */
    notes: entry?.notes || '',
    candidates,
    available: candidates.filter((candidate) => candidate.executable),
    forcesDecision: offered.some((method) => method.gui?.surfacing === 'force_a_decision'),
  };
}

/*
 * The rail reads down in the order the choices matter.
 *
 * A construct carrying an entry that forces a decision leaves every value below it
 * provisional until somebody resolves it, so it is the most consequential row on the page
 * and it is read first. Within that, the pipeline order, so a reader still meets the rules
 * in the order they run where consequence does not separate two rows.
 */
function orderByConsequence(rows) {
  return [...rows].sort(
    (a, b) => Number(b.forcesDecision) - Number(a.forcesDecision) || a.pipelineIndex - b.pipelineIndex,
  );
}

const STATUS_ORDER = ['recommended', 'accepted', 'contested', 'legacy', 'deprecated'];

export function rankCandidates(candidates) {
  return [...candidates].sort((a, b) => {
    if (a.registryBacked !== b.registryBacked) return a.registryBacked ? -1 : 1;
    return STATUS_ORDER.indexOf(a.status) - STATUS_ORDER.indexOf(b.status);
  });
}

/* Preferred opening selection for a slot that is allowed to have one. A slot that forces a
 * decision returns null and stays unresolved until the user picks. */
export function preferredCandidate(slot) {
  if (slot.forcesDecision) return null;
  return rankCandidates(slot.available)[0] || null;
}

/* Parameters start at their published default, and the default names the source that chose
 * it. On a slot that forces a decision, a required parameter carrying more than one
 * published value is left unset too, because picking one silently is exactly the behaviour
 * the registry exists to document. */
export function initialParameters(candidate, forcesDecision) {
  const values = {};
  const unresolved = [];
  for (const parameter of candidate?.method?.parameter || []) {
    const choices = (parameter.published_values || []).filter(Number.isFinite);
    if (forcesDecision && parameter.required && choices.length > 1) {
      unresolved.push(parameter.name);
    } else if (parameter.default != null) {
      values[parameter.name] = parameter.default;
    }
  }
  return { values, unresolved };
}

/* Every axis the spread view can sweep. Parameter axes come from the published values the
 * registry records; the method axis comes from the runnable rules. Nothing here is
 * invented. A row reached through `derived` sweeps under the construct id the request
 * carries it by, which is the same key the sweep resolves it against. */
export function availableAxes(slot, candidate) {
  const axes = [];

  if (slot.available.length > 1) {
    axes.push({
      id: `${slot.key}:__method__`,
      slot: slot.key,
      methodIds: slot.available.map((entry) => entry.id),
      label: `${slot.title}: the rule itself`,
      unit: '',
      note: `${slot.available.length} rules`,
      display: `${slot.available.length} rules`,
    });
  }

  for (const parameter of candidate?.method?.parameter || []) {
    const values = (parameter.published_values || []).filter(Number.isFinite);
    if (values.length < 2) continue;
    axes.push({
      id: `${slot.key}:${parameter.name}`,
      slot: slot.key,
      parameter: parameter.name,
      values,
      label: `${slot.title}: ${parameter.name}`,
      unit: parameter.unit || '',
      note: `${values.length} published values`,
      display: values.join(', '),
    });
  }
  return axes;
}

/* The weighing rules each name the window's length on their own registry row, and dragging
 * the window on the trace sets that same parameter. */
export function windowLengthParameter(candidate) {
  return (candidate?.method?.parameter || []).find((parameter) => parameter.unit === 'seconds')?.name || null;
}

/* Every value the registry publishes for gravity, read from the entry that declares the
 * parameter. A list written here instead offered three of the four the registry carries,
 * and a sweep that omits a published value is reporting a narrower disagreement than the
 * literature holds. */
function publishedGravityValues() {
  for (const method of loaded?.methods || []) {
    for (const parameter of method.parameter || []) {
      if (parameter.name === 'gravity' && parameter.published_values?.length) {
        return parameter.published_values;
      }
    }
  }
  return [];
}

export const GRAVITY_AXIS = {
  id: 'global:gravity',
  slot: 'global',
  parameter: 'gravity_meters_per_second_squared',
  get values() {
    return publishedGravityValues();
  },
  label: 'Gravity',
  unit: 'm/s2',
  note: 'the tools disagree on this constant',
  get display() {
    return publishedGravityValues().join(', ');
  },
};

export function findMethod(registry, id) {
  return registry.methods.find((method) => method.id === id) || null;
}
