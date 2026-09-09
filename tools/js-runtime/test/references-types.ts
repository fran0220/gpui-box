import type { GPUI, NativeRef } from '../sdk.js';
declare const gpui: GPUI;
declare const rich: NativeRef<'RichTextEditSession'>;
const richEditor = gpui.kit.RichTextEditor('rich', {document:{blocks:[{id:'a',text:'AλZ'}]}});
const richSession: Promise<NativeRef<'RichTextEditSession'>> = gpui.query(richEditor, 'session');
const richDocument = gpui.query(rich, 'document');
richDocument.then(document => {
  const bold: boolean = document.blocks[0].styles[0].style.bold;
  const alignment: 'start' | 'center' | 'end' = document.blocks[0].paragraph.alignment;
  void [bold, alignment];
});
const richUndo: Promise<boolean> = gpui.query(rich, 'can_undo');
// @ts-expect-error mutation must use the editor's allocator, never the session
gpui.invoke(rich, 'apply', {intent:{kind:'undo'}});
// @ts-expect-error even argument-free mutation is unavailable
gpui.invoke(rich, 'forbid_history');
// @ts-expect-error an editor-only query is not a session operation
gpui.query(rich, 'is_disabled');
gpui.releaseReference(rich);
void [richSession, richUndo];
declare const area: NativeRef<'TextArea'>;
const areaSnapshot: Promise<{revision: number; text: string}> = gpui.query(area, 'snapshot');
const areaFocus: Promise<NativeRef<'FocusHandle'>> = gpui.query(area, 'focus_handle');
const editorArea: Promise<NativeRef<'TextArea'>> = gpui.query(gpui.kit.Editor('editor'), 'text_area');
gpui.invoke(area, 'set_value', {value: 'Native λ document'});
// @ts-expect-error TextArea does not inherit SearchField methods
gpui.invoke(area, 'set_query', {text: 'wrong kind'});
// @ts-expect-error TextArea snapshots are full values, not references
const snapshotReference: Promise<NativeRef<'TextArea'>> = gpui.query(area, 'snapshot');
// @ts-expect-error no Editor reference kind exists
declare const editorReference: NativeRef<'Editor'>;
void [areaSnapshot, areaFocus, editorArea];
declare const input: NativeRef<'TextInput'>;
declare const focus: NativeRef<'FocusHandle'>;
declare const menu: NativeRef<'Menu'>;
declare const search: NativeRef<'SearchField'>;
const searchText: Promise<string> = gpui.query(search, 'query_text');
const searchInput: Promise<NativeRef<'TextInput'>> = gpui.query(search, 'query_input');
const searchFocus: Promise<NativeRef<'FocusHandle'>> = gpui.query(search, 'focus_handle');
gpui.invoke(search, 'set_match_case', { on: null });
gpui.invoke(search, 'set_count', { count: { state: 'known', total: 13, current: 4 } });
// @ts-expect-error the borrowed search field is not its nested TextInput
gpui.invoke(search, 'set_value', { value: 'wrong kind' });
// @ts-expect-error known counts require current, including explicit null
gpui.invoke(search, 'set_count', { count: { state: 'known', total: 13 } });
// @ts-expect-error a TextInput child does not become a SearchField reference
const wrongSearch: Promise<NativeRef<'SearchField'>> = gpui.query(search, 'query_input');
void [searchText, searchInput, searchFocus];
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
