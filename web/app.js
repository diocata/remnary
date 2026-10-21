'use strict';

// Archive and model strings enter the document only through textContent.
// No HTML injection, external requests, remote fonts, or persistent cookies.
const $ = (id) => document.getElementById(id);
const state = { view: 'discoveries', observations: [], sources: [], selected: null, filter: '', kind: 'all', busy: false };
const labels = {
  explicit_preference: 'Preference', inferred_preference: 'Inferred preference', correction: 'Correction', decision: 'Decision',
  topic: 'Topic', project_reference: 'Project reference', behavioral_pattern: 'Behavioral pattern',
  candidate: 'Needs review', supported: 'Recurring evidence', user_confirmed: 'Confirmed by you', user_rejected: 'Rejected', dismissed: 'Dismissed', superseded: 'Superseded',
  proposal: 'Proposed', consideration: 'Considered', reconsideration: 'Reconsidered', reversal: 'Reversed', confirmation: 'Reconfirmed', unknown_current_state: 'Current state unknown'
};
const views = {
  discoveries: ['Your evidence workspace', 'A little more self-knowledge.', 'Follow a finding back to the words that support it. You decide what stays.', 'Discoveries'],
  profile: ['Your confirmed profile', 'In your own terms.', 'Only findings you have confirmed appear here. Their original context stays attached.', 'Confirmed by you'],
  timeline: ['Your decision trail', 'See how your thinking moved.', 'Decisions are events in context. An old choice does not establish your current position.', 'Decision timeline'],
  search: ['Your conversation archive', 'Pick up the thread.', 'Find the words, choices, and questions you want to return to.', 'Search'],
  sources: ['Your source collection', 'Where every trail begins.', 'A readable archive of the conversations behind your discoveries.', 'Sources']
};

let token = '';
try { token = sessionStorage.getItem('observr-token') || ''; } catch (_) { /* Memory-only authentication is still available. */ }
const fragment = new URLSearchParams(location.hash.slice(1));
const freshToken = fragment.get('token');
if (freshToken && /^[a-f0-9]{64}$/.test(freshToken)) {
  token = freshToken;
  try { sessionStorage.setItem('observr-token', token); } catch (_) { /* Do not require browser storage. */ }
}
if (location.hash) history.replaceState(null, '', location.pathname);

function node(tag, className, text) {
  const element = document.createElement(tag);
  if (className) element.className = className;
  if (text !== undefined && text !== null) element.textContent = String(text);
  return element;
}

function button(text, className, action) {
  const element = node('button', className, text);
  element.type = 'button';
  element.addEventListener('click', action);
  return element;
}

function human(value) { return labels[value] || String(value || '').replaceAll('_', ' '); }
function shortId(value) { return String(value || '').slice(0, 12); }
function displayText(record) { return record.user_value || record.observation.summary; }
function sourceFor(id) { return state.sources.find((source) => source.id === id); }
function date(value) {
  if (typeof value !== 'number' || !Number.isFinite(value)) return 'Date unknown';
  const parsed = new Date(value * 1000);
  return Number.isNaN(parsed.getTime()) ? 'Date unknown' : parsed.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
}

async function api(path, options = {}) {
  if (!token) throw new Error('Open the private dashboard link printed in your terminal to connect to this archive.');
  const response = await fetch(path, {
    ...options,
    cache: 'no-store', credentials: 'omit', redirect: 'error', referrerPolicy: 'no-referrer',
    headers: { Authorization: `Bearer ${token}`, ...(options.body ? { 'Content-Type': 'application/json' } : {}), ...options.headers }
  });
  if (response.status === 401) {
    try { sessionStorage.removeItem('observr-token'); } catch (_) { /* Nothing is retained outside this tab. */ }
    throw new Error('This private link has expired. Open the dashboard link from the current Observr process in your terminal.');
  }
  let body;
  try { body = await response.json(); } catch (_) { throw new Error('The local dashboard returned an unreadable response. Refresh and try again.'); }
  if (!response.ok) throw new Error(typeof body.error === 'string' ? body.error : 'The local archive could not complete this action.');
  return body;
}

function showError(error) {
  $('connection-error').textContent = error instanceof TypeError ? 'The local dashboard is unavailable. Check that Observr is still running, then refresh.' : error.message;
  $('connection-error').hidden = false;
}

let announcementTimer;
function announce(text) {
  clearTimeout(announcementTimer);
  $('announcement').textContent = text;
  $('announcement').hidden = false;
  announcementTimer = setTimeout(() => { $('announcement').hidden = true; }, 4500);
}

