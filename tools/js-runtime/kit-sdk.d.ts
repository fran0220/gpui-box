export type KitSize = 'xs' | 'sm' | 'md' | 'lg';
export interface ControlProps { disabled?: boolean; size?: KitSize }
export interface ChoiceProps extends ControlProps { label?: string; description?: string }
export interface SelectionItem { id: string; label: string; disabled?: boolean }
export interface KitNode { kind: 'kit'; component: keyof KitAPI; id: string; props: object; slots: Record<string, never>; events: Record<string, string> }
export interface KitAPI {
  Checkbox(id: string, props?: ChoiceProps & { checked?: boolean | null }, events?: { change?(checked: boolean): void }): KitNode;
  Radio(id: string, props?: ChoiceProps & { selected?: boolean }, events?: { select?(): void }): KitNode;
  Switch(id: string, props?: ChoiceProps & { name?: string; on?: boolean; invalid?: boolean }, events?: { change?(on: boolean): void }): KitNode;
  Slider(id: string, props?: ControlProps & { label?: string; min?: number; max?: number; value?: number; high?: number; step?: number; length?: number; display?: string; orientation?: 'horizontal' | 'vertical'; marks?: number[] }, events?: { change?(value: number): void; rangeChange?(value: { low: number; high: number }): void }): KitNode;
  SegmentedControl(id: string, props?: ControlProps & { label?: string; segments?: SelectionItem[]; selected?: string }, events?: { select?(id: string): void }): KitNode;
  TextInput(id: string, props?: ControlProps & { text?: string; name?: string; placeholder?: string; invalid?: boolean; required?: boolean; readOnly?: boolean; secret?: boolean; bare?: boolean; maxLength?: number }, events?: { change?(text: string): void; submit?(): void; cancel?(): void; backspaceAtStart?(): void; focus?(): void; blur?(): void }): KitNode;
  Select(id: string, props?: ControlProps & { options?: SelectionItem[]; selected?: string | null; name?: string; placeholder?: string; invalid?: boolean; clearable?: boolean }, events?: { change?(id: string | null): void; open?(): void; close?(): void }): KitNode;
}
export declare function createKitBindings(registerHandler: (id: string, event: string, handler: (payload: unknown) => unknown) => string): Readonly<KitAPI>;
