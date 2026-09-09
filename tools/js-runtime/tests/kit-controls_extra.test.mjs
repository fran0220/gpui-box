import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { familySchemas, familyMethods, validateFamilyProps } from '../kit-controls_extra-schema.mjs';
import { validateValue } from '../kit-schema.mjs';

test('selection controls reuse full option metadata and exact native intent contracts',()=>{
  const options=[{id:'a',label:'Alpha',description:'First',group:'Letters',disabled:true},{id:'b',label:'Beta'}];
  for(const component of ['Combobox','MultiSelect']){
    validateValue({options},familySchemas[component].props);
    validateValue({options},familyMethods[component].invoke.set_options.args);
    assert.throws(()=>validateValue({options:[options[0],options[0]]},familyMethods[component].invoke.set_options.args));
    assert.throws(()=>validateValue({options:[{id:'x',label:'X',value:'invented'}]},familySchemas[component].props));
  }
  validateValue({id:'a',label:'Alpha',description:null,group:'Letters',disabled:true},familyMethods.Combobox.query.selected_option.result);
  validateValue(null,familyMethods.Combobox.query.selected_option.result);
  assert.throws(()=>validateValue({id:'a',label:'Alpha'},familyMethods.Combobox.query.selected_option.result));
  validateValue({max:null},familyMethods.TagInput.invoke.set_max.args);
  validateValue({visible:null},familyMethods.TagInput.invoke.set_collapse_at.args);
  assert.throws(()=>validateValue({max:-1},familyMethods.TagInput.invoke.set_max.args));
  validateValue({from:2,to:0},familySchemas.TagInput.events.moved);
  assert.throws(()=>validateValue(['a'],familySchemas.MultiSelect.events.toggled));
});

test('search counts preserve unavailable and incomplete answers and nested native events', () => {
  for (const count of [{state:'unsearched'},{state:'counting'},{state:'none'},{state:'known',total:7,current:2},{state:'tooMany',counted:500},{state:'unavailable',reason:'Refused'}]) {
    validateValue({count},familyMethods.SearchField.invoke.set_count.args);
    validateValue(count,familyMethods.FindReplace.query.count.result);
  }
  for (const count of [{state:'known',total:7},{state:'tooMany',total:500},{state:'unavailable'},{state:'none',total:0}]) assert.throws(() => validateValue({count},familySchemas.SearchField.props));
  validateValue({kind:'queryChanged',value:'fixture'},familySchemas.FindReplace.events.search);
  assert.throws(() => validateValue({kind:'next',value:'wrong'},familySchemas.FindReplace.events.search));
  validateValue({$nativeRef:'native-1',type:'SearchField'},familyMethods.FindReplace.query.search_field.result);
  assert.throws(() => validateValue({query:'fixture'},familyMethods.FindReplace.query.search_field.result));
});

test('sensitive controls reject invented authority and close their value/slot contracts', () => {
  for (const kind of ['PasswordInput','OneTimeCodeInput']) {
    validateValue({value:'fixture',readOnly:true},familySchemas[kind].props);
    assert.throws(() => validateValue({secret:false},familySchemas[kind].props));
    assert.throws(() => validateValue({provider:'remote'},familySchemas[kind].props));
    validateValue({name:null},familyMethods[kind].invoke.set_name.args);
    assert.throws(() => validateValue({readOnly:true},familyMethods[kind].invoke.set_read_only.args));
  }
  for (const slots of [0,13,1.5]) assert.throws(() => validateValue({slots},familySchemas.OneTimeCodeInput.props));
  validateValue({slots:12},familyMethods.OneTimeCodeInput.invoke.set_slots.args);
  assert.equal(familyMethods.PasswordInput.invoke.reveal,undefined);
});

