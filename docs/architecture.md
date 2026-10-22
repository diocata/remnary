# Rust and embedded storage

Remnary is one Rust crate with direct SQL through `rusqlite`. SQLCipher bundles the encrypted SQLite engine; the macOS build uses Apple CommonCrypto. There is no database service, ORM or mandatory model service. Application code uses blocking APIs; the HTTP client brings a transitive Tokio runtime.

```text
Selected local history → bounded normalization → atomic import / immutable revisions
  → SQLCipher: messages, FTS5, analysis, observations, evidence, feedback
     → review CLI / authenticated loopback dashboard
     → explicitly generated Markdown vault
```

SQLite handles frequent writes, foreign keys, transactions and full-text search. Markdown is a human-readable projection. JSON/JSONL is interchange/source input. DuckDB could query a future analytical export without becoming a second canonical store; adding it to ingestion currently adds packaging and consistency work.

## Source map

The project stays one crate. Folders group code by the responsibility a maintainer needs to change:

```text
src/
  main.rs                 CLI entry point and exit status
  cli/                    Argument definitions, command dispatch, terminal output
  sources/                Bounded file discovery and provider normalization
    chatgpt/              ChatGPT JSON/ZIP parsing
    claude.rs             Claude Code records
    codex.rs              Codex rollout records
    kiro.rs               Kiro chat records
    app_snapshot.rs       Explicit partial app-history imports
    origins.rs            Copied-dialogue origin accounting
  storage/                SQLCipher connection, revisions and search
    observations.rs       Analysis checkpoints, evidence and review state
    privacy/              Deletion tombstones and durable cleanup
    projections.rs        Generated-file ownership and locking
    operations.rs         Integrity checks, encrypted backup and restore
    security.rs           Recovery keys and native credential stores
    exclusions.rs         Source exclusion rules
    schema.sql
    migrations/           Existing transactional schema upgrades
  analysis/               Incremental rules/model orchestration
    intelligence/         Observation contracts and evidence validation
      rules.rs            Deterministic extraction and owner attribution
    ollama/               Bounded local-model transport and structured output
    verification/         Independent claim verification
    reconciliation/       Conservative identity reuse
    batches.rs            Bounded analysis windows
    redaction.rs          Shared secret/contact redaction boundary
  reports/                Ranked discoveries, daily reports and topic views
  projections/            Managed knowledge notes, transcripts and feedback
  web/                    Loopback server/authentication and API routes
  model.rs                Normalized conversation/message types and hashes
```

Large unit suites live beside their module in `tests.rs` or a named test module. The top-level `tests/` directory exercises the public library and actual CLI. Test fixtures and embedded web assets resolve from `CARGO_MANIFEST_DIR`, so moving a Rust module cannot silently select a different input.

`lib.rs` re-exports the original public paths, such as `remnary::store`, `remnary::local_sources` and `remnary::intelligence`. These are aliases to the grouped implementations, not duplicate code. Existing callers keep working. `ObservationStatus::is_visible` defines the shared visibility rule for storage, reports, topic groups and the API; feedback transitions and SQL persistence remain in storage.

Keep a change in its owning module. Share an existing rule when multiple views need it; do not add services, traits, configuration or dependencies merely to separate files. Keep provider-specific attribution in its adapter and common provenance checks in analysis. Historical validation documents may name the earlier flat source paths.

Conversation identity hashes `(provider, source_id)`; messages are scoped to `(conversation, revision, source_message_id)`. Fingerprints exclude import time. Changed input adds a revision; identical input does nothing; an older known update timestamp cannot replace a newer one. Missing times stay unknown. Branch relationships are preserved without implying a linear conversation. Original source metadata remains encrypted with normalized records.

Schema version 5 follows transactional version-1/2/3/4 migrations. Foreign/newer/corrupt databases are rejected; empty interrupted initialization is recoverable. Foreign keys, bounded locking and rollback journaling protect canonical writes. Encrypted connections apply keys before schema reads; temporary SQL storage uses memory. Online encrypted backup validates integrity and publishes a private file without overwriting an existing destination.

