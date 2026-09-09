import type {StructuredFactories,StructuredMethodContracts,NativeJsonValue} from '../../../../../js-runtime/kit-structured-sdk.js';
declare const kit:StructuredFactories;
kit.SchemaForm('form',{fields:[{name:'rows',kind:'list',item:{name:'value',kind:'text'}}]});
const value:NativeJsonValue={kind:'identifiedObject',members:[{id:'a',key:'same',value:{kind:'number',text:'1.10'}}]};
kit.JsonView('j',{value});
// @ts-expect-error arbitrary values must be tagged
kit.JsonView('j',{value:{value:'text'}});
// @ts-expect-error list requires item schema
kit.SchemaForm('f',{fields:[{name:'list',kind:'list'}]});
// @ts-expect-error redacted data has shape text, never a hidden value
kit.JsonView('j',{value:{kind:'redacted',secret:'secret'}});
// @ts-expect-error command uses named numeric indices
const bad:StructuredMethodContracts['SchemaForm']['invoke']['move_list_item']['args']={path:'rows',from:'0',to:1};
// @ts-expect-error invalid validation requires a reason
const invalid:StructuredMethodContracts['SchemaForm']['invoke']['set_validation']['args']={validation:{state:'invalid'}};
