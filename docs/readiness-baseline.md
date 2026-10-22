# Product requirements and release evidence

This tracker preserves the scope of the [original product brief](product-brief.md). The user selected Rust in place of the brief's proposed TypeScript/Node stack and selected SQLite plus Markdown after comparing file-based alternatives. The brief itself is an unchanged copy of the uploaded file. Rust modules may satisfy the suggested package boundaries without creating an empty monorepo.

**Release status: incomplete; production readiness is not established.** Passing foundation tests does not prove that the intelligence MVP, security requirements, or product-quality targets are satisfied.

Inventory date: 2026-10-19. This is the initial foundation inventory, taken before parallel implementation of intelligence and operations. Later changes must be inspected and tested before updating statuses. A file, command name, or passing unit test is not sufficient evidence for a broader claim.

Status meanings:

- **Verified:** the stated narrow behavior was inspected and exercised by the recorded test run.
- **Partial:** some implementation exists, but the requirement's full scope is incomplete or unverified.
- **Missing:** no implementation or evaluation evidence exists in the inventoried tree.
- **Future / optional:** the brief explicitly assigns this capability beyond the mandatory v0.1 scope. Its safety invariants still apply if implemented early.

## Mandatory v0.1 features

These rows follow the fifteen numbered requirements in §24.2. They are cumulative; a working archive alone does not satisfy the product brief.

| ID | Requirement | Initial status and evidence | Evidence needed to close |
| --- | --- | --- | --- |
| MVP-01 | CLI application | **Verified, foundation only.** `src/main.rs` defines `init`, `status`, `import`, `search`, `export`; `tests/cli.rs::imports_searches_and_exports_without_losing_edits` exercises the flow. | Exercise the complete §24.3 flow, help/errors, all supported data-directory modes, and nonzero failures on the release binary. |
| MVP-02 | ChatGPT export importer | **Partial.** `src/import.rs` tests JSON and ZIP mapping exports, all branches, null/missing metadata, malformed graphs, archive ambiguity, and a 256 MiB cap. Fixtures are synthetic. | Compatibility evidence from appropriately sanitized recent real exports; document unsupported attachments and formats; test graceful diagnostics without data loss. |
| MVP-03 | Normalized conversation storage | **Verified for supported fixtures.** `src/model.rs`, `src/schema.sql`, and store round-trip tests retain provider/source IDs, roles, optional times, metadata and branch parents. | Preserve timezone information whenever future source formats provide it; prove stable evidence identities across changes, reprocessing, backup and restore. |
| MVP-04 | SQLite database | **Partial.** v1 schema initialization is transactional, foreign keys are enabled, application/version IDs are checked, and interrupted empty initialization is recoverable (`src/store.rs`). | Sequential transactional migration tests, existing-data upgrade fixtures, corrupt/foreign/newer-schema refusal, recovery and protected backups. Encryption is independently mandatory below. |
| MVP-05 | Deduplication | **Verified for source records.** Store tests cover unchanged reimports, provider-scoped identities, immutable revisions, and atomic rollback. | Reimport and reanalysis must also preserve observation identities, feedback and pattern counts; test the changed-then-older archive case explicitly. |
| MVP-06 | Local conversation search | **Verified for current source text.** FTS5 indexes message text; literal-token search filters to current revisions and returns source/message revision references. | Search remains correct after migrations, deletion, restore, and exclusion; sensitive text is protected in FTS and storage journals. |
| MVP-07 | Structured observation extraction | **Missing.** No extraction module, observation schema, model adapter or extraction command exists in the initial inventory. | Validate candidates against a strict schema and actual user messages; store exact evidence, extraction/model/prompt versions and status; reject unsupported or invented claims. |
| MVP-08 | Recurring pattern detection | **Missing.** | Group corrections/preferences across genuinely distinct conversations, compute counts/time range/platforms from evidence, keep ambiguous topics separate, and evaluate labeled expected groups. |
| MVP-09 | Personal profile generation | **Missing.** | Separate explicit statements from inferences; implement candidate/supported/confirmed/rejected/superseded/dismissed lifecycle, context, history, contradictory evidence and editable feedback. |
| MVP-10 | Decisions and basic timelines | **Missing.** | Extract proposals, considerations, decisions, reconsiderations, reversals and confirmations; preserve history; do not turn a discussed option into a current decision. Unknown time/state stays unknown. |
| MVP-11 | Daily summary generation | **Missing.** | Generate date-scoped summaries with source links; preserve unknown dates and source coverage; distinguish discussed work from completed work; test deterministic timezone boundaries. |
| MVP-12 | Obsidian-compatible Markdown export | **Partial.** `src/vault.rs` generates immutable transcript snapshots with source anchors, safe filenames, literal source content and conflict preservation. | `vault sync` must produce the Home/profile/pattern/decision/daily/discovery knowledge projection, preserve user content, import corrections as feedback, handle conflicts and propagate deletion idempotently. Obsidian must remain optional. |
| MVP-13 | Evidence links | **Partial.** Search exposes `provider:conversation@revision#message`; Markdown includes message anchors. No derived knowledge exists yet. | Every observation, pattern, decision, insight and summary must resolve to an exact retained message revision and a validated quote; test stale/deleted/replaced evidence and all rendered links. |
| MVP-14 | User feedback on observations | **Missing.** | Accept/reject/correct/delete actions must persist with history, update profile/discoveries/vault, survive reanalysis/reimport/restore, and prevent silent resurrection of rejected or deleted claims. |
| MVP-15 | Minimal localhost dashboard | **Missing.** | Launch a protected loopback server; inspect discoveries/profile/timeline/sources and apply feedback; test the security gate and browser behavior described below. A polished Phase 4 dashboard is future work, but this minimal dashboard is mandatory. |

