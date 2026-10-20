# Synthetic performance check

Measured 2026-10-20 on an Apple M4 Pro Mac with 24 GiB unified memory, Rust 1.99.1 and an optimized development build. This is a local engineering check, not a cross-platform guarantee or a model-quality benchmark.

```sh
cargo build --release --locked
python3 scripts/benchmark.py --conversations 2000 --messages 16 --vault
```

The script creates a temporary directory containing only generated synthetic history, a private key, an encrypted archive, a vault and backup. It verifies unchanged reimport, cached analysis, unchanged vault generation and final integrity, then removes the temporary directory. It never scans real histories. `/usr/bin/time -l` supplies peak process RSS on macOS.

Input: **2,000 conversations / 32,000 messages**, 16.05 MiB JSON. The encrypted schema-3 database occupied 61.34 MiB. Integrity/foreign keys passed, with zero pending projection cleanup. Local-model inference ran independently during this check, so these figures are not an isolated microbenchmark.

| Operation | Wall time | Peak process RSS |
| --- | ---: | ---: |
| Import | 2.87 s | 282.38 MiB |
| Unchanged import | 0.26 s | 282.38 MiB |
| Rules analysis | 2.98 s | 147.12 MiB |
| Cached analysis | 0.17 s | 144.38 MiB |
| Search (five runs, including process startup/unlock) | 68–72 ms | 14.8–15.0 MiB |
| Discover | 40 ms | 17.91 MiB |
| Initial knowledge vault | 22.10 s | 193.39 MiB |
| Unchanged vault | 2.00 s | 194.95 MiB |
| Encrypted backup | 0.60 s | 17.39 MiB |
| Integrity check | 0.31 s | 14.92 MiB |

Current memory costs include buffering input and normalized records. Cached analysis still loads current snapshots before checking run identities; it avoids inference but is not constant-memory. Managed vault writes intentionally synchronize stages/ownership records, so initial output has a larger filesystem cost. Larger real messages, attachment metadata, many distinct claims and slow disks will change these figures. Input/analysis-window bounds and checkpointed conversation commits remain necessary.
