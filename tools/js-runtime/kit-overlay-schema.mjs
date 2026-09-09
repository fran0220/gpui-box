import { iconSchema } from './kit-icon-schema.mjs';
const string={type:'string',max:16384},id={type:'string',min:1,max:256},boolean={type:'boolean'};
const number={type:'number',min:-1e9,max:1e9},nonnegative={...number,min:0},integer={...nonnegative,integer:true,max:1000000},unit={...nonnegative,max:1};
const choice=(...values)=>({enum:values});
const object=(fields,required=[])=>({type:'object',fields,required});
const array=items=>({type:'array',items,max:1024});
const method=(fields={},result=choice(null))=>({args:object(fields,Object.keys(fields)),result});
const optional=schema=>({...schema,nullable:true});
// Marker shape only; the shared owner-scoped registry confers all authority.
const reference=kind=>object({$nativeRef:id,type:choice(kind)},['$nativeRef','type']);
const focusReference=reference('FocusHandle');
const slot=method({slot:choice(null,'content','footer','trigger','empty')});
const point=object({x:number,y:number},['x','y']);
const edge=choice('top','right','bottom','left'),hang=choice('start','end');
export const placementSchema={oneOf:[choice('above','below','center'),object({at:point},['at']),object({edge},['edge'])]};
const size=choice('xs','sm','md','lg'),surface=choice('backdrop','canvas','sunken','panel','raised','overlay'),radius=choice('small','control','card','dialog','bubble','pill');
const edges=object({top:nonnegative,right:nonnegative,bottom:nonnegative,left:nonnegative},['top','right','bottom','left']);
const variant=choice('primary','secondary','ghost','danger','link');
const menuBase={id,label:string,disabled:boolean,destructive:boolean,shortcut:string,icon:iconSchema};
function menuItem(depth){
  const branches=[object({...menuBase,kind:choice('command')},['id','label','kind']),object({...menuBase,kind:choice('check'),checked:boolean},['id','label','kind','checked']),object({id,kind:choice('separator')},['id','kind']),object({id,label:string,kind:choice('section')},['id','label','kind'])];
  if(depth)branches.push(object({...menuBase,kind:choice('submenu'),items:array(menuItem(depth-1))},['id','label','kind','items']));
  return {oneOf:branches};
}
export const menuItemsSchema=array(menuItem(8));
const menus=array(object({id,label:string,disabled:boolean,items:menuItemsSchema},['id','label','items']));
const commands=array(object({id,label:string,section:string,shortcut:string,unavailable:string},['id','label']));
const tone=choice('neutral','accent','success','warning','danger','info');
const toast=object({id,message:string,tone,detail:string,action:string,dismissable:boolean,timeout:integer,persistent:boolean},['id','message']);
const notification=object({id,message:string,tone,detail:string,at:string,read:boolean,action:string},['id','message']);
const corner=choice('top_left','top_right','bottom_left','bottom_right');
const lifecycle={open:choice(null),close:choice(null)};
export const familySchemas=Object.freeze({
  Drawer:{props:object({edge,size:nonnegative,title:string,description:string,dismissable:boolean,resizable:boolean,focus_stops:array(focusReference)}),events:{...lifecycle,dismiss:choice(null),resize_requested:nonnegative},slots:['content','footer']},
  HoverCard:{props:object({name:string,placement:placementSchema,hang,open_delay:integer,grace:integer}),events:lifecycle,slots:['content','trigger']},
  Menu:{props:object({trigger:string,trigger_icon:iconSchema,trigger_name:string,trigger_variant:variant,trigger_join:choice('alone','leading','middle','trailing'),control_size:size,items:menuItemsSchema,placement:placementSchema,hang}),events:{...lifecycle,dismiss:choice(null),invoked:id}},
  ContextMenu:{props:object({name:string,target:id,presentation:choice('native','in_window'),items:menuItemsSchema}),events:{open:id,close:choice(null),dismiss:choice(null),invoked:id,unavailable:string},slots:['content']},
  Menubar:{props:object({menus,control_size:size}),events:{open:id,close:id,invoked:object({menu:id,item:id},['menu','item'])}},
  CommandPalette:{props:object({commands,query:string}),events:{query_changed:string,invoked:id,dismiss:choice(null)},slots:['empty']},
  ToastLayer:{props:object({corner,capacity:integer,reserved_edges:edges}),events:{action:id}},
  NotificationCenter:{props:object({capacity:integer,control_size:size}),events:{read:id,dismissed:id,cleared:choice(null),action_taken:id,action:id},slots:['empty']},
  Kbd:{props:object({keystroke:string},['keystroke']),events:{}},
  Tooltip:{props:object({text:string,describes:id},['text']),events:{}},
  Frost:{props:object({surface,radius,blur:nonnegative}),events:{},slots:['content']},
  Glass:{props:object({surface,radius,elevation:choice('flat','raised','overlay','modal'),radius_px:nonnegative,blur:nonnegative,preset:choice('liquid','frosted','clear','lens'),refraction:nonnegative,dispersion:nonnegative,specular:nonnegative,light_angle:number,focused:boolean,track_pointer:boolean,pressable:boolean,adaptive:boolean,adaptive_appearance:boolean,dimmed:boolean,tint:object({h:unit,s:unit,l:unit,a:unit},['h','s','l','a']),edge_mask:object({edge:choice('none','top','right','bottom','left'),band:nonnegative},['edge','band'])}),events:{},slots:['content']},
  Overlay:{props:object({constructor:choice('new','modal','edge'),edge,placement:placementSchema,hang,scrim:boolean,progress:unit,stack:integer,layer:choice('content','sticky','dock','popover','tooltip','modal','toast')}),events:{dismiss:choice(null)},slots:['content']},
});
const dataMethods={
  Drawer:{invoke:{open:method(),close:method(),dismiss:method(),settle:method(),set_title:method({title:string}),set_description:method({description:optional(string)}),set_size:method({size:nonnegative}),set_edge:method({edge}),set_dismissable:method({dismissable:boolean}),set_resizable:method({resizable:boolean}),set_content:slot,set_footer:slot},query:{is_open:method({},boolean),is_dismissable:method({},boolean),is_rendered:method({},boolean)}},
  HoverCard:{invoke:{open:method(),close:method(),dismiss:method(),set_name:method({name:optional(string)}),set_placement:method({placement:placementSchema}),set_hang:method({hang}),set_open_delay:method({delay:integer}),set_grace:method({grace:integer}),set_content:slot,set_trigger:slot},query:{is_open:method({},boolean),is_leaving:method({},boolean),grace_period:method({},integer)}},
  Menu:{invoke:{open:method(),close:method(),toggle:method(),dismiss:method(),open_submenu:method({id},boolean),set_items:method({items:menuItemsSchema}),set_trigger:method({label:string}),set_trigger_name:method({name:string}),set_trigger_icon:method({icon:optional(iconSchema)}),set_placement:method({placement:placementSchema}),set_hang:method({hang}),set_trigger_style:method({variant,size}),set_trigger_join:method({join:choice('alone','leading','middle','trailing')})},query:{is_open:method({},boolean),offered:method({},array(object({id,label:string,disabled:boolean,destructive:boolean},['id','label','disabled','destructive'])))}},
  ContextMenu:{invoke:{open_at:method({position:point}),close:method(),dismiss:method(),set_items:method({items:menuItemsSchema}),set_name:method({name:string}),set_target:method({target:optional(id)}),set_presentation:method({presentation:choice('native','in_window')}),set_content:slot},query:{is_open:method({},boolean),position:method({},point)}},
  Menubar:{invoke:{open:method({id}),close:method(),set_menus:method({menus}),set_control_size:method({size})},query:{open_menu:method({},optional(id)),menus:method({},array(object({id,label:string,disabled:boolean},['id','label','disabled'])))}},
  CommandPalette:{invoke:{set_commands:method({commands}),set_query:method({query:string}),focus_query:method()},query:{query:method({},string),active_id:method({},optional(id))}},
  ToastLayer:{invoke:{push:method({toast}),dismiss:method({ident:id},boolean),clear:method(),set_corner:method({corner}),set_capacity:method({capacity:integer}),set_reserved_edges:method({edges})},query:{len:method({},integer),is_empty:method({},boolean),phase:method({ident:id},choice(null,'entering','present','exiting','gone'))}},
  NotificationCenter:{invoke:{record:method({notification}),show:method({notification},boolean),mark_read:method({id},boolean),mark_all_read:method(),dismiss:method({id},boolean),clear:method(),set_capacity:method({capacity:integer}),set_control_size:method({size})},query:{len:method({},integer),is_empty:method({},boolean),holds:method({id},boolean),is_read:method({id},optional(boolean)),unread:method({},object({kind:choice('exact','at-least'),value:integer},['kind','value']))}},
  Kbd:{invoke:{},query:{caps:method({},array(string))}},
};
export const familyReferenceMethods=Object.freeze({
  Drawer:{invoke:{set_focus_stops:method({stops:array(focusReference)})},query:{focus_handle:method({},focusReference)}},
  HoverCard:{query:{focus_handle:method({},focusReference)}},
  Menu:{query:{focus_handle:method({},focusReference)}},
  ContextMenu:{query:{focus_handle:method({},focusReference)}},
  CommandPalette:{query:{focus_handle:method({},focusReference),query_input:method({},reference('TextInput'))}},
  NotificationCenter:{query:{focus_handle:method({},focusReference)}},
});
export const familyMethods=Object.freeze(Object.fromEntries(Object.entries(dataMethods).map(([name,modes])=>[name,{
  invoke:{...modes.invoke,...familyReferenceMethods[name]?.invoke},query:{...modes.query,...familyReferenceMethods[name]?.query},
}])));
export function validateOverlayProps(component,props){
  function unique(items){const ids=new Set();function visit(items){for(const item of items??[]){if(ids.has(item.id))throw new TypeError('Menu: duplicate identity');ids.add(item.id);if(item.items)visit(item.items);}}visit(items);}
  if(component==='Menu'||component==='ContextMenu')unique(props.items);
  if(component==='Menubar')for(const menu of props.menus??[])unique(menu.items);
  if(component==='Overlay'&&props.constructor==='edge'&&!props.edge)throw new TypeError('Overlay.edge requires edge');
}

