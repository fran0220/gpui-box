import type { ControlProps, KitNode, SlotNode, SelectOption, KitMethodContracts } from './kit-sdk.js';
import type { BuiltinIconDescriptor } from './kit-icon-sdk.js';
import type { MenuItemDescriptor } from './kit-overlay-sdk.js';
import type { NativeRef } from './reference-sdk.js';
export type NativeKitButtonNode = Omit<KitNode, 'component'> & { component: 'Button' };
export type NativeSettingsRowNode = Omit<KitNode, 'component'> & { component: 'SettingsRow' };
export type NativeSettingsSectionNode = Omit<KitNode, 'component'> & { component: 'SettingsSection' };
export interface KitColor { h: number; s: number; l: number; a: number }
export interface ControlsExtraBindingValues { Toggle: boolean; ToggleGroup: string[]; NumberInput: number; PasswordInput: string; OneTimeCodeInput: string }
export interface SensitiveInputProps extends ControlProps { name?: string; value?: string; required?: boolean; invalid?: boolean; readOnly?: boolean }
export type HitCount = {state:'unsearched'|'counting'|'none'} | {state:'known';total:number;current:number|null} | {state:'tooMany';counted:number} | {state:'unavailable';reason:string};
export type SearchFieldEvent = {kind:'queryChanged';value:string} | {kind:'next'|'previous'|'cancelled';value:null} | {kind:'matchCaseToggled'|'wholeWordToggled';value:boolean};
export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'link';
export type ButtonStyle = ButtonVariant | 'filled' | 'light' | 'subtle' | 'default' | 'transparent' | 'white';
export type ControlGround = 'backdrop' | 'canvas' | 'sunken' | 'panel' | 'raised' | 'overlay';
export type ButtonJoin = 'alone' | 'leading' | 'middle' | 'trailing';
export type ControlColor = { palette: string; semantic?: never; custom?: never } | { semantic: 'accent' | 'accentStrong' | 'danger' | 'warning' | 'success' | 'info'; palette?: never; custom?: never } | { custom: KitColor; palette?: never; semantic?: never };
export interface NativeButtonProps extends ControlProps { accessibleName?: string; semanticParent?: string; icon?: BuiltinIconDescriptor; variant?: ButtonStyle; color?: ControlColor; ground?: ControlGround; join?: ButtonJoin; loading?: boolean }
export interface ToggleItem { id: string; label: string; icon?: BuiltinIconDescriptor; iconOnly?: boolean; disabled?: boolean }
export interface FilterCondition { id: string; field: string; operator: string; value: string; tone?: 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info' }
export interface TransferItem { id: string; label: string; disabled?: boolean }
export interface CascaderOption {id:string;label:string;disabled?:boolean;children?:{state:'idle'|'loading'|'empty'}|{state:'unavailable'|'error';reason:string}|{state:'ready';value:CascaderOption[]}}
export interface ByteRange {start:number;end:number}
export interface TextSelection {range:ByteRange;reversed:boolean}
export interface TextEdit {revision:number;replaced:ByteRange;inserted:string}
export interface TextSnapshot {revision:number;text:string}
export interface TextBounds {x:number;y:number;width:number;height:number}
export interface PasteRefusal {state:'unavailable';kind:'images'|'paths';reason:string}
export interface TextAreaProps extends ControlProps {placeholder?:string;value?:string;frame?:'own'|'host';wrap?:'soft'|'none';required?:boolean;invalid?:boolean;readOnly?:boolean;rows?:number;maxRows?:number;autosize?:{min:number;max:number};enter?:'opens'|'submits';maxLength?:number;arrowsClaimed?:boolean;completionClaimed?:boolean}
export interface TextAreaEvents {change?(text:string):void;edited?(edit:TextEdit):void;submit?():void;cancel?():void;pasteRefused?(refusal:PasteRefusal):void;moveUp?():void;moveDown?():void;acceptCompletion?():void;dismissCompletion?():void;indentRequested?():void;outdentRequested?():void;selectionChanged?(range:ByteRange):void;geometryChanged?():void;focus?():void;blur?():void}
export type EditorServiceRequest = NonNullable<KitMethodContracts['Editor']['invoke']['request_service']['result']>;
export interface EditorProps {disabled?:boolean;readOnly?:boolean;label?:string;value?:string;rows?:number;lineNumbers?:boolean;languageServices?:boolean}
export interface EditorEvents {changed?(text:string):void;edited?(edit:TextEdit):void;selectionChanged?(range:ByteRange):void;foldChanged?(event:{id:string;collapsed:boolean}):void;pasteRefused?(refusal:PasteRefusal):void;submitted?():void;cancelled?():void;focused?():void;blurred?():void;serviceRequested?(request:EditorServiceRequest):void;serviceAccepted?(id:string):void;definitionRequested?(event:{target:string;range:ByteRange}):void;codeActionRequested?(id:string):void}
export interface KeymapBinding { id: string; keystroke: string; conflict?: string; provenance?: string }
export interface KeymapCommand { id: string; label: string; context?: string; defaults?: string[]; bindings?: KeymapBinding[]; searchText?: string; keywords?: string[]; refusal?: string }
export interface KeymapCommandResult { id: string; label: string; context: string | null; defaults: string[]; bindings: { id: string; keystroke: string; conflict: string | null; provenance: string | null }[]; searchText: string; keywords: string[]; refusal: string | null }
export interface ControlsExtraFactories {
  Editor(id:string,props?:EditorProps,events?:EditorEvents):KitNode;
  TextArea(id:string,props?:TextAreaProps,events?:TextAreaEvents):KitNode;
  Cascader(id:string,props?:ControlProps & {options?:CascaderOption[];selected?:string;name?:string;placeholder?:string},events?:{selected?(id:string):void;expanded?(id:string):void;retry?(id:string):void;opened?():void;closed?():void}):KitNode;
  Combobox(id:string,props?:ControlProps & {name?:string;placeholder?:string;invalid?:boolean;options?:SelectOption[];selected?:string;query?:string;allowCustom?:boolean},events?:{queryChanged?(text:string):void;selected?(id:string):void;custom?(text:string):void;opened?():void;closed?():void}):KitNode;
  MultiSelect(id:string,props?:ControlProps & {name?:string;placeholder?:string;invalid?:boolean;options?:SelectOption[];selected?:string[];clearable?:boolean},events?:{queryChanged?(text:string):void;toggled?(id:string):void;removed?(id:string):void;cleared?():void;opened?():void;closed?():void}):KitNode;
  TagInput(id:string,props?:ControlProps & {placeholder?:string;invalid?:boolean;tags?:string[];max?:number;collapseAt?:number;reorderable?:boolean},events?:{added?(text:string):void;removed?(text:string):void;duplicate?(text:string):void;refused?(reason:string):void;editRequested?(text:string):void;moved?(event:{from:number;to:number}):void}):KitNode;
  SearchField(id: string, props?: ControlProps & { placeholder?: string; query?: string; matchCase?: boolean; wholeWord?: boolean; count?: HitCount }, events?: { queryChanged?(value:string):void;next?():void;previous?():void;cancelled?():void;matchCaseToggled?(value:boolean):void;wholeWordToggled?(value:boolean):void }): KitNode;
  FindReplace(id: string, props?: ControlProps & { count?: HitCount }, events?: { search?(event:SearchFieldEvent):void;replacementChanged?(value:string):void;replaceOne?():void;replaceAll?(value:{count:number}):void;close?():void }): KitNode;
  PasswordInput(id: string, props?: SensitiveInputProps & { placeholder?: string }, events?: { change?(value: string): void; submit?(): void; cancel?(): void; backspaceAtStart?(): void; focus?(): void; blur?(): void }): KitNode;
  OneTimeCodeInput(id: string, props?: SensitiveInputProps & { slots?: number }, events?: { change?(value: string): void; submit?(): void }): KitNode;
  KeybindingRecorder(id: string, props?: ControlProps & { label?: string; placeholder?: string; binding?: string; conflict?: string; allowEscape?: boolean }, events?: { started?(): void; captured?(keystroke: string): void; cancelled?(): void }): KitNode;
  InlineEdit(id: string, props?: ControlProps & { value?: string; placeholder?: string; editing?: boolean; multiline?: boolean; rows?: number; failure?: string }, events?: { edit?(): void; commit?(value: string): void; cancel?(): void }): KitNode;
  SplitButton(id: string, props?: ControlProps & { label?: string; icon?: BuiltinIconDescriptor; variant?: ButtonVariant; menuName?: string; defaultDisabled?: boolean; items?: MenuItemDescriptor[] }, events?: { click?(): void; open?(): void; close?(): void; dismiss?(): void; invoked?(id: string): void }): KitNode;
  SettingsList(id: string, props?: { query?: string }, events?: Record<string, never>, slots?: { sections?: NativeSettingsSectionNode[]; empty?: SlotNode[]; header?: SlotNode[]; sidebar?: SlotNode[]; footer?: SlotNode[] }): KitNode;
  SettingsSection(id: string, props: { title: string; description?: string; dimmedBy?: string; labelWidth?: number }, events?: Record<string, never>, slots?: { rows?: NativeSettingsRowNode[]; content?: SlotNode[]; action?: SlotNode[] }): NativeSettingsSectionNode;
  CopyButton(id: string, props?: ControlProps & { text?: string; label?: string; glyphOnly?: string; variant?: ButtonVariant; confirmationMs?: number }, events?: { copied?(): void; failed?(reason: string): void }): KitNode;
  ButtonGroup(id: string, props?: ControlProps, events?: Record<string, never>, slots?: { buttons?: (NativeKitButtonNode | { kind: 'button'; id: string })[] }): KitNode;
  KeymapEditor(id: string, props?: { disabled?: boolean; commands?: KeymapCommand[]; query?: string }, events?: { addCaptured?(value: { command_id: string; keystroke: string }): void; remove?(value: { command_id: string; binding_id: string }): void; reset?(value: { command_id: string }): void; recordingCancelled?(value: { command_id: string }): void }): KitNode;
  NumberInput(id: string, props?: ControlProps & { value?: number; min?: number; max?: number; step?: number; pageStep?: number; precision?: number; name?: string; unit?: string; prefix?: string; required?: boolean; invalid?: boolean }, events?: { change?(value: number): void; unparsable?(text: string): void; submit?(): void }): KitNode;
  TransferList(id: string, props?: ControlProps & { source?: TransferItem[]; target?: TransferItem[]; sourceSelected?: string[]; targetSelected?: string[]; sourceLabel?: string; targetLabel?: string; query?: string }, events?: { toggleSource?(id: string): void; toggleTarget?(id: string): void; moveToTarget?(): void; moveToSource?(): void; queryChange?(query: string): void }): KitNode;
  SettingsRow(id: string, props: { label: string; description?: string; labelWidth?: number; badge?: string; value?: string; searchTerms?: string[]; managed?: string }, events?: Record<string, never>, slots?: { control?: SlotNode[] }): NativeSettingsRowNode;
  SearchInput(id: string, props?: ControlProps & { name?: string; placeholder?: string; value?: string }, events?: { change?(value: string): void; submit?(): void; cancel?(): void; backspaceAtStart?(): void; focus?(): void; blur?(): void }): KitNode;
  Button(id: string, props?: NativeButtonProps & { label?: string; accessibleDescription?: string; iconOnly?: boolean; iconPosition?: 'leading' | 'trailing'; fullWidth?: boolean; checkedState?: boolean }, events?: { click?(): void }): NativeKitButtonNode;
  IconButton(id: string, props: NativeButtonProps & { icon: BuiltinIconDescriptor; accessibleName: string }, events?: { click?(): void }): KitNode;
  Toggle(id: string, props?: ControlProps & { label?: string; accessibleName?: string; semanticParent?: string; icon?: BuiltinIconDescriptor; iconOnly?: boolean; variant?: ButtonVariant; ground?: ControlGround; join?: ButtonJoin; pressed?: boolean }, events?: { press?(pressed: boolean): void }): KitNode;
  ToggleGroup(id: string, props?: ControlProps & { label?: string; items?: ToggleItem[]; pressed?: string[]; selection?: 'any' | 'atMostOne'; variant?: ButtonVariant; ground?: ControlGround }, events?: { change?(value: { pressed: string[]; changed: string }): void }): KitNode;
  ColorPicker(id: string, props: { disabled?: boolean; value: KitColor; alpha?: boolean; presets?: KitColor[]; recent?: KitColor[] }, events?: { change?(color: KitColor): void }): KitNode;
  ColorSwatch(id: string, props: { disabled?: boolean; color: KitColor; selected?: boolean }, events?: { click?(color: KitColor): void }): KitNode;
  FormField(id: string, props: { label: string; control?: string; description?: string; validation?: 'pending' | 'validating' | 'invalid' | 'valid'; reason?: string; error?: string; hint?: string; required?: boolean }, events?: Record<string, never>, slots?: { content?: SlotNode[] }): KitNode;
  FilterBar(id: string, props?: ControlProps & { conditions?: FilterCondition[]; countState?: 'unknown' | 'counting' | 'known' | 'unavailable'; count?: number; countReason?: string; noun?: string; addLabel?: string; clearLabel?: string }, events?: { add?(): void; remove?(id: string): void; clear?(): void }, slots?: { add_control?: SlotNode[] }): KitNode;
}
interface FocusQueries { focus_handle: { args: Record<string, never>; result: NativeRef<'FocusHandle'> } }
interface SensitiveInputCommands {
  set_value: { args: { value: string }; result: null };
  set_name: { args: { name: string | null }; result: null };
  set_required: { args: { required: boolean }; result: null };
  set_invalid: { args: { invalid: boolean }; result: null };
  set_read_only: { args: { read_only: boolean }; result: null };
  set_disabled: { args: { disabled: boolean }; result: null };
  set_control_size: { args: { size: 'xs' | 'sm' | 'md' | 'lg' }; result: null };
}
interface SensitiveInputQueries extends FocusQueries {
  value: { args: Record<string, never>; result: string };
  is_disabled: { args: Record<string, never>; result: boolean };
}
type SelectionCommand<Args> = {args:Args;result:null};
type SelectionQuery<Result> = {args:Record<string,never>;result:Result};
interface SelectionCommands {
  set_placeholder:SelectionCommand<{placeholder:string|null}>;
  set_invalid:SelectionCommand<{invalid:boolean}>;
  set_disabled:SelectionCommand<{disabled:boolean}>;
  set_control_size:SelectionCommand<{size:'xs'|'sm'|'md'|'lg'}>;
}
interface OptionCommands {
  set_options:SelectionCommand<{options:SelectOption[]}>;
  set_name:SelectionCommand<{name:string}>;
}
interface SelectionQueries extends FocusQueries {is_disabled:SelectionQuery<boolean>}
export interface ControlsExtraMethodContracts {
  Editor: KitMethodContracts['Editor'];
  TextArea:{invoke:{
    set_value:SelectionCommand<{value:string}>;insert:SelectionCommand<{text:string}>;replace_range:{args:{range:ByteRange;text:string};result:ByteRange|null};replace_ranges:{args:{edits:{range:ByteRange;text:string}[]};result:boolean};set_selected_range:SelectionCommand<{range:ByteRange}>;set_selections:{args:{selections:TextSelection[]};result:boolean};select_rectangle:{args:{anchor:{x:number;y:number};focus:{x:number;y:number}};result:boolean};
    set_placeholder:SelectionCommand<{placeholder:string}>;set_frame:SelectionCommand<{frame:'own'|'host'}>;set_wrap:SelectionCommand<{wrap:'soft'|'none'}>;set_enter:SelectionCommand<{enter:'opens'|'submits'}>;set_rows:SelectionCommand<{rows:number}>;set_max_rows:SelectionCommand<{max_rows:number|null}>;set_autosize:SelectionCommand<{rows:{min:number;max:number}|null}>;set_max_length:SelectionCommand<{max_length:number|null}>;set_disabled:SelectionCommand<{disabled:boolean}>;set_read_only:SelectionCommand<{read_only:boolean}>;set_invalid:SelectionCommand<{invalid:boolean}>;set_required:SelectionCommand<{required:boolean}>;set_arrows_claimed:SelectionCommand<{claimed:boolean}>;set_completion_claimed:SelectionCommand<{claimed:boolean}>;set_control_size:SelectionCommand<{size:'xs'|'sm'|'md'|'lg'}>;focus:SelectionCommand<Record<string,never>>;
  };query:FocusQueries & {value:SelectionQuery<string>;snapshot:SelectionQuery<TextSnapshot>;revision:SelectionQuery<number>;is_empty:SelectionQuery<boolean>;is_disabled:SelectionQuery<boolean>;is_read_only:SelectionQuery<boolean>;wrap_mode:SelectionQuery<'soft'|'none'>;selected_range:SelectionQuery<ByteRange>;cursor_offset:SelectionQuery<number>;cursor_row:SelectionQuery<number>;arrows_claimed:SelectionQuery<boolean>;completion_claimed:SelectionQuery<boolean>;selections:SelectionQuery<TextSelection[]>;bounds_for_range:{args:{range:ByteRange};result:TextBounds[]|null};bounds_for_position:{args:{offset:number};result:TextBounds|null};caret_bounds:SelectionQuery<TextBounds|null>;measured:SelectionQuery<{text:number;height:number;wrapped:number;pass:number}|null>;horizontal_scroll_offset:SelectionQuery<number>}};
  Cascader:{invoke:Omit<SelectionCommands,'set_invalid'> & {set_options:SelectionCommand<{options:CascaderOption[]}>;set_selected:SelectionCommand<{selected:string|null}>;set_name:SelectionCommand<{name:string}>;open:SelectionCommand<Record<string,never>>;close:SelectionCommand<Record<string,never>>};query:SelectionQueries & {is_open:SelectionQuery<boolean>;selected_id:SelectionQuery<string|null>;open_path:SelectionQuery<string[]>}};
  Combobox:{invoke:SelectionCommands & OptionCommands & {set_selected:SelectionCommand<{selected:string|null}>;set_query:SelectionCommand<{text:string}>;set_allow_custom:SelectionCommand<{allow:boolean}>;open:SelectionCommand<Record<string,never>>;toggle:SelectionCommand<Record<string,never>>};query:SelectionQueries & {query_input:SelectionQuery<NativeRef<'TextInput'>>;is_open:SelectionQuery<boolean>;query_text:SelectionQuery<string>;selected_id:SelectionQuery<string|null>;selected_option:SelectionQuery<{id:string;label:string;disabled:boolean;description:string|null;group:string|null}|null>}};
  MultiSelect:{invoke:SelectionCommands & OptionCommands & {set_selected:SelectionCommand<{selected:string[]}>;set_clearable:SelectionCommand<{clearable:boolean}>;open:SelectionCommand<Record<string,never>>};query:SelectionQueries & {query_input:SelectionQuery<NativeRef<'TextInput'>>;is_open:SelectionQuery<boolean>;selected_ids:SelectionQuery<string[]>}};
  TagInput:{invoke:SelectionCommands & {set_tags:SelectionCommand<{tags:string[]}>;set_max:SelectionCommand<{max:number|null}>;set_collapse_at:SelectionCommand<{visible:number|null}>;set_reorderable:SelectionCommand<{reorderable:boolean}>};query:SelectionQueries & {field:SelectionQuery<NativeRef<'TextInput'>>;current:SelectionQuery<string[]>;targeted:SelectionQuery<string|null>;refusal:SelectionQuery<string|null>}};
  SearchField: {
    invoke: {
      set_query: {args:{text:string};result:null};
      set_count: {args:{count:HitCount};result:null};
      set_match_case: {args:{on:boolean|null};result:null};
      set_whole_word: {args:{on:boolean|null};result:null};
      set_placeholder: {args:{placeholder:string|null};result:null};
      set_disabled: {args:{disabled:boolean};result:null};
      set_control_size: {args:{size:'xs'|'sm'|'md'|'lg'};result:null};
      focus: {args:Record<string,never>;result:null};
    };
    query: FocusQueries & {count:{args:Record<string,never>;result:HitCount};query_text:{args:Record<string,never>;result:string};is_disabled:{args:Record<string,never>;result:boolean};query_input:{args:Record<string,never>;result:NativeRef<'TextInput'>}};
  };
  FindReplace: {
    invoke: {set_count:{args:{count:HitCount};result:null};set_disabled:{args:{disabled:boolean};result:null};set_control_size:{args:{size:'xs'|'sm'|'md'|'lg'};result:null}};
    query: FocusQueries & {count:{args:Record<string,never>;result:HitCount};replacement_text:{args:Record<string,never>;result:string};is_disabled:{args:Record<string,never>;result:boolean};replacement_input:{args:Record<string,never>;result:NativeRef<'TextInput'>};search_field:{args:Record<string,never>;result:NativeRef<'SearchField'>}};
  };
  PasswordInput: {
    invoke: SensitiveInputCommands & { set_placeholder: { args: { placeholder: string | null }; result: null } };
    query: SensitiveInputQueries & { is_revealed: { args: Record<string, never>; result: boolean }; selected_range: { args: Record<string, never>; result: {start:number;end:number} } };
  };
  OneTimeCodeInput: {
    invoke: SensitiveInputCommands & { set_slots: { args: { slots: number }; result: null } };
    query: SensitiveInputQueries & { len: { args: Record<string, never>; result: number }; slot_count: { args: Record<string, never>; result: number }; is_empty: { args: Record<string, never>; result: boolean }; is_complete: { args: Record<string, never>; result: boolean } };
  };
  KeybindingRecorder: {
    invoke: {
      start: { args: Record<string, never>; result: null };
      cancel: { args: Record<string, never>; result: null };
      set_binding: { args: { binding: string | null }; result: null };
      set_conflict: { args: { reason: string | null }; result: null };
      set_label: { args: { label: string | null }; result: null };
      set_placeholder: { args: { placeholder: string | null }; result: null };
      set_allow_escape: { args: { allow: boolean }; result: null };
      set_disabled: { args: { disabled: boolean }; result: null };
      set_control_size: { args: { size: 'xs' | 'sm' | 'md' | 'lg' }; result: null };
    };
    query: FocusQueries & {
      is_recording: { args: Record<string, never>; result: boolean };
      current_binding: { args: Record<string, never>; result: string | null };
      is_disabled: { args: Record<string, never>; result: boolean };
    };
  };
  SplitButton: {
    invoke: {
      open_menu: { args: Record<string, never>; result: null };
      set_label: { args: { label: string }; result: null };
      set_icon: { args: { icon: BuiltinIconDescriptor | null }; result: null };
      set_variant: { args: { variant: ButtonVariant }; result: null };
      set_control_size: { args: { size: 'xs' | 'sm' | 'md' | 'lg' }; result: null };
      set_default_disabled: { args: { disabled: boolean }; result: null };
      set_disabled: { args: { disabled: boolean }; result: null };
      set_items: { args: { items: MenuItemDescriptor[] }; result: null };
      set_menu_name: { args: { name: string }; result: null };
    };
    query: FocusQueries & {
      menu: { args: Record<string, never>; result: NativeRef<'Menu'> };
      is_open: { args: Record<string, never>; result: boolean };
      is_disabled: { args: Record<string, never>; result: boolean };
    };
  };
  CopyButton: {
    invoke: {
      copy: { args: Record<string, never>; result: null };
      set_text: { args: { text: string }; result: null };
      set_label: { args: { label: string | null }; result: null };
      set_glyph_only: { args: { name: string | null }; result: null };
      set_variant: { args: { variant: ButtonVariant }; result: null };
      set_control_size: { args: { size: 'xs' | 'sm' | 'md' | 'lg' }; result: null };
      set_confirmation: { args: { confirmation_ms: number }; result: null };
      set_disabled: { args: { disabled: boolean }; result: null };
    };
    query: FocusQueries & {
      state: { args: Record<string, never>; result: { state: 'idle' | 'copied' | 'failed'; reason: string | null } };
      is_disabled: { args: Record<string, never>; result: boolean };
    };
  };
  KeymapEditor: {
    invoke: {
      set_commands: { args: { commands: KeymapCommand[] }; result: null };
      set_query: { args: { query: string }; result: null };
      set_disabled: { args: { disabled: boolean }; result: null };
    };
    query: {
      current_commands: { args: Record<string, never>; result: KeymapCommandResult[] };
      active_command: { args: Record<string, never>; result: string | null };
      is_disabled: { args: Record<string, never>; result: boolean };
    };
  };
  NumberInput: {
    invoke: {
      set_value: { args: { value: number }; result: null };
      set_invalid: { args: { invalid: boolean }; result: null };
      set_disabled: { args: { disabled: boolean }; result: null };
      set_required: { args: { required: boolean }; result: null };
      set_range: { args: { min: number | null; max: number | null }; result: null };
      set_steps: { args: { step: number; page_step: number | null }; result: null };
      set_precision: { args: { precision: number }; result: null };
      set_presentation: { args: { name: string | null; unit: string | null; prefix: string | null; size: 'xs' | 'sm' | 'md' | 'lg' }; result: null };
    };
    query: FocusQueries & {
      current: { args: Record<string, never>; result: number | null };
      shown: { args: Record<string, never>; result: number | null };
      is_disabled: { args: Record<string, never>; result: boolean };
      is_invalid: { args: Record<string, never>; result: boolean };
      invalid_reason: { args: Record<string, never>; result: string | null };
      can_step: { args: { delta: number }; result: boolean };
    };
  };
  TransferList: {
    invoke: {
      set_query: { args: { query: string }; result: null };
      set_items: { args: { source: TransferItem[]; target: TransferItem[] }; result: null };
      set_selection: { args: { source: string[]; target: string[] }; result: null };
      set_labels: { args: { source: string; target: string }; result: null };
      set_control_size: { args: { size: 'xs' | 'sm' | 'md' | 'lg' }; result: null };
      set_disabled: { args: { disabled: boolean }; result: null };
    };
    query: { is_disabled: { args: Record<string, never>; result: boolean } };
  };
  SearchInput: {
    invoke: {
      set_value: { args: { value: string }; result: null };
      set_name: { args: { name: string }; result: null };
      set_placeholder: { args: { placeholder: string }; result: null };
      set_disabled: { args: { disabled: boolean }; result: null };
      set_presentation: { args: { name: string | null; placeholder: string | null; size: 'xs' | 'sm' | 'md' | 'lg' }; result: null };
    };
    query: FocusQueries & {
      value: { args: Record<string, never>; result: string };
      is_disabled: { args: Record<string, never>; result: boolean };
    };
  };
  FormField: { invoke: Record<string, never>; query: {
    is_invalid: { args: Record<string, never>; result: boolean };
    is_validating: { args: Record<string, never>; result: boolean };
  } };
}