async function refresh() {
  $('refresh').disabled = true;
  $('connection-error').hidden = true;
  try {
    const [status, records, sources] = await Promise.all([
      api('/api/status'), api(`/api/observations?hidden=${$('include-hidden').checked ? '1' : '0'}`), api('/api/sources')
    ]);
    state.observations = records.observations;
    state.sources = sources.sources;
    $('archive-status').textContent = `${status.conversations} conversations · ${status.messages} messages · ${status.revisions} saved revisions`;
    render();
  } catch (error) { showError(error); }
  finally { $('refresh').disabled = false; }
}

function selectView(view) {
  state.view = view;
  state.filter = '';
  state.kind = 'all';
  $('filter').value = '';
  $('kind-filter').value = 'all';
  document.querySelectorAll('[data-view]').forEach((item) => {
    const active = item.dataset.view === view;
    item.classList.toggle('active', active);
    if (active) item.setAttribute('aria-current', 'page'); else item.removeAttribute('aria-current');
  });
  const [eyebrow, title, description, heading] = views[view];
  $('page-eyebrow').textContent = eyebrow;
  $('page-title').textContent = title;
  $('page-description').textContent = description;
  $('list-heading').textContent = heading;
  $('list-context').textContent = view === 'profile' ? 'Your confirmed wording' : view === 'timeline' ? 'Events, with context' : 'Yours to review';
  $('review-view').hidden = view === 'search' || view === 'sources';
  $('search-view').hidden = view !== 'search';
  $('sources-view').hidden = view !== 'sources';
  $('kind-filter').hidden = view === 'timeline';
  render();
  if (view === 'search') $('search-query').focus();
}

function filtered() {
  let records = state.observations;
  if (state.view === 'profile') records = records.filter((record) => record.status === 'user_confirmed');
  if (state.view === 'timeline') records = records.filter((record) => record.observation.kind === 'decision');
  if (state.kind !== 'all') records = records.filter((record) => state.kind === 'preference' ? record.observation.kind.includes('preference') : record.observation.kind === state.kind);
  const filter = state.filter.trim().toLocaleLowerCase();
  if (filter) records = records.filter((record) => [displayText(record), record.observation.topic, record.observation.context || '', record.observation.canonical_value].join(' ').toLocaleLowerCase().includes(filter));
  // Preserve the server's discovery order across filters and list lengths.
  if (state.view !== 'timeline') return records;
  return [...records].sort((a, b) => {
    const knownA = Number.isFinite(a.last_seen), knownB = Number.isFinite(b.last_seen);
    return Number(!knownA) - Number(!knownB) || (knownA && knownB ? b.last_seen - a.last_seen : 0) || a.id.localeCompare(b.id);
  });
}

function empty(title, description) {
  const section = node('div', 'empty-state');
  section.append(node('div', 'empty-symbol', '⌁'), node('h3', 'empty-title', title), node('p', 'empty-message', description));
  return section;
}

function render() {
  if (state.view === 'sources') { renderSources(); return; }
  if (state.view === 'search') return;
  const records = filtered();
  $('result-count').textContent = records.length;
  if (!records.some((record) => record.id === state.selected)) state.selected = records[0]?.id || null;
  const list = $('observation-list');
  const previousScroll = list.scrollTop;
  list.replaceChildren();
  if (!records.length) {
    const title = state.view === 'profile' ? 'Make it yours.' : state.view === 'timeline' ? 'A trail will take shape.' : state.filter || state.kind !== 'all' ? 'No matching findings.' : 'Your discoveries start here.';
    const description = state.view === 'profile' ? 'Confirm a finding in Discoveries to include it in your profile.' : state.view === 'timeline' ? 'Decisions found in your conversations will appear here with their original evidence.' : state.filter || state.kind !== 'all' ? 'Try another topic or clear your filters.' : 'Import conversations and run analysis, then refresh this workspace.';
    list.append(empty(title, description));
  }
  records.forEach((record) => {
    const row = button('', `observation-row${record.id === state.selected ? ' selected' : ''}`, () => {
      state.selected = record.id;
      render();
      [...$('observation-list').querySelectorAll('button')].find((item) => item.dataset.id === record.id)?.focus({ preventScroll: true });
    });
    row.dataset.id = record.id;
    row.setAttribute('aria-pressed', String(record.id === state.selected));
    const topline = node('div', 'observation-topline');
    topline.append(node('span', 'kind-label', state.view === 'timeline' ? human(record.observation.decision_event) : human(record.observation.kind)), node('span', `status-dot${record.status === 'user_confirmed' ? ' confirmed' : ''}`, human(record.status)));
    const footer = node('div', 'row-footer');
    footer.append(node('span', 'row-context', state.view === 'timeline' ? date(record.last_seen) : record.observation.context || human(record.observation.topic)), node('span', 'row-source-count', `${record.distinct_conversations} source${record.distinct_conversations === 1 ? '' : 's'} ↗`));
    row.append(topline, node('p', 'row-summary', displayText(record)), footer);
    list.append(row);
  });
  list.scrollTop = previousScroll;
  renderEvidence(records.find((record) => record.id === state.selected));
}

