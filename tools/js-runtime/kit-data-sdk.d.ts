import type { KitNode, KitSlots } from './kit-sdk.js';
import type {BuiltinIconDescriptor} from './kit-icon-sdk.js';
export interface DataEmpty {title:string;detail?:string;kind?:'empty'|'unstarted'|'queued'|'blocked'|'cancelled'|'unavailable'|'failed'|'unauthorized';icon?:BuiltinIconDescriptor}
export interface DataSized {size?:'xs'|'sm'|'md'|'lg'}
export interface DataColumn { id:string; header:string; fixed?:number; flex?:number; align?:'start'|'center'|'end'; sortable?:boolean }
export interface NativeGridColumn extends DataColumn { minWidth?:number; resizable?:boolean; reorderable?:boolean; pinned?:boolean; editable?:boolean }
export interface DataCell { id:string; text:string; slot?:string; published?:boolean }
export interface DataRow { id:string; label?:string; disabled?:boolean; cells:DataCell[] }
export interface NativeTreeGridRow extends DataRow { level:number; hasChildren?:boolean; expanded?:boolean; parent?:string }
export interface DataSurface { disabled?:boolean; loading?:boolean; failure?:string; visibleRows?:number; rowHeight?:number; lines?:'none'|'rows';empty?:DataEmpty }
export interface DataSort { column:string; direction:'ascending'|'descending' }
export interface DataRange { startRow:string; startColumn:string; endRow:string; endColumn:string }
export interface DataAddress { row:string; column:string }
export interface DataDrop { id:string; source:string; label:string;kind:string;icon:BuiltinIconDescriptor|null;anchor:string; position:'before'|'after'|'into';velocity:{x:number;y:number} }
export type DataSelection = {kind:'replace'|'toggle';id:string}|{kind:'range';anchor:string;to:string}|{kind:'loaded'|'everything'|'clear'};
export interface NativeDataGridProps extends DataSurface,DataSized {
  rows:DataRow[];columns:NativeGridColumn[];total?:number;groups?:{id:string;label:string;columns:string[]}[];
  footer?:{id:string;text:string}[];sort?:DataSort|null;selectionMode?:'none'|'single'|'multiple';selected?:string[];
  expanded?:{id:string;index:number}[];detailRows?:number;editing?:(DataAddress&{value:string})|null;
  range?:DataRange|null;scrollToCell?:{row:number;column:string};slotNames?:{id:string}[];
}
export interface NativeDataGridEvents {
  sort?(v:DataSort):void;select?(v:DataSelection):void;resize?(v:{column:string;width:number}):void;fit?(column:string):void;
  reorder?(v:DataDrop):void;rangeChange?(v:DataRange):void;copy?(text:string):void;expand?(v:{id:string;expanded:boolean}):void;
  editRequest?(v:DataAddress):void;edit?(v:DataAddress&{value:string;outcome:'commit'|'revert';next:DataAddress|null}):void;
}
export interface NativeTreeNode {id:string;label:string;disabled?:boolean;icon?:BuiltinIconDescriptor;branch?:'ready'|'loading'|'unavailable'|'failed';reason?:string;children?:NativeTreeNode[]}
export interface TileLayout {columns?:number;columnsAt?:{breakpoint:'sm'|'md'|'lg'|'xl';columns:number}[];gap?:'xxs'|'xs'|'sm'|'md'|'lg'|'xl'|'xxl'}
export interface NativeKanbanCard {id:string;title:string;column:string;detail?:string}
export type DiagnosticSeverity = 'error'|'warning'|'information'|'hint';
export interface NativeDiagnostic {id:string;severity:DiagnosticSeverity;location:string;message:string;disabled?:boolean;actions?:{id:string;label:string;disabled?:boolean}[]}
export interface DataFactories {
  BulkBar(id:string,props:{count:number;total?:number;noun?:string;disabled?:boolean},events?:{selectAll?():void;dismiss?():void},slots?:KitSlots):KitNode;
  DataGrid(id:string,props:NativeDataGridProps,events?:NativeDataGridEvents,slots?:KitSlots):KitNode;
  Flow(id:string,props:{rows:{id:string;label?:string;revision?:number}[];estimate?:number;visibleRows?:number;fills?:boolean;anchoredToEnd?:boolean;inset?:{top:number;bottom:number}},events?:Record<string,never>,slots?:KitSlots):KitNode;
  Table(id:string,props:DataSurface&DataSized&{rows:DataRow[];columns:DataColumn[];sort?:DataSort|null;selected?:string;slotNames?:{id:string}[]},events?:{sort?(v:DataSort):void;select?(id:string):void},slots?:KitSlots):KitNode;
  TreeGrid(id:string,props:DataSurface&{rows:NativeTreeGridRow[];columns:NativeGridColumn[];selected?:string;unavailable?:string;slotNames?:{id:string}[]},events?:{select?(id:string):void;expand?(v:{id:string;expanded:boolean}):void},slots?:KitSlots):KitNode;
  Tree(id:string,props:DataSized&{nodes:NativeTreeNode[];disabled?:boolean;loading?:boolean;failure?:string;visibleRows?:number;expanded?:string[];selected?:string;reorderable?:boolean;empty?:DataEmpty},events?:{select?(id:string):void;toggle?(v:{id:string;expanded:boolean}):void;move?(v:DataDrop):void},slots?:KitSlots):KitNode;
  ImageList(id:string,props:TileLayout&{disabled?:boolean;items:{id:string;label:string;disabled?:boolean}[];selected?:string},events?:{select?(id:string):void},slots?:KitSlots):KitNode;
  Masonry(id:string,props:TileLayout&{items:{id:string;height:number}[]},events?:Record<string,never>,slots?:KitSlots):KitNode;
  KanbanBoard(id:string,props:{disabled?:boolean;held?:string;columns:{id:string;title:string;limit?:number}[];cards:NativeKanbanCard[];state?:'loading'|'ready'|'empty'|'unavailable'|'error';reason?:string},events?:{card?(card:NativeKanbanCard):void;move?(v:{card:NativeKanbanCard;column:string}):void;add?(column:string):void},slots?:KitSlots):KitNode;
  DiagnosticsList(id:string,props:DataSized&{disabled?:boolean;state:'idle'|'loading'|'ready'|'empty'|'unavailable'|'error';reason?:string;diagnostics:NativeDiagnostic[];filter?:DiagnosticSeverity[];selected?:string;visibleRows?:number},events?:{filter?(v:DiagnosticSeverity[]):void;select?(id:string):void;action?(v:{id:string;action:string}):void;retry?():void},slots?:KitSlots):KitNode;
}
export interface DataMethodContracts {}
