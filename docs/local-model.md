# Local model setup and verification

Current producer/prompt: **v29**; encrypted schema: **5**. Model-quality acceptance remains open. The default rules engine works without a model but only recognizes a narrow set of direct statements. This guide describes the current adapter; [historical experiments](model-evaluation-history.md) preserve retired settings and earlier results.

## Run a local model

The observed setup is Homebrew Ollama **0.40.6** on Apple Silicon. The installed evaluation comparator is `gemma4:12b-it-q4_K_M`: GGUF Q4_K_M, 11.9B, 8,021,618,941 bytes, manifest digest `6114515d63c17436a7c0417d82820ac65ad643e2806c5a3c89cb62846436ed0b`. It is under evaluation, not a qualified default. Observr requires an explicit model name.

```sh
brew install ollama
./scripts/ollama-local
```

In another terminal:

```sh
OLLAMA_HOST=127.0.0.1:11435 ollama pull gemma4:12b-it-q4_K_M
observr analyze --engine ollama --model gemma4:12b-it-q4_K_M \
  --endpoint http://127.0.0.1:11435 --progress
```

The download contacts the model registry. Extraction sends redacted history to the local daemon. The launcher binds loopback, disables cloud features and request-body debug logging, and limits loaded models and parallel requests to one. It runs in the foreground; Ctrl-C stops it. It installs no login service or shell-profile changes. Homebrew can change shared dependencies; the observed installation added MLX support and upgraded Python, OpenSSL and related packages.

Observr never installs or downloads a model automatically. A trusted local daemon is necessary: preflight checks reject advertised remote/cloud aliases before history is sent, but cannot attest what an arbitrary daemon executes. Keep the complete dashboard URL private while its separate server runs.

## Processing and limits

Generation emits seven observation kinds using eight ordered canonical fields. Rust derives the displayed description; legacy free-form descriptions require a separate identity-consistency check. Every accepted model claim then requires independent quotation support. Source context can invalidate an excerpt but cannot supply positive support for an uncited fact. Generated scope must occur in its own citations; existing reviewed identities may retain paraphrased scopes and still require new-source verification.

Inputs contain bounded eligible user text, not raw metadata. Provider instructions, recognized pasted transcripts, subagent notifications and assistant-authored question text are excluded as personal evidence. Secret redaction is best effort. Exact provider, revision, message and quotation validation remains mandatory. Malformed, incomplete or unsupported output is not repaired into a claim.

| Operation | Reasoning | Output-token cap | Request deadline |
| --- | --- | --- | --- |
| Gemma 4 / Qwen 3.5 generation | Off | 4,096 | 240 seconds |
| Thinking-capable source verification | On | 8,192 | 240 seconds |
| Completion-only source verification | Off | 4,096 | 120 seconds |
| Identity matching | Off | 4,096 | 120 seconds |

Gemma thinking verification uses temperature 1, top-p 0.95, top-k 64 and seed 42, matching the installed model's declared sampler. Qwen uses its separately documented author profile, preserved in the historical log. Generation and identity completion use temperature 0. Other thinking-capable families can use reasoning during generation; they have not been qualified by the Gemma results. Fixed seeds do not guarantee identical results across runtimes.

The context setting is 32,768 tokens; prompt plus conversation data is bounded to 24,000 bytes, with at most eight observations per window and 256 windows per conversation. Request and response bodies are bounded; context shifting/truncation, redirects and HTTP proxies are disabled. Only loopback HTTP is allowed, including pinned localhost resolution.

A failed window leaves that conversation pending; earlier completed sources remain committed. Manifest digest, daemon version and processor/prompt version identify cached runs. Model changes during processing leave the affected source pending. Re-running resumes pending sources. `--replace-other-models` explicitly selects the new model's completed results; it does not manufacture confirmations or clear owner reviews.

Canonical reuse requires semantic identity and new-source support, with a transactional recheck of reviews, exclusions and source heads. Opposite literal transition directions cannot merge. Rejection and corrected wording remain authoritative. Recurrence raises support, never user confirmation. [Identity and deletion rules](architecture.md#canonical-identity-reuse).

## Reproduce development checks

Start the local daemon and install the explicitly named model first:

```sh
./scripts/cargo-local test --locked --all-targets
./scripts/cargo-local build --locked
python3 scripts/evaluate-model.py --binary target/debug/observr \
  --model gemma4:12b-it-q4_K_M --endpoint http://127.0.0.1:11435
./scripts/cargo-local run --locked --example evaluate-verifier -- \
  http://127.0.0.1:11435 gemma4:12b-it-q4_K_M
OBSERVR_TEST_OLLAMA_MODEL=gemma4:12b-it-q4_K_M \
OBSERVR_TEST_OLLAMA_ENDPOINT=http://127.0.0.1:11435 \
  ./scripts/cargo-local test --locked --lib reconciliation::local_model -- \
  --ignored --nocapture --test-threads=1
```

The generation harness contains **36** hand-labeled cases; `--case CASE_ID` selects a targeted subset. It freezes the tested executable and reports binary/fixture hashes. The verifier has **48** cases and the identity fixture **13**, plus an encrypted review lifecycle. Administrative test labels are excluded from semantic verdict inputs. These are development regressions, not independent accuracy estimates.

The unchanged verifier and identity policies passed all 48 and 13 cases with Gemma under v28. The encrypted lifecycle passed too. V28 full generation completed 34/34 sources but passed 32/34 cases; two genuine corrections were omitted. V29's targeted correction/routine controls pass 3/3. The complete expanded v29 suite and fresh real-history review remain open; passing targeted controls does not close them.

Real acceptance uses a private encrypted forty-source corpus and agent source review under the owner's explicit delegation, without fabricated owner confirmation or independent-human ratings. Real histories bypass synthetic tracing proxies. Current results, exact package provenance and acceptance limits are in [readiness.md](readiness.md); source/format boundaries are in [sources.md](sources.md). No private corpus, model trace or key is committed.
