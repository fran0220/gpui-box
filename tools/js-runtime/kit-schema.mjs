// Explicit adapter contracts, not the catalog. An absent component is unsupported.
const string = { type: 'string', max: 16384 };
const identity = { type: 'string', min: 1, max: 256 };
const boolean = { type: 'boolean' };
const number = { type: 'number', min: -1e9, max: 1e9 };
const positive = { type: 'number', min: Number.MIN_VALUE, max: 1e9 };
const integer = { type: 'number', integer: true, min: 0, max: 1000000 };
const choice = (...values) => ({ enum: values });
const array = (items) => ({ type: 'array', items, max: 1024 });
const object = (fields, required = []) => ({ type: 'object', fields, required });
const common = { disabled: boolean, size: choice('xs', 'sm', 'md', 'lg') };
const labeled = { ...common, label: string, description: string };
const selectionItem = object({ id: identity, label: string, disabled: boolean }, ['id', 'label']);

export const kitSchemas = Object.freeze({
  Checkbox: { props: object({ ...labeled, checked: choice(true, false, null) }), events: { change: boolean } },
  Radio: { props: object({ ...labeled, selected: boolean }), events: { select: choice(null) } },
  Switch: { props: object({ ...labeled, name: string, on: boolean, invalid: boolean }), events: { change: boolean } },
  Slider: { props: object({ ...common, label: string, min: number, max: number, value: number, high: number, step: positive, length: positive, display: string, orientation: choice('horizontal', 'vertical'), marks: array(number) }), events: { change: number, rangeChange: object({ low: number, high: number }, ['low', 'high']) } },
  SegmentedControl: { props: object({ ...common, label: string, segments: array(selectionItem), selected: identity }), events: { select: identity } },
  TextInput: { props: object({ ...common, text: string, name: string, placeholder: string, invalid: boolean, required: boolean, readOnly: boolean, secret: boolean, bare: boolean, maxLength: integer }), events: { change: string, submit: choice(null), cancel: choice(null), backspaceAtStart: choice(null), focus: choice(null), blur: choice(null), clipboardDenied: choice('missingOwner', 'denied') } },
  Select: { props: object({ ...common, options: array(selectionItem), selected: { ...identity, nullable: true }, name: string, placeholder: string, invalid: boolean, clearable: boolean }), events: { change: { ...identity, nullable: true }, open: choice(null), close: choice(null) } },
  Pagination: { props: object({ ...common, page: { ...integer, min: 1 }, totalPages: { ...integer, min: 1 }, hasNext: boolean, siblings: integer }), events: { select: { ...integer, min: 1 } } },
  Tabs: { props: object({ ...common, tabs: array(object({ ...selectionItem.fields, badge: string, closable: boolean }, ['id', 'label'])), selected: identity, capsules: boolean, scrolling: boolean, overflowAfter: integer }), events: { select: identity, close: identity } },
  Accordion: { props: object({ size: common.size, sections: array(object({ id: identity, title: string, description: string, disabled: boolean }, ['id', 'title'])), expanded: array(identity), exclusive: boolean }), events: { toggle: object({ id: identity, expanded: boolean }, ['id', 'expanded']) }, slotIds: 'sections' },
  ScrollArea: { props: object({ axis: choice('vertical', 'horizontal', 'both'), label: string, width: positive, height: positive, fitHeight: boolean }), events: {}, slots: ['content'] },
  SplitPane: { props: object({ axis: choice('horizontal', 'vertical'), ratio: { type: 'number', min: 0, max: 1 }, minStart: { ...number, min: 0 }, minEnd: { ...number, min: 0 }, step: positive, collapsible: boolean, handleLabel: string }), events: { resize: { type: 'number', min: 0, max: 1 }, collapse: choice('start', 'end') }, slots: ['start', 'end'] },
  Divider: { props: object({ label: string, axis: choice('horizontal', 'vertical') }), events: {} },
  List: { props: object({ ...common, rows: array(object({ ...selectionItem.fields, within: identity }, ['id', 'label'])), selected: identity, rowHeight: positive, visibleRows: { ...integer, min: 1 }, flowing: boolean, anchoredToEnd: boolean, fills: boolean, arriving: boolean, reorderable: boolean }), events: { select: identity, reorder: object({ id: identity, source: identity, anchor: identity, position: choice('before', 'after', 'into') }, ['id', 'source', 'anchor', 'position']) }, slotIds: 'rows' },
  Popover: { props: object({ trigger: string, placement: choice('above', 'below'), hang: choice('start', 'end'), dismissable: boolean }), events: { open: choice(null), close: choice(null), dismiss: choice(null) }, slots: ['content'] },
  Dialog: { props: object({ title: string, description: string, confirmLabel: string, cancelLabel: string, destructive: boolean, dismissable: boolean }), events: { open: choice(null), close: choice(null), confirm: choice(null), cancel: choice(null), dismiss: choice(null) }, slots: ['content'] },
});

