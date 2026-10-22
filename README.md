# Observr

Local AI conversation history, grounded discoveries and a reviewable knowledge vault, written in Rust.

Observr imports selected histories, keeps immutable evidence, extracts candidate preferences/corrections/decision events and lets you confirm, correct or reject them. A localhost dashboard and Markdown vault make results inspectable. **This is an implemented MVP under evaluation; production readiness and useful intelligence are not established.** See [release evidence](docs/readiness.md).

## Storage

One embedded **SQLCipher-encrypted SQLite file** stores conversations, revisions, full-text search, observations and feedback. Markdown is an explicitly generated readable projection. JSON/JSONL is an input format. There is no database server, ORM, vector service or account requirement.

SQLite fits transactional imports, edits, deletion and evidence relationships. DuckDB can be added later for analytical exports if needed; maintaining two databases now adds work without improving the core workflow. See [architecture](docs/architecture.md).

## Build and try

The tested toolchain is Rust 1.99.1 on Apple Silicon macOS 27.0.1. Bundled SQLCipher uses Apple CommonCrypto and needs native C build tools. [Platform policy](docs/platforms.md) · [Native dependency baseline](docs/native-dependencies.md).

```sh
cargo build --locked
cargo run -- init
cargo run -- import tests/fixtures/chatgpt.json
cargo run -- analyze
cargo run -- discover
cargo run -- web
```

This workspace also provides `./scripts/cargo-local` for its isolated, ignored development toolchain. Use it in place of `cargo` when that toolchain is present. `cargo install --path . --locked` installs the CLI from your checkout; no Homebrew package or published binary is provided yet. A clean commit can produce a local evaluation archive with checksums using `python3 scripts/package-release.py`; see [package contents and checks](docs/releasing.md).

On macOS, `init` creates a random key in macOS Keychain and an encrypted archive under `~/Library/Application Support/observr`. Keychain access may prompt. `--data-dir DIRECTORY` selects an independent archive. Initialization does not scan sources or call a model.

## Bring your history

```sh
observr sources --detect
observr import /path/to/conversations.json
observr import /path/to/chatgpt-export.zip
observr import ~/.claude/projects --source claude-code
observr import ~/.codex/sessions --source codex
observr import "$HOME/Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent" --source kiro
observr status
observr analyze
observr search "SQLite"
```

Detection inventories known session directories; importing reads the selected location. ChatGPT needs an account export. Collection is manual, with no watcher, browser extension or daemon. [Supported formats, limits and attribution safeguards](docs/sources.md).

## Review findings

```sh
observr discover --all
observr patterns
observr patterns --topics
observr profile --candidates
observr timeline
observr today --timezone Europe/Luxembourg
observr feedback OBSERVATION_ID confirm
observr feedback OBSERVATION_ID correct --text "My corrected preference and context"
observr feedback OBSERVATION_ID reject
observr web
```

`web` prints a loopback-only URL with a random session token. Open that complete link locally; treat it as private while the server runs. The dashboard supports evidence inspection, search and feedback. Ctrl-C stops the server.

The default rules engine is deliberately narrow. It recognizes direct English statements and offers no broad understanding of your history. All findings start as candidates; recurrence alone never confirms a personal fact. Pasted conversations can still be misinterpreted. Only explicit review promotes a finding to the confirmed profile. Daily outputs are activity digests with evidence, rather than assertions that requested work was completed.

An optional Ollama adapter supports a separately installed **local** model:

```sh
# Start your trusted Ollama daemon with cloud features disabled first.
OLLAMA_NO_CLOUD=1 ollama serve
observr analyze --engine ollama --model YOUR_INSTALLED_LOCAL_MODEL
```

Observr does not install/download models automatically. It accepts loopback HTTP only, rejects advertised remote/cloud models before sending content, applies best-effort secret redaction, constrains citation choices, separately checks claim support and checks output against original user-message evidence. The tested local setup and model tag are in [local-model.md](docs/local-model.md). A trusted daemon is necessary. Real-data checks pass provenance validation; semantic accuracy/usefulness remain release blockers.

Related topics are available with `patterns --topics` (or `--topics --json`) and the dashboard’s Topics view. They connect findings with the same normalized topic label across at least two available independent conversations. Each claim keeps its context, evidence and review status. These connections do not establish an enduring preference or a current choice. Review the individual findings to correct or remove support.

## Generate a vault

```sh
observr vault sync /path/to/Observr-vault --timezone Europe/Luxembourg
observr export /path/to/transcript-snapshots
```

The vault contains Home, Profile, Discoveries, Candidates, Patterns, related Topics, Decisions, Daily notes and linked transcript snapshots. Obsidian is optional. Generated sections have ownership hashes; edits outside them survive synchronization. Conflicting generated edits are preserved and reported. [Import an explicit Markdown correction](docs/vault-feedback.md) with `vault apply-feedback FILE --check`, then apply it after reviewing. Stale review counts prevent accidental replay or overwriting another correction.

**Markdown exports are plaintext.** Generate them only in a location you choose to protect. Observr tracks owned projections for deletion/feedback cleanup, preserving user-written sections and reporting conflicts. Independent copies and older backups remain your responsibility.

## Protect and recover your archive

```sh
observr doctor
observr key export /private/location/recovery.key
observr backup /private/location/archive-backup.sqlite3
observr --data-dir /path/to/new-archive --key-file /private/location/recovery.key restore /private/location/archive-backup.sqlite3
```

Keep recovery keys separate from backups. Restore requires an unused destination and the key that encrypted that backup. It validates integrity and does not take ownership of the original archive's Markdown files. Losing both Keychain access and the recovery key loses access to encrypted data.

For explicit key-file operation (also the available path on non-macOS systems):

```sh
observr key create /private/location/observr.key
observr --data-dir /path/to/archive --key-file /private/location/observr.key init
```

Use that same `--key-file` with subsequent commands. On Unix key files must be private regular files, with mode `0600`. `--insecure-plaintext` explicitly selects development/legacy storage and never decrypts an encrypted archive. Default commands refuse plaintext stores; `encrypt-copy --new-key-file KEY` creates a separate encrypted copy. [Upgrade, restore and key-change workflows](docs/recovery.md).

`exclude CONVERSATION_ID` hides retained evidence from search, analysis and projections; `include` reverses it. `delete conversation ID` purges canonical source revisions and derived dependencies; `delete observation ID` removes a finding and records an identity tombstone. `cleanup` retries projection conflicts. Persistent provider/project/directory/literal-topic rules are available through `privacy add/list/remove`. [Matching and limits](docs/privacy.md). Key changes use a verified backup/new-archive workflow. Deletion cannot erase independent backups or original exports. [Security boundaries](SECURITY.md).

## Development

```sh
./scripts/cargo-local fmt --check
./scripts/cargo-local test --locked
./scripts/cargo-local clippy --locked --all-targets -- -D warnings
```

Only synthetic fixtures belong in Git. Keep real histories, keys, generated vaults and local model data outside the checkout or under ignored `.observr/`. The [original product brief](docs/product-brief.md) is unchanged; [current release evidence](docs/readiness.md) distinguishes implementation from unverified requirements and future work.

## License

Observr is licensed under [MIT](LICENSE). Native dependency notices are included under [third_party/notices](third_party/notices/).
