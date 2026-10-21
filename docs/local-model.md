# Local model setup and verification

The current development setup uses Homebrew Ollama 0.40.6 on Apple Silicon with the explicit `qwen3.5:9b-q4_K_M` model (about 6.6 GB). Model downloads contact the registry; conversation extraction uses the local daemon. No provider account/API key is needed. Local model digest: `sha256:56671c2ab9385f9cfcb404638e32cd62d88e3501d44822208363c010179a3c90` (6,550,825,550 bytes). Current quality acceptance is still under evaluation; versioned results and earlier comparators follow below.

```sh
brew install ollama
./scripts/ollama-local
# In another terminal:
OLLAMA_HOST=127.0.0.1:11435 ollama pull qwen3.5:9b-q4_K_M
observr analyze --engine ollama --model qwen3.5:9b-q4_K_M --endpoint http://127.0.0.1:11435
```

The script binds `127.0.0.1:11435`, disables cloud features/request-body debug logging and limits parallel/loaded models. It runs in the foreground; no login/background service is installed. Stop it with Ctrl-C. Homebrew may install/upgrade shared dependencies: the local setup installed MLX support and upgraded its Python, OpenSSL and related dependencies. It did not change shell profiles or start a login service.

The earlier 4B GGUF comparator is also retained (3,324,173,934 bytes; digest `sha256:d8b0f5e9760cd1682034f292d7ef72ec46f432149be0df7574bf2d6e92e38c04`). Its historical results do not establish current-pipeline quality. The initial default `qwen3.5:4b` download selected an MLX/safetensors runner. On this Homebrew build that runner returned HTTP 501 (`structured output is unavailable`) because its grammar library was absent. Those requests accepted no output. The explicit GGUF variant passed synthetic extraction checks. The unused MLX model was removed; the working GGUF model remains under `~/.ollama/models`.

The adapter preflights local weights and rejects advertised remote/cloud aliases before sending history. Prompts use bounded user-only windows; long messages overlap without changing evidence identities. Each citation is constrained to exact provider/conversation/revision/message IDs and literal source quote choices. Quotes are bounded to 1 KiB choices (sentences/lines/fragments, up to 512 per message), then checked again against both the submitted window and original unredacted source. This improves citation reliability without rewriting a model's claims after generation. Secret-redaction placeholders cannot become fabricated evidence. Schema alternatives constrain event/inference combinations; semantic validation remains mandatory.

Each conversation has a maximum of 256 windows. A failed window prevents the conversation checkpoint; earlier completed conversations stay committed. A newer completed processor version retires the previous automatic results for the same engine/model while preserving grounded human review. Different selected models/engines may contribute separate candidates.

Observed evaluation: the first permissive-generation run rejected five of 32 private conversations for non-exact quotations; 27 committed. After grounding decoder choices, all **32/32** completed with 43 candidate extractions in 346.6 seconds. Three additional partial ChatGPT views completed in 56.6 seconds with ten candidates, for **35 conversations / 53 extractions**. No-user sources checkpoint empty results without inference. These numbers describe validator acceptance, **not** semantic accuracy or usefulness. Exploratory questions and one-time actions are still sometimes mislabeled as preferences. Human review and improved semantic evaluation are required before profile confirmation or a production-quality claim.

Processor v4 restricts generation to explicit preferences, corrections and decision events (at most five per window). Known transcript introductions exclude a message from personal evidence. Recognized exploratory option questions cannot supply preference/correction citations; they can support decision considerations, or the model can abstain. This conservative English-prefix safeguard is not a complete multilingual intent classifier. Larger mixed messages may contain valid statements that it omits.

Run the development semantic evaluation with the built binary and the daemon running:

```sh
./scripts/cargo-local build --locked
python3 scripts/evaluate-model.py
```

