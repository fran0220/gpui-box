import type { NavigationExtraFactories } from '../../../../../js-runtime/kit-navigation_extra-sdk.js';
import type { LayoutExtraFactories, LayoutExtraMethods } from '../../../../../js-runtime/kit-layout_extra-sdk.js';
import type { DatetimeFactories, DatetimeMethods, DateAdapterData } from '../../../../../js-runtime/kit-datetime-sdk.js';
declare const nav: NavigationExtraFactories;
declare const layout: LayoutExtraFactories;
declare const dates: DatetimeFactories;
declare const adapter: DateAdapterData;
nav.NavStack('visits', { entries: [{id:'a'},{id:'b'}], cursor: 1, label: 'Visits' });
nav.Sidebar('side', { sections:[{id:'s'}], items:[{id:'a',label:'A',section:'s',icon:{key:'arrow-left',weight:'fill'}}] });
nav.Wizard('flow', {}, { navigate(value) { if(value.kind === 'step') value.id satisfies string; } });
// @ts-expect-error missing caller-owned history
nav.NavStack('visits', {cursor:0,label:'Visits'});
// @ts-expect-error native handles and image paths cannot cross the worker
nav.Sidebar('side', {items:[{id:'a',label:'A',section:'s',image:'/etc/passwd'}]});
layout.DockTree('dock', {records:[{id:'s',kind:'stack',panels:['a']}]}, {event(value) {
  if(value.kind === 'floatingChanged') {value.finished satisfies boolean;value.bounds.width satisfies number;}
  if(value.kind === 'floatingCancelled') {
    // @ts-expect-error cancellation has no completed geometry
    value.finished;
  }
}});
// @ts-expect-error dimension is a number
layout.AspectRatio('ratio',{ratio:'16/9'});
dates.DateInput('date',{adapter,value:31});
dates.RangePicker('range',{adapter,range:{start:31,end:11}});
// @ts-expect-error Day is an opaque number, not a date string
dates.DateInput('date',{adapter,value:'2026-09-09'});
const command: DatetimeMethods['Calendar']['invoke']['set_selection']['args']={days:[31,11]};
command.days satisfies number[];
// @ts-expect-error named arguments are exact
const bad: DatetimeMethods['Calendar']['invoke']['set_selection']['args']={selection:[11]};
declare const blocked: DatetimeMethods['RangePicker']['query']['blocked']['result'];
if(blocked.kind==='blocked') blocked.days[0].reason satisfies string;
if(blocked.kind==='unchecked') {
  // @ts-expect-error unchecked is not a clear or checked empty list
  blocked.days;
}
declare const ratio: LayoutExtraMethods['AspectRatio']['query']['ratio']['result'];
ratio satisfies number;
// @ts-expect-error native Entity<TextInput> getter is unsupported, not a data query
type FieldRef = DatetimeMethods['DateInput']['query']['field'];
// @ts-expect-error native Entity<Calendar> getter is unsupported
type DateCalendarRef = DatetimeMethods['DateInput']['query']['calendar'];
// @ts-expect-error native Entity<Calendar> getter is unsupported
type RangeCalendarRef = DatetimeMethods['RangePicker']['query']['calendar'];
// @ts-expect-error native Rc<dyn DateAdapter> getter is unsupported
type AdapterRef = DatetimeMethods['Calendar']['query']['adapter'];
declare const fieldSnapshot: DatetimeMethods['DateInput']['query']['field_snapshot']['result'];
fieldSnapshot.value satisfies string;
declare const calendarSnapshot: DatetimeMethods['RangePicker']['query']['calendar_snapshot']['result'];
calendarSnapshot.selection satisfies number[];
declare const adapterSnapshot: DatetimeMethods['Calendar']['query']['adapter_snapshot']['result'];
adapterSnapshot.weekdays satisfies string[];
