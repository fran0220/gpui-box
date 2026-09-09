// Shared data contract for Select, Combobox and MultiSelect. Kept independent
// of the aggregate schemas so family imports cannot create an initialization cycle.
export const selectOptionSchema = {
  type: 'object',
  fields: {
    id: { type: 'string', min: 1, max: 256 },
    label: { type: 'string', max: 16384 },
    disabled: { type: 'boolean' },
    description: { type: 'string', max: 16384 },
    group: { type: 'string', max: 16384 },
  },
  required: ['id', 'label'],
};
