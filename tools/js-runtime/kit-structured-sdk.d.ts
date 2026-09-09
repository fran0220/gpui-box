import type { KitNode, KitSize } from './kit-sdk.js';
export type NativeJsonValue={kind:'null'}|{kind:'boolean';boolean:boolean}|{kind:'number'|'string'|'redacted';text:string}|{kind:'array';items:NativeJsonValue[]}|{kind:'object';members:{key:string;value:NativeJsonValue}[]}|{kind:'identifiedObject';members:{id:string;key:string;value:NativeJsonValue}[]};
export interface SchemaChoiceData {id:string;label:string;description?:string}
export type SchemaFieldData={name:string;label?:string;description?:string;required?:boolean}&(
  {kind:'text';placeholder?:string;secret?:boolean}|{kind:'number'|'integer';min?:number;max?:number;step?:number}|
  {kind:'boolean'|'date'|'time'|'dateRange'}|{kind:'enum'|'openEnum';choices:SchemaChoiceData[]}|
  {kind:'textList'|'files';maxItems?:number}|{kind:'object';fields:SchemaFieldData[]}|
  {kind:'list';item:SchemaFieldData;maxItems?:number}|{kind:'unrenderable';reason:string}
);
export type StructuredValidation={state:'pending'|'validating'|'valid';reason?:null}|{state:'invalid';reason:string};
export type StructuredVisibility='visible'|'hiddenInclude'|'hiddenOmit';
export type StructuredFieldValue={kind:'text'|'choice';text:string}|{kind:'number'|'itemCount'|'day';number:number}|{kind:'boolean';boolean:boolean}|{kind:'list'|'files';items:string[]}|{kind:'time';hour:number;minute:number;second:number|null}|{kind:'range';start:number;end:number|null}|{kind:'absent'|'unrenderable'};
export interface StructuredFactories {
  JsonView(id:string,props:{value:NativeJsonValue;rootLabel?:string;expanded?:string[];selected?:string;visibleRows?:number;rowHeight?:number;disabled?:boolean;size?:KitSize},events?:{toggle?(v:{path:string;expanded:boolean}):void;select?(path:string):void}):KitNode;
  SchemaForm(id:string,props:{fields:SchemaFieldData[];disabled?:boolean},events?:{change?(path:string):void;submit?():void;filesRequested?(v:{path:string;label:string;max:number|null}):void}):KitNode;
}
type Method<A,R=null>={args:A;result:R};
type NoArgs=Record<string,never>;
export interface StructuredMethodContracts {
  JsonView:{invoke:Record<string,never>;query:{disclosed_paths:Method<NoArgs,string[]>}};
  SchemaForm:{invoke:{
    set_files:Method<{path:string;files:string[]},boolean>;add_list_item:Method<{path:string},boolean>;
    remove_list_item:Method<{path:string;index:number},boolean>;move_list_item:Method<{path:string;from:number;to:number},boolean>;
    set_field_validation:Method<{path:string;validation:StructuredValidation},boolean>;clear_field_validation:Method<{path:string},boolean>;
    set_field_visibility:Method<{path:string;visibility:StructuredVisibility},boolean>;set_validation:Method<{validation:StructuredValidation}>;
    clear_validation:Method<NoArgs>;set_error:Method<{path:string;message:string}>;clear_host_errors:Method<NoArgs>;validate:Method<NoArgs,boolean>;set_disabled:Method<{disabled:boolean}>;
  };query:{
    field_validation:Method<{path:string},StructuredValidation|null>;field_visibility:Method<{path:string},StructuredVisibility|null>;validation:Method<NoArgs,StructuredValidation|null>;
    values:Method<NoArgs,{path:string;value:StructuredFieldValue}[]>;submission_values:Method<NoArgs,{path:string;value:StructuredFieldValue}[]>;
    unrenderable:Method<NoArgs,{path:string;label:string;required:boolean;reason:string}[]>;has_unrenderable_required:Method<NoArgs,boolean>;
  }};
}