The §24.4 success criteria require correct ingestion, safe repeated imports, reliable evidence, useful patterns, accurate summaries, correctable profiles, usable notes and no fabricated source conversations. Only the foundation subset has evidence at this baseline.

## Privacy and security release requirements

The brief calls §23 foundational. These requirements do not become optional because the initial program runs locally. The additional failure-case checks below make those requirements reviewable; they are release gates, not claims that the brief specified a particular cryptographic library or mechanism.

| ID | Requirement and gate | Initial status / authoritative evidence |
| --- | --- | --- |
| SEC-01 | Explicitly selected sources only; no scanning or implied connection. Explain import-only versus live/periodic/unsupported collection accurately. | **Partial.** The current CLI reads only the explicitly passed ChatGPT file. No persisted source controls or exclusions exist. Future adapters must require opt-in before reading. |
| SEC-02 | Keep data local by default; cloud analysis/sync must require explicit opt-in with visible provider/scope. No hidden telemetry or account requirement. | **Verified for the initial runtime feature set.** Initial `Cargo.toml` and source have no network client, remote calls, telemetry or account flow. Repeat this audit when any model/dashboard/network code is added. |
| SEC-03 | Encrypt stored conversations and derived knowledge, including FTS, journals, temporary files and backups. Document the threat model and deliberate plaintext export boundary. | **Missing.** Initial `rusqlite` uses ordinary bundled SQLite; `remnary.sqlite3` and Markdown are plaintext. Restrictive Unix permissions are not encryption. A plaintext Markdown export must be an explicit user-controlled action with clear handling. |
| SEC-04 | Use vetted authenticated encryption or an appropriately configured encrypted SQLite implementation; use OS-protected keys where available. Define key creation, unlock, recovery, rotation, backup portability and unsupported-platform behavior. | **Missing.** No keys/crypto exist. Release checks must show keys never appear in repo, CLI arguments, logs or backups beside ciphertext; wrong/missing keys and tampered data fail safely; interrupted rotation is recoverable; locked stores fail closed. Do not invent custom cryptography. |
| SEC-05 | Detect likely API keys, tokens and credentials before analysis or transmission. Redaction is best-effort and must not be advertised as complete. | **Missing.** No redaction path exists. Test obvious credential families, mixed Unicode, URLs, code blocks and metadata; prove all analysis adapters use the same redaction boundary and errors do not echo secrets. |
| SEC-06 | Exclude providers, projects, conversations, directories, topics and future sessions. Pausing future collection must work without deleting data. | **Missing.** Define stored exclusion rules and their effect on already stored evidence, search, analysis and export. Test scope precedence, persistence and no accidental reintroduction. Pause controls become applicable with collection. |
| SEC-07 | Avoid automatically inferred sensitive-characteristic profiles and unsupported psychological judgments. Explicit sensitive disclosures require user control before long-term profile promotion. | **Missing.** Implement and evaluate the extraction/review policy; an evidence quote alone does not justify sensitive inference or established-fact status. |
| SEC-08 | Delete source conversations, observations, dependent patterns/insights and generated projections. Avoid resurrection on reanalysis/reimport without new evidence and a clear review process. | **Missing.** Schema foreign keys help source referential integrity but there is no deletion operation, feedback tombstone or vault cleanup. Test source revisions, FTS, derived rows, generated notes and edited-note conflicts; document retained external exports and backup retention. |
| SEC-09 | Protect localhost UI from arbitrary local/remote websites. Bind loopback, authenticate access, validate origins/Host, protect mutations, escape untrusted data and use secure defaults. | **Missing.** No server exists initially. Test unauthenticated access, cross-origin requests/forms, DNS-rebinding Host values, token leakage, XSS transcripts, path traversal/symlinks, oversized requests and unsupported methods. Loopback binding alone does not satisfy this gate. |
| SEC-10 | Private filesystem creation and safe output paths across supported platforms. Do not overwrite user-owned data. | **Partial.** New Unix directories/files request 0700/0600; export hashes names, refuses preexisting symlink snapshots and preserves edits. Audit preexisting directories, database symlinks, ancestor symlinks/races, atomic durable writes and Windows permissions before claiming broader guarantees. |
| SEC-11 | No system-wide keylogging, hidden recording, unrelated browser monitoring, private traffic interception or unauthorized account access (§23, §37). | **Verified for initial implementation.** No such collectors exist. Keep as an invariant for every new adapter, extension and automation feature. |

