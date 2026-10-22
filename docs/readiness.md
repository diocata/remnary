# Release validation

## Unreleased source organization

The reorganized source passes **226 offline tests** locally: 202 library tests, one terminal-output test and 23 integration tests. The two opt-in live-model tests remain ignored in the offline suite. All 203 pre-existing Rust unit-test functions are retained. Formatting and locked all-target Clippy pass with warnings denied.

A direct synthetic comparison with the published v0.2.0 Apple Silicon executable preserves all 37 captured CLI help surfaces, nine query surfaces before and after confirmation, 26 generated vault files byte-for-byte, an owner annotation and the profile after encrypted backup/restore. The encrypted import/reimport, rules, review, vault, search, backup/restore and authenticated HTTP smoke workflow also passes. Migration bytes, Cargo.lock and model prompt constants are unchanged; no live-model accuracy claim is added by this refactor. The six-target CI workflow remains the portability gate.

## Cross-platform v0.2.0

The v0.2 release workflow builds and executes Windows, Linux and macOS packages on native x86-64 and ARM64 runners. Public tag-based publication requires all six matrix jobs plus the advisory scan to pass. Every target runs the actual packaged CLI through encryption, import/deduplication, rules/review/profile, vault, search, Unicode-path, backup/restore and authenticated dashboard smoke checks, with a freshly collected target-specific license inventory. See [platform baselines](platforms.md), [the release process](releasing.md) and [the actual workflow runs](https://github.com/diocata/remnary/actions/workflows/ci.yml).

The current local Mac suite passes **224 offline tests** after adding portable Windows/POSIX exclusion coverage. Platform-conditional tests mean exact counts can differ between runners. No new LLM quality claim is inferred from a platform port. The original 86-conversation assessment, model suites and performance checks below remain historical evidence for the named binaries; runtime portability is established separately by native CI and each release's BUILD.json.

The About description is set to “Your thoughts leave traces. We connect them. A local-first, encrypted record of your ideas, decisions and discoveries.”

---

# Historical local v0.1 acceptance evidence

Updated 2026-10-22. **The v0.1 acceptance gates pass for local use on Apple Silicon macOS 27.0.1.** The [product brief](product-brief.md), §24, defines this first release boundary. This does not implement its later roadmap or establish public-distribution or cross-platform readiness. The final local release archive is produced by the checked [release recipe](releasing.md); its `BUILD.json` and adjacent checksum identify the exact commit and binary.

## Current implementation and verification

One Rust crate, direct `rusqlite`/SQLCipher, schema **5**, Rules **explicit-rules-v7**, optional Ollama producer/prompt **36**. The pre-rename release passed **218 offline tests**: 197 library, fourteen analysis and seven CLI/source tests. The Remnary rename adds five compatibility regressions, for **223 passing offline tests**. Formatting, locked all-target Clippy with warnings denied and optimized compilation pass. No dependencies changed for Rules v7 or the topic view.

The frozen, unchanged model pipeline passes **57/57 verifier**, **39/39 generation**, **13/13 identity** and **2/2 model-review lifecycle** cases. Runtime model generation, verification, reconciliation, analysis orchestration/batching and processor behavior are unchanged from the completed producer-36 run; product names in test fixtures and comments have been renamed. The shared statement helper preserves its prior output; Rules v7 separately adds bounded observed comparison requests. [Model setup and evidence](local-model.md) · [Historical experiments](model-evaluation-history.md).

| Required v0.1 capability | Evidence and limits |
| --- | --- |
| 1. CLI | Packaged and installed binaries run the principal workflow; argument/errors and encrypted/source flows have integration coverage. |
| 2. ChatGPT export importer | Mapping JSON, ZIP and numeric ZIP shards; all branches/null roots, reconstruction, byte/entry bounds, ambiguity and conflicting-copy rejection tested. Five published real records (82 nodes / 77 messages) passed exact reconstruction and repeat import in JSON and generated ZIP/shard wrappers. [Compatibility evidence](import-compatibility.md). An original account ZIP remains unverified. |
| 3. Normalized storage | Immutable source revisions, parent/message identity, source coverage, atomic failure, migrations and foreign keys tested. Real local imports/reimports exercised. |
| 4. SQLite | Direct SQL with bundled SQLCipher 4.19.0 / SQLite 3.53.4 and Apple CommonCrypto; no database server or ORM. [Native provenance](native-dependencies.md). |
| 5. Deduplication | Repeated/older imports, new revisions, tombstones and partial-view safeguards tested; packaged repeat import and real local reimports observed. |
| 6. Local search | Current-source FTS, literal query handling and exclusion/deletion tested. Packaged search and browser search-to-source verified. |
| 7. Structured extraction | Seven typed kinds; exact eligible-user evidence, redaction, strict decoding, immutable revisions and model/daemon cache identity tested. Full model suites pass; current agent review rates 33/34 observations grounded, including one supported inferred pattern. One uncertainty-loss error remains. This is a small development corpus, not independent model accuracy research. |
| 8. Recurring patterns | Context-aware grouping, distinct-conversation counts and inferred-claim two-message floor tested. Rules v7 comparison-request pattern: four exact owner citations in two independent real conversations; synthetic copied-history/review/deletion/restore cases pass. Related-topic views keep distinct contexts separate. |
| 9. Personal profile | Confirmed-only generation, explicit corrections, feedback history and restored profile tested. Agent review never fabricates user confirmation. |
| 10. Decisions/timelines | Typed event stages, dated ordering and unknown timestamps tested; synthetic consideration/choice/reversal passed. All four retained real decision candidates were source-reviewed; event variants also have synthetic regression coverage. |
| 11. Daily summaries | Evidence-based activity, date boundaries and timezones tested; requested work is not represented as completed work. |
| 12. Obsidian Markdown | Linked knowledge/transcript notes, ownership hashes, atomic staging, conflicts, durable cleanup and explicit Markdown feedback tested. Packaged sync preserves owner annotations. Markdown output is explicitly generated plaintext. |
| 13. Evidence links | Exact source/revision/message/quote checks and historical retrieval tested; dashboard source dialog and transcript anchors verified. |
| 14. User feedback | Confirm/correct/reject/dismiss/supersede/reset lifecycle, stale-review guards, cleanup and retained review history tested. Packaged correction survives backup/restore. |
| 15. Minimal localhost dashboard | Loopback/bearer/Host/Origin/CSP/type/size protections and HTTP tests pass. Synthetic 1280×720 checks cover review/profile, search, partial labels, visible keyboard focus, skip navigation, announced results and source-dialog Escape/focus restoration. |



## Real-history acceptance

The current encrypted evaluation contains **86 conversations, 3,364 messages and 97 retained revisions**: local Claude Code/Codex/Kiro histories, selected partial ChatGPT views and fourteen Claude native views. Native captures cover every reported ordinal (140 messages, seventy owner messages), while attachments, provider message UUIDs/timestamps and unexposed context remain unavailable. All native views stay explicitly partial. Original sources and earlier revisions remain preserved. [Source boundaries](sources.md).

The combined current set has **34 observations**. Agent source review rates **32/33 explicit factual observations grounded (96.97%)**; including the supported inferred pattern, **33/34 observations are grounded (97.06%)**. All **38 evidence references** validate against exact eligible owner spans in their immutable revisions. No fabricated supporting reference is present. One candidate omits an uncertainty qualifier and is counted as an error. Known overlapping atomic/combined decisions and ambiguous reading-interest presentation limit some findings' utility; they are not credited as new useful insights.

**Four of the top five discoveries are rated useful**: the recurring comparison-request workflow signal, a vehicle preference, a travel-climate preference and a reading-friction observation. The fifth has ambiguous practical presentation. The recurring signal has **four distinct owner-message citations across two independent Claude conversations** about separate research choices. It is labelled inferred/supported and describes recorded requests without asserting an enduring preference, purchasing habit, frequency or personality trait. A broad software-design related-topic thread is accurate but not credited as useful recurrence.

The owner explicitly delegated evaluation without further questions. These are agent judgments, **not independent human research or owner confirmation**. The real confirmed profile remains empty; no feedback was fabricated to improve metrics. Grounding ≥90%, top-five usefulness ≥3/5 and useful real recurrence now pass under that delegation. Rules v7 processed all 86 sources; repeat Rules and model analysis process zero sources. Cache upgrade from the exact v6 package is verified.

## Installed artifact, UI and recovery

The optimized evaluation artifact from code commit `5c0bb475c59a` has binary SHA-256 `d4ca70f74687ea77a1549b831b999cd94280a71e5c86bc2ff5ad9af50798fcc1` and archive SHA-256 `a30a71ee5f74a493eaf0b6f24900f829ce0169a5dc592318ed2d708ffcd943fa`. Its exact extracted binary passes encrypted init, ZIP import/JSON deduplication, analysis/recurrence, profile confirmation/correction with synthetic data, annotation-preserving vault sync, search, exact revision evidence, different-key backup/restore, restored feedback, exclusions and integrity. Comparison-specific tests additionally cover Unicode paths, v6→v7 cache upgrade, source append, rejection persistence and deletion followed by fresh-source reanalysis.

Fresh native Keychain init/import/unlock succeeds with the exact optimized binary, including a Unicode data path. A read-only probe against an older ad-hoc build's Keychain item waited for OS access and was cancelled; no access setting changed. Keychain access can require owner approval. Private recovery keys provide an explicit alternative. This is not a claim of unattended access to every older Keychain ACL.

Native Firefox verifies the real pattern's inference/support labels, four citations, exact partial revision opening, Escape/focus restoration and an empty confirmed profile. The real managed Pattern note resolves all four source anchors. Synthetic review exercises use no real owner feedback. The dashboard separately renders **2,000 source rows and a 2,000-citation finding** without captured browser warnings/errors. [Synthetic performance results](performance.md).

Current optimized-binary SIGKILL checks interrupt observed backup, restore and plaintext-conversion writes. Original data is preserved; retries and integrity checks pass. This covers process interruption, not every hardware power-loss boundary. Encrypted backup/restore, migrations, locking, exclusions, feedback/deletion propagation, tombstones, edited-file conflicts and source revisions have regression coverage. [Recovery](recovery.md) · [Privacy](privacy.md).

## Security, dependencies and release limits

Encryption covers canonical conversations, FTS and derived database content. Markdown is an explicitly requested plaintext projection. Loopback/bearer/Host/Origin/CSP/type/size controls and literal rendering are tested. All adapters share best-effort secret redaction; sensitive or psychological inference is restricted and no automatic result becomes a confirmed fact. No telemetry, cloud extraction, hidden collection or external publication is enabled. [Security](../SECURITY.md).

Cargo-audit on 2026-10-22 reports **zero advisories and warnings** for 189 locked dependencies against RustSec commit `640bf5a9f06e66d8721e8772766531fe77c8630d`. All **153 target notice sets** and their files are present, including the unmodified MPL dependency source. Task-secret comparison at `5c0bb475c59a` finds no matches in tracked files or 649 historical Git blobs for six recovery-key files and seven captured dashboard-token occurrences; values are never printed. This is scoped actual-secret comparison, not a guarantee about every possible third-party credential. Private histories, keys, model traces, `.remnary`, `.tools` and `target` are ignored by Git.

The checkout action's pinned SHA matches its upstream tag. CI configuration is present; **the pre-rename evidence does not include a hosted CI run**. Local builds and installed runtime checks supply the recorded evidence. The source repository is now [diocata/remnary](https://github.com/diocata/remnary); publication does not imply a successful hosted CI run. [Rename compatibility](renaming.md) describes the new names and preserved old-format readers. Packages have the linker’s ad-hoc signature and are unnotarized. Developer ID signing, public distribution, repository-specific private security reporting and other platforms require separate setup. [Platform policy](platforms.md) · [Releasing](releasing.md) · [Changelog](changelog.md).

An original downloaded account-export ZIP and Claude consumer-export compatibility remain unverified; supported ChatGPT mapping records have real published-record and synthetic ZIP/shard evidence. The pending optional Claude export is not required for the now completed local-history evaluation. The browser extension, universal capture, production daemon, complete TUI, autonomous rule enforcement, every-provider integration, mobile and cloud sync are explicitly later phases under §24.2. [Initial inventory and section map](readiness-baseline.md).

## Remnary rename verification

The renamed build passes all 223 offline tests, formatting and locked all-target Clippy with warnings denied. Five added regressions cover old-name encrypted archives and backup restore, ambiguous archive filenames, retained partial-view metadata, managed-note marker upgrades with owner annotations, and feedback outside legacy generated sections. Only compatibility readers/tests and this migration guidance retain the former name. The database schema and application ID are unchanged; new archives, Keychain services, CLI, dashboard, vault output and packages use Remnary. Earlier model-quality, performance and exact-binary hashes above identify the pre-rename commits and remain historical measurements.

After preserving the original Kiro origin hash domain, a direct old/new CLI comparison returns the same complete set of 34 real observations, including every evidence reference and message-origin identity. Synthetic credential fixtures are assembled from compile-time fragments, preserving their redaction coverage without embedding complete token-shaped strings in source history.
