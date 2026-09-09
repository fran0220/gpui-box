import type {DataFactories} from '../../../../../js-runtime/kit-data-sdk.js';
declare const kit:DataFactories;
kit.DataGrid('g',{columns:[{id:'one',header:'One'}],rows:[{id:'row',cells:[{id:'one',text:'1'}]}]},{select(v){if(v.kind==='range')v.anchor.toUpperCase();}});
// @ts-expect-error constructor data required
kit.DataGrid('g',{});
// @ts-expect-error native closure never crosses wire
kit.Flow('f',{rows:[],renderRow(){}});
// @ts-expect-error incorrect event payload
kit.Tree('tree',{nodes:[]},{toggle(v:string){}});
// @ts-expect-error dimensions are numeric
kit.Masonry('m',{items:[{id:'one',height:'large'}]});
