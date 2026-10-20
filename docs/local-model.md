# Local model setup and verification

The evaluated setup uses Homebrew Ollama 0.40.6 on Apple Silicon with the explicit `qwen3.5:4b-q4_K_M` model (about 3.3 GB). Model downloads contact the registry; conversation extraction uses the local daemon. No provider account/API key is needed. Evaluated local model digest: `sha256:d8b0f5e9760cd1682034f292d7ef72ec46f432149be0df7574bf2d6e92e38c04` (3,324,173,934 bytes).

```sh
brew install ollama
./scripts/ollama-local
# In another terminal:
OLLAMA_HOST=127.0.0.1:11435 ollama pull qwen3.5:4b-q4_K_M
observr analyze --engine ollama --model qwen3.5:4b-q4_K_M --endpoint http://127.0.0.1:11435
```

The script binds `127.0.0.1:11435`, disables cloud features/request-body debug logging and limits parallel/loaded models. It runs in the foreground; no login/background service is installed. Stop it with Ctrl-C. Homebrew may install/upgrade shared dependencies: the local setup installed MLX support and upgraded its Python, OpenSSL and related dependencies. It did not change shell profiles or start a login service.

The initial default `qwen3.5:4b` download selected an MLX/safetensors runner. On this Homebrew build that runner returned HTTP 501 (`structured output is unavailable`) because its grammar library was absent. Those requests accepted no output. The explicit GGUF variant passed synthetic extraction checks. The unused MLX model was removed; the working GGUF model remains under `~/.ollama/models`.

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