/** Regenerate the family declaration from the same closed wire grammar. */
export function generateOverlayTypes(){
  function type(s){
    if(s.oneOf)return s.oneOf.map(type).join(' | ');
    if(s.type==='object'&&!Object.keys(s.fields).length)return 'Record<string, never>';
    let t=s.enum?s.enum.map(v=>JSON.stringify(v)).join(' | '):s.type==='array'?`Array<${type(s.items)}>`:s.type==='object'?`{ ${Object.entries(s.fields).map(([k,v])=>`${JSON.stringify(k)}${s.required.includes(k)?'':'?'}: ${type(v)}`).join('; ')} }`:s.type;
    return s.nullable?`${t} | null`:t;
  }
  const factories=Object.entries(familySchemas).map(([name,s])=>`  ${name}(id: string, props${s.props.required.length?'':'?'}: ${type(s.props)}, events?: ${Object.keys(s.events).length?`{ ${Object.entries(s.events).map(([name,s])=>`${JSON.stringify(name)}?: (value: ${type(s)}) => void`).join('; ')} }`:'Record<string, never>'}, slots?: ${s.slots?.length?`{ ${s.slots.map(name=>`${JSON.stringify(name)}?: SlotNode[]`).join('; ')} }`:'Record<string, never>'}): KitNode;`).join('\n');
  const methods=Object.entries(familyMethods).map(([name,modes])=>`  ${name}: { ${Object.entries(modes).map(([mode,methods])=>`${mode}: { ${Object.entries(methods).map(([name,m])=>`${name}: { args: ${type(m.args)}; result: ${type(m.result)} }`).join('; ')} }`).join('; ')} };`).join('\n');
  return `// Generated by generateOverlayTypes in kit-overlay-schema.mjs.\nimport type { KitNode, SlotNode } from './kit-sdk.js';\nexport interface OverlayFactories {\n${factories}\n}\nexport interface OverlayMethodContracts {\n${methods}\n}\n`;
}
