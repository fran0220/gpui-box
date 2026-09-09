const query = gpui.state('');
const last = gpui.state('No setting action requested');
gpui.mount(() => gpui.column('settings.fixture', [
  gpui.text('settings.title', 'Native nested settings · caller-owned fixture actions'),
  gpui.text('settings.last', last.get()),
  gpui.kit.SettingsList('settings.list', {query:query.get()}, {}, {
    header:[gpui.row('settings.filters', [
      gpui.button('settings.all', 'All settings', () => query.set('')),
      gpui.button('settings.filter', 'Find quota', () => query.set('quota')),
      gpui.button('settings.none', 'No matches', () => query.set('not-a-setting')),
    ])],
    empty:[gpui.text('settings.empty', 'No matching settings · fixture')],
    footer:[gpui.text('settings.footer', 'Page footer remains visible when the list is empty')],
    sections:[
      gpui.kit.SettingsSection('settings.storage', {title:'Storage',description:'Fixture controls; no host settings are changed',labelWidth:160}, {}, {
        action:[gpui.button('settings.reset', 'Reset fixture', () => last.set('Section action retained'))],
        rows:[
          gpui.kit.SettingsRow('settings.capacity', {label:'Capacity',description:'Available storage',searchTerms:['quota']}, {}, {
            control:[gpui.kit.Button('settings.change', {label:'Change'}, {click:() => last.set('Nested row action retained')})],
          }),
          gpui.kit.SettingsRow('settings.policy', {label:'Retention',value:'30 days',managed:'Organization policy'}, {}, {
            control:[gpui.kit.Button('settings.forbidden', {label:'Must not mount'}, {click:() => last.set('Forbidden action')})],
          }),
        ],
        content:[
          gpui.text('settings.block.before','A full-width block before the next native row'),
          gpui.kit.SettingsRow('settings.mixed', {label:'Storage class',value:'Standard'}),
          gpui.text('settings.block.after','A full-width block after the native row'),
        ],
      }),
      gpui.kit.SettingsSection('settings.unavailable', {title:'Advanced settings',dimmedBy:'Not available in this fixture'}, {}, {
        rows:[gpui.kit.SettingsRow('settings.withheld', {label:'Withheld'}, {}, {control:[gpui.text('settings.withheld.control','Must not mount')]})],
      }),
    ],
  }),
]));
