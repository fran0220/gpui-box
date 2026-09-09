import { iconSchema } from './kit-icon-schema.mjs';

// Native DragItem, not DropIntent or an external file capability.
const string = { type: 'string', max: 16384 };
const identity = { type: 'string', min: 1, max: 256 };
export const dragItemSchema = Object.freeze({
  type: 'object',
  fields: { id: identity, source: identity, label: string, kind: string, icon: { ...iconSchema, nullable: true } },
  required: ['id', 'source', 'label', 'kind', 'icon'],
});