function renderEvidence(record) {
  const panel = $('evidence-panel');
  panel.replaceChildren();
  if (!record) {
    panel.append(empty('Every finding has a trail.', 'Select a discovery to read the words behind it, then confirm, correct, or set it aside.'));
    return;
  }
  panel.append(node('p', 'detail-eyebrow', 'Finding → evidence'), node('h2', 'detail-title', displayText(record)));
  const meta = node('div', 'detail-meta');
  meta.append(node('span', `badge${record.status === 'user_confirmed' ? ' confirmed' : ''}`, human(record.status)), node('span', `badge${record.observation.inference === 'inferred' ? ' inferred' : ''}`, record.observation.inference === 'inferred' ? 'Tentative inference' : 'Explicit evidence'));
  if (record.observation.decision_event) meta.append(node('span', 'badge', human(record.observation.decision_event)));
  panel.append(meta);
  if (record.observation.context) panel.append(node('p', 'detail-context', `Context: ${record.observation.context}`));
  const heading = node('div', 'evidence-heading');
  heading.append(node('h3', '', 'The original words'), node('span', '', `${record.occurrences} occurrence${record.occurrences === 1 ? '' : 's'}`));
  panel.append(heading);
  if (Object.keys(record.conversation_origins || {}).length) panel.append(node('p', 'detail-context', 'Copied history appears in multiple source records and is counted once.'));
  record.observation.evidence.forEach((evidence) => {
    const item = node('article', 'evidence-item');
    item.append(node('blockquote', 'quote', evidence.quote));
    const reference = node('p', 'evidence-reference');
    reference.append(node('span', 'evidence-source-title', sourceFor(evidence.conversation_id)?.title || 'Untitled conversation'), document.createTextNode(`${evidence.provider} · revision ${evidence.revision} · message ${shortId(evidence.source_message_id)}`));
    item.append(reference, button('Open this source revision ↗', 'source-link', () => openSource(evidence.conversation_id, evidence.revision, evidence.source_message_id)));
    panel.append(item);
  });
  const actions = node('div', 'review-actions');
  actions.append(button('✓ Confirm', 'button button-primary', () => feedback(record, 'confirm')), button('Correct wording', 'button', () => correct(record)), button('Reject', 'button button-danger', () => feedback(record, 'reject')), button('Dismiss', 'button button-quiet', () => feedback(record, 'dismiss')));
  if (['user_confirmed', 'user_rejected', 'dismissed', 'superseded'].includes(record.status)) actions.append(button('Undo review', 'button button-quiet', () => feedback(record, 'reset')));
  panel.append(actions, node('p', 'review-note', record.observation.inference === 'inferred' ? 'This is an interpretation, not a fact about you. Confirm it only if the context and wording fit.' : 'Confirm adds this finding to your profile. Dismiss sets it aside; reject records that it is wrong.'));
}

async function feedback(record, action, value) {
  if (state.busy) return;
  state.busy = true;
  document.querySelectorAll('.review-actions button').forEach((item) => { item.disabled = true; });
  try {
    await api('/api/feedback', { method: 'POST', body: JSON.stringify({ observation_id: record.id, action, ...(value !== undefined ? { value } : {}) }) });
    if (action === 'correct') $('correction-dialog').close();
    await refresh();
    announce({ confirm: 'Confirmed. This finding is now in your profile.', correct: 'Correction saved and confirmed.', reject: 'Rejected. Your review will survive future analysis.', dismiss: 'Dismissed. Use “Show reviewed items” to revisit it.', reset: 'Review undone. This finding is ready to review again.' }[action]);
  } catch (error) {
    if (action === 'correct') { $('correction-error').textContent = error.message; $('correction-error').hidden = false; }
    else showError(error);
  }
  finally { state.busy = false; document.querySelectorAll('.review-actions button').forEach((item) => { item.disabled = false; }); }
}

