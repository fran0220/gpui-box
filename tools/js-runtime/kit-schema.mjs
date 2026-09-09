// Explicit adapter contracts, not the catalog. An absent component is unsupported.
import { selectOptionSchema as selectOption } from './kit-select-option-schema.mjs';
import { familySchemas as controlsSchemas, familyMethods as controlsMethods, validateFamilyProps as validateControls } from './kit-controls_extra-schema.mjs';
import { familySchemas as navigationSchemas, familyMethods as navigationMethods, validateFamilyProps as validateNavigation } from './kit-navigation_extra-schema.mjs';
import { familySchemas as layoutSchemas, familyMethods as layoutMethods, validateFamilyProps as validateLayout } from './kit-layout_extra-schema.mjs';
import { familySchemas as dateSchemas, familyMethods as dateMethods, validateFamilyProps as validateDataFamily } from './kit-datetime-schema.mjs';
import { familySchemas as displaySchemas, familyMethods as displayMethods, validateFamilyProps as validateDisplay } from './kit-display-schema.mjs';
import { familySchemas as chartsSchemas, familyMethods as chartsMethods, validateFamilyProps as validateCharts } from './kit-charts-schema.mjs';
import { familySchemas as agentSchemas, familyMethods as agentMethods, validateProps as validateAgent } from './kit-agent-schema.mjs';
import { familySchemas as gameSchemas, familyMethods as gameMethods, validateProps as validateGame } from './kit-game-effects-schema.mjs';
import { familySchemas as canvasSchemas, familyMethods as canvasMethods, validateCanvasProps } from './kit-canvas-schema.mjs';
import { familySchemas as overlaySchemas, familyMethods as overlayMethods, validateOverlayProps } from './kit-overlay-schema.mjs';
import { familySchemas as contentSchemas, familyMethods as contentMethods, validateDescriptor as validateContent } from './kit-content-schema.mjs';
import { familySchemas as mediaSchemas, familyMethods as mediaMethods, validateDescriptor as validateMedia } from './kit-media-schema.mjs';
import { familySchemas as dataSchemas, familyMethods as dataMethods, validateFamilyProps as validateDataViews } from './kit-data-schema.mjs';
import { familySchemas as structuredSchemas, familyMethods as structuredMethods, validateFamilyProps as validateStructured } from './kit-structured-schema.mjs';
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
const dropIntent = object({ id: identity, source: identity, label: string, kind: identity, anchor: identity, position: choice('before', 'after', 'into'), velocity: object({ x: number, y: number }, ['x', 'y']) }, ['id', 'source', 'label', 'kind', 'anchor', 'position', 'velocity']);

