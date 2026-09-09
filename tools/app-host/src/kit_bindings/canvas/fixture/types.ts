import type { CanvasFactories, NodeGraphEvent } from '../../../../../js-runtime/kit-canvas-sdk.js';
import type { OverlayFactories, OverlayMethodContracts, MenuItemDescriptor } from '../../../../../js-runtime/kit-overlay-sdk.js';

const sharedItem: MenuItemDescriptor = { kind: 'submenu', id: 'more', label: 'More', items: [{ kind: 'check', id: 'pin', label: 'Pin', checked: true }] };
// @ts-expect-error shared menu alternatives reject fields from other variants
const invalidSharedItem: MenuItemDescriptor = { kind: 'separator', id: 'line', checked: true };
void [sharedItem, invalidSharedItem];

declare const canvas: CanvasFactories;
declare const overlay: OverlayFactories;
canvas.NodeGraph('graph', { offset: { x: 17, y: -31 }, zoom: 0.75, empty: { title: 'Not started', kind: 'unstarted' } }, { node_click(id) { id.toUpperCase(); } }, { 'source:thumbnail': [] });
overlay.HoverCard('preview', { placement: { at: { x: 13, y: 37 } }, grace: 151 }, {}, { content: [] });
overlay.ContextMenu('context', {}, { unavailable(reason) { reason.toUpperCase(); } });
const title: OverlayMethodContracts['Drawer']['invoke']['set_title']['args'] = { title: 'Updated' };
void title;
declare const focus: OverlayMethodContracts['Drawer']['query']['focus_handle']['result'];
overlay.Drawer('focus-drawer', { focus_stops: [focus] });
declare const input: OverlayMethodContracts['CommandPalette']['query']['query_input']['result'];
// @ts-expect-error a TextInput reference is not a FocusHandle reference
overlay.Drawer('wrong-reference', { focus_stops: [input] });
// @ts-expect-error unknown graph event discriminator
const badEvent: NodeGraphEvent = { type: 'click', id: 'a' };
// @ts-expect-error removed lossy navigation representation
const badButton: NodeGraphEvent = { type: 'surface_pressed', position: { x: 1, y: 2 }, button: 'navigate', click_count: 1 };
// @ts-expect-error no native closures are transported
canvas.NodeGraph('graph', { can_connect: () => true });
// @ts-expect-error only typed slot suffixes are admitted
canvas.NodeGraph('graph', {}, {}, { 'source:footer': [] });
// @ts-expect-error menu alternatives are closed
overlay.Menu('menu', { items: [{ kind: 'separator', id: 'sep', label: 'Invalid' }] });
// @ts-expect-error no event surface for Kbd
overlay.Kbd('shortcut', { keystroke: 'k' }, { click() {} });
// @ts-expect-error no extra fields on no-argument commands
const badOpen: OverlayMethodContracts['Drawer']['invoke']['open']['args'] = { arbitrary: 1 };
// @ts-expect-error native method spelling and named argument are exact
const badTitle: OverlayMethodContracts['Drawer']['invoke']['set_title']['args'] = { value: 'Wrong' };
void [badEvent, badButton, badOpen, badTitle];
