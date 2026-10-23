# Using Remnary

Start with the [README quick start](../README.md#quick-start). This guide covers imports, review, search, Markdown output and recovery. Commands assume `remnary` is on your `PATH`; use `remnary.exe` and Windows paths in PowerShell. On Linux, add your `--key-file` to every archive command.

## Choose a separate archive

```sh
# Use an existing private parent directory; choose a new key filename.
remnary key create /private/keys/remnary.key
remnary --data-dir /private/archive --key-file /private/keys/remnary.key init
remnary --data-dir /private/archive --key-file /private/keys/remnary.key import /path/to/conversations.json
```

On Linux, choose a private location such as `$HOME/.local/share/remnary` and create the parent directory first. The `/private/...` paths above are placeholders, not required OS directories. On PowerShell, use Windows paths and `remnary.exe`. Use the same `--data-dir` and `--key-file` with subsequent commands. Key files must be private regular files, with Unix mode `0600`. Keychain access may prompt. [Existing development archives →](renaming.md)

## Bring your own history

| Source | Selected input | Status |
| --- | --- | --- |
| **ChatGPT** | Mapping-format `conversations.json`; ZIP with that file or numbered conversation shards | Supported format with synthetic ZIP tests and five published real mapping records. An original account-export ZIP remains unverified. |
| **Claude Code** | Local session JSONL file or directory | Supported; exercised on local histories. This is different from Claude consumer exports. |
| **Codex** | Local rollout JSONL file or directory | Supported; attribution and inherited-session handling have regression coverage. |
| **Kiro IDE** | Local `.chat` JSON file or selected chat directory | Supported observed format; cumulative copied dialogue is correlated conservatively. |
| **Partial app history views** | Explicitly supplied ChatGPT snapshot JSON | Advanced/manual adapter; missing context and truncation stay labelled. No built-in account connector. |

```sh
# Inventory known local session locations; this does not import message bodies.
remnary sources --detect

# Choose the files or directory you want imported.
remnary import /path/to/chatgpt-export.zip
remnary import ~/.claude/projects --source claude-code
remnary import ~/.codex/sessions --source codex
remnary import "$HOME/Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent" --source kiro

remnary status
remnary sources
```

Imports preserve normalized revisions. Reimporting unchanged input does not add duplicate records. Detection is separate from import; there is no automatic watcher, browser extension or provider login flow. Private application formats can change. Claude consumer exports, Kiro CLI exports, Cursor databases and Gemini account histories are not supported adapters today. [Exact format boundaries, limits and attribution →](sources.md)

## Review what Remnary finds

```sh
remnary discover --all --limit 100
remnary patterns
remnary patterns --topics
remnary profile --candidates
remnary timeline
remnary today --date 2026-10-18 --timezone Europe/Luxembourg
```

Findings carry a type, a source-linked statement, an explicit/inferred label, supporting messages and a review status. Recurring support means the evidence repeats; it does not make a personal fact confirmed.

```sh
# Replace OBSERVATION_ID with an ID from `discover`.
remnary feedback OBSERVATION_ID confirm
remnary feedback OBSERVATION_ID correct --text "My preferred wording and context"
remnary feedback OBSERVATION_ID reject
remnary feedback OBSERVATION_ID dismiss
remnary feedback OBSERVATION_ID reset

remnary profile
```

**Confirm** includes the finding in your profile. **Correct** saves your wording as confirmed. **Reject** records that it is wrong; **dismiss** sets it aside. **Reset** undoes the current review while retaining its history. Source evidence stays attached. The dashboard offers the same main review actions.

The typed extraction format supports explicit and inferred preferences, corrections, decisions, topics, project references and behavioral patterns. The default rules engine recognizes a deliberately small set of English statements and comparison requests. Richer extraction uses the optional model; no engine gives perfect interpretation. [Evaluation and known errors →](readiness.md)

## Search and follow the evidence

```sh
remnary search "SQLite"
remnary sources
remnary show CONVERSATION_ID --revision 1
```

Search uses SQLite full-text indexing over current, available conversations. Query words are treated literally and must match within a message. Source links in findings open their exact retained revision, even after a newer revision is imported. Missing timestamps stay unknown; partial views remain partial. There is no semantic/vector search service in this release.

## Build a readable knowledge vault

```sh
remnary vault sync /path/to/Remnary-vault --timezone Europe/Luxembourg
remnary export /path/to/transcript-snapshots
```

`vault sync` creates linked knowledge notes; `export` creates immutable transcript snapshots. Obsidian is optional—these are regular Markdown files.

```text
Remnary-vault/
├── Home.md                 Your starting point
├── Profile.md              Only confirmed knowledge
├── Discoveries.md          Ranked, reviewable findings
├── Topics.md               Related-topic threads
├── Candidates/             Individual unconfirmed findings
├── Decisions/              Decision-event notes
├── Patterns/               Evidence-linked pattern notes
├── Topics/                 Individual related-topic notes
├── Daily/                  Activity digests by local date
└── Transcripts/            Source-revision snapshots
```

Generated sections have ownership hashes. Write your own annotations **outside** those sections; synchronization preserves them. Edited generated sections produce conflicts rather than silent overwrites. You can also author an explicit `remnary-feedback` block and validate it before applying it. [Markdown feedback example →](vault-feedback.md)

**Vaults and transcript exports are plaintext.** Generate them in a location you choose to protect. Sync again after review changes; `cleanup` retries managed projection cleanup. Original exports, independent copies and older backups remain outside the canonical deletion boundary.

## Optional local model

Rules analysis works without a model. For richer candidate extraction, install Ollama and a local model separately, then start a trusted loopback daemon with cloud features disabled:

```sh
OLLAMA_NO_CLOUD=1 ollama serve
remnary analyze --engine ollama --model YOUR_INSTALLED_LOCAL_MODEL
```

The evaluated setup uses `gemma4:12b-it-q4_K_M` on an M4 Pro with 24 GiB memory. Model downloads and inference require additional disk space, memory and time. Remnary does not download models automatically. [Exact setup, model identity and evaluation →](local-model.md)

Remnary rejects advertised cloud models and non-loopback endpoints, applies best-effort redaction, constrains citations, separately checks claim support and validates exact eligible-owner evidence. It still depends on a trustworthy local daemon. Model errors can remain; your review determines confirmed knowledge.

## Back up and recover

```sh
remnary doctor
remnary key export /private/keys/recovery.key
remnary backup /private/backups/archive.sqlite3
remnary --data-dir /path/to/new-archive --key-file /private/keys/recovery.key restore /private/backups/archive.sqlite3
remnary --data-dir /path/to/new-archive --key-file /private/keys/recovery.key doctor
```

Keep recovery keys separate from backups. Restore uses the key that encrypted that backup and requires an unused destination. Losing both native credential-store access and the recovery key loses access to the encrypted data. Key rotation uses a verified new-key copy/new-archive workflow. [Recovery, plaintext upgrades and key rotation →](recovery.md)

## Command reference

Run `remnary COMMAND --help` for arguments and flags. `--data-dir` and `--key-file` are global options.

| Command | Purpose |
| --- | --- |
| `init`, `status`, `doctor` | Initialize, inspect counts, check integrity and pending cleanup. |
| `import`, `import-app-snapshot` | Import selected files or an explicitly supplied partial view. |
| `sources --detect`, `sources` | Inventory known local locations; list imported source identities. |
| `analyze` | Incremental rules or local-model extraction. |
| `discover`, `patterns`, `patterns --topics` | Review ranked findings, recurring evidence and related topics. |
| `profile`, `profile --candidates` | Confirmed knowledge or candidate review. |
| `timeline`, `today` | Decision events and local-day activity digests. |
| `search`, `show` | Literal full-text search and exact normalized revisions. |
| `feedback` | Confirm, correct, reject, dismiss, supersede or reset a review. |
| `web` | Start the authenticated localhost dashboard. |
| `vault sync`, `vault apply-feedback`, `export` | Managed knowledge notes, explicit Markdown review and transcript snapshots. |
| `exclude`, `include`, `privacy` | Manual and persistent source exclusions. |
| `delete`, `cleanup` | Canonical deletion and managed projection cleanup. |
| `key`, `backup`, `restore`, `encrypt-copy` | Recovery keys, encrypted snapshots and explicit legacy conversion. |

`--insecure-plaintext` is an explicit development/legacy mode. It never decrypts an encrypted archive. Prefer the default encrypted workflow.

## Frequently asked questions

<details>
<summary><strong>Does Remnary need an LLM, subscription or provider account?</strong></summary>

No for the rules workflow. The demo runs entirely from bundled fictional files. An optional local model needs its own separately installed runtime and model. To import your account history, you provide an export or supported local session files.

</details>

<details>
<summary><strong>Can it read all my ChatGPT or Claude conversations automatically?</strong></summary>

No. The CLI does not log into provider accounts or scrape your browser. ChatGPT mapping exports and selected local Claude Code/Codex/Kiro files are supported. Partial app snapshots are an explicit advanced input, not a built-in account connector. Claude consumer exports are a separate unsupported format.

</details>

<details>
<summary><strong>Does recurring evidence mean a preference is confirmed?</strong></summary>

No. It means the available evidence repeats across counted conversations. Only your confirmation or correction adds a finding to the confirmed profile. Related topics preserve separate claims and contexts instead of silently merging them into an enduring personal trait.

</details>

<details>
<summary><strong>Do I need Obsidian? Can I edit generated notes?</strong></summary>

Obsidian is optional. Any Markdown reader works. Edit your annotations outside generated sections; managed-body edits become conflicts. Explicit review blocks can change findings through the validated feedback workflow.

</details>

<details>
<summary><strong>Why is the profile empty after analysis?</strong></summary>

That is expected. Analysis creates candidates. Review one with `feedback ID confirm`, correct its wording, or use the dashboard. The fictional demo also starts with an empty profile.

</details>

<details>
<summary><strong>Where does the name come from?</strong></summary>

**Remnant + diary.** A remnant is a trace of something from the past; a diary is a personal record of experiences, thoughts and activities. Remnary is an invented word, not an established dictionary entry. It represents a living record built from fragmented conversations, with room to grow beyond AI tools.

</details>
