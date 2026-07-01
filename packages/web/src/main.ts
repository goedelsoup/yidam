import { loadBundle } from './bundle.ts';
import { CorpusIndex } from './db.ts';
import { initEmbedder, embed } from './embed.ts';
import { setStatus, setDomainName, enableSearch, renderResults } from './ui.ts';

function getBundleUrl(): string {
  const params = new URLSearchParams(window.location.search);
  return params.get('bundle') ?? '/bundle.yiz';
}

async function main(): Promise<void> {
  const bundleUrl = getBundleUrl();
  setStatus(`Fetching ${bundleUrl}…`);

  const { manifest, arrowBytes } = await loadBundle(bundleUrl);

  if (!manifest || !arrowBytes) {
    setStatus(
      'Bundle has no vector index. ' +
      'Run `yidam embed && yidam index-build && yidam bundle` to produce one.'
    );
    return;
  }

  setStatus(`Loading model (${manifest.model_name})…`);
  await initEmbedder(manifest.model_name);

  const index = CorpusIndex.fromArrow(arrowBytes);
  setDomainName('Corpus');
  enableSearch();
  setStatus(`Ready — ${index.nodeCount} nodes indexed.`);

  const input = document.getElementById('query') as HTMLInputElement;

  let debounce: ReturnType<typeof setTimeout>;
  input.addEventListener('input', () => {
    clearTimeout(debounce);
    debounce = setTimeout(async () => {
      const q = input.value.trim();
      if (!q) {
        renderResults([]);
        setStatus(`${index.nodeCount} nodes indexed.`);
        return;
      }
      setStatus('Searching…');
      const vec = await embed(q);
      const results = index.search(vec);
      renderResults(results);
      setStatus(`${results.length} result(s) for "${q}"`);
    }, 300);
  });
}

main().catch(err => {
  console.error(err);
  setStatus(`Error: ${String(err)}`);
});
