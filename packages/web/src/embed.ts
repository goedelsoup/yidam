import { pipeline, env } from '@xenova/transformers';

env.allowLocalModels = false;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
let extractor: any = null;

export async function initEmbedder(modelName: string): Promise<void> {
  extractor = await pipeline('feature-extraction', modelName);
}

export async function embed(text: string): Promise<Float32Array> {
  if (!extractor) throw new Error('Embedder not initialized. Call initEmbedder() first.');
  const output = await extractor(text, { pooling: 'mean', normalize: true });
  return output.data as Float32Array;
}
