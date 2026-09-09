export const MAX_MESSAGE = 256 * 1024;

/** Data-only event payloads never carry native handles, functions or prototypes. */
export function validatePayload(value) {
  let count = 0;
  function visit(item, depth) {
    if (depth > 8 || ++count > 1024) throw new Error('Event payload limit exceeded');
    if (item === null || typeof item === 'string' || typeof item === 'boolean') return;
    if (typeof item === 'number' && Number.isFinite(item)) return;
    if (Array.isArray(item)) { for (const child of item) visit(child, depth + 1); return; }
    if (!item || typeof item !== 'object' || Object.getPrototypeOf(item) !== Object.prototype) throw new Error('Event payload must be JSON data');
    for (const [key, child] of Object.entries(item)) {
      if (['__proto__', 'prototype', 'constructor'].includes(key)) throw new Error('Invalid event payload key');
      visit(child, depth + 1);
    }
  }
  visit(value, 0);
  if (Buffer.byteLength(JSON.stringify(value)) > 16384) throw new Error('Event payload limit exceeded');
  return value;
}

/** Bound bytes before parsing JSON. Never use Node's unbounded IPC deserializer. */
export function readFrames(stream, receive, fail) {
  let pending = Buffer.alloc(0);
  let failed = false;
  stream.on('data', chunk => {
    if (failed) return;
    try {
      pending = Buffer.concat([pending, chunk]);
      let end;
      while ((end = pending.indexOf(10)) !== -1) {
        if (end > MAX_MESSAGE) throw new Error('Protocol frame exceeds limit');
        const frame = pending.subarray(0, end);
        pending = pending.subarray(end + 1);
        receive(JSON.parse(frame.toString('utf8')));
      }
      if (pending.length > MAX_MESSAGE) throw new Error('Protocol frame exceeds limit');
    } catch (error) { failed = true; fail(error); }
  });
  stream.on('end', () => { if (!failed && pending.length) fail(new Error('Truncated protocol frame')); });
}
export function encodeFrame(message) {
  const frame = JSON.stringify(message) + '\n';
  if (Buffer.byteLength(frame) > MAX_MESSAGE) throw new Error('Protocol frame exceeds limit');
  return frame;
}
