/*
 * Turning the registry into the decisions the interface presents.
 *
 * Nothing in here hardcodes a method. The slots below name constructs, the candidates come
 * from whatever the compiled registry contains for that construct, and the surfacing field
 * on each entry decides how hard the choice is pushed at the user.
 */

export const SLOTS = [
  {
    key: 'weighing',
    construct: 'system_weight',
    title: 'Weighing epoch',
    why: 'Sets system weight, and therefore the onset band and every impulse below it.',
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
    why: 'Moves jump height by about 1.4 cm across six published rules.',
  },
];

/* Entries the registry rules out as user choices. They are listed with their reason rather
 * than dropped, because a choice removed without explanation is indistinguishable from a
 * choice nobody thought of. */
const NOT_A_CHOICE = new Set(['never_a_user_choice', 'refuse']);

export function buildDecisionModel(registry, executableIds) {
  const executable = new Set(executableIds);
  const byConstruct = new Map();
  for (const method of registry.methods) {
    if (!byConstruct.has(method.construct)) byConstruct.set(method.construct, []);
    byConstruct.get(method.construct).push(method);
  }

  return SLOTS.map((slot) => {
    const all = byConstruct.get(slot.construct) || [];
    const withheld = all.filter((m) => NOT_A_CHOICE.has(m.gui?.surfacing));
    const offered = all.filter((m) => !NOT_A_CHOICE.has(m.gui?.surfacing));
    const available = offered.filter((m) => executable.has(m.id));
    const unavailable = offered.filter((m) => !executable.has(m.id));
    const forcesDecision = offered.some((m) => m.gui?.surfacing === 'force_a_decision');

    return {
      ...slot,
      construct: registry.constructs.find((c) => c.id === slot.construct) || null,
      all,
      offered,
      available,
      unavailable,
      withheld,
      forcesDecision,
      surfacing: dominantSurfacing(offered),
      fallbackId: null,
    };
  });
}

/* The loudest surfacing among the candidates wins, because a slot is only as quiet as its
 * most consequential entry. */
function dominantSurfacing(methods) {
  const order = ['force_a_decision', 'default_and_show', 'surface_on_demand', 'default_and_hide'];
  for (const level of order) {
    if (methods.some((m) => m.gui?.surfacing === level)) return level;
  }
  return 'default_and_show';
}

/* Preferred opening selection for a slot that is allowed to have one. A slot that forces a
 * decision returns null and stays unresolved until the user picks. */
export function preferredMethod(slot) {
  if (slot.forcesDecision) return null;
  const byStatus = ['recommended', 'accepted', 'contested', 'legacy', 'deprecated'];
  const ranked = [...slot.available].sort(
    (a, b) => byStatus.indexOf(a.status) - byStatus.indexOf(b.status),
  );
  return ranked[0] || null;
}

/* Parameters start at their published default, and the default names the source that chose
 * it. On a slot that forces a decision, a required parameter carrying more than one
 * published value is left unset too, because picking one silently is the behaviour the
 * registry exists to document. */
export function initialParameters(method, forcesDecision) {
  const values = {};
  const unresolved = [];
  for (const parameter of method?.parameter || []) {
    const choices = parameter.published_values || [];
    const mustChoose = forcesDecision && parameter.required && choices.length > 1;
    if (mustChoose) {
      unresolved.push(parameter.name);
    } else if (parameter.default != null) {
      values[parameter.name] = parameter.default;
    } else if (choices.length) {
      values[parameter.name] = choices[0];
    }
  }
  return { values, unresolved };
}

/* Every axis the spread view can sweep: the published values of each bound parameter, plus
 * the weighing epoch durations the literature uses. Nothing invented. */
export function availableAxes(slot, method) {
  const axes = [];
  for (const parameter of method?.parameter || []) {
    const values = (parameter.published_values || []).filter((v) => Number.isFinite(v));
    if (values.length < 2) continue;
    axes.push({
      id: `${slot.key}:${parameter.name}`,
      slot: slot.key,
      parameter: parameter.name,
      values,
      label: `${slot.title}: ${parameter.name}`,
      unit: parameter.unit || '',
      note: `${values.length} published values`,
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
  unit: 'seconds',
  note: 'threshold width moves +/-72% at 0.5 s and +/-23% at 1.5 s',
};

export function statusRank(status) {
  return { recommended: 'ok', accepted: 'accent', contested: 'warning', legacy: 'quiet', deprecated: 'danger' }[status] || 'quiet';
}

export function findMethod(registry, id) {
  return registry.methods.find((method) => method.id === id) || null;
}
