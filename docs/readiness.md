# Current release evidence

Updated 2026-10-21. **Production readiness is not established:** the full blinded model evaluation and source-grounded usefulness assessment remain open. The unchanged [product brief](product-brief.md) defines the MVP; [initial inventory](readiness-baseline.md) and [evaluation history](evaluation-history.md) retain earlier findings.

## Current implementation

Producer/prompt **v26**, encrypted schema **5**. All **195 offline tests** pass: 176 library, twelve analysis and seven CLI/source tests. Formatting, locked all-target Clippy with warnings denied and debug compilation pass. Two ignored local-model tests run separately and are not counted in 195.

Generation produces eight canonical fields and Rust derives their displayed description. Existing free-form descriptions require a separate source-free identity-consistency check. Every accepted model claim needs reasoned quotation support; surrounding context may invalidate but cannot rescue a claim absent from its citations. Administrative IDs and fixture labels are excluded from semantic verification. [Current setup and versioned evidence](local-model.md).

The targeted v25 legacy-description controls pass 2/2, and the previously missed technical-topic pipeline case passes 1/1. Its broader six-source run completed but passed 4/6, exposing option/choice stage errors and redundant project findings. V26 clarifies those classifications and adds corresponding negative regressions. These are development regressions, not an independent quality estimate. The complete blinded checks contain 31 full-pipeline cases, 39 verifier cases and ten identity cases; their current complete results remain pending. The targeted v26 classification run passes 3/3 in 316.16 seconds; the fresh real-corpus run is in progress.

| Required v0.1 capability | Evidence and limits |
| --- | --- |
| 1. CLI | Packaged and installed binaries run the principal workflow; argument/errors and encrypted/source flows have integration coverage. |
| 2. ChatGPT export importer | Mapping JSON, ZIP and numeric ZIP shards; all branches/null roots, reconstruction, byte/entry bounds, ambiguity and conflicting-copy rejection tested. Five published real records (82 nodes / 77 messages) passed exact reconstruction and repeat import in JSON and generated ZIP/shard wrappers. [Compatibility evidence](import-compatibility.md). An original account ZIP remains unverified. |
| 3. Normalized storage | Immutable source revisions, parent/message identity, source coverage, atomic failure, migrations and foreign keys tested. Real local imports/reimports exercised. |
| 4. SQLite | Direct SQL with bundled SQLCipher 4.19.0 / SQLite 3.53.4 and Apple CommonCrypto; no database server or ORM. [Native provenance](native-dependencies.md). |
| 5. Deduplication | Repeated/older imports, new revisions, tombstones and partial-view safeguards tested; packaged repeat import and real local reimports observed. |
| 6. Local search | Current-source FTS, literal query handling and exclusion/deletion tested. Packaged search and browser search-to-source verified. |
| 7. Structured extraction | Seven typed kinds; exact eligible-user evidence, redaction, strict decoding, immutable revisions and model/daemon cache identity tested. Current v26 model and broader real quality checks are in progress. |
| 8. Recurring patterns | Context-aware grouping, distinct-conversation counts and inferred-claim two-message floor tested. Synthetic recurrence observed; useful real recurrence remains unestablished. |
| 9. Personal profile | Confirmed-only generation, explicit corrections, feedback history and restored profile tested. Agent review never fabricates user confirmation. |
| 10. Decisions/timelines | Typed event stages, dated ordering and unknown timestamps tested; synthetic consideration/choice/reversal passed. Broader source interpretation still needs review. |
| 11. Daily summaries | Evidence-based activity, date boundaries and timezones tested; requested work is not represented as completed work. |
| 12. Obsidian Markdown | Linked knowledge/transcript notes, ownership hashes, atomic staging, conflicts, durable cleanup and explicit Markdown feedback tested. Packaged sync preserves owner annotations. Markdown output is explicitly generated plaintext. |
| 13. Evidence links | Exact source/revision/message/quote checks and historical retrieval tested; dashboard source dialog and transcript anchors verified. |
| 14. User feedback | Confirm/correct/reject/dismiss/supersede/reset lifecycle, stale-review guards, cleanup and retained review history tested. Packaged correction survives backup/restore. |
| 15. Minimal localhost dashboard | Loopback/bearer/Host/Origin/CSP/type/size protections and HTTP tests pass. Synthetic 1280×720 checks cover review/profile, search, partial labels, visible keyboard focus, skip navigation, announced results and source-dialog Escape/focus restoration. |

