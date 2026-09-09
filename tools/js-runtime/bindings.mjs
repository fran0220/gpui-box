/** Implemented adapters only. A name in the Rust catalog is not automatically a JS binding. */
import { kitSchemas, kitMethods } from './kit-schema.mjs';

export const bindings = {
  Button: { api: 'gpui.button', status: 'partial', methods: ['new', 'label', 'disabled', 'on_click'] },
  ...Object.fromEntries(Object.entries(kitSchemas).map(([component, schema]) => [component, {
    api: `gpui.kit.${component}`, status: 'partial',
    props: Object.keys(schema.props.fields), events: Object.keys(schema.events),
    nativeMethods: kitMethods[component] ?? { invoke: {}, query: {} },
  }])),
};
