// Only implemented native surfaces are advertised. All values are data-only.
import { iconSchema } from './kit-icon-schema.mjs';
import { menuItemsSchema, validateMenuItems } from './kit-overlay-schema.mjs';
import { selectOptionSchema } from './kit-select-option-schema.mjs';
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
const confirmation = { type: 'number', integer: true, min: 0, max: 60000 };
const step = { ...number, min: Number.MIN_VALUE };
const method = (fields, result) => ({ args: object(fields, Object.keys(fields)), result });
const focusQuery = method({}, object({ $nativeRef: identity, type: choice('FocusHandle') }, ['$nativeRef', 'type']));
const authProps = { ...common, name: string, value: string, required: boolean, invalid: boolean, readOnly: boolean };
const authCommands = {
  set_value: method({ value: string }, choice(null)), set_name: method({ name: { ...string, nullable: true } }, choice(null)),
  set_required: method({ required: boolean }, choice(null)), set_invalid: method({ invalid: boolean }, choice(null)),
  set_read_only: method({ read_only: boolean }, choice(null)), set_disabled: method({ disabled: boolean }, choice(null)),
  set_control_size: method({ size: common.size }, choice(null)),
};
const authQueries = { focus_handle: focusQuery, value: method({}, string), is_disabled: method({}, boolean) };
const hitCount = { oneOf: [
  object({state:choice('unsearched','counting','none')},['state']),
  object({state:choice('known'),total:integer,current:{...integer,nullable:true}},['state','total','current']),
  object({state:choice('tooMany'),counted:integer},['state','counted']),
  object({state:choice('unavailable'),reason:string},['state','reason']),
] };
const searchEvents = { queryChanged:string,next:choice(null),previous:choice(null),cancelled:choice(null),matchCaseToggled:boolean,wholeWordToggled:boolean };
const searchEvent = { oneOf:Object.entries(searchEvents).map(([kind,value])=>object({kind:choice(kind),value},['kind','value'])) };
const textInputQuery = method({},object({$nativeRef:identity,type:choice('TextInput')},['$nativeRef','type']));
const selectionProps = {...common,placeholder:string,invalid:boolean};
const selectionCommands = {
  set_placeholder:method({placeholder:{...string,nullable:true}},choice(null)),
  set_invalid:method({invalid:boolean},choice(null)),set_disabled:method({disabled:boolean},choice(null)),set_control_size:method({size:common.size},choice(null)),
};
const optionCommands = {set_options:method({options:array(selectOptionSchema)},choice(null)),set_name:method({name:string},choice(null))};
const selectionQueries = {is_disabled:method({},boolean),focus_handle:focusQuery};
const cascaderDefs={CascaderOption:object({id:identity,label:string,disabled:boolean,children:{oneOf:[object({state:choice('idle','loading','empty')},['state']),object({state:choice('unavailable','error'),reason:string},['state','reason']),object({state:choice('ready'),value:array({$ref:'CascaderOption'})},['state','value'])]}},['id','label'])};
const cascaderOptions=array({$ref:'CascaderOption'});
const byteRange=object({start:integer,end:integer},['start','end']);
const textSelection=object({range:byteRange,reversed:boolean},['range','reversed']);
const textEdit=object({revision:integer,replaced:byteRange,inserted:string},['revision','replaced','inserted']);
const textSnapshot=object({revision:integer,text:string},['revision','text']);
const point=object({x:number,y:number},['x','y']);
const bounds=object({x:number,y:number,width:number,height:number},['x','y','width','height']);
const rowCount={...integer,min:1,max:4096};
const autosize=object({min:rowCount,max:rowCount},['min','max']);
const pasteRefusal=object({state:choice('unavailable'),kind:choice('images','paths'),reason:string},['state','kind','reason']);
const textAreaProps={...common,placeholder:string,value:string,frame:choice('own','host'),wrap:choice('soft','none'),required:boolean,invalid:boolean,readOnly:boolean,rows:rowCount,maxRows:rowCount,autosize,enter:choice('opens','submits'),maxLength:integer,arrowsClaimed:boolean,completionClaimed:boolean};
const textAreaEvents={change:string,edited:textEdit,submit:choice(null),cancel:choice(null),pasteRefused:pasteRefusal,moveUp:choice(null),moveDown:choice(null),acceptCompletion:choice(null),dismissCompletion:choice(null),indentRequested:choice(null),outdentRequested:choice(null),selectionChanged:byteRange,geometryChanged:choice(null),focus:choice(null),blur:choice(null)};
const editorKind=choice('completion','hover','definition','codeActions');
const editorRequest=object({id:integer,revision:integer,kind:editorKind,position:integer,selection:byteRange,document:string},['id','revision','kind','position','selection','document']);
const replacement=object({range:byteRange,text:string},['range','text']);
const serviceEffect={oneOf:[object({kind:choice('edits'),edits:array(replacement)},['kind','edits']),object({kind:choice('definition'),target:identity,range:byteRange},['kind','target','range']),object({kind:choice('action'),id:identity},['kind','id'])]};
const serviceValue={oneOf:[object({kind:choice('items'),items:array(object({id:identity,label:string,detail:string,effect:serviceEffect},['id','label','effect']))},['kind','items']),object({kind:choice('hover'),range:byteRange,contents:string},['kind','range','contents'])]};
const serviceResult={oneOf:[object({state:choice('idle','loading','empty'),attempts:integer},['state']),object({state:choice('ready','refreshing'),value:serviceValue,attempts:integer},['state','value']),object({state:choice('error','unavailable'),reason:string,value:serviceValue,attempts:integer},['state','reason'])]};
const mentionCandidate=object({id:identity,label:string,description:string,replacement:string,searchTerms:array(string),refusal:string},['id','label']);
const mentionSuggestions={oneOf:[object({state:choice('idle','loading','empty'),attempts:integer},['state']),object({state:choice('ready','refreshing'),value:array(mentionCandidate),attempts:integer},['state','value']),object({state:choice('error','unavailable'),reason:string,value:array(mentionCandidate),attempts:integer},['state','reason'])]};
const mentionQuery={...object({text:string,range:byteRange},['text','range']),nullable:true};
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
  PasswordInput: { prop: 'value', event: 'change' },
  OneTimeCodeInput: { prop: 'value', event: 'change' },
  NumberInput: { prop: 'value', event: 'change' },
  Toggle: { prop: 'pressed', event: 'press' },
  ToggleGroup: { prop: 'pressed', event: 'change', project: 'pressed' },
});