export const kitSchemas = Object.freeze({
  ...controlsSchemas, ...navigationSchemas, ...layoutSchemas, ...dateSchemas,
  ...displaySchemas, ...chartsSchemas, ...agentSchemas, ...gameSchemas, ...canvasSchemas,
  ...overlaySchemas, ...contentSchemas, ...mediaSchemas, ...dataSchemas, ...structuredSchemas,
  Checkbox: { props: object({ ...labeled, checked: choice(true, false, null) }), events: { change: boolean } },
  Radio: { props: object({ ...labeled, selected: boolean }), events: { select: choice(null) } },
  Switch: { props: object({ ...labeled, name: string, on: boolean, invalid: boolean }), events: { change: boolean } },
  Slider: { props: object({ ...common, label: string, min: number, max: number, value: number, high: number, step: positive, length: positive, display: string, orientation: choice('horizontal', 'vertical'), marks: array(number) }), events: { change: number, rangeChange: object({ low: number, high: number }, ['low', 'high']) } },
  SegmentedControl: { props: object({ ...common, label: string, segments: array(selectionItem), selected: identity }), events: { select: identity } },
  TextInput: { props: object({ ...common, text: string, name: string, placeholder: string, invalid: boolean, required: boolean, readOnly: boolean, secret: boolean, bare: boolean, maxLength: integer }), events: { change: string, submit: choice(null), cancel: choice(null), backspaceAtStart: choice(null), focus: choice(null), blur: choice(null), clipboardDenied: choice('missingOwner', 'denied') } },
  Select: { props: object({ ...common, options: array(selectOption), selected: { ...identity, nullable: true }, name: string, placeholder: string, invalid: boolean, clearable: boolean }), events: { change: { ...identity, nullable: true }, open: choice(null), close: choice(null) } },
  Pagination: { props: object({ ...common, page: { ...integer, min: 1 }, totalPages: { ...integer, min: 1 }, hasNext: boolean, siblings: integer }), events: { select: { ...integer, min: 1 } } },
  Tabs: { props: object({ ...common, tabs: array(object({ ...selectionItem.fields, badge: string, closable: boolean }, ['id', 'label'])), selected: identity, capsules: boolean, scrolling: boolean, overflowAfter: integer, reorderable: boolean }), events: { select: identity, close: identity, reorder: dropIntent }, predicates: { accepts: dropIntent } },
  Accordion: { props: object({ size: common.size, sections: array(object({ id: identity, title: string, description: string, disabled: boolean }, ['id', 'title'])), expanded: array(identity), exclusive: boolean }), events: { toggle: object({ id: identity, expanded: boolean }, ['id', 'expanded']) }, slotIds: 'sections' },
  ScrollArea: { props: object({ axis: choice('vertical', 'horizontal', 'both'), label: string, width: positive, height: positive, fitHeight: boolean }), events: {}, slots: ['content'] },
  SplitPane: { props: object({ axis: choice('horizontal', 'vertical'), ratio: { type: 'number', min: 0, max: 1 }, minStart: { ...number, min: 0 }, minEnd: { ...number, min: 0 }, step: positive, collapsible: boolean, handleLabel: string }), events: { resize: { type: 'number', min: 0, max: 1 }, collapse: choice('start', 'end') }, slots: ['start', 'end'] },
  Divider: { props: object({ label: string, axis: choice('horizontal', 'vertical') }), events: {} },
  List: { props: object({ ...common, rows: array(object({ ...selectionItem.fields, within: identity }, ['id', 'label'])), selected: identity, rowHeight: positive, visibleRows: { ...integer, min: 1 }, flowing: boolean, anchoredToEnd: boolean, fills: boolean, arriving: boolean, reorderable: boolean }), events: { select: identity, reorder: dropIntent }, slotIds: 'rows', predicates: { accepts: dropIntent } },
  Popover: { props: object({ trigger: string, placement: choice('above', 'below'), hang: choice('start', 'end'), dismissable: boolean }), events: { open: choice(null), close: choice(null), dismiss: choice(null) }, slots: ['content'] },
  Dialog: { props: object({ title: string, description: string, confirmLabel: string, cancelLabel: string, destructive: boolean, dismissable: boolean }), events: { open: choice(null), close: choice(null), confirm: choice(null), cancel: choice(null), dismiss: choice(null) }, slots: ['content'] },
});

// Budgets count data edges separately from schema/ref/union work. A failed
// oneOf branch consumes work too; exceeding either limit is never a mismatch.
export const schemaLimits = Object.freeze({ dataDepth: 32, work: 100000, schemaNodes: 4096, schemaDepth: 128, validationStack: 256 });
const definitionName = /^[A-Za-z_][A-Za-z0-9_]{0,63}$/;

