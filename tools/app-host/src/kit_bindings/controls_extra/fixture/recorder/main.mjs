const last = gpui.state('Fixture bindings; no keymap writes');
gpui.mount(() => gpui.column('recorder.fixture', [
  gpui.text('recorder.title', 'Native KeybindingRecorder · controlled bindings'),
  gpui.text('recorder.last', last.get()),
  gpui.kit.KeybindingRecorder('recorder.ready', {label:'Record fixture shortcut',binding:'ctrl-s'}, {
    started:() => last.set('Native recording started'),
    captured:key => last.set(`Captured intent: ${key}`),
    cancelled:() => last.set('Recording cancelled'),
  }),
  gpui.kit.KeybindingRecorder('recorder.empty', {label:'Unbound fixture'}),
  gpui.kit.KeybindingRecorder('recorder.conflict', {label:'Conflicted fixture',binding:'ctrl-k',conflict:'Refused: fixture binding already assigned'}),
  gpui.kit.KeybindingRecorder('recorder.disabled', {label:'Disabled fixture',binding:'ctrl-p',disabled:true}),
]));