test('keybinding recorder methods separate recording from caller binding and nullable options', () => {
  const methods = familyMethods.KeybindingRecorder;
  for (const [name, key] of [['set_label','label'],['set_placeholder','placeholder'],['set_binding','binding'],['set_conflict','reason']]) {
    validateValue({[key]:null}, methods.invoke[name].args);
    assert.throws(() => validateValue({}, methods.invoke[name].args));
  }
  assert.throws(() => validateValue({keystroke:'ctrl-k'}, methods.invoke.start.args));
  validateValue('ctrl-shift-k', familySchemas.KeybindingRecorder.events.captured);
  assert.throws(() => validateValue({keystroke:'ctrl-k'}, familySchemas.KeybindingRecorder.events.captured));
  assert.throws(() => validateValue({recording:true}, familySchemas.KeybindingRecorder.props));
});

test('inline edits have controlled sessions and data-only commit payloads', () => {
  validateValue({value:'Fixture',editing:true,multiline:true,rows:3,failure:'Save refused'}, familySchemas.InlineEdit.props);
  for (const props of [{rows:0},{rows:1.5},{rows:1025},{editing:'true'},{editor:{$nativeRef:'native-1',type:'TextInput'}}]) {
    assert.throws(() => validateValue(props, familySchemas.InlineEdit.props));
  }
  validateValue('Complete\ndocument', familySchemas.InlineEdit.events.commit);
  assert.throws(() => validateValue({text:'Document'}, familySchemas.InlineEdit.events.commit));
  assert.equal(familyMethods.InlineEdit, undefined);
});

test('native reference getters expose only actual focus and menu contracts', () => {
  for (const component of ['SearchInput', 'NumberInput', 'CopyButton', 'SplitButton']) {
    const query = familyMethods[component].query.focus_handle;
    validateValue({}, query.args);
    validateValue({$nativeRef:'native-1',type:'FocusHandle'}, query.result);
    assert.throws(() => validateValue({$nativeRef:'native-1',type:'TextInput'}, query.result));
    assert.throws(() => validateValue({$nativeRef:'native-1',type:'FocusHandle',pointer:1}, query.result));
    assert.throws(() => validateValue({extra:true}, query.args));
  }
  for (const component of ['TransferList', 'KeymapEditor']) {
    assert.equal(familyMethods[component].query.focus_handle, undefined);
  }
  const menu = familyMethods.SplitButton.query.menu;
  validateValue({$nativeRef:'native-2',type:'Menu'}, menu.result);
  assert.throws(() => validateValue({items:[]}, menu.result));
});

test('split buttons reuse closed recursive menu items and exact named methods', () => {
  const item = {kind:'check',id:'pin',label:'Pin',checked:true};
  validateValue({items:[{kind:'submenu',id:'more',label:'More',items:[item]}]},familySchemas.SplitButton.props);
  for (const items of [[{...item,checked:undefined}],[{...item,callback:'execute'}],[{kind:'separator',id:'separator',label:'Not allowed'}]]) {
    assert.throws(() => validateValue({items},familySchemas.SplitButton.props));
  }
  assert.throws(() => validateFamilyProps('SplitButton',{items:[{kind:'submenu',id:'pin',label:'More',items:[item]}]}));
  for (const [method,args] of [['set_icon',{icon:{key:'plus-circle',path:'/tmp/icon'}}],['set_menu_name',{menuName:'More'}],['open_menu',{items:[]}],['set_default_disabled',{disabled:null}]]) {
    assert.throws(() => validateValue(args,familyMethods.SplitButton.invoke[method].args));
  }
});

test('settings constructors require explicit titles and reject invented mutable methods', () => {
  validateValue({title:'Storage',labelWidth:160,dimmedBy:'Policy'},familySchemas.SettingsSection.props);
  for (const props of [{}, {title:'Storage',labelWidth:-1}, {title:'Storage',dimmedBy:true}, {title:'Storage',onChange:'callback'}]) {
    assert.throws(() => validateValue(props,familySchemas.SettingsSection.props));
  }
  assert.equal(familyMethods.SettingsList,undefined);
  assert.equal(familyMethods.SettingsSection,undefined);
});

