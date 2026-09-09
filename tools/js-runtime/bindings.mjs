/** Implemented adapters only. A name in the Rust catalog is not automatically a JS binding. */
import { kitSchemas, kitMethods } from './kit-schema.mjs';
import { familySchemas as display } from './kit-display-schema.mjs';
import { familySchemas as charts } from './kit-charts-schema.mjs';
import { familySchemas as agent } from './kit-agent-schema.mjs';
import { familySchemas as game } from './kit-game-effects-schema.mjs';
import { familySchemas as canvas } from './kit-canvas-schema.mjs';
import { familySchemas as overlay, familyReferenceMethods } from './kit-overlay-schema.mjs';
import { familySchemas as content } from './kit-content-schema.mjs';
import { familySchemas as media } from './kit-media-schema.mjs';
import { familySchemas as data } from './kit-data-schema.mjs';
import { familySchemas as structured } from './kit-structured-schema.mjs';

// These families are registered in the native host. Registration is not a
// claim of complete component, transport, or platform feature parity.
const registeredNative = new Set(Object.keys({ ...display, ...charts, ...agent, ...game,
  ...canvas, ...overlay, ...content, ...media, ...data, ...structured }));

// This is a discoverability list, not a flattened validation schema: alternatives
// such as AgentRoster's agents/run remain exact-one closed objects at runtime.
function propertyNames(schema, definitions = schema.$defs) {
  if (schema.$ref) return propertyNames(definitions[schema.$ref], definitions);
  if (schema.oneOf) return [...new Set(schema.oneOf.flatMap(branch => propertyNames(branch, definitions)))];
  return Object.keys(schema.fields);
}

// Snapshot aliases remain data-only; focus_handle names its native trait authority.
const nativeMethodSources = {
  ...Object.fromEntries(Object.entries(kitMethods)
    .filter(([, modes]) => modes.query?.focus_handle)
    .map(([component]) => [component, { focus_handle: 'gpui::window::Focusable::focus_handle' }])),
  Calendar: { adapter_snapshot: 'adapter' },
  DateInput: { field_snapshot: 'field', calendar_snapshot: 'calendar' },
  RangePicker: { calendar_snapshot: 'calendar' },
  SankeyChart: { layout: 'gpui_kit::display::plot::SankeyData::layout' },
  Sparkline: { published_points: 'gpui_kit::display::sparkline::SparklineReading::published_points' },
};

export const bindings = {
  Button: { api: 'gpui.button', status: 'partial', methods: ['new', 'label', 'disabled', 'on_click'] },
  ...Object.fromEntries(Object.entries(kitSchemas).map(([component, schema]) => [component, {
    api: `gpui.kit.${component}`, status: component === 'CinematicEffect' ? 'fallback-only' : 'partial',
    props: propertyNames(schema.props), events: Object.keys(schema.events),
    nativeMethods: kitMethods[component] ?? { invoke: {}, query: {} },
    ...(nativeMethodSources[component] ? { nativeMethodSources: nativeMethodSources[component] } : {}),
    ...(registeredNative.has(component) ? { nativeIntegration: 'registered' } : {}),
    ...(familyReferenceMethods[component] ? {
      referenceIntegration: 'registered',
      referenceMethods: Object.fromEntries(Object.entries(familyReferenceMethods[component])
        .map(([mode, methods]) => [mode, Object.keys(methods)])),
    } : {}),
    ...(component === 'CinematicEffect' ? { reason: 'Fallback only; no bounded clip runtime or dotlottie playback' } : {}),
  }])),
};
