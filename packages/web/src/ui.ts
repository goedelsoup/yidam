import type { SearchResult } from './db.ts';

export function setStatus(msg: string): void {
  const el = document.getElementById('status');
  if (el) el.textContent = msg;
}

export function setDomainName(name: string): void {
  const el = document.getElementById('domain-name');
  if (el) el.textContent = name;
  document.title = name;
}

export function enableSearch(): void {
  const el = document.getElementById('query') as HTMLInputElement | null;
  if (el) { el.disabled = false; el.focus(); }
}

export function renderResults(results: SearchResult[]): void {
  const el = document.getElementById('results');
  if (!el) return;

  if (results.length === 0) {
    el.innerHTML = '<p class="empty">No results.</p>';
    return;
  }

  el.innerHTML = results
    .map(r => `
      <article class="result">
        <h3 class="result-label">${esc(r.label)}</h3>
        <p class="result-text">${esc(r.text)}</p>
        <span class="result-meta">${esc(r.class)} · ${(r.score * 100).toFixed(1)}% match</span>
      </article>
    `)
    .join('');
}

function esc(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}