function schemaDocument(root) {
  const plain = value => {
    if (!value || Object.getPrototypeOf(value) !== Object.prototype) throw new TypeError('Expected plain schema object');
    for (const key of Reflect.ownKeys(value)) {
      if (typeof key !== 'string' || !Object.hasOwn(Object.getOwnPropertyDescriptor(value, key), 'value')) throw new TypeError('Schema accessors not permitted');
    }
  };
  plain(root);
  const defs = root.$defs === undefined ? {} : root.$defs;
  plain(defs);
  for (const name of Object.keys(defs)) if (!definitionName.test(name)) throw new TypeError('Invalid local definition name');
  const nodes = new Set(), ancestors = new Set();
  let count = 0;
  const arrayValues = value => {
    if (!Array.isArray(value) || value.length > schemaLimits.schemaNodes) throw new TypeError('Invalid schema array');
    for (const key of Reflect.ownKeys(value)) {
      if (key !== 'length' && (typeof key !== 'string' || !/^(0|[1-9][0-9]*)$/.test(key))) throw new TypeError('Invalid schema array field');
    }
    return Array.from({ length: value.length }, (_, index) => {
      const item = Object.getOwnPropertyDescriptor(value, String(index));
      if (!item || !Object.hasOwn(item, 'value')) throw new TypeError('Schema accessors not permitted');
      return item.value;
    });
  };
  const visit = (schema, depth) => {
    plain(schema);
    if (depth > schemaLimits.schemaDepth) throw new TypeError('Schema depth exceeded');
    if (ancestors.has(schema)) throw new TypeError('Cyclic schema object; use local refs');
    ancestors.add(schema);
    nodes.add(schema);
    if (++count > schemaLimits.schemaNodes) throw new TypeError('Schema size exceeded');
    if (schema !== root && Object.hasOwn(schema, '$defs')) throw new TypeError('Definitions must belong to document root');
    if (Object.hasOwn(schema, '$ref')) {
      if (typeof schema.$ref !== 'string' || !definitionName.test(schema.$ref) || !Object.hasOwn(defs, schema.$ref)) throw new TypeError('Unknown local schema ref');
      if (Object.keys(schema).some(key => key !== '$ref' && key !== 'nullable' && !(schema === root && key === '$defs'))) throw new TypeError('Ref cannot have sibling constraints except nullable');
    } else if (schema.oneOf !== undefined) {
      if (!Array.isArray(schema.oneOf) || !schema.oneOf.length) throw new TypeError('Invalid oneOf schema');
      for (const branch of arrayValues(schema.oneOf)) visit(branch, depth + 1);
    } else if (schema.enum !== undefined) {
      if (!arrayValues(schema.enum).every(value => value === null || ['string', 'boolean', 'number'].includes(typeof value))) throw new TypeError('Expected primitive enum choices');
    } else if (schema.type === 'object') {
      plain(schema.fields);
      if (!arrayValues(schema.required).every(key => typeof key === 'string' && Object.hasOwn(schema.fields, key))) throw new TypeError('Invalid required fields');
      for (const field of Object.values(schema.fields)) visit(field, depth + 1);
    } else if (schema.type === 'array') {
      if (!Number.isSafeInteger(schema.max) || schema.max < 0) throw new TypeError('Invalid array max');
      visit(schema.items, depth + 1);
    }
    ancestors.delete(schema);
  };
  visit(root, 0);
  for (const schema of Object.values(defs)) visit(schema, 0);
  // Only refs and unions keep the same data value. Cycles along these edges
  // can never make progress, even if another branch would otherwise match.
  const active = new Set(), finished = new Set();
  const progress = (schema, depth) => {
    if (active.has(schema)) throw new TypeError('Non-progressing schema ref cycle');
    if (finished.has(schema)) return;
    if (depth > schemaLimits.schemaDepth) throw new TypeError('Schema ref depth exceeded');
    active.add(schema);
    if (schema.$ref !== undefined) progress(defs[schema.$ref], depth + 1);
    else for (const branch of schema.oneOf ?? []) progress(branch, depth + 1);
    active.delete(schema);
    finished.add(schema);
  };
  for (const schema of nodes) progress(schema, 0);
  return defs;
}

class SchemaLimitError extends TypeError {}

export function validateValue(value, schema, path = 'value') {
  const context = { defs: schemaDocument(schema), work: 0, stack: 0 };
  validateData(value, schema, path, context, 0);
}

function validateData(value, schema, path, context, depth) {
  if (depth > schemaLimits.dataDepth || ++context.work > schemaLimits.work || ++context.stack > schemaLimits.validationStack) throw new SchemaLimitError(`${path}: schema validation budget exceeded`);
  try { return validateDataInner(value, schema, path, context, depth); }
  finally { context.stack--; }
}

