// Only implemented native surfaces are advertised. All values are data-only.
import { iconSchema } from './kit-icon-schema.mjs';
const string = { type: 'string', max: 16384 };
const identity = { ...string, min: 1, max: 256 };
const boolean = { type: 'boolean' };
const choice = (...values) => ({ enum: values });
const object = (fields, required = []) => ({ type: 'object', fields, required });
const array = items => ({ type: 'array', items, max: 1024 });
const unit = { type: 'number', min: 0, max: 1 };
const color = object({ h: unit, s: unit, l: unit, a: unit }, ['h', 's', 'l', 'a']);
const common = { disabled: boolean, size: choice('xs', 'sm', 'md', 'lg') };
const integer = { type: 'number', integer: true, min: 0, max: 1000000 };
const number = { type: 'number', min: -1e12, max: 1e12 };
const precision = { type: 'number', integer: true, min: 0, max: 12 };
const step = { ...number, min: Number.MIN_VALUE };
const method = (fields, result) => ({ args: object(fields, Object.keys(fields)), result });
const ground = choice('backdrop', 'canvas', 'sunken', 'panel', 'raised', 'overlay');
const variant = choice('primary', 'secondary', 'ghost', 'danger', 'link');
const join = choice('alone', 'leading', 'middle', 'trailing');
const colorChoice = object({ palette: identity, semantic: choice('accent', 'accentStrong', 'danger', 'warning', 'success', 'info'), custom: color });
const button = { ...common, accessibleName: string, semanticParent: identity, icon: iconSchema, variant: choice(...variant.enum, 'filled', 'light', 'subtle', 'default', 'transparent', 'white'), color: colorChoice, ground, join, loading: boolean };
const transferItems = array(object({ id: identity, label: string, disabled: boolean }, ['id', 'label']));
const keymapBinding = object({ id: identity, keystroke: string, conflict: string, provenance: string }, ['id', 'keystroke']);
const keymapCommand = object({ id: identity, label: string, context: string, defaults: array(string), bindings: array(keymapBinding), searchText: string, keywords: array(string), refusal: string }, ['id', 'label']);
const keymapResult = object({ ...keymapCommand.fields, context: { ...string, nullable: true }, refusal: { ...string, nullable: true }, bindings: array(object({ ...keymapBinding.fields, conflict: { ...string, nullable: true }, provenance: { ...string, nullable: true } }, Object.keys(keymapBinding.fields))) }, Object.keys(keymapCommand.fields));

export const familyBindings = Object.freeze({
  NumberInput: { prop: 'value', event: 'change' },
  Toggle: { prop: 'pressed', event: 'press' },
  ToggleGroup: { prop: 'pressed', event: 'change', project: 'pressed' },
});

