import { test } from 'node:test';
import assert from 'node:assert/strict';
import { RESOURCE_LIMITS, rgbaResource, validateResourceRef, validateResourceRegistration } from '../resource-schema.mjs';

test('closed resource reference rejects every IO spelling and owner authority', () => {
  for (const key of ['', '../x', '/etc/passwd', 'a/b', 'a\\b', 'https://example.com/x', 'file:///x', '%2e%2e', 'é', 'a'.repeat(129)]) assert.throws(() => validateResourceRef({ key }));
  for (const extra of ['owner', 'generation', 'url', 'path']) assert.throws(() => validateResourceRef({ key: 'known', [extra]: 1 }));
  assert.deepEqual(validateResourceRef({ key: 'Red_2-pixels' }), { key: 'Red_2-pixels' });
});

test('RGBA wire bytes have exact dimensions and known channel order', () => {
  const registration = rgbaResource('pixels', 2, 1, Uint8Array.from([255, 0, 0, 255, 0, 0, 255, 255]));
  assert.deepEqual([...Buffer.from(registration.data, 'base64')], [2,0,0,0,1,0,0,0,255,0,0,255,0,0,255,255]);
  assert.equal(validateResourceRegistration(registration), registration);
  for (const [width, height, pixels] of [[0,1,new Uint8Array()], [1025,1,new Uint8Array()], [1024,1024,new Uint8Array()], [2,1,new Uint8Array(4)]]) assert.throws(() => rgbaResource('x', width, height, pixels));
});

test('encoded bytes and decoded allocation are bounded; compressed bombs never reach decoder', () => {
  const value = { key: 'x', mime: 'application/octet-stream', data: 'AQ==' };
  for (const data of ['', '!!!!', 'AQ', 'AR==', 'AQ==\n', Buffer.alloc(RESOURCE_LIMITS.bytes + 1).toString('base64')]) assert.throws(() => validateResourceRegistration({ ...value, data }));
  assert.equal(validateResourceRegistration({ ...value, data: Buffer.alloc(RESOURCE_LIMITS.bytes).toString('base64') }).key, 'x');
  for (const mime of ['image/png', 'image/jpeg', 'image/gif', 'image/webp', 'image/svg+xml', 'application/gzip']) assert.throws(() => validateResourceRegistration({ ...value, mime }));
  for (const raw of [Buffer.alloc(7), Buffer.alloc(8, 255), Buffer.from([0,4,0,0,0,4,0,0])]) assert.throws(() => validateResourceRegistration({ ...value, mime: 'image/x.gpui-rgba8', data: raw.toString('base64') }));
  assert.throws(() => validateResourceRegistration({ ...value, path: '/etc/passwd' }));
});
