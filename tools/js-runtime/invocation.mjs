import { validateInvocation } from './kit-schema.mjs';
import { validatePayload } from './wire.mjs';

export function invocationTarget(tree, target, method, args, mode) {
  if (!target || typeof target !== 'object' || Array.isArray(target) ||
      typeof target.id !== 'string' || typeof target.component !== 'string') throw new Error('Invalid native target');
  function find(node) {
    if (node?.id === target.id) return node;
    for (const child of [...(node?.children ?? []), ...Object.values(node?.slots ?? {}).flat()]) {
      const found = find(child); if (found) return found;
    }
  }
  const node = find(tree);
  if (!node || node.kind !== 'kit' || node.component !== target.component) throw new Error('Native target is not mounted with that component identity');
  if (mode === 'invoke' && node.props.disabled) throw new Error('Native command refused: target disabled');
  validatePayload(args);
  validateInvocation(node.component, method, args, mode);
  return node;
}
