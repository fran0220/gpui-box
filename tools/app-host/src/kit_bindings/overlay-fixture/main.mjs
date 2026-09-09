// Explicit non-product fixture for retained native surfaces and nested slots.
const result = gpui.state('No surface opened');
gpui.mount(() => gpui.column('overlay-fixture', [
  gpui.text('overlay-title', 'Retained native overlay fixture'),
  gpui.kit.Popover('overlay-popover', { trigger: 'Open native popover' }, {}, {
    content: [gpui.kit.TextInput('popover-field', { placeholder: 'Retained popover input' })],
  }),
  gpui.button('open-dialog', 'Open native dialog', async () => {
    await gpui.invoke({ id: 'overlay-dialog', component: 'Dialog' }, 'open');
    result.set('Dialog opened through native command');
  }),
  gpui.kit.Dialog('overlay-dialog', { title: 'Native retained dialog', description: 'Close and reopen without replacing the input.', confirmLabel: 'Confirm', cancelLabel: 'Cancel' }, {}, {
    content: [gpui.kit.TextInput('dialog-field', { placeholder: 'Retained dialog input' })],
  }),
  gpui.text('overlay-result', result.get()),
]));
