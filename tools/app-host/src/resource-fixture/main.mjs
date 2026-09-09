// Explicit fixture pixels. No filesystem or network resource permission.
const status = gpui.state('Awaiting registration');
const bytes = Buffer.alloc(8 + 32 * 16 * 4);
bytes.writeUInt32LE(32, 0); bytes.writeUInt32LE(16, 4);
for (let pixel = 0; pixel < 32 * 16; pixel++) bytes.set(pixel % 32 < 16 ? [255, 0, 0, 255] : [0, 64, 255, 255], 8 + pixel * 4);
const registration = { key: 'pixels', mime: 'image/x.gpui-rgba8', data: bytes.toString('base64') };
gpui.mount(() => gpui.column('resource-root', [
  gpui.text('resource-status', status.get()),
  gpui.button('unsafe-resource', 'Reject unsafe resource', async () => {
    try { await gpui.resources.register({ ...registration, key: 'file:///etc/passwd' }); status.set('UNSAFE RESOURCE ACCEPTED'); }
    catch { status.set('Unsafe resource rejected'); }
  }),
  gpui.button('register-resource', 'Register fixture pixels', async () => {
    status.set('Registration pending');
    try { const reference = await gpui.resources.register(registration); status.set(`Ready: ${reference.key}`); }
    catch (error) { status.set(`Registration refused: ${error.message}`); }
  }),
]));
