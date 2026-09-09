const last = gpui.state('Fixture host has not received a numeric request');
gpui.mount(() => gpui.column('number.fixture', [
  gpui.text('number.title', 'Native numeric controls · fixture data'),
  gpui.kit.NumberInput('number.ready', { value: 7.5, min: -3, max: 9, step: 2, precision: 1, name: 'Duration', unit: 'ms' }, {
    change: value => last.set(`Requested ${value}; host value remains 7.5`),
    unparsable: () => last.set('Not a number; original text remains visible'),
  }),
  gpui.text('number.last', last.get()),
  gpui.text('number.invalid.label', 'Out of range stays visible, never silently clamped'),
  gpui.kit.NumberInput('number.invalid', { value: -4.25, min: -2, max: 8, precision: 2, prefix: '$', invalid: true, name: 'Refused amount' }),
  gpui.text('number.disabled.label', 'Disabled native field and step controls'),
  gpui.kit.NumberInput('number.disabled', { value: 2.5, disabled: true, precision: 1, name: 'Disabled amount' }),
  gpui.text('number.empty.label', 'Unseeded is empty, not zero'),
  gpui.kit.NumberInput('number.empty', { min: 5, max: 17, name: 'Unseeded amount' }),
]));
