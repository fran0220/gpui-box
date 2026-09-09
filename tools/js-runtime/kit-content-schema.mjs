// Native content adapters. Display strings never authorize resource acquisition.
import { validateResourceRef } from './resource-schema.mjs';
const string = { type: 'string', max: 16384 };
const id = { type: 'string', min: 1, max: 256 };
const boolean = { type: 'boolean' };
const number = { type: 'number', min: -1e9, max: 1e9 };
const integer = { type: 'number', integer: true, min: 0, max: 1000000 };
const positive = { type: 'number', min: 0.001, max: 1000000 };
const unit = { type: 'number', min: 0, max: 1 };
const choice = (...values) => ({ enum: values });
const array = items => ({ type: 'array', max: 1024, items });
const object = (fields, required = []) => ({ type: 'object', fields, required });
const resource = object({ key: { type: 'string', min: 1, max: 128 } }, ['key']);
const method = result => ({ args: object({}), result });
const states = { state: choice('loading', 'empty', 'unavailable', 'error', 'ready'), reason: string };
const tone = choice('neutral', 'accent', 'success', 'warning', 'danger', 'info');
const range = object({ start: integer, end: integer }, ['start', 'end']);
const spans = array(object({ ...range.fields, role: choice('keyword', 'string', 'comment', 'number', 'inline', 'inlineWash', 'added', 'addedWash', 'removed', 'removedWash') }, ['start', 'end', 'role']));
const markdownOptions = { maxLines: integer, selectionOrderStart: integer, codePresentation: choice('card', 'flat'), highlights: array(object({ text: string, language: string, spans }, ['text', 'spans'])), images: array(object({ src: string, resource }, ['src', 'resource'])) };
const markdownEvent = object({ kind: choice('linkClicked', 'imageRequested', 'codeCopied', 'codeCopyRefused', 'moreRequested'), href: string, src: string, alt: string, text: string, language: { ...string, nullable: true }, reason: choice('missingOwner', 'denied'), lines: integer }, ['kind']);
export { markdownEvent as markdownEventSchema, markdownOptions as markdownOptionsSchema, spans as codeSpansSchema };
const codeLine = object({ number: integer, text: string, spans, mark: choice('added', 'removed', 'changed', 'highlighted', 'error') }, ['number', 'text']);
const block = object({ id, kind: choice('text', 'markdown', 'code', 'tool-call', 'diff', 'artifact', 'schema', 'chart', 'image', 'notice', 'choice', 'custom'), text: string, revision: integer, streaming: boolean, label: string, tone }, ['id', 'kind']);
const diffLine = object({ id, kind: choice('context', 'added', 'removed', 'paired'), text: string, old: string, oldNumber: integer, newNumber: integer, spans, oldSpans: spans, newSpans: spans }, ['id', 'text']);
const diffFile = object({ id, label: string, language: string, folded: boolean, notes: array(object({ kind: choice('added', 'removed', 'binary', 'renamed', 'mode'), from: string, to: string }, ['kind'])), hunks: array(object({ id, header: string, collapsed: boolean, lines: array(diffLine) }, ['id', 'header', 'lines'])) }, ['id', 'label', 'hunks']);

