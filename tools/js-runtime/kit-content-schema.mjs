// Native content adapters. Display strings never authorize resource acquisition.
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
const method = result => ({ args: object({}), result });
const states = { state: choice('loading', 'empty', 'unavailable', 'error', 'ready'), reason: string };
const tone = choice('neutral', 'accent', 'success', 'warning', 'danger', 'info');
const range = object({ start: integer, end: integer }, ['start', 'end']);
const spans = array(object({ ...range.fields, role: choice('keyword', 'string', 'comment', 'number', 'inline', 'inlineWash', 'added', 'addedWash', 'removed', 'removedWash') }, ['start', 'end', 'role']));
const markdownEvent = object({ kind: choice('linkClicked', 'imageRequested', 'codeCopied', 'codeCopyRefused', 'moreRequested'), href: string, src: string, alt: string, text: string, language: { ...string, nullable: true }, reason: choice('missingOwner', 'denied'), lines: integer }, ['kind']);
const codeLine = object({ number: integer, text: string, spans, mark: choice('added', 'removed', 'changed', 'highlighted', 'error') }, ['number', 'text']);
const block = object({ id, kind: choice('text', 'markdown', 'code', 'tool-call', 'diff', 'artifact', 'schema', 'chart', 'image', 'notice', 'choice', 'custom'), text: string, revision: integer, streaming: boolean, label: string, tone }, ['id', 'kind']);
const diffLine = object({ id, kind: choice('context', 'added', 'removed', 'paired'), text: string, old: string, oldNumber: integer, newNumber: integer, spans, oldSpans: spans, newSpans: spans }, ['id', 'text']);
const diffFile = object({ id, label: string, language: string, folded: boolean, notes: array(object({ kind: choice('added', 'removed', 'binary', 'renamed', 'mode'), from: string, to: string }, ['kind'])), hunks: array(object({ id, header: string, collapsed: boolean, lines: array(diffLine) }, ['id', 'header', 'lines'])) }, ['id', 'label', 'hunks']);

export const familySchemas = Object.freeze({
  Markdown: { props: object({ source: string, streaming: boolean, maxLines: integer, selectionOrderStart: integer, codePresentation: choice('card', 'flat') }, ['source']), events: { event: markdownEvent } },
  CodeView: { props: object({ text: string, lines: array(codeLine), language: string, lineNumbers: boolean, visibleLines: integer, copyable: boolean }), events: {}, slots: ['empty'] },
  AgentDocument: { props: object({ blocks: array(block), virtualized: integer, state: choice('idle', 'loading', 'empty', 'unavailable', 'failed', 'ready'), reason: string }), events: { markdown: object({ blockId: id, event: markdownEvent }, ['blockId', 'event']) }, slots: ['empty', 'failed', 'loading'], slotIds: 'blocks' },
  DiffView: { props: object({ files: array(diffFile), cursor: object({ fileId: id, hunkId: id, lineId: id }, ['fileId']), presentation: choice('unified', 'split'), visibleRows: integer, fills: boolean, wrapping: boolean, language: string }), events: { event: object({ kind: choice('fileActivated', 'hunkActivated', 'lineActivated', 'expandHunk', 'unfoldFile'), fileId: id, hunkId: id, lineId: id }, ['kind', 'fileId']) }, slots: ['empty'] },
  LogStream: { props: object({ ...states, state: choice('loading', 'empty', 'unavailable', 'error', 'stale', 'ready'), entries: array(object({ id, message: string, timestamp: string, source: string, level: string, tone, searchHits: array(range), currentHit: integer }, ['id', 'message'])), visibleRows: integer, selected: id, ansi: boolean }), events: { select: id, copy: id }, slots: ['empty', 'failed', 'loading', 'header_extra'] },
  MessageList: { props: object({ messages: array(object({ id, text: string, markdown: boolean, author: string, time: string, streaming: boolean, delivery: choice('sending', 'sent', 'delivered', 'read', 'failed'), reason: string, attachments: array(object({ id, name: string, detail: string }, ['id', 'name'])), reactions: array(object({ id, label: string, count: integer }, ['id', 'label', 'count'])) }, ['id', 'text'])), visibleRows: integer, bodyLines: integer, growsToFit: boolean, groupConsecutive: boolean }), events: { retry: id, markdown: object({ messageId: id, event: markdownEvent }, ['messageId', 'event']) } },
  Outline: { props: object({ over: id, slots: integer, marks: array(object({ id, row: integer, title: string, detail: string }, ['id', 'row', 'title'])) }), events: { select: id } },
  BrowserPanel: { props: object({ ...states, url: string }), events: { back: choice(null), forward: choice(null), reload: choice(null) }, slots: ['viewport', 'empty', 'failed', 'loading'] },
  ImageViewer: { props: object({ frames: array(object({ id, label: string, ...states, width: integer, height: integer }, ['id', 'label'])), showing: id, fit: choice('contain', 'cover', 'actual', 'zoom'), zoom: positive, minZoom: positive, maxZoom: positive, height: positive }), events: { event: object({ kind: choice('fitChanged', 'stepped', 'imageRequested'), id, label: string, fit: choice('contain', 'cover', 'actual', 'zoom'), zoom: { ...positive, nullable: true } }, ['kind']) }, slotIds: 'frames' },
  Terminal: { props: object({ state: choice('loading', 'unavailable', 'error', 'ready'), reason: string, text: string, focused: boolean, scrollback: boolean }), events: { event: object({ kind: choice('selectionStarted', 'selectionUpdated', 'selectionCleared', 'scrolled'), hit: object({ row: integer, col: integer, side: choice('left', 'right') }, ['row', 'col', 'side']), selectionKind: choice('drag', 'word', 'line'), lines: { ...number, integer: true } }, ['kind']) }, slots: ['empty', 'failed', 'loading'] },
  TransportBar: { props: object({ label: string, state: choice('playing', 'paused', 'buffering'), position: { ...number, min: 0 }, duration: { ...positive, nullable: true }, elapsed: string, remaining: string, volume: unit, muted: boolean, stepSeconds: positive, speeds: array(positive), speed: positive, buffered: array(object({ start: { ...number, min: 0 }, end: { ...number, min: 0 } }, ['start', 'end'])), seekable: boolean, volumeControl: boolean, hasPrevious: boolean, hasNext: boolean }), events: { event: object({ kind: choice('playRequested', 'pauseRequested', 'seekPreview', 'seekRequested', 'volumeRequested', 'muteToggled', 'speedRequested', 'stepped'), value: number, step: choice('previous', 'next') }, ['kind']) } },
});

export const familyMethods = Object.freeze({
  CodeView: { invoke: {}, query: { text: method({ ...string, max: 16778239 }) } },
  AgentDocument: { invoke: { remeasure_block: { args: object({ block: id }, ['block']), result: choice(null) } }, query: { duplicate_ids: method(array(id)), work: method(object({ input_checks: integer, planned_rows: integer, parser: object({ parser_passes: integer, parsed_bytes: integer, copied_bytes: integer }, ['parser_passes', 'parsed_bytes', 'copied_bytes']) }, ['input_checks', 'planned_rows', 'parser'])) } },
});

export const unsupportedBoundaries = Object.freeze({
  nativeIO: 'No image, browser or terminal process resource handles exist. URL is address-bar text only; supplied viewport/image elements are caller UI, not native IO.',
  Markdown: ['block_renderer callback transport', 'image resource handles', 'highlight callback transport'],
  AgentDocument: ['configure_markdown callback transport'],
  Terminal: ['process/PTY creation', 'live grid callback transport; text is a pure ANSI snapshot'],
});
