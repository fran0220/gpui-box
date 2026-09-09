// The wire format is deliberately data-only. Native code owns all GPUI objects.
import { validateKitDescriptor } from './kit-schema.mjs';

export function validateTree(tree) {
  const ids = new Set();
  let count = 0;
  function visit(node, depth) {
    if (!node || typeof node !== 'object' || Array.isArray(node) || depth > 32 || ++count > 1000)
      throw new Error('Invalid tree or tree limit exceeded');
    const fields = node.kind === 'kit' ? ['kind', 'component', 'id', 'props', 'slots', 'events', 'predicates'] : ['kind', 'id', 'text', 'disabled', 'action', 'children'];
    if (Object.keys(node).some(key => !fields.includes(key)))
      throw new Error('Unsupported node field');
    if (!['column', 'row', 'text', 'button', 'kit'].includes(node.kind)) throw new Error('Unsupported component');
    if (typeof node.id !== 'string' || !/^[\w.-]{1,120}$/.test(node.id) || ids.has(node.id))
      throw new Error('Every node needs a unique stable semantic id');
    ids.add(node.id);
    if (node.kind === 'kit') {
      validateKitDescriptor(node);
      for (const children of Object.values(node.slots)) for (const child of children) visit(child, depth + 1);
      return;
    }
    if (node.text !== undefined && (typeof node.text !== 'string' || node.text.length > 16384))
      throw new Error('Invalid text');
    if (node.disabled !== undefined && typeof node.disabled !== 'boolean') throw new Error('Invalid disabled');
    if (node.action !== undefined && (node.kind !== 'button' || typeof node.action !== 'string' || node.action.length > 120))
      throw new Error('Invalid action');
    if (node.children !== undefined && (!['column', 'row'].includes(node.kind) || !Array.isArray(node.children)))
      throw new Error('Only containers accept children');
    for (const child of node.children ?? []) visit(child, depth + 1);
  }
  visit(tree, 0);
  return tree;
}
