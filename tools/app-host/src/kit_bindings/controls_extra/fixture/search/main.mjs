const last = gpui.state('Caller-owned fixture counts; no document search');
gpui.mount(() => {
  const find = gpui.kit.FindReplace('search.find',{count:{state:'known',total:7,current:2}}, {replaceAll:({count}) => last.set(`Replace intent for ${count} verified matches`)});
  return gpui.column('search.fixture',[
    gpui.text('search.title','Native SearchField / FindReplace · counts and typed references'),
    gpui.text('search.last',last.get()),
    gpui.button('search.references','Use native query reference',async () => {
      const search = await gpui.query(find,'search_field');
      const input = await gpui.query(search,'query_input');
      await gpui.invoke(input,'set_value',{value:'Native child fixture'});
      const text = await gpui.query(search,'query_text');
      last.set(`Verified native child: ${text}`);
    }),
    find,
    gpui.kit.FindReplace('search.too-many',{count:{state:'tooMany',counted:500}}),
    gpui.kit.SearchField('search.known',{query:'Fixture',count:{state:'known',total:9,current:1},matchCase:true,wholeWord:false}),
    gpui.kit.SearchField('search.unsearched',{count:{state:'unsearched'}}),
    gpui.kit.SearchField('search.counting',{query:'Counting fixture',count:{state:'counting'}}),
    gpui.kit.SearchField('search.none',{query:'No hits fixture',count:{state:'none'}}),
    gpui.kit.SearchField('search.refused',{query:'Refused fixture',count:{state:'unavailable',reason:'Host refused fixture search'},disabled:true}),
  ]);
});
