# Synthetic performance check

Measured 2026-10-22 on an Apple M4 Pro Mac with 24 GiB unified memory, Rust 1.99.1 and the optimized packaged Rules v7 executable from commit `5c0bb475c59a` (binary SHA-256 `d4ca70f74687ea77a1549b831b999cd94280a71e5c86bc2ff5ad9af50798fcc1`). This is a local engineering check, not a cross-platform guarantee or a model-quality benchmark.

```sh
python3 scripts/benchmark.py --binary /path/to/extracted/observr \
  --conversations 2000 --messages 16 --vault
```

The script creates a temporary directory containing only generated synthetic history, a private key, an encrypted archive, a vault and backup. It verifies unchanged reimport, cached analysis, unchanged vault generation and final integrity, then removes the temporary directory. It never scans real histories. `/usr/bin/time -l` supplies peak process RSS on macOS.

Input: **2,000 conversations / 32,000 messages**, 16.05 MiB JSON. The encrypted schema-5 database occupied 65.18 MiB. Integrity/foreign keys passed, with zero pending projection cleanup. Local Observr inference had finished before this check. Other desktop activity was not isolated, so these figures remain an engineering observation rather than an isolated microbenchmark.

| Operation | Wall time | Peak process RSS |
| --- | ---: | ---: |
| Import | 3.04 s | 280.44 MiB |
| Unchanged import | 0.72 s | 280.42 MiB |
| Rules analysis | 4.02 s | 145.45 MiB |
| Cached analysis | 0.17 s | 142.41 MiB |
| Search (five runs, including startup/unlock) | 75.6–76.4 ms | 12.98–13.03 MiB |
| Discover | 0.04 s | 16.28 MiB |
| Initial knowledge vault | 39.73 s | 191.89 MiB |
| Unchanged vault | 6.78 s | 196.44 MiB |
| Encrypted backup | 0.49 s | 15.45 MiB |
| Integrity check | 0.26 s | 12.91 MiB |

A separate browser check rendered 2,000 source rows and a finding with 2,000 citation articles without captured browser warnings or errors. One measured interaction took 446 ms including automation overhead; this is not an isolated UI latency benchmark.

The package also writes a Topics index and evidence-linked related-topic notes with managed source/claim dependencies. These add projection output and ownership metadata.

Current memory costs include buffering input and normalized records. Cached analysis still loads current snapshots before checking run identities; it avoids inference but is not constant-memory. Managed vault writes intentionally synchronize stages/ownership records, so initial output has a larger filesystem cost. Larger real messages, attachment metadata, many distinct claims and slow disks will change these figures. Input/analysis-window bounds and checkpointed conversation commits remain necessary.


The earlier v31 package (`4cd19f6b0339`) recorded import 4.08 seconds, rules analysis 5.80 seconds and initial vault 35.30 seconds on the same fixture size while local inference was active. The differing background activity prevents a controlled speed comparison. This benchmark verifies Rules processing, not the speed or accuracy of model extraction.
