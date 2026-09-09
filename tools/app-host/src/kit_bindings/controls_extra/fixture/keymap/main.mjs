const last = gpui.state('No request; fixture bindings are caller-owned');
const commands = [
  { id: 'save', label: 'Save document', context: 'Editor', defaults: ['ctrl-s'], bindings: [{ id: 'custom', keystroke: 'ctrl-shift-s', conflict: 'Also assigned to export', provenance: 'Fixture override' }] },
  { id: 'locked', label: 'Managed command', bindings: [{ id: 'managed', keystroke: 'ctrl-l' }], refusal: 'Organization policy owns this binding' },
];
gpui.mount(() => gpui.column('keymap.fixture', [
  gpui.text('keymap.title', 'Native keymap editor · fixture data'),
  gpui.kit.KeymapEditor('keymap.ready', { commands }, {
    remove: value => last.set(`Removal refused for ${value.command_id}/${value.binding_id}`),
    reset: value => last.set(`Reset requested for ${value.command_id}`),
    addCaptured: value => last.set(`Captured ${value.keystroke} for ${value.command_id}`),
    recordingCancelled: value => last.set(`Recording cancelled for ${value.command_id}`),
  }),
  gpui.text('keymap.last', last.get()),
  gpui.text('keymap.disabled.label', 'Disabled editor keeps verified bindings, without action handlers'),
  gpui.kit.KeymapEditor('keymap.disabled', { commands: [{ id: 'save', label: 'Save document', bindings: commands[0].bindings, defaults: commands[0].defaults }], disabled: true }),
]));
