import { validateResourceRef } from './resource-schema.mjs';
import { markdownEventSchema, markdownOptionsSchema } from './kit-content-schema.mjs';
const string = { type: 'string', max: 16384 };
const id = { type: 'string', min: 1, max: 256 };
const boolean = { type: 'boolean' };
const choice = (...values) => ({ enum: values });
const object = (fields, required = []) => ({ type: 'object', fields, required });
export const imageSchema = object({ key: { type: 'string', min: 1, max: 128 } }, ['key']);
const array = items => ({ type: 'array', items, max: 1024 });
const number = { type: 'number', min: 0, max: 1e9 };
const oneOf = (...branches) => ({ oneOf: branches });
const reading = oneOf(object({ basis: choice('measured', 'estimated'), amount: number, text: string }, ['basis', 'amount', 'text']), object({ basis: choice('unavailable'), reason: string }, ['basis']));
const item = oneOf(object({ id, label: string, state: choice('ahead', 'doing', 'done') }, ['id', 'label', 'state']), object({ id, label: string, state: choice('blocked', 'dropped'), reason: string }, ['id', 'label', 'state', 'reason']));
const reasoning = oneOf(object({ kind: choice('present', 'withheld'), text: string }, ['kind', 'text']), object({ kind: choice('absent') }, ['kind']));
const phase = oneOf(object({ kind: choice('loading', 'empty', 'ready') }, ['kind']), object({ kind: choice('unavailable', 'error'), reason: string }, ['kind', 'reason']));
const outcome = oneOf(object({ outcome: choice('succeeded', 'cancelled') }, ['outcome']), object({ outcome: choice('partial', 'failed', 'refused', 'timed-out'), reason: string }, ['outcome', 'reason']));
const activity = oneOf(object({ activity: choice('idle', 'planning', 'thinking', 'speaking', 'aggregating') }, ['activity']), object({ activity: choice('using-tool', 'custom'), detail: string }, ['activity', 'detail']));
const wait = oneOf(object({ reason: choice('user-input', 'approval', 'rate-limit') }, ['reason']), object({ reason: choice('dependency', 'remote-agent', 'custom'), detail: id }, ['reason', 'detail']));
export const executionSchema = oneOf(
  object({ state: choice('idle', 'queued', 'starting', 'cancelling') }, ['state']),
  object({ state: choice('active'), detail: activity }, ['state', 'detail']),
  object({ state: choice('waiting'), detail: wait }, ['state', 'detail']),
  object({ state: choice('completed'), detail: outcome }, ['state', 'detail']),
  object({ state: choice('blocked', 'unavailable'), detail: string }, ['state', 'detail']),
);
export const agentSnapshotSchema = object({
  descriptor: object({ id, name: string, role: string, capabilities: array(string) }, ['id', 'name']),
  presence: oneOf(object({ state: choice('unknown', 'online', 'away', 'offline') }, ['state']), object({ state: choice('unavailable'), reason: string }, ['state', 'reason'])),
  execution: executionSchema, current_task: id, progress: { type: 'number', min: 0, max: 1 },
}, ['descriptor', 'presence', 'execution']);
const subject = object({ kind: choice('agent', 'task', 'invocation'), id }, ['kind', 'id']);
const task = object({ id, label: string, owner: id, execution: executionSchema }, ['id', 'label', 'execution']);
const count = { ...number, integer: true };
const run = object({ id, root: id, execution: executionSchema, agents: array(agentSnapshotSchema), tasks: array(task), links: array(object({ id, from: subject, to: subject, kind: choice('spawn', 'delegation', 'dependency', 'handoff', 'report', 'aggregation', 'retry'), label: string }, ['id', 'from', 'to', 'kind'])), aggregation: object({ expected: count, received: count, conflicts: count, outcome }, ['expected', 'received', 'conflicts']) }, ['id', 'root', 'execution', 'agents', 'tasks', 'links']);
export const tintSchema = object({ h: { type: 'number', min: 0, max: 1 }, s: { type: 'number', min: 0, max: 1 }, l: { type: 'number', min: 0, max: 1 }, a: { type: 'number', min: 0, max: 1 } }, ['h', 's', 'l', 'a']);
const appearance = object({ id, tint: tintSchema, image: imageSchema }, ['id']);
const action = oneOf(object({ kind: choice('select-agent', 'open-task'), id }, ['kind', 'id']), object({ kind: choice('request-cancel', 'request-retry', 'focus-result'), subject }, ['kind', 'subject']));
const scope = oneOf(object({ kind: choice('session') }, ['kind']), object({ kind: choice('tool', 'path', 'host'), subject: string }, ['kind', 'subject']));
const decision = oneOf(object({ kind: choice('once') }, ['kind']), object({ kind: choice('always'), scope }, ['kind', 'scope']));
const approval = oneOf(object({ kind: choice('pending', 'declined', 'expired') }, ['kind']), object({ kind: choice('approved'), decision }, ['kind', 'decision']), object({ kind: choice('superseded'), by: string }, ['kind', 'by']));
const clarification = oneOf(object({ kind: choice('pending', 'skipped') }, ['kind']), object({ kind: choice('answered'), ids: array(id) }, ['kind', 'ids']), object({ kind: choice('withdrawn'), reason: string }, ['kind', 'reason']), object({ kind: choice('superseded'), by: string }, ['kind', 'by']));
const candidate = object({ id, label: string, detail: string, unavailable: string }, ['id', 'label']);
const method = (fields = {}, result = choice(null)) => ({ args: object(fields, Object.keys(fields)), result });
export const expressionSchema = oneOf(object({ kind: choice('neutral', 'warm', 'focused', 'concerned', 'celebrating') }, ['kind']), object({ kind: choice('custom'), name: string }, ['kind', 'name']));
export const effectPlanSchema = object({ id, surface: id, target: id, origin: id,
  cue: choice('arrival', 'delegation', 'handoff', 'aggregation', 'success', 'reward', 'attention', 'refusal', 'failure'),
  recipe: choice('arrival-halo', 'delegation-trace', 'handoff-trace', 'aggregation-pulse', 'success-burst', 'reward-celebration', 'attention-pulse', 'refusal-mark', 'failure-pulse'),
  presentation: oneOf(object({ kind: choice('animated') }, ['kind']), object({ kind: choice('suppressed'), reason: choice('replay') }, ['kind', 'reason']), object({ kind: choice('static'), reason: choice('quality', 'budget', 'reduced-motion') }, ['kind', 'reason'])),
  seed: { type: 'number', min: 0, max: Number.MAX_SAFE_INTEGER, integer: true },
}, ['id', 'surface', 'target', 'cue', 'recipe', 'presentation', 'seed']);
const body = object({ text: string, maxLines: { ...count, min: 1, nullable: true } }, ['text']);
const toolState = oneOf(object({ kind: choice('pending-approval', 'running') }, ['kind']), object({ kind: choice('succeeded'), output: oneOf(object({ kind: choice('silent') }, ['kind']), object({ kind: choice('body'), body }, ['kind', 'body'])) }, ['kind', 'output']), object({ kind: choice('failed'), error: string }, ['kind', 'error']), object({ kind: choice('refused'), reason: string }, ['kind', 'reason']));
const offering = object({ id, name: string, kind: choice('tool', 'skill', 'resource'), summary: string, qualifier: string }, ['id', 'name', 'kind']);
const catalog = oneOf(object({ kind: choice('unasked', 'asking') }, ['kind']), object({ kind: choice('offers'), offerings: array(offering) }, ['kind', 'offerings']), object({ kind: choice('unavailable'), reason: string }, ['kind', 'reason']));
const serverState = oneOf(object({ kind: choice('connected', 'connecting', 'disconnected') }, ['kind']), object({ kind: choice('failed'), reason: string }, ['kind', 'reason']), object({ kind: choice('disabled'), reason: string }, ['kind']));
const searchable = array(object({ offering, searchableText: string }, ['offering', 'searchableText']));
const sourceState = oneOf(object({ kind: choice('loading', 'empty') }, ['kind']), object({ kind: choice('unavailable', 'error'), reason: string }, ['kind', 'reason']), object({ kind: choice('ready'), offerings: searchable }, ['kind', 'offerings']), object({ kind: choice('stale'), offerings: searchable, reason: string }, ['kind', 'offerings', 'reason']));
const offeringIdentity = object({ serverId: id, offeringId: id }, ['serverId', 'offeringId']);
const permission = choice('allowed', 'denied', 'ask', 'not-applicable');
const voice = object({ state: oneOf(object({ kind: choice('silent', 'listening', 'speaking') }, ['kind']), object({ kind: choice('unavailable'), reason: string }, ['kind', 'reason'])), level: { type: 'number', min: 0, max: 1 }, envelope: { type: 'number', min: 0, max: 1 } }, ['state', 'level', 'envelope']);
const elapsed = { ...count, max: 86400000 };
const coordinate = { type: 'number', min: -1e7, max: 1e7 };
const viewport = object({ x: coordinate, y: coordinate, zoom: { type: 'number', min: 0.01, max: 100 } }, ['x', 'y', 'zoom']);
const position = object({ subject, x: coordinate, y: coordinate }, ['subject', 'x', 'y']);
const issue = oneOf(object({ kind: choice('missing-root', 'duplicate-agent', 'duplicate-task', 'duplicate-link', 'self-link'), id }, ['kind', 'id']), object({ kind: choice('missing-task-owner'), task: id, owner: id }, ['kind', 'task', 'owner']), object({ kind: choice('missing-link-endpoint'), link: id, endpoint: subject }, ['kind', 'link', 'endpoint']));
const rosterOptions = { appearances: array(appearance), selected: id, visibleRows: { type: 'number', min: 1, max: 1024, integer: true } };
export const familySchemas = Object.freeze({
  AgentPlan: { props: object({ items: array(item) }), events: { pick: id } },
  ThinkingBlock: { props: object({ reasoning, expanded: boolean, thinking: boolean, elapsed: string, presentation: choice('inset', 'flow') }, ['reasoning']), events: { toggle: boolean } },
  FeedbackRating: { props: object({ vote: choice('up', 'down', null), tags: array(object({ id, label: string }, ['id', 'label'])), currentTag: id, disabled: boolean }), events: { vote: choice('up', 'down'), tag: id } },
  PromptBuilder: { props: object({ label: string, body: string, slots: array(object({ id, name: string, value: string }, ['id', 'name', 'value'])), state: phase, disabled: boolean }, ['label', 'state']), events: { slot: object({ id, name: string, value: string }, ['id', 'name', 'value']) }, slots: ['empty'] },
  CostMeter: { props: object({ label: string, lines: array(object({ id, label: string, reading, stale: string }, ['id', 'label', 'reading'])) }), events: {} },
  ContextGauge: { props: object({ used: reading, limit: reading, label: string, stale: string }, ['used']), events: {} },
  AgentAvatar: { props: object({ agent: agentSnapshotSchema, image: imageSchema, tint: tintSchema, size: { type: 'number', min: 16, max: 1024 }, parent: id }, ['agent']), events: {} },
  AgentActivityLine: { props: object({ execution: executionSchema, tint: tintSchema }, ['execution']), events: {} },
  AgentCard: { props: object({ agent: agentSnapshotSchema, image: imageSchema, taskLabel: string, tint: tintSchema, selected: boolean }, ['agent']), events: { action } },
  AgentGroup: { props: object({ agents: array(agentSnapshotSchema), appearances: array(appearance), maxVisible: { type: 'number', min: 1, max: 1024, integer: true }, size: { type: 'number', min: 16, max: 1024 } }, ['agents']), events: {} },
  AgentRoster: { props: oneOf(object({ agents: array(agentSnapshotSchema), tasks: array(task), ...rosterOptions }, ['agents']), object({ run, ...rosterOptions }, ['run'])), events: { action } },
  SubagentTree: { props: object({ run, expanded: array(id), selected: id, visibleRows: { type: 'number', min: 1, max: 1024, integer: true } }, ['run']), events: { action, toggle: object({ id, expanded: boolean }, ['id', 'expanded']) } },
  AgentRunIssues: { props: oneOf(object({ run }, ['run']), object({ issues: array(issue) }, ['issues'])), events: {} },
  ApprovalPrompt: { props: object({ action: string, details: array(object({ id, term: string, value: string }, ['id', 'term', 'value'])), always: array(scope), status: approval }, ['action', 'status']), events: { approve: decision, decline: choice(null) } },
  ClarificationPanel: { props: object({ question: string, options: array(candidate), multiple: boolean, skippable: boolean, status: clarification }, ['question', 'options', 'status']), events: { answer: array(id), skip: choice(null) } },
  ArtifactPreview: { props: object({ title: string, body: string, kind: choice('code', 'document', 'markup'), language: choice('rust', 'typescript', 'python', 'go', 'json', 'shell', 'toml', 'yaml', 'markdown'), state: phase }, ['title', 'state']), events: {}, slots: ['empty', 'failed', 'loading'] },
  ToolCall: { props: object({ family: choice('read', 'network', 'shell', 'edit', 'external'), tool: string, summary: string, arguments: body, state: toolState, elapsed: { ...string, nullable: true }, expanded: boolean, presentation: choice('inset', 'flow') }, ['family', 'tool', 'state']), events: { toggle: boolean, retry: choice(null) }, slots: ['diff'] },
  ServerList: { props: object({ servers: array(object({ id, name: string, detail: string, state: serverState, catalog }, ['id', 'name', 'state', 'catalog'])), expanded: array(id), selected: id, size: choice('xs', 'sm', 'md', 'lg'), disabled: boolean }), events: { select: id, retry: id, toggle: object({ id, expanded: boolean }, ['id', 'expanded']) }, slots: ['empty', 'loading'] },
  OfferingCatalog: { props: object({ sources: array(object({ id, name: string, state: sourceState }, ['id', 'name', 'state'])), query: string, kinds: array(choice('tool', 'skill', 'resource')), selected: offeringIdentity, size: choice('xs', 'sm', 'md', 'lg'), disabled: boolean }), events: { activate: offeringIdentity }, slots: ['empty', 'failed', 'loading'] },
  PermissionMatrix: { props: object({ actions: array(object({ key: id, label: string }, ['key', 'label'])), subjects: array(object({ id, label: string, cells: array(object({ action: id, state: permission, inherited: string }, ['action', 'state'])) }, ['id', 'label', 'cells'])) }), events: { change: object({ subject: id, action: id, next: permission }, ['subject', 'action', 'next']) } },
  VoiceReactive: { props: object({ sample: voice, sampleAt: elapsed }, ['sample']), events: {} },
  PersonaPortrait: { props: object({ agent: agentSnapshotSchema, image: imageSchema, expression: expressionSchema, voice, tint: tintSchema, effect: effectPlanSchema, sampleAt: elapsed, size: { type: 'number', min: 16, max: 1024 } }, ['agent']), events: {} },
  PersonaDialogue: { props: object({ turn: object({ id, agent: agentSnapshotSchema, body: object({ text: string, markdown: boolean }, ['text']), streaming: boolean, choices: array(object({ id, label: string, detail: string, unavailable: string, selected: boolean }, ['id', 'label'])) }, ['id', 'agent', 'body', 'choices']), image: imageSchema, expression: expressionSchema, voice, tint: tintSchema, highlights: markdownOptionsSchema.highlights, images: markdownOptionsSchema.images }, ['turn']), events: { choiceRequested: object({ turnId: id, choiceId: id }, ['turnId', 'choiceId']), markdown: object({ turnId: id, event: markdownEventSchema }, ['turnId', 'event']) } },
  AgentRunCanvas: { props: object({ run, layout: choice('horizontal', 'vertical'), selected: array(subject), positions: array(position), viewport, arrangeable: boolean }, ['run']), events: { selectionChanged: array(subject), positionChanged: position, viewportChanged: viewport } },
});
export const familyMethods = Object.freeze({
  AgentPlan: { invoke: {}, query: { done: { args: object({}), result: { type: 'number', min: 0, max: 1024, integer: true } } } },
  ContextGauge: { invoke: {}, query: { fraction: { args: object({}), result: { type: 'number', min: 0, max: 1, nullable: true } } } },
  ApprovalPrompt: { invoke: { set_status: method({ status: approval }), approve: method({ decision }), decline: method() }, query: { current_status: method({}, approval) } },
  ClarificationPanel: { invoke: { set_status: method({ status: clarification }), choose: method({ id }), answer: method(), skip: method() }, query: { current_status: method({}, clarification), candidates: method({}, array(candidate)), chosen: method({}, array(id)) } },
});

// Structural validation has already ruled out accessors and unexpected fields.
export function validateProps(_component, props) {
  const visit = value => {
    if (Array.isArray(value)) { for (const item of value) visit(item); }
    else if (value && typeof value === 'object') for (const [key, item] of Object.entries(value)) {
      if (key === 'image' || key === 'resource') validateResourceRef(item); else visit(item);
    }
  };
  visit(props);
}