export function validateValue(value, schema, path = 'value') {
  if (value === null && schema.nullable) return;
  if (schema.enum) {
    if (!schema.enum.includes(value)) throw new TypeError(`${path}: invalid choice`);
    return;
  }
  if (schema.type === 'array') {
    if (!Array.isArray(value) || value.length > schema.max) throw new TypeError(`${path}: invalid array`);
    for (let i = 0; i < value.length; i++) {
      const item = Object.getOwnPropertyDescriptor(value, String(i));
      if (!item || !Object.hasOwn(item, 'value')) throw new TypeError(`${path}: sparse arrays and accessors not permitted`);
      validateValue(item.value, schema.items, `${path}[${i}]`);
    }
    if (schema.items.fields?.id && new Set(value.map(item => item.id)).size !== value.length) throw new TypeError(`${path}: duplicate identity`);
    return;
  }
  if (schema.type === 'object') {
    if (!value || Object.getPrototypeOf(value) !== Object.prototype) throw new TypeError(`${path}: expected plain object`);
    for (const key of Reflect.ownKeys(value)) {
      if (typeof key !== 'string' || !Object.hasOwn(schema.fields, key)) throw new TypeError(`${path}: unknown field ${String(key)}`);
      const descriptor = Object.getOwnPropertyDescriptor(value, key);
      if (!Object.hasOwn(descriptor, 'value')) throw new TypeError(`${path}.${key}: accessor not permitted`);
      validateValue(descriptor.value, schema.fields[key], `${path}.${key}`);
    }
    for (const key of schema.required) if (!Object.hasOwn(value, key)) throw new TypeError(`${path}.${key}: required`);
    return;
  }
  if (typeof value !== schema.type) throw new TypeError(`${path}: expected ${schema.type}`);
  if (schema.type === 'string' && (value.length > schema.max || value.length < (schema.min ?? 0))) throw new TypeError(`${path}: invalid string length`);
  if (schema.type === 'number' && (!Number.isFinite(value) || value < schema.min || value > schema.max || (schema.integer && !Number.isSafeInteger(value)))) throw new TypeError(`${path}: invalid number`);
}

export function validateKitProps(component, id, props) {
  if (!Object.hasOwn(kitSchemas, component)) throw new TypeError(`Unsupported Kit component: ${component}`);
  validateValue(id, identity, 'id');
  validateValue(props, kitSchemas[component].props, `${component}.props`);
  if (component === 'Slider') {
    const { min = 0, max = 1, value = min, high } = props;
    if (min >= max || value < min || value > max || (high !== undefined && (high < value || high > max))) throw new TypeError('Slider: invalid range');
  }
  if (component === 'List') {
    const parents = new Map((props.rows ?? []).map(row => [row.id, row.within]));
    for (const id of parents.keys()) {
      const seen = new Set([id]);
      for (let parent = parents.get(id); parent !== undefined; parent = parents.get(parent)) {
        if (!parents.has(parent) || seen.has(parent)) throw new TypeError('List: invalid row parent');
        seen.add(parent);
      }
    }
  }
}

// The tree validator owns recursive slots, semantic-id uniqueness and aggregate budgets.
export function validateKitSlots(component, props, slots) {
  if (!slots || Object.getPrototypeOf(slots) !== Object.prototype) throw new TypeError('Expected slots object');
  const schema = kitSchemas[component];
  const allowed = new Set(schema.slots ?? []);
  if (schema.slotIds) for (const item of props[schema.slotIds] ?? []) allowed.add(item.id);
  for (const name of Reflect.ownKeys(slots)) {
    const value = Object.getOwnPropertyDescriptor(slots, name)?.value;
    if (!allowed.has(name) || !Array.isArray(value) || value.length > 1024) throw new TypeError(`slots: unknown field or invalid slot ${String(name)}`);
  }
}

export function validateKitDescriptor(node) {
  if (node.kind !== 'kit') throw new TypeError('Expected Kit descriptor');
  validateKitProps(node.component, node.id, node.props);
  validateKitSlots(node.component, node.props, node.slots);
  const events = Object.fromEntries(Object.keys(kitSchemas[node.component].events).map(name => [name, identity]));
  validateValue(node.events, object(events), 'events');
  if (node.props.disabled && Object.keys(node.events).length) throw new TypeError('Disabled control has actions');
  return node;
}

