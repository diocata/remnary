# Current release evidence

Updated 2026-10-20. **Production readiness is not established.** The unchanged [product brief](product-brief.md) controls scope. The detailed [initial baseline](readiness-baseline.md) is preserved; its missing-feature rows describe the initial foundation, not the current tree.

The latest full local run passed **125 tests**: 115 library, five analysis and five CLI/source integration tests. Formatting and all-target Clippy with warnings denied passed on macOS with the pinned Rust 1.99.1 toolchain. An optimized build also succeeded. CI is configured but has not run remotely; no independent security audit has been observed.

| Mandatory v0.1 capability | Current evidence / limits |
| --- | --- |
| CLI | Init/import/search/analyze/review/reports/vault/dashboard/privacy/backup commands integration-tested. Release installation checks remain. |
| ChatGPT export | Synthetic JSON/ZIP, branches, null roots, bounds and reconstruction tests. Real export compatibility unverified. |
| Storage / SQLite | Immutable revisions, identities, schema-3 migrations, foreign keys/locking tested. SQLCipher and macOS Keychain init/unlock exercised. |
| Deduplication / search | Unchanged/older imports, atomic failure, current FTS, tombstones/exclusion tested. Real local reimports exercised. |
| Extraction | Strict schema, source-constrained decoder choices, original-user evidence and versioned runs tested. Real local-model pipeline completed 35 conversations; semantic quality remains insufficient. |
| Patterns | Conservative grouping, distinct-conversation counts and labeled synthetic tests. Useful real recurrence not established. |
| Profile / feedback | Lifecycle, correction history, confirmed-only profile and restore persistence tested. Browser confirmation/Profile flow exercised. |
| Decisions / timelines | Event types/chronology and unknown dates tested. Broader semantic accuracy needs evaluation. |
| Daily digest | Timezones, dated evidence and unknown times tested. Does not claim requested work was completed. |
| Markdown knowledge | Knowledge/transcript notes, anchors, ownership, staging, conflicts and cleanup tested. Explicit Markdown feedback import has preview, atomic stale-review guards and preservation tests. |
| Evidence links | Current/historical source retrieval, exact quotes and generated links tested. Interpretation accuracy is a separate gate. |
| Dashboard | Loopback/bearer/Host/Origin/types/size/CSP and real HTTP tests. Synthetic browser checks cover correction/profile, reject/undo, search-to-revision and partial/truncated source labels. Wider viewport/accessibility acceptance remains. |

## Real-history evaluation

The user authorized finding local histories. Seventeen Claude Code files, eleven Codex rollouts and eight Kiro snapshots yielded **32 conversations**. Three selected software-related ChatGPT views were then read through connected app tools and sent directly into encryption over standard input, for **35 total**. Partial/truncated coverage is retained and cannot overwrite a full export. Original files were not modified. Private data, the encrypted evaluation archive and keys are ignored by Git.

This evaluation found and fixed provider-prompt, inherited subagent metadata, task-notification, pasted-transcript and cumulative-snapshot attribution errors. Reimport, incremental analysis and SQLCipher integrity checks succeeded. Updated rules no longer extract the identified pasted-transcript case. No useful recurring pattern or user-confirmed insight was established. A human usefulness review is pending in the private dashboard; the target (at least three of the top five useful) has **not passed**.

The user authorized a local model. Ollama 0.40.6 and Qwen 3.5 4B GGUF are installed on this 24 GiB Apple Silicon Mac. A dedicated loopback daemon reports cloud disabled. The default MLX variant could not provide structured output and was removed. After tightening grammar/citation choices, all 35 conversations completed the pipeline with **53 candidate extractions**, with no pending source-analysis failures or projection cleanup. Sources without eligible user text checkpoint empty results without model calls. Model interpretations still include exploratory questions mislabeled as preferences and one-time actions presented as preferences. These remain release-blocking quality findings, despite valid citations. [Model evaluation](local-model.md).

Schema 3 now persists active analysis heads per source revision/engine/model. Processor replacement retires obsolete automatic evidence, invalidates projections and preserves grounded human review/history. Upgrade and VACUUM regression tests pass. Distinct explicitly selected engines/models can contribute candidates concurrently; an already cached older processor is not silently reactivated.

## Open release gates

- Real ChatGPT compatibility, broader history and human accuracy/usefulness ratings.
- Provider/project/directory/topic exclusions. Conversation controls exist; pause becomes applicable with collection.
- Key rotation, deliberate plaintext upgrade, recovery interruption evidence and supported-platform policy.
- Full browser acceptance, release install/upgrade and additional concurrency/crash checks. A reproducible [2,000-conversation performance check](performance.md) passed.
- Observed CI results, full native/transitive dependency/license review, release artifacts/checksums and release instructions. The toolchain and contribution guide are now declared. Cargo-audit 0.22.2 found zero advisories/warnings in 190 locked dependencies against RustSec commit `999ec89d5d3f10f20be14750ec8c2d8b44e0ac9b` (2026-10-17). ZIP was upgraded to 9.0.5 to resolve three reported extraction advisories; Observr does not extract archives to disk. This does not audit bundled native libraries or prove security.
- Select project license and private security-reporting channel before public release.

The baseline's foundational security matrix and sign-off procedure still apply. Encryption, redaction, durable cleanup and recovery now have implementation/test evidence, but do not close every security gate. Plaintext exports and trusted-local-component limits are documented in [SECURITY.md](../SECURITY.md).

A daemon, browser/live capture, full TUI, every-provider support, mobile, cloud sync and autonomous agent-rule enforcement remain future phases. Manual Claude Code/Codex/Kiro adapters follow the user's discovery request and do not imply universal live collection. [Source matrix](sources.md).
