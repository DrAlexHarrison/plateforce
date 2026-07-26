/*
 * Turning the registry into the decisions the interface presents.
 *
 * Nothing here hardcodes a method. The slots name constructs, the candidates are the
 * union of what the registry documents and what this build can execute, and the surfacing
 * field on each entry decides how hard the choice is pushed at the user.
 *
 * The union matters in both directions. A registry entry this build cannot run is offered
 * as unavailable rather than omitted, and a rule this build can run that the registry has
 * not documented is offered as not registry backed rather than presented as citable.
 *
 * Not registry backed splits two ways. A composition binds an operator on a registry row
 * and inherits its citations. An unfiled rule has no row anywhere.
 */

export const SLOTS = [
  {
    key: 'weighing',
    construct: 'system_weight',
    title: 'Weighing epoch',
    why: 'Sets system weight, and with it the onset band and every impulse below.',
  },
  {
    key: 'onset',
    construct: 'movement_onset',
    title: 'Movement onset',
    why: 'Net impulse reliability runs from 0.984 to 0.479 across published onset rules on identical data.',
  },
  {
    key: 'takeoff',
    construct: 'takeoff',
    title: 'Takeoff',
    why: 'Moves jump height by about 1.4 cm across six published rules, and decides flight time outright.',
  },
];

/* Entries the registry rules out as user choices. They are listed with their reason rather
 * than dropped, because a choice removed without explanation is indistinguishable from a
 * choice nobody thought of. */
const NOT_A_CHOICE = new Set(['never_a_user_choice', 'refuse']);

export function buildDecisionModel(registry, bindings) {
  const byConstruct = new Map();
  for (const method of registry.methods) {
    if (!byConstruct.has(method.construct)) byConstruct.set(method.construct, []);
    byConstruct.get(method.construct).push(method);
  }

  return SLOTS.map((slot) => {
    const documented = byConstruct.get(slot.construct) || [];
    const slotBindings = bindings.filter((binding) => binding.slot === slot.key);
    const executableIds = new Set(slotBindings.map((binding) => binding.id));

    const withheld = documented.filter((method) => NOT_A_CHOICE.has(method.gui?.surfacing));
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

    for (const binding of slotBindings) {
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

    return {
      ...slot,
      constructEntry: registry.constructs.find((c) => c.id === slot.construct) || null,
      candidates,
      available: candidates.filter((candidate) => candidate.executable),
      unavailable: candidates.filter((candidate) => !candidate.executable),
      withheld,
      forcesDecision: offered.some((method) => method.gui?.surfacing === 'force_a_decision'),
      surfacing: dominantSurfacing(offered),
    };
  });
}

/* The loudest surfacing among the candidates wins, because a slot is only as quiet as its
 * most consequential entry. */
function dominantSurfacing(methods) {
  const order = ['force_a_decision', 'default_and_show', 'surface_on_demand', 'default_and_hide'];
  for (const level of order) {
    if (methods.some((method) => method.gui?.surfacing === level)) return level;
  }
  return 'default_and_show';
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
    } else if (choices.length) {
      values[parameter.name] = choices[0];
    }
  }
  return { values, unresolved };
}

/* Every axis the spread view can sweep. Parameter axes come from the published values the
 * registry records; the method axis comes from what this build can execute. Nothing here
 * is invented. */
export function availableAxes(slot, candidate) {
  const axes = [];

  if (slot.available.length > 1) {
    axes.push({
      id: `${slot.key}:__method__`,
      slot: slot.key,
      methodIds: slot.available.map((entry) => entry.id),
      label: `${slot.title}: the rule itself`,
      unit: '',
      note: `${slot.available.length} rules this build can run`,
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

export const WEIGHING_DURATION_AXIS = {
  id: 'weighing:duration_seconds',
  slot: 'weighing',
  parameter: 'duration_seconds',
  values: [0.5, 1.0, 1.5, 2.0],
  label: 'Weighing epoch: duration',
  unit: 's',
  note: 'threshold width moves +/-72% at 0.5 s and +/-23% at 1.5 s',
  display: '0.5, 1, 1.5, 2',
};

export const GRAVITY_AXIS = {
  id: 'global:gravity',
  slot: 'global',
  parameter: 'gravity_meters_per_second_squared',
  values: [9.8, 9.80665, 9.81],
  label: 'Gravity',
  unit: 'm/s2',
  note: 'the tools disagree on this constant',
  display: '9.8, 9.80665, 9.81',
};

export function findMethod(registry, id) {
  return registry.methods.find((method) => method.id === id) || null;
}