## Analysis and review

`analysis` checkpoints each source revision and processor/model/prompt version. Adapters receive the same best-effort redaction boundary with raw metadata removed. Strict output/evidence validation rejects missing fields, invented quotes, assistant-origin facts and disallowed sensitive inferences. Rules are conservative and incomplete; Ollama communicates only with a separately trusted loopback daemon. A quote proves provenance, not semantic truth.

Deterministic observation identities preserve context, negation, kind and event type. Groups count distinct conversations and spans; revisions do not count as additional conversations. Feedback has durable history, authoritative display corrections and explicit lifecycle states. No automatic inference becomes a confirmed profile fact. Historical decision events do not imply the current choice.

Rules processor `explicit-rules-v7` adds a bounded comparison-request detector. Positive English comparison commands, best-option questions and noun-phrase alternatives with a use case can supply evidence; negated instructions, quoted/code material and nonowner/context frames cannot. Each source needs two distinct message IDs and distinct normalized question text. Punctuation/case-only repetition does not establish two different requests. The canonical inferred claim describes comparison requests in recorded interactions, without asserting an enduring preference, frequency or personality trait. Grouping preserves exact citations and uses existing source-origin correlation, review and deletion safeguards. The v6→v7 tag makes previously cached sources eligible for this added detector; unchanged v7 results cache normally. Ollama producer/prompt 36 is unchanged.

Related-topic threads are computed from current visible observations without new canonical tables or merged identities. Exact topic/subject labels normalize whitespace and case; different claims and contexts remain distinct. Counts deduplicate cited messages and use the same Kiro dialogue-origin correlation as claim recurrence. Dismissed/rejected/superseded and excluded/deleted evidence cannot support a thread. Free-text corrected records are omitted because the old automatic topic need not describe the correction. Threads appear at `patterns --topics`, in the dashboard and in evidence-linked managed Topics notes; they never confirm profile facts or assign a current choice. Exact-label grouping can miss related topics with different labels, and broad labels may provide little useful intelligence.

Reports resolve evidence against current, included source revisions. Daily digests use IANA timezones and explicitly report unknown times. Persistent active-run heads select the latest completed processor/prompt version per source revision/engine/model. Automatic evidence from replaced runs retires; grounded human review remains authoritative. Distinct selected engines/models can contribute candidates concurrently. Heads survive VACUUM and backup; old payloads remain history. Replaced projections enter durable cleanup.

Local-model extraction uses user-only windows bounded to 8 KiB of text, with UTF-8-safe overlapping fragments for long messages. Window sizing additionally accounts for the complete serialized prompt/data envelope, including quote choices and escaped source identifiers. A conversation is limited to 256 windows and commits only after every window validates. Windows preserve original source IDs/revisions; citations are checked against both the submitted window and original source. Omitted context stays unknown. No-user conversations checkpoint an empty result without model calls. Each request has a 24,000-byte prompt/data cap, a 32K context request, a 4K completion cap (8K total for compatible reasoning families; the evaluated Qwen family uses non-thinking generation) and existing HTTP/response time/size bounds.

## Files and deletion

Transcripts use content-addressed filenames and stable message anchors. Source Markdown renders literally. Managed sections have body/whole-file hashes; unowned files and edited generated bodies are preserved as conflicts. User-authored bytes outside owned sections survive regeneration.

Persistent exclusion rules and manual exclusions feed a single effective-source view, used by all evidence/read paths. Rules also filter incoming normalized conversations before persistence. Literal topic/path semantics are documented in [privacy controls](privacy.md). Adding a rule atomically enqueues projection cleanup; removing one cannot undo independent rules/manual exclusions.

An advisory projection lock serializes publication with feedback, exclusion and deletion. Managed writes reserve destination/staging paths and dependencies in SQLite before writing. Private stages are synchronized and safely published. A durable cleanup outbox bridges transactions and filesystem changes. Cleanup removes owned generated material, preserves outside notes and retains conflicts for retry. Tombstones prevent silent resurrection.