function validateDataInner(value, schema, path, context, depth) {
  if (value === null && schema.nullable === true) return;
  if (schema.$ref !== undefined) return validateData(value, context.defs[schema.$ref], path, context, depth);
  if (schema.oneOf !== undefined) {
    if (!Array.isArray(schema.oneOf) || !schema.oneOf.length) throw new TypeError(`${path}: invalid oneOf schema`);
    let matches = 0;
    for (const branch of schema.oneOf) {
      try { validateData(value, branch, path, context, depth); matches++; } catch (error) {
        if (!(error instanceof TypeError) || error instanceof SchemaLimitError) throw error;
      }
    }
    if (matches !== 1) throw new TypeError(`${path}: expected exactly one matching branch`);
    return;
  }
  if (schema.enum) {
    if (!schema.enum.includes(value)) throw new TypeError(`${path}: invalid choice`);
    return;
  }
  if (schema.type === 'array') {
    if (!Array.isArray(value) || value.length > schema.max) throw new TypeError(`${path}: invalid array`);
    let itemSchema = schema.items;
    while (itemSchema.$ref !== undefined) itemSchema = context.defs[itemSchema.$ref];
    const ids = new Set();
    for (let i = 0; i < value.length; i++) {
      const item = Object.getOwnPropertyDescriptor(value, String(i));
      if (!item || !Object.hasOwn(item, 'value')) throw new TypeError(`${path}: sparse arrays and accessors not permitted`);
      validateData(item.value, schema.items, `${path}[${i}]`, context, depth + 1);
      if (itemSchema.fields?.id) {
        const id = item.value && Object.getOwnPropertyDescriptor(item.value, 'id')?.value;
        if (typeof id !== 'string' || ids.has(id)) throw new TypeError(`${path}: invalid or duplicate identity`);
        ids.add(id);
      }
    }
    return;
  }
  if (schema.type === 'object') {
    if (!value || Object.getPrototypeOf(value) !== Object.prototype) throw new TypeError(`${path}: expected plain object`);
    const keys = Reflect.ownKeys(value);
    if (keys.some(key => typeof key !== 'string')) throw new TypeError(`${path}: unknown symbol field`);
    for (const key of keys.sort()) {
      if (typeof key !== 'string' || !Object.hasOwn(schema.fields, key)) throw new TypeError(`${path}: unknown field ${String(key)}`);
      const descriptor = Object.getOwnPropertyDescriptor(value, key);
      if (!Object.hasOwn(descriptor, 'value')) throw new TypeError(`${path}.${key}: accessor not permitted`);
      validateData(descriptor.value, schema.fields[key], `${path}.${key}`, context, depth + 1);
    }
    for (const key of schema.required) if (!Object.hasOwn(value, key)) throw new TypeError(`${path}.${key}: required`);
    return;
  }
  if (typeof value !== schema.type) throw new TypeError(`${path}: expected ${schema.type}`);
  if (schema.type === 'string') {
    if (!value.isWellFormed()) throw new TypeError(`${path}: invalid Unicode`);
    if (value.length > schema.max || value.length < (schema.min ?? 0)) throw new TypeError(`${path}: invalid string length`);
  }
  if (schema.type === 'number' && (!Number.isFinite(value) || value < schema.min || value > schema.max || (schema.integer && !Number.isSafeInteger(value)))) throw new TypeError(`${path}: invalid number`);
}