// Only methods with native implementations belong here. Framework parameters
// and Entity/Signal handles never cross this data-only boundary.
const method = (fields, result = choice(null)) => ({ args: object(fields, Object.keys(fields)), result });
export const kitMethods = Object.freeze({
  TextInput: {
    invoke: {
      set_name: method({ name: string }), set_placeholder: method({ placeholder: string }),
      set_value: method({ value: string }), set_text_quietly: method({ value: string }),
      set_secret: method({ secret: boolean }), set_bare: method({ bare: boolean }),
      set_max_length: method({ max_length: { ...integer, nullable: true } }),
      set_disabled: method({ disabled: boolean }), set_read_only: method({ read_only: boolean }),
      set_required: method({ required: boolean }), set_invalid: method({ invalid: boolean }),
      set_control_size: method({ size: common.size }),
    },
    query: {
      value: method({}, string), is_empty: method({}, boolean), is_disabled: method({}, boolean),
      is_secret: method({}, boolean), selected_range: method({}, object({ start: integer, end: integer }, ['start', 'end'])),
      cursor_offset: method({}, integer),
    },
  },
  Select: {
    invoke: {
      set_name: method({ name: string }), set_placeholder: method({ placeholder: { ...string, nullable: true } }),
      set_options: method({ options: array(selectionItem) }), set_selected: method({ id: { ...identity, nullable: true } }),
      set_disabled: method({ disabled: boolean }), set_invalid: method({ invalid: boolean }),
      set_clearable: method({ clearable: boolean }), set_control_size: method({ size: common.size }),
    },
    query: {
      selected_id: method({}, { ...identity, nullable: true }), is_open: method({}, boolean),
      is_disabled: method({}, boolean),
      selected_option: method({}, { ...object({ ...selectionItem.fields, description: { ...string, nullable: true }, group: { ...string, nullable: true } }, ['id', 'label', 'disabled', 'description', 'group']), nullable: true }),
    },
  },
  Popover: {
    invoke: { open: method({}), close: method({}), toggle: method({}), dismiss: method({}), set_trigger: method({ label: string }), set_dismissable: method({ dismissable: boolean }), set_placement: method({ placement: choice('above', 'below') }), set_hang: method({ hang: choice('start', 'end') }) },
    query: { is_open: method({}, boolean), is_dismissable: method({}, boolean) },
  },
  Dialog: {
    invoke: { open: method({}), close: method({}), confirm: method({}), cancel: method({}), dismiss: method({}), set_title: method({ title: string }), set_description: method({ description: { ...string, nullable: true } }), set_confirm_label: method({ label: { ...string, nullable: true } }), set_cancel_label: method({ label: { ...string, nullable: true } }), set_dismissable: method({ dismissable: boolean }), set_destructive: method({ destructive: boolean }) },
    query: { is_open: method({}, boolean), is_dismissable: method({}, boolean) },
  },
});

export function validateInvocation(component, name, args, mode) {
  if (mode !== 'invoke' && mode !== 'query') throw new TypeError('Unknown Kit invocation mode');
  const methods = Object.hasOwn(kitMethods, component) && kitMethods[component][mode];
  if (!methods || !Object.hasOwn(methods, name)) throw new TypeError(`Unsupported Kit ${mode}: ${component}.${name}`);
  validateValue(args, methods[name].args, `${component}.${name}`);
  return methods[name];
}

/** Source-derived method contracts for kit-sdk.d.ts; no catalog-only methods. */
export function generateKitMethodTypes() {
  function type(schema) {
    let result;
    if (schema.enum) result = schema.enum.map(value => JSON.stringify(value)).join(' | ');
    else if (schema.type === 'array') result = `Array<${type(schema.items)}>`;
    else if (schema.type === 'object') result = Object.keys(schema.fields).length
      ? `{ ${Object.entries(schema.fields).map(([key, value]) => `${JSON.stringify(key)}${schema.required.includes(key) ? '' : '?'}: ${type(value)}`).join('; ')} }`
      : 'Record<string, never>';
    else result = schema.type;
    return schema.nullable ? `${result} | null` : result;
  }
  const contracts = Object.entries(kitMethods).map(([component, modes]) => `  ${component}: {\n${Object.entries(modes).map(([mode, methods]) => `    ${mode}: {\n${Object.entries(methods).map(([name, schema]) => `      ${name}: { args: ${type(schema.args)}; result: ${type(schema.result)} };`).join('\n')}\n    };`).join('\n')}\n  };`).join('\n');
  return [
    '// Generated from kitMethods by generateKitMethodTypes.',
    `export interface KitMethodContracts {\n${contracts}\n}`,
    'type MethodArguments<S> = S extends { args: infer A } ? {} extends A ? [args?: A] : [args: A] : never;',
    'type MethodResult<S> = S extends { result: infer R } ? R : never;',
    "export type KitInvoke = <C extends keyof KitMethodContracts, M extends keyof KitMethodContracts[NoInfer<C>]['invoke']>(target: Pick<KitNode<C>, 'id' | 'component'>, method: M, ...args: MethodArguments<KitMethodContracts[C]['invoke'][M]>) => Promise<MethodResult<KitMethodContracts[C]['invoke'][M]>>;",
    "export type KitQuery = <C extends keyof KitMethodContracts, M extends keyof KitMethodContracts[NoInfer<C>]['query']>(target: Pick<KitNode<C>, 'id' | 'component'>, method: M, ...args: MethodArguments<KitMethodContracts[C]['query'][M]>) => Promise<MethodResult<KitMethodContracts[C]['query'][M]>>;",
    '',
  ].join('\n');
}
