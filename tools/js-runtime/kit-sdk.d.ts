export type KitSize = 'xs' | 'sm' | 'md' | 'lg';
export interface ControlProps { disabled?: boolean; size?: KitSize }
export interface ChoiceProps extends ControlProps { label?: string; description?: string }
export interface SelectionItem { id: string; label: string; disabled?: boolean }
export interface SelectOption extends SelectionItem { description?: string; group?: string }
export interface SlotNode { kind: string; id: string }
export type KitSlots = Record<string, SlotNode[]>;
export interface KitNode<C extends keyof KitFactories = keyof KitFactories> { kind: 'kit'; component: C; id: string; props: object; slots: KitSlots; events: Record<string, string> }
export type KitAPI = { [C in keyof KitFactories]: (...args: Parameters<KitFactories[C]>) => KitNode<C> };
export interface KitFactories {
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
type MethodResult<S> = S extends { result: infer R } ? R : never;
export type KitInvoke = <C extends keyof KitMethodContracts, M extends keyof KitMethodContracts[NoInfer<C>]['invoke']>(target: Pick<KitNode<C>, 'id' | 'component'>, method: M, ...args: MethodArguments<KitMethodContracts[C]['invoke'][M]>) => Promise<MethodResult<KitMethodContracts[C]['invoke'][M]>>;
export type KitQuery = <C extends keyof KitMethodContracts, M extends keyof KitMethodContracts[NoInfer<C>]['query']>(target: Pick<KitNode<C>, 'id' | 'component'>, method: M, ...args: MethodArguments<KitMethodContracts[C]['query'][M]>) => Promise<MethodResult<KitMethodContracts[C]['query'][M]>>;
