// MCE Finnish NLP Demo — app.js
// Loads the WASM module and dictionary, wires up the UI.

import init, { MceEngine } from './pkg/mce_wasm.js';

const $ = (sel) => document.querySelector(sel);
const statusEl = $('#status');
const resultsEl = $('#results');
const timingEl = $('#timing');
const inputEl = $('#input');
const buttons = [
  $('#btn-analyze'),
  $('#btn-spell'),
  $('#btn-compound'),
  $('#btn-baseform'),
  $('#btn-raw'),
];

let engine = null;

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

async function bootstrap() {
  try {
    statusEl.textContent = 'Initializing WASM module...';

    await init();

    statusEl.textContent = 'Loading dictionary (mor.vfst)...';

    const resp = await fetch('mor.vfst');
    if (!resp.ok) {
      throw new Error(
        `Failed to load mor.vfst (${resp.status}). ` +
        'Copy mor.vfst from voikko-fi/vvfst/ into the demo/ directory.'
      );
    }

    const dictBytes = new Uint8Array(await resp.arrayBuffer());
    const sizeKB = (dictBytes.byteLength / 1024).toFixed(0);
    statusEl.textContent = `Loading engine (dictionary: ${sizeKB} KB)...`;

    engine = MceEngine.load(dictBytes);

    // Show version.
    const version = MceEngine.version();
    $('#version').textContent = version;

    statusEl.textContent = `Ready. Dictionary loaded (${sizeKB} KB). MCE v${version}`;
    statusEl.className = 'ready';

    buttons.forEach((b) => (b.disabled = false));
  } catch (err) {
    statusEl.textContent = `Error: ${err.message}`;
    statusEl.className = 'error';
    console.error('MCE bootstrap error:', err);
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function getInput() {
  return inputEl.value.trim();
}

function timed(fn) {
  const t0 = performance.now();
  const result = fn();
  const elapsed = (performance.now() - t0).toFixed(2);
  timingEl.textContent = `${elapsed} ms`;
  return result;
}

function escapeHtml(s) {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

// ---------------------------------------------------------------------------
// Analyze Sentence
// ---------------------------------------------------------------------------

function doAnalyze() {
  const text = getInput();
  if (!text) { resultsEl.innerHTML = '<span class="placeholder">No input.</span>'; return; }

  const json = timed(() => engine.disambiguate_sentence(text));
  const data = JSON.parse(json);

  if (data.length === 0) {
    resultsEl.innerHTML = '<span class="placeholder">No tokens found.</span>';
    return;
  }

  let html = '<table><thead><tr><th>Word</th><th>POS</th><th>Baseform</th></tr></thead><tbody>';
  for (const item of data) {
    const word = escapeHtml(item.word);
    const pos = item.pos ? `<span class="tag">${escapeHtml(item.pos)}</span>` : '<span class="tag">?</span>';
    const bf = item.baseform ? escapeHtml(item.baseform) : '-';
    html += `<tr><td class="word-cell">${word}</td><td>${pos}</td><td>${bf}</td></tr>`;
  }
  html += '</tbody></table>';
  resultsEl.innerHTML = html;
}

// ---------------------------------------------------------------------------
// Spell Check
// ---------------------------------------------------------------------------

function doSpellCheck() {
  const text = getInput();
  if (!text) { resultsEl.innerHTML = '<span class="placeholder">No input.</span>'; return; }

  const words = text.split(/\s+/).filter(Boolean);
  const results = timed(() =>
    words.map((w) => {
      // Strip trailing punctuation for spell checking.
      const clean = w.replace(/[.,!?;:]+$/, '');
      if (!clean) return { word: w, valid: true, clean };
      return { word: w, valid: engine.spell_check(clean), clean };
    })
  );

  let html = '<p>';
  for (const r of results) {
    if (r.valid) {
      html += `<span class="spell-ok">${escapeHtml(r.word)}</span> `;
    } else {
      html += `<span class="spell-bad" title="Unknown: ${escapeHtml(r.clean)}">${escapeHtml(r.word)}</span> `;
    }
  }
  html += '</p>';

  const okCount = results.filter((r) => r.valid).length;
  const badCount = results.length - okCount;
  html += `<p style="margin-top:0.75rem;color:var(--text-muted);font-size:0.85rem;">`;
  html += `${okCount} valid, ${badCount} unknown out of ${results.length} words.`;
  html += `</p>`;

  resultsEl.innerHTML = html;
}

// ---------------------------------------------------------------------------
// Compound Split
// ---------------------------------------------------------------------------

function doCompoundSplit() {
  const text = getInput();
  if (!text) { resultsEl.innerHTML = '<span class="placeholder">No input.</span>'; return; }

  const words = text.split(/\s+/).filter(Boolean).map((w) => w.replace(/[.,!?;:]+$/, '')).filter(Boolean);

  const results = timed(() =>
    words.map((w) => {
      const json = engine.compound_split(w);
      const splits = JSON.parse(json);
      return { word: w, splits };
    })
  );

  let html = '<table><thead><tr><th>Word</th><th>Compound Parts</th><th>Penalty</th></tr></thead><tbody>';
  for (const r of results) {
    const word = escapeHtml(r.word);
    if (r.splits.length === 0) {
      html += `<tr><td class="word-cell">${word}</td><td><span class="tag">not compound</span></td><td>-</td></tr>`;
    } else {
      const best = r.splits[0];
      let partsHtml = '<div class="compound-parts">';
      for (let i = 0; i < best.parts.length; i++) {
        if (i > 0) partsHtml += '<span class="compound-sep">+</span>';
        const cls = best.parts[i].is_linking ? 'compound-part compound-linking' : 'compound-part';
        partsHtml += `<span class="${cls}">${escapeHtml(best.parts[i].surface)}</span>`;
      }
      partsHtml += '</div>';
      html += `<tr><td class="word-cell">${word}</td><td>${partsHtml}</td><td>${best.penalty}</td></tr>`;
    }
  }
  html += '</tbody></table>';
  resultsEl.innerHTML = html;
}

// ---------------------------------------------------------------------------
// Baseforms
// ---------------------------------------------------------------------------

function doBaseforms() {
  const text = getInput();
  if (!text) { resultsEl.innerHTML = '<span class="placeholder">No input.</span>'; return; }

  const words = text.split(/\s+/).filter(Boolean).map((w) => w.replace(/[.,!?;:]+$/, '')).filter(Boolean);

  const results = timed(() =>
    words.map((w) => ({
      word: w,
      baseform: engine.get_baseform(w),
    }))
  );

  let html = '<table><thead><tr><th>Word</th><th>Baseform</th></tr></thead><tbody>';
  for (const r of results) {
    const word = escapeHtml(r.word);
    const bf = escapeHtml(r.baseform);
    const changed = r.word !== r.baseform;
    html += `<tr><td class="word-cell">${word}</td><td>${changed ? `<strong>${bf}</strong>` : bf}</td></tr>`;
  }
  html += '</tbody></table>';
  resultsEl.innerHTML = html;
}

// ---------------------------------------------------------------------------
// Raw JSON
// ---------------------------------------------------------------------------

function doRawJson() {
  const text = getInput();
  if (!text) { resultsEl.innerHTML = '<span class="placeholder">No input.</span>'; return; }

  const json = timed(() => engine.analyze_sentence(text));

  let formatted;
  try {
    formatted = JSON.stringify(JSON.parse(json), null, 2);
  } catch {
    formatted = json;
  }

  resultsEl.innerHTML = `<pre>${escapeHtml(formatted)}</pre>`;
}

// ---------------------------------------------------------------------------
// Event binding
// ---------------------------------------------------------------------------

$('#btn-analyze').addEventListener('click', doAnalyze);
$('#btn-spell').addEventListener('click', doSpellCheck);
$('#btn-compound').addEventListener('click', doCompoundSplit);
$('#btn-baseform').addEventListener('click', doBaseforms);
$('#btn-raw').addEventListener('click', doRawJson);

// Allow Ctrl+Enter to run analysis.
inputEl.addEventListener('keydown', (e) => {
  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
    e.preventDefault();
    if (engine) doAnalyze();
  }
});

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------

bootstrap();
