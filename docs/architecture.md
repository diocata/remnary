# Rust foundation and storage decision

Status: provisional implementation choice for the first milestone. The supplied product brief establishes the product vision; this document narrows the starting implementation and replaces its proposed TypeScript stack with Rust.

## Decision

Build one Rust package with small modules: CLI, normalized models, ChatGPT import, SQLite store and Markdown export. Split into crates only when there is a concrete reason.

Use SQLite as the canonical source of normalized conversation evidence. Produce Markdown as an export. Markdown remains a good format for human knowledge; source history requires stable IDs, transactional imports, revision history, querying and eventually evidence-to-observation relationships.

SQLite is already a lightweight local storage solution. Markdown-only storage would trade a mature embedded engine for our own locking, recovery, indexing and referential-integrity implementation. JSON/JSONL can be added as a portable interchange format without making it a second source of truth.

```text
Explicitly selected ChatGPT export (JSON / ZIP)
    → validate + normalize the conversation tree
    → fingerprint + transactionally import
    → embedded SQLite (source revisions + messages + FTS5)
    → CLI search / immutable Markdown snapshots
```

SQLite's application-file rationale: https://www.sqlite.org/appfileformat.html

Rust embedded integration: https://github.com/rusqlite/rusqlite

## Data integrity

Conversation identity is `(provider, source_id)`, represented by a deterministic SHA-256 ID. Message identity is scoped to `(conversation_id, revision, source_id)`. A conversation content fingerprint covers normalized messages and retained metadata, excluding import time.

Reimporting an unchanged current snapshot makes no new revision. Changed source content appends a revision and updates the active pointer in the same transaction. Earlier source revisions remain available in SQLite, so future findings can cite exact evidence without silently mutating their source. Reimporting an older export can make that content current again; recency reconciliation needs explicit design before continuous collection.

An import batch is atomic. Unique constraints prevent duplicate message identities; foreign keys and FTS triggers are enabled. Schema v1 initializes transactionally, and unsupported database versions are rejected. Future changes will add sequential transactional migrations rather than rebuilding databases implicitly.

All source branches are kept. Parent references describe the tree; display order is not a claim that alternative assistant replies happened sequentially. Provider Unix timestamps stay as optional UTC epoch seconds. The importer preserves original message metadata, including unsupported content descriptors, but does not fetch referenced assets. Conversation metadata retains mapping nodes with the full message objects stored separately, preserving null roots, intermediate nodes and arbitrary node fields without duplicating message bodies.

The store uses SQLite's default rollback journal, with a bounded lock wait. Back up a closed store for this initial CLI; if concurrent background writers are added, introduce an online backup path. WAL is not needed for the current single-command workflow.

## Markdown ownership

Export creates content-addressed snapshot filenames, so newer source revisions do not overwrite older notes. Each source message has a deterministic anchor and displays its source identity and parent relationship. Exports are evidence transcripts, with source text in literal fenced blocks, so source headings, image links and wiki links remain text.

An existing identical file is unchanged. A different file at the expected path is a conflict: preserve it and report a nonzero exit status. Exports never ingest edits or remove old snapshots. Interrupted writes can leave partial files; subsequent exports report those as conflicts rather than silently overwriting them. Full Obsidian synchronization will need manifests, atomic file updates, user-owned sections, correction import and deletion propagation.

## Next milestone: prove useful discoveries

1. Add a local model adapter with validated structured output and an explicit `analyze` command.
2. Extract a small set of candidate corrections and explicit preferences, each citing a source message **revision** and verified excerpt. Keep assistant suggestions distinct from user statements.
3. Group repeated corrections across distinct conversations and expose them through `discover` with evidence.
4. Add accept/reject/correct feedback and respect that feedback during reanalysis.
5. Generate a small insight report in Markdown, keeping generated content separate from user edits.

Evaluate a synthetic labeled dataset and a user-selected real corpus before expanding into daemon collection or a dashboard. Do not make psychological claims or promote inferred observations to established facts.

## Before broader personal-data use

Implement selective deletion across retained source revisions, FTS and exported projections; define encryption/key management and secret handling; test importer compatibility with real exports. SQLite and Markdown are plaintext by default, so local-first storage alone does not satisfy the brief's encryption requirement.

Keep runtime networking opt-in when model integration arrives. No network client or telemetry exists in the foundation.
