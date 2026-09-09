import { iconSchema } from './kit-icon-schema.mjs';
import { validateResourceRef } from './resource-schema.mjs';
const str = { type: 'string', max: 16384 };
const id = { type: 'string', min: 1, max: 256 };
const bool = { type: 'boolean' };
const num = { type: 'number', min: -1e9, max: 1e9 };
const unit = { type: 'number', min: 0, max: 1 };
const positive = { type: 'number', min: Number.MIN_VALUE, max: 1000000 };
const integer = { type: 'number', min: 0, max: 1000000, integer: true };
const choice = (...values) => ({ enum: values });
const obj = (fields, required = []) => ({ type: 'object', fields, required });
const arr = items => ({ type: 'array', items, max: 1024 });
const tint = obj({ h: unit, s: unit, l: unit, a: unit }, ['h', 's', 'l', 'a']);
const tone = choice('neutral', 'accent', 'success', 'warning', 'danger', 'info');
const variant = choice('filled', 'light', 'subtle', 'default', 'transparent', 'white');
const size = choice('xs', 'sm', 'md', 'lg');
const color = obj({ kind: choice('palette', 'semantic', 'custom'), name: id, tint }, ['kind']);
const activity = choice('advancing', 'working', 'deliberating', 'signaling', 'transmitting');
const xy = obj({ x: unit, y: unit }, ['x', 'y']);
const state = data => obj({ kind: choice('loading', 'ready', 'empty', 'unavailable', 'error', 'stale'), data, reason: str }, ['kind']);
const simpleState = obj({ kind: choice('loading', 'ready', 'empty', 'unavailable', 'error'), reason: str }, ['kind']);
const avatar = { name: str, presence: choice('unknown', 'online', 'away', 'busy', 'offline'), size: positive, tint, image: obj({ key: { ...id, max: 128 } }, ['key']) };
const progress = { label: str, fraction: unit, count: obj({ done: integer, total: integer }, ['done', 'total']), display: str, stalled: bool, paused: bool, disabled: bool };
const entry = obj({ id, description: str, time: { ...str, nullable: true }, actor: str, tone }, ['id', 'description']);
const traceSpan = obj({ id, label: str, start: unit, end: unit, depth: { ...integer, max: 128 }, state: choice('pending', 'running', 'succeeded', 'failed'), detail: str, duration: str }, ['id', 'label', 'start', 'end']);
const trace = { label: str, spans: arr(traceSpan), axis: obj({ start: str, end: str }, ['start', 'end']), ticks: arr(obj({ position: unit, label: str }, ['position', 'label'])), current: id, disabled: bool };
const ms = { ...num, min: 0 };
const timing = obj({ sampleCount: integer, framesPerSecond: ms, frameBudgetMs: ms, meanDrawMs: ms, p95DrawMs: ms, overBudgetFraction: unit, meanInvalidations: ms, meanDirtyToDrawMs: { ...ms, nullable: true }, meanSubmissionMs: ms, meanDirtyToSubmissionMs: { ...ms, nullable: true }, meanInputToSubmissionMs: { ...ms, nullable: true }, meanInputEvents: ms, drawDurationsMs: arr(ms), submissionDurationsMs: arr(ms) }, ['sampleCount', 'framesPerSecond', 'frameBudgetMs', 'meanDrawMs', 'p95DrawMs', 'overBudgetFraction', 'meanInvalidations', 'meanSubmissionMs', 'meanInputEvents', 'drawDurationsMs', 'submissionDurationsMs']);
const def = (fields, required = [], events = {}, slots = []) => ({ props: obj(fields, required), events, slots });
const nil = choice(null);
export const familySchemas = Object.freeze({
  AnimatedNumber: def({ value: num, format: obj({ decimals: { ...integer, max: 20 }, prefix: str, suffix: str }), spec: obj({ durationMs: integer, delayMs: integer, curve: obj({ x1: unit, y1: num, x2: unit, y2: num }, ['x1', 'y1', 'x2', 'y2']), spring: obj({ stiffness: { ...positive, min: 0.01 }, damping: { ...positive, min: 0.01 }, mass: { ...positive, min: 0.01 } }, ['stiffness', 'damping', 'mass']) }), typeScale: choice('caption', 'label', 'body', 'strong', 'subtitle', 'title', 'code') }, ['value']),
  AttachmentTile: def({ title: str, description: str, state: obj({ kind: choice('ready', 'queued', 'transferring', 'paused', 'processing', 'unavailable', 'failed', 'cancelled'), completed: integer, total: { ...integer, nullable: true }, reason: str }, ['kind']) }, ['title'], {}, ['media', 'title', 'description', 'actions']),
  Avatar: def(avatar, ['name']),
  AvatarGroup: def({ members: arr(obj({ id, ...avatar }, ['id', 'name'])), size: positive, overflow: str }, ['members']),
  Badge: def({ label: str, count: bool, icon: iconSchema, dot: bool, tone, tint, variant, color, size }, ['label']),
  Banner: def({ message: str, tone, title: str, disabled: bool }, ['message'], { dismiss: nil }, ['action']),
  BarLoader: def({ label: str, tint }),
  Bubble: def({ label: str, placement: choice('start', 'end'), grouped: bool, maxWidth: positive }, ['label'], {}, ['content', 'actions']),
  Callout: def({ message: str, tone }, ['message']),
  Card: def({ name: str, variant: choice('elevated', 'filled', 'ghost'), ground: choice('backdrop', 'canvas', 'sunken', 'panel', 'raised', 'overlay'), header: obj({ title: str, subtitle: str }, ['title']), padded: bool, padding: choice(null, 'xxs', 'xs', 'sm', 'md', 'lg', 'xl', 'xxl'), disabled: bool }, [], { click: nil }, ['content', 'media', 'footer', 'headerAction']),
  DescriptionList: def({ items: arr(obj({ id, term: str, value: obj({ kind: choice('text', 'unknown', 'notApplicable', 'redacted'), text: str }, ['kind']), copyable: bool }, ['id', 'term', 'value'])), columns: { ...integer, min: 1, max: 32 }, disabled: bool }, ['items'], { copy: id }),
  EmptyState: def({ title: str, kind: choice('empty', 'unstarted', 'queued', 'blocked', 'cancelled', 'unavailable', 'failed', 'unauthorized'), icon: iconSchema, detail: str }, ['title'], {}, ['action']),
  FailurePanel: def({ reason: str, result: obj({ ok: bool, error: str }, ['ok']), title: str, detail: str, attempts: integer, retrying: bool, disabled: bool }, [], { retry: nil }),
  Heatmap: def({ label: str, tint, rows: arr(obj({ id, label: str, group: str }, ['id', 'label'])), columns: arr(obj({ id, label: str, group: str }, ['id', 'label'])), cells: arr(obj({ id, row: id, column: id, level: { ...integer, max: 4, nullable: true }, label: str, value: str }, ['id', 'row', 'column'])), state: simpleState }, ['label'], {}, ['empty']),
  HighlightedText: def({ text: str, selectable: bool, document: obj({ order: integer, virtualized: bool }, ['order', 'virtualized']), hits: arr(obj({ start: integer, end: integer }, ['start', 'end'])), current: integer, monospace: bool }, ['text']),
  Icon: def({ glyph: iconSchema, name: str, tone: choice('primary', 'muted', 'faint', 'onAccent', 'accent', 'accentStrong', 'danger', 'warning', 'success', 'info'), size, followDirection: bool, motion: choice('none', 'spinning', 'breathing', 'heartbeat', 'bounce', 'wobble', 'pop', 'sparkle') }, ['glyph']),
  ListRow: def({ disabled: bool }, [], { click: nil }, ['content', 'leading', 'trailing']),
  LoadMore: def({ state: choice('idle', 'loading', 'exhausted'), disabled: bool }, [], { more: nil }),
  MetricCard: def({ label: str, state: state(obj({ value: str, delta: str, tone, direction: choice('up', 'down', 'flat'), trend: arr(xy) }, ['value'])), tint }, ['label', 'state'], {}, ['empty', 'failed', 'loading']),
  OutcomePanel: def({ kind: choice('success', 'partial', 'failed'), title: str, detail: str, count: str }, ['kind'], {}, ['action']),
  PerformanceHud: def({ state: obj({ kind: choice('waiting', 'ready', 'unavailable'), data: timing, reason: str }, ['kind']), expanded: bool, disabled: bool }, ['state'], { expanded: bool }),
  ProgressBar: def(progress, [], { cancel: nil }),
  ProgressCircle: def({ ...progress, centre: str }, [], { cancel: nil }),
  PulseLoader: def({ label: str, tint }),
  Rating: def({ label: str, value: { ...num, min: 0, max: 1000, nullable: true }, maximum: { ...integer, min: 1, max: 1000 }, precision: choice('whole', 'half'), clearable: bool, disabled: bool }, [], { change: { ...num, min: 0, max: 1000, nullable: true } }),
  RefreshVeil: def({ label: str }, [], {}, ['content']),
  Skeleton: def({ label: str, rows: { ...integer, max: 128 }, rowHeight: positive, widths: arr(unit), shapes: arr(obj({ kind: choice('row', 'paragraph', 'circle', 'rect', 'card'), width: unit, height: positive, size: positive, lines: { ...integer, min: 1, max: 128 } }, ['kind'])) }),
  SpanTimeline: def(trace, ['label'], { select: id }, ['empty']),
  Spinner: def({ label: str, tint }),
  StageProgress: def({ stages: arr(obj({ id, label: str, status: choice('pending', 'active', 'done', 'failed') }, ['id', 'label', 'status'])) }, ['stages']),
  StaleMark: def({ reason: str, updated: str }, ['reason']),
  StateView: def({ state: obj({ kind: choice('idle', 'queued', 'blocked', 'loading', 'refreshing', 'ready', 'empty', 'unavailable', 'error', 'cancelled'), reason: str, stale: bool }, ['kind']), elapsedMs: ms, fromAsync: bool }, ['state'], {}, ['content', 'empty', 'failed', 'loading']),
  StatusDot: def({ tone, tint, busy: bool, activity }),
  StatusLine: def({ label: str, tone, tint, busy: bool, activity }, ['label']),
  Tag: def({ label: str, tone, tint, variant, color, disabled: bool }, ['label'], { remove: nil }),
  Timeline: { ...def({ entries: arr(entry), groups: arr(obj({ id, label: str, entries: arr(entry) }, ['id', 'label', 'entries'])) }), slotPaths: ['entries', 'groups.entries'] },
  TraceView: def(trace, ['label'], { select: id }, ['empty']),
});
const method = (fields, result) => ({ args: obj(fields, Object.keys(fields)), result });
export const familyMethods = Object.freeze({
  HighlightedText: { invoke: {}, query: { published_hits: method({}, integer) } },
  Icon: { invoke: {}, query: { resolved_size: method({}, positive), resolved_color: method({}, tint), flips_in: method({ direction: choice('ltr', 'rtl') }, bool) } },
});

