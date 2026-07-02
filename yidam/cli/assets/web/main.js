// yidam web agent — offline browser agent over a .yiz bundle.
// Design contract: yidam/design/ui_kits/web-agent/DESIGN.md
//
// No build step, no framework. transformers.js, apache-arrow, and WebLLM are
// dynamically imported from the CDN URLs pinned in web.config.json — nothing
// heavy loads until the stage that needs it.

const config = JSON.parse(document.getElementById('web-config').textContent);

const $ = (id) => document.getElementById(id);
const el = (tag, cls, text) => {
  const node = document.createElement(tag);
  if (cls) node.className = cls;
  if (text !== undefined) node.textContent = text;
  return node;
};

const state = {
  nodes: new Map(),   // id -> {id, class, label, description, content, links: [{target, relationship}]}
  incoming: new Map(),// id -> [{source, relationship}]
  index: null,        // {rows: [{path, class, label, text, vector}], dims}
  embedder: null,
  chat: { engine: null, status: 'off' },
  selected: null,
};

// ── status bar ──────────────────────────────────────────────────────

function setStatus() {
  const parts = [config.domain || 'corpus', `${state.nodes.size} nodes`];
  parts.push(state.index ? `index: ${config.embed_config?.model_id ?? 'unknown'}` : 'index: absent');
  parts.push(`webllm: ${state.chat.status}`);
  $('status').textContent = parts.join(' · ');
}

// ── load sequence (states 1–2) ──────────────────────────────────────

const stages = {};
function stage(name, progress, done = false) {
  const list = $('load-stages');
  list.classList.remove('hidden');
  if (!stages[name]) {
    const row = el('div', 'load-stage');
    row.append(el('span', 'label', name), el('span', 'progress'));
    list.append(row);
    stages[name] = row;
  }
  stages[name].querySelector('.progress').textContent = progress;
  stages[name].classList.toggle('done', done);
}

function loadError(message) {
  const banner = $('load-error');
  banner.textContent = message;
  banner.classList.remove('hidden');
}

async function boot() {
  $('header-domain').textContent = config.domain || 'yidam';
  setStatus();

  const drop = $('drop-target');
  const input = $('file-input');
  drop.addEventListener('click', () => input.click());
  input.addEventListener('change', () => input.files[0] && loadFromFile(input.files[0]));
  drop.addEventListener('dragover', (e) => { e.preventDefault(); drop.classList.add('hover'); });
  drop.addEventListener('dragleave', () => drop.classList.remove('hover'));
  drop.addEventListener('drop', (e) => {
    e.preventDefault();
    drop.classList.remove('hover');
    const file = e.dataTransfer.files[0];
    if (file) loadFromFile(file);
  });

  // Auto-fetch the co-located bundle; expected to fail under file://, in
  // which case the drop target stays as the entry point.
  try {
    const resp = await fetch(config.bundle_path);
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    const bytes = await resp.arrayBuffer();
    stage('bundle', `fetched · ${fmtBytes(bytes.byteLength)}`, true);
    await loadBundle(bytes);
  } catch {
    // stay in state 1
  }
}

async function loadFromFile(file) {
  stage('bundle', `read · ${fmtBytes(file.size)}`, true);
  await loadBundle(await file.arrayBuffer()).catch((e) => loadError(String(e)));
}