export function validateKitProps(component, id, props) {
  if (!Object.hasOwn(kitSchemas, component)) throw new TypeError(`Unsupported Kit component: ${component}`);
  validateValue(id, identity, 'id');
  validateValue(props, kitSchemas[component].props, `${component}.props`);
  if (Object.hasOwn(controlsSchemas, component)) validateControls(component, props);
  if (Object.hasOwn(navigationSchemas, component)) validateNavigation(component, props);
  if (Object.hasOwn(layoutSchemas, component)) validateLayout(component, props);
  if (Object.hasOwn(dateSchemas, component)) validateDataFamily(component, props);
  if (Object.hasOwn(displaySchemas, component)) validateDisplay(component, props);
  if (Object.hasOwn(chartsSchemas, component)) validateCharts(component, props);
  if (Object.hasOwn(agentSchemas, component)) validateAgent(component, props);
  if (Object.hasOwn(gameSchemas, component)) validateGame(component, props);
  if (Object.hasOwn(canvasSchemas, component)) validateCanvasProps(component, props);
  if (Object.hasOwn(overlaySchemas, component)) validateOverlayProps(component, props);
  if (Object.hasOwn(dataSchemas, component)) validateDataViews(component, props);
  if (Object.hasOwn(structuredSchemas, component)) validateStructured(component, props);
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
  validateSlots(kitSchemas[component], props, slots);
  if (Object.hasOwn(contentSchemas, component)) validateContent({ component, props, slots });
  if (Object.hasOwn(mediaSchemas, component)) validateMedia({ component, props, slots });
}

export function validateSlots(schema, props, slots) {
  if (!slots || Object.getPrototypeOf(slots) !== Object.prototype) throw new TypeError('Expected slots object');
  if (schema.slotPaths !== undefined && !Array.isArray(schema.slotPaths)) throw new TypeError('slots: invalid slot paths');
  const allowed = new Set(schema.slots ?? []);
  const own = (value, key) => {
    if (!value || typeof value !== 'object') return undefined;
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (descriptor && !Object.hasOwn(descriptor, 'value')) throw new TypeError('slots: accessor not permitted');
    return descriptor?.value;
  };
  for (const path of [...(schema.slotIds ? [schema.slotIds] : []), ...(schema.slotPaths ?? [])]) {
    const fields = typeof path === 'string' ? path.split('.') : [];
    if (!fields.length || fields.length > 32 || fields.some(field => !definitionName.test(field))) throw new TypeError('slots: invalid slot path');
    let values = [props];
    for (const field of fields) {
      const next = [];
      for (const value of values) {
        const child = own(value, field);
        if (Array.isArray(child)) for (let index = 0; index < child.length; index++) next.push(own(child, String(index)));
        else if (child !== undefined) next.push(child);
      }
      values = next;
    }
    for (const item of values) {
      const id = own(item, 'id');
      if (typeof id !== 'string') continue;
      if (schema.slotSuffixes) for (const suffix of schema.slotSuffixes) allowed.add(`${id}:${suffix}`);
      else allowed.add(id);
    }
  }
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
  const predicates = Object.fromEntries(Object.keys(kitSchemas[node.component].predicates ?? {}).map(name => [name, identity]));
  validateValue(node.predicates === undefined ? {} : node.predicates, object(predicates), 'predicates');
  if (node.props.disabled && Object.keys(node.predicates ?? {}).length) throw new TypeError('Disabled control has predicates');
  return node;
}