test('copy contracts bound durations and prohibit serialized clipboard callbacks', () => {
  validateValue({text:'fixture',glyphOnly:'Copy value',confirmationMs:0}, familySchemas.CopyButton.props);
  for (const props of [{confirmationMs:-1},{confirmationMs:60001},{confirmationMs:0.5},{copier:'write'},{glyphOnly:''}]) {
    assert.throws(() => validateValue(props,familySchemas.CopyButton.props));
  }
  for (const [method,args] of [['copy',{text:'extra'}],['set_confirmation',{confirmationMs:30}],['set_glyph_only',{name:''}],['set_label',{}]]) {
    assert.throws(() => validateValue(args,familyMethods.CopyButton.invoke[method].args));
  }
  validateValue({state:'failed',reason:'Verification refused'},familyMethods.CopyButton.query.state.result);
  assert.throws(() => validateValue({state:'submitted',reason:null},familyMethods.CopyButton.query.state.result));
});

test('keymap metadata is closed and command and binding identities cannot collide', () => {
  const command = { id: 'save', label: 'Save', bindings: [{ id: 'custom', keystroke: 'ctrl-k' }] };
  validateValue({ commands: [command] }, familySchemas.KeymapEditor.props);
  for (const commands of [[command, command], [{ ...command, bindings: [...command.bindings, ...command.bindings] }], [{ ...command, execute: 'shell' }]]) {
    assert.throws(() => validateValue({ commands }, familySchemas.KeymapEditor.props));
  }
  assert.throws(() => validateValue({ command_id: 'save', index: 0 }, familySchemas.KeymapEditor.events.remove));
  assert.throws(() => validateValue({ query: '' }, familyMethods.KeymapEditor.query.current_commands.args));
});

test('number options and named command/query arguments reject nonfinite and unbounded data', () => {
  validateValue({ value: -4.25, min: 9, max: -3, step: 0.25, precision: 2 }, familySchemas.NumberInput.props);
  for (const props of [{value: NaN}, {value: Infinity}, {step: 0}, {pageStep: -1}, {precision: 1.5}, {precision: 13}, {bind: {signal: 2}}]) {
    assert.throws(() => validateValue(props, familySchemas.NumberInput.props));
  }
  for (const [method, args] of [['set_range',{min:null}], ['set_steps',{step:1,pageStep:3}], ['set_presentation',{name:null,unit:null,prefix:null,size:'huge'}]]) {
    assert.throws(() => validateValue(args, familyMethods.NumberInput.invoke[method].args));
  }
  validateValue(null, familyMethods.NumberInput.query.current.result);
  assert.throws(() => validateValue('4', familyMethods.NumberInput.query.shown.result));
});

test('color payloads are complete, finite normalized HSLA without executable fields', () => {
  const color = { h: 0.125, s: 0.75, l: 0.25, a: 0.5 };
  const schema = familySchemas.ColorPicker.props;
  validateValue({ value: color, presets: [color], alpha: true }, schema);
  for (const value of [{ ...color, h: -0.01 }, { ...color, a: 1.01 }, { ...color, s: NaN }, { ...color, l: Infinity }, { h: 0, s: 0, l: 0 }, { ...color, path: '/tmp/icon' }]) {
    assert.throws(() => validateValue({ value }, schema));
  }
  assert.throws(() => validateValue({ value: color, bind: { signal: 1 } }, schema));
});

test('filter identities and all method argument/result contracts fail closed', () => {
  const condition = { id: 'owner', field: 'Owner', operator: 'is', value: 'Alice' };
  validateValue({ conditions: [condition], countState: 'unavailable', countReason: 'Permission denied' }, familySchemas.FilterBar.props);
  assert.throws(() => validateValue({ conditions: [condition, condition] }, familySchemas.FilterBar.props));
  assert.throws(() => validateValue({ countState: 'failed' }, familySchemas.FilterBar.props));
  for (const contract of Object.values(familyMethods.FormField.query)) {
    validateValue({}, contract.args);
    assert.throws(() => validateValue({ value: 1 }, contract.args));
    validateValue(false, contract.result);
    assert.throws(() => validateValue(null, contract.result));
  }
});

