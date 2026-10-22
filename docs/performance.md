# Synthetic performance check

Measured 2026-10-22 on an Apple M4 Pro Mac with 24 GiB unified memory, Rust 1.99.1 and the optimized packaged topic-view executable from commit `b329dc524447` (binary SHA-256 `3e1695edc50f9cc9aae4710f700f965bceb651174717007c7325000c88a976cf`). This is a local engineering check, not a cross-platform guarantee or a model-quality benchmark.

```sh
python3 scripts/benchmark.py --binary /path/to/extracted/observr \
  --conversations 2000 --messages 16 --vault
```

The script creates a temporary directory containing only generated synthetic history, a private key, an encrypted archive, a vault and backup. It verifies unchanged reimport, cached analysis, unchanged vault generation and final integrity, then removes the temporary directory. It never scans real histories. `/usr/bin/time -l` supplies peak process RSS on macOS.

Input: **2,000 conversations / 32,000 messages**, 16.05 MiB JSON. The encrypted schema-5 database occupied 65.32 MiB. Integrity/foreign keys passed, with zero pending projection cleanup. Local Observr inference had finished before this check. Other desktop activity was not isolated, so these figures remain an engineering observation rather than an isolated microbenchmark.

| Operation | Wall time | Peak process RSS |
| --- | ---: | ---: |
| Import | 3.11 s | 280.78 MiB |
| Unchanged import | 0.74 s | 280.39 MiB |
| Rules analysis | 4.07 s | 145.42 MiB |
| Cached analysis | 0.17 s | 142.39 MiB |
| Search (five runs, including startup/unlock) | 76–78 ms | 12.91–13.02 MiB |
| Discover | 0.04 s | 16.38 MiB |
| Initial knowledge vault | 40.07 s | 191.33 MiB |
| Unchanged vault | 6.89 s | 196.03 MiB |
| Encrypted backup | 0.49 s | 15.53 MiB |
| Integrity check | 0.26 s | 12.97 MiB |

The topic-view package also writes a Topics index and evidence-linked related-topic notes with managed source/claim dependencies. These add projection output and ownership metadata.

Current memory costs include buffering input and normalized records. Cached analysis still loads current snapshots before checking run identities; it avoids inference but is not constant-memory. Managed vault writes intentionally synchronize stages/ownership records, so initial output has a larger filesystem cost. Larger real messages, attachment metadata, many distinct claims and slow disks will change these figures. Input/analysis-window bounds and checkpointed conversation commits remain necessary.


The earlier v31 package (`4cd19f6b0339`) recorded import 4.08 seconds, rules analysis 5.80 seconds and initial vault 35.30 seconds on the same fixture size while local inference was active. The differing background activity prevents a controlled speed comparison. This benchmark verifies Rules processing, not the speed or accuracy of model extraction.
