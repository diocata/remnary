# Local model setup and verification

The evaluated setup uses Homebrew Ollama 0.40.6 on Apple Silicon with the explicit `qwen3.5:4b-q4_K_M` model (about 3.3 GB). Model downloads contact the registry; conversation extraction uses the local daemon. No provider account/API key is needed.

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

Model/daemon updates can change behavior; keep the installed version/tag/digest with an evaluation record and rerun the synthetic checks. Smaller models may miss context or produce poor interpretations despite valid quotes. Do not bypass validation or switch to cloud processing to make a test pass.

References: [Ollama model tags](https://ollama.com/library/qwen3.5/tags), [local-only configuration](https://docs.ollama.com/faq#how-do-i-disable-ollamas-cloud-features), [structured chat API](https://docs.ollama.com/api/chat), [Homebrew package](https://formulae.brew.sh/formula/ollama).