## Real-history acceptance

The private encrypted corpus contains **40 conversations, 3,061 current messages and 47 retained revisions** from Claude Code, Codex, Kiro and selected ChatGPT app views. App-view pagination, truncation and unknown timestamps remain explicit. Original histories are unchanged. No owner account-export ZIP was found; independently published real ChatGPT records provide importer compatibility evidence, including generated ZIP/shard wrappers. [Source compatibility](import-compatibility.md).

The last complete real semantic evaluation, v16, produced 36 visible claims but failed the delegated top-five assessment: **1/5 accurate and useful**. It misclassified questions and one-time instructions and added meanings absent from citations. Subsequent changes address these observed failures; a fresh v26 evaluation is required before claiming improvement. No private quotations, model traces or keys are committed.

The owner delegated evaluation without further questions. Findings are assessed against their sources for grounding, appropriate scope/type, duplicates, temporal meaning and concrete usefulness as a memory or decision record. Ratings are **agent assessments**, not independent user research or owner endorsement. The acceptance targets are at least 90% factual grounding and three of the first five accurate and useful. Evaluation never creates user-confirmed records. Useful real recurrence has not been established; copied cumulative Kiro histories count once and remain linked to their physical sources.

## Privacy, recovery and distribution scope

Encryption includes FTS and derived database content; macOS Keychain and private recovery-key workflows are exercised. Provider/project/directory/topic/conversation exclusions apply to retained sources and future imports, preserve overlapping/manual controls, and invalidate generated content. Source/observation deletion, edited-projection conflicts, backup key rotation and retained review history have regression coverage. [Privacy matching semantics](privacy.md) · [Security boundaries](../SECURITY.md).

Three release-binary interruption checks sent SIGKILL after nonempty backup/restore/plaintext-conversion stages appeared. Originals stayed byte-identical, incomplete destinations were not published, and all retries passed integrity/search. Older 4.14/OpenSSL encrypted backups restored on 4.19/CommonCrypto without changing the original. This is observed process-interruption coverage, not exhaustive power-loss testing. [Recovery](recovery.md).

The historical 2,000-conversation / 32,000-message performance check passed on this Mac; its measured schema/version is stated in [performance.md](performance.md). Cargo-audit 0.22.2 found zero advisories/warnings in 189 locked dependencies against RustSec commit `4b587f00996ebe5c05a73eaef105ef241e876414` (2026-10-21). The notice inventory covers 153 Rust dependency packages, native notices, Rust library notices and the unmodified MPL source archive. Neither is an independent security/legal certification.

The advertised initial platform is this Apple Silicon macOS target. Other operating systems and narrow-viewport layouts remain unverified. CI is configured but unobserved remotely. Publishing, Developer ID signing/notarization and a repository-specific private reporting channel are separate distribution setup after the user creates a remote; this local task does not authorize publication. These limits are recorded without inventing a requirement for an independent audit, mobile application, hardware power-loss campaign or public deployment.

A collector/session pause, browser extension, background daemon, full TUI, every-provider support, mobile, cloud sync and autonomous agent-rule enforcement remain future phases. [Platform policy](platforms.md) · [Source coverage](sources.md).

## Review and deletion guarantees

Canonical reuse compares complete identities, requires new source support and rechecks the review/source snapshot transactionally. Explicitly distinct claims may share a quotation; ambiguous overlaps with reviewed claims remain pending. Rejection and owner corrections stay authoritative. Schema-5 opaque message-digest/span tombstones prevent deleted claims returning from unchanged overlapping evidence without retaining their deleted text. Changed text and unrelated spans remain eligible. [Identity and deletion rules](architecture.md#canonical-identity-reuse).

The v25 optimized package at commit `62d20b37033a` passes the exact-package encrypted workflow, a refreshed 2,000-conversation performance check and synthetic dashboard checks for evidence, source labels, focus restoration, confirmed-only profile and search. Archive SHA-256: `0c091cc446269daa03f6e1dc51c523c9de3382cc63aa47c8c6b9e3d8766fd462`. It predates v26 classification changes; final current packaging remains open. [Performance measurements](performance.md).
