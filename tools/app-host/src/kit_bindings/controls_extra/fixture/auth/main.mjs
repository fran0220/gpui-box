const last = gpui.state('Synthetic fixtures only; no authentication or account service');
gpui.mount(() => gpui.column('auth.fixture', [
  gpui.text('auth.title','Native sensitive inputs · masking, refusal and controlled data'),
  gpui.text('auth.last',last.get()),
  gpui.kit.PasswordInput('auth.password',{name:'Fixture password',value:'fixture-only',placeholder:'Enter fixture'}, {change:() => last.set('Native password change received')}),
  gpui.kit.PasswordInput('auth.password.invalid',{name:'Refused fixture',value:'invalid-fixture',invalid:true,readOnly:true}),
  gpui.kit.PasswordInput('auth.password.disabled',{name:'Disabled fixture',value:'disabled-fixture',disabled:true}),
  gpui.kit.OneTimeCodeInput('auth.code',{name:'Fixture code',value:'123',slots:6},{change:() => last.set('Native code change received'),submit:() => last.set('Code submit intent received')}),
  gpui.kit.OneTimeCodeInput('auth.code.invalid',{name:'Invalid code fixture',value:'1234',slots:4,invalid:true}),
  gpui.kit.OneTimeCodeInput('auth.code.disabled',{name:'Disabled code fixture',value:'12',slots:4,disabled:true}),
]));
