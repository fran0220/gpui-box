export type KitSize = 'xs' | 'sm' | 'md' | 'lg';
export interface ControlProps { disabled?: boolean; size?: KitSize }
export interface ChoiceProps extends ControlProps { label?: string; description?: string }
export interface SelectionItem { id: string; label: string; disabled?: boolean }
export interface SlotNode { kind: string; id: string }
export type KitSlots = Record<string, SlotNode[]>;
export interface KitNode { kind: 'kit'; component: keyof KitAPI; id: string; props: object; slots: KitSlots; events: Record<string, string> }
export interface KitAPI {
  Checkbox(id: string, props?: ChoiceProps & { checked?: boolean | null }, events?: { change?(checked: boolean): void }): KitNode;
  Radio(id: string, props?: ChoiceProps & { selected?: boolean }, events?: { select?(): void }): KitNode;
  Switch(id: string, props?: ChoiceProps & { name?: string; on?: boolean; invalid?: boolean }, events?: { change?(on: boolean): void }): KitNode;
  Slider(id: string, props?: ControlProps & { label?: string; min?: number; max?: number; value?: number; high?: number; step?: number; length?: number; display?: string; orientation?: 'horizontal' | 'vertical'; marks?: number[] }, events?: { change?(value: number): void; rangeChange?(value: { low: number; high: number }): void }): KitNode;
  SegmentedControl(id: string, props?: ControlProps & { label?: string; segments?: SelectionItem[]; selected?: string }, events?: { select?(id: string): void }): KitNode;
  TextInput(id: string, props?: ControlProps & { text?: string; name?: string; placeholder?: string; invalid?: boolean; required?: boolean; readOnly?: boolean; secret?: boolean; bare?: boolean; maxLength?: number }, events?: { change?(text: string): void; submit?(): void; cancel?(): void; backspaceAtStart?(): void; focus?(): void; blur?(): void }): KitNode;
  Select(id: string, props?: ControlProps & { options?: SelectionItem[]; selected?: string | null; name?: string; placeholder?: string; invalid?: boolean; clearable?: boolean }, events?: { change?(id: string | null): void; open?(): void; close?(): void }): KitNode;
  Pagination(id: string, props?: ControlProps & { page?: number; totalPages?: number; hasNext?: boolean; siblings?: number }, events?: { select?(page: number): void }): KitNode;
  Tabs(id: string, props?: ControlProps & { tabs?: (SelectionItem & { badge?: string; closable?: boolean })[]; selected?: string; capsules?: boolean; scrolling?: boolean; overflowAfter?: number }, events?: { select?(id: string): void; close?(id: string): void }): KitNode;
  Accordion(id: string, props?: { size?: KitSize; sections?: { id: string; title: string; description?: string; disabled?: boolean }[]; expanded?: string[]; exclusive?: boolean }, events?: { toggle?(value: { id: string; expanded: boolean }): void }, slots?: KitSlots): KitNode;
  ScrollArea(id: string, props?: { axis?: 'vertical' | 'horizontal' | 'both'; label?: string; width?: number; height?: number; fitHeight?: boolean }, events?: Record<string, never>, slots?: { content?: SlotNode[] }): KitNode;
  SplitPane(id: string, props?: { axis?: 'horizontal' | 'vertical'; ratio?: number; minStart?: number; minEnd?: number; step?: number; collapsible?: boolean; handleLabel?: string }, events?: { resize?(ratio: number): void; collapse?(side: 'start' | 'end'): void }, slots?: { start?: SlotNode[]; end?: SlotNode[] }): KitNode;
  Divider(id: string, props?: { label?: string; axis?: 'horizontal' | 'vertical' }): KitNode;
}
export declare function createKitBindings(registerHandler: (id: string, event: string, handler: (payload: unknown) => unknown) => string): Readonly<KitAPI>;
