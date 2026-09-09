import { builtinIconKeys } from './kit-icon-catalog.mjs';

// Built-ins only. A descriptor cannot nominate a path, URL, or asset provider.
export const iconSchema = Object.freeze({
  type: 'object',
  fields: {
    key: { enum: builtinIconKeys },
    weight: { enum: ['regular', 'fill'] },
  },
  required: ['key'],
});
