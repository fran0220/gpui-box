import type { KitNode, SlotNode } from './kit-sdk.js';
export interface ChartTint { h: number; s: number; l: number; a: number }
export interface ChartXY { x: number; y: number }
export interface ChartBounds extends ChartXY { width: number; height: number }
export interface ChartPoint extends ChartXY { id: string; label: string; value: string; weight?: number }
export interface ChartSeries { id: string; label: string; points: ChartPoint[]; tint?: ChartTint }
export type ChartState<T> = { kind: 'loading' | 'empty' } | { kind: 'unavailable' | 'error'; reason: string } | { kind: 'ready'; data: T } | { kind: 'stale'; data: T; reason: string };
export interface ChartAxes { xLabel?: string; yLabel?: string; xStart?: string; xEnd?: string; yStart?: string; yEnd?: string }
export interface ChartSelection { seriesId: string; pointId: string }
export interface ChartProps<T = ChartSeries[]> { label: string; state: ChartState<T> }
export interface InteractiveChartProps extends ChartProps { axes?: ChartAxes; current?: ChartSelection; crosshair?: boolean; disabled?: boolean }
export interface ChartEvents { current?(selection: ChartSelection): void }
export interface MarkEvents { current?(id: string): void }
export interface ChartSlots { empty?: SlotNode[]; failed?: SlotNode[]; loading?: SlotNode[] }
export interface Candlestick { id: string; x: number; open: number; high: number; low: number; close: number; label: string; value: string }
export interface PlotMark { id: string; label: string; value: string; bounds: ChartBounds }
export type PlotPaint = { kind: 'rect'; bounds: ChartBounds; tint: ChartTint } | { kind: 'line'; points: ChartXY[]; width: number; tint: ChartTint } | { kind: 'polygon'; points: ChartXY[]; tint: ChartTint };
export interface SankeyNode extends PlotMark { tint?: ChartTint }
export interface SankeyLink { id: string; source: string; target: string; label: string; value: string; start: ChartXY; end: ChartXY; startWidth: number; endWidth: number; tint?: ChartTint }
export interface SankeyData { nodes: SankeyNode[]; links: SankeyLink[] }
export interface SparklineReading { points: ChartXY[]; current: string; minimum: string; maximum: string }
export interface FamilyFactories {
  AreaChart(id: string, props: InteractiveChartProps & { polyline?: boolean }, events?: ChartEvents, slots?: ChartSlots): KitNode;
  BarChart(id: string, props: ChartProps & { axes?: ChartAxes }, events?: Record<string, never>, slots?: ChartSlots): KitNode;
  CandlestickChart(id: string, props: ChartProps<Candlestick[]> & { bodyWidth?: number; risingTint?: ChartTint; fallingTint?: ChartTint; current?: string; disabled?: boolean }, events?: MarkEvents): KitNode;
  ChartLegend(id: string, props: { series: ChartSeries[]; hidden?: string[]; disabled?: boolean }, events?: { toggle?(value: { id: string; hidden: boolean }): void }): KitNode;
  GaugeChart(id: string, props: ChartProps, events?: Record<string, never>, slots?: ChartSlots): KitNode;
  LineChart(id: string, props: InteractiveChartProps & { area?: boolean; smooth?: boolean }, events?: ChartEvents, slots?: ChartSlots): KitNode;
  PieChart(id: string, props: ChartProps & { donut?: boolean }, events?: Record<string, never>, slots?: ChartSlots): KitNode;
  Plot(id: string, props: ChartProps<PlotMark[]> & { current?: string; disabled?: boolean; paint?: PlotPaint[] }, events?: MarkEvents): KitNode;
  RadarChart(id: string, props: ChartProps, events?: Record<string, never>, slots?: ChartSlots): KitNode;
  SankeyChart(id: string, props: ChartProps<SankeyData> & { current?: string; disabled?: boolean }, events?: MarkEvents): KitNode;
  ScatterChart(id: string, props: InteractiveChartProps, events?: ChartEvents, slots?: ChartSlots): KitNode;
  Sparkline(id: string, props: ChartProps<SparklineReading> & { tint?: ChartTint; stale?: boolean; embedded?: boolean }, events?: Record<string, never>, slots?: ChartSlots): KitNode;
  StackedBarChart(id: string, props: ChartProps & { axes?: ChartAxes }, events?: Record<string, never>, slots?: ChartSlots): KitNode;
}
export interface FamilyMethodContracts {
  Sparkline: { invoke: {}; query: { published_points: { args: Record<string, never>; result: number } } };
  SankeyChart: { invoke: {}; query: { layout: { args: { data: SankeyData; weights: number[]; nodeWidth: number; gap: number; alignment: 'left' | 'right' | 'justify' }; result: { scale: number; nodes: { id: string; bounds: ChartBounds }[]; links: { id: string; start: ChartXY; end: ChartXY; startWidth: number; endWidth: number }[] } } } };
}
