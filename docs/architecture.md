# Rust and embedded storage

Observr is one Rust crate with direct SQL through `rusqlite`. SQLCipher bundles the encrypted SQLite engine; the macOS build uses Apple CommonCrypto. There is no database service, ORM, async runtime or mandatory model service.

```text
Selected local history → bounded normalization → atomic import / immutable revisions
  → SQLCipher: messages, FTS5, analysis, observations, evidence, feedback
     → review CLI / authenticated loopback dashboard
     → explicitly generated Markdown vault
```

SQLite handles frequent writes, foreign keys, transactions and full-text search. Markdown is a human-readable projection. JSON/JSONL is interchange/source input. DuckDB could query a future analytical export without becoming a second canonical store; adding it to ingestion currently adds packaging and consistency work.

Conversation identity hashes `(provider, source_id)`; messages are scoped to `(conversation, revision, source_message_id)`. Fingerprints exclude import time. Changed input adds a revision; identical input does nothing; an older known update timestamp cannot replace a newer one. Missing times stay unknown. Branch relationships are preserved without implying a linear conversation. Original source metadata remains encrypted with normalized records.

Schema version 5 follows transactional version-1/2/3/4 migrations. Foreign/newer/corrupt databases are rejected; empty interrupted initialization is recoverable. Foreign keys, bounded locking and rollback journaling protect canonical writes. Encrypted connections apply keys before schema reads; temporary SQL storage uses memory. Online encrypted backup validates integrity and publishes a private file without overwriting an existing destination.

## Analysis and review

`analysis` checkpoints each source revision and processor/model/prompt version. Adapters receive the same best-effort redaction boundary with raw metadata removed. Strict output/evidence validation rejects missing fields, invented quotes, assistant-origin facts and disallowed sensitive inferences. Rules are conservative and incomplete; Ollama communicates only with a separately trusted loopback daemon. A quote proves provenance, not semantic truth.

Deterministic observation identities preserve context, negation, kind and event type. Groups count distinct conversations and spans; revisions do not count as additional conversations. Feedback has durable history, authoritative display corrections and explicit lifecycle states. No automatic inference becomes a confirmed profile fact. Historical decision events do not imply the current choice.

Reports resolve evidence against current, included source revisions. Daily digests use IANA timezones and explicitly report unknown times. Persistent active-run heads select the latest completed processor/prompt version per source revision/engine/model. Automatic evidence from replaced runs retires; grounded human review remains authoritative. Distinct selected engines/models can contribute candidates concurrently. Heads survive VACUUM and backup; old payloads remain history. Replaced projections enter durable cleanup.

Local-model extraction uses user-only windows bounded to 8 KiB of text, with UTF-8-safe overlapping fragments for long messages. Window sizing additionally accounts for the complete serialized prompt/data envelope, including quote choices and escaped source identifiers. A conversation is limited to 256 windows and commits only after every window validates. Windows preserve original source IDs/revisions; citations are checked against both the submitted window and original source. Omitted context stays unknown. No-user conversations checkpoint an empty result without model calls. Each request has a 24,000-byte prompt/data cap, a 32K context request, a 4K completion cap (8K total for compatible reasoning families; the evaluated Qwen family uses non-thinking generation) and existing HTTP/response time/size bounds.

## Files and deletion

Transcripts use content-addressed filenames and stable message anchors. Source Markdown renders literally. Managed sections have body/whole-file hashes; unowned files and edited generated bodies are preserved as conflicts. User-authored bytes outside owned sections survive regeneration.

Persistent exclusion rules and manual exclusions feed a single effective-source view, used by all evidence/read paths. Rules also filter incoming normalized conversations before persistence. Literal topic/path semantics are documented in [privacy controls](privacy.md). Adding a rule atomically enqueues projection cleanup; removing one cannot undo independent rules/manual exclusions.

An advisory projection lock serializes publication with feedback, exclusion and deletion. Managed writes reserve destination/staging paths and dependencies in SQLite before writing. Private stages are synchronized and safely published. A durable cleanup outbox bridges transactions and filesystem changes. Cleanup removes owned generated material, preserves outside notes and retains conflicts for retry. Tombstones prevent silent resurrection.

Exports remain plaintext by explicit user choice. Original sources, independent copies, historical backups, operating-system memory and forensic erasure are outside canonical deletion's guarantees. Restore clears projection ownership so recovery cannot delete the original archive's external files.

The dashboard binds loopback, uses a random bearer token, checks Host/Origin and mutation types, serves a restrictive CSP and renders through text nodes. No telemetry, cloud account, remote extraction or watcher is implemented. Same-user malware and a hostile local model daemon are outside the protection boundary. See [sources](sources.md), [security](../SECURITY.md) and [release evidence](readiness.md).


## Canonical identity reuse

Producer v15 compares a new verified claim with a bounded catalog of existing identities. Retrieval prioritizes overlapping citations and shared claim words, includes same-kind alternatives and caps comparisons at twelve roots per candidate. Catalog identities are possible labels, never evidence; identifying text recognized by the redactor excludes a root from model matching. Review text and status are not sent to the model. Free-form owner-corrected roots are excluded from semantic reuse because their old automatic identity may no longer describe the correction; unresolved overlaps retain the conservative guard. An exact deletion tombstone is checked before matching so alias selection cannot bypass it.

A single unambiguous semantic equivalent may reuse the existing complete canonical identity and automatic summary. Kind, explicit/inferred status, decision stage and the presence/absence of scope must remain identical. A model cannot merge an unqualified identity with a scoped one. The reused claim then passes the independent entailment check using only the new window's exact user quotations. If reuse fails this check, the original separately verified claim remains separate. Multiple equivalent proposals and uncertain matches do not merge. This is conservative bounded retrieval, not exhaustive entity resolution.

A reviewed overlapping citation can also contain a distinct fact. Only an explicit pairwise distinct verdict permits that new claim through the overlap guard. The receipt hashes canonical identities, evidence, feedback history, deletion tombstones, source revisions/exclusions and active analysis heads; the write transaction rechecks it. A change while matching leaves the source pending. Existing review status, corrections and history remain authoritative. There are no alias chains or rewritten source messages. Recurrence counts only actual distinct included conversations; semantic matching never confirms a profile fact.


Verification first checks claims against quotations alone. Only surviving claims reach a separate check with matching source-message context, when a citation omits surrounding text. Context cannot rescue a detail rejected in the first pass. Explicit preference candidates additionally need a recognizable first-person preference declaration within a citation before semantic verification. This conservative gate currently recognizes common English/Portuguese forms; it can omit valid preferences expressed in other ways or languages. A request, question, budget constraint or speculative belief alone does not satisfy it. Model verdicts and these safeguards still require real quality assessment.
