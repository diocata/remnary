# Local model setup and verification

Current producer/prompt: **v35**; encrypted schema: **5**. Model-quality acceptance remains open. The default rules engine works without a model but only recognizes a narrow set of direct statements. This guide describes the current adapter; [historical experiments](model-evaluation-history.md) preserve retired settings and earlier results.

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

Model generation emits seven observation kinds using eight ordered canonical fields. V30 also proposes literal sentences for explicit English feedback about an assistant action performed after a prior restriction or without authorization. These bounded proposals skip an already-generated correction for that message and still require the same independent semantic verification; they cannot confirm themselves or enter the profile automatically. Rust derives the displayed description; legacy free-form descriptions require a separate identity-consistency check. Every accepted model claim then requires independent quotation support. Inferred preferences/patterns first need at least two distinct owner messages that independently support the claimed behavioral content. Adjacent but unrelated requests do not count; irrelevant evidence is removed. Tentative inference does not assert a declared or lifelong trait. Source context can invalidate an excerpt but cannot supply positive support for an uncited fact. Explicit closing approval is ineligible as a correction; linked sentence checks prevent clipping away that approval. Mixed approval and criticism still receive semantic verification. Generated scope must occur in its own citations; existing reviewed identities may retain paraphrased scopes and still require new-source verification.

Inputs contain bounded eligible user text, not raw metadata. Provider instructions, recognized pasted transcripts, subagent notifications and assistant-authored question text are excluded as personal evidence. Exact fingerprints exclude the observed Kiro steering-initialization prefix from personal evidence and preserve owner additions; raw source text remains archived. Machine-only frames cannot establish the owner reply history used to correlate copied sessions. Secret redaction is best effort. Exact provider, revision, message and quotation validation remains mandatory. Malformed, incomplete or unsupported output is not repaired into a claim.

| Operation | Reasoning | Output-token cap | Request deadline |
| --- | --- | --- | --- |
| Gemma 4 / Qwen 3.5 generation | Off | 4,096 | 240 seconds |
| Thinking-capable source verification | On | 8,192 | 240 seconds |
| Completion-only source verification | Off | 4,096 | 120 seconds |
| Identity matching | Off | 4,096 | 120 seconds |

Gemma thinking verification uses temperature 1, top-p 0.95, top-k 64 and seed 42, matching the installed model's declared sampler. Qwen thinking verification uses temperature 1, top-p 0.95, top-k 20, min-p 0, presence penalty 1.5, repetition penalty 1 and seed 42. Generation and identity completion use temperature 0. Other thinking-capable families can use reasoning during generation; they have not been qualified by the Gemma results. Fixed seeds do not guarantee identical results across runtimes.

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

The generation harness contains **39** hand-labeled cases; `--case CASE_ID` selects a targeted subset. It freezes the tested executable and reports binary/fixture hashes. The verifier has **57** cases and the identity fixture **13**, plus an encrypted review lifecycle. Administrative test labels are excluded from semantic verdict inputs. These are development regressions, not independent accuracy estimates.

V31 passed 38/38 generation cases, 48/48 verifier cases, 13/13 identity cases and the encrypted review lifecycle, but real review found only 19/23 grounded claims. V32's targeted relevance controls pass 3/3 in 39.75 seconds and its checks exclude all four disputed real findings. The patched-runtime v32 generation run passed 38/39 in 726.66 seconds, failing a valid repeated-workflow case. V33 separates per-message behavioral repetition from cross-occasion breadth, while preserving concrete project restrictions and requiring the full combined claim to pass source entailment. V33 then passed full generation 39/39, but strict verifier validation stopped its next stage on inconsistent verdict/reason fields. V34 constrains those pairs in the decoding schema and still rejects inconsistent daemon output at runtime. V34 verifier completed 54 cases with consistent output, scoring 53/54; it accepted one offered option under the overly broad topic kind. V35 prioritizes the specific deliberation role and passes five targeted classification controls, including genuine topic and consideration positives. Full v35 verification passes **57/57 in 829.53 seconds**. Full v35 generation passes **39/39 in 759.64 seconds**. Both current model review tests pass in 148.23 seconds, including 13/13 identity cases in 49.21 seconds. Fresh real-history extraction and assessment remain open.

Real acceptance uses a private encrypted forty-source corpus and agent source review under the owner's explicit delegation, without fabricated owner confirmation or independent-human ratings. Real histories bypass synthetic tracing proxies. Current results, exact package provenance and acceptance limits are in [readiness.md](readiness.md); source/format boundaries are in [sources.md](sources.md). No private corpus, model trace or key is committed.

V33 targeted verification passes **6/6 in 188.90 seconds**: two positive repeated-behavior controls and four insufficient-support controls, including concrete project scope and compound actions. This is a targeted regression result, not full acceptance.