export const familySchemas = Object.freeze({
  MentionInput:{props:object({disabled:boolean,readOnly:boolean,value:string,placeholder:string,rows:rowCount,suggestions:mentionSuggestions}),events:{changed:string,submitted:choice(null),cancelled:choice(null),focused:choice(null),blurred:choice(null),pasteRefused:pasteRefusal,queryChanged:mentionQuery,accepted:object({id:identity,range:byteRange},['id','range'])}},
  Editor:{props:object({disabled:boolean,readOnly:boolean,label:string,value:string,rows:rowCount,lineNumbers:boolean,languageServices:boolean}),events:{changed:string,edited:textEdit,selectionChanged:byteRange,foldChanged:object({id:identity,collapsed:boolean},['id','collapsed']),pasteRefused:pasteRefusal,submitted:choice(null),cancelled:choice(null),focused:choice(null),blurred:choice(null),serviceRequested:editorRequest,serviceAccepted:identity,definitionRequested:object({target:identity,range:byteRange},['target','range']),codeActionRequested:identity}},
  TextArea:{props:object(textAreaProps),events:textAreaEvents},
  Cascader:{props:{$defs:cascaderDefs,...object({...common,name:string,placeholder:string,selected:identity,options:cascaderOptions})},events:{selected:identity,expanded:identity,retry:identity,opened:choice(null),closed:choice(null)}},
  Combobox:{props:object({...selectionProps,name:string,options:array(selectOptionSchema),selected:identity,query:string,allowCustom:boolean}),events:{queryChanged:string,selected:identity,custom:string,opened:choice(null),closed:choice(null)}},
  MultiSelect:{props:object({...selectionProps,name:string,options:array(selectOptionSchema),selected:array(identity),clearable:boolean}),events:{queryChanged:string,toggled:identity,removed:identity,cleared:choice(null),opened:choice(null),closed:choice(null)}},
  TagInput:{props:object({...selectionProps,tags:array(identity),max:integer,collapseAt:integer,reorderable:boolean}),events:{added:string,removed:string,duplicate:string,refused:string,editRequested:string,moved:object({from:integer,to:integer},['from','to'])}},
  SearchField: { props:object({...common,placeholder:string,query:string,matchCase:boolean,wholeWord:boolean,count:hitCount}),events:searchEvents },
  FindReplace: { props:object({...common,count:hitCount}),events:{search:searchEvent,replacementChanged:string,replaceOne:choice(null),replaceAll:object({count:integer},['count']),close:choice(null)} },
  PasswordInput: { props: object({ ...authProps, placeholder: string }), events: { change: string, submit: choice(null), cancel: choice(null), backspaceAtStart: choice(null), focus: choice(null), blur: choice(null) } },
  OneTimeCodeInput: { props: object({ ...authProps, slots: { ...integer, min: 1, max: 12 } }), events: { change: string, submit: choice(null) } },
  KeybindingRecorder: { props: object({ ...common, label: string, placeholder: string, binding: string, conflict: string, allowEscape: boolean }), events: { started: choice(null), captured: string, cancelled: choice(null) } },
  InlineEdit: { props: object({ ...common, value: string, placeholder: string, editing: boolean, multiline: boolean, rows: { ...integer, min: 1, max: 1024 }, failure: string }), events: { edit: choice(null), commit: string, cancel: choice(null) } },
  SplitButton: { props: object({ ...common, label: string, icon: iconSchema, variant, menuName: string, defaultDisabled: boolean, items: menuItemsSchema }), events: { click: choice(null), open: choice(null), close: choice(null), dismiss: choice(null), invoked: identity } },
  SettingsList: { props: object({ query: string }), events: {}, slots: ['sections', 'empty', 'header', 'sidebar', 'footer'] },
  SettingsSection: { props: object({ title: string, description: string, dimmedBy: string, labelWidth: { type: 'number', min: 0, max: 100000 } }, ['title']), events: {}, slots: ['rows', 'content', 'action'] },
  CopyButton: { props: object({ ...common, text: string, label: string, glyphOnly: identity, variant, confirmationMs: confirmation }), events: { copied: choice(null), failed: string } },
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
  MentionInput:{invoke:{set_suggestions:method({suggestions:mentionSuggestions},choice(null))},query:{editor:method({},object({$nativeRef:identity,type:choice('TextArea')},['$nativeRef','type'])),active_query:method({},mentionQuery),is_open:method({},boolean)}},
  Editor:{invoke:{
    set_diagnostics:method({revision:integer,diagnostics:array(object({id:identity,range:byteRange,message:string,severity:choice('error','warning','information','hint')},['id','range','message','severity']))},boolean),
    set_semantic_tokens:method({revision:integer,tokens:array(object({range:byteRange,class:choice('keyword','stringLiteral','comment','number','inline','inlineWash','added','addedWash','removed','removedWash')},['range','class']))},boolean),
    set_value:method({value:string},choice(null)),set_label:method({label:string},choice(null)),set_rows:method({rows:rowCount},choice(null)),set_line_numbers:method({visible:boolean},choice(null)),set_read_only:method({read_only:boolean},choice(null)),set_disabled:method({disabled:boolean},choice(null)),set_folds:method({revision:integer,folds:array(object({id:identity,lines:byteRange},['id','lines']))},boolean),set_fold_collapsed:method({id:identity,collapsed:boolean},boolean),set_selections:method({selections:array(textSelection)},boolean),apply_edits:method({revision:integer,edits:array(replacement)},boolean),set_language_services:method({enabled:boolean},choice(null)),request_service:method({kind:editorKind,position:integer},{...editorRequest,nullable:true}),set_service_result:method({request:integer,result:serviceResult},boolean),dismiss_service:method({},choice(null)),accept_service_item:method({id:identity},boolean),
  },query:{text_area:method({},object({$nativeRef:identity,type:choice('TextArea')},['$nativeRef','type'])),snapshot:method({},textSnapshot),is_disabled:method({},boolean),is_read_only:method({},boolean),selected_range:method({},byteRange),selections:method({},array(textSelection)),is_fold_collapsed:method({id:identity},boolean),geometry:method({},{...object({revision:integer,viewport:bounds,horizontal_scroll:number,vertical_scroll:number,lines:array(object({line:integer,range:byteRange,bounds},['line','range','bounds']))},['revision','viewport','horizontal_scroll','vertical_scroll','lines']),nullable:true})}},
  TextArea:{invoke:{
    set_value:method({value:string},choice(null)),insert:method({text:string},choice(null)),replace_range:method({range:byteRange,text:string},{...byteRange,nullable:true}),replace_ranges:method({edits:array(object({range:byteRange,text:string},['range','text']))},boolean),set_selected_range:method({range:byteRange},choice(null)),set_selections:method({selections:array(textSelection)},boolean),select_rectangle:method({anchor:point,focus:point},boolean),
    set_placeholder:method({placeholder:string},choice(null)),set_frame:method({frame:textAreaProps.frame},choice(null)),set_wrap:method({wrap:textAreaProps.wrap},choice(null)),set_enter:method({enter:textAreaProps.enter},choice(null)),set_rows:method({rows:rowCount},choice(null)),set_max_rows:method({max_rows:{...rowCount,nullable:true}},choice(null)),set_autosize:method({rows:{...autosize,nullable:true}},choice(null)),set_max_length:method({max_length:{...integer,nullable:true}},choice(null)),set_disabled:method({disabled:boolean},choice(null)),set_read_only:method({read_only:boolean},choice(null)),set_invalid:method({invalid:boolean},choice(null)),set_required:method({required:boolean},choice(null)),set_arrows_claimed:method({claimed:boolean},choice(null)),set_completion_claimed:method({claimed:boolean},choice(null)),set_control_size:method({size:common.size},choice(null)),focus:method({},choice(null)),
  },query:{focus_handle:focusQuery,value:method({},string),snapshot:method({},textSnapshot),revision:method({},integer),is_empty:method({},boolean),is_disabled:method({},boolean),is_read_only:method({},boolean),wrap_mode:method({},textAreaProps.wrap),selected_range:method({},byteRange),cursor_offset:method({},integer),cursor_row:method({},integer),arrows_claimed:method({},boolean),completion_claimed:method({},boolean),selections:method({},array(textSelection)),bounds_for_range:method({range:byteRange},{...array(bounds),nullable:true}),bounds_for_position:method({offset:integer},{...bounds,nullable:true}),caret_bounds:method({},{...bounds,nullable:true}),measured:method({},{...object({text:number,height:number,wrapped:number,pass:integer},['text','height','wrapped','pass']),nullable:true}),horizontal_scroll_offset:method({},number)}},
  Cascader:{invoke:{set_options:{args:{$defs:cascaderDefs,...object({options:cascaderOptions},['options'])},result:choice(null)},set_selected:method({selected:{...identity,nullable:true}},choice(null)),set_name:method({name:string},choice(null)),set_placeholder:selectionCommands.set_placeholder,set_disabled:selectionCommands.set_disabled,set_control_size:selectionCommands.set_control_size,open:method({},choice(null)),close:method({},choice(null))},query:{...selectionQueries,is_open:method({},boolean),selected_id:method({},{...identity,nullable:true}),open_path:method({},array(identity))}},
  Combobox:{invoke:{...selectionCommands,...optionCommands,set_selected:method({selected:{...identity,nullable:true}},choice(null)),set_query:method({text:string},choice(null)),set_allow_custom:method({allow:boolean},choice(null)),open:method({},choice(null)),toggle:method({},choice(null))},query:{...selectionQueries,query_input:textInputQuery,is_open:method({},boolean),query_text:method({},string),selected_id:method({},{...identity,nullable:true}),selected_option:method({},{...object({...selectOptionSchema.fields,description:{...string,nullable:true},group:{...string,nullable:true}},Object.keys(selectOptionSchema.fields)),nullable:true})}},
  MultiSelect:{invoke:{...selectionCommands,...optionCommands,set_selected:method({selected:array(identity)},choice(null)),set_clearable:method({clearable:boolean},choice(null)),open:method({},choice(null))},query:{...selectionQueries,query_input:textInputQuery,selected_ids:method({},array(identity)),is_open:method({},boolean)}},
  TagInput:{invoke:{...selectionCommands,set_tags:method({tags:array(identity)},choice(null)),set_max:method({max:{...integer,nullable:true}},choice(null)),set_collapse_at:method({visible:{...integer,nullable:true}},choice(null)),set_reorderable:method({reorderable:boolean},choice(null))},query:{...selectionQueries,field:textInputQuery,current:method({},array(identity)),targeted:method({},{...string,nullable:true}),refusal:method({},{...string,nullable:true})}},
  SearchField: {
    invoke:{set_query:method({text:string},choice(null)),set_count:method({count:hitCount},choice(null)),set_match_case:method({on:{...boolean,nullable:true}},choice(null)),set_whole_word:method({on:{...boolean,nullable:true}},choice(null)),set_placeholder:method({placeholder:{...string,nullable:true}},choice(null)),set_disabled:method({disabled:boolean},choice(null)),set_control_size:method({size:common.size},choice(null)),focus:method({},choice(null))},
    query:{count:method({},hitCount),query_text:method({},string),is_disabled:method({},boolean),query_input:textInputQuery,focus_handle:focusQuery},
  },
  FindReplace: {
    invoke:{set_count:method({count:hitCount},choice(null)),set_disabled:method({disabled:boolean},choice(null)),set_control_size:method({size:common.size},choice(null))},
    query:{count:method({},hitCount),replacement_text:method({},string),is_disabled:method({},boolean),replacement_input:textInputQuery,search_field:method({},object({$nativeRef:identity,type:choice('SearchField')},['$nativeRef','type'])),focus_handle:focusQuery},
  },
  PasswordInput: { invoke: { ...authCommands, set_placeholder: method({ placeholder: { ...string, nullable: true } }, choice(null)) }, query: { ...authQueries, is_revealed: method({}, boolean), selected_range: method({}, object({start:integer,end:integer},['start','end'])) } },
  OneTimeCodeInput: { invoke: { ...authCommands, set_slots: method({ slots: { ...integer, min:1, max:12 } }, choice(null)) }, query: { ...authQueries, len: method({}, integer), is_empty: method({},boolean), is_complete: method({},boolean), slot_count: method({},integer) } },
  KeybindingRecorder: {
    invoke: {
      start: method({}, choice(null)), cancel: method({}, choice(null)),
      set_binding: method({ binding: { ...string, nullable: true } }, choice(null)),
      set_conflict: method({ reason: { ...string, nullable: true } }, choice(null)),
      set_label: method({ label: { ...string, nullable: true } }, choice(null)),
      set_placeholder: method({ placeholder: { ...string, nullable: true } }, choice(null)),
      set_allow_escape: method({ allow: boolean }, choice(null)),
      set_disabled: method({ disabled: boolean }, choice(null)),
      set_control_size: method({ size: common.size }, choice(null)),
    },
    query: { focus_handle: focusQuery, is_recording: method({}, boolean), current_binding: method({}, { ...string, nullable: true }), is_disabled: method({}, boolean) },
  },
  SplitButton: {
    invoke: {
      open_menu: method({}, choice(null)), set_label: method({ label: string }, choice(null)),
      set_icon: method({ icon: { ...iconSchema, nullable: true } }, choice(null)),
      set_variant: method({ variant }, choice(null)), set_control_size: method({ size: common.size }, choice(null)),
      set_default_disabled: method({ disabled: boolean }, choice(null)), set_disabled: method({ disabled: boolean }, choice(null)),
      set_items: method({ items: menuItemsSchema }, choice(null)), set_menu_name: method({ name: string }, choice(null)),
    },
    query: { menu: method({}, object({ $nativeRef: identity, type: choice('Menu') }, ['$nativeRef', 'type'])), focus_handle: focusQuery, is_open: method({}, boolean), is_disabled: method({}, boolean) },
  },
  CopyButton: {
    invoke: {
      copy: method({}, choice(null)), set_text: method({ text: string }, choice(null)),
      set_label: method({ label: { ...string, nullable: true } }, choice(null)),
      set_glyph_only: method({ name: { ...identity, nullable: true } }, choice(null)),
      set_variant: method({ variant }, choice(null)), set_control_size: method({ size: common.size }, choice(null)),
      set_confirmation: method({ confirmation_ms: confirmation }, choice(null)), set_disabled: method({ disabled: boolean }, choice(null)),
    },
    query: { focus_handle: focusQuery, state: method({}, object({ state: choice('idle', 'copied', 'failed'), reason: { ...string, nullable: true } }, ['state', 'reason'])), is_disabled: method({}, boolean) },
  },
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
      focus_handle: focusQuery,
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
    query: { focus_handle: focusQuery, value: method({}, string), is_disabled: method({}, boolean) },
  },
  FormField: { invoke: {}, query: { is_invalid: method({}, boolean), is_validating: method({}, boolean) } },
});

// Called after closed-shape validation, in both worker and native host.
export function validateFamilyProps(component, props) {
  if(component==='Cascader') {
    const ids=new Set();
    const visit=options=>{for(const option of options??[]){if(ids.has(option.id))throw new TypeError('duplicate Cascader identity');ids.add(option.id);if(option.children?.state==='ready')visit(option.children.value);}};
    visit(props.options);
  }
  if (component === 'SplitButton') validateMenuItems(props.items ?? []);
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
