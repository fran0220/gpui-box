import type { KitNode, SlotNode } from './kit-sdk.js';
import type { BuiltinIconDescriptor } from './kit-icon-sdk.js';
import type { ResourceRef } from './resource-sdk.js';
import type { ChartTint, ChartXY, ChartState } from './kit-charts-sdk.js';
export type DisplayTone = 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info';
export type DisplayVariant = 'filled' | 'light' | 'subtle' | 'default' | 'transparent' | 'white';
export type DisplaySize = 'xs' | 'sm' | 'md' | 'lg';
export type DisplayColor = { kind: 'palette'; name: string } | { kind: 'semantic'; name: 'accent' | 'accentStrong' | 'danger' | 'warning' | 'success' | 'info' } | { kind: 'custom'; tint: ChartTint };
export type Activity = 'advancing' | 'working' | 'deliberating' | 'signaling' | 'transmitting';
export interface AvatarProps { name: string; presence?: 'unknown' | 'online' | 'away' | 'busy' | 'offline'; size?: number; tint?: ChartTint; image?: ResourceRef }
export interface ProgressProps { label?: string; fraction?: number; count?: { done: number; total: number }; display?: string; stalled?: boolean; paused?: boolean; disabled?: boolean }
export type DescriptionValue = { kind: 'text' | 'redacted'; text: string } | { kind: 'unknown' | 'notApplicable' };
export interface DescriptionItem { id: string; term: string; value: DescriptionValue; copyable?: boolean }
export interface TimelineEntry { id: string; description: string; time?: string | null; actor?: string; tone?: DisplayTone }
export interface TraceSpan { id: string; label: string; start: number; end: number; depth?: number; state?: 'pending' | 'running' | 'succeeded' | 'failed'; detail?: string; duration?: string }
export interface TraceProps { label: string; spans?: TraceSpan[]; axis?: { start: string; end: string }; ticks?: { position: number; label: string }[]; current?: string; disabled?: boolean }
export interface MetricReading { value: string; delta?: string; tone?: DisplayTone; direction?: 'up' | 'down' | 'flat'; trend?: ChartXY[] }
export interface FrameTimingSummary { sampleCount: number; framesPerSecond: number; frameBudgetMs: number; meanDrawMs: number; p95DrawMs: number; overBudgetFraction: number; meanInvalidations: number; meanDirtyToDrawMs?: number | null; meanSubmissionMs: number; meanDirtyToSubmissionMs?: number | null; meanInputToSubmissionMs?: number | null; meanInputEvents: number; drawDurationsMs: number[]; submissionDurationsMs: number[] }
export type AttachmentState = { kind: 'ready' | 'queued' | 'processing' | 'cancelled' } | { kind: 'unavailable' | 'failed'; reason: string } | { kind: 'transferring' | 'paused'; completed: number; total?: number | null };
export type SkeletonShape = { kind: 'row' | 'rect'; width: number; height: number } | { kind: 'circle'; size: number } | { kind: 'paragraph'; lines: number } | { kind: 'card' };
export type DisplaySlots = { empty?: SlotNode[]; failed?: SlotNode[]; loading?: SlotNode[] };
export interface IconProps { glyph: BuiltinIconDescriptor; name?: string; tone?: 'primary' | 'muted' | 'faint' | 'onAccent' | 'accent' | 'accentStrong' | 'danger' | 'warning' | 'success' | 'info'; size?: DisplaySize; followDirection?: boolean; motion?: 'none' | 'spinning' | 'breathing' | 'heartbeat' | 'bounce' | 'wobble' | 'pop' | 'sparkle' }
export interface FamilyFactories {
  AnimatedNumber(id: string, props: { value: number; format?: { decimals?: number; prefix?: string; suffix?: string }; spec?: { delayMs?: number } & ({ durationMs: number; curve: { x1: number; y1: number; x2: number; y2: number }; spring?: never } | { spring: { stiffness: number; damping: number; mass: number }; durationMs?: never; curve?: never }); typeScale?: 'caption' | 'label' | 'body' | 'strong' | 'subtitle' | 'title' | 'code' }): KitNode;
  AttachmentTile(id: string, props: { title: string; description?: string; state?: AttachmentState }, events?: Record<string, never>, slots?: { media?: SlotNode[]; title?: SlotNode[]; description?: SlotNode[]; actions?: SlotNode[] }): KitNode;
  Avatar(id: string, props: AvatarProps): KitNode;
  AvatarGroup(id: string, props: { members: (AvatarProps & { id: string })[]; size?: number; overflow?: string }): KitNode;
  Badge(id: string, props: { label: string; count?: boolean; icon?: BuiltinIconDescriptor; dot?: boolean; tone?: DisplayTone; tint?: ChartTint; variant?: DisplayVariant; color?: DisplayColor; size?: DisplaySize }): KitNode;
  Banner(id: string, props: { message: string; tone?: DisplayTone; title?: string; disabled?: boolean }, events?: { dismiss?(): void }, slots?: { action?: SlotNode[] }): KitNode;
  BarLoader(id: string, props?: { label?: string; tint?: ChartTint }): KitNode;
  Bubble(id: string, props: { label: string; placement?: 'start' | 'end'; grouped?: boolean; maxWidth?: number }, events?: Record<string, never>, slots?: { content?: SlotNode[]; actions?: SlotNode[] }): KitNode;
  Callout(id: string, props: { message: string; tone?: DisplayTone }): KitNode;
  Card(id: string, props?: { name?: string; variant?: 'elevated' | 'filled' | 'ghost'; ground?: 'backdrop' | 'canvas' | 'sunken' | 'panel' | 'raised' | 'overlay'; header?: { title: string; subtitle?: string }; padded?: boolean; padding?: null | 'xxs' | 'xs' | 'sm' | 'md' | 'lg' | 'xl' | 'xxl'; disabled?: boolean }, events?: { click?(): void }, slots?: { content?: SlotNode[]; media?: SlotNode[]; footer?: SlotNode[]; headerAction?: SlotNode[] }): KitNode;
  DescriptionList(id: string, props: { items: DescriptionItem[]; columns?: number; disabled?: boolean }, events?: { copy?(id: string): void }): KitNode;
  EmptyState(id: string, props: { title: string; kind?: 'empty' | 'unstarted' | 'queued' | 'blocked' | 'cancelled' | 'unavailable' | 'failed' | 'unauthorized'; icon?: BuiltinIconDescriptor; detail?: string }, events?: Record<string, never>, slots?: { action?: SlotNode[] }): KitNode;
  FailurePanel(id: string, props: ({ reason: string; result?: never } | { reason?: never; result: { ok: true; error?: never } | { ok: false; error: string } }) & { title?: string; detail?: string; attempts?: number; retrying?: boolean; disabled?: boolean }, events?: { retry?(): void }): KitNode;
  Heatmap(id: string, props: { label: string; tint?: ChartTint; rows?: { id: string; label: string; group?: string }[]; columns?: { id: string; label: string; group?: string }[]; cells?: { id: string; row: string; column: string; level?: number | null; label?: string; value?: string }[]; state?: { kind: 'loading' | 'ready' | 'empty' } | { kind: 'unavailable' | 'error'; reason: string } }, events?: Record<string, never>, slots?: { empty?: SlotNode[] }): KitNode;
  HighlightedText(id: string, props: { text: string; selectable?: boolean; document?: { order: number; virtualized: boolean }; hits?: { start: number; end: number }[]; current?: number; monospace?: boolean }): KitNode;
  Icon(id: string, props: IconProps): KitNode;
  ListRow(id: string, props?: { disabled?: boolean }, events?: { click?(): void }, slots?: { content?: SlotNode[]; leading?: SlotNode[]; trailing?: SlotNode[] }): KitNode;
  LoadMore(id: string, props?: { state?: 'idle' | 'loading' | 'exhausted'; disabled?: boolean }, events?: { more?(): void }): KitNode;
  MetricCard(id: string, props: { label: string; state: ChartState<MetricReading>; tint?: ChartTint }, events?: Record<string, never>, slots?: DisplaySlots): KitNode;
  OutcomePanel(id: string, props: { kind: 'success' | 'partial' | 'failed'; title?: string; detail?: string; count?: string }, events?: Record<string, never>, slots?: { action?: SlotNode[] }): KitNode;
  PerformanceHud(id: string, props: { state: { kind: 'waiting' } | { kind: 'ready'; data: FrameTimingSummary } | { kind: 'unavailable'; reason: string }; expanded?: boolean; disabled?: boolean }, events?: { expanded?(value: boolean): void }): KitNode;
  ProgressBar(id: string, props?: ProgressProps, events?: { cancel?(): void }): KitNode;
  ProgressCircle(id: string, props?: ProgressProps & { centre?: string }, events?: { cancel?(): void }): KitNode;
  PulseLoader(id: string, props?: { label?: string; tint?: ChartTint }): KitNode;
  Rating(id: string, props?: { label?: string; value?: number | null; maximum?: number; precision?: 'whole' | 'half'; clearable?: boolean; disabled?: boolean }, events?: { change?(value: number | null): void }): KitNode;
  RefreshVeil(id: string, props?: { label?: string }, events?: Record<string, never>, slots?: { content?: SlotNode[] }): KitNode;
  Skeleton(id: string, props?: { label?: string; rows?: number; rowHeight?: number; widths?: number[]; shapes?: SkeletonShape[] }): KitNode;
  SpanTimeline(id: string, props: TraceProps, events?: { select?(id: string): void }, slots?: { empty?: SlotNode[] }): KitNode;
  Spinner(id: string, props?: { label?: string; tint?: ChartTint }): KitNode;
  StageProgress(id: string, props: { stages: { id: string; label: string; status: 'pending' | 'active' | 'done' | 'failed' }[] }): KitNode;
  StaleMark(id: string, props: { reason: string; updated?: string }): KitNode;
  StateView(id: string, props: { state: { kind: 'idle' | 'queued' | 'blocked' | 'loading' | 'refreshing' | 'ready' | 'empty' | 'unavailable' | 'error' | 'cancelled'; reason?: string; stale?: boolean }; elapsedMs?: number; fromAsync?: boolean }, events?: Record<string, never>, slots?: DisplaySlots & { content?: SlotNode[] }): KitNode;
  StatusDot(id: string, props?: { tone?: DisplayTone; tint?: ChartTint; busy?: boolean; activity?: Activity }): KitNode;
  StatusLine(id: string, props: { label: string; tone?: DisplayTone; tint?: ChartTint; busy?: boolean; activity?: Activity }): KitNode;
  Tag(id: string, props: { label: string; tone?: DisplayTone; tint?: ChartTint; variant?: DisplayVariant; color?: DisplayColor; disabled?: boolean }, events?: { remove?(): void }): KitNode;
  Timeline(id: string, props?: { entries?: TimelineEntry[]; groups?: { id: string; label: string; entries: TimelineEntry[] }[] }, events?: Record<string, never>, slots?: Record<string, SlotNode[]>): KitNode;
  TraceView(id: string, props: TraceProps, events?: { select?(id: string): void }, slots?: { empty?: SlotNode[] }): KitNode;
}
export interface FamilyMethodContracts {
  HighlightedText: { invoke: {}; query: { published_hits: { args: Record<string, never>; result: number } } };
  Icon: { invoke: {}; query: { resolved_size: { args: Record<string, never>; result: number }; resolved_color: { args: Record<string, never>; result: ChartTint }; flips_in: { args: { direction: 'ltr' | 'rtl' }; result: boolean } } };
}