export const familySchemas = Object.freeze({
  Markdown: { props: object({ source: string, streaming: boolean, ...markdownOptions }, ['source']), events: { event: markdownEvent } },
  CodeView: { props: object({ text: string, lines: array(codeLine), language: string, lineNumbers: boolean, visibleLines: integer, copyable: boolean }), events: {}, slots: ['empty'] },
  AgentDocument: { props: object({ blocks: array(block), virtualized: integer, state: choice('idle', 'loading', 'empty', 'unavailable', 'failed', 'ready'), reason: string, markdownOptions: object(markdownOptions) }), events: { markdown: object({ blockId: id, event: markdownEvent }, ['blockId', 'event']) }, slots: ['empty', 'failed', 'loading'], slotIds: 'blocks' },
  DiffView: { props: object({ files: array(diffFile), cursor: object({ fileId: id, hunkId: id, lineId: id }, ['fileId']), presentation: choice('unified', 'split'), visibleRows: integer, fills: boolean, wrapping: boolean, language: string }), events: { event: object({ kind: choice('fileActivated', 'hunkActivated', 'lineActivated', 'expandHunk', 'unfoldFile'), fileId: id, hunkId: id, lineId: id }, ['kind', 'fileId']) }, slots: ['empty'] },
  LogStream: { props: object({ ...states, state: choice('loading', 'empty', 'unavailable', 'error', 'stale', 'ready'), entries: array(object({ id, message: string, timestamp: string, source: string, level: string, tone, searchHits: array(range), currentHit: integer }, ['id', 'message'])), visibleRows: integer, selected: id, ansi: boolean }), events: { select: id, copy: id }, slots: ['empty', 'failed', 'loading', 'header_extra'] },
  MessageList: { props: object({ messages: array(object({ id, text: string, markdown: boolean, author: string, time: string, streaming: boolean, delivery: choice('sending', 'sent', 'delivered', 'read', 'failed'), reason: string, attachments: array(object({ id, name: string, detail: string }, ['id', 'name'])), reactions: array(object({ id, label: string, count: integer }, ['id', 'label', 'count'])) }, ['id', 'text'])), visibleRows: integer, bodyLines: integer, growsToFit: boolean, groupConsecutive: boolean }), events: { retry: id, markdown: object({ messageId: id, event: markdownEvent }, ['messageId', 'event']) } },
  Outline: { props: object({ over: id, slots: integer, marks: array(object({ id, row: integer, title: string, detail: string }, ['id', 'row', 'title'])) }), events: { select: id } },
  BrowserPanel: { props: object({ ...states, url: string }), events: { back: choice(null), forward: choice(null), reload: choice(null) }, slots: ['viewport', 'empty', 'failed', 'loading'] },
  ImageViewer: { props: object({ disabled: boolean, frames: array(object({ id, label: string, state: choice('loading', 'unavailable', 'error', 'ready'), reason: string, width: integer, height: integer, resource }, ['id', 'label'])), showing: id, fit: choice('contain', 'cover', 'actual', 'zoom'), zoom: positive, minZoom: positive, maxZoom: positive, height: positive }), events: { event: object({ kind: choice('fitChanged', 'stepped', 'imageRequested'), id, label: string, fit: choice('contain', 'cover', 'actual', 'zoom'), zoom: { ...positive, nullable: true } }, ['kind']) }, slotIds: 'frames' },
  Terminal: { props: object({ state: choice('loading', 'unavailable', 'error', 'ready'), reason: string, text: string, focused: boolean, scrollback: boolean }), events: { event: object({ kind: choice('selectionStarted', 'selectionUpdated', 'selectionCleared', 'scrolled'), hit: object({ row: integer, col: integer, side: choice('left', 'right') }, ['row', 'col', 'side']), selectionKind: choice('drag', 'word', 'line'), lines: { ...number, integer: true } }, ['kind']) }, slots: ['empty', 'failed', 'loading'] },
  TransportBar: { props: object({ disabled: boolean, label: string, state: choice('playing', 'paused', 'buffering'), position: { ...number, min: 0 }, duration: { ...positive, nullable: true }, elapsed: string, remaining: string, volume: unit, muted: boolean, stepSeconds: positive, speeds: array(positive), speed: positive, buffered: array(object({ start: { ...number, min: 0 }, end: { ...number, min: 0 } }, ['start', 'end'])), seekable: boolean, volumeControl: boolean, hasPrevious: boolean, hasNext: boolean }), events: { event: object({ kind: choice('playRequested', 'pauseRequested', 'seekPreview', 'seekRequested', 'volumeRequested', 'muteToggled', 'speedRequested', 'stepped'), value: number, step: choice('previous', 'next') }, ['kind']) } },
});

export const familyMethods = Object.freeze({
  CodeView: { invoke: {}, query: { text: method({ ...string, max: 16778239 }) } },
  AgentDocument: { invoke: { remeasure_block: { args: object({ block: id }, ['block']), result: choice(null) } }, query: { duplicate_ids: method(array(id)), work: method(object({ input_checks: integer, planned_rows: integer, parser: object({ parser_passes: integer, parsed_bytes: integer, copied_bytes: integer }, ['parser_passes', 'parsed_bytes', 'copied_bytes']) }, ['input_checks', 'planned_rows', 'parser'])) } },
});

export const unsupportedBoundaries = Object.freeze({
  nativeIO: 'Only approved revocable raw image resources. No browser or terminal process handles. URL is address-bar text only; supplied viewport elements are caller UI, not browser execution.',
  Markdown: ['block_renderer callback transport', 'arbitrary highlight callback transport; preclassified exact text/language spans supported'],
  AgentDocument: ['arbitrary per-block configure_markdown callback transport; declarative markdownOptions supported'],
  Terminal: ['process/PTY creation', 'live grid callback transport; text is a pure ANSI snapshot'],
});

// Called after the shared closed schema/slot validation, before registration.
export function validateDescriptor(node) {
  const p = node.props;
  for (const image of [...(p.images ?? []), ...(p.markdownOptions?.images ?? []), ...(p.frames ?? [])]) if (image.resource) validateResourceRef(image.resource);
  if (node.component === 'CodeView') {
    if (p.text !== undefined && p.lines !== undefined) throw new TypeError('CodeView: choose text or lines');
    if (new Set((p.lines ?? []).map(line => line.number)).size !== (p.lines ?? []).length) throw new TypeError('CodeView: duplicate line number');
  }
  if (node.component === 'AgentDocument') for (const block of p.blocks ?? []) {
    if (!['text', 'markdown', 'notice'].includes(block.kind) && !Object.hasOwn(node.slots, block.id)) throw new TypeError('AgentDocument: typed block needs its slot');
    if (['text', 'markdown'].includes(block.kind) && block.text === undefined) throw new TypeError('AgentDocument: text required');
    if (block.kind === 'notice' && block.text === undefined && !Object.hasOwn(node.slots, block.id)) throw new TypeError('AgentDocument: notice text or slot required');
  }
  if (node.component === 'ImageViewer' && (p.minZoom ?? 0.1) > (p.maxZoom ?? 10)) throw new TypeError('ImageViewer: invalid zoom range');
}