export const familySchemas = Object.freeze({
  ButtonGroup: { props: object(common), events: {}, slots: ['buttons'] },
  KeymapEditor: { props: object({ disabled: boolean, commands: array(keymapCommand), query: string }), events: { addCaptured: object({ command_id: identity, keystroke: string }, ['command_id', 'keystroke']), remove: object({ command_id: identity, binding_id: identity }, ['command_id', 'binding_id']), reset: object({ command_id: identity }, ['command_id']), recordingCancelled: object({ command_id: identity }, ['command_id']) } },
  NumberInput: { props: object({ ...common, value: number, min: number, max: number, step, pageStep: step, precision, name: string, unit: string, prefix: string, required: boolean, invalid: boolean }), events: { change: number, unparsable: string, submit: choice(null) } },
  TransferList: { props: object({ ...common, source: transferItems, target: transferItems, sourceSelected: array(identity), targetSelected: array(identity), sourceLabel: string, targetLabel: string, query: string }), events: { toggleSource: identity, toggleTarget: identity, moveToTarget: choice(null), moveToSource: choice(null), queryChange: string } },
  SettingsRow: { props: object({ label: string, description: string, labelWidth: { type: 'number', min: 0, max: 100000 }, badge: string, value: string, searchTerms: array(string), managed: string }, ['label']), events: {}, slots: ['control'] },
  SearchInput: { props: object({ ...common, name: string, placeholder: string, value: string }), events: { change: string, submit: choice(null), cancel: choice(null), backspaceAtStart: choice(null), focus: choice(null), blur: choice(null) } },
  Button: { props: object({ ...button, label: string, accessibleDescription: string, iconOnly: boolean, iconPosition: choice('leading', 'trailing'), fullWidth: boolean, checkedState: boolean }), events: { click: choice(null) } },
  IconButton: { props: object(button, ['icon', 'accessibleName']), events: { click: choice(null) } },
  Toggle: { props: object({ ...common, label: string, accessibleName: string, semanticParent: identity, icon: iconSchema, iconOnly: boolean, variant, ground, join, pressed: boolean }), events: { press: boolean } },
  ToggleGroup: { props: object({ ...common, label: string, items: array(object({ id: identity, label: string, icon: iconSchema, iconOnly: boolean, disabled: boolean }, ['id', 'label'])), pressed: array(identity), selection: choice('any', 'atMostOne'), variant, ground }), events: { change: object({ pressed: array(identity), changed: identity }, ['pressed', 'changed']) } },
  ColorPicker: { props: object({ disabled: boolean, value: color, alpha: boolean, presets: array(color), recent: array(color) }, ['value']), events: { change: color } },
  ColorSwatch: { props: object({ disabled: boolean, color, selected: boolean }, ['color']), events: { click: color } },
  FormField: { props: object({ label: string, control: identity, description: string, validation: choice('pending', 'validating', 'invalid', 'valid'), reason: string, error: string, hint: string, required: boolean }, ['label']), events: {}, slots: ['content'] },
  FilterBar: { props: object({ ...common, conditions: array(object({ id: identity, field: string, operator: string, value: string, tone: choice('neutral', 'accent', 'success', 'warning', 'danger', 'info') }, ['id', 'field', 'operator', 'value'])), countState: choice('unknown', 'counting', 'known', 'unavailable'), count: integer, countReason: string, noun: string, addLabel: string, clearLabel: string }), events: { add: choice(null), remove: identity, clear: choice(null) }, slots: ['add_control'] },
});
export const familyMethods = Object.freeze({
  KeymapEditor: { invoke: { set_commands: method({ commands: array(keymapCommand) }, choice(null)), set_query: method({ query: string }, choice(null)), set_disabled: method({ disabled: boolean }, choice(null)) }, query: { current_commands: method({}, array(keymapResult)), active_command: method({}, { ...identity, nullable: true }), is_disabled: method({}, boolean) } },
  NumberInput: {
    invoke: {
      set_value: method({ value: number }, choice(null)),
      set_invalid: method({ invalid: boolean }, choice(null)),
      set_disabled: method({ disabled: boolean }, choice(null)),
      set_required: method({ required: boolean }, choice(null)),
      set_range: method({ min: { ...number, nullable: true }, max: { ...number, nullable: true } }, choice(null)),
      set_steps: method({ step, page_step: { ...step, nullable: true } }, choice(null)),
      set_precision: method({ precision }, choice(null)),
      set_presentation: method({ name: { ...string, nullable: true }, unit: { ...string, nullable: true }, prefix: { ...string, nullable: true }, size: common.size }, choice(null)),
    },
    query: {
      current: method({}, { ...number, nullable: true }), shown: method({}, { ...number, nullable: true }),
      is_disabled: method({}, boolean), is_invalid: method({}, boolean),
      invalid_reason: method({}, { ...string, nullable: true }), can_step: method({ delta: number }, boolean),
    },
  },
  TransferList: {
    invoke: {
      set_query: method({ query: string }, choice(null)),
      set_items: method({ source: transferItems, target: transferItems }, choice(null)),
      set_selection: method({ source: array(identity), target: array(identity) }, choice(null)),
      set_labels: method({ source: string, target: string }, choice(null)),
      set_control_size: method({ size: common.size }, choice(null)),
      set_disabled: method({ disabled: boolean }, choice(null)),
    },
    query: { is_disabled: method({}, boolean) },
  },
  SearchInput: {
    invoke: {
      set_value: method({ value: string }, choice(null)), set_name: method({ name: string }, choice(null)), set_placeholder: method({ placeholder: string }, choice(null)), set_disabled: method({ disabled: boolean }, choice(null)),
      set_presentation: method({ name: { ...string, nullable: true }, placeholder: { ...string, nullable: true }, size: common.size }, choice(null)),
    },
    query: { value: method({}, string), is_disabled: method({}, boolean) },
  },
  FormField: { invoke: {}, query: { is_invalid: method({}, boolean), is_validating: method({}, boolean) } },
});

// Called after closed-shape validation, in both worker and native host.
export function validateFamilyProps(component, props) {
  if (['Button', 'IconButton'].includes(component) && props.color && Object.keys(props.color).length !== 1) throw new TypeError('color requires exactly one source');
  if (props.iconOnly && (!props.icon || !props.accessibleName)) throw new TypeError('iconOnly requires icon and accessibleName');
  if (component === 'ToggleGroup') for (const item of props.items ?? []) {
    if (item.iconOnly && !item.icon) throw new TypeError('iconOnly item requires icon');
  }
  if (component === 'FormField' && props.validation === 'invalid' && props.reason === undefined && props.error === undefined) throw new TypeError('invalid form requires reason');
  if (component === 'FilterBar') {
    if (props.countState === 'known' && props.count === undefined) throw new TypeError('known count requires count');
    if (props.countState === 'unavailable' && props.countReason === undefined) throw new TypeError('unavailable count requires reason');
  }
}
