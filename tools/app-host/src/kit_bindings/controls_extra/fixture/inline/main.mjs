const editing = gpui.state(false);
const last = gpui.state('Fixture values; no external save');
gpui.mount(() => gpui.column('inline.fixture', [
  gpui.text('inline.title', 'Native InlineEdit · retained text and truthful refusal'),
  gpui.text('inline.last', last.get()),
  gpui.kit.InlineEdit('inline.ready', {value:'Editable fixture',editing:editing.get()}, {
    edit:() => { editing.set(true); last.set('Native edit requested'); },
    commit:value => last.set(`Commit intent: ${value}`),
    cancel:() => { editing.set(false); last.set('Edit cancelled'); },
  }),
  gpui.kit.InlineEdit('inline.empty', {placeholder:'Add a fixture note'}, {edit:() => last.set('Empty edit requested')}),
  gpui.kit.InlineEdit('inline.disabled', {value:'Disabled fixture',disabled:true,editing:true}),
  gpui.kit.InlineEdit('inline.failed', {value:'Retained refused fixture',editing:true,failure:'Save refused: fixture policy'}, {commit:value => last.set(`Retry intent: ${value}`)}),
  gpui.kit.InlineEdit('inline.block', {value:'Multiline fixture\nSecond line',multiline:true,editing:true,rows:3}, {commit:value => last.set(`Block intent: ${value}`)}),
]));