export function validateFamilyProps(component, p) {
  if (component === 'AnimatedNumber' && p.spec) {
    const spring = Object.hasOwn(p.spec, 'spring');
    if (Object.hasOwn(p.spec, 'curve') === spring || Object.hasOwn(p.spec, 'durationMs') === spring) throw new TypeError('choose spring or duration and curve');
  }
  if (component === 'Heatmap' && p.state && Object.hasOwn(p.state, 'reason') !== ['unavailable', 'error'].includes(p.state.kind)) throw new TypeError('heatmap reason mismatch');
  if (component === 'Timeline') {
    const entries = [...(p.entries ?? []), ...(p.groups ?? []).flatMap(g => g.entries)];
    if (new Set(entries.map(e => e.id)).size !== entries.length) throw new TypeError('duplicate timeline entry identity');
  }
  if (!Object.hasOwn(familySchemas, component)) return;
  if (component === 'Avatar' && p.image) validateResourceRef(p.image);
  if (component === 'AvatarGroup') for (const member of p.members) if (member.image) validateResourceRef(member.image);
  if (component === 'FailurePanel') {
    if (Object.hasOwn(p, 'reason') === Object.hasOwn(p, 'result')) throw new TypeError('choose reason or result');
    if (p.result && Object.hasOwn(p.result, 'error') === p.result.ok) throw new TypeError('result error mismatch');
  }
  if (component === 'StateView' && p.fromAsync && !['idle', 'loading', 'refreshing', 'ready', 'empty', 'unavailable', 'error'].includes(p.state.kind)) throw new TypeError('phase not representable by AsyncValue');
  const state = p.state;
  if (component === 'AttachmentTile' && state) {
    const counts = ['transferring', 'paused'].includes(state.kind);
    if (Object.hasOwn(state, 'reason') !== ['unavailable', 'failed'].includes(state.kind) || Object.hasOwn(state, 'completed') !== counts || (Object.hasOwn(state, 'total') && !counts)) throw new TypeError('attachment state payload mismatch');
  }
  if (['MetricCard', 'PerformanceHud'].includes(component)) {
    const data = ['ready', 'stale'].includes(state.kind);
    const reason = ['unavailable', 'error', 'stale'].includes(state.kind);
    if (Object.hasOwn(state, 'data') !== data || Object.hasOwn(state, 'reason') !== reason) throw new TypeError('state payload mismatch');
  }
  if (component === 'DescriptionList') for (const i of p.items) if (Object.hasOwn(i.value, 'text') !== ['text', 'redacted'].includes(i.value.kind)) throw new TypeError('description value payload mismatch');
  if (component === 'Rating' && p.value != null && p.value > (p.maximum ?? 5)) throw new TypeError('rating exceeds maximum');
  if (component === 'ProgressBar' || component === 'ProgressCircle') if (p.count && p.fraction !== undefined) throw new TypeError('choose count or fraction');
  if (component === 'Skeleton') for (const shape of p.shapes ?? []) {
    const fields = { row: ['width', 'height'], rect: ['width', 'height'], circle: ['size'], paragraph: ['lines'], card: [] }[shape.kind];
    for (const key of ['width', 'height', 'size', 'lines']) if (Object.hasOwn(shape, key) !== fields.includes(key)) throw new TypeError('skeleton shape payload mismatch');
  }
  if (component === 'Heatmap') {
    const rows = new Set((p.rows ?? []).map(r => r.id)), columns = new Set((p.columns ?? []).map(c => c.id));
    for (const cell of p.cells ?? []) if (!rows.has(cell.row) || !columns.has(cell.column)) throw new TypeError('unknown heatmap axis identity');
  }
  if (component === 'TraceView' || component === 'SpanTimeline') for (const span of p.spans ?? []) if (span.start > span.end) throw new TypeError('reversed span');
  if (p.color) {
    if (p.color.kind === 'custom') { if (!p.color.tint || p.color.name !== undefined) throw new TypeError('custom color payload mismatch'); }
    else {
      if (!p.color.name || p.color.tint !== undefined) throw new TypeError('named color payload mismatch');
      if (p.color.kind === 'semantic' && !['accent', 'accentStrong', 'danger', 'warning', 'success', 'info'].includes(p.color.name)) throw new TypeError('unknown semantic color');
    }
  }
}
