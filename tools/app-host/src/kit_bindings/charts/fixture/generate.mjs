// Family-owned static contracts embedded by the native validation boundary.
import { writeFileSync } from 'node:fs';
import { familySchemas, familyMethods } from '../../../../../js-runtime/kit-charts-schema.mjs';
import { props } from './props.mjs';
writeFileSync(new URL('../schemas.json', import.meta.url), JSON.stringify(familySchemas, null, 2) + '\n');
writeFileSync(new URL('../methods.json', import.meta.url), JSON.stringify(familyMethods, null, 2) + '\n');
writeFileSync(new URL('props.json', import.meta.url), JSON.stringify(props, null, 2) + '\n');
