// Explicit caller-owned fixtures, never product/service results.
const agent = { descriptor: { id: 'fixture-agent', name: 'Fixture agent', role: 'Reviewer' }, presence: { state: 'unknown' }, execution: { state: 'waiting', detail: { reason: 'approval' } } };
const run = { id: 'fixture-run', root: 'fixture-agent', execution: { state: 'queued' }, agents: [agent], tasks: [], links: [] };
export const cases = [
  { component: 'AgentPlan', id: 'plan', props: { items: [{ id: 'inspect', label: 'Inspect fixture', state: 'done' }, { id: 'publish', label: 'Publish fixture', state: 'blocked', reason: 'Requires caller approval' }] }, events: { pick: 'pick' } },
  { component: 'ThinkingBlock', id: 'thinking', props: { reasoning: { kind: 'present', text: 'Caller-provided fixture evidence' }, expanded: true, presentation: 'flow', elapsed: '2 s' }, events: { toggle: 'toggle' } },
  { component: 'FeedbackRating', id: 'feedback', props: { vote: 'down', tags: [{ id: 'accuracy', label: 'Accuracy' }], currentTag: 'accuracy' }, events: { vote: 'vote', tag: 'tag' } },
  { component: 'PromptBuilder', id: 'prompt', props: { label: 'Fixture template', body: 'Inspect {{language}}', state: { kind: 'ready' }, slots: [{ id: 'language', name: 'Language', value: 'Rust' }] }, events: { slot: 'slot' } },
  { component: 'CostMeter', id: 'cost', props: { lines: [{ id: 'tokens', label: 'Fixture tokens', reading: { basis: 'estimated', amount: 23, text: '23' }, stale: '09:13' }] } },
  { component: 'ContextGauge', id: 'context', props: { used: { basis: 'measured', amount: 23, text: '23 tokens' }, limit: { basis: 'estimated', amount: 80, text: '80 tokens' }, stale: '09:13' } },
  { component: 'AgentAvatar', id: 'avatar', props: { agent, size: 44 } },
  { component: 'AgentActivityLine', id: 'activity', props: { execution: { state: 'completed', detail: { outcome: 'refused', reason: 'Caller denied execution' } } } },
  { component: 'AgentCard', id: 'card', props: { agent, taskLabel: 'Fixture task', selected: true }, events: { action: 'action' } },
  { component: 'AgentGroup', id: 'group', props: { agents: [agent], maxVisible: 2 } },
  { component: 'AgentRoster', id: 'roster', props: { agents: [agent], selected: 'fixture-agent', visibleRows: 2 }, events: { action: 'action' } },
  { component: 'SubagentTree', id: 'tree', props: { run, expanded: ['fixture-agent'], visibleRows: 2 }, events: { action: 'action', toggle: 'toggle' } },
  { component: 'AgentRunIssues', id: 'issues', props: { run: { ...run, root: 'missing' } } },
  { component: 'ApprovalPrompt', id: 'approval', props: { action: 'Inspect fixture only', status: { kind: 'pending' }, details: [{ id: 'scope', term: 'Scope', value: 'Fixture data' }], always: [{ kind: 'tool', subject: 'inspect' }] }, events: { approve: 'approve', decline: 'decline' } },
  { component: 'ClarificationPanel', id: 'clarification', props: { question: 'Which fixture?', status: { kind: 'pending' }, multiple: true, skippable: true, options: [{ id: 'west', label: 'West' }, { id: 'east', label: 'East', unavailable: 'Caller refused' }] }, events: { answer: 'answer', skip: 'skip' } },
  { component: 'ArtifactPreview', id: 'artifact', props: { title: 'Fixture code', kind: 'code', language: 'rust', body: 'let answer = 23;', state: { kind: 'ready' } } },
  { component: 'ToolCall', id: 'tool', props: { family: 'shell', tool: 'inspect', state: { kind: 'refused', reason: 'Caller denied execution' }, arguments: { text: 'fixture-only', maxLines: 3 }, expanded: true, presentation: 'flow' }, events: { toggle: 'toggle', retry: 'retry' } },
  { component: 'ServerList', id: 'servers', props: { servers: [{ id: 'fixture-server', name: 'Fixture service', state: { kind: 'failed', reason: 'Explicit fixture failure' }, catalog: { kind: 'unavailable', reason: 'No verified offerings' } }], expanded: ['fixture-server'] }, events: { select: 'select', retry: 'retry', toggle: 'toggle' } },
  { component: 'OfferingCatalog', id: 'offerings', props: { sources: [{ id: 'fixture-source', name: 'Fixture source', state: { kind: 'stale', reason: 'Refresh refused', offerings: [{ offering: { id: 'inspect', name: 'Inspect fixture', kind: 'tool' }, searchableText: 'fixture inspect' }] } }] }, events: { activate: 'activate' } },
  { component: 'PermissionMatrix', id: 'permissions', props: { actions: [{ key: 'read', label: 'Read' }], subjects: [{ id: 'fixture-subject', label: 'Fixture subject', cells: [{ action: 'read', state: 'denied' }] }] }, events: { change: 'change' } },
  { component: 'VoiceReactive', id: 'voice', props: { sample: { state: { kind: 'speaking' }, level: 0.23, envelope: 0.81 }, sampleAt: 173 } },
  { component: 'PersonaPortrait', id: 'portrait', props: { agent, expression: { kind: 'concerned' }, voice: { state: { kind: 'unavailable', reason: 'Caller supplied no audio' }, level: 0, envelope: 0 }, size: 64 } },
  { component: 'PersonaDialogue', id: 'dialogue', props: { turn: { id: 'fixture-turn', agent, body: { text: 'Caller-provided **fixture** dialogue', markdown: true }, choices: [{ id: 'inspect', label: 'Inspect' }, { id: 'execute', label: 'Execute', unavailable: 'Caller refused' }] }, expression: { kind: 'warm' } }, events: { choiceRequested: 'choice' } },
  { component: 'AgentRunCanvas', id: 'canvas', props: { run, layout: 'vertical', viewport: { x: 23, y: 7, zoom: 0.8 }, selected: [{ kind: 'agent', id: 'fixture-agent' }] }, events: { selectionChanged: 'selection', positionChanged: 'position', viewportChanged: 'viewport' } },
];
