// No filesystem/network IO, compressed decoders, or native object references.
export const RESOURCE_LIMITS = Object.freeze({ encoded: 128 * 1024, bytes: 96 * 1024, dimension: 1024, ownerBytes: 1024 * 1024, ownerCount: 32 });
const RGBA = 'image/x.gpui-rgba8';

function closed(value, keys) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).length !== keys.length || keys.some(key => !Object.hasOwn(value, key)))
    throw new Error('Invalid resource object');
}

export function validateResourceRef(value) {
  closed(value, ['key']);
  if (typeof value.key !== 'string' || !/^[A-Za-z0-9_-]{1,128}$/.test(value.key)) throw new Error('Invalid resource key');
  return value;
}

export function validateResourceRegistration(value) {
  closed(value, ['key', 'mime', 'data']);
  validateResourceRef({ key: value.key });
  if (![RGBA, 'application/octet-stream'].includes(value.mime)) throw new Error('Unsupported resource MIME');
  if (typeof value.data !== 'string' || value.data.length === 0 || value.data.length > RESOURCE_LIMITS.encoded || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value.data)) throw new Error('Invalid resource encoding');
  const bytes = Buffer.from(value.data, 'base64');
  if (bytes.toString('base64') !== value.data || bytes.length > RESOURCE_LIMITS.bytes) throw new Error('Invalid resource bytes');
  if (value.mime === RGBA) {
    if (bytes.length < 8) throw new Error('Truncated RGBA header');
    const width = bytes.readUInt32LE(0), height = bytes.readUInt32LE(4);
    if (width < 1 || height < 1 || width > RESOURCE_LIMITS.dimension || height > RESOURCE_LIMITS.dimension || bytes.length !== 8 + width * height * 4) throw new Error('Invalid RGBA dimensions/allocation');
  }
  return value;
}

/** Encode bounded raw pixels; this is not a compressed image decoder. */
export function rgbaResource(key, width, height, pixels) {
  if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1 || width > RESOURCE_LIMITS.dimension || height > RESOURCE_LIMITS.dimension || width * height * 4 + 8 > RESOURCE_LIMITS.bytes || !(pixels instanceof Uint8Array) || pixels.length !== width * height * 4) throw new Error('Invalid RGBA dimensions/allocation');
  const bytes = Buffer.alloc(8 + pixels.length);
  bytes.writeUInt32LE(width, 0); bytes.writeUInt32LE(height, 4); bytes.set(pixels, 8);
  return validateResourceRegistration({ key, mime: RGBA, data: bytes.toString('base64') });
}