let correctionRecord;
function correct(record) {
  correctionRecord = record;
  $('correction-value').value = displayText(record);
  $('correction-error').hidden = true;
  $('correction-dialog').showModal();
  $('correction-value').focus();
}

async function openSource(id, revision, messageId) {
  try {
    const params = new URLSearchParams({ conversation_id: id });
    if (revision) params.set('revision', revision);
    const source = await api(`/api/source?${params}`);
    $('source-title').textContent = source.title || 'Untitled conversation';
    $('source-meta').textContent = `${source.provider} · revision ${revision || source.revision || 'current'}${source.partial_view ? ' · Partial history view; messages or context may be missing' : ''}`;
    $('source-messages').replaceChildren();
    source.messages.forEach((message) => {
      const item = node('article', `source-message${message.source_id === messageId ? ' highlighted' : ''}`);
      const heading = node('div', 'source-message-header');
      heading.append(node('strong', '', message.role), document.createTextNode(`${date(message.created_at)} · ${message.source_id}`));
      if (message.truncated) heading.append(document.createTextNode(' · Truncated source text'));
      item.append(heading, node('pre', '', message.content || '(No text content)'));
      if (message.source_id === messageId) item.dataset.highlighted = 'true';
      $('source-messages').append(item);
    });
    $('source-dialog').showModal();
    $('source-messages').querySelector('[data-highlighted]')?.scrollIntoView({ block: 'center', behavior: 'instant' });
  } catch (error) { showError(error); }
}

function renderSources() {
  $('source-count').textContent = state.sources.length;
  const container = $('sources-list');
  container.replaceChildren();
  if (!state.sources.length) { container.append(empty('Bring your conversations together.', 'Import an AI conversation export from the command line, then refresh your archive.')); return; }
  [...state.sources].sort((a, b) => (b.updated_at || b.created_at || 0) - (a.updated_at || a.created_at || 0)).forEach((source) => {
    const row = node('article', 'source-row');
    const information = node('div');
    information.append(node('h3', '', source.title || 'Untitled conversation'), node('p', '', `${source.provider} · ${source.messages} messages · revision ${source.revision} · ${date(source.updated_at || source.created_at)}${source.partial_view ? ' · Partial history view' : ''}`));
    row.append(information, button('Read source ↗', 'button button-quiet', () => openSource(source.id, source.revision)));
    container.append(row);
  });
}

$('navigation').addEventListener('click', (event) => { const target = event.target.closest('[data-view]'); if (target) selectView(target.dataset.view); });
$('refresh').addEventListener('click', refresh);
$('include-hidden').addEventListener('change', refresh);
$('filter').addEventListener('input', (event) => { state.filter = event.target.value; render(); });
$('kind-filter').addEventListener('change', (event) => { state.kind = event.target.value; render(); });
$('cancel-correction').addEventListener('click', () => $('correction-dialog').close());
$('close-source').addEventListener('click', () => $('source-dialog').close());
$('correction-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const value = $('correction-value').value.trim();
  if (!value || !correctionRecord) return;
  if (new TextEncoder().encode(value).length > 16384) {
    $('correction-error').textContent = 'This correction is too long. Keep it under 16 KB.';
    $('correction-error').hidden = false;
    return;
  }
  $('save-correction').disabled = true;
  try { await feedback(correctionRecord, 'correct', value); } finally { $('save-correction').disabled = false; }
});
$('search-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const query = $('search-query').value.trim();
  if (!query) return;
  const submit = $('search-form').querySelector('button');
  submit.disabled = true;
  $('connection-error').hidden = true;
  try {
    const { hits } = await api(`/api/search?${new URLSearchParams({ q: query })}`);
    $('search-results').replaceChildren();
    if (!hits.length) $('search-results').append(empty('No matching words.', 'Try fewer words or another phrase from the conversation.'));
    hits.forEach((hit) => {
      const item = node('article', 'search-hit');
      item.append(node('h3', '', hit.title || 'Untitled conversation'), node('p', '', hit.excerpt), node('p', 'evidence-reference', `${hit.provider} · ${hit.role} · revision ${hit.revision} · message ${shortId(hit.message_id)}`));
      item.append(button('Open this source revision ↗', 'source-link', () => openSource(hit.conversation_id, hit.revision, hit.message_id)));
      $('search-results').append(item);
    });
    announce(`${hits.length} matching message${hits.length === 1 ? '' : 's'} found.`);
  } catch (error) { showError(error); }
  finally { submit.disabled = false; }
});

selectView('discoveries');
refresh();
