# Current release evidence

Updated 2026-10-21. **Production readiness is not established:** the full blinded model evaluation and source-grounded usefulness assessment remain open. The unchanged [product brief](product-brief.md) defines the MVP; [initial inventory](readiness-baseline.md) and [evaluation history](evaluation-history.md) retain earlier findings.

## Current implementation

Producer/prompt **v31**, encrypted schema **5**. All **203 offline tests** pass: 184 library, twelve analysis and seven CLI/source tests. Formatting, locked all-target Clippy with warnings denied and debug compilation pass. Two ignored local-model tests run separately and are not counted in 203.

Generation produces eight canonical fields and Rust derives their displayed description. Existing free-form descriptions require a separate source-free identity-consistency check. Every accepted model claim needs reasoned quotation support; surrounding context may invalidate but cannot rescue a claim absent from its citations. Administrative IDs and fixture labels are excluded from semantic verification. [Current setup and versioned evidence](local-model.md).

V27 Ministral 3 14B passed the complete blinded verifier 43/43 but only 31/32 full-pipeline cases. Real processing completed 36/40 sources in 1,260.2 seconds; four generation requests timed out. Review of the 14 visible claims found at least four unsupported or wrongly classified records, plus overlapping storage suggestions. This fails the grounding target even if all other records were accepted. Synthetic verifier success therefore does not establish real precision.

The unchanged Gemma verifier/identity policies pass 48/48 verifier cases (587.33 seconds), 13 identity cases (48.54 seconds) and the encrypted review lifecycle under v28. The complete v29 generation pass processed all 36 sources in 710.12 seconds, passing 35/36 with 17 candidate extractions. One explicit correction was omitted; its raw generation was empty, rather than rejected by grounding. The worker therefore stopped before the v29 real-history stage.

V30 supplements model generation with bounded literal proposals for explicit English feedback about an assistant's past action and a prior restriction or missing authorization. It preserves the full sentence, creates no inferred preference, skips an already-generated correction for that message, and runs the same provenance, source/context verification and reconciliation as model proposals. A praise control deliberately triggers a literal proposal that independent verification rejects. The targeted correction/routine/praise/hypothetical checks pass 4/4 in 92.23 seconds. The full 38-case generation suite and real-history acceptance remain open.

| Required v0.1 capability | Evidence and limits |
| --- | --- |
| 1. CLI | Packaged and installed binaries run the principal workflow; argument/errors and encrypted/source flows have integration coverage. |
| 2. ChatGPT export importer | Mapping JSON, ZIP and numeric ZIP shards; all branches/null roots, reconstruction, byte/entry bounds, ambiguity and conflicting-copy rejection tested. Five published real records (82 nodes / 77 messages) passed exact reconstruction and repeat import in JSON and generated ZIP/shard wrappers. [Compatibility evidence](import-compatibility.md). An original account ZIP remains unverified. |
| 3. Normalized storage | Immutable source revisions, parent/message identity, source coverage, atomic failure, migrations and foreign keys tested. Real local imports/reimports exercised. |
| 4. SQLite | Direct SQL with bundled SQLCipher 4.19.0 / SQLite 3.53.4 and Apple CommonCrypto; no database server or ORM. [Native provenance](native-dependencies.md). |
| 5. Deduplication | Repeated/older imports, new revisions, tombstones and partial-view safeguards tested; packaged repeat import and real local reimports observed. |
| 6. Local search | Current-source FTS, literal query handling and exclusion/deletion tested. Packaged search and browser search-to-source verified. |
| 7. Structured extraction | Seven typed kinds; exact eligible-user evidence, redaction, strict decoding, immutable revisions and model/daemon cache identity tested. Current pipeline and broader real quality checks are in progress. |
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

The last complete real semantic evaluation, v16, produced 36 visible claims but failed the delegated top-five assessment: **1/5 accurate and useful**. It misclassified questions and one-time instructions and added meanings absent from citations. Subsequent changes address these observed failures; the current full real-corpus evaluation is required before claiming improvement. No private quotations, model traces or keys are committed.

The owner delegated evaluation without further questions. Findings are assessed against their sources for grounding, appropriate scope/type, duplicates, temporal meaning and concrete usefulness as a memory or decision record. Ratings are **agent assessments**, not independent user research or owner endorsement. The acceptance targets are at least 90% factual grounding and three of the first five accurate and useful. Evaluation never creates user-confirmed records. The sole apparent v27 recurrence shared copied dialogue with different transient execution frames; v28 correlates those copies. Useful real recurrence has not been established; copied cumulative Kiro histories count once and remain linked to their physical sources.

## Privacy, recovery and distribution scope

Encryption includes FTS and derived database content; macOS Keychain and private recovery-key workflows are exercised. Provider/project/directory/topic/conversation exclusions apply to retained sources and future imports, preserve overlapping/manual controls, and invalidate generated content. Source/observation deletion, edited-projection conflicts, backup key rotation and retained review history have regression coverage. [Privacy matching semantics](privacy.md) · [Security boundaries](../SECURITY.md).

Three release-binary interruption checks sent SIGKILL after nonempty backup/restore/plaintext-conversion stages appeared. Originals stayed byte-identical, incomplete destinations were not published, and all retries passed integrity/search. Older 4.14/OpenSSL encrypted backups restored on 4.19/CommonCrypto without changing the original. This is observed process-interruption coverage, not exhaustive power-loss testing. [Recovery](recovery.md).