## Provenance, review and knowledge invariants

These are the acceptance conditions for §3–4, §8–14, §20 and §22, beyond merely having a command with the right name.

| ID | Invariant | Initial status | Verification needed |
| --- | --- | --- | --- |
| KNOW-01 | An observation carries type, subject, topic, canonical value, explicit/inferred label, context, status, timestamps and evidence. | **Missing** | Schema and round-trip tests for each supported candidate; no implicit confidence-to-fact promotion. |
| KNOW-02 | Evidence points to provider/conversation/message **revision**, validated quote and evidence type; assistant suggestions are not user preferences or completed user actions. | **Partial** | Source revisions exist; derived foreign keys and malicious/malformed candidate tests are absent. Show every accepted evidence quote matches its stored source. |
| KNOW-03 | Reprocessing is versioned, incremental and idempotent. Model/processor/prompt versions and content fingerprints explain every derived result. | **Partial** | Source fingerprints exist; no extraction runs, checkpoints or caching exist. Test restart, changed processor/model/prompt, unchanged input and failed batches. |
| KNOW-04 | Grouping uses distinct conversations and context; recurrence does not imply independent evidence or certainty. | **Missing** | Labeled near-duplicate/paraphrase/ambiguous-topic tests, deterministic counts, evidence diversity and chronology. |
| KNOW-05 | Historical information and contradictory evidence survive reconciliation; proposals and alternatives do not silently change a confirmed decision. | **Missing** | Synthetic evolving-decision and contradictory-preference timelines with manually defined current/unknown states. |
| KNOW-06 | Feedback is authoritative and traceable; reanalysis must not overwrite corrections, rejection or deletion. | **Missing** | Accept, reject, correct, delete, reanalyze, source update, vault regeneration and restore tests with exact expected results. |
| KNOW-07 | Generated summaries, counts, dates and project associations come from available evidence. Missing coverage, unknown times and ambiguity are visible. | **Missing** | Expected daily/timeline outputs, boundary dates, unknown timestamps, branches, small/incomplete corpora and links that open the cited evidence. |
| KNOW-08 | Markdown is a projection of canonical SQLite records. Generated identities are stable; user sections survive; conflicts request review; manual corrections import as feedback. | **Partial** | Immutable transcript conflict tests pass. No managed knowledge sections, feedback import or deletion manifest exists. Test repeated sync, edited/deleted files, interrupted sync and user-owned content. |
| KNOW-09 | The engine produces a few useful discoveries with evidence, context and restrained wording rather than generic analytics or unsupported personality judgments. | **Missing** | Quality evaluation below, including human usefulness judgments and a recorded result with actual corpus coverage. |

