import type { FamilyFactories, FamilyMethodContracts } from '../../../../../js-runtime/kit-display-sdk.js';
declare const display: FamilyFactories;
display.AnimatedNumber('total', { value: 12.5, spec: { spring: { stiffness: 170, damping: 23, mass: 1.2 } } });
display.FailurePanel('failed', { result: { ok: false, error: 'Refused' } });
// @ts-expect-error failed result requires a reason
display.FailurePanel('bad', { result: { ok: false } });
// @ts-expect-error spring and duration are exclusive
display.AnimatedNumber('bad', { value: 1, spec: { spring: { stiffness: 1, damping: 1, mass: 1 }, durationMs: 30 } });
display.Avatar('ada', { name: 'Ada', image: { key: 'approved-image' } });
display.Icon('back', { glyph: { key: 'arrow-left', weight: 'fill' }, tone: 'accent' });
display.MetricCard('metric', { label: 'Revenue', state: { kind: 'stale', data: { value: '$17.3' }, reason: 'Refresh refused' } });
display.Heatmap('heat', { label: 'Activity', state: { kind: 'ready' } });
display.Skeleton('skeleton', { shapes: [{ kind: 'circle', size: 32 }] });
// @ts-expect-error arbitrary image IO is not an adapter source
display.Avatar('bad', { name: 'Bad', image: '/etc/passwd' });
// @ts-expect-error exact catalog membership
display.Icon('bad', { glyph: { key: 'not-in-the-catalog' } });
// @ts-expect-error exact weights
display.Icon('bad', { glyph: { key: 'arrow-left', weight: 'bold' } });
// @ts-expect-error stale requires last verified data
display.MetricCard('bad', { label: 'Revenue', state: { kind: 'stale', reason: 'Offline' } });
// @ts-expect-error circle has size, not width
display.Skeleton('bad', { shapes: [{ kind: 'circle', width: 32 }] });
const args: FamilyMethodContracts['Icon']['query']['flips_in']['args'] = { direction: 'rtl' };
void args;
// @ts-expect-error display native builders have no set_value command
type AbsentCommand = FamilyMethodContracts['Icon']['invoke']['set_value'];
