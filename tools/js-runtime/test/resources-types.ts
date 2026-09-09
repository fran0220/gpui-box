import type { ResourceAPI, ResourceRef, ResourceRegistration, AssetDeclaration } from '../resource-sdk.js';
declare const resources: ResourceAPI;
const registration: ResourceRegistration = { key: 'image', mime: 'image/x.gpui-rgba8', data: 'AQ==' };
const result: Promise<ResourceRef> = resources.register(registration);
const asset: AssetDeclaration = { key: 'model', path: 'assets/model.bin', mime: 'application/octet-stream' };
void result; void asset;
// @ts-expect-error a path/URL is not a resource reference
const path: ResourceRef = 'file:///etc/passwd';
// @ts-expect-error callers cannot select another owner
const foreign: ResourceRef = { key: 'image', owner: 42 };
// @ts-expect-error compressed formats are not supported by this boundary
resources.register({ key: 'image', mime: 'image/png', data: '' });
// @ts-expect-error there is no implicit native network resource
resources.register({ key: 'image', url: 'https://example.com/x' });
void path; void foreign;