## Engineering, compatibility and operations gates

| ID | Gate | Initial status / evidence needed |
| --- | --- | --- |
| ENG-01 | Automated parser/normalization/deduplication tests (§27.1). | **Verified for synthetic supported exports.** Initial run: 21 library tests + 1 CLI integration test passed. Add real-format compatibility coverage before calling importer production-ready. |
| ENG-02 | Automated migration/evidence/grouping/profile/decision/vault/feedback/deletion tests (§27.1). | **Partial.** Initial schema/version guards and transcript export are tested. No sequential migrations, derived evidence, grouping, profile, decisions, feedback or deletion tests exist. |
| ENG-03 | Synthetic evaluation corpus with manually defined expected outputs (§27.2). | **Partial.** Two ChatGPT fixture files include repeated preferences and branches. Add repeated corrections, contradictory preferences, one project across conversations, evolving decisions, similar unrelated topics, inferred patterns and duplicates; expected extractions/groups/timelines must be independently defined. |
| ENG-04 | Evaluate grounding, precision, duplicates, relevance, temporal accuracy and user trust (§27.3–4). | **Missing.** Record versioned corpus, engine/model/prompt, evidence checks, expected/actual outputs and measurement method. Target ≥90% grounded factual observations, zero fabricated evidence and duplicates, correct decision reconstruction and feedback/deletion propagation. Passing an invented fixture does not establish real-world precision. |
| ENG-05 | Human usefulness and substantial real-history evaluation (§25 Phase 0, §27.4, §35 Task 10). | **Missing.** A test user must review actual top discoveries; target ≥3 of top 5 useful. Record accuracy, evidence quality, duplicates, processing cost and useful/incorrect examples without committing personal data. Human ratings cannot be replaced by automated self-grading. |
| ENG-06 | Thousands-of-conversations usability and incremental processing (§28). | **Partial.** Bounded input and atomic batches exist, but the importer buffers the full document and store reads/export load full current history. No performance dataset, checkpoint/resume, analysis cache or budget evidence exists. Record wall time, peak memory, output sizes, search latency and reanalysis work on a reproducible generated corpus; justify limits. |
| ENG-07 | Transactional recovery and operational diagnosis. | **Partial.** Empty initialization recovery, rollback and 5-second busy timeout are tested. Add crash/interruption and contention tests, migration failure recovery, bounded work, structured privacy-safe errors/status and a `doctor` diagnostic. Logs must not contain conversation text, secrets or keys. |
| ENG-08 | Backup/restore and deletion retention (§28.2). | **Missing.** The architecture only advises copying a closed initial database. Implement safe consistent snapshots, encryption/key portability, integrity/schema validation before restore, non-destructive failure handling and end-to-end restored search/feedback/evidence checks. Define external-vault and historical-backup deletion responsibilities. |
| ENG-09 | Provider format compatibility and graceful degradation (§32.3). | **Partial.** Synthetic parser cases cover unknown metadata, malformed input and branch trees. Maintain sanitized format/version fixtures, unknown-role/content tests, multilingual/Unicode text, missing IDs/timestamps and documented unsupported features. Never silently omit a record or claim an unsupported source is connected. |
| ENG-10 | Supported-platform release verification (§38). | **Missing.** macOS is initial target; Linux/Windows are future until explicitly tested/supported. Test release installation, data paths, permissions, SQLite/FTS, Unicode paths, backup/export and clean uninstall on each advertised target. A compile job alone does not prove runtime portability. |
| ENG-11 | Reproducible build, formatting, lint, tests and dependency review. | **Partial.** `Cargo.lock`, Cargo manifest and `scripts/cargo-local` exist. Add CI with locked tests, fmt, Clippy, release build, license/dependency vulnerability audit and platform smoke tests; inspect actual CI results before claiming green CI. Pin/declare a supported Rust toolchain/MSRV. |
| ENG-12 | Open-source release documentation and artifacts (§26, §33, §35). | **Partial.** README and architecture exist. Add license, contributing/security/privacy docs, supported-source/version/limit matrix, install/upgrade/backup/recovery/delete instructions, CLI help examples, changelog and release process/checksums. Installation proposals (`brew`/`npm`) are not shipped mechanisms. |
| ENG-13 | Local Git history requested by the user. | **Partial at inventory.** A local repository exists; no commit was present when inspected. Make logical tested commits; verify clean status and absence of secrets/personal exports; do not publish or upload data implicitly. |
| ENG-14 | Full MVP workflow on release artifact (§24.3, §31). | **Missing.** Install/init → import ZIP → analyze → discover → vault sync → web → review profile/evidence. Repeat with duplicates, changed history, feedback, exclusion, delete, backup and restore; inspect generated Markdown and the rendered dashboard. Demo counts must come from actual data. |