The fixture contains ten synthetic, hand-labeled cases. Processor v4 passed 10/10 in 37.9 seconds, producing five candidate claims: scoped English/Portuguese preferences, an option consideration, a choice and a reversal. Routine commands, assistant assertions, a pasted transcript and a sensitive question produced none; an uncertain option was omitted (permitted by the fixture). All ten conversations completed. These development cases helped shape the safeguards and do not establish real-world precision or usefulness. The private real-history v3 results are retained unchanged as a comparison baseline; the original archive was not changed. A separate encrypted copy completed 35 sources with 48 v4 candidates, but ordinary execution commands were still mislabeled as preferences/corrections and a quoted question was attributed to the user. No recurring claim had two supporting conversations. The real evaluation therefore still fails the quality gate.

Processor v5 isolates the answer fields in structured question-panel replies, instead of treating the adjacent assistant-authored question as user speech. It keeps exact raw JSON string interiors (including escapes) as citation spans, without rewriting canonical history. Malformed/nontext answers and quoted transcripts cannot supply facts; separate answers are batched without duplicate message IDs. Both rules and model validation enforce this boundary.

A larger local comparator, `qwen3.5:9b-q4_K_M`, is installed (6,550,825,550 bytes; digest `sha256:56671c2ab9385f9cfcb404638e32cd62d88e3501d44822208363c010179a3c90`). With v5 it passed the same ten development cases in 54.2 seconds. The subsequent real-history run completed all 35 sources with 45 candidates (42 preferences, three corrections) and retired the 35 older Ollama result sets in the evaluation copy. It still mislabeled execution requests and inferred more than some quoted spans established; no recurring claim had two sources. A larger non-reasoning model therefore did not close the quality gate. Both installed models remain local; neither is automatically downloaded by Observr.

Processor v7 restores all seven typed extraction kinds, with at most eight candidates per window. It requests reasoning only when `/api/show` advertises `thinking`, increasing that request's total token budget to 8,192 with a bounded 240-second generation timeout and strict response bounds; metadata calls and completion-only generation retain 120 seconds. Completion-only models retain 4,096 tokens and `think: false`.

