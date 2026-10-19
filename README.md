# Observr

A local-first personal intelligence engine for AI conversation history, written in Rust.

The product direction is to discover recurring corrections, preferences and changing decisions, with every finding linked to source messages. This first foundation implements the import, storage, search and Markdown export pipeline. Intelligence extraction is the next milestone.

## Storage recommendation

Use **embedded SQLite for structured evidence and Markdown for readable knowledge**. SQLite is a file on your computer, bundled into the executable; no database server, account, Docker or network connection is required at runtime.

| Approach | Tradeoff |
| --- | --- |
| Markdown only | Easy to read and edit; reliable deduplication, revisions, evidence links and indexes would need custom infrastructure. |
| JSON/JSONL files | Useful for interchange; updates, deletion, locking and consistent indexes become our responsibility. |
| SQLite + Markdown | Transactions and search for structured records; ordinary Markdown for Obsidian and user notes. Recommended. |

The initial application has one Rust crate and direct SQL through `rusqlite`. There is no ORM, async runtime, vector database or service to operate. See [the architecture decision](docs/architecture.md) for the storage boundaries and next steps.

## Try it

With a Rust toolchain installed, run `cargo` normally. This workspace also has an isolated development toolchain under the ignored `.tools/` directory; `./scripts/cargo-local` uses it without changing your shell profile.

```sh
./scripts/cargo-local run -- --data-dir .observr init
./scripts/cargo-local run -- --data-dir .observr import tests/fixtures/chatgpt.json
./scripts/cargo-local run -- --data-dir .observr status
./scripts/cargo-local run -- --data-dir .observr search "Rust"
./scripts/cargo-local run -- --data-dir .observr export .observr/vault
```

For real history, explicitly pass a ChatGPT export ZIP or extracted `conversations.json`. Import does not contact providers, scan other applications or call a model.

```sh
cargo run -- import /path/to/conversations.json
cargo run -- search "SQLite"
cargo run -- export /path/to/Observr-vault
```

Run `cargo run -- init` first when using the default data location. Without `--data-dir`, storage uses the OS application data directory (on macOS, `~/Library/Application Support/observr`). `status` prints the actual location.

## What works

- ChatGPT mapping-format JSON and ZIP imports, with bounded file sizes and no archive extraction onto disk.
- Provider-scoped conversation identities, source message IDs, timestamps and metadata.
- Conversation trees, including alternative branches and empty mapping nodes; unknown timestamps stay unknown.
- Idempotent imports. Changed conversations create immutable revisions; current search results exclude previous revisions.
- SQLite FTS5 search over message text. Search words are treated literally and combined with AND.
- Markdown conversation snapshots with stable message anchors, branch references, literal transcript text and safe filenames.
- Export preserves existing edited files and reports conflicts. New source content produces a new snapshot.

## Current limits

This is the foundation, not the complete intelligence MVP. There is no analysis, profile, pattern detection, local model integration, dashboard, background collection or bidirectional vault sync yet. Exported notes are conversation evidence, not generated insights.

SQLite, its search index and Markdown exports are **plaintext**. On Unix, newly created application directories and files use private permissions. Encryption and key management still need a deliberate implementation before treating this as a hardened private archive. No secret redaction is implemented.

Imports load the supported JSON document into memory, capped at 256 MiB uncompressed, then commit the supplied batch in one transaction. Attachment payloads and images are not imported; their source message metadata is retained. Provider export formats can change; compatibility is currently tested with synthetic fixtures rather than a real export corpus.

Changed source revisions and old Markdown snapshots remain on disk. Selective deletion and derived-content cleanup are not implemented yet; use synthetic data while developing. Deleting the entire chosen data directory removes that store, but separately exported folders and your original provider archive must be removed separately.

## Development

```sh
./scripts/cargo-local fmt --check
./scripts/cargo-local test --locked
./scripts/cargo-local clippy --locked --all-targets -- -D warnings
```

Only synthetic conversation data belongs in the repository. Keep real exports and generated vaults outside the checkout or under the ignored `.observr/` directory.