// Only methods with native implementations belong here. Framework parameters
// and Entity/Signal handles never cross this data-only boundary.
const method = (fields, result = choice(null)) => ({ args: object(fields, Object.keys(fields)), result });
export const kitMethods = Object.freeze({
  ...controlsMethods, ...navigationMethods, ...layoutMethods, ...dateMethods,
  ...displayMethods, ...chartsMethods, ...agentMethods, ...gameMethods, ...canvasMethods,
  ...overlayMethods, ...contentMethods, ...mediaMethods, ...dataMethods, ...structuredMethods,
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
      focus_handle: method({}, object({ $nativeRef: { type: 'string', min: 8, max: 27 }, type: choice('FocusHandle') }, ['$nativeRef', 'type'])),
    },
  },
  Select: {
    invoke: {
      set_name: method({ name: string }), set_placeholder: method({ placeholder: { ...string, nullable: true } }),
      set_options: method({ options: array(selectOption) }), set_selected: method({ id: { ...identity, nullable: true } }),
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

/** Prints the shared data schema grammar for family props and method contracts. */
export function schemaType(schema, definitionsName) {
  const defs = schemaDocument(schema);
  if (Object.keys(defs).length && (typeof definitionsName !== 'string' || !definitionName.test(definitionsName))) throw new TypeError('Named TypeScript definitions required');
  return printSchemaType(schema, definitionsName);
}

/** Emits each local definition once; recursive references remain named types. */
export function schemaDefinitions(schema, definitionsName) {
  const defs = schemaDocument(schema);
  if (!definitionName.test(definitionsName ?? '')) throw new TypeError('Invalid TypeScript definitions name');
  return `export interface ${definitionsName} {\n${Object.entries(defs).map(([name, value]) => `  ${JSON.stringify(name)}: ${printSchemaType(value, definitionsName)};`).join('\n')}\n}`;
}

function printSchemaType(schema, definitionsName) {
  let result;
  if (schema.$ref !== undefined) result = `${definitionsName}[${JSON.stringify(schema.$ref)}]`;
  else if (schema.oneOf) result = schema.oneOf.map(value => printSchemaType(value, definitionsName)).join(' | ');
  else if (schema.enum) result = schema.enum.map(value => JSON.stringify(value)).join(' | ');
  else if (schema.type === 'array') result = `Array<${printSchemaType(schema.items, definitionsName)}>`;
  else if (schema.type === 'object') result = Object.keys(schema.fields).length
    ? `{ ${Object.entries(schema.fields).map(([key, value]) => `${JSON.stringify(key)}${schema.required.includes(key) ? '' : '?'}: ${printSchemaType(value, definitionsName)}`).join('; ')} }`
    : 'Record<string, never>';
  else result = schema.type;
  return schema.nullable === true ? `${result} | null` : result;
}

/** Source-derived method contracts for kit-sdk.d.ts; no catalog-only methods. */
export function generateKitMethodTypes(methods = kitMethods) {
  const definitions = [];
  const type = schema => {
    if (!schema.$defs) return schemaType(schema);
    const name = `KitMethodDefinitions${definitions.length}`;
    definitions.push(schemaDefinitions(schema, name));
    return schemaType(schema, name);
  };
  const contracts = Object.entries(methods).map(([component, modes]) => `  ${component}: {\n${Object.entries(modes).map(([mode, methods]) => `    ${mode}: {\n${Object.entries(methods).map(([name, schema]) => `      ${name}: { args: ${type(schema.args)}; result: ${type(schema.result)} };`).join('\n')}\n    };`).join('\n')}\n  };`).join('\n');
  return [
    '// Generated from kitMethods by generateKitMethodTypes.',
    ...definitions,
    `export interface KitMethodContracts {\n${contracts}\n}`,
    'type MethodArguments<S> = S extends { args: infer A } ? {} extends A ? [args?: A] : [args: A] : never;',
    contracts.includes('"$nativeRef"')
      ? "type MethodResult<S> = S extends { result: infer R } ? R extends { $nativeRef: string; type: infer T extends keyof import('./reference-sdk.js').NativeReferenceContracts } ? import('./reference-sdk.js').NativeRef<T> : R : never;"
      : 'type MethodResult<S> = S extends { result: infer R } ? R : never;',
    "export type KitInvoke = <C extends keyof KitMethodContracts, M extends keyof KitMethodContracts[NoInfer<C>]['invoke']>(target: Pick<KitNode<C>, 'id' | 'component'>, method: M, ...args: MethodArguments<KitMethodContracts[C]['invoke'][M]>) => Promise<MethodResult<KitMethodContracts[C]['invoke'][M]>>;",
    "export type KitQuery = <C extends keyof KitMethodContracts, M extends keyof KitMethodContracts[NoInfer<C>]['query']>(target: Pick<KitNode<C>, 'id' | 'component'>, method: M, ...args: MethodArguments<KitMethodContracts[C]['query'][M]>) => Promise<MethodResult<KitMethodContracts[C]['query'][M]>>;",
    '',
  ].join('\n');
}
