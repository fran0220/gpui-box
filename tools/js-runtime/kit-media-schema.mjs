const string = { type: 'string', max: 16384 };
const number = { type: 'number', min: -1e9, max: 1e9 };
const positive = { type: 'number', min: 0.001, max: 1000000 };
const unit = { type: 'number', min: 0, max: 1 };
const choice = (...values) => ({ enum: values });
const array = items => ({ type: 'array', max: 1024, items });
const object = (fields, required = []) => ({ type: 'object', fields, required });
const player = { title: string, elapsed: string, remaining: string, stepSeconds: positive, speeds: array(positive) };

export const familySchemas = Object.freeze({
  AudioWaveform: { props: object({ peaks: array(unit), playhead: unit, state: choice('loading', 'empty', 'unavailable', 'error', 'ready'), reason: string }), events: {}, slots: ['empty'] },
  AudioPlayer: { props: object({ ...player, subtitle: string, peaks: array(unit) }), events: {} },
  VideoPlayer: { props: object({ ...player, ratio: positive }), events: {}, slots: ['poster'] },
  ModelViewer: { props: object({ title: string, document: string, state: choice('empty', 'loading'), shading: choice('flat', 'wireframe'), yaw: number, pitch: number, height: positive }), events: { event: object({ kind: choice('orbitChanged', 'shadingChanged'), yaw: number, pitch: number, shading: choice('flat', 'wireframe') }, ['kind']) } },
});
export const familyMethods = Object.freeze({});
export const unsupportedBoundaries = Object.freeze({
  AudioPlayer: 'No capability-owned transport handle exists. Native unavailable state; no simulated playback or success events.',
  VideoPlayer: 'No capability-owned transport/frame handle exists. Caller poster UI does not represent playback.',
  ModelViewer: 'Only bounded inline glTF parser data; external resources are rejected by the native parser.',
});
