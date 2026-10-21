# Synthetic performance check

Measured 2026-10-21 on an Apple M4 Pro Mac with 24 GiB unified memory, Rust 1.99.1 and the optimized packaged v25 executable from commit `62d20b37033a` (binary SHA-256 `e35808ff24d1f937e8e63194b0ae4c4cb502379e6a121382c7b43d569548b870`). This is a local engineering check, not a cross-platform guarantee or a model-quality benchmark.

```sh
cargo build --release --locked
python3 scripts/benchmark.py --conversations 2000 --messages 16 --vault
```

The script creates a temporary directory containing only generated synthetic history, a private key, an encrypted archive, a vault and backup. It verifies unchanged reimport, cached analysis, unchanged vault generation and final integrity, then removes the temporary directory. It never scans real histories. `/usr/bin/time -l` supplies peak process RSS on macOS.

Input: **2,000 conversations / 32,000 messages**, 16.05 MiB JSON. The encrypted schema-5 database occupied 61.33 MiB. Integrity/foreign keys passed, with zero pending projection cleanup. Local-model inference ran independently during this check, so these figures are not an isolated microbenchmark.

| Operation | Wall time | Peak process RSS |
| --- | ---: | ---: |
| Import | 3.81 s | 280.41 MiB |
| Unchanged import | 0.90 s | 280.39 MiB |
| Rules analysis | 4.54 s | 145.44 MiB |
| Cached analysis | 0.19 s | 142.36 MiB |
| Search (five runs, including startup/unlock) | 86–90 ms | 12.9–13.0 MiB |
| Discover | 0.04 s | 16.02 MiB |
| Initial knowledge vault | 35.51 s | 192.50 MiB |
| Unchanged vault | 7.12 s | 192.50 MiB |
| Encrypted backup | 0.65 s | 15.33 MiB |
| Integrity check | 0.36 s | 12.91 MiB |

Current memory costs include buffering input and normalized records. Cached analysis still loads current snapshots before checking run identities; it avoids inference but is not constant-memory. Managed vault writes intentionally synchronize stages/ownership records, so initial output has a larger filesystem cost. Larger real messages, attachment metadata, many distinct claims and slow disks will change these figures. Input/analysis-window bounds and checkpointed conversation commits remain necessary.