Exports remain plaintext by explicit user choice. Original sources, independent copies, historical backups, operating-system memory and forensic erasure are outside canonical deletion's guarantees. Restore clears projection ownership so recovery cannot delete the original archive's external files.

The dashboard binds loopback, uses a random bearer token, checks Host/Origin and mutation types, serves a restrictive CSP and renders through text nodes. No telemetry, cloud account, remote extraction or watcher is implemented. Same-user malware and a hostile local model daemon are outside the protection boundary. See [sources](sources.md), [security](../SECURITY.md) and [release evidence](readiness.md).


## Canonical identity reuse

Producer v16 compares a new verified claim with a bounded catalog of existing identities. Retrieval prioritizes overlapping citations and shared claim words, includes same-kind alternatives and caps comparisons at twelve roots per candidate. Catalog identities are possible labels, never evidence; identifying text recognized by the redactor excludes a root from model matching. Review text and status are not sent to the model. Free-form owner-corrected roots are excluded from semantic reuse because their old automatic identity may no longer describe the correction; unresolved overlaps retain the conservative guard. An exact deletion tombstone is checked before matching so alias selection cannot bypass it.

A single unambiguous semantic equivalent may reuse the existing complete canonical identity and automatic summary. Kind, explicit/inferred status, decision stage and the presence/absence of scope must remain identical. A model cannot merge an unqualified identity with a scoped one. The reused claim then passes the independent entailment check using only the new window's exact user quotations. If reuse fails this check, the original separately verified claim remains separate. Multiple equivalent proposals and uncertain matches do not merge. This is conservative bounded retrieval, not exhaustive entity resolution.

A reviewed overlapping citation can also contain a distinct fact. Only an explicit pairwise distinct verdict permits that new claim through the overlap guard. The receipt hashes canonical identities, evidence, feedback history, deletion tombstones, source revisions/exclusions and active analysis heads; the write transaction rechecks it. A change while matching leaves the source pending. Existing review status, corrections and history remain authoritative. There are no alias chains or rewritten source messages. Recurrence counts only actual distinct included conversations; semantic matching never confirms a profile fact.


Verification first checks claims against quotations alone. Only surviving claims reach a separate check with matching source-message context, when a citation omits surrounding text. Context cannot rescue a detail rejected in the first pass. Explicit preference candidates additionally need a recognizable first-person preference declaration within a citation before semantic verification. This conservative gate currently recognizes common English/Portuguese forms; it can omit valid preferences expressed in other ways or languages. A request, question, budget constraint or speculative belief alone does not satisfy it. Model verdicts and these safeguards still require real quality assessment.


Common routine file/draft/push/continuation commands are filtered across claim kinds, preventing a task omitted as a preference from reappearing as a decision proposal. Mixed statements with a separate declaration/choice remain eligible for semantic checking. This is a conservative bounded policy, not a universal intent classifier.


Reviewed identities whose original quotation still occurs in a current user message remain available to identity matching after a source append, even while their older evidence is hidden from current reports. A successful new-source proof reattaches current-revision evidence without resetting the review. Encrypted append/backup/restore and actual local-model cross-conversation lifecycle checks cover this path.

Kiro execution files can contain exact cumulative copies of earlier chats under different execution IDs. Reports retain every source and evidence link, but count an exact complete dialogue prefix with at least 256 bytes of assistant replies after a user message as one logical conversation. Native metadata, tool entries and empty streaming frames do not change dialogue identity. Message occurrence counts use the same dialogue-prefix identity, including after report serialization. Repeated user prompts, provider system text and short greetings alone do not trigger this correlation. The derived `conversation_origins` map carries that accounting through ranking and date-scoped observation reports; inherited message spans count once. This is conservative exact-prefix detection for the observed Kiro format, not universal fork or cross-provider identity resolution.
