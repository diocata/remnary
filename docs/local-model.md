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

The fixture contains ten synthetic, hand-labeled cases. Processor v4 passed 10/10 in 37.9 seconds, producing five candidate claims: scoped English/Portuguese preferences, an option consideration, a choice and a reversal. Routine commands, assistant assertions, a pasted transcript and a sensitive question produced none; an uncertain option was omitted (permitted by the fixture). All ten conversations completed. These development cases helped shape the safeguards and do not establish real-world precision or usefulness. The private real-history v3 results are retained unchanged pending human review; the original archive was not changed. A separate encrypted copy completed 35 sources with 48 v4 candidates, but ordinary execution commands were still mislabeled as preferences/corrections and a quoted question was attributed to the user. No recurring claim had two supporting conversations. The real evaluation therefore still fails the quality gate.

Processor v5 isolates the answer fields in structured question-panel replies, instead of treating the adjacent assistant-authored question as user speech. It keeps exact raw JSON string interiors (including escapes) as citation spans, without rewriting canonical history. Malformed/nontext answers and quoted transcripts cannot supply facts; separate answers are batched without duplicate message IDs. Both rules and model validation enforce this boundary.

A larger local comparator, `qwen3.5:9b-q4_K_M`, is installed (6,550,825,550 bytes; digest `sha256:56671c2ab9385f9cfcb404638e32cd62d88e3501d44822208363c010179a3c90`). With v5 it passed the same ten development cases in 54.2 seconds. The subsequent real-history run completed all 35 sources with 45 candidates (42 preferences, three corrections) and retired the 35 older Ollama result sets in the evaluation copy. It still mislabeled execution requests and inferred more than some quoted spans established; no recurring claim had two sources. A larger non-reasoning model therefore did not close the quality gate. Both installed models remain local; neither is automatically downloaded by Observr.

When switching models, explicitly retire older automatic results after a successful run:

```sh
observr analyze --engine ollama --model qwen3.5:9b-q4_K_M --endpoint http://127.0.0.1:11435 --replace-other-models
```

Without this flag, explicitly selected models contribute concurrently. With it, a fully successful analysis selects the exact current processor/model/prompt result for each current included source with a completed run, and retires the other Ollama active results for that source. Incomplete sources, other engines, historical runs and human feedback remain intact. If analysis fails, this selection step is not applied. A later explicit switch back can select that exact cached processor result; cache lookup alone does not reactivate a retired model. Generated notes affected by replacement enter cleanup; refresh the vault explicitly.

Model/daemon updates can change behavior; keep the installed version/tag/digest with an evaluation record and rerun the synthetic checks. Smaller models may miss context or produce poor interpretations despite valid quotes. Do not bypass validation or switch to cloud processing to make a test pass.

References: [Ollama model tags](https://ollama.com/library/qwen3.5/tags), [local-only configuration](https://docs.ollama.com/faq#how-do-i-disable-ollamas-cloud-features), [structured chat API](https://docs.ollama.com/api/chat), [Homebrew package](https://formulae.brew.sh/formula/ollama).
