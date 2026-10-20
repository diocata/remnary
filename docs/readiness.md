# Current release evidence

Updated 2026-10-20. **Production readiness is not established:** source-grounded accuracy, usefulness and recurrence on the expanded real corpus remain under evaluation. The unchanged [product brief](product-brief.md) defines the MVP. The [initial baseline](readiness-baseline.md) and [evaluation history](evaluation-history.md) preserve prior findings without presenting them as current verification.

## Verified local candidate

The release candidate is commit `aaf0412ccbdf`, built with Rust 1.99.1 for Apple Silicon macOS 27.0.1. All **156 tests** passed: 141 library, eight analysis and seven CLI/source integration tests. Formatting, locked all-target Clippy with warnings denied, optimized compilation and local installation passed. Subsequent commits record evidence only.

The local package contains 306 files, including the CLI, MIT license, security/product documentation and complete target-specific dependency notices. Its SHA-256 is `f7f16b1a2a142864f512770a6920498481c7abe5278c2c90d62dd64afdbae61f`. Archive contents, binary digest, source-path remapping and the native linker’s ad-hoc signature were verified. It has no Developer ID distribution signature and is not notarized. [Packaging recipe](releasing.md).

The exact packaged binary passed a fresh encrypted synthetic workflow: initialization, repeat import/deduplication, analysis and recurrence, confirmation/correction, vault synchronization preserving an owner annotation, search and exact-revision evidence, backup with a different key, restore retaining feedback, a new restored vault, provider exclusion/removal, integrity and zero pending cleanup. The installed binary also passed integrity/foreign-key checks on the separate real evaluation archive.

| Required v0.1 capability | Evidence and limits |
| --- | --- |
| 1. CLI | Packaged and installed binaries run the principal workflow; argument/errors and encrypted/source flows have integration coverage. |
| 2. ChatGPT export importer | Mapping JSON, ZIP and numeric ZIP shards; all branches/null roots, reconstruction, byte/entry bounds, ambiguity and conflicting-copy rejection tested. Five published real records (82 nodes / 77 messages) passed exact reconstruction and repeat import in JSON and generated ZIP/shard wrappers. [Compatibility evidence](import-compatibility.md). An original account ZIP remains unverified. |
| 3. Normalized storage | Immutable source revisions, parent/message identity, source coverage, atomic failure, migrations and foreign keys tested. Real local imports/reimports exercised. |
| 4. SQLite | Direct SQL with bundled SQLCipher 4.19.0 / SQLite 3.53.4 and Apple CommonCrypto; no database server or ORM. [Native provenance](native-dependencies.md). |
| 5. Deduplication | Repeated/older imports, new revisions, tombstones and partial-view safeguards tested; packaged repeat import and real local reimports observed. |
| 6. Local search | Current-source FTS, literal query handling and exclusion/deletion tested. Packaged search and browser search-to-source verified. |
| 7. Structured extraction | Seven typed kinds; exact eligible-user evidence, redaction, strict decoding, immutable revisions and model/daemon cache identity tested. Current local-model development evaluation passed 20/20; broader real quality remains open. |
| 8. Recurring patterns | Context-aware grouping, distinct-conversation counts and inferred-claim two-message floor tested. Synthetic recurrence observed; useful real recurrence remains unestablished. |
| 9. Personal profile | Confirmed-only generation, explicit corrections, feedback history and restored profile tested. Agent review never fabricates user confirmation. |
| 10. Decisions/timelines | Typed event stages, dated ordering and unknown timestamps tested; synthetic consideration/choice/reversal passed. Broader source interpretation still needs review. |
| 11. Daily summaries | Evidence-based activity, date boundaries and timezones tested; requested work is not represented as completed work. |
| 12. Obsidian Markdown | Linked knowledge/transcript notes, ownership hashes, atomic staging, conflicts, durable cleanup and explicit Markdown feedback tested. Packaged sync preserves owner annotations. Markdown output is explicitly generated plaintext. |
| 13. Evidence links | Exact source/revision/message/quote checks and historical retrieval tested; dashboard source dialog and transcript anchors verified. |
| 14. User feedback | Confirm/correct/reject/dismiss/supersede/reset lifecycle, stale-review guards, cleanup and retained review history tested. Packaged correction survives backup/restore. |
| 15. Minimal localhost dashboard | Loopback/bearer/Host/Origin/CSP/type/size protections and HTTP tests pass. Synthetic 1280×720 checks cover review/profile, search, partial labels, visible keyboard focus, skip navigation, announced results and source-dialog Escape/focus restoration. |

