export type SfxLayer = Record<string, unknown>;
export interface SfxSpec {
  name?: string;
  preset?: string;
  seed?: number;
  pitch?: number;
  length?: number;
  volume?: number;
  echo?: [number, number, number?];
  layers: SfxLayer[];
}
export const SAMPLE_RATE: number;
export function rng(seed: number): () => number;
export function sfxDuration(spec: SfxSpec): number;
export function renderSfx(spec: SfxSpec, rate?: number): Float32Array;
export function encodeWav(samples: Float32Array, rate?: number): Uint8Array;
export const PRESETS: Record<string, { label: string; build: (r: () => number) => { echo?: number[]; layers: SfxLayer[] } }>;
export function makePreset(
  preset: string,
  options?: { seed?: number; pitch?: number; length?: number; name?: string },
): SfxSpec;
