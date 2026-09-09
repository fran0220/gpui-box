// Correlation for native release decisions, not a callback or component registry.
// Session owns worker RPC IDs; native owns the original typed DropRequestId.
import { PREDICATE_LIMIT, PREDICATE_TIMEOUT, predicateTarget, validatePredicatePayload } from '../js-runtime/predicates.mjs';

export class DropBridge {
  constructor(context, output) { this.context = context; this.output = output; this.pending = new Map(); this.closed = false; }
  live(call) {
    const context = this.context(), { message, session, target, workerRevision } = call;
    const matches = context.nativeTargets.get(`${message.instance}:${target.id}`);
    return context.revision === message.revision && context.sessions.includes(session) && !session.closed && session.revision === workerRevision &&
      matches?.length === 1 && matches[0].id === message.target && matches[0].component === message.component;
  }
  reconcile() {
    for (const [key, call] of this.pending) if (!this.live(call)) {
      this.pending.delete(key); call.abort.abort();
    }
  }
  cancel(message) {
    const key = `${message.instance}:${message.id}`, call = this.pending.get(key);
    if (call && call.message.revision === message.revision) { this.pending.delete(key); call.abort.abort(); }
  }
  close() { this.closed = true; for (const call of this.pending.values()) call.abort.abort(); this.pending.clear(); }
  async request(message) {
    if (this.closed) return;
    const fields = ['kind','id','instance','revision','target','component','reference','payload','deadline'];
    if (!message || Object.keys(message).length !== fields.length || fields.some(key => !Object.hasOwn(message, key)) || message.kind !== 'drop-request' ||
        ![message.id,message.instance,message.revision].every(value => Number.isSafeInteger(value) && value > 0)) return;
    const key = `${message.instance}:${message.id}`;
    if (this.pending.has(key)) return;
    const reply = accepted => this.output({kind:'drop-response',id:message.id,instance:message.instance,revision:message.revision,accepted});
    let call;
    try {
      if (this.pending.size >= PREDICATE_LIMIT || !Number.isSafeInteger(message.deadline) || message.deadline <= Date.now() || message.deadline > Date.now() + PREDICATE_TIMEOUT) throw new Error('Drop deadline/capacity refused');
      const context = this.context();
      const session = context.sessions.find(session => session.generation === message.instance && !session.closed);
      const match = [...context.nativeTargets].find(([key, values]) => key.startsWith(`${message.instance}:`) && values.length === 1 && values[0].id === message.target && values[0].component === message.component);
      if (!session || !match) throw new Error('Drop target is inactive or ambiguous');
      const target = {id:match[0].slice(String(message.instance).length + 1),component:message.component};
      call = {message,session,target,workerRevision:session.revision,abort:new AbortController()};
      if (!this.live(call)) throw new Error('Drop revision expired');
      predicateTarget(session.tree,target,'accepts',message.reference);
      validatePredicatePayload(target.component,message.payload);
      this.pending.set(key,call);
      const accepted = await session.evaluatePredicate({target,name:'accepts',reference:message.reference,payload:message.payload,deadline:message.deadline},call.abort.signal);
      if (this.pending.get(key) === call && this.live(call)) reply(accepted === true);
    } catch {
      // Exceptions, non-booleans, expired requests and unavailable workers refuse.
      if (!call?.abort.signal.aborted) reply(false);
    } finally {
      if (this.pending.get(key) === call) this.pending.delete(key);
    }
  }
}