The model wire protocol orders classification/event/evidence before dependent summary fields using `a_kind` through `i_summary`. The adapter performs only a strict bijective field-name decode; missing, unknown or mixed fields fail. Values, quotes, categories and events are never repaired. Canonical records/dashboard/API field names are unchanged. This avoids generating an event placeholder before choosing its semantic kind when grammar properties are alphabetically ordered; [upstream ordering discussion](https://github.com/ollama/ollama/issues/18717). The v6 broad/reasoning experiment scored 15/18 on the expanded development set; v7 also scored 15/18 (385.8 seconds), with remaining option/reversal/project classification errors. Neither score is independent validation.

Long runs show counters automatically in interactive terminals. Use `--progress` to receive the same conversation/window progress on stderr in scripted runs. These events contain no source text/titles/quotes. Completed sources remain checkpointed; failed sources remain pending. Model tag contents can change without their name changing. Cache identity now includes the installed manifest SHA-256 and daemon version, resolved through the local inventory/version APIs. Both are checked before and after each generation; a change leaves that conversation pending. Completed prior conversations remain checkpointed under their original identity. Metadata failure stops analysis before reading targets. Exact resolved provenance is also used by `--replace-other-models`. The trusted daemon could still conceal changes or forwarding; these checks do not attest its execution.

The subsequent v8 targeted rerun passed 3/3 and its full 18-case development run passed 18/18 in 375.5 seconds. This is development evidence, not an independent accuracy estimate. Producer v10 additionally excludes recognized provider control frames, checks statement support at sentence level so mixed messages retain genuine preferences, and requires at least two distinct user-message IDs for inferred preferences/patterns. Duplicate quotes from one message do not establish recurrence. The broad typed contract still supports all seven kinds; narrow rhetorical corrections remain eligible. The expanded fixture now also checks one-off versus repeated workflows.

`evaluate-model.py --case CASE_ID` supports targeted reruns. Full reports include the loaded fixture SHA-256 and a frozen tested-executable SHA-256, so concurrent local rebuilds do not silently change an evaluation's binary. The twenty-case v9 run completed 19 sources successfully with no semantic case failures among those sources; the option-question source hit the former 120-second deadline, so the overall result is **19/20**, not a complete pass. The 240-second retry also timed out. A synthetic diagnostic with the 4B model did finish in 133.8 seconds; it exposed extensive uncertainty about the output structure because the API grammar was not fully described in the visible prompt. Processor v11 now supplies an explicit root/observation/evidence contract and example with clearly marked placeholders, retaining the same strict schema and source validation. The previously failing 9B option case then passed in **24.1 seconds**. The full twenty-case v11 run then passed **20/20** in **437.95 seconds**, with all 20 sources checkpointed and 11 candidate extractions. These are hand-labeled development cases, not an independent accuracy estimate. The broader real-corpus run remains necessary.

Discovery ranking now favors explicit corrections/preferences/decisions over generic topics when recurrence and human-confirmation status tie, before recency/hash ordering. It never fabricates timestamps or confirmation. The consumer-history trial exposed this unknown-date ranking problem; revised source-based usefulness review is still required.

When switching models, explicitly retire older automatic results after a successful run:

```sh
observr analyze --engine ollama --model qwen3.5:9b-q4_K_M --endpoint http://127.0.0.1:11435 --replace-other-models
```

Without this flag, explicitly selected models contribute concurrently. With it, a fully successful analysis selects the exact current processor/model/prompt result for each current included source with a completed run, and retires the other Ollama active results for that source. Incomplete sources, other engines, historical runs and human feedback remain intact. If analysis fails, this selection step is not applied. A later explicit switch back can select that exact cached processor result; cache lookup alone does not reactivate a retired model. Generated notes affected by replacement enter cleanup; refresh the vault explicitly.

Model/daemon updates can change behavior; keep the installed version/tag/digest with an evaluation record and rerun the synthetic checks. Smaller models may miss context or produce poor interpretations despite valid quotes. Do not bypass validation or switch to cloud processing to make a test pass.

References: [Ollama model tags](https://ollama.com/library/qwen3.5/tags), [local-only configuration](https://docs.ollama.com/faq#how-do-i-disable-ollamas-cloud-features), [structured chat API](https://docs.ollama.com/api/chat), [Homebrew package](https://formulae.brew.sh/formula/ollama).

## Producer v12: verification and scope checks

The v11 real run completed 39/40 sources with 58 extractions; one generation exceeded its deadline. An isolated current-results review contained 59 visible claims, no recurring groups and no user confirmations. Its top-five ranking still exposed questions as corrections, low-value one-off edits and unsupported claim details. It did not meet the delegated usefulness target.

Producer/prompt v12 adds a separate local verification request after strict generation/source validation and before persistence. It checks both summary and canonical value, kind/inference and scope against cited user quotations. Matching message context is supplied only to catch omitted qualifiers, negation or fictional/quoted attribution; uncited context cannot rescue a missing positive detail. Raw metadata is omitted. Missing/duplicate/unknown candidate verdicts, inconsistent reasons, absent/mismatched source context and malformed responses fail the conversation. Only fully supported candidates survive, unchanged. Unsupported/uncertain candidates are omitted, with a count reported after successful conversation commits. A failed source stays pending. Both stages check the exact model-manifest/daemon identity; no result is promoted to user-confirmed.

The verifier uses `think: false`, 4,096 tokens and a bounded 120-second deadline. A thinking-enabled eight-case development check passed 8/8 in 155.6 seconds; the expanded non-thinking/context check passed **15/15 in 16.5 seconds**, accepting five grounded controls and rejecting ten overclaims, attribution errors, mismatched decision stages and injection attempts. The fixture/harness are `tests/fixtures/verifier-semantic-evaluation.json` and `examples/evaluate-verifier.rs`:

```sh
./scripts/cargo-local run --example evaluate-verifier -- http://127.0.0.1:11435 qwen3.5:9b-q4_K_M
```

These are development regressions, not independent proof of real-world precision. The verifier can also make mistakes. The complete v12 extraction-plus-verification run finished all 20 sources in 1,021.3 seconds and scored **19/20**, with ten extractions and one verification omission. Diagnostic capture identified a generated reversal summary with the change direction inverted; the verifier correctly omitted it. A targeted non-thinking generation produced a supported reversal in 13.46 seconds plus 3.18 seconds checking. The revised pipeline and real-history assessment remain pending.

For generation only, advertised `qwen35` thinking models now receive the [model author's general thinking profile](https://huggingface.co/Qwen/Qwen3.5-9B#best-practices): temperature 1.0, top-p 0.95, top-k 20, min-p 0, presence penalty 1.5 and repetition penalty 1.0. Seed 42 records a stable requested seed; hardware/backend execution is not guaranteed deterministic. This addresses the observed greedy-generation stall without increasing the hard token/deadline limits. Other model families retain their prior sampling behavior; verification remains non-thinking/temperature 0.

Best-effort redaction now additionally removes common email addresses and international '+' phone forms. Decoder quote choices omit redaction markers and can retain an unchanged safe clause next to a redacted span. The original source is not rewritten; unchanged exact evidence is still required. Rules are `explicit-rules-v5` and skip redacted spans. This does not provide complete personal-information detection.

Short discovery lists now favor explicit preferences over single-source corrections when recurrence and confirmation tie. Within the same recurrence/confirmation/kind tier, a different conversation is shown before more findings from an already covered source. Stronger recurrence/confirmation priorities and genuine timestamps remain intact; the algorithm invents no usefulness score or dates.

The store now stops an unreconciled candidate when a different canonical identity overlaps a previously reviewed source span, including widened quotations and changed kinds. Disjoint statements remain eligible. The transaction is rolled back, so the source stays pending and the review remains intact. Automatic semantic identity reconciliation is still required; this is not a reason to reset user reviews or silently suppress unrelated facts. Schema-5 deletion provenance now blocks re-extraction from the unchanged cited spans under new wordings/kinds, without retaining deleted claim text. The remaining semantic identity work and real-quality findings keep release acceptance open.


## Producer v13: bounded requests and consistent summaries

Producer and prompt version 13 disable automatic reasoning for the evaluated `qwen35` family, using `think: false`, 4,096 output tokens and a 120-second request deadline. Other families retain capability-based behavior. The slower v12 reversal diagnostic justified this change; its separate targeted comparison is not a full-pipeline accuracy result. The complete v13 development extraction run passed 20/20 in 184.33 seconds with twelve candidate extractions. Its frozen binary SHA-256 is `9a3b4b9667eb5a60823e106f15038c7e3d7a65e275f2af0f0febdf5962b4347d`. This is development evidence, not independent accuracy or usefulness acceptance.

Generation and verification now require the summary to paraphrase the full canonical identity without adding facts absent from that identity, even when those facts appear elsewhere in a citation. Directional reversals must identify which choice was abandoned and which was selected. The expanded synthetic verifier check passed 18/18 in 19.09 seconds (six supported controls and twelve rejected errors); these are development regressions rather than an independent accuracy estimate.

Window sizing includes the actual serialized analysis envelope, source identifiers, JSON escaping and quote choices. Oversized text fragments split at UTF-8 boundaries with bounded overlap; many short messages also split before exceeding the envelope budget. A successfully reverified extraction may refresh an unreviewed automatic summary under its unchanged canonical ID. Human review and corrected text stay authoritative; an explicit reset returns wording to automatic management.


## Producer v14: conservative canonical identity reuse

Version 14 adds bounded semantic matching against existing identities, followed by source entailment of every proposed reused identity. Explicit pairwise distinction can resolve separate facts sharing a reviewed quotation. Uncertain/ambiguous reviewed overlaps stay pending. The database rechecks a knowledge/review snapshot before committing. [Identity and transaction rules](architecture.md#canonical-identity-reuse).

Offline tests cover strict relation decoding, persistent rejection under paraphrase, distinct facts with shared quotes, concurrent feedback/deletion, real conversation recurrence without confirmation and failed-reuse fallback retaining exact evidence. The local-model semantic identity fixture must be run separately from offline tests:

```sh
./scripts/cargo-local test --locked --lib reconciliation::local_model -- --ignored --nocapture --test-threads=1
```

The ten cases check paraphrases, task/project scope, consideration versus choice, negation, distinct facts, general versus scoped preferences and reversal direction. That initial v14 model check failed its broader-scope case; the v15 changes and new results below supersede it. The full v14 run was not started. Code-level tests alone do not establish semantic quality.


## Producer v15: declaration gate and quotation-only support

The v13 real run was stopped deliberately after 27/40 completed sources (608.76 seconds), rather than spending more time on a pipeline whose partial top five failed agent usefulness assessment (0/5). Its review copy contained 33 visible claims, zero recurring groups and no confirmations. Fifteen claims were labeled explicit preferences; ordinary tasks, questions, one-off constraints and speculative beliefs appeared under that kind. The audit also found wrong project types and details supplied by uncited context. This is a partial diagnostic, not a complete-corpus accuracy score or independent human rating.

V15 requires a recognizable English/Portuguese first-person preference declaration in an exact citation before an explicit preference can proceed. Recognized forms include `I prefer`, `I like`, `I dislike`, `prefiro` and `gosto de`; ordinary commands, requests for more options, hypothetical comparisons and speculative beliefs do not suffice. This is deliberately conservative and can omit valid preferences in unrecognized wording/languages. Generation requests the declaration itself in the citation; no quote is repaired.

Support verification now receives only claims and their citations. Survivors receive a separate context check if any citation omits surrounding source text. The second pass may invalidate a claim but can never restore one the first pass rejected. The initial expanded verifier development check passed 23/23 in 18.06 seconds, including the ordinary-request failures. A further expansion adds product ownership, one appointment, an unnamed project, a named project control and an unsupported book-interest claim; that 28-case result remains pending.

The v14 identity-only model check had failed the general-versus-scoped case. V15 adds an independent structural boundary for null versus non-null context and makes the prompt's scope rule explicit. Its ten identity development cases now pass in 16.77 seconds. The full extraction fixture has expanded from twenty to twenty-six cases, including genuine declared shopping preference as a positive control. Its v15 result and the clean real-corpus run remain pending.


## Producer v16: routine-command type bypass

The v15 full extraction development run completed all 26 sources in 364.61 seconds and scored 25/26. Its sole failure reclassified a routine file-creation command as a decision proposal, bypassing the preference-only gate. No v15 real run was started.

V16 filters common routine file/draft/push/continuation commands across all kinds. It retains mixed messages containing a separate preference/choice or multiple sentences for semantic checking. This is a conservative list, not a complete multilingual intent classifier. The targeted file-creation case now passes with no observation. All 182 offline tests pass (163 library, twelve analysis, seven CLI/source); the expanded verifier passed 28/28 in 24.09 seconds (seven supported controls and twenty-one rejected errors). The complete 26-case pipeline check is pending.


The complete v16 development run passed 26/26 in 356.17 seconds. The real attempt completed 38/40 in 1,380.68 seconds and both pending sources completed on retry after extending Qwen generation's hard deadline to 240 seconds while retaining 4,096 tokens and non-thinking generation. This does not establish the original failure stage; new payload-free error context identifies stages for subsequent failures. Canonical matching and verification retain their shorter bounds.

Full real coverage produced 36 visible claims. The apparent repeated Kiro questions were copied cumulative history, not independent repetitions; the reporting fix retains all evidence but counts those origins once. The current full top-five agent score is still only 1/5 accurate/useful. A private diagnostic found that stronger instructions and verdict-before-reason output order did not fix fast-verifier false acceptances. Reasoning rejected four of five disputed claims and retained the valid control, but still accepted a formatting request as a decision. No final quality/package acceptance follows from the synthetic pass.

The source-revision lookup fix also passed actual local-model lifecycle coverage, including real semantic canonical reuse across two synthetic conversations. Together with the ten identity cases, the two explicitly invoked model tests completed in 73.22 seconds. They remain separate from offline tests and from actual owner confirmations.

## Producer v17: reasoned verification and complete statements

The current policy keeps Qwen generation and canonical matching non-thinking, but enables reasoning for verification when the daemon advertises that capability. Verification runs at most four candidates per request, with 8,192 output tokens and a 240-second deadline; no reasoning text is returned or persisted by Observr. Generation remains bounded to 4,096 tokens and 240 seconds. Other structured matching calls retain their shorter deadline.

Explicit preferences must cite complete source statements, including their scope, without speculative questions. The constrained decoder uses the same statement gate as postvalidation. A conservative English/Portuguese decision-cue gate precedes semantic checking; ordinary formatting, translation, summarization and execution requests do not qualify merely because they request an action. Generated topic/context labels supply no evidence. These gates can omit valid formulations in unsupported wording or languages.

The expanded development fixtures contain 31 full-pipeline cases and 34 verifier cases. Current results are recorded in [readiness](readiness.md); historical results above apply only to their stated versions. The default evaluation command now selects the installed 9B GGUF model.

### Producer v18: one claim per verification request

The v17 verifier attempt timed out during quotation-only support, so no verifier acceptance score is claimed for that run. V18 reduces verification batches to **one candidate per request** and asks for brief reasoning. All context/quotation boundaries, strict verdict validation and token/deadline limits remain in force. A timeout leaves the source pending rather than accepting unverified output.

### Producer v19: use Qwen's thinking sampler

The v18 verifier was stopped after more than twenty minutes without a completed score; it is not an acceptance pass. Runtime metadata showed active decoding, including a single claim generating thousands of reasoning tokens. V19 keeps one-claim verification and uses Qwen35's recommended general-thinking sampler: temperature 1.0, top-p 0.95, top-k 20, min-p 0, presence penalty 1.5 and repetition penalty 1.0. A fixed seed of 42 supports local comparisons without promising exact reproducibility across runtimes. Generation and identity matching retain their separately measured non-thinking policy. The policy is selected by the daemon's advertised model family/capability, not a tag-name guess.

Source: [Qwen3.5-9B official model card, sampling guidance](https://huggingface.co/Qwen/Qwen3.5-9B#best-practices), checked 2026-10-21. The locally installed model metadata already declares the same temperature, top-p, top-k and presence penalty; Observr had overridden its temperature with greedy decoding.

### Producer v20: routine test/summary/translation evidence

The v19 verifier run stopped on a strict-JSON error: the local daemon returned a terminal response with an empty final answer for the synthetic repeated-test-commands case. An isolated rerun reproduced the empty final answer. Observr rejected it and did not parse reasoning or repair JSON. V20 adds ordinary test, summary and translation execution prefixes to the existing routine-evidence gate, independently preventing those requests from becoming preferences, decisions or inferred habits. Genuine mixed declarations and choices remain eligible. Backend response validation remains strict; future empty/malformed model answers leave the source pending.

The verifier example accepts optional case IDs after endpoint and model for focused reproduction. Reported selected cases and the whole fixture hash distinguish targeted checks from the full fixture.

### Producer v21: cheap rejection before reasoned acceptance

V20 timed out during quotation-only support on the synthetic extra-summary-fact case; its private diagnostic finished after the client deadline. V21 first runs a non-thinking quotation-only rejection check. Only survivors reach the reasoned quotation check and, when necessary, the separate reasoned source-context check. A fast verdict can omit a claim but can never accept one into the archive. This preserves the stronger acceptance boundary while avoiding expensive reasoning for obvious negatives. The targeted previously timed-out case now rejects correctly in 0.89 seconds. The whole fixture and real run are separate gates.

### Producer v22: owner identity and source-literal scope

V21 passed the 34-case verifier fixture (eight positive controls, 26 omitted negatives) in 521.58 seconds, but its full pipeline scored 25/31 in 1,172.62 seconds despite completing every source. Six required valid classifications were missing. Synthetic-only final-output tracing found generated subjects containing whole claims or object labels, an envelope coverage note copied into context, and uncited qualifiers placed in context. No reasoning text was inspected or recorded.

V22 constrains new model subjects to the literal owner identity `user` and checks it at runtime. Model input omits titles and coverage metadata; the system prompt still explains excerpt limits. Optional context is selected from bounded, literal restrictive phrases in eligible source text, with null available; source scope must remain in the complete canonical meaning. This keeps envelope prose and invented scope out of generated context choices. Semantic verification still checks that the chosen scope actually follows from the citations.

A first owner-only diagnostic scored 1/6 and exposed an independent false rejection of the ordinary paraphrase chose → selected. Verification now explicitly distinguishes generic organizing categories from additional personal facts, permits ordinary paraphrases and preserves unresolved quoted scope such as this prototype. Named files, projects, features and comparisons still require cited support. A paired regression accepts the chosen-option paraphrase and rejects consideration as a settled choice (2/2 in 30.50 seconds). The verifier fixture now contains 36 cases, and complete pipeline/real-history checks remain required.

### Producer v23: semantic evidence without administrative labels

Earlier verifier fixtures exposed human-readable case names inside evidence identifiers. Their reported scores were not blinded and are insufficient evidence of semantic accuracy. V22's six targeted full-pipeline cases still scored 2/6 (all completed), despite its selected/considering pair passing with named fixture references. This exposed a mismatch between controlled verifier inputs and actual generated claims.

V23 removes provider, conversation/revision and message identifiers from semantic requests after Rust validates the original citations. It groups exact quotes by distinct source user message in source order, attaching source context only to its own group and only on the separate context pass. Multiple excerpts of one message cannot masquerade as independent repetitions. The original stored evidence and transaction checks remain unchanged. Tests show that administrative renaming leaves the semantic request identical and that message grouping and context association survive without exposing identity.

Verifier fixture references and full-pipeline synthetic message IDs are now opaque hashes; case names/expected ratings remain outside model requests. The blind chosen-option/consideration pair passes 2/2 in 23.87 seconds. Prior scores remain historical diagnostics; the newly blinded full verifier, full pipeline and real-history assessment must establish current acceptance. No reasoning text is persisted or inspected.

### Producer v24: defer uncertainty and prevent undeclared preferences

The blinded v23 six-case pipeline scored 4/6 in 499.43 seconds. It retained the preference, chosen option, named project and technical topic, but omitted an option question and a self-described repeated workflow. Tracing showed fast verification returned uncertain for the valid consideration, while generation mislabeled workflow statements as explicit preferences despite no preference declaration.

V24 defers uncertain fast-screening verdicts to reasoned verification; unsupported fast verdicts still omit a candidate, and final acceptance still requires supported. Tests explicitly distinguish deferral from acceptance. Generation no longer offers an explicit-preference branch when the eligible text has no recognized complete preference declaration; repeated routine descriptions remain eligible for inferred behavioral patterns. The blind two-case regression passes 2/2 in 74.41 seconds. The full blinded matrix and private real-corpus assessment remain required.