Initial reproducible foundation check (not a release sign-off):

```sh
./scripts/cargo-local test --locked
# Inventory run on 2026-10-19: 21 library tests and 1 CLI integration test passed.
```

## Command coverage

Commands in the brief are product requirements or proposals with differing phases; this matrix prevents a name in documentation from implying a shipped feature.

| Command | Initial state | Intended scope |
| --- | --- | --- |
| `init`, `status`, `import <file>`, `search <query>` | Implemented; foundation CLI integration test passes. | Mandatory v0.1 foundation. Setup is manual-import-only and does not verify/enable other applications. |
| `export <output>` | Implemented as immutable transcript Markdown snapshots. | Mandatory export foundation; full knowledge projection remains incomplete. |
| `analyze`, `discover`, `profile`, `patterns`, `timeline`, `today`, `vault sync` | Missing initially. | Mandatory v0.1 behaviors (§24) exposed through the proposed CLI (§6). |
| `web` | Missing initially. | Minimal secure localhost dashboard is mandatory v0.1; polished explorers are Phase 4. |
| `delete` | Missing initially. | Mandatory user-control/privacy behavior; source and derived deletion must propagate. |
| `doctor` | Missing initially. | Proposed operational interface; should diagnose release requirements without exposing content. |
| `week` | Missing initially. | Weekly summary is optional v0.1 and richer weekly review is future (§36.5). |
| `ask` | Missing initially. | Historical QA is Phase 5; basic local full-text search remains mandatory. If implemented, §17 requires citations, fact/inference distinction and coverage disclosure. |
| `start`, `stop`, `restart`, `logs` | Missing initially. | Phase 2 daemon, explicitly outside mandatory v0.1. |
| `tui` | Missing initially. | Future (§16), explicitly outside mandatory v0.1. |
| `export rules --target codex` | Missing initially. | Phase 6 / §36.3; user-reviewed selective instruction export, no automatic external changes. |

## Complete-brief scope map

This map accounts for all numbered sections without treating the long-term vision as already implemented or silently dropping its requirements. v0.1 is the first release boundary explicitly defined by the brief; completing it does not mean the entire future roadmap is implemented.