The historical 2,000-conversation / 32,000-message performance check passed on this Mac; its measured schema/version is stated in [performance.md](performance.md). Cargo-audit 0.22.2 found zero advisories/warnings in 189 locked dependencies against RustSec commit `4b587f00996ebe5c05a73eaef105ef241e876414` (2026-10-21). The notice inventory covers 153 Rust dependency packages, native notices, Rust library notices and the unmodified MPL source archive. Neither is an independent security/legal certification.

The advertised initial platform is this Apple Silicon macOS target. Other operating systems and narrow-viewport layouts remain unverified. CI is configured but unobserved remotely. Publishing, Developer ID signing/notarization and a repository-specific private reporting channel are separate distribution setup after the user creates a remote; this local task does not authorize publication. These limits are recorded without inventing a requirement for an independent audit, mobile application, hardware power-loss campaign or public deployment.

A collector/session pause, browser extension, background daemon, full TUI, every-provider support, mobile, cloud sync and autonomous agent-rule enforcement remain future phases. [Platform policy](platforms.md) · [Source coverage](sources.md).

## Review and deletion guarantees

Canonical reuse compares complete identities, requires new source support and rechecks the review/source snapshot transactionally. Explicitly distinct claims may share a quotation; ambiguous overlaps with reviewed claims remain pending. Rejection and owner corrections stay authoritative. Schema-5 opaque message-digest/span tombstones prevent deleted claims returning from unchanged overlapping evidence without retaining their deleted text. Changed text and unrelated spans remain eligible. [Identity and deletion rules](architecture.md#canonical-identity-reuse).

The v25 optimized package at commit `62d20b37033a` passes the exact-package encrypted workflow, a refreshed 2,000-conversation performance check and synthetic dashboard checks for evidence, source labels, focus restoration, confirmed-only profile and search. Archive SHA-256: `0c091cc446269daa03f6e1dc51c523c9de3382cc63aa47c8c6b9e3d8766fd462`. The v26 package at commit `64184bfa042c` also passes formatting, 195 tests, Clippy, optimized compilation, ad-hoc signature verification and the exact-package encrypted workflow. Its archive SHA-256 is `9a78fffc2e96fac436b7c808a785c4f8457c8149707ea743b8262212c9827e83` and binary SHA-256 is `15e9c1603946d6b9f0c51f894b6f95e10cebf8258b4d81b70a18a1fa83be6285`. It remains an evaluation package while the complete intelligence assessment is open. [Performance measurements](performance.md).


The v28 package at commit `34a46033e3ae` passes 200 offline tests, formatting, Clippy, optimized compilation, ad-hoc signature verification and its exact-package encrypted workflow (init, repeat import, recurrence, feedback/correction, annotation-preserving vault, historical evidence, search, rotated-key backup/restore and exclusions). The extracted binary was installed in the private local test prefix and its `--version` executed. Archive SHA-256: `636ecdd2eaba907849b890603891be7940d6056672c5f7c445e23c3ca3500494`; binary SHA-256: `cf58fe87d7baa573080d4fded4c415895ae7d3f957340b9b19f152cc0ef5129d`. This remains an evaluation package while model quality gates are open.

The exact v28 packaged dashboard also passes a fresh 1280×720 synthetic check: findings and source-revision evidence render correctly; Escape restores focus to the opener; the profile is empty before confirmation and updates after a synthetic confirmation; literal search returns six matching fixture messages and opens their immutable revision. The temporary tab and server were closed afterward. No real owner feedback changed.


V29's targeted correction/routine retest passes **3/3** in 190.13 seconds while sharing the local daemon with the v28 experiment; this is not a standalone latency baseline. The superseded v28 real run was deliberately interrupted after seven sources saved, leaving 33 pending and raw revisions unchanged. Its partial results are retained without a full quality score. A frozen v29 binary and a fresh encrypted copy of all forty sources are queued behind the complete 36-case generation suite and renewed encrypted review lifecycle. Current source commit for that run: `90c2762`.


At source commit `13f4f5d`, an in-memory comparison of six task recovery-key files and two captured ephemeral dashboard-token occurrences found zero matches in tracked files or 541 historical Git blobs (9,829,007 bytes). No secret values were printed or committed. This checks those actual task secrets, not every possible third-party credential or private quotation. No database, recovery-key, `.observr`, `.tools` or `target` paths are tracked.


V30 completed all 38 synthetic sources in 714.82 seconds and passed 37/38. It retained the transferred correction but accepted approval as a correction in the full run; therefore the worker did not start real histories. V31 adds a source-aware correction eligibility gate for explicit closing approval. It checks the complete linked sentence when a quote is clipped, permits genuine contrast/criticism to reach semantic verification, and avoids treating separate criticism as approval. The unit test covers whole/clipped approval, gratitude and mixed messages. The targeted four controls pass in 34.15 seconds after restarting the local daemon with the same Ollama version and model digest. The initial unavailable-daemon attempt processed no sources and is not a model-quality result. Full generation, renewed verifier/lifecycle checks and real acceptance remain open.


The frozen v31 CLI at code commit `db194fa15ae9` now passes the complete 38-case generation suite: **38/38**, all sources completed, 18 candidate extractions, four omitted unsupported/uncertain proposals, **649.41 seconds**. Tested executable SHA-256: `83af97d1edacf7407682451a32da79d4a91777937629527c3ca6f457a8500348`; fixture SHA-256: `5b9b4beeb3f90cbda668674aa7ccfd6174f2c19be6d6a824935f213c28d8b96a`. The separately frozen verifier and review-test executables are running next, before the forty-source archive. Real semantic acceptance is still open.
