# Synthetic performance check

Measured 2026-10-21 on an Apple M4 Pro Mac with 24 GiB unified memory, Rust 1.99.1 and the optimized packaged v31 executable from commit `4cd19f6b0339` (binary SHA-256 `d7a6a6363df387ccdc480273dc48a4b5d68c22bd1c7b5c4fbfa994f70bb1c7e6`). This is a local engineering check, not a cross-platform guarantee or a model-quality benchmark.

```sh
python3 scripts/benchmark.py --binary /path/to/extracted/observr \
  --conversations 2000 --messages 16 --vault
```

The script creates a temporary directory containing only generated synthetic history, a private key, an encrypted archive, a vault and backup. It verifies unchanged reimport, cached analysis, unchanged vault generation and final integrity, then removes the temporary directory. It never scans real histories. `/usr/bin/time -l` supplies peak process RSS on macOS.

Input: **2,000 conversations / 32,000 messages**, 16.05 MiB JSON. The encrypted schema-5 database occupied 61.34 MiB. Integrity/foreign keys passed, with zero pending projection cleanup. Local-model inference ran independently during this check, so these figures are not an isolated microbenchmark.

| Operation | Wall time | Peak process RSS |
| --- | ---: | ---: |
| Import | 4.08 s | 282.56 MiB |
| Unchanged import | 0.89 s | 280.94 MiB |
| Rules analysis | 5.80 s | 145.28 MiB |
| Cached analysis | 0.24 s | 142.61 MiB |
| Search (five runs, including startup/unlock) | 86–89 ms | 12.92–13.09 MiB |
| Discover | 0.04 s | 16.33 MiB |
| Initial knowledge vault | 35.30 s | 189.59 MiB |
| Unchanged vault | 6.54 s | 190.23 MiB |
| Encrypted backup | 0.60 s | 15.47 MiB |
| Integrity check | 0.29 s | 12.95 MiB |

Current memory costs include buffering input and normalized records. Cached analysis still loads current snapshots before checking run identities; it avoids inference but is not constant-memory. Managed vault writes intentionally synchronize stages/ownership records, so initial output has a larger filesystem cost. Larger real messages, attachment metadata, many distinct claims and slow disks will change these figures. Input/analysis-window bounds and checkpointed conversation commits remain necessary.


The earlier v25 package (`62d20b37033a`, binary `e35808ff24d1f937e8e63194b0ae4c4cb502379e6a121382c7b43d569548b870`) recorded import 3.81 seconds, rules analysis 4.54 seconds and initial vault 35.51 seconds on the same fixture size. These are historical observations; neither run isolates background inference or guarantees another machine's latency. The current benchmark verifies Rules processing, not the speed or accuracy of model extraction.
