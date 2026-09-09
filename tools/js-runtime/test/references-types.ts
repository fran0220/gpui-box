import type { GPUI, NativeRef } from '../sdk.js';
declare const gpui: GPUI;
declare const input: NativeRef<'TextInput'>;
declare const focus: NativeRef<'FocusHandle'>;
declare const menu: NativeRef<'Menu'>;
const submenu: Promise<boolean> = gpui.invoke(menu, 'open_submenu', { id: 'more' });
const menuFocus: Promise<NativeRef<'FocusHandle'>> = gpui.query(menu, 'focus_handle');
gpui.invoke(menu, 'set_items', { items: [{ kind: 'submenu', id: 'more', label: 'More', items: [{ kind: 'command', id: 'pin', label: 'Pin' }] }] });
// @ts-expect-error Menu does not expose TextInput setters
gpui.invoke(menu, 'set_value', { value: 'wrong' });
// @ts-expect-error Menu children use the shared discriminated family grammar
gpui.invoke(menu, 'set_items', { items: [{ kind: 'check', id: 'pin', label: 'Pin' }] });
const value: Promise<string> = gpui.query(input, 'value');
const focused: Promise<boolean> = gpui.query(focus, 'is_focused');
gpui.invoke(input, 'set_text_quietly', { value: 'É🙂' });
gpui.invoke(focus, 'focus');
gpui.releaseReference(input);
const descriptor = gpui.kit.TextInput('source');
const issued: Promise<NativeRef<'FocusHandle'>> = gpui.query(descriptor, 'focus_handle');
// @ts-expect-error plain wire markers cannot fabricate an issued reference
gpui.invoke({ $nativeRef: 'native-1', type: 'FocusHandle' }, 'focus');
// @ts-expect-error reference kind fixes its dispatcher
gpui.invoke(focus, 'set_value', { value: 'wrong kind' });
// @ts-expect-error commands cannot be queried
gpui.query(input, 'set_value', { value: 'wrong mode' });
// @ts-expect-error method arguments remain typed
gpui.invoke(input, 'set_value', { value: 37 });
void [value, focused, issued, submenu, menuFocus];
