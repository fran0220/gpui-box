/** A name in the current owner's current generation. Never a file path or URL. */
export interface ResourceRef { readonly key: string }
export interface ResourceRegistration {
  readonly key: string;
  /** RGBA8 uses LE u32 width,height then tightly packed RGBA8 pixels. */
  readonly mime: 'image/x.gpui-rgba8' | 'application/octet-stream';
  /** Canonical base64; at most 128KiB encoded / 96KiB decoded. */
  readonly data: string;
}
export interface ResourceAPI {
  /** Requires a separate host-approved resources grant. Errors remain retryable.
   * Keys are immutable until owner revocation; no path/network authority follows.
   */
  register(registration: ResourceRegistration): Promise<ResourceRef>;
}
export interface AssetDeclaration {
  readonly key: string;
  readonly path: string;
  readonly mime: ResourceRegistration['mime'];
}
export declare const RESOURCE_LIMITS: Readonly<{ encoded: number; bytes: number; dimension: number; ownerBytes: number; ownerCount: number }>;
export declare function validateResourceRef(value: unknown): ResourceRef;
export declare function validateAssetDeclarations(value: unknown): AssetDeclaration[];
export declare function validateResourceRegistration(value: unknown): ResourceRegistration;
export declare function rgbaResource(key: string, width: number, height: number, pixels: Uint8Array): ResourceRegistration;
