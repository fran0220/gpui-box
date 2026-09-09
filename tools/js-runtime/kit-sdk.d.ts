import type { ControlsExtraFactories } from './kit-controls_extra-sdk.js';
import type { NavigationExtraFactories } from './kit-navigation_extra-sdk.js';
import type { LayoutExtraFactories } from './kit-layout_extra-sdk.js';
import type { DatetimeFactories } from './kit-datetime-sdk.js';
export type KitSize = 'xs' | 'sm' | 'md' | 'lg';
export interface ControlProps { disabled?: boolean; size?: KitSize }
export interface ChoiceProps extends ControlProps { label?: string; description?: string }
export interface SelectionItem { id: string; label: string; disabled?: boolean }
export interface SelectOption extends SelectionItem { description?: string; group?: string }
export interface SlotNode { kind: string; id: string }
export type KitSlots = Record<string, SlotNode[]>;
export interface KitNode<C extends keyof KitFactories = keyof KitFactories> { kind: 'kit'; component: C; id: string; props: object; slots: KitSlots; events: Record<string, string> }
export type KitAPI = { [C in keyof KitFactories]: (...args: Parameters<KitFactories[C]>) => KitNode<C> };
export interface KitFactories extends ControlsExtraFactories, NavigationExtraFactories, LayoutExtraFactories, DatetimeFactories {
  Checkbox(id: string, props?: ChoiceProps & { checked?: boolean | null }, events?: { change?(checked: boolean): void }): KitNode;
  Radio(id: string, props?: ChoiceProps & { selected?: boolean }, events?: { select?(): void }): KitNode;
  Switch(id: string, props?: ChoiceProps & { name?: string; on?: boolean; invalid?: boolean }, events?: { change?(on: boolean): void }): KitNode;
  Slider(id: string, props?: ControlProps & { label?: string; min?: number; max?: number; value?: number; high?: number; step?: number; length?: number; display?: string; orientation?: 'horizontal' | 'vertical'; marks?: number[] }, events?: { change?(value: number): void; rangeChange?(value: { low: number; high: number }): void }): KitNode;
  SegmentedControl(id: string, props?: ControlProps & { label?: string; segments?: SelectionItem[]; selected?: string }, events?: { select?(id: string): void }): KitNode;
  TextInput(id: string, props?: ControlProps & { text?: string; name?: string; placeholder?: string; invalid?: boolean; required?: boolean; readOnly?: boolean; secret?: boolean; bare?: boolean; maxLength?: number }, events?: { change?(text: string): void; submit?(): void; cancel?(): void; backspaceAtStart?(): void; focus?(): void; blur?(): void; clipboardDenied?(reason: 'missingOwner' | 'denied'): void }): KitNode;
  Select(id: string, props?: ControlProps & { options?: SelectOption[]; selected?: string | null; name?: string; placeholder?: string; invalid?: boolean; clearable?: boolean }, events?: { change?(id: string | null): void; open?(): void; close?(): void }): KitNode;
  Pagination(id: string, props?: ControlProps & { page?: number; totalPages?: number; hasNext?: boolean; siblings?: number }, events?: { select?(page: number): void }): KitNode;
  Tabs(id: string, props?: ControlProps & { tabs?: (SelectionItem & { badge?: string; closable?: boolean })[]; selected?: string; capsules?: boolean; scrolling?: boolean; overflowAfter?: number }, events?: { select?(id: string): void; close?(id: string): void }): KitNode;
  Accordion(id: string, props?: { size?: KitSize; sections?: { id: string; title: string; description?: string; disabled?: boolean }[]; expanded?: string[]; exclusive?: boolean }, events?: { toggle?(value: { id: string; expanded: boolean }): void }, slots?: KitSlots): KitNode;
  ScrollArea(id: string, props?: { axis?: 'vertical' | 'horizontal' | 'both'; label?: string; width?: number; height?: number; fitHeight?: boolean }, events?: Record<string, never>, slots?: { content?: SlotNode[] }): KitNode;
  SplitPane(id: string, props?: { axis?: 'horizontal' | 'vertical'; ratio?: number; minStart?: number; minEnd?: number; step?: number; collapsible?: boolean; handleLabel?: string }, events?: { resize?(ratio: number): void; collapse?(side: 'start' | 'end'): void }, slots?: { start?: SlotNode[]; end?: SlotNode[] }): KitNode;
  Divider(id: string, props?: { label?: string; axis?: 'horizontal' | 'vertical' }): KitNode;
  List(id: string, props?: ControlProps & { rows?: (SelectionItem & { within?: string })[]; selected?: string; rowHeight?: number; visibleRows?: number; flowing?: boolean; anchoredToEnd?: boolean; fills?: boolean; arriving?: boolean; reorderable?: boolean }, events?: { select?(id: string): void; reorder?(intent: { id: string; source: string; anchor: string; position: 'before' | 'after' | 'into' }): void }, slots?: KitSlots): KitNode;
  Popover(id: string, props?: { trigger?: string; placement?: 'above' | 'below'; hang?: 'start' | 'end'; dismissable?: boolean }, events?: { open?(): void; close?(): void; dismiss?(): void }, slots?: { content?: SlotNode[] }): KitNode;
  Dialog(id: string, props?: { title?: string; description?: string; confirmLabel?: string; cancelLabel?: string; destructive?: boolean; dismissable?: boolean }, events?: { open?(): void; close?(): void; confirm?(): void; cancel?(): void; dismiss?(): void }, slots?: { content?: SlotNode[] }): KitNode;
}
export declare function createKitBindings(registerHandler: (id: string, event: string, handler: (payload: unknown) => unknown) => string): Readonly<KitAPI>;
// Generated from kitMethods by generateKitMethodTypes.
export interface KitMethodContracts {
  TransferList: {
    invoke: {
      set_query: { args: { "query": string }; result: null };
      set_items: { args: { "source": Array<{ "id": string; "label": string; "disabled"?: boolean }>; "target": Array<{ "id": string; "label": string; "disabled"?: boolean }> }; result: null };
      set_selection: { args: { "source": Array<string>; "target": Array<string> }; result: null };
      set_labels: { args: { "source": string; "target": string }; result: null };
      set_control_size: { args: { "size": "xs" | "sm" | "md" | "lg" }; result: null };
      set_disabled: { args: { "disabled": boolean }; result: null };
    };
    query: {
      is_disabled: { args: Record<string, never>; result: boolean };
    };
  };
  SearchInput: {
    invoke: {
      set_value: { args: { "value": string }; result: null };
      set_name: { args: { "name": string }; result: null };
      set_placeholder: { args: { "placeholder": string }; result: null };
      set_disabled: { args: { "disabled": boolean }; result: null };
      set_presentation: { args: { "name": string | null; "placeholder": string | null; "size": "xs" | "sm" | "md" | "lg" }; result: null };
    };
    query: {
      value: { args: Record<string, never>; result: string };
      is_disabled: { args: Record<string, never>; result: boolean };
    };
  };
  FormField: {
    invoke: {

    };
    query: {
      is_invalid: { args: Record<string, never>; result: boolean };
      is_validating: { args: Record<string, never>; result: boolean };
    };
  };
  AspectRatio: {
    invoke: {

    };
    query: {
      ratio: { args: Record<string, never>; result: number };
    };
  };
  Toolbar: {
    invoke: {

    };
    query: {
      item_count: { args: Record<string, never>; result: number };
    };
  };
  Calendar: {
    invoke: {
      set_disabled: { args: { "disabled": boolean }; result: null };
      set_overlay: { args: { "marks": Array<{ "day": number; "label": string; "tone"?: "neutral" | "accent" | "success" | "warning" | "danger" | "info" }> }; result: null };
      reset_navigation: { args: Record<string, never>; result: null };
      set_selection: { args: { "days": Array<number> }; result: null };
      set_multi: { args: { "multi": boolean }; result: null };
      set_range: { args: { "range": { "start": number; "end"?: number | null } | null }; result: null };
      set_hovered_day: { args: { "day": number | null }; result: null };
      shift: { args: { "delta": number }; result: null };
      show_month: { args: { "month": number }; result: null };
    };
    query: {
      is_disabled: { args: Record<string, never>; result: boolean };
      adapter_snapshot: { args: Record<string, never>; result: { "today": number | null; "weekdays": Array<string>; "clock": { "hourMin": number; "hourMax": number; "minuteMax": number; "secondMax": number; "meridiem": Array<string> | null } } };
      selection: { args: Record<string, never>; result: Array<number> };
      cursor: { args: Record<string, never>; result: number | null };
      hovered_day: { args: Record<string, never>; result: number | null };
      shown_month: { args: Record<string, never>; result: number | null };
    };
  };
  DateInput: {
    invoke: {
      set_disabled: { args: { "disabled": boolean }; result: null };
      set_invalid: { args: { "invalid": boolean }; result: null };
      set_control_size: { args: { "size": "xs" | "sm" | "md" | "lg" }; result: null };
      set_value: { args: { "value": number | null }; result: null };
      set_required: { args: { "required": boolean }; result: null };
      open: { args: Record<string, never>; result: null };
      close: { args: Record<string, never>; result: null };
      toggle: { args: Record<string, never>; result: null };
    };
    query: {
      is_disabled: { args: Record<string, never>; result: boolean };
      field_snapshot: { args: Record<string, never>; result: { "value": string; "cursor": number; "selection": { "start": number; "end": number }; "disabled": boolean } };
      calendar_snapshot: { args: Record<string, never>; result: { "selection": Array<number>; "cursor": number | null; "hoveredDay": number | null; "shownMonth": number | null; "disabled": boolean } };
      current: { args: Record<string, never>; result: number | null };
      parsed_day: { args: Record<string, never>; result: number | null };
      message: { args: Record<string, never>; result: string | null };
      is_open: { args: Record<string, never>; result: boolean };
      shown_text: { args: Record<string, never>; result: string };
      is_invalid: { args: Record<string, never>; result: boolean };
    };
  };
  RangePicker: {
    invoke: {
      set_disabled: { args: { "disabled": boolean }; result: null };
      set_invalid: { args: { "invalid": boolean }; result: null };
      set_overlay: { args: { "marks": Array<{ "day": number; "label": string; "tone"?: "neutral" | "accent" | "success" | "warning" | "danger" | "info" }> }; result: null };
      set_range: { args: { "range": { "start": number; "end"?: number | null } | null }; result: null };
    };
    query: {
      is_disabled: { args: Record<string, never>; result: boolean };
      calendar_snapshot: { args: Record<string, never>; result: { "selection": Array<number>; "cursor": number | null; "hoveredDay": number | null; "shownMonth": number | null; "disabled": boolean } };
      current_range: { args: Record<string, never>; result: { "start": number; "end"?: number | null } | null };
      state: { args: Record<string, never>; result: "unset" | "incomplete" | "complete" | "end before start" };
      blocked: { args: Record<string, never>; result: { "kind": "notApplicable" } | { "kind": "unchecked" } | { "kind": "clear" } | { "kind": "blocked"; "days": Array<{ "day": number; "reason": string }> } };
    };
  };
  TimeInput: {
    invoke: {
      set_disabled: { args: { "disabled": boolean }; result: null };
      set_invalid: { args: { "invalid": boolean }; result: null };
      set_control_size: { args: { "size": "xs" | "sm" | "md" | "lg" }; result: null };
      set_value: { args: { "value": { "hour": number; "minute": number; "second"?: number | null; "meridiem"?: 0 | 1 | null } }; result: null };
      set_seconds: { args: { "seconds": boolean }; result: null };
    };
    query: {
      is_disabled: { args: Record<string, never>; result: boolean };
      current: { args: Record<string, never>; result: { "hour": number; "minute": number; "second"?: number | null; "meridiem"?: 0 | 1 | null } };
      active_segment: { args: Record<string, never>; result: "hour" | "minute" | "second" | "meridiem" };
      clock: { args: Record<string, never>; result: { "hourMin": number; "hourMax": number; "minuteMax": number; "secondMax": number; "meridiem": Array<string> | null } };
    };
  };
  TextInput: {
    invoke: {
      set_name: { args: { "name": string }; result: null };
      set_placeholder: { args: { "placeholder": string }; result: null };
      set_value: { args: { "value": string }; result: null };
      set_text_quietly: { args: { "value": string }; result: null };
      set_secret: { args: { "secret": boolean }; result: null };
      set_bare: { args: { "bare": boolean }; result: null };
      set_max_length: { args: { "max_length": number | null }; result: null };
      set_disabled: { args: { "disabled": boolean }; result: null };
      set_read_only: { args: { "read_only": boolean }; result: null };
      set_required: { args: { "required": boolean }; result: null };
      set_invalid: { args: { "invalid": boolean }; result: null };
      set_control_size: { args: { "size": "xs" | "sm" | "md" | "lg" }; result: null };
    };
    query: {
      value: { args: Record<string, never>; result: string };
      is_empty: { args: Record<string, never>; result: boolean };
      is_disabled: { args: Record<string, never>; result: boolean };
      is_secret: { args: Record<string, never>; result: boolean };
      selected_range: { args: Record<string, never>; result: { "start": number; "end": number } };
      cursor_offset: { args: Record<string, never>; result: number };
      focus_handle: { args: Record<string, never>; result: { "$nativeRef": string; "type": "FocusHandle" } };
    };
  };
  Select: {
    invoke: {
      set_name: { args: { "name": string }; result: null };
      set_placeholder: { args: { "placeholder": string | null }; result: null };
      set_options: { args: { "options": Array<{ "id": string; "label": string; "disabled"?: boolean; "description"?: string; "group"?: string }> }; result: null };
      set_selected: { args: { "id": string | null }; result: null };
      set_disabled: { args: { "disabled": boolean }; result: null };
      set_invalid: { args: { "invalid": boolean }; result: null };
      set_clearable: { args: { "clearable": boolean }; result: null };
      set_control_size: { args: { "size": "xs" | "sm" | "md" | "lg" }; result: null };
    };
    query: {
      selected_id: { args: Record<string, never>; result: string | null };
      is_open: { args: Record<string, never>; result: boolean };
      is_disabled: { args: Record<string, never>; result: boolean };
      selected_option: { args: Record<string, never>; result: { "id": string; "label": string; "disabled": boolean; "description": string | null; "group": string | null } | null };
    };
  };
  Popover: {
    invoke: {
      open: { args: Record<string, never>; result: null };
      close: { args: Record<string, never>; result: null };
      toggle: { args: Record<string, never>; result: null };
      dismiss: { args: Record<string, never>; result: null };
      set_trigger: { args: { "label": string }; result: null };
      set_dismissable: { args: { "dismissable": boolean }; result: null };
      set_placement: { args: { "placement": "above" | "below" }; result: null };
      set_hang: { args: { "hang": "start" | "end" }; result: null };
    };
    query: {
      is_open: { args: Record<string, never>; result: boolean };
      is_dismissable: { args: Record<string, never>; result: boolean };
    };
  };
  Dialog: {
    invoke: {
      open: { args: Record<string, never>; result: null };
      close: { args: Record<string, never>; result: null };
      confirm: { args: Record<string, never>; result: null };
      cancel: { args: Record<string, never>; result: null };
      dismiss: { args: Record<string, never>; result: null };
      set_title: { args: { "title": string }; result: null };
      set_description: { args: { "description": string | null }; result: null };
      set_confirm_label: { args: { "label": string | null }; result: null };
      set_cancel_label: { args: { "label": string | null }; result: null };
      set_dismissable: { args: { "dismissable": boolean }; result: null };
      set_destructive: { args: { "destructive": boolean }; result: null };
    };
    query: {
      is_open: { args: Record<string, never>; result: boolean };
      is_dismissable: { args: Record<string, never>; result: boolean };
    };
  };
}
type MethodArguments<S> = S extends { args: infer A } ? {} extends A ? [args?: A] : [args: A] : never;
type MethodResult<S> = S extends { result: infer R } ? R extends { $nativeRef: string; type: infer T extends keyof import('./reference-sdk.js').NativeReferenceContracts } ? import('./reference-sdk.js').NativeRef<T> : R : never;
export type KitInvoke = <C extends keyof KitMethodContracts, M extends keyof KitMethodContracts[NoInfer<C>]['invoke']>(target: Pick<KitNode<C>, 'id' | 'component'>, method: M, ...args: MethodArguments<KitMethodContracts[C]['invoke'][M]>) => Promise<MethodResult<KitMethodContracts[C]['invoke'][M]>>;
export type KitQuery = <C extends keyof KitMethodContracts, M extends keyof KitMethodContracts[NoInfer<C>]['query']>(target: Pick<KitNode<C>, 'id' | 'component'>, method: M, ...args: MethodArguments<KitMethodContracts[C]['query'][M]>) => Promise<MethodResult<KitMethodContracts[C]['query'][M]>>;