test('button sources, glyph-only controls, and refused states are explicit', () => {
  for (const [component, props] of [
    ['Button', { color: {} }], ['Button', { color: { semantic: 'info', palette: 'blue' } }],
    ['Button', { iconOnly: true }], ['ToggleGroup', { items: [{ id: 'a', label: 'A', iconOnly: true }] }],
    ['FormField', { label: 'Field', validation: 'invalid' }],
    ['FilterBar', { countState: 'known' }], ['FilterBar', { countState: 'unavailable' }],
  ]) {
    validateValue(props, familySchemas[component].props);
    assert.throws(() => validateFamilyProps(component, props));
  }
  for (const icon of [{ key: 'unknown' }, { key: 'plus-circle', path: '/tmp/icon' }, { key: 'plus-circle', weight: null }]) {
    assert.throws(() => validateValue({ icon, accessibleName: 'Add' }, familySchemas.IconButton.props));
  }
});

test('family factory options, event payloads, and every search method typecheck exactly', () => {
  const dir = mkdtempSync(join(tmpdir(), 'controls-extra-types-'));
  try {
    const sdk = fileURLToPath(new URL('../kit-controls_extra-sdk', import.meta.url));
    const path = join(dir, 'contract.ts');
    writeFileSync(path, `import type { ControlsExtraFactories, ControlsExtraMethodContracts } from ${JSON.stringify(sdk)};
declare const kit: ControlsExtraFactories;
kit.SplitButton('split', {items:[{kind:'check',id:'pin',label:'Pin',checked:true}],defaultDisabled:true}, {invoked(id) { const value: string = id; }});
type SplitMethods = ControlsExtraMethodContracts['SplitButton']['invoke'];
const splitValues: { [K in keyof SplitMethods]: SplitMethods[K]['args'] } = {
open_menu:{},set_label:{label:'Store'},set_icon:{icon:null},set_variant:{variant:'ghost'},set_control_size:{size:'lg'},set_default_disabled:{disabled:true},set_disabled:{disabled:true},set_items:{items:[]},set_menu_name:{name:'Alternatives'}
};
// @ts-expect-error checked native menu items require their state
kit.SplitButton('bad', {items:[{kind:'check',id:'pin',label:'Pin'}]});
// @ts-expect-error command methods are snake_case with exact named arguments
const splitBad: SplitMethods['set_menu_name']['args'] = {menuName:'Wrong'};
kit.SettingsList('settings', {query:'quota'}, {}, {sections:[kit.SettingsSection('section', {title:'Storage'}, {}, {rows:[kit.SettingsRow('row', {label:'Capacity'}, {}, {control:[kit.Button('change',{label:'Change'})]})]})],header:[],empty:[],sidebar:[],footer:[]});
// @ts-expect-error sections require native section builders, not ordinary rows
kit.SettingsList('bad', {}, {}, {sections:[kit.SettingsRow('row',{label:'Wrong'})]});
// @ts-expect-error section title is required
kit.SettingsSection('bad', {});
// @ts-expect-error typed row slots do not accept arbitrary text nodes
kit.SettingsSection('bad', {title:'Bad'}, {}, {rows:[{kind:'text',id:'wrong'}]});
kit.CopyButton('copy', {text:'fixture',glyphOnly:'Copy fixture'}, {copied() {}, failed(reason) { const text: string = reason; }});
type CopyMethods = ControlsExtraMethodContracts['CopyButton']['invoke'];
const copyValues: { [K in keyof CopyMethods]: CopyMethods[K]['args'] } = {
copy:{},set_text:{text:'next'},set_label:{label:null},set_glyph_only:{name:null},set_variant:{variant:'ghost'},set_control_size:{size:'lg'},set_confirmation:{confirmation_ms:30},set_disabled:{disabled:true}
};
// @ts-expect-error custom native copier needs a host capability bridge
kit.CopyButton('bad', {copier:() => {}});
// @ts-expect-error failed reports refusal text rather than success boolean
kit.CopyButton('bad', {}, {failed(reason:boolean) {}});
kit.ButtonGroup('group', {size:'sm'}, {}, {buttons:[kit.Button('child', {label:'Run'}, {click() {}}), {kind:'button',id:'legacy'}]});
// @ts-expect-error typed native group cannot consume a text element
kit.ButtonGroup('bad', {}, {}, {buttons:[{kind:'text',id:'wrong'}]});
// @ts-expect-error group actions belong to each button
kit.ButtonGroup('bad', {}, {click() {}});
kit.KeymapEditor('keys', {commands:[{id:'save',label:'Save',bindings:[{id:'custom',keystroke:'ctrl-k'}]}]}, {remove(value) { const id: string = value.binding_id; }});
type KeymapMethods = ControlsExtraMethodContracts['KeymapEditor']['invoke'];
const keymapValues: { [K in keyof KeymapMethods]: KeymapMethods[K]['args'] } = {set_commands:{commands:[]},set_query:{query:'save'},set_disabled:{disabled:true}};
// @ts-expect-error remove uses binding identity, not position
kit.KeymapEditor('bad', {}, {remove(value:{index:number}) {}});
kit.NumberInput('number', {min:-3,max:9,precision:2}, {change(value) { const n: number = value; }, unparsable(text) { const s: string = text; }});
type NumberMethods = ControlsExtraMethodContracts['NumberInput']['invoke'];
const numberValues: { [K in keyof NumberMethods]: NumberMethods[K]['args'] } = {
set_value:{value:3.5},set_invalid:{invalid:false},set_disabled:{disabled:false},set_required:{required:true},set_range:{min:null,max:9},set_steps:{step:0.5,page_step:null},set_precision:{precision:2},set_presentation:{name:null,unit:'ms',prefix:null,size:'lg'}
};
// @ts-expect-error optional native numeric query is not always a number
const alwaysNumber: number = null as ControlsExtraMethodContracts['NumberInput']['query']['current']['result'];
// @ts-expect-error no serialized signal handles
kit.NumberInput('bad', {bind:{signal:1}});
kit.Button('button', {variant:'white', color:{custom:{h:0.1,s:0.7,l:0.2,a:0.5}}});
kit.IconButton('icon', {icon:{key:'plus-circle',weight:'fill'},accessibleName:'Add'});
kit.Toggle('toggle', {pressed:true}, {press(value) { const checked: boolean = value; }});
kit.ToggleGroup('group', {items:[{id:'beta',label:'Beta'}],pressed:['beta']}, {change(value) { const ids: string[] = value.pressed; const changed: string = value.changed; }});
kit.ColorPicker('color', {value:{h:0.1,s:0.7,l:0.2,a:0.5}}, {change(value) { const opacity: number = value.a; }});
kit.ColorSwatch('swatch', {color:{h:0,s:1,l:0.5,a:1}});
kit.FormField('field', {label:'Name',validation:'validating'}, {}, {content:[]});
kit.FilterBar('filter', {countState:'unavailable',countReason:'Refused'}, {remove(id) { const key: string = id; }});
kit.SearchInput('search', {}, {change(value) { const query: string = value; }});
kit.Combobox('combo',{options:[{id:'a',label:'Alpha',description:'First',group:'Letters'}],allowCustom:true},{custom(text){const value:string=text;}});
kit.MultiSelect('multi',{selected:['a']},{toggled(id){const value:string=id;}});
kit.TagInput('tags',{tags:['a'],collapseAt:1},{moved(event){const index:number=event.from;}});
// @ts-expect-error multi-select emits one toggled identity, not a replacement array
kit.MultiSelect('bad',{}, {toggled(ids:string[]) {}});
// @ts-expect-error options do not serialize native callbacks
kit.Combobox('bad',{options:[{id:'a',label:'Alpha',onClick(){}}]});
kit.SearchField('searchfield', {count:{state:'known',total:7,current:null},matchCase:true}, {next() {}});
kit.FindReplace('find', {}, {search(event) { if(event.kind==='queryChanged') {const value:string=event.value;} }});
// @ts-expect-error an incomplete count is not an exact total
kit.SearchField('bad', {count:{state:'tooMany',total:50}});
// @ts-expect-error an entity getter cannot be replaced with a value snapshot
const searchSnapshot:ControlsExtraMethodContracts['FindReplace']['query']['search_field']['result'] = {query:'text'};
kit.PasswordInput('password', {placeholder:'Fixture',readOnly:true}, {change(value) { const text:string = value; }});
kit.OneTimeCodeInput('code', {slots:6}, {submit() {}});
// @ts-expect-error native sensitive controls cannot be made non-secret
kit.PasswordInput('bad', {secret:false});
// @ts-expect-error one-time codes do not expose an invented complete event
kit.OneTimeCodeInput('bad', {}, {complete() {}});
kit.KeybindingRecorder('recorder', {allowEscape:true,binding:'ctrl-k'}, {captured(value) { const key: string = value; }});
// @ts-expect-error recording is native transient state, not a controlled prop
kit.KeybindingRecorder('bad', {recording:true});
const recorderClear: ControlsExtraMethodContracts['KeybindingRecorder']['invoke']['set_label']['args'] = {label:null};
kit.InlineEdit('inline', {editing:true,multiline:true,rows:3,failure:'Refused'}, {commit(value) { const text: string = value; }});
// @ts-expect-error commit is complete text, not an entity or snapshot metadata
kit.InlineEdit('bad', {}, {commit(value:{text:string}) {}});
kit.SettingsRow('setting', {label:'Retention',managed:'Policy'}, {}, {control:[]});
kit.TransferList('transfer', {source:[{id:'alpha',label:'Alpha',disabled:true}],targetSelected:['zeta']}, {toggleSource(id) { const key: string = id; }});
type TransferMethods = ControlsExtraMethodContracts['TransferList']['invoke'];
const transferValues: { [K in keyof TransferMethods]: TransferMethods[K]['args'] } = {
set_query:{query:'Alpha'},set_items:{source:[],target:[]},set_selection:{source:['alpha'],target:['zeta']},set_labels:{source:'Available',target:'Assigned'},set_control_size:{size:'sm'},set_disabled:{disabled:true}
};
// @ts-expect-error transfer selections use identities, not positions
kit.TransferList('bad', {sourceSelected:[0]});
// @ts-expect-error only the named control slot is accepted
kit.SettingsRow('bad', {label:'Setting'}, {}, {content:[]});
type Methods = ControlsExtraMethodContracts['SearchInput']['invoke'];
const values: { [K in keyof Methods]: Methods[K]['args'] } = {
set_value:{value:'next'},set_name:{name:'Name'},set_placeholder:{placeholder:'Query'},set_disabled:{disabled:true},set_presentation:{name:null,placeholder:null,size:'lg'}
};
const queried: ControlsExtraMethodContracts['SearchInput']['query']['value']['result'] = 'text';
// @ts-expect-error color source is exclusive
kit.Button('bad', {color:{palette:'blue',semantic:'info'}});
// @ts-expect-error custom icon path is not a builtin descriptor
kit.IconButton('bad', {icon:{key:'plus-circle',path:'/tmp/icon'},accessibleName:'Bad'});
// @ts-expect-error required constructor color cannot be omitted
kit.ColorPicker('missing', {});
// @ts-expect-error toggle reports boolean
kit.Toggle('bad', {}, {press(value:string) {}});
// @ts-expect-error count is not text
kit.FilterBar('bad', {count:'42'});
// @ts-expect-error named method argument, not a positional value
const bad: Methods['set_value']['args'] = 'text';
// @ts-expect-error no serialized Signal handle
kit.SearchInput('bad', {bind:{signal:1}});
`);
    const compiler = fileURLToPath(new URL('../../app-host/node_modules/typescript/bin/tsc', import.meta.url));
    const result = spawnSync(process.execPath, [compiler, '--strict', '--noEmit', path], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stdout + result.stderr);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('transfer methods reject positional IDs, duplicate records and missing pane arguments', () => {
  const schema = familySchemas.TransferList.props;
  assert.throws(() => validateValue({ sourceSelected: [0] }, schema));
  assert.throws(() => validateValue({ source: [{id:'a',label:'A'},{id:'a',label:'Different'}] }, schema));
  for (const [method, args] of [
    ['set_query',{query:4}], ['set_items',{source:[]}], ['set_selection',{source:[1],target:[]}],
    ['set_labels',{source:'A',target:'B',extra:true}], ['set_control_size',{size:'huge'}], ['set_disabled',{disabled:null}],
  ]) assert.throws(() => validateValue(args, familyMethods.TransferList.invoke[method].args));
});