## Intelligence acceptance still open

The user authorized finding local histories, installing a local model and delegating the usefulness assessment to the agent without further questions. The current encrypted evaluation copy contains **40 conversations, 3,061 current messages and 47 retained revisions** from Claude Code, Codex, Kiro and selected ChatGPT app views. App-view pagination/truncation stays explicit; these views are not account exports. Original histories and comparison archives remain unchanged. Evaluation data and keys are private and ignored by Git.

Current deterministic rules completed all 40 sources with two candidates. The current v11 9B model run is processing that same combined corpus with exact model-manifest/daemon provenance and checkpointing. After completion, assess factual accuracy, evidence quality, duplicates, usefulness, recurrence and processing cost. The delegated top-five target (at least three accurate/useful discoveries) has **not yet passed**. No useful real recurrence or independent test-user rating has been established. Agent ratings will be labeled as such and will not promote observations to user-confirmed.

The current Qwen 3.5 9B synthetic development evaluation completed all 20 sources, produced 11 candidates and passed **20/20** in **437.95 seconds**. It checks options versus choices, reversals, assistant/quoted attribution, routine tasks, scoped correction, Portuguese preference, project/topic extraction and one-off versus repeated workflow. The cases informed development and do not provide an independent accuracy estimate. Earlier real runs exposed semantic errors despite exact quotes; those findings remain in the [history](evaluation-history.md).

No owner account-export ZIP was found in the selected local or indexed filename search. Five independently published real mapping records now pass import, exact graph/message/metadata reconstruction and repeat import, including generated ZIP/shard wrappers. This closes the absence of real-record format evidence; an original account ZIP and other export layouts remain unverified. [Detailed provenance and limits](import-compatibility.md).

## Privacy, recovery and distribution scope

Encryption includes FTS and derived database content; macOS Keychain and private recovery-key workflows are exercised. Provider/project/directory/topic/conversation exclusions apply to retained sources and future imports, preserve overlapping/manual controls, and invalidate generated content. Source/observation deletion, edited-projection conflicts, backup key rotation and retained review history have regression coverage. [Privacy matching semantics](privacy.md) · [Security boundaries](../SECURITY.md).

Three release-binary interruption checks sent SIGKILL after nonempty backup/restore/plaintext-conversion stages appeared. Originals stayed byte-identical, incomplete destinations were not published, and all retries passed integrity/search. Older 4.14/OpenSSL encrypted backups restored on 4.19/CommonCrypto without changing the original. This is observed process-interruption coverage, not exhaustive power-loss testing. [Recovery](recovery.md).

The historical 2,000-conversation / 32,000-message performance check passed on this Mac; its measured schema/version is stated in [performance.md](performance.md). Cargo-audit 0.22.2 found zero advisories/warnings in 189 locked dependencies against RustSec commit `999ec89d5d3f10f20be14750ec8c2d8b44e0ac9b` (2026-10-17). The notice inventory covers 153 Rust dependency packages, native notices, Rust library notices and the unmodified MPL source archive. Neither is an independent security/legal certification.

The advertised initial platform is this Apple Silicon macOS target. Other operating systems and narrow-viewport layouts remain unverified. CI is configured but unobserved remotely. Publishing, Developer ID signing/notarization and a repository-specific private reporting channel are separate distribution setup after the user creates a remote; this local task does not authorize publication. These limits are recorded without inventing a requirement for an independent audit, mobile application, hardware power-loss campaign or public deployment.

A collector/session pause, browser extension, background daemon, full TUI, every-provider support, mobile, cloud sync and autonomous agent-rule enforcement remain future phases. [Platform policy](platforms.md) · [Source coverage](sources.md).
