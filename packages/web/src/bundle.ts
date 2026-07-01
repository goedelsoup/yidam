export interface BundleManifest {
  model_name: string;
  embedding_dim: number;
  node_count: number;
  indexed_commit: string;
}

export interface BundleContents {
  manifest: BundleManifest | null;
  arrowBytes: Uint8Array | null;
}

async function decompressGzip(data: ArrayBuffer): Promise<ArrayBuffer> {
  const ds = new DecompressionStream('gzip');
  const readable = new Response(data).body!.pipeThrough(ds);
  const reader = readable.getReader();
  const chunks: Uint8Array[] = [];
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
  }
  const total = chunks.reduce((n, c) => n + c.length, 0);
  const out = new Uint8Array(total);
  let pos = 0;
  for (const chunk of chunks) {
    out.set(chunk, pos);
    pos += chunk.length;
  }
  return out.buffer;
}

function parseTar(buffer: ArrayBuffer): Map<string, Uint8Array> {
  const files = new Map<string, Uint8Array>();
  const view = new Uint8Array(buffer);
  const dec = new TextDecoder();
  let offset = 0;

  while (offset + 512 <= view.length) {
    const nameBytes = view.subarray(offset, offset + 100);
    const nullIdx = nameBytes.indexOf(0);
    const name = dec.decode(nullIdx >= 0 ? nameBytes.subarray(0, nullIdx) : nameBytes).trim();
    if (!name) break;

    const sizeStr = dec.decode(view.subarray(offset + 124, offset + 136));
    const size = parseInt(sizeStr.trim(), 8) || 0;
    const typeFlag = dec.decode(view.subarray(offset + 156, offset + 157));

    offset += 512;

    // typeFlag '0' or '\0' = regular file; '5' = directory — skip dirs
    if (size > 0 && typeFlag !== '5') {
      files.set(name, view.slice(offset, offset + size));
    }
    offset += Math.ceil(size / 512) * 512;
  }

  return files;
}

export async function loadBundle(url: string): Promise<BundleContents> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch bundle: ${response.status} ${response.statusText}`);
  }

  const compressed = await response.arrayBuffer();
  const raw = await decompressGzip(compressed);
  const files = parseTar(raw);

  const metaBytes = files.get('index/meta.json');
  const manifest: BundleManifest | null = metaBytes
    ? JSON.parse(new TextDecoder().decode(metaBytes))
    : null;

  const arrowBytes = files.get('index/corpus.arrow') ?? null;

  return { manifest, arrowBytes };
}
