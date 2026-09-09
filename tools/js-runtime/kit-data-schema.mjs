// Data-family wire contracts. No executable builders cross the boundary.
import {iconSchema} from './kit-icon-schema.mjs';
const s = {type:'string',max:16384};
const id = {type:'string',min:1,max:256};
const b = {type:'boolean'};
const n = {type:'number',min:0,max:1000000};
const positive = {...n,min:0.01};
const int = {...n,integer:true};
const count = {...int,min:1};
const e = (...values) => ({enum:values});
const a = items => ({type:'array',items,max:1024});
const o = (fields,required=[]) => ({type:'object',fields,required});
const nil=e(null);
const empty=o({title:s,detail:s,kind:e('empty','unstarted','queued','blocked','cancelled','unavailable','failed','unauthorized'),icon:iconSchema},['title']);
const common={disabled:b,loading:b,failure:s,visibleRows:count,rowHeight:positive,lines:e('none','rows'),empty};
const sized={size:e('xs','sm','md','lg')};
const column=o({id,header:s,fixed:positive,flex:positive,align:e('start','center','end'),sortable:b},['id','header']);
const gridColumn=o({...column.fields,minWidth:positive,resizable:b,reorderable:b,pinned:b,editable:b},['id','header']);
const cell=o({id,text:s,slot:id,published:b},['id','text']);
const row=o({id,label:s,disabled:b,cells:a(cell)},['id','cells']);
const sort=o({column:id,direction:e('ascending','descending')},['column','direction']);
const toggle=o({id,expanded:b},['id','expanded']);
const velocity=o({x:{type:'number',min:-1e9,max:1e9},y:{type:'number',min:-1e9,max:1e9}},['x','y']);
const drop=o({id,source:id,label:s,kind:s,icon:{...iconSchema,nullable:true},anchor:id,position:e('before','after','into'),velocity},['id','source','label','kind','icon','anchor','position','velocity']);
const range=o({startRow:id,startColumn:id,endRow:id,endColumn:id},['startRow','startColumn','endRow','endColumn']);
const address=o({row:id,column:id},['row','column']);
const surfaceSlots=['empty','failed','loading','header_extra','empty_action'];
const tile={columns:count,columnsAt:a(o({breakpoint:e('sm','md','lg','xl'),columns:count},['breakpoint','columns'])),gap:e('xxs','xs','sm','md','lg','xl','xxl')};
const severity=e('error','warning','information','hint');
const card=o({id,title:s,column:id,detail:s},['id','title','column']);
function tree() {
  return o({id,label:s,disabled:b,icon:iconSchema,branch:e('ready','loading','unavailable','failed'),reason:s,children:a({$ref:'TreeNode'})},['id','label']);
}
const schemas={
  BulkBar:{props:o({count:int,total:int,noun:s,disabled:b},['count']),events:{selectAll:nil,dismiss:nil},slots:['actions']},
  Flow:{props:o({rows:a(o({id,label:s,revision:int},['id'])),estimate:positive,visibleRows:count,fills:b,anchoredToEnd:b,inset:o({top:n,bottom:n},['top','bottom'])},['rows']),events:{},slotIds:'rows'},
  DataGrid:{props:o({...common,rows:a(row),columns:a(gridColumn),total:int,groups:a(o({id,label:s,columns:a(id)},['id','label','columns'])),footer:a(o({id,text:s},['id','text'])),sort:{...sort,nullable:true},selectionMode:e('none','single','multiple'),selected:a(id),expanded:a(o({id,index:int},['id','index'])),detailRows:count,editing:{...o({...address.fields,value:s},['row','column','value']),nullable:true},range:{...range,nullable:true},scrollToCell:o({row:int,column:id},['row','column']),slotNames:a(o({id},['id']))},['rows','columns']),events:{sort,select:o({kind:e('replace','toggle','range','loaded','everything','clear'),id,anchor:id,to:id},['kind']),resize:o({column:id,width:positive},['column','width']),fit:id,reorder:drop,rangeChange:range,copy:s,expand:toggle,editRequest:address,edit:o({...address.fields,value:s,outcome:e('commit','revert'),next:{...address,nullable:true}},['row','column','value','outcome','next'])},slots:surfaceSlots,slotIds:'slotNames'},
  Table:{props:o({...common,rows:a(row),columns:a(column),sort:{...sort,nullable:true},selected:id,slotNames:a(o({id},['id']))},['rows','columns']),events:{sort,select:id},slots:surfaceSlots,slotIds:'slotNames'},
  TreeGrid:{props:o({...common,rows:a(o({...row.fields,level:count,hasChildren:b,expanded:b,parent:id},['id','cells','level'])),columns:a(gridColumn),selected:id,unavailable:s,slotNames:a(o({id},['id']))},['rows','columns']),events:{select:id,expand:toggle},slots:['empty','failed','loading'],slotIds:'slotNames'},
  Tree:{props:{$defs:{TreeNode:tree()},...o({disabled:b,loading:b,failure:s,visibleRows:count,nodes:a({$ref:'TreeNode'}),expanded:a(id),selected:id,reorderable:b},['nodes'])},events:{select:id,toggle,move:drop},slots:surfaceSlots},
  ImageList:{props:o({...tile,items:a(o({id,label:s,disabled:b},['id','label'])),selected:id},['items']),events:{select:id},slotIds:'items'},
  Masonry:{props:o({...tile,items:a(o({id,height:positive},['id','height']))},['items']),events:{},slotIds:'items'},
  KanbanBoard:{props:o({held:id,columns:a(o({id,title:s,limit:int},['id','title'])),cards:a(card),state:e('loading','ready','empty','unavailable','error'),reason:s},['columns','cards']),events:{card,move:o({card,column:id},['card','column']),add:id},slots:['empty']},
  DiagnosticsList:{props:o({disabled:b,state:e('idle','loading','ready','empty','unavailable','error'),reason:s,diagnostics:a(o({id,severity,location:s,message:s,disabled:b,actions:a(o({id,label:s,disabled:b},['id','label']))},['id','severity','location','message'])),filter:a(severity),selected:id,visibleRows:count},['state','diagnostics']),events:{filter:a(severity),select:id,action:o({id,action:id},['id','action']),retry:nil},slots:['empty','failed','loading']},
};
for(const name of ['DataGrid','Table','Tree','DiagnosticsList']) Object.assign(schemas[name].props.fields,sized);
schemas.Tree.props.fields.empty=empty;
schemas.TreeGrid.slots.push('empty_action');
schemas.ImageList.props.fields.disabled=b;
schemas.KanbanBoard.props.fields.disabled=b;
schemas.DataGrid.events.select={oneOf:[o({kind:e('replace','toggle'),id},['kind','id']),o({kind:e('range'),anchor:id,to:id},['kind','anchor','to']),o({kind:e('loaded','everything','clear')},['kind'])]};
export const familySchemas=Object.freeze(schemas);
export const familyMethods=Object.freeze({});

export function validateFamilyProps(component,p) {
  const fail=message=>{throw new TypeError(message);};
  if(component==='Tree') {
    const ids=new Set();
    const visit=nodes=>{for(const n of nodes){if(ids.has(n.id))fail('Duplicate tree identity');ids.add(n.id);visit(n.children??[]);}};
    visit(p.nodes);
  }
  if(['DataGrid','TreeGrid','Table'].includes(component)) {
    const columns=new Set(p.columns.map(c=>c.id)), names=new Set((p.slotNames??[]).map(s=>s.id)), rows=new Set();
    for(const row of p.rows) {
      if(component==='TreeGrid'&&row.parent!==undefined&&!rows.has(row.parent))fail('TreeGrid parent must precede child');
      rows.add(row.id);
      for(const c of row.cells){if(!columns.has(c.id))fail('Unknown cell column');if(c.slot!==undefined&&!names.has(c.slot))fail('Undeclared cell slot');}
    }
    for(const v of p.expanded??[])if(p.rows[v.index]?.id!==v.id)fail('Expanded identity/index mismatch');
    for(const v of p.columns)if(v.fixed!==undefined&&v.flex!==undefined)fail('Column width is either fixed or flex');
  }
}