function fmtBytes(n) {
  return n > 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${(n / 1024).toFixed(1)} KB`;
}

// ── bundle: gunzip + untar ──────────────────────────────────────────

async function gunzip(buffer) {
  const ds = new DecompressionStream('gzip');
  const stream = new Response(buffer).body.pipeThrough(ds);
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

async function untar(view) {
  const files = new Map();
  const dec = new TextDecoder();
  let offset = 0;
  let entries = 0;
  while (offset + 512 <= view.length) {
    const nameField = view.subarray(offset, offset + 100);
    const nul = nameField.indexOf(0);
    const name = dec.decode(nul >= 0 ? nameField.subarray(0, nul) : nameField).trim();
    if (!name) break;
    const size = parseInt(dec.decode(view.subarray(offset + 124, offset + 136)).trim(), 8) || 0;
    const typeFlag = dec.decode(view.subarray(offset + 156, offset + 157));
    offset += 512;
    if (typeFlag === '0' || typeFlag === '\0' || typeFlag === '') {
      files.set(name, view.subarray(offset, offset + size));
    }
    offset += Math.ceil(size / 512) * 512;
    // Keep the main thread breathing on large bundles.
    if (++entries % 200 === 0) await new Promise((r) => setTimeout(r));
  }
  return files;
}

async function loadBundle(buffer) {
  const raw = await gunzip(buffer);
  stage('decompress', 'done', true);
  const files = await untar(raw);

  parseCorpus(files);
  stage('corpus', `${state.nodes.size} nodes parsed`, true);

  const arrow = files.get('index/corpus.arrow');
  if (arrow) {
    await parseArrow(arrow);
    stage('vector index', `${state.index.rows.length} rows · ${state.index.dims} dims`, true);
  } else {
    stage('vector index', 'absent — keyword search only', true);
  }

  const dec = new TextDecoder();
  const provenance = [];
  const manifest = files.get('manifest.yml');
  if (manifest) {
    const genesis = /genesis:\s*"?([^"\n]+)/.exec(dec.decode(manifest));
    if (genesis) provenance.push(`genesis ${genesis[1]}`);
  }
  provenance.push(`${state.nodes.size} nodes`);
  $('header-provenance').textContent = provenance.join(' · ');

  if (state.index && config.embed_config) {
    await initEmbedder();
  } else if (state.index) {
    stage('embedding model', 'no embed.config.json — keyword search only', true);
    state.index = null;
  }

  enterSearchState();
}

// ── corpus parsing (minimal YAML for the instance schema) ──────────

function parseInstanceYaml(text) {
  const node = { label: '', description: '', links: [] };
  let current = null;
  for (const line of text.split('\n')) {
    let m;
    if ((m = /^label:\s*"?(.*?)"?\s*$/.exec(line))) node.label = m[1];
    else if ((m = /^description:\s*"?(.*?)"?\s*$/.exec(line))) node.description = m[1];
    else if (/^\s+-\s+target:/.test(line)) {
      m = /target:\s*"?(.*?)"?\s*$/.exec(line);
      current = { target: m ? m[1] : '', relationship: 'link' };
      node.links.push(current);
    } else if (current && (m = /^\s+relationship:\s*"?(.*?)"?\s*$/.exec(line))) {
      current.relationship = m[1];
    }
  }
  return node;
}

function resolveTarget(sourceClass, target) {
  const parts = [sourceClass];
  for (const comp of target.split('/')) {
    if (comp === '.' || comp === '') continue;
    if (comp === '..') { if (!parts.pop()) return target; }
    else parts.push(comp);
  }
  return parts.join('/').replace(/\.yml$/, '');
}

function parseCorpus(files) {
  const dec = new TextDecoder();
  for (const [path, bytes] of files) {
    const m = /^corpus\/([^/]+)\/([^/]+)\.yml$/.exec(path);
    if (!m) continue;
    const [, cls, name] = m;
    const content = dec.decode(bytes);
    const parsed = parseInstanceYaml(content);
    const id = `${cls}/${name}`;
    state.nodes.set(id, {
      id, class: cls, label: parsed.label || name, description: parsed.description,
      content,
      links: parsed.links.map((l) => ({ target: resolveTarget(cls, l.target), relationship: l.relationship })),
    });
  }
  for (const node of state.nodes.values()) {
    for (const link of node.links) {
      if (!state.incoming.has(link.target)) state.incoming.set(link.target, []);
      state.incoming.get(link.target).push({ source: node.id, relationship: link.relationship });
    }
  }
}

// ── vector index ────────────────────────────────────────────────────

async function parseArrow(bytes) {
  const { tableFromIPC } = await import(config.cdn.arrow);
  const table = tableFromIPC(bytes.slice());
  const rows = [];
  const dims = table.getChild('vector')?.type?.listSize ?? 0;
  for (let i = 0; i < table.numRows; i++) {
    const row = table.get(i);
    rows.push({
      path: row.path, class: row.class, label: row.label, text: row.text,
      vector: Float32Array.from(row.vector.toArray()),
    });
  }
  state.index = { rows, dims };
}

async function initEmbedder() {
  const ec = config.embed_config;
  stage('embedding model', `loading ${ec.model_id}…`);
  const { pipeline } = await import(`${config.cdn.transformers}`);
  // The embed contract pins the weights file; quantized and fp32 exports of
  // the same model are ~1e-2 apart per element — dtype must match.
  const dtype = ec.model_file.includes('quantized') ? 'q8' : 'fp32';
  state.embedder = await pipeline('feature-extraction', ec.model_id, {
    dtype,
    progress_callback: (p) => {
      if (p.status === 'progress' && p.total) {
        stage('embedding model', `downloading · ${fmtBytes(p.loaded)} / ${fmtBytes(p.total)}`);
      }
    },
  });
  stage('embedding model', 'ready', true);
}

async function embedQuery(text) {
  const ec = config.embed_config;
  const output = await state.embedder(text, { pooling: ec.pooling, normalize: ec.normalize });
  return output.data;
}

// ── search (state 3) ────────────────────────────────────────────────

function enterSearchState() {
  $('load-state').classList.add('hidden');
  $('search').classList.add('active');
  setStatus();
  renderOverview();
  initChatPanel();

  let debounce;
  $('query').addEventListener('input', () => {
    clearTimeout(debounce);
    debounce = setTimeout(runSearch, 250);
  });
  $('query').focus();
}

function renderOverview() {
  const counts = new Map();
  let open = 0;
  for (const n of state.nodes.values()) {
    counts.set(n.class, (counts.get(n.class) ?? 0) + 1);
    if (n.label.startsWith('?') || n.content.includes('[open]')) open++;
  }
  const overview = $('overview');
  overview.replaceChildren();
  for (const [cls, count] of [...counts].sort()) {
    overview.append(el('span', 'badge', `${cls} · ${count}`));
  }
  if (open) overview.append(el('span', 'badge open', `${open} open`));
}

async function runSearch() {
  const query = $('query').value.trim();
  const results = $('results');
  if (!query) { results.replaceChildren(); return; }

  let ranked;
  if (state.index && state.embedder) {
    const qv = await embedQuery(query);
    ranked = state.index.rows
      .map((r) => {
        let score = 0;
        for (let i = 0; i < r.vector.length; i++) score += r.vector[i] * qv[i];
        return { ...r, score };
      })
      .sort((a, b) => b.score - a.score)
      .slice(0, 8);
  } else {
    const terms = query.toLowerCase().split(/\s+/);
    ranked = [...state.nodes.values()]
      .map((n) => {
        const haystack = `${n.label} ${n.description} ${n.content}`.toLowerCase();
        const hits = terms.filter((t) => haystack.includes(t)).length;
        return { path: `.yidam/corpus/${n.id}.yml`, class: n.class, label: n.label,
                 text: n.description, score: hits / terms.length };
      })
      .filter((r) => r.score > 0)
      .sort((a, b) => b.score - a.score)
      .slice(0, 8);
  }

  results.replaceChildren();
  for (const r of ranked) {
    const id = /corpus\/(.+)\.yml$/.exec(r.path)?.[1] ?? r.path;
    const node = state.nodes.get(id);
    const card = el('div', 'result-card');
    const top = el('div', 'top');
    top.append(el('span', 'label', r.label), el('span', 'score', r.score.toFixed(3)));
    card.append(top);
    card.append(el('div', 'desc', node?.description || r.text));
    const meta = el('div', 'meta');
    meta.append(el('span', 'badge', r.class));
    if (node) {
      const inc = state.incoming.get(id)?.length ?? 0;
      meta.append(el('span', 'links', `→ ${node.links.length} · ← ${inc}`));
    }
    card.append(meta);
    card.addEventListener('click', () => openDetail(id));
    results.append(card);
  }
}

// ── node detail (state 4) ───────────────────────────────────────────

function openDetail(id) {
  const node = state.nodes.get(id);
  if (!node) return;
  state.selected = node;
  $('panel').classList.add('open');
  switchTab('detail');

  const detail = $('detail');
  detail.replaceChildren();
  detail.append(el('div', 'crumb', `corpus / ${node.class} / ${node.id.split('/')[1]}`));
  detail.append(el('h2', null, node.label));

  const badges = el('div', 'badges');
  badges.append(el('span', 'badge', node.class));
  for (const marker of ['verified', 'inference', 'open']) {
    if (node.content.includes(`[${marker}]`)) badges.append(el('span', `marker ${marker}`, `[${marker}]`));
  }
  detail.append(badges);

  const dl = el('dl');
  for (const line of node.content.split('\n')) {
    const m = /^(\w[\w-]*):\s*(.*)$/.exec(line);
    if (m && m[1] !== 'links') {
      dl.append(el('dt', null, m[1]), el('dd', null, m[2].replace(/^"|"$/g, '')));
    }
  }
  detail.append(dl);

  renderMinimap(detail, node);

  if (state.chat.status === 'ready' || state.chat.status === 'available') {
    const ask = el('button', 'ghost', 'Ask about this node');
    ask.addEventListener('click', () => {
      switchTab('chat');
      $('chat-input').value = `Tell me about ${node.label}`;
      $('chat-input').focus();
    });
    detail.append(ask);
  }
}

function renderMinimap(container, node) {
  const out = node.links.map((l) => ({ id: l.target, rel: l.relationship, dir: 'out' }));
  const inc = (state.incoming.get(node.id) ?? []).map((l) => ({ id: l.source, rel: l.relationship, dir: 'in' }));
  const all = [...inc, ...out];
  if (!all.length) return;

  container.append(el('div', 'section-label', `links · → ${out.length} · ← ${inc.length}`));
  const H = Math.max(120, 34 * Math.max(inc.length, out.length) + 40);
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('id', 'minimap');
  svg.setAttribute('viewBox', `0 0 356 ${H}`);

  const mk = (name, attrs, text) => {
    const n = document.createElementNS('http://www.w3.org/2000/svg', name);
    for (const [k, v] of Object.entries(attrs)) n.setAttribute(k, v);
    if (text) n.textContent = text;
    return n;
  };
  const cy = H / 2;
  svg.append(mk('circle', { cx: 178, cy, r: 6, fill: 'var(--gold-500)' }));

  const place = (items, side) => {
    items.forEach((item, i) => {
      const y = 24 + i * ((H - 40) / Math.max(1, items.length - 1) || 0);
      const x = side === 'in' ? 30 : 326;
      const line = mk('line', {
        class: 'edge',
        x1: side === 'in' ? x + 6 : 172, y1: side === 'in' ? y : cy,
        x2: side === 'in' ? 172 : x - 6, y2: side === 'in' ? cy : y,
      });
      svg.append(line);
      svg.append(mk('circle', { cx: x, cy: y, r: 4, fill: 'var(--ink-400)' }));
      const label = mk('text', {
        x: side === 'in' ? x - 8 : x + 8, y: y + 3,
        'text-anchor': side === 'in' ? 'end' : 'start',
      }, (state.nodes.get(item.id)?.label ?? item.id).slice(0, 24));
      label.addEventListener('click', () => openDetail(item.id));
      svg.append(label);
    });
  };
  place(inc, 'in');
  place(out, 'out');
  container.append(svg);
}

function switchTab(name) {
  $('tab-detail').classList.toggle('active', name === 'detail');
  $('tab-chat').classList.toggle('active', name === 'chat');
  $('detail').classList.toggle('hidden', name !== 'detail');
  $('chat').classList.toggle('hidden', name !== 'chat');
}

$('tab-detail').addEventListener('click', () => switchTab('detail'));
$('tab-chat').addEventListener('click', () => { $('panel').classList.add('open'); switchTab('chat'); });

// ── chat (states 5–6) ───────────────────────────────────────────────

function initChatPanel() {
  const chat = $('chat');
  chat.replaceChildren();

  if (!navigator.gpu) {
    state.chat.status = 'unavailable';
    chat.append(el('div', 'no-webgpu',
      'On-device generation requires WebGPU. Retrieval is fully available — ' +
      'use the results to explore the corpus.'));
    setStatus();
    return;
  }

  state.chat.status = 'available';
  setStatus();
  const consent = el('div', 'consent');
  consent.append(el('div', null, 'Chat runs a language model entirely in your browser.'));
  const size = el('div', 'size');
  size.textContent = `${config.webllm.model} · large download, cached by your browser`;
  consent.append(size);
  const enable = el('button', 'primary', 'Download and enable');
  enable.addEventListener('click', () => startChat(chat));
  consent.append(enable);
  chat.append(consent);
}

async function startChat(chat) {
  chat.replaceChildren(el('div', 'progress', 'initialising…'));
  state.chat.status = 'loading';
  setStatus();
  try {
    const webllm = await import(config.cdn.webllm);
    const engine = await webllm.CreateMLCEngine(config.webllm.model, {
      initProgressCallback: (p) => {
        chat.querySelector('.progress').textContent = p.text;
      },
    });
    state.chat.engine = engine;
    state.chat.status = 'ready';
  } catch (e) {
    // Weak GPUs: fall back to the smaller model before giving up.
    try {
      const webllm = await import(config.cdn.webllm);
      const engine = await webllm.CreateMLCEngine(config.webllm.fallback_model, {
        initProgressCallback: (p) => {
          chat.querySelector('.progress').textContent = p.text;
        },
      });
      state.chat.engine = engine;
      state.chat.status = 'ready';
    } catch (e2) {
      state.chat.status = 'failed';
      setStatus();
      chat.replaceChildren(el('div', 'no-webgpu', `Model failed to load: ${e2}`));
      return;
    }
  }
  setStatus();

  chat.replaceChildren();
  const thread = el('div');
  thread.id = 'thread';
  const input = el('input');
  input.id = 'chat-input';
  input.placeholder = 'Ask a question about the corpus…';
  chat.append(thread, input);
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && input.value.trim()) {
      const q = input.value.trim();
      input.value = '';
      askQuestion(thread, q);
    }
  });
  input.focus();
}

async function askQuestion(thread, question) {
  thread.append(el('div', 'turn user', question));
  const answer = el('div', 'turn assistant', '…');
  thread.append(answer);

  // RAG: retrieve grounding nodes exactly the way search does.
  let sources = [];
  if (state.index && state.embedder) {
    const qv = await embedQuery(question);
    sources = state.index.rows
      .map((r) => {
        let score = 0;
        for (let i = 0; i < r.vector.length; i++) score += r.vector[i] * qv[i];
        return { ...r, score };
      })
      .sort((a, b) => b.score - a.score)
      .slice(0, 5);
  }

  const context = sources.map((s) => `- ${s.label}: ${s.text}`).join('\n');
  const messages = [
    { role: 'system', content:
      'You answer questions about a specific knowledge corpus. Ground every answer in the ' +
      'provided corpus nodes; if they do not contain the answer, say so plainly.\n\n' +
      `Corpus nodes:\n${context || '(no vector index available)'}` },
    { role: 'user', content: question },
  ];

  let text = '';
  const chunks = await state.chat.engine.chat.completions.create({ messages, stream: true });
  for await (const chunk of chunks) {
    text += chunk.choices[0]?.delta?.content ?? '';
    answer.textContent = text;
  }

  if (sources.length) {
    const row = el('div', 'sources');
    for (const s of sources) {
      const chip = el('span', 'source-chip', s.label);
      const id = /corpus\/(.+)\.yml$/.exec(s.path)?.[1];
      if (id) chip.addEventListener('click', () => openDetail(id));
      row.append(chip);
    }
    answer.append(row);
  }
}

boot();