| Brief sections | Commitment / phase | Current relationship |
| --- | --- | --- |
| §1–5 | Product vision, target users, evidence/local-first/control/incremental principles. | Invariants for every phase. Archive-only behavior is an incomplete product. |
| §6 | CLI/manual setup for v0.1; installation mechanisms proposed; daemon commands/TUI future. | See command matrix. Never imply live collection for manual imports. |
| §7 | ChatGPT export mandatory v0.1; Claude export optional; Gemini/other provider exports and local coding-agent adapters are later additions; continuous capture is Phase 2–3. | Initial source support is ChatGPT only. The broad initial-sources list does not override §24's explicit boundary. |
| §8–12 | v0.1 focus: evidence-backed preferences/corrections/patterns, profile and basic decisions/history. | Full project intelligence, workflow discovery, forgotten-idea suggestions, long-term preference evolution and cross-provider comparisons continue in Phases 1/5. Advanced labels must not be attached to shallow keyword matches. |
| §13 | Daily summaries mandatory v0.1; weekly optional; monthly trends later. | Accuracy and evidence rules apply to every summary. Automatic scheduling waits for Phase 2. |
| §14 | Obsidian-compatible Markdown knowledge projection mandatory; user edits/corrections and deletion rules are requirements. | Optional Bases/views and exact folder organization are not mandatory; source/knowledge ownership rules are. |
| §15–17 | Minimal localhost dashboard mandatory v0.1; full explorers/profile UI Phase 4; TUI future; evidence-grounded history QA Phase 5. | Security applies from the first served page. Embeddings are optional. |
| §18–20 | Modular normalized architecture, canonical SQLite, FTS, provenance/history/feedback/versioning and migrations. | Rust supersedes the proposed TS/Node tooling. No empty packages are required. Data-model concepts must exist where their feature is implemented. |
| §21 | Production daemon and its scheduling, source monitoring, crash recovery, safe shutdown, structured logs and health state. | Explicitly Phase 2 / outside v0.1. Low idle CPU/no constant inference remain future acceptance gates. |
| §22 | Validated provider-neutral extraction and incremental/versioned processing; local preferred and cloud opt-in. | Ollama is optional v0.1 (§24); remote adapters are optional implementation choices, never authorization to transmit data. |
| §23–24 | Foundational privacy/security plus fifteen mandatory MVP features and success criteria. | Release-blocking tracker above. |
| §25 | Phases 0–1 intelligence/reliable local foundation, Phase 2 collection, Phase 3 extension/universal collection, Phase 4 complete interface, Phase 5 advanced intelligence, Phase 6 optional actions. | Preserve phases and their exit criteria. Do not describe future capabilities as shipped v0.1 behavior. |
| §26–28 | Repository guidance, automated testing, labeled/human quality evaluation, performance and backup operations. | Rust substitution preserves engineering objectives. No phase may skip provenance, privacy or applicable tests. |
| §29–30 | Honest product positioning and comparison against existing alternatives before expansion beyond MVP. | Competitive usefulness evaluation is missing; do not claim novel invention or proven superiority. Marketing/distribution is not runtime functionality. |
| §31 | Illustrative demo flow and truthful actual output counts. | Mandatory MVP flow must run with actual imported data; example counts are not fixture expectations. |
| §32–35 | Risks, open-source/distribution principles, priority ordering and ten first tasks including substantial real-data evaluation. | Priorities guide implementation; they do not remove mandatory dashboard/security/user-control requirements. Monetization/hosted services are future. |
| §36 | Explicit future possibilities: graph, cross-agent comparisons, reviewed agent instructions, selective context portability, weekly review, historical QA, additional structured exports, permission-scoped MCP. | **Future / optional**, incomplete. Plain Markdown export already overlaps MVP, but structured profiles/JSON/agent rules are not implicitly shipped. |
| §37–38 | Product non-goals and proposed defaults. | No surveillance, unsupported personality claims, forced cloud upload or unrelated chatbot/task-manager/orchestrator scope. Rust overrides TS; initial macOS, ChatGPT, SQLite/Markdown and mandatory evidence/control remain. |
| §39–42 | Useful accurate trustworthy discoveries, long-term always-on vision, evidence-to-vault proof first and guiding principle. | Human product evidence is required; the first pipeline and complete future platform must be reported separately. |

Explicitly outside the mandatory v0.1 release (§24): browser extension, universal live capture, production daemon, complete TUI, autonomous rule enforcement, every-provider integration, mobile app and cloud synchronization. These are not implemented by calling the project “production ready”; future phase delivery requires its own verification.

## Release sign-off procedure

1. Update every applicable row against the actual candidate commit, using observed source, test outputs, artifacts and runtime behavior. Preserve scope and record limitations; stale baseline rows must not be presented as current verification.
2. Close all fifteen mandatory feature rows and all applicable privacy/security gates. Complete automated integrity tests for migrations, feedback, evidence, exclusion, deletion, backup/restore and localhost protections.
3. Run the labeled synthetic evaluation and substantial user-authorized real-corpus evaluation. Record results and human usefulness ratings. Investigate errors and rerun affected checks; evidence-accurate quotations do not by themselves prove that interpretations are correct.
4. Run clean locked build/test/lint/security checks and full release-binary CLI/dashboard/vault workflow on advertised platforms. Record performance on thousands of conversations and prove failure/recovery behavior.
5. Review encrypted storage/key lifecycle, plaintext export boundaries, backup handling and data-deletion limitations. Confirm no secrets or private data are staged or embedded in fixtures/artifacts.
6. Verify installation, upgrade, recovery, privacy, license, security-reporting and release documentation against actual commands. Inspect CI results and generated release artifacts; confirm logical local commits and clean Git status.
7. Only then declare the explicitly verified release scope ready. Any missing evaluation, unsupported security assumption or unverified requirement keeps release readiness open. Future roadmap work remains visible rather than being counted as completed.
