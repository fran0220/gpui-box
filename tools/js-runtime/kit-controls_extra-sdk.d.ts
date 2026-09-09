import type { ControlProps, KitNode, SlotNode } from './kit-sdk.js';
import type { BuiltinIconDescriptor } from './kit-icon-sdk.js';
export interface KitColor { h: number; s: number; l: number; a: number }
export interface ControlsExtraBindingValues { Toggle: boolean; ToggleGroup: string[] }
export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'link';
export type ButtonStyle = ButtonVariant | 'filled' | 'light' | 'subtle' | 'default' | 'transparent' | 'white';
export type ControlGround = 'backdrop' | 'canvas' | 'sunken' | 'panel' | 'raised' | 'overlay';
export type ButtonJoin = 'alone' | 'leading' | 'middle' | 'trailing';
export type ControlColor = { palette: string; semantic?: never; custom?: never } | { semantic: 'accent' | 'accentStrong' | 'danger' | 'warning' | 'success' | 'info'; palette?: never; custom?: never } | { custom: KitColor; palette?: never; semantic?: never };
export interface NativeButtonProps extends ControlProps { accessibleName?: string; semanticParent?: string; icon?: BuiltinIconDescriptor; variant?: ButtonStyle; color?: ControlColor; ground?: ControlGround; join?: ButtonJoin; loading?: boolean }
export interface ToggleItem { id: string; label: string; icon?: BuiltinIconDescriptor; iconOnly?: boolean; disabled?: boolean }
export interface FilterCondition { id: string; field: string; operator: string; value: string; tone?: 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info' }
export interface TransferItem { id: string; label: string; disabled?: boolean }
export interface ControlsExtraFactories {
  TransferList(id: string, props?: ControlProps & { source?: TransferItem[]; target?: TransferItem[]; sourceSelected?: string[]; targetSelected?: string[]; sourceLabel?: string; targetLabel?: string; query?: string }, events?: { toggleSource?(id: string): void; toggleTarget?(id: string): void; moveToTarget?(): void; moveToSource?(): void; queryChange?(query: string): void }): KitNode;
  SettingsRow(id: string, props: { label: string; description?: string; labelWidth?: number; badge?: string; value?: string; searchTerms?: string[]; managed?: string }, events?: Record<string, never>, slots?: { control?: SlotNode[] }): KitNode;
  SearchInput(id: string, props?: ControlProps & { name?: string; placeholder?: string; value?: string }, events?: { change?(value: string): void; submit?(): void; cancel?(): void; backspaceAtStart?(): void; focus?(): void; blur?(): void }): KitNode;
  Button(id: string, props?: NativeButtonProps & { label?: string; accessibleDescription?: string; iconOnly?: boolean; iconPosition?: 'leading' | 'trailing'; fullWidth?: boolean; checkedState?: boolean }, events?: { click?(): void }): KitNode;
  IconButton(id: string, props: NativeButtonProps & { icon: BuiltinIconDescriptor; accessibleName: string }, events?: { click?(): void }): KitNode;
  Toggle(id: string, props?: ControlProps & { label?: string; accessibleName?: string; semanticParent?: string; icon?: BuiltinIconDescriptor; iconOnly?: boolean; variant?: ButtonVariant; ground?: ControlGround; join?: ButtonJoin; pressed?: boolean }, events?: { press?(pressed: boolean): void }): KitNode;
  ToggleGroup(id: string, props?: ControlProps & { label?: string; items?: ToggleItem[]; pressed?: string[]; selection?: 'any' | 'atMostOne'; variant?: ButtonVariant; ground?: ControlGround }, events?: { change?(value: { pressed: string[]; changed: string }): void }): KitNode;
  ColorPicker(id: string, props: { disabled?: boolean; value: KitColor; alpha?: boolean; presets?: KitColor[]; recent?: KitColor[] }, events?: { change?(color: KitColor): void }): KitNode;
  ColorSwatch(id: string, props: { disabled?: boolean; color: KitColor; selected?: boolean }, events?: { click?(color: KitColor): void }): KitNode;
  FormField(id: string, props: { label: string; control?: string; description?: string; validation?: 'pending' | 'validating' | 'invalid' | 'valid'; reason?: string; error?: string; hint?: string; required?: boolean }, events?: Record<string, never>, slots?: { content?: SlotNode[] }): KitNode;
  FilterBar(id: string, props?: ControlProps & { conditions?: FilterCondition[]; countState?: 'unknown' | 'counting' | 'known' | 'unavailable'; count?: number; countReason?: string; noun?: string; addLabel?: string; clearLabel?: string }, events?: { add?(): void; remove?(id: string): void; clear?(): void }, slots?: { add_control?: SlotNode[] }): KitNode;
}
export interface ControlsExtraMethodContracts {
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
    query: {
      value: { args: Record<string, never>; result: string };
      is_disabled: { args: Record<string, never>; result: boolean };
    };
  };
  FormField: { invoke: Record<string, never>; query: {
    is_invalid: { args: Record<string, never>; result: boolean };
    is_validating: { args: Record<string, never>; result: boolean };
  } };
}
