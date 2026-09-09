const last = gpui.state('No clipboard access granted to this fixture');
gpui.mount(() => gpui.column('copy.fixture', [
  gpui.text('copy.title', 'Native CopyButton · explicit clipboard refusal'),
  gpui.kit.CopyButton('copy.denied', {text:'Fixture content',label:'Copy fixture'}, {
    failed: reason => last.set(`Refused: ${reason}`),
    copied: () => last.set('Verified readback'),
  }),
  gpui.text('copy.last', last.get()),
  gpui.kit.CopyButton('copy.glyph', {text:'Fixture glyph content',glyphOnly:'Copy fixture value',variant:'ghost',size:'lg'}),
  gpui.kit.CopyButton('copy.disabled', {text:'Disabled fixture',label:'Disabled copy',disabled:true}),
]));
