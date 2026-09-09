import { iconSchema } from './kit-icon-schema.mjs';
import { agentSnapshotSchema, tintSchema, expressionSchema, effectPlanSchema, imageSchema, validateProps as validateImages } from './kit-agent-schema.mjs';
export { expressionSchema, effectPlanSchema } from './kit-agent-schema.mjs';
const string = { type: 'string', max: 16384 };
const id = { type: 'string', min: 1, max: 256 };
const choice = (...values) => ({ enum: values });
const object = (fields, required = Object.keys(fields)) => ({ type: 'object', fields, required });
const oneOf = (...branches) => ({ oneOf: branches });
const array = items => ({ type: 'array', items, max: 1024 });
const fraction = { type: 'number', min: 0, max: 1 };
const count = { type: 'number', min: 0, max: 1000000, integer: true };
const elapsed = { type: 'number', min: 0, max: 86400000, integer: true };
const unavailable = object({ kind: choice('unavailable'), reason: string });
const ability = object({ id, label: string, detail: string, shortcut: string, cost: string, icon: iconSchema,
  charges: object({ current: count, maximum: { ...count, min: 1 } }),
  state: oneOf(object({ kind: choice('ready') }), object({ kind: choice('cooling-down'), remaining: string, remainingFraction: fraction }), object({ kind: choice('disabled', 'unavailable'), reason: string })),
}, ['id', 'label', 'state']);
const objective = object({ id, title: string, detail: string, parent: id, progress: fraction,
  state: oneOf(object({ kind: choice('locked', 'active', 'completed') }), object({ kind: choice('failed', 'unavailable'), reason: string })),
}, ['id', 'title', 'state']);
const gauge = object({ id, label: string, state: oneOf(object({ kind: choice('unknown') }), object({ kind: choice('known'), fraction, display: string }), unavailable) });
const reward = object({ id, title: string, detail: string,
  items: array(object({ id, label: string, detail: string, quantity: count, icon: iconSchema, image: imageSchema }, ['id', 'label', 'quantity'])),
  state: oneOf(object({ kind: choice('hidden', 'revealed', 'claimed') }), unavailable),
}, ['id', 'title', 'items', 'state']);
export const familySchemas = Object.freeze({
  AbilityBar: { props: object({ abilities: array(ability), selected: id }, ['abilities']), events: { activate: id } },
  ObjectiveTracker: { props: object({ objectives: array(objective), selected: id }, ['objectives']), events: { select: id } },
  PartyRoster: { props: object({ members: array(object({ agent: agentSnapshotSchema, image: imageSchema, expression: expressionSchema, tint: tintSchema, gauges: array(gauge) }, ['agent', 'gauges'])), selected: id }, ['members']), events: { selectMember: id } },
  RewardReveal: { props: object({ reward, effect: effectPlanSchema, sampleAt: elapsed }, ['reward']), events: { revealRequested: id, claimRequested: id } },
  EffectParticles: { props: object({ plan: effectPlanSchema, sampleAt: elapsed }, ['plan']), events: {} },
  CinematicEffect: { props: object({ plan: effectPlanSchema, sampleAt: elapsed, unavailable: object({ kind: choice('runtime-unavailable', 'invalid-limits', 'empty-asset', 'encoded-size', 'archive-invalid', 'archive-entries', 'archive-entry-size', 'archive-expanded-size', 'archive-compression-ratio', 'animation-count', 'state-machine-count', 'image-count', 'image-size', 'canvas-size', 'frame-rate', 'frame-count', 'duration', 'unsupported-feature', 'invalid-input', 'render-failed'), detail: string }) }, ['plan', 'unavailable']), events: {} },
  MicroMark: { props: object({ kind: choice('heartbeat', 'bounce', 'wobble', 'pop', 'sparkle'), label: string }), events: {} },
});
export const familyMethods = Object.freeze({});
// Called only after the closed structural schema succeeds.
export function validateProps(component, props) {
  validateImages(component, props);
  if (component === 'AbilityBar') for (const ability of props.abilities) {
    if (ability.charges && ability.charges.current > ability.charges.maximum) throw new TypeError('AbilityBar: charges exceed maximum');
  }
}
