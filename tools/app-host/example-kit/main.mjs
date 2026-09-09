const checked = gpui.state(false);
const radio = gpui.state(false);
const on = gpui.state(true);
const level = gpui.state(23);
const segment = gpui.state('first');
const text = gpui.state('Retained native input');
const selection = gpui.state('second');
const items = [{ id: 'first', label: 'First' }, { id: 'second', label: 'Second' }];
gpui.mount(() => gpui.column('controls', [
  gpui.text('title', 'JavaScript-created native Kit controls'),
  gpui.kit.Checkbox('check', { label: 'Checked state', checked: checked.get() }, { change: value => checked.set(value) }),
  gpui.kit.Radio('radio', { label: 'Select radio', selected: radio.get() }, { select: () => radio.set(true) }),
  gpui.kit.Switch('switch', { label: 'Native switch', on: on.get() }, { change: value => on.set(value) }),
  gpui.kit.Slider('slider', { label: 'Volume', min: 0, max: 100, value: level.get(), length: 300 }, { change: value => level.set(value) }),
  gpui.kit.SegmentedControl('segments', { label: 'Segment', segments: items, selected: segment.get() }, { select: value => segment.set(value) }),
  gpui.kit.TextInput('input', { name: 'Native text', text: text.get(), placeholder: 'Type here' }, { change: value => text.set(value) }),
  gpui.kit.Select('select', { name: 'Native select', options: items, selected: selection.get() }, { change: value => selection.set(value) }),
  gpui.text('result', `Checked: ${checked.get()} · radio: ${radio.get()} · switch: ${on.get()} · volume: ${level.get()}`),
]));
